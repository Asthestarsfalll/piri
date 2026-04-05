use anyhow::Result;
use log::{debug, info, warn};
use niri_ipc::Event;
use std::collections::HashMap;

use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils;

/// Position of a tiled window in the scrolling layout (1-based indices)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutPos {
    col: usize,
    row: usize,
}

/// State for a window that is currently fullscreened
#[derive(Debug, Clone)]
struct FullscreenRestore {
    /// Saved position before entering fullscreen
    saved_pos: LayoutPos,
    /// Whether the window was sharing its column with other windows
    shared_column: bool,
    /// Whether we're currently in the process of restoring (exiting fullscreen)
    pending_restore: bool,
}

/// Action that the plugin needs niri to execute
#[derive(Debug, Clone, PartialEq, Eq)]
enum RestoreAction {
    /// No restore needed (window already at correct position)
    None,
    /// Restore needed with current and saved positions
    Restore {
        window_id: u64,
        current: (usize, usize),
        saved: (usize, usize),
        shared_column: bool,
    },
}

/// Pure state management, separated from async niri interactions for testability
struct FullscreenState {
    /// Current layout position of all tiled windows
    positions: HashMap<u64, LayoutPos>,
    /// Windows currently in fullscreen with their restore info
    fullscreen_windows: HashMap<u64, FullscreenRestore>,
}

impl FullscreenState {
    fn new() -> Self {
        Self {
            positions: HashMap::new(),
            fullscreen_windows: HashMap::new(),
        }
    }

    /// Update position tracking from a window's layout info
    fn track_position(&mut self, window_id: u64, pos: Option<(usize, usize)>) {
        if let Some((col, row)) = pos {
            self.positions.insert(window_id, LayoutPos { col, row });
        }
    }

    /// Record that a window is entering fullscreen. Returns true if position was saved.
    fn enter_fullscreen(&mut self, window_id: u64) -> bool {
        if let Some(pos) = self.positions.get(&window_id) {
            let shared_column = self.windows_in_same_col(window_id, pos.col) > 0;
            self.fullscreen_windows.insert(
                window_id,
                FullscreenRestore {
                    saved_pos: *pos,
                    shared_column,
                    pending_restore: false,
                },
            );
            true
        } else {
            false
        }
    }

    /// Mark a window as pending restore (exiting fullscreen). Returns true if window was tracked.
    fn exit_fullscreen(&mut self, window_id: u64) -> bool {
        if let Some(restore) = self.fullscreen_windows.get_mut(&window_id) {
            restore.pending_restore = true;
            true
        } else {
            false
        }
    }

    /// Check if a window is currently tracked as fullscreen
    fn is_fullscreen(&self, window_id: u64) -> bool {
        self.fullscreen_windows.contains_key(&window_id)
    }

    /// Handle WindowsChanged event (full state reset)
    fn handle_windows_changed(&mut self, windows: &[niri_ipc::Window]) {
        self.positions.clear();
        for window in windows {
            if let Some(pos) = window.layout.pos_in_scrolling_layout {
                self.positions.insert(
                    window.id,
                    LayoutPos {
                        col: pos.0,
                        row: pos.1,
                    },
                );
            }
        }
        debug!(
            "WindowsChanged: tracking {} tiled windows",
            self.positions.len()
        );
    }

    /// Handle WindowOpenedOrChanged event
    fn handle_window_opened_or_changed(&mut self, window: &niri_ipc::Window) {
        self.track_position(window.id, window.layout.pos_in_scrolling_layout);
    }

    /// Handle WindowClosed event
    fn handle_window_closed(&mut self, id: u64) {
        self.positions.remove(&id);
        self.fullscreen_windows.remove(&id);
    }

