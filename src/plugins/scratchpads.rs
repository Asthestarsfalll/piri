use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{Config, Direction, ScratchpadConfig};
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use crate::plugins::FromConfig;
use crate::utils::send_notification;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadsPluginConfig {
    pub scratchpads: HashMap<String, ScratchpadConfig>,
    pub default_size: String,
    pub default_margin: u32,
    pub move_to_workspace: Option<String>,
}

impl Default for ScratchpadsPluginConfig {
    fn default() -> Self {
        Self {
            scratchpads: HashMap::new(),
            default_size: "75% 60%".to_string(),
            default_margin: 50,
            move_to_workspace: None,
        }
    }
}

impl FromConfig for ScratchpadsPluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        // Scratchpads plugin is always enabled if not explicitly disabled,
        // because it can be used dynamically via IPC even without initial config.
        Some(Self {
            scratchpads: config.scratchpads.clone(),
            default_size: config.piri.scratchpad.default_size.clone(),
            default_margin: config.piri.scratchpad.default_margin,
            move_to_workspace: config.piri.scratchpad.move_to_workspace.clone(),
        })
    }
}

#[derive(Debug, Clone)]
struct ScratchpadState {
    window_id: Option<u64>,
    is_visible: bool,
    previous_focused_window: Option<u64>,
    config: ScratchpadConfig,
    is_dynamic: bool,
}

struct ScratchpadManager {
    niri: NiriIpc,
    states: HashMap<String, ScratchpadState>,
    pub matcher_cache: Arc<WindowMatcherCache>,
}

impl ScratchpadManager {
    fn new(niri: NiriIpc) -> Self {
        Self {
            niri,
            states: HashMap::new(),
            matcher_cache: Arc::new(WindowMatcherCache::new()),
        }
    }

    async fn get_target_position(
        &self,
        config: &ScratchpadConfig,
        window_width: u32,
        window_height: u32,
        is_visible: bool,
    ) -> Result<(i32, i32)> {
        let (output_width, output_height) = self.niri.get_output_size().await?;

        let (x, y) = if is_visible {
            window_utils::calculate_position(
                config.direction,
                output_width,
                output_height,
                window_width,
                window_height,
                config.margin,
            )
        } else {
            window_utils::calculate_hide_position(
                config.direction,
                output_width,
                output_height,
                window_width,
                window_height,
                config.margin,
            )
        };
        Ok((x, y))
    }

    async fn get_target_geometry(
        &self,
        config: &ScratchpadConfig,
        is_visible: bool,
    ) -> Result<(i32, i32, u32, u32)> {
        let (output_width, output_height) = self.niri.get_output_size().await?;
        let (width_ratio, height_ratio) = config.parse_size()?;
        let window_width = (output_width as f64 * width_ratio) as u32;
        let window_height = (output_height as f64 * height_ratio) as u32;

        let (x, y) = self
            .get_target_position(config, window_width, window_height, is_visible)
            .await?;
        Ok((x, y, window_width, window_height))
    }

    async fn setup_window(&mut self, window_id: u64, config: &ScratchpadConfig) -> Result<()> {
        debug!("Setting up window {} as scratchpad", window_id);
        self.niri.set_window_floating(window_id, true).await?;

        let (hide_x, hide_y, width, height) = self.get_target_geometry(config, false).await?;
        self.niri.resize_floating_window(window_id, width, height).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (current_x, current_y, _, _) = self
            .niri
            .get_window_position_async(window_id)
            .await?
            .context("Failed to get window position")?;

        window_utils::move_window_to_position(
            &self.niri, window_id, current_x, current_y, hide_x, hide_y,
        )
        .await?;
        Ok(())
    }

