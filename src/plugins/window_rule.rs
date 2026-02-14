use anyhow::{Ok, Result};
use log::{debug, info};
use niri_ipc::Event;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{Config, WindowRuleConfig};
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use crate::plugins::FromConfig;
use crate::utils::Throttle;

/// Window rule plugin config (for internal use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRulePluginConfig {
    /// List of window rules
    pub rules: Vec<WindowRuleConfig>,
}

impl Default for WindowRulePluginConfig {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
}

impl FromConfig for WindowRulePluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        if config.window_rule.is_empty() {
            None
        } else {
            Some(Self {
                rules: config.window_rule.clone(),
            })
        }
    }
}
struct ExecuteRules {
    lose_focus: HashSet<usize>,
    focus: HashSet<usize>,
}
impl ExecuteRules {
    pub fn clear(&mut self) {
        self.lose_focus.clear();
        self.focus.clear();
    }
}
impl Default for ExecuteRules {
    fn default() -> Self {
        ExecuteRules {
            lose_focus: HashSet::new(),
            focus: HashSet::new(),
        }
    }
}
/// Window rule plugin that moves windows to workspaces based on app_id and title matching
pub struct WindowRulePlugin {
    niri: NiriIpc,
    config: WindowRulePluginConfig,
    /// Window matcher cache for regex pattern matching
    matcher_cache: Arc<WindowMatcherCache>,
    /// Last window ID that triggered focus command
    last_focused_window: Option<u64>,
    /// Throttle for focus command execution
    execution_throttle: Throttle,
    /// Set of rule indices that have already executed focus_command (when focus_command_once is true)
    executed_rules: ExecuteRules,
    /// Last window ID that was processed by handle_focus_command (for throttling)
    last_handled_window: Option<u64>,
    /// Throttle for handle_focus_command
    handle_throttle: Throttle,
}

