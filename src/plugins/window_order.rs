use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use niri_ipc::{Action, Event, Reply, Request};
use std::collections::HashMap;

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils;

/// Window order plugin that reorders windows in workspace based on configuration
pub struct WindowOrderPlugin {
    niri: NiriIpc,
    config: Config,
}

impl WindowOrderPlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None).expect("Failed to initialize niri IPC"),
            config: Config::default(),
        }
    }

    /// Helper function to run blocking operations
    async fn run_blocking<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(NiriIpc) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        window_utils::run_blocking(self.niri.clone(), f).await
    }

    /// Get order value for a window based on its app_id
    /// Uses configured weight if exists, otherwise uses default_weight from config
    fn get_window_order(&self, app_id: Option<&String>) -> u32 {
        if let Some(app_id) = app_id {
            self.config.get_window_order(app_id)
        } else {
            self.config.window_order_config.default_weight
        }
    }

    /// Check if window ordering should be applied to the given workspace
    /// Returns true if workspaces list is empty (apply to all) or if workspace matches
    fn should_apply_to_workspace(&self, workspace_name: &str) -> bool {
        let workspaces = &self.config.window_order_config.workspaces;

        debug!(
            "Checking if window ordering should apply to workspace '{}', configured workspaces: {:?}",
            workspace_name, workspaces
        );

        // If no workspaces specified, apply to all
        if workspaces.is_empty() {
            debug!("No workspaces configured, applying to all workspaces");
            return true;
        }

        // Try to match workspace by exact name or idx
        for configured_ws in workspaces {
            // Exact name match
            if configured_ws == workspace_name {
                debug!(
                    "Workspace '{}' matched configured workspace '{}' (exact name match)",
                    workspace_name, configured_ws
                );
                return true;
            }

            // Exact idx match
            if let (Ok(configured_idx), Ok(ws_idx)) =
                (configured_ws.parse::<u8>(), workspace_name.parse::<u8>())
            {
                if configured_idx == ws_idx {
                    debug!(
                        "Workspace '{}' matched configured workspace '{}' (exact idx match)",
                        workspace_name, configured_ws
                    );
                    return true;
                }
            }
        }

        debug!(
            "Workspace '{}' did not match any configured workspace",
            workspace_name
        );
        false
    }

    /// Reorder windows in the current workspace based on configuration
    /// This method does not check workspace filtering - it always applies to the current workspace
    async fn reorder_windows(&self) -> Result<()> {
        info!("Reordering windows in current workspace");

        // Get current focused workspace
        let current_workspace = self.run_blocking(|niri| niri.get_focused_workspace()).await?;

        // Get all windows
        let windows = self.run_blocking(|niri| niri.get_windows()).await?;

        // Filter windows in current workspace
        let workspace_windows: Vec<_> = windows
            .iter()
            .filter(|w| {
                // Check if window is in current workspace
                match (&w.workspace, &w.workspace_id) {
                    (Some(ws), _) => ws == &current_workspace.name,
                    (_, Some(ws_id)) => ws_id.to_string() == current_workspace.name,
                    _ => false,
                }
            })
            .filter(|w| !w.floating) // Only reorder tiled windows
            .collect();

        if workspace_windows.is_empty() {
            info!("No tiled windows in current workspace to reorder");
            return Ok(());
        }

        info!(
            "Found {} tiled windows in workspace {}",
            workspace_windows.len(),
            current_workspace.name
        );

        // Step 1: Get current column positions for each window (current sort)
        let mut current_positions: Vec<_> = workspace_windows
            .iter()
            .map(|w| {
                let current_col = w
                    .layout
                    .as_ref()
                    .and_then(|l| l.pos_in_scrolling_layout)
                    .map(|(col, _)| col)
                    .unwrap_or(1); // Default to column 1 if not found (1-based)
                (w.id, current_col, w.app_id.clone())
            })
            .collect();

        // Sort by current column to show current order
        current_positions.sort_by_key(|(_, col, _)| *col);

        info!(
            "Current window order (by column): {:?}",
            current_positions
                .iter()
                .map(|(id, col, app_id)| format!(
                    "window {} (app_id: {:?}, column: {})",
                    id, app_id, col
                ))
                .collect::<Vec<_>>()
        );

        // Step 2: Calculate target positions based on order weights (target sort)
        // Important: When windows have the same weight, preserve their current relative order
        // to minimize unnecessary moves

        // Get current column positions for stable sorting
        let current_col_map: HashMap<u64, usize> =
            current_positions.iter().map(|(id, col, _)| (*id, *col)).collect();

        let mut windows_with_order: Vec<_> = workspace_windows
            .iter()
            .map(|w| {
                let order = self.get_window_order(w.app_id.as_ref());
                let current_col = current_col_map.get(&w.id).copied().unwrap_or(0);
                (w.id, order, current_col, w.app_id.clone())
            })
            .collect();

        // Sort by order (descending - larger values go to the left, i.e., lower column index)
        // When order is the same, preserve current column order (stable sort)
        windows_with_order.sort_by(|a, b| {
            // First sort by order (descending)
            match b.1.cmp(&a.1) {
                std::cmp::Ordering::Equal => {
                    // If order is the same, preserve current column order (ascending)
                    a.2.cmp(&b.2)
                }
                other => other,
            }
        });

        // Assign target column indices (1-based: 1, 2, 3, ...)
        let target_positions: Vec<_> = windows_with_order
            .iter()
            .enumerate()
            .map(|(idx, (window_id, order, _current_col, app_id))| {
                let target_col = idx + 1; // 1-based column index
                (*window_id, target_col, *order, app_id.clone())
            })
            .collect();

        info!(
            "Target window order (by order weight): {:?}",
            target_positions
                .iter()
                .map(|(id, col, order, app_id)| format!(
                    "window {} (app_id: {:?}, order: {}, target_column: {})",
                    id, app_id, order, col
                ))
                .collect::<Vec<_>>()
        );

        // Step 3: Move windows to target positions using optimal algorithm
        // Strategy: Greedy approach that minimizes total moves and move distance

        let mut current_state: HashMap<u64, usize> =
            current_positions.iter().map(|(id, col, _)| (*id, *col)).collect();

        let target_state: HashMap<u64, usize> =
            target_positions.iter().map(|(id, col, _, _)| (*id, *col)).collect();

        // Build window metadata
        let window_info: HashMap<u64, (u32, Option<String>)> = target_positions
            .iter()
            .map(|(id, _, order, app_id)| (*id, (*order, app_id.clone())))
            .collect();

        // Check if already in correct positions
        let mut needs_move = false;
        for (window_id, &target_col) in &target_state {
            if current_state.get(window_id).copied().unwrap_or(0) != target_col {
                needs_move = true;
                break;
            }
        }

        if !needs_move {
            info!("All windows are already in correct positions");
            return Ok(());
        }

        // Get currently focused window ID for preference
        let focused_window_id = self.run_blocking(|niri| niri.get_focused_window_id()).await?;

        // Find optimal move sequence
        // Strategy: Try each possible move, simulate it, and choose the one that
        // maximizes the number of windows in correct positions after the move
        // Special case: if only one move is needed, prefer moving the focused window
        let mut move_sequence: Vec<(u64, usize, usize)> = Vec::new();
        let max_iterations = 100; // Safety limit
        let mut iterations = 0;

        while iterations < max_iterations {
            iterations += 1;

            // Check if we're done
            let mut all_correct = true;
            for (window_id, &target_col) in &target_state {
                if current_state.get(window_id).copied().unwrap_or(0) != target_col {
                    all_correct = false;
                    break;
                }
            }
            if all_correct {
                break;
            }

            // Find the best move by trying each possible move and evaluating the result
            // Strategy: First minimize number of moves, then minimize total move distance
            let mut best_move: Option<(u64, usize, usize)> = None;
            let mut best_correct_count: Option<usize> = None;
            let mut best_move_distance = usize::MAX;

            for (window_id, &target_col) in &target_state {
                let current_col = current_state.get(window_id).copied().unwrap_or(0);
                if current_col == target_col {
                    continue; // Already in correct position
                }

                // Calculate move distance for this window
                let move_distance = (current_col as i32 - target_col as i32).abs() as usize;

                // Simulate this move and count how many windows would be in correct position
                let mut test_state = current_state.clone();

                // Apply the move: move window from current_col to target_col
                test_state.insert(*window_id, target_col);

                // Update other windows' positions based on the move
                // When moving from A to B: windows between A and B shift
                let from = current_col;
                let to = target_col;

                for (other_id, &other_col) in current_state.iter() {
                    if *other_id == *window_id {
                        continue;
                    }

                    if from < to {
                        // Moving right: windows in (from, to] shift left by 1
                        if other_col > from && other_col <= to {
                            test_state.insert(*other_id, other_col - 1);
                        }
                    } else if from > to {
                        // Moving left: windows in [to, from) shift right by 1
                        if other_col >= to && other_col < from {
                            test_state.insert(*other_id, other_col + 1);
                        }
                    }
                }

                // Count how many windows are in correct position after this move
                let mut correct_count = 0;
                for (wid, &tgt_col) in &target_state {
                    if test_state.get(wid).copied().unwrap_or(0) == tgt_col {
                        correct_count += 1;
                    }
                }

                // Choose the move that:
                // 1. Maximizes the number of windows in correct position (minimizes remaining moves)
                // 2. Among moves with same correct_count, minimizes move distance
                // 3. If only one move is needed, prefer moving the focused window
                let is_focused =
                    focused_window_id.as_ref().map(|id| id == window_id).unwrap_or(false);
                let all_correct_after_move = correct_count == target_state.len();

                let is_better = match best_correct_count {
                    None => true, // First move
                    Some(best_count) => {
                        if correct_count > best_count {
                            true
                        } else if correct_count == best_count {
                            // If this move would complete the sorting, prefer the focused window
                            if all_correct_after_move {
                                let best_is_focused = best_move
                                    .as_ref()
                                    .and_then(|(id, _, _)| {
                                        focused_window_id.as_ref().map(|fid| fid == id)
                                    })
                                    .unwrap_or(false);
                                if is_focused && !best_is_focused {
                                    true
                                } else if !is_focused && best_is_focused {
                                    false
                                } else {
                                    move_distance < best_move_distance
                                }
                            } else {
                                move_distance < best_move_distance
                            }
                        } else {
                            false
                        }
                    }
                };

                if is_better {
                    best_move = Some((*window_id, current_col, target_col));
                    best_correct_count = Some(correct_count);
                    best_move_distance = move_distance;
                }
            }

            if let Some((window_id, from_col, to_col)) = best_move {
                move_sequence.push((window_id, from_col, to_col));

                // Apply the move to current_state
                current_state.insert(window_id, to_col);

                // Update other windows' positions
                let from = from_col;
                let to = to_col;

                let mut new_state = current_state.clone();
                for (other_id, &other_col) in current_state.iter() {
                    if *other_id == window_id {
                        continue;
                    }

                    if from < to {
                        // Moving right: windows in (from, to] shift left
                        if other_col > from && other_col <= to {
                            new_state.insert(*other_id, other_col - 1);
                        }
                    } else if from > to {
                        // Moving left: windows in [to, from) shift right
                        if other_col >= to && other_col < from {
                            new_state.insert(*other_id, other_col + 1);
                        }
                    }
                }
                current_state = new_state;
            } else {
                // No valid move found, break to avoid infinite loop
                warn!("Could not find valid move, stopping");
                break;
            }
        }

        if iterations >= max_iterations {
            warn!("Reached maximum iterations, some windows may not be in correct positions");
        }

        info!(
            "Optimal move sequence ({} moves): {:?}",
            move_sequence.len(),
            move_sequence
                .iter()
                .map(|(id, cur, tgt)| {
                    let (order, app_id) = window_info.get(id).cloned().unwrap_or((0, None));
                    format!(
                        "window {} (app_id: {:?}, order: {}): col {} -> {}",
                        id, app_id, order, cur, tgt
                    )
                })
                .collect::<Vec<_>>()
        );

        let windows_to_move = move_sequence;

        // Save currently focused window BEFORE any moves
        // This ensures we can restore focus to the original window after reordering
        if let Some(focused_id) = focused_window_id {
            info!(
                "Saved focused window ID: {} (will restore after reordering)",
                focused_id
            );
        } else {
            info!("No window is currently focused");
        }

        // Get order and app_id for each window in move sequence
        for (window_id, current_col, target_col) in windows_to_move {
            let (order, app_id) = window_info.get(&window_id).cloned().unwrap_or((0, None));

            debug!(
                "Moving window {} (app_id: {:?}, order: {}) from column {} to column {}",
                window_id, app_id, order, current_col, target_col
            );

            // Focus the window first, then move column in the same socket connection
            // This ensures the focus is applied before MoveColumnToIndex is executed
            self.run_blocking({
                let window_id = window_id;
                let target_col = target_col;
                move |_niri| {
                    let mut socket = niri_ipc::socket::Socket::connect()
                        .context("Failed to connect to niri socket")?;

                    // First, focus the window
                    let focus_action = Action::FocusWindow { id: window_id };
                    let focus_request = Request::Action(focus_action);
                    match socket.send(focus_request)? {
                        Reply::Ok(_) => {}
                        Reply::Err(err) => {
                            return Err(anyhow::anyhow!(
                                "Failed to focus window {}: {}",
                                window_id,
                                err
                            ));
                        }
                    }

                    // Small delay to ensure focus is applied
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    // Move column to target index (1-based)
                    let column_index = target_col;
                    let move_action = Action::MoveColumnToIndex {
                        index: column_index,
                    };
                    let move_request = Request::Action(move_action);
                    match socket.send(move_request)? {
                        Reply::Ok(_) => Ok(()),
                        Reply::Err(err) => Err(anyhow::anyhow!(
                            "Failed to move column to index {}: {}",
                            column_index,
                            err
                        )),
                    }
                }
            })
            .await?;

            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

            // Get window order after this move
            let current_workspace_after =
                self.run_blocking(|niri| niri.get_focused_workspace()).await?;

            let windows_after = self.run_blocking(|niri| niri.get_windows()).await?;

            let mut positions_after: Vec<_> = windows_after
                .iter()
                .filter(|w| {
                    let in_workspace = match (&w.workspace, &w.workspace_id) {
                        (Some(ws), _) => ws == &current_workspace_after.name,
                        (_, Some(ws_id)) => ws_id.to_string() == current_workspace_after.name,
                        _ => false,
                    };
                    in_workspace && !w.floating
                })
                .filter_map(|w| {
                    w.layout
                        .as_ref()
                        .and_then(|l| l.pos_in_scrolling_layout)
                        .map(|(col, _)| (w.id, col, w.app_id.clone()))
                })
                .collect();

            positions_after.sort_by_key(|(_, col, _)| *col);

            info!(
                "Window order after move: {:?}",
                positions_after
                    .iter()
                    .map(|(id, col, app_id)| format!(
                        "window {} (app_id: {:?}, column: {})",
                        id, app_id, col
                    ))
                    .collect::<Vec<_>>()
            );
        }

        // Restore focus to the previously focused window if it existed
        if let Some(window_id) = focused_window_id {
            info!("Restoring focus to original window {}", window_id);
            self.run_blocking({
                let window_id = window_id;
                move |_niri| {
                    let mut socket = niri_ipc::socket::Socket::connect()
                        .context("Failed to connect to niri socket")?;
                    let action = Action::FocusWindow { id: window_id };
                    let request = Request::Action(action);
                    match socket.send(request)? {
                        Reply::Ok(_) => {
                            debug!("Successfully restored focus to window {}", window_id);
                            Ok(())
                        }
                        Reply::Err(err) => {
                            // Log warning but don't fail - the window might have been closed
                            warn!("Failed to restore focus to window {}: {} (window may have been closed)", window_id, err);
                            Ok(())
                        }
                    }
                }
            })
            .await?;
        } else {
            debug!("No original focused window to restore");
        }

        info!("Windows reordered successfully");
        Ok(())
    }
}

