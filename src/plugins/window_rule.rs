use anyhow::Result;
use log::info;
use niri_ipc::Event;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::{Config, WindowRuleConfig};
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use crate::plugins::FromConfig;

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

/// Window rule plugin that moves windows to workspaces based on app_id and title matching
pub struct WindowRulePlugin {
    niri: NiriIpc,
    config: WindowRulePluginConfig,
    /// Window matcher cache for regex pattern matching
    matcher_cache: Arc<WindowMatcherCache>,
    /// Last window ID that triggered focus command
    last_focused_window: Option<u64>,
    /// Last time a focus command was executed
    last_execution_time: Option<Instant>,
}

impl WindowRulePlugin {
    /// Execute focus command with de-duplication
    async fn execute_focus_rule(&mut self, window_id: u64, focus_command: &str) -> Result<()> {
        let now = Instant::now();
        if let (Some(last_id), Some(last_time)) =
            (self.last_focused_window, self.last_execution_time)
        {
            if last_id == window_id && now.duration_since(last_time) < Duration::from_millis(200) {
                return Ok(());
            }
        }

        info!(
            "Executing focus_command for window {}: {}",
            window_id, focus_command
        );
        window_utils::execute_command(focus_command)?;

        self.last_focused_window = Some(window_id);
        self.last_execution_time = Some(now);
        Ok(())
    }

    /// Handle focus command execution for currently focused window
    async fn handle_focus_command(&mut self, window_id: u64) -> Result<()> {
        let windows = self.niri.get_windows().await?;
        let window = windows
            .into_iter()
            .find(|w| w.id == window_id)
            .ok_or_else(|| anyhow::anyhow!("Focused window {} not found", window_id))?;

        let rules = self.config.rules.clone();
        for rule in &rules {
            if let Some(ref focus_command) = rule.focus_command {
                let matcher = WindowMatcher::new(rule.app_id.clone(), rule.title.clone());
                if self
                    .matcher_cache
                    .matches(window.app_id.as_ref(), Some(&window.title), &matcher)
                    .await?
                {
                    self.execute_focus_rule(window_id, focus_command).await?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn handle_window_opened(&mut self, window: &niri_ipc::Window) -> Result<()> {
        let rules = self.config.rules.clone();
        for rule in &rules {
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
                    self.execute_focus_rule(window.id, focus_command).await?;
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
            last_execution_time: None,
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
        Ok(())
    }
}