    /// Process WindowLayoutsChanged event: update positions and return restore actions
    fn process_layouts_changed(
        &mut self,
        changes: &[(u64, niri_ipc::WindowLayout)],
    ) -> Vec<RestoreAction> {
        let mut actions = Vec::new();

        for (window_id, layout) in changes {
            let pos = match layout.pos_in_scrolling_layout {
                Some((col, row)) => LayoutPos { col, row },
                None => continue,
            };

            if let Some(restore) = self.fullscreen_windows.get(window_id) {
                if restore.pending_restore {
                    let saved = restore.saved_pos;
                    if pos != saved || restore.shared_column {
                        actions.push(RestoreAction::Restore {
                            window_id: *window_id,
                            current: (pos.col, pos.row),
                            saved: (saved.col, saved.row),
                            shared_column: restore.shared_column,
                        });
                    } else {
                        actions.push(RestoreAction::None);
                    }
                }
            }

            // Update tracking
            self.positions.insert(*window_id, pos);
        }

        // Remove all pending-restore windows from fullscreen tracking
        // (both Restore actions and None/already-at-position)
        for action in &actions {
            match action {
                RestoreAction::Restore { window_id, .. } => {
                    self.fullscreen_windows.remove(window_id);
                }
                RestoreAction::None => {}
            }
        }
        // Remove windows that were at correct position and not shared
        for (window_id, _) in changes {
            if let Some(restore) = self.fullscreen_windows.get(window_id) {
                if restore.pending_restore {
                    // This handles the case where pending_restore was set
                    // but the RestoreAction::None was pushed (not shared, same pos)
                    self.fullscreen_windows.remove(window_id);
                }
            }
        }

        actions
    }

    /// Count how many other windows share the same column as the given window
    fn windows_in_same_col(&self, window_id: u64, col: usize) -> usize {
        self.positions
            .iter()
            .filter(|(id, pos)| **id != window_id && pos.col == col)
            .count()
    }
}

pub struct FullscreenPlugin {
    niri: NiriIpc,
    state: FullscreenState,
}

impl FullscreenPlugin {
    /// Handle the fullscreen toggle IPC request
    async fn handle_toggle(&mut self) -> Result<()> {
        let focused = window_utils::get_focused_window(&self.niri).await?;
        let window_id = focused.id;

        if self.state.is_fullscreen(window_id) {
            // --- EXIT FULLSCREEN ---
            debug!("Exiting fullscreen for window {}", window_id);
            self.state.exit_fullscreen(window_id);
            self.niri.fullscreen_window(window_id).await?;
        } else {
            // --- ENTER FULLSCREEN ---
            if self.state.enter_fullscreen(window_id) {
                let pos = self.state.positions[&window_id];
                debug!(
                    "Entering fullscreen for window {} (saved pos: col={}, row={})",
                    window_id, pos.col, pos.row
                );
                self.niri.fullscreen_window(window_id).await?;
            } else if focused.floating {
                debug!(
                    "Toggling fullscreen for floating window {} (no position restore)",
                    window_id
                );
                self.niri.fullscreen_window(window_id).await?;
            } else {
                warn!(
                    "Window {} not found in position tracking, fullscreen anyway",
                    window_id
                );
                self.niri.fullscreen_window(window_id).await?;
            }
        }

        Ok(())
    }