impl WindowRulePlugin {
    /// Execute focus command with de-duplication
    async fn execute_focus_rule(
        &mut self,
        window_id: u64,
        focus_command: &str,
        rule_index: usize,
        focus_once: bool,
    ) -> Result<()> {
        // If focus_once is true and this rule has already executed focus_command, skip
        if focus_once && self.executed_rules.focus.contains(&rule_index) {
            return Ok(());
        }

        // Global throttle: prevent executing focus_command too frequently regardless of window ID
        if self.execution_throttle.check_and_update(Duration::from_millis(200)) {
            info!(
                "Executing focus_command for window {}: {}",
                window_id, focus_command
            );
            window_utils::execute_command(focus_command)?;

            // Mark this rule as having executed focus_command if focus_once is true
            if focus_once {
                self.executed_rules.focus.insert(rule_index);
            }

            self.last_focused_window = Some(window_id);
        }

        Ok(())
    }
    /// Execute lose focus command with de-duplication
    async fn execute_lose_focus_rule(
        &mut self,
        window_id: u64,
        lose_focus_command: &str,
        rule_index: usize,
        lose_focus_once: bool,
    ) -> Result<()> {
        if lose_focus_once && self.executed_rules.lose_focus.contains(&rule_index) {
            return Ok(());
        }
        // Global throttle: prevent executing focus_command too frequently regardless of window ID
        if self.execution_throttle.check_and_update(Duration::from_millis(200)) {
            info!(
                "Executing lose_focus_command for window {}: {}",
                window_id, lose_focus_command
            );
            window_utils::execute_command(lose_focus_command)?;

            // Mark this rule as having executed focus_command if focus_once is true
            if lose_focus_once {
                self.executed_rules.lose_focus.insert(rule_index);
            }

            self.last_focused_window = Some(window_id);
        }
        Ok(())
    }
    /// Handle focus command execution for currently focused window
    async fn handle_focus_command(&mut self, window_id: u64) -> Result<()> {
        // Check if this is a programmatic focus change (e.g., from auto_fill)
        if window_utils::should_ignore_focus_change().await {
            debug!(
                "Ignoring programmatic focus change for window {}",
                window_id
            );
            return Ok(());
        }

        // Global throttle: prevent processing focus changes too frequently
        if !self.handle_throttle.check_and_update(Duration::from_millis(200)) {
            return Ok(());
        }

        // Update tracking before processing
        let windows = self.niri.get_windows().await?;

        let mut last_window = None;
        // Get the ID of the last handled window
        if let Some(last_window_id) = self.last_handled_window {
            if last_window_id != window_id {
                last_window = windows.clone().into_iter().find(|w| w.id == last_window_id);
            }
        }

        self.last_handled_window = Some(window_id);

        let window = match windows.into_iter().find(|w| w.id == window_id) {
            Some(w) => w,
            None => {
                // Window not found - this is normal when a window is closing or has just closed
                // Silently return instead of erroring
                return Ok(());
            }
        };

        let rules = self.config.rules.clone();
        for (rule_index, rule) in rules.iter().enumerate() {
            if let Some(window) = last_window.clone() {
                if let Some(ref lose_focus_command) = rule.lose_focus_command {
                    let matcher = WindowMatcher::new(rule.app_id.clone(), rule.title.clone());
                    if self
                        .matcher_cache
                        .matches(window.app_id.as_ref(), Some(&window.title), &matcher)
                        .await?
                    {
                        self.execute_lose_focus_rule(
                            window_id,
                            lose_focus_command,
                            rule_index,
                            rule.focus_command_once,
                        )
                        .await?;
                    }
                }
            }
            if let Some(ref focus_command) = rule.focus_command {
                let matcher = WindowMatcher::new(rule.app_id.clone(), rule.title.clone());
                if self
                    .matcher_cache
                    .matches(window.app_id.as_ref(), Some(&window.title), &matcher)
                    .await?
                {
                    self.execute_focus_rule(
                        window_id,
                        focus_command,
                        rule_index,
                        rule.focus_command_once,
                    )
                    .await?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn handle_window_opened(&mut self, window: &niri_ipc::Window) -> Result<()> {
        let rules = self.config.rules.clone();
        for (rule_index, rule) in rules.iter().enumerate() {
            let matcher = WindowMatcher::new(rule.app_id.clone(), rule.title.clone());
            if self
                .matcher_cache
                .matches(window.app_id.as_ref(), window.title.as_ref(), &matcher)
                .await?
            {
                // 1. Move to workspace if specified
                if let Some(ref workspace_name) = rule.open_on_workspace {
                    if let Some(matched_ws) =
                        window_utils::match_workspace(workspace_name, self.niri.clone()).await?
                    {
                        // Check if already there
                        let current_workspaces = self.niri.get_workspaces_for_mapping().await?;
                        let is_already_there = current_workspaces.iter().any(|ws| {
                            ws.id == window.workspace_id.unwrap_or(0)
                                && (ws.name.as_ref() == Some(&matched_ws)
                                    || ws.idx.to_string() == matched_ws)
                        });

                        if !is_already_there {
                            info!("Moving window {} to workspace {}", window.id, matched_ws);
                            self.niri.move_window_to_workspace(window.id, &matched_ws).await?;
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            let _ = window_utils::focus_window(self.niri.clone(), window.id).await;
                        }
                    }
                }

                // 2. Execute focus command if specified (unified de-duplication)
                if let Some(ref focus_command) = rule.focus_command {
                    self.execute_focus_rule(
                        window.id,
                        focus_command,
                        rule_index,
                        rule.focus_command_once,
                    )
                    .await?;
                }

                // Only apply the first matching rule
                break;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for WindowRulePlugin {
    type Config = WindowRulePluginConfig;

    fn new(niri: NiriIpc, config: WindowRulePluginConfig) -> Self {
        info!(
            "Window rule plugin initialized with {} rules",
            config.rules.len()
        );
        Self {
            niri,
            config,
            matcher_cache: Arc::new(WindowMatcherCache::new()),
            last_focused_window: None,
            execution_throttle: Throttle::new(),
            executed_rules: ExecuteRules::default(),
            last_handled_window: None,
            handle_throttle: Throttle::new(),
        }
    }

    async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowFocusChanged {
                id: Some(window_id),
            } => {
                tokio::time::sleep(Duration::from_millis(10)).await;
                self.handle_focus_command(*window_id).await?;
            }
            Event::WindowOpenedOrChanged { window } => {
                self.handle_window_opened(window).await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowOpenedOrChanged { .. } | Event::WindowFocusChanged { id: Some(_) }
        )
    }

    async fn update_config(&mut self, config: WindowRulePluginConfig) -> Result<()> {
        info!(
            "Updating window rule plugin configuration: {} rules",
            config.rules.len()
        );
        self.config = config;
        self.matcher_cache.clear_cache().await;
        // Clear executed rules tracking since rule indices may have changed
        self.executed_rules.clear();
        Ok(())
    }
}
