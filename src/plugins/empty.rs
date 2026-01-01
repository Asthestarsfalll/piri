use anyhow::{Context, Result};
use log::{debug, info, warn};
use niri_ipc::Event;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::EmptyPluginConfig;
use crate::niri::NiriIpc;

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

    /// Get workspace identifier from config entry  
    fn parse_workspace_identifier(workspace: &str) -> WorkspaceIdentifier {
        if let Ok(idx) = workspace.parse::<u8>() {
            WorkspaceIdentifier::Idx(idx)
        } else {
            WorkspaceIdentifier::Name(workspace.to_string())
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
                                let config_guard = self.config.lock().await;

                                // Try direct match first (by idx)
                                let mut command_found = false;
                                if let Some(command) = config_guard.workspaces.get(&workspace_key) {
                                    let cmd = command.clone();
                                    drop(config_guard);
                                    info!("Workspace {} is empty (WorkspaceActivated), executing command: {}", 
                                              workspace_key, cmd);
                                    Self::execute_command(&workspace_key, &cmd).await?;
                                    command_found = true;
                                } else if let Some(name) = &focused_ws.name {
                                    // Try matching by name (workspace name from niri)
                                    if let Some(command) = config_guard.workspaces.get(name) {
                                        let cmd = command.clone();
                                        drop(config_guard);
                                        info!("Workspace {} (name: {}) matched by name in WorkspaceActivated, executing command: {}", 
                                                  workspace_key, name, cmd);
                                        Self::execute_command(&workspace_key, &cmd).await?;
                                        command_found = true;
                                    } else {
                                        drop(config_guard);
                                    }
                                } else {
                                    drop(config_guard);
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

    /// Try to match workspace by name and idx, then execute if empty
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
        let current_workspace = Self::parse_workspace_identifier(workspace_key);

        debug!(
            "Trying to match workspace '{}' (parsed as {:?}) against {} configured rules",
            workspace_key,
            current_workspace,
            workspaces_map.len()
        );

        // First pass: try name matching
        debug!("First pass: trying name-based matching");
        for (key, command) in &workspaces_map {
            let key_identifier = Self::parse_workspace_identifier(key);
            let command = command.clone();

            let matches = match (&current_workspace, &key_identifier) {
                (WorkspaceIdentifier::Name(a), WorkspaceIdentifier::Name(b)) => {
                    debug!("  Comparing name '{}' with name '{}'", a, b);
                    a == b
                }
                (WorkspaceIdentifier::Name(name), WorkspaceIdentifier::Idx(key_idx)) => {
                    debug!("  Comparing name '{}' with idx '{}'", name, key_idx);
                    name == &key_idx.to_string()
                }
                (WorkspaceIdentifier::Idx(idx), WorkspaceIdentifier::Name(name)) => {
                    debug!("  Comparing idx '{}' with name '{}'", idx, name);
                    name == &idx.to_string()
                }
                _ => false,
            };

            if matches {
                info!(
                    "Workspace {} matched by name with config key '{}', executing command: {}",
                    workspace_key, key, command
                );
                Self::execute_command(workspace_key, &command).await?;
                workspace_empty.lock().await.insert(workspace_key.to_string(), true);
                return Ok(());
            }
        }

        // Second pass: try idx matching
        debug!("Second pass: trying idx-based matching");
        for (key, command) in &workspaces_map {
            let key_identifier = Self::parse_workspace_identifier(key);
            let command = command.clone();

            let matches = match (&current_workspace, &key_identifier) {
                (WorkspaceIdentifier::Idx(a), WorkspaceIdentifier::Idx(b)) => {
                    debug!("  Comparing idx '{}' with idx '{}'", a, b);
                    a == b
                }
                _ => false,
            };

            if matches {
                info!(
                    "Workspace {} matched by idx with config key '{}', executing command: {}",
                    workspace_key, key, command
                );
                Self::execute_command(workspace_key, &command).await?;
                workspace_empty.lock().await.insert(workspace_key.to_string(), true);
                return Ok(());
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

        // Execute command
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to execute command: {}", command))?;

        info!(
            "Command executed successfully for workspace {} (PID: {})",
            workspace_key,
            output.id()
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

/// Workspace identifier enum to handle different identifier types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum WorkspaceIdentifier {
    Idx(u8),
    Id(u64),
    Name(String),
}

impl WorkspaceIdentifier {
    fn to_string(&self) -> String {
        match self {
            WorkspaceIdentifier::Idx(idx) => idx.to_string(),
            WorkspaceIdentifier::Id(id) => id.to_string(),
            WorkspaceIdentifier::Name(name) => name.clone(),
        }
    }
}
