use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use tokio::time::{sleep, Duration};

use crate::config::ScratchpadConfig;
use crate::niri::NiriIpc;

/// Manages scratchpad windows
pub struct ScratchpadManager {
    niri: NiriIpc,
    /// Map of scratchpad name to window ID
    scratchpads: HashMap<String, u64>,
    /// Map of scratchpad name to whether it's currently visible
    visibility: HashMap<String, bool>,
    /// Map of scratchpad name to original workspace (workspace name or ID as string)
    original_workspaces: HashMap<String, String>,
    /// Map of scratchpad name to previously focused window ID (before showing scratchpad)
    previous_focused_windows: HashMap<String, Option<u64>>,
    /// Map of scratchpad name to config (for dynamically added scratchpads)
    dynamic_configs: HashMap<String, ScratchpadConfig>,
}

impl ScratchpadManager {
    pub fn new(niri: NiriIpc) -> Self {
        Self {
            niri,
            scratchpads: HashMap::new(),
            visibility: HashMap::new(),
            original_workspaces: HashMap::new(),
            previous_focused_windows: HashMap::new(),
            dynamic_configs: HashMap::new(),
        }
    }

    /// Toggle scratchpad visibility
    pub async fn toggle(&mut self, name: &str, config: &ScratchpadConfig) -> Result<()> {
        info!("Toggling scratchpad: {}", name);

        // Get window match pattern from app_id (required in config)
        let window_match = config.app_id.clone();

        // Check if we already have this scratchpad registered
        let is_currently_visible = self.visibility.get(name).copied().unwrap_or(false);

        // Record focus BEFORE any operations, but only if we're about to show (currently hidden)
        // This ensures we capture the real focused window before any scratchpad operations
        if !is_currently_visible {
            info!(
                "Getting current focused window before showing scratchpad {} (at toggle start)",
                name
            );
            let previous_focused = tokio::task::spawn_blocking({
                let niri = self.niri.clone();
                move || niri.get_focused_window_id()
            })
            .await
            .context("Task join error")??;

            info!(
                "Recording previous focused window for scratchpad {}: {:?}",
                name, previous_focused
            );
            self.previous_focused_windows.insert(name.to_string(), previous_focused);
        }

        if let Some(&window_id) = self.scratchpads.get(name) {
            // Check if window still exists
            if let Some(window) = self.niri.find_window_async(&window_match).await? {
                if window.id == window_id {
                    // Window exists, toggle visibility
                    self.toggle_window_visibility(window_id, name, config).await?;
                    return Ok(());
                }
            }
            // Window doesn't exist anymore, remove from registry
            warn!(
                "Scratchpad window {} not found, removing from registry",
                name
            );
            self.scratchpads.remove(name);
            self.visibility.remove(name);
            self.original_workspaces.remove(name);
            self.previous_focused_windows.remove(name);
            self.dynamic_configs.remove(name);
        }

        // Try to find existing window
        if let Some(window) = self.niri.find_window_async(&window_match).await? {
            info!("Found existing window for scratchpad {}", name);
            // Ensure window is floating before registering
            tokio::task::spawn_blocking({
                let niri = self.niri.clone();
                let window_id = window.id;
                move || niri.set_window_floating(window_id, true)
            })
            .await
            .context("Task join error")??;

            // Register the window (this will move it off-screen and set visibility to false)
            self.register_scratchpad(name.to_string(), window.id, config).await?;
            // First toggle: show the window (move to correct position, visible = true)
            // Note: Focus was already recorded at the start of toggle() if needed
            self.show_scratchpad(window.id, name, config).await?;
            self.visibility.insert(name.to_string(), true);
            return Ok(());
        }

        // Launch application
        info!("Launching application for scratchpad {}", name);
        info!("Looking for window matching pattern: {}", window_match);
        self.launch_application(config).await?;

        // Wait for window to appear
        let mut attempts = 0;
        let max_attempts = 50; // 5 seconds with 100ms intervals

        loop {
            sleep(Duration::from_millis(100)).await;
            attempts += 1;

            if let Some(window) = self.niri.find_window_async(&window_match).await? {
                info!("Window appeared for scratchpad {} (ID: {}, app_id: {:?}, class: {:?}, title: {})", 
                      name, window.id, window.app_id, window.class, window.title);
                self.register_scratchpad(name.to_string(), window.id, config).await?;
                // Toggle to show on first launch (will change visibility from false to true)
                self.toggle_window_visibility(window.id, name, config).await?;
                return Ok(());
            }

            // Log available windows every 10 attempts (every second) for debugging
            if attempts % 10 == 0 {
                debug!(
                    "Still waiting for window (attempt {}/{})...",
                    attempts, max_attempts
                );
                match tokio::task::spawn_blocking({
                    let niri = self.niri.clone();
                    move || niri.get_windows()
                })
                .await
                .context("Task join error")?
                {
                    Ok(windows) => {
                        debug!("Available windows: {}", windows.len());
                        for window in windows.iter().take(5) {
                            debug!(
                                "  - ID: {}, app_id: {:?}, class: {:?}, title: {}",
                                window.id, window.app_id, window.class, window.title
                            );
                        }
                    }
                    Err(e) => {
                        debug!("Failed to get windows: {}", e);
                    }
                }
            }

            if attempts >= max_attempts {
                // Before bailing, list all available windows for debugging
                warn!(
                    "Timeout waiting for window matching '{}' for scratchpad {}",
                    window_match, name
                );
                match tokio::task::spawn_blocking({
                    let niri = self.niri.clone();
                    move || niri.get_windows()
                })
                .await
                .context("Task join error")?
                {
                    Ok(windows) => {
                        warn!("Available windows at timeout ({} total):", windows.len());
                        for window in windows.iter() {
                            warn!(
                                "  - ID: {}, app_id: {:?}, class: {:?}, title: {}",
                                window.id, window.app_id, window.class, window.title
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Failed to get windows for debugging: {}", e);
                    }
                }
                anyhow::bail!("Timeout waiting for window to appear for scratchpad {} (searched for pattern: '{}')", name, window_match);
            }
        }
    }

    async fn register_scratchpad(
        &mut self,
        name: String,
        window_id: u64,
        config: &ScratchpadConfig,
    ) -> Result<()> {
        debug!(
            "Registering scratchpad {} with window ID {}",
            name, window_id
        );

        // Record original workspace before making any changes
        let original_workspace = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            let window_id = window_id;
            move || -> Result<String> {
                let windows = niri.get_windows()?;
                for window in windows {
                    if window.id == window_id {
                        if let Some(workspace) = &window.workspace {
                            return Ok(workspace.clone());
                        }
                        if let Some(workspace_id) = window.workspace_id {
                            return Ok(workspace_id.to_string());
                        }
                    }
                }
                // Fallback to "1" if workspace not found
                Ok("1".to_string())
            }
        })
        .await
        .context("Task join error")??;

        debug!(
            "Scratchpad {} original workspace: {}",
            name, original_workspace
        );
        self.original_workspaces.insert(name.clone(), original_workspace);

        // Set window to floating
        self.niri
            .set_window_floating(window_id, true)
            .context("Failed to set window to floating")?;

        // Get focused output dimensions (for initial registration, use focused output)
        let (output_width, output_height, output_x, output_y) = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || -> Result<(u32, u32, i32, i32)> {
                let focused = niri.get_focused_output()?;
                if let Some(logical) = focused.logical {
                    Ok((logical.width, logical.height, logical.x, logical.y))
                } else {
                    Ok((1920, 1080, 0, 0))
                }
            }
        })
        .await
        .context("Task join error")??;

        // Parse size to get window dimensions
        let (width_ratio, height_ratio) = config.parse_size()?;
        let window_width = (output_width as f64 * width_ratio) as u32;
        let window_height = (output_height as f64 * height_ratio) as u32;

        // Set window size first
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.resize_floating_window(window_id, window_width, window_height)
        })
        .await
        .context("Task join error")??;

        // Small delay to ensure resize completes
        sleep(Duration::from_millis(100)).await;

        // Get current window position
        // Note: We use window_width and window_height from config, not from position query
        // as the window might not have the correct size yet
        let (current_x, current_y, _, _) = self
            .niri
            .get_window_position_async(window_id)
            .await?
            .context("Failed to get window position")?;

        // Calculate off-screen centered position based on direction (relative to output)
        let (rel_hide_x, rel_hide_y) = self.calculate_hide_position(
            &config.direction,
            output_width,
            output_height,
            window_width,
            window_height,
            config.margin,
        );

        // Convert to absolute position by adding output offset
        let hide_x = output_x + rel_hide_x;
        let hide_y = output_y + rel_hide_y;

        // Use the abstracted move function
        self.move_window_to_position(window_id, current_x, current_y, hide_x, hide_y)
            .await?;

        // Initialize visibility state to false (hidden)
        self.visibility.insert(name.clone(), false);
        self.scratchpads.insert(name, window_id);
        Ok(())
    }

    async fn toggle_window_visibility(
        &mut self,
        window_id: u64,
        name: &str,
        config: &ScratchpadConfig,
    ) -> Result<()> {
        let is_visible = self.visibility.get(name).copied().unwrap_or(false);
        info!(
            "Toggling visibility for scratchpad {}: current state is_visible={}",
            name, is_visible
        );

        if is_visible {
            info!("Hiding scratchpad {}", name);
            // When hiding, use the previously recorded focus (recorded when showing)
            // Don't re-record focus here, as the current focus is the scratchpad itself
            self.hide_scratchpad(window_id, name, config).await?;
            self.visibility.insert(name.to_string(), false);
            info!("Scratchpad {} hidden, visibility set to false", name);
        } else {
            info!("Showing scratchpad {}", name);
            // Note: Focus should already be recorded in toggle() function at the very beginning
            // before any scratchpad operations, to ensure we capture the real focused window
            self.show_scratchpad(window_id, name, config).await?;
            self.visibility.insert(name.to_string(), true);
            info!("Scratchpad {} shown, visibility set to true", name);
        }

        Ok(())
    }

    async fn show_scratchpad(
        &mut self,
        window_id: u64,
        name: &str,
        config: &ScratchpadConfig,
    ) -> Result<()> {
        debug!("Showing scratchpad window {} ({})", window_id, name);

        // Note: Previous focused window should already be recorded in toggle() function
        // before calling this function

        // Ensure window is floating
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.set_window_floating(window_id, true)
        })
        .await
        .context("Task join error")??;

        // Move window to focused workspace before showing
        info!("Moving scratchpad {} to focused workspace", name);
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.move_floating_window(window_id)
        })
        .await
        .context("Task join error")??;

        // Small delay to ensure workspace change completes
        sleep(Duration::from_millis(100)).await;

        // Get output dimensions
        let (output_width, output_height) = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.get_output_dimensions()
        })
        .await
        .context("Task join error")??;

        // Parse size
        let (width_ratio, height_ratio) = config.parse_size()?;
        let window_width = (output_width as f64 * width_ratio) as u32;
        let window_height = (output_height as f64 * height_ratio) as u32;

        // Calculate target position based on direction (relative to focused output, no offset)
        let (target_x, target_y) = self.calculate_position(
            &config.direction,
            output_width,
            output_height,
            window_width,
            window_height,
            config.margin,
        );

        debug!(
            "Positioning window at ({}, {}) with size {}x{}",
            target_x, target_y, window_width, window_height
        );

        // Resize window
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.resize_floating_window(window_id, window_width, window_height)
        })
        .await
        .context("Task join error")??;

        // Get current window position
        let (current_x, current_y, _, _) = self
            .niri
            .get_window_position_async(window_id)
            .await?
            .context("Failed to get window position")?;

        // Use the abstracted move function
        self.move_window_to_position(window_id, current_x, current_y, target_x, target_y)
            .await?;

        // Focus the scratchpad window
        info!("Focusing scratchpad window {}", window_id);
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.focus_window(window_id)
        })
        .await
        .context("Task join error")??;

        Ok(())
    }

    fn calculate_position(
        &self,
        direction: &str,
        output_width: u32,
        output_height: u32,
        window_width: u32,
        window_height: u32,
        margin: u32,
    ) -> (i32, i32) {
        match direction {
            "fromTop" => {
                let x = ((output_width - window_width) / 2) as i32;
                let y = margin as i32;
                (x, y)
            }
            "fromBottom" => {
                let x = ((output_width - window_width) / 2) as i32;
                let y = (output_height - window_height - margin) as i32;
                (x, y)
            }
            "fromLeft" => {
                let x = margin as i32;
                let y = ((output_height - window_height) / 2) as i32;
                (x, y)
            }
            "fromRight" => {
                let x = (output_width - window_width - margin) as i32;
                let y = ((output_height - window_height) / 2) as i32;
                (x, y)
            }
            _ => {
                warn!("Unknown direction: {}, defaulting to fromTop", direction);
                let x = ((output_width - window_width) / 2) as i32;
                let y = margin as i32;
                (x, y)
            }
        }
    }

    async fn hide_scratchpad(
        &mut self,
        window_id: u64,
        name: &str,
        config: &ScratchpadConfig,
    ) -> Result<()> {
        info!("Hiding scratchpad window {}", window_id);

        // Ensure window is floating before moving
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.set_window_floating(window_id, true)
        })
        .await
        .context("Task join error")??;

        // Get output dimensions
        let (output_width, output_height) = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.get_output_dimensions()
        })
        .await
        .context("Task join error")??;

        // Get current window position and size
        let (current_x, current_y, window_width, window_height) = self
            .niri
            .get_window_position_async(window_id)
            .await?
            .context("Failed to get window position")?;

        info!(
            "Current window position: ({}, {}), size: {}x{}, output: {}x{}",
            current_x, current_y, window_width, window_height, output_width, output_height
        );

        // Calculate off-screen centered position based on direction (relative to focused output, no offset)
        let (hide_x, hide_y) = self.calculate_hide_position(
            &config.direction,
            output_width,
            output_height,
            window_width,
            window_height,
            config.margin,
        );

        // Use the abstracted move function
        self.move_window_to_position(window_id, current_x, current_y, hide_x, hide_y)
            .await?;

        info!("Window {} moved off-screen", window_id);

        // Restore focus logic
        info!("Starting focus restoration for scratchpad {}", name);
        let previous_focused_opt = self.previous_focused_windows.get(name).copied();
        info!(
            "Previous focused window for scratchpad {}: {:?}",
            name, previous_focused_opt
        );

        // Handle the case where previous_focused was None (empty workspace when shown)
        // previous_focused_opt is Option<Option<u64>>:
        // - None: key doesn't exist (shouldn't happen, but handle gracefully)
        // - Some(None): key exists but value is None (empty workspace when shown)
        // - Some(Some(id)): key exists and has a window ID
        let previous_focused = match previous_focused_opt {
            None => {
                info!("No previous focused window entry found for scratchpad {} (unexpected), will look for middle window", name);
                None
            }
            Some(None) => {
                info!("Previous focused window was None (empty workspace when shown), skipping focus restoration");
                return Ok(());
            }
            Some(Some(id)) => {
                info!("Previous focused window ID: {}", id);
                Some(id)
            }
        };

        // If we reach here, we either have a valid previous_focused or need to find middle window
        // Get current focused workspace
        info!("Getting current focused workspace");
        let current_workspace = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.get_focused_workspace()
        })
        .await
        .context("Task join error")??;
        info!("Current focused workspace: {}", current_workspace.name);

        // Get all windows to check workspace and find middle window
        let windows = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.get_windows()
        })
        .await
        .context("Task join error")??;
        debug!("Total windows found: {}", windows.len());

        // Check if previous focused window exists and is in current workspace
        if let Some(prev_window_id) = previous_focused {
            info!(
                "Checking if previous focused window {} exists and is in current workspace",
                prev_window_id
            );
            let prev_window = windows.iter().find(|w| w.id == prev_window_id);
            if let Some(window) = prev_window {
                // Check if window is in current workspace
                let in_current_workspace = match (&window.workspace, &window.workspace_id) {
                    (Some(ws), _) => ws == &current_workspace.name,
                    (_, Some(ws_id)) => ws_id.to_string() == current_workspace.name,
                    _ => false,
                };

                info!(
                    "Previous window {} found, workspace: {:?}/{:?}, in current workspace: {}",
                    prev_window_id, window.workspace, window.workspace_id, in_current_workspace
                );

                if in_current_workspace {
                    // Focus the previous window
                    info!(
                        "Previous window {} is in current workspace, focusing it",
                        prev_window_id
                    );
                    tokio::task::spawn_blocking({
                        let niri = self.niri.clone();
                        move || niri.focus_window(prev_window_id)
                    })
                    .await
                    .context("Task join error")??;
                    info!("Successfully focused previous window {}", prev_window_id);
                    return Ok(());
                } else {
                    // Previous window is in a different workspace
                    // Need to convert workspace_id to workspace idx for switching
                    let target_workspace = match (&window.workspace, &window.workspace_id) {
                        (Some(ws), _) => {
                            // If workspace name is available, use it directly
                            Some(ws.clone())
                        }
                        (_, Some(ws_id)) => {
                            // Convert workspace_id to workspace idx
                            info!("Converting workspace_id {} to workspace idx", ws_id);
                            let workspace_idx = tokio::task::spawn_blocking({
                                let niri = self.niri.clone();
                                let id = *ws_id;
                                move || niri.get_workspace_idx_from_id(id)
                            })
                            .await
                            .context("Task join error")??;

                            match workspace_idx {
                                Some(idx) => {
                                    info!("Workspace id {} corresponds to idx {}", ws_id, idx);
                                    Some(idx.to_string())
                                }
                                None => {
                                    warn!("Could not find workspace idx for workspace_id {}, will look for middle window", ws_id);
                                    None
                                }
                            }
                        }
                        _ => {
                            info!("Previous window {} has no workspace info, will look for middle window", prev_window_id);
                            None
                        }
                    };

                    if let Some(ws) = target_workspace {
                        info!(
                            "Previous window {} is in workspace {}, switching to it and focusing",
                            prev_window_id, ws
                        );

                        // Switch to the workspace using idx
                        tokio::task::spawn_blocking({
                            let niri = self.niri.clone();
                            let ws_str = ws.clone();
                            move || niri.switch_to_workspace(&ws_str)
                        })
                        .await
                        .context("Task join error")??;

                        // Small delay to ensure workspace switch completes
                        sleep(Duration::from_millis(100)).await;

                        // Focus the window
                        tokio::task::spawn_blocking({
                            let niri = self.niri.clone();
                            move || niri.focus_window(prev_window_id)
                        })
                        .await
                        .context("Task join error")??;
                        info!(
                            "Successfully switched to workspace {} and focused window {}",
                            ws, prev_window_id
                        );
                        return Ok(());
                    } else {
                        info!("Could not determine target workspace, will look for middle window");
                    }
                }
            } else {
                info!(
                    "Previous focused window {} not found in window list",
                    prev_window_id
                );
            }
        } else {
            info!(
                "No previous focused window recorded for scratchpad {}",
                name
            );
        }

        // Previous window not found or not in current workspace
        // Find windows in current workspace (excluding the scratchpad being hidden)
        info!(
            "Looking for windows in current workspace (excluding scratchpad {})",
            window_id
        );
        let current_workspace_windows: Vec<_> = windows
            .iter()
            .filter(|w| {
                w.id != window_id && // Exclude the scratchpad being hidden
                match (&w.workspace, &w.workspace_id) {
                    (Some(ws), _) => ws == &current_workspace.name,
                    (_, Some(ws_id)) => ws_id.to_string() == current_workspace.name,
                    _ => false,
                }
            })
            .collect();

        info!(
            "Found {} windows in current workspace",
            current_workspace_windows.len()
        );

        if !current_workspace_windows.is_empty() {
            // Find the middle window
            // For tiled windows, we can use layout position
            // For floating windows, we can use screen position
            // For simplicity, we'll pick the window closest to the center
            let middle_index = current_workspace_windows.len() / 2;
            let middle_window = current_workspace_windows[middle_index];

            info!(
                "Focusing middle window {} (index {}/{}) in current workspace",
                middle_window.id,
                middle_index,
                current_workspace_windows.len()
            );
            tokio::task::spawn_blocking({
                let niri = self.niri.clone();
                let window_id = middle_window.id;
                move || niri.focus_window(window_id)
            })
            .await
            .context("Task join error")??;
            info!("Successfully focused middle window {}", middle_window.id);
        } else {
            info!("No other windows in current workspace to focus");
        }

        Ok(())
    }

    /// Abstracted function to move window from current position to target position
    /// Automatically calculates the relative offset
    async fn move_window_to_position(
        &self,
        window_id: u64,
        current_x: i32,
        current_y: i32,
        target_x: i32,
        target_y: i32,
    ) -> Result<()> {
        // Calculate relative movement from current position to target position
        let rel_x = target_x - current_x;
        let rel_y = target_y - current_y;

        info!(
            "Moving window {} from ({}, {}) to ({}, {}) with relative movement ({}, {})",
            window_id, current_x, current_y, target_x, target_y, rel_x, rel_y
        );

        // Move window using relative movement
        tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.move_window_relative(window_id, rel_x, rel_y)
        })
        .await
        .context("Task join error")??;

        Ok(())
    }

    fn calculate_hide_position(
        &self,
        direction: &str,
        output_width: u32,
        output_height: u32,
        window_width: u32,
        window_height: u32,
        margin: u32,
    ) -> (i32, i32) {
        // Calculate off-screen centered position where window is completely outside the screen
        // Add margin to ensure window is fully hidden
        match direction {
            "fromTop" => {
                // Position above screen, horizontally centered
                // Move window completely above screen, accounting for margin
                let x = ((output_width - window_width) / 2) as i32;
                let y = -((window_height + margin) as i32); // Completely above screen with margin
                (x, y)
            }
            "fromBottom" => {
                // Position below screen, horizontally centered
                let x = ((output_width - window_width) / 2) as i32;
                let y = (output_height + margin) as i32; // Completely below screen with margin
                (x, y)
            }
            "fromLeft" => {
                // Position left of screen, vertically centered
                let x = -((window_width + margin) as i32); // Completely left of screen with margin
                let y = ((output_height - window_height) / 2) as i32;
                (x, y)
            }
            "fromRight" => {
                // Position right of screen, vertically centered
                let x = (output_width + margin) as i32; // Completely right of screen with margin
                let y = ((output_height - window_height) / 2) as i32;
                (x, y)
            }
            _ => {
                // Default: move far off-screen diagonally
                warn!("Unknown direction: {}, defaulting to fromTop", direction);
                let x = ((output_width - window_width) / 2) as i32;
                let y = -((window_height + margin) as i32);
                (x, y)
            }
        }
    }

    async fn launch_application(&self, config: &ScratchpadConfig) -> Result<()> {
        debug!("Launching: {}", config.command);

        // Parse command - it may contain environment variables and arguments
        // Use shell to execute the full command
        Command::new("sh")
            .arg("-c")
            .arg(&config.command)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to launch application: {}", config.command))?;

        Ok(())
    }

    /// Get window ID for a scratchpad
    pub fn get_window_id(&self, name: &str) -> Option<u64> {
        self.scratchpads.get(name).copied()
    }

    /// Check if scratchpad is visible
    pub fn is_visible(&self, name: &str) -> bool {
        self.visibility.get(name).copied().unwrap_or(false)
    }

    /// Add current focused window as scratchpad
    /// This will float the window, resize it, and move it off-screen
    pub async fn add_current_window(
        &mut self,
        name: &str,
        direction: &str,
        default_size: &str,
        default_margin: u32,
    ) -> Result<()> {
        info!("Adding current focused window as scratchpad: {}", name);

        // Get current focused window
        let focused_window_id = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.get_focused_window_id()
        })
        .await
        .context("Task join error")??;

        let window_id = match focused_window_id {
            Some(id) => id,
            None => anyhow::bail!("No focused window found"),
        };

        info!("Found focused window ID: {}", window_id);

        // Get window details to extract app_id
        let windows = tokio::task::spawn_blocking({
            let niri = self.niri.clone();
            move || niri.get_windows()
        })
        .await
        .context("Task join error")??;

        let window = windows
            .iter()
            .find(|w| w.id == window_id)
            .ok_or_else(|| anyhow::anyhow!("Window {} not found", window_id))?;

        // Get app_id for matching (required for scratchpad config)
        let app_id = window.app_id.clone().unwrap_or_else(|| format!("window_{}", window_id));

        info!("Window app_id: {:?}, using: {}", window.app_id, app_id);

        // Create a temporary config with default values from config file
        // The user can later add this to the config file if needed
        let config = ScratchpadConfig {
            direction: direction.to_string(),
            command: format!("# Window {} was added dynamically", window_id),
            app_id: app_id.clone(),
            size: default_size.to_string(),
            margin: default_margin,
        };

        // Check if scratchpad with this name already exists
        if self.scratchpads.contains_key(name) {
            anyhow::bail!("Scratchpad '{}' already exists", name);
        }

        // Register the scratchpad (this will float, resize, and move off-screen)
        self.register_scratchpad(name.to_string(), window_id, &config).await?;

        // Store the config for dynamic scratchpads
        self.dynamic_configs.insert(name.to_string(), config);

        info!(
            "Successfully added window {} as scratchpad '{}'",
            window_id, name
        );
        Ok(())
    }

    /// Get config for a scratchpad (from dynamic configs or provided config)
    /// This allows toggle to work with dynamically added scratchpads
    pub fn get_config(
        &self,
        name: &str,
        provided_config: Option<&ScratchpadConfig>,
    ) -> Option<ScratchpadConfig> {
        // First check if we have a dynamic config
        if let Some(config) = self.dynamic_configs.get(name) {
            return Some(config.clone());
        }
        // Otherwise use provided config
        provided_config.map(|c| c.clone())
    }
}