#[async_trait]
impl crate::plugins::Plugin for WindowOrderPlugin {
    fn name(&self) -> &str {
        "window_order"
    }

    async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
        self.config = config.clone();
        self.niri = niri;
        info!(
            "WindowOrder plugin initialized with {} window order configurations",
            config.window_order.len()
        );
        Ok(())
    }

    async fn update_config(&mut self, _niri: NiriIpc, config: &Config) -> Result<()> {
        info!("Updating window_order plugin configuration");

        let old_count = self.config.window_order.len();
        self.config = config.clone();
        let new_count = self.config.window_order.len();

        info!(
            "WindowOrder plugin config updated: {} -> {} window order configurations",
            old_count, new_count
        );

        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::WindowOrderToggle => {
                info!("Handling window_order toggle");
                self.reorder_windows().await?;
                Ok(Some(Ok(())))
            }
            _ => Ok(None), // Not handled by this plugin
        }
    }

    /// Handle niri events to automatically reorder windows
    /// Only processes events if event listener is enabled in config
    /// For event-driven reordering, workspace filtering is applied
    async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
        // Check if event listener is enabled
        if !self.config.is_window_order_event_listener_enabled() {
            return Ok(());
        }

        // Get current workspace to check if event-driven reordering should apply
        let current_workspace = self.run_blocking(|niri| niri.get_focused_workspace()).await?;

        // For event-driven reordering, check workspace filtering
        if !self.should_apply_to_workspace(&current_workspace.name) {
            debug!(
                "Window ordering not configured for workspace '{}', skipping event-driven reorder.",
                current_workspace.name
            );
            return Ok(());
        }

        match event {
            Event::WindowLayoutsChanged { .. } => {
                debug!("Received WindowLayoutsChanged event, triggering window reorder");
                // Use a small delay to ensure layout changes are complete
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                self.reorder_windows().await?;
            }
            Event::WindowOpenedOrChanged { .. } => {
                debug!("Received WindowOpenedOrChanged event, triggering window reorder");
                // Use a small delay to ensure window is fully opened
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                self.reorder_windows().await?;
            }
            _ => {
                // Other events are not handled
            }
        }

        Ok(())
    }

    /// Check if plugin is interested in a specific event type
    /// Only interested in events that might affect window layout
    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowLayoutsChanged { .. } | Event::WindowOpenedOrChanged { .. }
        )
    }
}
