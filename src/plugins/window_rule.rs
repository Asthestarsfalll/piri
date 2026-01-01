use anyhow::{Context, Result};
use log::{debug, info, warn};
use niri_ipc::Event;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::config::{WindowRuleConfig, WindowRulePluginConfig};
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};

/// Window rule plugin that moves windows to workspaces based on app_id and title matching
pub struct WindowRulePlugin {
    niri: NiriIpc,
    /// Shared config that can be updated without restarting the event listener
    config: Arc<Mutex<WindowRulePluginConfig>>,
    /// Window matcher cache for regex pattern matching
    matcher_cache: Arc<WindowMatcherCache>,
    /// Last window ID that triggered focus command
    last_focused_window: Option<u64>,
    /// Last time a focus command was executed
    last_execution_time: Option<Instant>,
}

impl WindowRulePlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None),
            config: Arc::new(Mutex::new(WindowRulePluginConfig::default())),
            matcher_cache: Arc::new(WindowMatcherCache::new()),
            last_focused_window: None,
            last_execution_time: None,
        }
    }

    /// Log window rules (helper to avoid code duplication)
    fn log_rules(rules: &[WindowRuleConfig], prefix: &str) {
        if !rules.is_empty() {
            info!("{} window rules:", prefix);
            for (i, rule) in rules.iter().enumerate() {
                info!(
                    "  Rule {}: app_id={:?}, title={:?}, workspace={:?}, focus_command={:?}",
                    i + 1,
                    rule.app_id,
                    rule.title,
                    rule.open_on_workspace,
                    rule.focus_command
                );
            }
        } else {
            warn!(
                "Window rule plugin {} but no rules configured",
                prefix.to_lowercase()
            );
        }
    }

    /// Handle focus command execution for currently focused window
    async fn handle_focus_command(&mut self, niri: &NiriIpc, window_id: u64) -> Result<()> {
        // De-duplication: skip if same window focused within 200ms
        let now = Instant::now();
        if let (Some(last_id), Some(last_time)) =
            (self.last_focused_window, self.last_execution_time)
        {
            if last_id == window_id && now.duration_since(last_time) < Duration::from_millis(200) {
                debug!("Skipping duplicate focus command for window {}", window_id);
                return Ok(());
            }
        }

        let windows = niri.get_windows().await?;
        let window = windows.into_iter().find(|w| w.id == window_id).ok_or_else(|| {
            debug!("Focused window {} not found", window_id);
            anyhow::anyhow!("Focused window not found")
        })?;

        let config_guard = self.config.lock().await;
        let rules = config_guard.rules.clone();
        drop(config_guard);

        for rule in rules.iter() {
            let matcher = WindowMatcher::new(rule.app_id.clone(), rule.title.clone());
            if self
                .matcher_cache
                .matches(window.app_id.as_ref(), Some(&window.title), &matcher)
                .await?
            {
                if let Some(ref focus_command) = rule.focus_command {
                    info!(
                        "Executing focus_command for window {}: {}",
                        window_id, focus_command
                    );
                    window_utils::execute_command(focus_command)?;

                    // Update tracking for de-duplication
                    self.last_focused_window = Some(window_id);
                    self.last_execution_time = Some(Instant::now());

                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Handle a single event (internal implementation)
    async fn handle_event_internal(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowFocusChanged { id } => {
                debug!("Received WindowFocusChanged event: id={:?}", id);
                if let Some(window_id) = id {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    if let Err(e) = self.handle_focus_command(niri, *window_id).await {
                        warn!("Failed to handle focus_command: {}", e);
                    }
                }
            }
            Event::WindowOpenedOrChanged { window } => {
                debug!(
                    "Received WindowOpenedOrChanged event: id={}, app_id={:?}, title={:?}",
                    window.id, window.app_id, window.title
                );

                let config_guard = self.config.lock().await;
                let rules = config_guard.rules.clone();
                drop(config_guard);

                // Check each rule
                for rule in &rules {
                    let matcher = WindowMatcher::new(rule.app_id.clone(), rule.title.clone());
                    match self
                        .matcher_cache
                        .matches(window.app_id.as_ref(), window.title.as_ref(), &matcher)
                        .await
                    {
                        Ok(true) => {
                            debug!(
                                "Window {} matched rule: app_id={:?}, title={:?}",
                                window.id, rule.app_id, rule.title
                            );
                            info!(
                                "Window {} matched rule: workspace={:?}, focus_command={:?}",
                                window.id, rule.open_on_workspace, rule.focus_command
                            );

                            // Move window to workspace if open_on_workspace is specified
                            if let Some(ref open_on_workspace) = rule.open_on_workspace {
                                // Match workspace (exact name first, then exact idx)
                                match window_utils::match_workspace(open_on_workspace, niri.clone())
                                    .await
                                {
                                    Ok(Some(workspace)) => {
                                        // Check if window is already in the target workspace
                                        let target_workspace_id =
                                            niri.get_workspaces_for_mapping().await.ok().and_then(
                                                |workspaces: Vec<niri_ipc::Workspace>| {
                                                    workspaces
                                                        .iter()
                                                        .find(|ws| {
                                                            ws.idx.to_string() == workspace
                                                                || ws
                                                                    .name
                                                                    .as_ref()
                                                                    .map(|n| n == &workspace)
                                                                    .unwrap_or(false)
                                                        })
                                                        .map(|ws| ws.id)
                                                },
                                            );

                                        // If window is already in target workspace, skip moving
                                        if let (Some(window_ws_id), Some(target_ws_id)) =
                                            (window.workspace_id, target_workspace_id)
                                        {
                                            if window_ws_id == target_ws_id {
                                                debug!(
                                                "Window {} is already in target workspace {} (id: {}), skipping move",
                                                window.id, workspace, target_ws_id
                                            );
                                                // Only apply the first matching rule
                                                break;
                                            }
                                        }

                                        // Move window to workspace
                                        niri.move_window_to_workspace(window.id, &workspace)
                                            .await
                                            .context("Failed to move window to workspace")?;

                                        info!(
                                            "Successfully moved window {} to workspace {}",
                                            window.id, workspace
                                        );

                                        // Focus the moved window
                                        tokio::time::sleep(tokio::time::Duration::from_millis(100))
                                            .await;
                                        if let Err(e) =
                                            window_utils::focus_window(niri.clone(), window.id)
                                                .await
                                        {
                                            warn!(
                                                "Failed to focus window {} after moving: {}",
                                                window.id, e
                                            );
                                        } else {
                                            info!(
                                                "Focused window {} after moving to workspace {}",
                                                window.id, workspace
                                            );
                                        }

                                        // Only apply the first matching rule
                                        break;
                                    }
                                    Ok(None) => {
                                        warn!(
                                            "Window {} matched rule but workspace '{}' not found",
                                            window.id, open_on_workspace
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to match workspace '{}' for window {}: {}",
                                            open_on_workspace, window.id, e
                                        );
                                    }
                                }
                            }
                            // If no open_on_workspace, continue to check other rules or focus_command
                        }
                        Ok(false) => {
                            // No match, continue to next rule
                        }
                        Err(e) => {
                            warn!("Error matching rule for window {}: {}", window.id, e);
                        }
                    }
                }
            }
            Event::WorkspaceActivated { .. } => {
                // Workspace activation will trigger WindowFocusChanged event,
                // so we don't need to handle focus_command here to avoid duplicate execution
                debug!("Received WorkspaceActivated event");
            }
            other => {
                // Log other events for debugging
                debug!("Received other event: {:?}", other);
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for WindowRulePlugin {
    fn name(&self) -> &str {
        "window_rule"
    }

    async fn init(&mut self, niri: NiriIpc, config: &crate::config::Config) -> Result<()> {
        // Store niri instance first, then clone from self for the async task
        self.niri = niri;

        // Get window rule plugin config
        if let Some(window_rule_config) = config.get_window_rule_plugin_config() {
            *self.config.lock().await = window_rule_config;
        }

        let config_guard = self.config.lock().await;
        let rule_count = config_guard.rules.len();

        info!("Window rule plugin initialized with {} rules", rule_count);

        Self::log_rules(&config_guard.rules, "Configured");
        drop(config_guard);

        // Event listener is now handled by PluginManager
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Window rule plugin shutdown");
        Ok(())
    }

    async fn handle_event(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
        self.handle_event_internal(event, niri).await
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowOpenedOrChanged { .. } | Event::WindowFocusChanged { .. }
        )
    }

    async fn update_config(&mut self, niri: NiriIpc, config: &crate::config::Config) -> Result<()> {
        info!("Updating window rule plugin configuration");

        // Update niri instance
        self.niri = niri;

        // Get new window rule plugin config
        let mut config_guard = self.config.lock().await;
        let old_count = config_guard.rules.len();

        if let Some(window_rule_config) = config.get_window_rule_plugin_config() {
            *config_guard = window_rule_config;
        }

        let new_count = config_guard.rules.len();
        info!(
            "Window rule plugin config updated: {} -> {} rules",
            old_count, new_count
        );

        // Clear regex cache when config changes
        self.matcher_cache.clear_cache().await;

        Self::log_rules(&config_guard.rules, "Updated");
        drop(config_guard);

        Ok(())
    }
}
