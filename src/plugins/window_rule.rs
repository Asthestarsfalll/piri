use anyhow::{Context, Result};
use log::{debug, info, warn};
use niri_ipc::Event;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{WindowRuleConfig, WindowRulePluginConfig};
use crate::niri::NiriIpc;

/// Window rule plugin that moves windows to workspaces based on app_id and title matching
pub struct WindowRulePlugin {
    niri: NiriIpc,
    /// Shared config that can be updated without restarting the event listener
    config: Arc<Mutex<WindowRulePluginConfig>>,
    /// Compiled regex patterns cache
    regex_cache: Arc<Mutex<HashMap<String, Regex>>>,
}

impl WindowRulePlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None).expect("Failed to initialize niri IPC"),
            config: Arc::new(Mutex::new(WindowRulePluginConfig::default())),
            regex_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get or compile a regex pattern (with caching)
    async fn get_regex(
        pattern: &str,
        regex_cache: &Arc<Mutex<HashMap<String, Regex>>>,
    ) -> Result<Regex> {
        let mut cache = regex_cache.lock().await;
        if let Some(regex) = cache.get(pattern) {
            return Ok(regex.clone());
        }

        let regex = Regex::new(pattern)
            .with_context(|| format!("Failed to compile regex pattern: {}", pattern))?;
        cache.insert(pattern.to_string(), regex.clone());
        Ok(regex)
    }

    /// Parse workspace identifier from config entry (same as empty plugin)
    fn parse_workspace_identifier(workspace: &str) -> WorkspaceIdentifier {
        if let Ok(idx) = workspace.parse::<u8>() {
            WorkspaceIdentifier::Idx(idx)
        } else {
            WorkspaceIdentifier::Name(workspace.to_string())
        }
    }

    /// Match workspace by name and idx (same logic as empty plugin)
    async fn match_workspace(target_workspace: &str, niri: &NiriIpc) -> Result<Option<String>> {
        let niri_clone = niri.clone();
        let workspaces_result =
            tokio::task::spawn_blocking(move || niri_clone.get_workspaces_for_mapping()).await;

        let workspaces = match workspaces_result {
            Ok(Ok(ws)) => ws,
            Ok(Err(e)) => {
                debug!("Failed to get workspaces: {}", e);
                return Ok(None);
            }
            Err(e) => {
                debug!("Task join error: {}", e);
                return Ok(None);
            }
        };

        let target_identifier = Self::parse_workspace_identifier(target_workspace);

        // First pass: try name matching
        for workspace in &workspaces {
            let workspace_identifier: WorkspaceIdentifier = if let Some(ref name) = workspace.name {
                WorkspaceIdentifier::Name(name.clone())
            } else {
                WorkspaceIdentifier::Idx(workspace.idx)
            };

            let matches = match (&target_identifier, &workspace_identifier) {
                (WorkspaceIdentifier::Name(a), WorkspaceIdentifier::Name(b)) => a == b,
                (WorkspaceIdentifier::Name(name), WorkspaceIdentifier::Idx(key_idx)) => {
                    name == &key_idx.to_string()
                }
                (WorkspaceIdentifier::Idx(idx), WorkspaceIdentifier::Name(name)) => {
                    name == &idx.to_string()
                }
                (WorkspaceIdentifier::Idx(a), WorkspaceIdentifier::Idx(b)) => a == b,
            };

            if matches {
                // Return workspace identifier for moving window
                let workspace_key: String = if let Some(ref name) = workspace.name {
                    name.clone()
                } else {
                    workspace.idx.to_string()
                };
                debug!(
                    "Matched workspace by name: target={:?}, found={}",
                    target_identifier, workspace_key
                );
                return Ok(Some(workspace_key));
            }
        }

        // Second pass: try idx matching
        for workspace in &workspaces {
            let matches = match (&target_identifier, &workspace.idx) {
                (WorkspaceIdentifier::Idx(a), b) => a == b,
                _ => false,
            };

            if matches {
                let workspace_key = workspace.idx.to_string();
                debug!(
                    "Matched workspace by idx: target={:?}, found={}",
                    target_identifier, workspace_key
                );
                return Ok(Some(workspace_key));
            }
        }

        debug!("No matching workspace found for: {:?}", target_identifier);
        Ok(None)
    }

    /// Check if a window matches a rule
    async fn matches_rule(
        window: &niri_ipc::Window,
        rule: &WindowRuleConfig,
        regex_cache: &Arc<Mutex<HashMap<String, Regex>>>,
    ) -> Result<bool> {
        // Check app_id match (if specified)
        if let Some(ref app_id_pattern) = rule.app_id {
            if let Some(ref window_app_id) = window.app_id {
                let regex = Self::get_regex(app_id_pattern, regex_cache).await?;
                if regex.is_match(window_app_id) {
                    debug!(
                        "Window {} matched rule by app_id: {} matches {}",
                        window.id, window_app_id, app_id_pattern
                    );
                    return Ok(true);
                }
            }
        }

        // Check title match (if specified)
        if let Some(ref title_pattern) = rule.title {
            if let Some(ref window_title) = window.title {
                let regex = Self::get_regex(title_pattern, regex_cache).await?;
                if regex.is_match(window_title) {
                    debug!(
                        "Window {} matched rule by title: {} matches {}",
                        window.id, window_title, title_pattern
                    );
                    return Ok(true);
                }
            }
        }

        // If both app_id and title are specified, match if either matches (OR logic)
        // If only one is specified, it must match
        Ok(false)
    }

    /// Handle a single event (internal implementation)
    async fn handle_event_internal(&self, event: &Event, niri: &NiriIpc) -> Result<()> {
        match event {
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
                    match Self::matches_rule(&window, rule, &self.regex_cache).await {
                        Ok(true) => {
                            info!(
                                "Window {} matched rule: app_id={:?}, title={:?}, workspace={}",
                                window.id, rule.app_id, rule.title, rule.open_on_workspace
                            );

                            // Match workspace (name first, then idx)
                            match Self::match_workspace(&rule.open_on_workspace, niri).await {
                                Ok(Some(workspace)) => {
                                    // Move window to workspace
                                    let niri_clone = niri.clone();
                                    let window_id = window.id;
                                    let workspace_clone = workspace.clone();

                                    tokio::task::spawn_blocking(move || {
                                        niri_clone
                                            .move_window_to_workspace(window_id, &workspace_clone)
                                    })
                                    .await
                                    .context("Task join error")?
                                    .context("Failed to move window to workspace")?;

                                    info!(
                                        "Successfully moved window {} to workspace {}",
                                        window.id, workspace
                                    );
                                    // Only apply the first matching rule
                                    return Ok(());
                                }
                                Ok(None) => {
                                    warn!(
                                        "Window {} matched rule but workspace '{}' not found",
                                        window.id, rule.open_on_workspace
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to match workspace '{}' for window {}: {}",
                                        rule.open_on_workspace, window.id, e
                                    );
                                }
                            }
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

        // Log all configured rules
        if !config_guard.rules.is_empty() {
            info!("Configured window rules:");
            for (i, rule) in config_guard.rules.iter().enumerate() {
                info!(
                    "  Rule {}: app_id={:?}, title={:?}, workspace={}",
                    i + 1,
                    rule.app_id,
                    rule.title,
                    rule.open_on_workspace
                );
            }
        } else {
            warn!("Window rule plugin initialized but no rules configured");
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
        info!("Window rule plugin shutdown");
        Ok(())
    }

    async fn handle_event(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
        self.handle_event_internal(event, niri).await
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(event, Event::WindowOpenedOrChanged { .. })
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
        self.regex_cache.lock().await.clear();

        // Log all configured rules
        if !config_guard.rules.is_empty() {
            info!("Updated window rules:");
            for (i, rule) in config_guard.rules.iter().enumerate() {
                info!(
                    "  Rule {}: app_id={:?}, title={:?}, workspace={}",
                    i + 1,
                    rule.app_id,
                    rule.title,
                    rule.open_on_workspace
                );
            }
        } else {
            warn!("Window rule plugin config updated but no rules configured");
        }
        drop(config_guard);

        Ok(())
    }
}

/// Workspace identifier enum (same as empty plugin)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum WorkspaceIdentifier {
    Idx(u8),
    Name(String),
}