    /// Execute a restore action by sending niri commands
    async fn execute_restore(
        &self,
        window_id: u64,
        current: (usize, usize),
        saved: (usize, usize),
        shared_column: bool,
    ) -> Result<()> {
        let (cur_col, _cur_row) = current;
        let (saved_col, saved_row) = saved;

        debug!(
            "Restoring window {} from col={},row={} to col={},row={} (shared_column={})",
            window_id, cur_col, _cur_row, saved_col, saved_row, shared_column
        );

        // Focus the window first so that column/window movement commands apply to it
        self.niri.focus_window(window_id).await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Step 1: Handle column movement — ensure the window is alone in its own column
        // at the correct column index
        if cur_col != saved_col {
            let currently_shared = self.state.windows_in_same_col(window_id, cur_col);
            if currently_shared > 0 {
                self.niri.expel_window_from_column().await?;
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }

            self.niri.move_column_to_index(saved_col).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        // Step 2: If the window was in a shared column, consume it back into
        // the adjacent column and adjust its row position.
        // After MoveColumnToIndex(saved_col), the original column (with the
        // remaining windows) is now at saved_col+1 (pushed right), so we
        // consume to the right.
        if shared_column {
            self.niri.consume_or_expel_window_right(None).await?;
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // After consume, the window is appended at the bottom of the column.
            // Re-read position to determine how many moves are needed.
            let windows = self.niri.get_windows().await?;
            if let Some(w) = windows.iter().find(|w| w.id == window_id) {
                if let Some(layout) = &w.layout {
                    if let Some((_, current_row)) = layout.pos_in_scrolling_layout {
                        if current_row > saved_row {
                            for _ in 0..(current_row - saved_row) {
                                self.niri.move_window_up().await?;
                            }
                        } else if current_row < saved_row {
                            for _ in 0..(saved_row - current_row) {
                                self.niri.move_window_down().await?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle WindowLayoutsChanged: update state and execute restore actions
    async fn handle_layouts_changed(
        &mut self,
        changes: &[(u64, niri_ipc::WindowLayout)],
    ) -> Result<()> {
        let actions = self.state.process_layouts_changed(changes);

        for action in actions {
            match action {
                RestoreAction::Restore {
                    window_id,
                    current,
                    saved,
                    shared_column,
                } => {
                    if let Err(e) =
                        self.execute_restore(window_id, current, saved, shared_column).await
                    {
                        warn!("Failed to restore position for window {}: {}", window_id, e);
                    }
                }
                RestoreAction::None => {}
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for FullscreenPlugin {
    type Config = ();

    fn new(niri: NiriIpc, _config: ()) -> Self {
        info!("Fullscreen plugin initialized");
        Self {
            niri,
            state: FullscreenState::new(),
        }
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::FullscreenToggle => Ok(Some(self.handle_toggle().await)),
            _ => Ok(None),
        }
    }

    async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowsChanged { windows } => {
                self.state.handle_windows_changed(windows);
            }
            Event::WindowOpenedOrChanged { window } => {
                self.state.handle_window_opened_or_changed(window);
            }
            Event::WindowClosed { id } => {
                self.state.handle_window_closed(*id);
            }
            Event::WindowLayoutsChanged { changes } => {
                self.handle_layouts_changed(changes).await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowsChanged { .. }
                | Event::WindowOpenedOrChanged { .. }
                | Event::WindowClosed { .. }
                | Event::WindowLayoutsChanged { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use niri_ipc::WindowLayout;

    /// Helper to create a niri_ipc::Window for tests
    fn make_niri_window(id: u64, col: usize, row: usize) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: None,
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused: false,
            is_floating: false,
            is_urgent: false,
            layout: WindowLayout {
                pos_in_scrolling_layout: Some((col, row)),
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    /// Helper to create a floating niri_ipc::Window (no layout pos)
    fn make_floating_window(id: u64) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: None,
            app_id: None,
            pid: None,
            workspace_id: None,
            is_focused: false,
            is_floating: true,
            is_urgent: false,
            layout: WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
            focus_timestamp: None,
        }
    }

    /// Helper to create a WindowLayout for layout change events
    fn make_layout(col: usize, row: usize) -> WindowLayout {
        WindowLayout {
            pos_in_scrolling_layout: Some((col, row)),
            tile_size: (0.0, 0.0),
            window_size: (0, 0),
            tile_pos_in_workspace_view: None,
            window_offset_in_tile: (0.0, 0.0),
        }
    }

    // ──────────────────────────────────────────
    // Position tracking
    // ──────────────────────────────────────────

    #[test]
    fn track_position_inserts_tiled_window() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((2, 3)));
        assert_eq!(state.positions[&1], LayoutPos { col: 2, row: 3 });
    }

    #[test]
    fn track_position_ignores_none() {
        let mut state = FullscreenState::new();
        state.track_position(1, None);
        assert!(state.positions.is_empty());
    }

    #[test]
    fn track_position_updates_existing() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));
        state.track_position(1, Some((3, 2)));
        assert_eq!(state.positions[&1], LayoutPos { col: 3, row: 2 });
    }

    // ──────────────────────────────────────────
    // WindowsChanged (full reset)
    // ──────────────────────────────────────────

    #[test]
    fn windows_changed_populates_positions() {
        let mut state = FullscreenState::new();
        let windows = vec![
            make_niri_window(10, 1, 1),
            make_niri_window(20, 2, 1),
            make_niri_window(30, 2, 2),
        ];

        state.handle_windows_changed(&windows);

        assert_eq!(state.positions.len(), 3);
        assert_eq!(state.positions[&10], LayoutPos { col: 1, row: 1 });
        assert_eq!(state.positions[&20], LayoutPos { col: 2, row: 1 });
        assert_eq!(state.positions[&30], LayoutPos { col: 2, row: 2 });
    }

    #[test]
    fn windows_changed_clears_old_positions() {
        let mut state = FullscreenState::new();
        state.track_position(99, Some((5, 5)));

        let windows = vec![make_niri_window(10, 1, 1)];
        state.handle_windows_changed(&windows);

        assert!(!state.positions.contains_key(&99));
        assert_eq!(state.positions.len(), 1);
    }

    #[test]
    fn windows_changed_skips_floating() {
        let mut state = FullscreenState::new();
        let windows = vec![make_niri_window(10, 1, 1), make_floating_window(20)];

        state.handle_windows_changed(&windows);

        assert_eq!(state.positions.len(), 1);
        assert!(!state.positions.contains_key(&20));
    }

    // ──────────────────────────────────────────
    // WindowOpenedOrChanged
    // ──────────────────────────────────────────

    #[test]
    fn window_opened_tracks_new_window() {
        let mut state = FullscreenState::new();
        let w = make_niri_window(42, 3, 1);
        state.handle_window_opened_or_changed(&w);
        assert_eq!(state.positions[&42], LayoutPos { col: 3, row: 1 });
    }

    #[test]
    fn window_changed_updates_position() {
        let mut state = FullscreenState::new();
        state.track_position(42, Some((1, 1)));

        let w = make_niri_window(42, 2, 3);
        state.handle_window_opened_or_changed(&w);

        assert_eq!(state.positions[&42], LayoutPos { col: 2, row: 3 });
    }

    // ──────────────────────────────────────────
    // WindowClosed
    // ──────────────────────────────────────────

    #[test]
    fn window_closed_removes_position() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));
        state.handle_window_closed(1);
        assert!(!state.positions.contains_key(&1));
    }

    #[test]
    fn window_closed_removes_fullscreen_state() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((2, 1)));
        state.enter_fullscreen(1);
        assert!(state.is_fullscreen(1));

        state.handle_window_closed(1);
        assert!(!state.is_fullscreen(1));
        assert!(!state.positions.contains_key(&1));
    }

    #[test]
    fn window_closed_noop_for_unknown() {
        let mut state = FullscreenState::new();
        state.handle_window_closed(999); // should not panic
    }

    // ──────────────────────────────────────────
    // Enter / exit fullscreen
    // ──────────────────────────────────────────

    #[test]
    fn enter_fullscreen_saves_position() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((2, 3)));

        assert!(state.enter_fullscreen(1));
        assert!(state.is_fullscreen(1));

        let restore = &state.fullscreen_windows[&1];
        assert_eq!(restore.saved_pos, LayoutPos { col: 2, row: 3 });
        assert!(!restore.shared_column);
        assert!(!restore.pending_restore);
    }