    async fn sync_state(
        &mut self,
        name: &str,
        global_move_to_workspace: Option<String>,
    ) -> Result<()> {
        let (mut config, is_visible, window_id, is_dynamic) = {
            let state = self.states.get_mut(name).context("State not found")?;
            (
                state.config.clone(),
                state.is_visible,
                state.window_id.context("Window ID not found")?,
                state.is_dynamic,
            )
        };

        if is_visible {
            // Move to current workspace if needed
            self.niri.move_floating_window(window_id).await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Get current position and size
        let (current_x, current_y, current_width, current_height) = self
            .niri
            .get_window_position_async(window_id)
            .await?
            .context("Failed to get window position")?;

        // For dynamic scratchpads, update margin from current position before hiding
        if is_dynamic && !is_visible {
            let (output_width, output_height) = self.niri.get_output_size().await?;
            let new_margin = window_utils::extract_margin(
                config.direction,
                output_width,
                output_height,
                current_width,
                current_height,
                current_x,
                current_y,
            );
            debug!(
                "Updating dynamic scratchpad '{}' margin to {}",
                name, new_margin
            );
            config.margin = new_margin;
            // Update state with new margin
            if let Some(state) = self.states.get_mut(name) {
                state.config.margin = new_margin;
            }
        }

        let (target_x, target_y, target_width, target_height) = if is_dynamic {
            // For dynamic scratchpads, use current size to calculate target position
            let (tx, ty) = self
                .get_target_position(&config, current_width, current_height, is_visible)
                .await?;
            (tx, ty, current_width, current_height)
        } else {
            // For configured scratchpads, use config size
            self.get_target_geometry(&config, is_visible).await?
        };

        // Only resize for non-dynamic scratchpads when showing
        if is_visible && !is_dynamic {
            self.niri.resize_floating_window(window_id, target_width, target_height).await?;
        }

        window_utils::move_window_to_position(
            &self.niri, window_id, current_x, current_y, target_x, target_y,
        )
        .await?;

        if is_visible {
            window_utils::focus_window(self.niri.clone(), window_id).await?;
        } else {
            // After hiding, optionally move to a specific workspace if configured
            if let Some(workspace) = global_move_to_workspace {
                debug!(
                    "Moving hidden scratchpad window {} to workspace {}",
                    window_id, workspace
                );
                if let Err(e) = self.niri.move_window_to_workspace(window_id, &workspace).await {
                    log::warn!(
                        "Failed to move hidden scratchpad to workspace {}: {}",
                        workspace,
                        e
                    );
                }
            }

            let previous_focused = {
                let state = self.states.get_mut(name).context("State not found")?;
                state.previous_focused_window.take()
            };
            if let Some(id) = previous_focused {
                window_utils::focus_window(self.niri.clone(), id).await?;
            }
        }

        Ok(())
    }

    async fn ensure_window_id(&mut self, name: &str) -> Result<u64> {
        let state = self.states.get_mut(name).context("State not found")?;

        if let Some(window_id) = state.window_id {
            if window_utils::window_exists(&self.niri, window_id).await? {
                return Ok(window_id);
            }
            debug!(
                "Scratchpad window {} no longer exists, clearing ID",
                window_id
            );
            state.window_id = None;
            state.is_visible = false;
        }

        // For dynamic scratchpads, if the specific window is gone, we don't try to find/launch another one.
        if state.is_dynamic {
            let msg = format!("Dynamic scratchpad '{}' window no longer exists", name);
            self.states.remove(name);
            anyhow::bail!(msg);
        }

        info!("Finding or launching window for scratchpad {}", name);
        let config = state.config.clone();
        let matcher = WindowMatcher::new(Some(vec![config.app_id.clone()]), None);

        let window_id = if let Some(window) =
            window_utils::find_window_by_matcher(self.niri.clone(), &matcher, &self.matcher_cache)
                .await?
        {
            window.id
        } else {
            window_utils::launch_application(&config.command).await?;
            let window = window_utils::wait_for_window(
                self.niri.clone(),
                &config.app_id,
                name,
                50,
                &self.matcher_cache,
            )
            .await?
            .context("Failed to launch/find window")?;
            window.id
        };

        self.setup_window(window_id, &config).await?;
        let state = self.states.get_mut(name).unwrap();
        state.window_id = Some(window_id);

        Ok(window_id)
    }

    async fn toggle(
        &mut self,
        name: &str,
        config: Option<ScratchpadConfig>,
        move_to_workspace: Option<String>,
    ) -> Result<()> {
        // 1. Ensure state exists
        if !self.states.contains_key(name) {
            let config = config.context("No config provided for new scratchpad")?;
            self.states.insert(
                name.to_string(),
                ScratchpadState {
                    window_id: None,
                    is_visible: false,
                    previous_focused_window: None,
                    config,
                    is_dynamic: false,
                },
            );
        }

        // 2. Ensure window exists and is set up
        let window_id = self.ensure_window_id(name).await?;
        let state = self.states.get_mut(name).unwrap();

        // 3. Determine next state
        if state.is_visible {
            let (current_workspace, windows) =
                window_utils::get_workspace_and_windows(&self.niri).await?;
            let in_current_workspace = windows.iter().any(|w| {
                w.id == window_id && window_utils::is_window_in_workspace(w, &current_workspace)
            });

            if in_current_workspace {
                state.is_visible = false;
            } else {
                // Already visible but elsewhere, re-record focus and it will be moved in sync_state
                state.previous_focused_window = self.niri.get_focused_window_id().await?;
            }
        } else {
            state.previous_focused_window = self.niri.get_focused_window_id().await?;
            state.is_visible = true;
        }

        // 4. Sync
        self.sync_state(name, move_to_workspace).await
    }

    async fn add_current_window(
        &mut self,
        name: &str,
        direction: Direction,
        default_size: &str,
        default_margin: u32,
    ) -> Result<()> {
        let window = window_utils::get_focused_window(&self.niri).await?;
        let app_id = window
            .app_id
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No app_id for current window"))?;

        let config = ScratchpadConfig {
            direction,
            command: format!("# Window {} added dynamically", window.id),
            app_id,
            size: default_size.to_string(),
            margin: default_margin,
        };

        if let Some(state) = self.states.get(name) {
            if let Some(wid) = state.window_id {
                if window_utils::window_exists(&self.niri, wid).await? {
                    anyhow::bail!("Scratchpad {} already exists with window {}", name, wid);
                }
            }
        }

        self.setup_window(window.id, &config).await?;

        self.states.insert(
            name.to_string(),
            ScratchpadState {
                window_id: Some(window.id),
                is_visible: false,
                previous_focused_window: None,
                config,
                is_dynamic: true,
            },
        );

        Ok(())
    }
}

/// Scratchpads plugin that wraps ScratchpadManager
pub struct ScratchpadsPlugin {
    manager: ScratchpadManager,
    config: ScratchpadsPluginConfig,
}

#[async_trait]
impl crate::plugins::Plugin for ScratchpadsPlugin {
    type Config = ScratchpadsPluginConfig;

