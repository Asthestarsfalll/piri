use anyhow::Result;
use log::{debug, info, warn};
use niri_ipc::Event;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::EmptyPluginConfig;
use crate::niri::NiriIpc;
use crate::plugins::window_utils;

/// Empty plugin that executes commands when switching to empty workspaces
pub struct EmptyPlugin {
    niri: NiriIpc,
    /// Shared config that can be updated without restarting the event listener
    config: Arc<Mutex<EmptyPluginConfig>>,
    /// Last focused workspace (idx as string for comparison)
    last_workspace: Arc<Mutex<Option<String>>>,
    /// Map of workspace identifier to whether it's empty (per-workspace state)
    workspace_empty: Arc<Mutex<HashMap<String, bool>>>,
}

impl EmptyPlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None).expect("Failed to initialize niri IPC"),
            config: Arc::new(Mutex::new(EmptyPluginConfig::default())),
            last_workspace: Arc::new(Mutex::new(None)),
            workspace_empty: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Handle a single event (internal implementation)
    async fn handle_event_internal(&self, event: &Event, niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WorkspaceActivated { id, focused } => {
                if !focused {
                    // Only process when workspace is focused
                    return Ok(());
                }

                debug!(
                    "Received WorkspaceActivated event: id={}, focused={}",
                    id, focused
                );

                // WorkspaceActivated only gives us the workspace id, not the full workspace info
                // We need to query the current workspace state to determine idx and if it's empty
                let niri_clone = niri.clone();
                let workspaces_result =
                    tokio::task::spawn_blocking(move || niri_clone.get_workspaces()).await;

                match workspaces_result {
                    Ok(Ok(workspaces)) => {
                        // Find the workspace with matching id
                        if let Some(focused_ws) =
                            workspaces.into_iter().find(|ws| ws.id == *id && ws.is_focused)
                        {
                            let workspace_key = focused_ws.idx.to_string();
                            let is_empty = focused_ws.active_window_id.is_none();

                            debug!("WorkspaceActivated - Current workspace: id={}, idx={}, name={:?}, is_empty={}, active_window_id={:?}", 
                                   focused_ws.id, focused_ws.idx, focused_ws.name, is_empty, focused_ws.active_window_id);

                            // WorkspaceActivated event means workspace has switched
                            // Update last workspace
                            let mut last_ws = self.last_workspace.lock().await;
                            let old_workspace = last_ws.clone();
                            *last_ws = Some(workspace_key.clone());
                            drop(last_ws);

                            info!(
                                "Workspace activated: {:?} -> {} (is_empty={})",
                                old_workspace, workspace_key, is_empty
                            );

                            // Update empty state
                            self.workspace_empty
                                .lock()
                                .await
                                .insert(workspace_key.clone(), is_empty);

                            // Execute command if workspace is empty
                            if is_empty {
                                // Get current config (may have been updated)
                                // Try exact match: first by name, then by idx
                                let mut command_found = false;

                                // First: exact name match
                                if let Some(name) = &focused_ws.name {
                                    let config_guard = self.config.lock().await;
                                    if let Some(command) = config_guard.workspaces.get(name) {
                                        let cmd = command.clone();
                                        drop(config_guard);
                                        info!("Workspace {} (name: {}) matched by exact name, executing command: {}", 
                                                  workspace_key, name, cmd);
                                        Self::execute_command(&workspace_key, &cmd).await?;
                                        command_found = true;
                                    } else {
                                        drop(config_guard);
                                    }
                                }

                                // Second: exact idx match (if name match failed)
                                if !command_found {
                                    let config_guard = self.config.lock().await;
                                    if let Some(command) =
                                        config_guard.workspaces.get(&workspace_key)
                                    {
                                        let cmd = command.clone();
                                        drop(config_guard);
                                        info!("Workspace {} matched by exact idx, executing command: {}", 
                                                  workspace_key, cmd);
                                        Self::execute_command(&workspace_key, &cmd).await?;
                                        command_found = true;
                                    } else {
                                        drop(config_guard);
                                    }
                                }

                                if !command_found {
                                    debug!("Workspace {} (name: {:?}) is empty (WorkspaceActivated), trying name/idx matching", 
                                              workspace_key, focused_ws.name);
                                    Self::try_match_and_execute(
                                        &workspace_key,
                                        true,
                                        niri,
                                        &self.config,
                                        &self.workspace_empty,
                                    )
                                    .await?;
                                }
                            }
                        } else {
                            debug!(
                                "WorkspaceActivated: workspace with id {} not found or not focused",
                                id
                            );
                        }
                    }
                    Ok(Err(e)) => {
                        debug!("WorkspaceActivated: failed to get workspaces: {}", e);
                    }
                    Err(e) => {
                        debug!("WorkspaceActivated: task join error: {}", e);
                    }
                }
            }
            other => {
                // Log other events for debugging
                debug!("Received other event: {:?}", other);
            }
        }

        Ok(())
    }

    /// Try to match workspace by exact name or idx, then execute if empty
    async fn try_match_and_execute(
        workspace_key: &str,
        is_empty: bool,
        _niri: &NiriIpc,
        config: &Arc<Mutex<EmptyPluginConfig>>,
        workspace_empty: &Arc<Mutex<HashMap<String, bool>>>,
    ) -> Result<()> {
        if !is_empty {
            debug!(
                "Workspace {} is not empty, skipping matching",
                workspace_key
            );
            workspace_empty.lock().await.insert(workspace_key.to_string(), false);
            return Ok(());
        }

        let config_guard = config.lock().await;
        let workspaces_map = config_guard.workspaces.clone();
        drop(config_guard);

        debug!(
            "Trying to match workspace '{}' against {} configured rules",
            workspace_key,
            workspaces_map.len()
        );

        // First pass: exact name match
        debug!("First pass: trying exact name matching");
        for (key, command) in &workspaces_map {
            // Exact name match only
            if key == workspace_key {
                info!(
                    "Workspace {} matched by exact name with config key '{}', executing command: {}",
                    workspace_key, key, command
                );
                Self::execute_command(workspace_key, command).await?;
                workspace_empty.lock().await.insert(workspace_key.to_string(), true);
                return Ok(());
            }
        }

        // Second pass: exact idx match
        debug!("Second pass: trying exact idx matching");
        if let Ok(workspace_idx) = workspace_key.parse::<u8>() {
            for (key, command) in &workspaces_map {
                if let Ok(key_idx) = key.parse::<u8>() {
                    if key_idx == workspace_idx {
                        info!(
                            "Workspace {} matched by exact idx with config key '{}', executing command: {}",
                            workspace_key, key, command
                        );
                        Self::execute_command(workspace_key, command).await?;
                        workspace_empty.lock().await.insert(workspace_key.to_string(), true);
                        return Ok(());
                    }
                }
            }
        }

        debug!("No matching rule found for workspace '{}'", workspace_key);
        Ok(())
    }

    /// Execute command
    async fn execute_command(workspace_key: &str, command: &str) -> Result<()> {
        info!(
            "Executing command for empty workspace {}: {}",
            workspace_key, command
        );

        window_utils::execute_command(command)?;

        info!(
            "Command executed successfully for workspace {}",
            workspace_key
        );
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for EmptyPlugin {
    fn name(&self) -> &str {
        "empty"
    }

    async fn init(&mut self, niri: NiriIpc, config: &crate::config::Config) -> Result<()> {
        // Store niri instance first, then clone from self for the async task
        self.niri = niri;

        // Get empty plugin config (converts new format to old format)
        if let Some(empty_config) = config.get_empty_plugin_config() {
            *self.config.lock().await = empty_config;
        }

        let config_guard = self.config.lock().await;
        let rule_count = config_guard.workspaces.len();

        info!(
            "Empty plugin initialized with {} workspace rules",
            rule_count
        );

        // Log all configured workspace rules
        if !config_guard.workspaces.is_empty() {
            info!("Configured workspace rules:");
            for (workspace, command) in &config_guard.workspaces {
                info!("  - {}: {}", workspace, command);
            }
        } else {
            warn!("Empty plugin initialized but no workspace rules configured");
        }
        drop(config_guard);

        // Event listener is now handled by PluginManager
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        // Event-driven plugin, no polling needed
        // The event listener is started in init() and runs in a separate task
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Shutdown is handled by runtime
        info!("Empty plugin shutdown");
        Ok(())
    }

    async fn handle_event(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
        self.handle_event_internal(event, niri).await
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(event, Event::WorkspaceActivated { .. })
    }

    async fn update_config(&mut self, niri: NiriIpc, config: &crate::config::Config) -> Result<()> {
        info!("Updating empty plugin configuration");

        // Update niri instance
        self.niri = niri;

        // Get new empty plugin config
        let mut config_guard = self.config.lock().await;
        let old_count = config_guard.workspaces.len();

        if let Some(empty_config) = config.get_empty_plugin_config() {
            *config_guard = empty_config;
        }

        let new_count = config_guard.workspaces.len();
        info!(
            "Empty plugin config updated: {} -> {} workspace rules",
            old_count, new_count
        );

        // Log all configured workspace rules
        if !config_guard.workspaces.is_empty() {
            info!("Updated workspace rules:");
            for (workspace, command) in &config_guard.workspaces {
                info!("  - {}: {}", workspace, command);
            }
        } else {
            warn!("Empty plugin config updated but no workspace rules configured");
        }
        drop(config_guard);

        Ok(())
    }
}