    #[test]
    fn enter_fullscreen_detects_shared_column() {
        let mut state = FullscreenState::new();
        // Two windows in the same column
        state.track_position(1, Some((2, 1)));
        state.track_position(2, Some((2, 2)));

        assert!(state.enter_fullscreen(1));
        assert!(state.fullscreen_windows[&1].shared_column);

        assert!(state.enter_fullscreen(2));
        assert!(state.fullscreen_windows[&2].shared_column);
    }

    #[test]
    fn enter_fullscreen_not_shared_when_alone() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));
        state.track_position(2, Some((2, 1))); // different column

        assert!(state.enter_fullscreen(1));
        assert!(!state.fullscreen_windows[&1].shared_column);
    }

    #[test]
    fn enter_fullscreen_fails_for_untracked() {
        let mut state = FullscreenState::new();
        assert!(!state.enter_fullscreen(1));
        assert!(!state.is_fullscreen(1));
    }

    #[test]
    fn exit_fullscreen_sets_pending() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));
        state.enter_fullscreen(1);

        assert!(state.exit_fullscreen(1));
        assert!(state.fullscreen_windows[&1].pending_restore);
    }

    #[test]
    fn exit_fullscreen_fails_for_non_fullscreen() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));
        assert!(!state.exit_fullscreen(1));
    }

    // ──────────────────────────────────────────
    // process_layouts_changed
    // ──────────────────────────────────────────

    #[test]
    fn layouts_changed_updates_positions() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));

        let changes = vec![(1, make_layout(3, 2))];
        state.process_layouts_changed(&changes);

        assert_eq!(state.positions[&1], LayoutPos { col: 3, row: 2 });
    }

    #[test]
    fn layouts_changed_skips_floating() {
        let mut state = FullscreenState::new();
        let changes = vec![(
            1,
            WindowLayout {
                pos_in_scrolling_layout: None,
                tile_size: (0.0, 0.0),
                window_size: (0, 0),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
            },
        )];

        let actions = state.process_layouts_changed(&changes);
        assert!(actions.is_empty());
        assert!(!state.positions.contains_key(&1));
    }

    #[test]
    fn layouts_changed_triggers_restore_when_position_differs() {
        let mut state = FullscreenState::new();
        // Window alone in its column (not shared)
        state.track_position(1, Some((2, 1)));
        state.enter_fullscreen(1);
        state.exit_fullscreen(1);

        // Window comes back at a different position after unfullscreen
        let changes = vec![(1, make_layout(1, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            RestoreAction::Restore {
                window_id: 1,
                current: (1, 1),
                saved: (2, 1),
                shared_column: false,
            }
        );
        assert!(!state.is_fullscreen(1));
    }

    #[test]
    fn layouts_changed_no_restore_when_position_matches_and_not_shared() {
        let mut state = FullscreenState::new();
        // Window alone in its column
        state.track_position(1, Some((2, 1)));
        state.enter_fullscreen(1);
        state.exit_fullscreen(1);

        // Window comes back at the same position
        let changes = vec![(1, make_layout(2, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], RestoreAction::None);
        assert!(!state.is_fullscreen(1));
    }

    #[test]
    fn layouts_changed_ignores_non_pending_fullscreen() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((2, 1)));
        state.enter_fullscreen(1);
        // NOT calling exit_fullscreen — pending_restore is false

        let changes = vec![(1, make_layout(1, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert!(actions.is_empty());
        // Still tracked as fullscreen
        assert!(state.is_fullscreen(1));
    }

    #[test]
    fn layouts_changed_ignores_non_fullscreen_windows() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((1, 1)));

        let changes = vec![(1, make_layout(2, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert!(actions.is_empty());
    }

    #[test]
    fn layouts_changed_handles_multiple_windows() {
        let mut state = FullscreenState::new();
        // Window 1: alone in col 1, fullscreen, pending restore, position changed
        state.track_position(1, Some((1, 1)));
        state.enter_fullscreen(1);
        state.exit_fullscreen(1);

        // Window 2: alone in col 3, fullscreen, pending restore, position unchanged
        state.track_position(2, Some((3, 1)));
        state.enter_fullscreen(2);
        state.exit_fullscreen(2);

        // Window 3: not fullscreen
        state.track_position(3, Some((2, 1)));

        let changes = vec![
            (1, make_layout(2, 1)), // moved
            (2, make_layout(3, 1)), // same position
            (3, make_layout(2, 2)), // just a normal change
        ];
        let actions = state.process_layouts_changed(&changes);

        assert_eq!(actions.len(), 2);
        assert_eq!(
            actions[0],
            RestoreAction::Restore {
                window_id: 1,
                current: (2, 1),
                saved: (1, 1),
                shared_column: false,
            }
        );
        assert_eq!(actions[1], RestoreAction::None);
        assert!(!state.is_fullscreen(1));
        assert!(!state.is_fullscreen(2));
    }

    // ──────────────────────────────────────────
    // windows_in_same_col
    // ──────────────────────────────────────────

    #[test]
    fn windows_in_same_col_counts_correctly() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((2, 1)));
        state.track_position(2, Some((2, 2)));
        state.track_position(3, Some((2, 3)));
        state.track_position(4, Some((3, 1)));

        assert_eq!(state.windows_in_same_col(1, 2), 2); // windows 2 and 3
        assert_eq!(state.windows_in_same_col(4, 3), 0); // alone in col 3
        assert_eq!(state.windows_in_same_col(1, 5), 0); // no windows in col 5
    }

    // ──────────────────────────────────────────
    // is_interested_in_event
    // ──────────────────────────────────────────

    #[test]
    fn interested_in_relevant_events() {
        use crate::plugins::Plugin;
        let plugin = FullscreenPlugin {
            niri: NiriIpc::new(None),
            state: FullscreenState::new(),
        };

        assert!(plugin.is_interested_in_event(&Event::WindowsChanged { windows: vec![] }));
        assert!(
            plugin.is_interested_in_event(&Event::WindowOpenedOrChanged {
                window: make_niri_window(1, 1, 1)
            })
        );
        assert!(plugin.is_interested_in_event(&Event::WindowClosed { id: 1 }));
        assert!(plugin.is_interested_in_event(&Event::WindowLayoutsChanged { changes: vec![] }));
    }

    #[test]
    fn not_interested_in_other_events() {
        use crate::plugins::Plugin;
        let plugin = FullscreenPlugin {
            niri: NiriIpc::new(None),
            state: FullscreenState::new(),
        };

        assert!(!plugin.is_interested_in_event(&Event::WorkspaceActivated {
            id: 1,
            focused: true
        }));
    }

    // ──────────────────────────────────────────
    // Full enter/exit cycle (state only)
    // ──────────────────────────────────────────

    #[test]
    fn full_cycle_enter_exit_restore() {
        let mut state = FullscreenState::new();

        // Initial state: 3 windows in 2 columns
        let windows = vec![
            make_niri_window(1, 1, 1),
            make_niri_window(2, 2, 1),
            make_niri_window(3, 2, 2),
        ];
        state.handle_windows_changed(&windows);

        // Window 2 enters fullscreen (was at col=2, row=1, shared with window 3)
        assert!(state.enter_fullscreen(2));
        assert!(state.is_fullscreen(2));
        assert!(state.fullscreen_windows[&2].shared_column);

        // Simulate: niri moves windows around during fullscreen
        // Window 2 gets layout change while fullscreen (but not pending restore yet)
        let changes = vec![(2, make_layout(1, 1))];
        let actions = state.process_layouts_changed(&changes);
        assert!(actions.is_empty()); // not pending

        // User toggles off fullscreen
        assert!(state.exit_fullscreen(2));

        // Niri sends layout change: window 2 is now at col=1, row=1
        let changes = vec![(2, make_layout(1, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            RestoreAction::Restore {
                window_id: 2,
                current: (1, 1),
                saved: (2, 1),
                shared_column: true,
            }
        );
        assert!(!state.is_fullscreen(2));
    }

    #[test]
    fn shared_column_top_window_triggers_restore_even_if_same_col() {
        let mut state = FullscreenState::new();
        // Two windows in column 2
        state.track_position(1, Some((2, 1)));
        state.track_position(2, Some((2, 2)));

        // Window 1 (top) enters fullscreen
        state.enter_fullscreen(1);
        state.exit_fullscreen(1);

        // After unfullscreen, window comes back at col=2, row=1 (same position!)
        // But since it was shared, it's now in its own column — needs restore
        let changes = vec![(1, make_layout(2, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            RestoreAction::Restore {
                window_id: 1,
                current: (2, 1),
                saved: (2, 1),
                shared_column: true,
            }
        );
    }

    #[test]
    fn non_shared_column_same_position_no_restore() {
        let mut state = FullscreenState::new();
        // Window alone in its column
        state.track_position(1, Some((2, 1)));

        state.enter_fullscreen(1);
        state.exit_fullscreen(1);

        // Same position, not shared → no restore needed
        let changes = vec![(1, make_layout(2, 1))];
        let actions = state.process_layouts_changed(&changes);

        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], RestoreAction::None);
    }

    #[test]
    fn window_closed_during_fullscreen() {
        let mut state = FullscreenState::new();
        state.track_position(1, Some((2, 1)));
        state.enter_fullscreen(1);

        // Window gets closed while fullscreen
        state.handle_window_closed(1);

        assert!(!state.is_fullscreen(1));
        assert!(!state.positions.contains_key(&1));
    }
}