    fn new(niri: NiriIpc, config: ScratchpadsPluginConfig) -> Self {
        let count = config.scratchpads.len();
        info!("Scratchpads plugin initialized with {} scratchpads", count);

        let mut manager = ScratchpadManager::new(niri);
        for (name, s_config) in &config.scratchpads {
            manager.states.insert(
                name.clone(),
                ScratchpadState {
                    window_id: None,
                    is_visible: false,
                    previous_focused_window: None,
                    config: s_config.clone(),
                    is_dynamic: false,
                },
            );
        }

        Self { manager, config }
    }

    async fn update_config(&mut self, config: ScratchpadsPluginConfig) -> Result<()> {
        info!("Updating scratchpads plugin configuration");

        // Merge configs
        for (name, s_config) in &config.scratchpads {
            if let Some(state) = self.manager.states.get_mut(name) {
                state.config = s_config.clone();
                state.is_dynamic = false; // It's in the config now
            } else {
                self.manager.states.insert(
                    name.clone(),
                    ScratchpadState {
                        window_id: None,
                        is_visible: false,
                        previous_focused_window: None,
                        config: s_config.clone(),
                        is_dynamic: false,
                    },
                );
            }
        }

        // Remove old states that are not dynamic and not in the new config
        self.manager
            .states
            .retain(|name, state| state.is_dynamic || config.scratchpads.contains_key(name));

        self.config = config;

        // Clear matcher cache to reflect potential regex changes in config
        self.manager.matcher_cache.clear_cache().await;

        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::ScratchpadToggle { name } => {
                info!("Handling scratchpad toggle for: {}", name);

                let config = self.config.scratchpads.get(name).cloned();
                match self.manager.toggle(name, config, self.config.move_to_workspace.clone()).await
                {
                    Ok(_) => Ok(Some(Ok(()))),
                    Err(e) => {
                        let error_msg = format!("Scratchpad '{}' error: {}", name, e);
                        send_notification("piri", &error_msg);
                        Err(e)
                    }
                }
            }
            IpcRequest::ScratchpadAdd { name, direction } => {
                info!(
                    "Handling scratchpad add for: {} with direction: {}",
                    name, direction
                );

                let direction = Direction::from_str(direction)
                    .map_err(|e| anyhow::anyhow!("Invalid direction: {}", e))?;

                self.manager
                    .add_current_window(
                        name,
                        direction,
                        &self.config.default_size,
                        self.config.default_margin,
                    )
                    .await?;

                Ok(Some(Ok(())))
            }
            _ => Ok(None), // Not handled by this plugin
        }
    }
}
