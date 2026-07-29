use anyhow::{Context, Result};
use log::{debug, info, warn};
use niri_ipc::{Action, ColumnDisplay, Reply, Request, WorkspaceReferenceArg};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::time::Duration;

// ---------------------------------------------------------------------------
// Global sticky window registry — shared between sticky plugin & scratchpads
// ---------------------------------------------------------------------------

/// Global registry of windows that should follow the focused workspace.
/// Maps window_id -> cross_monitor flag.
static STICKY_REGISTRY: OnceLock<StdMutex<HashMap<u64, bool>>> = OnceLock::new();

fn sticky_registry() -> &'static StdMutex<HashMap<u64, bool>> {
    STICKY_REGISTRY.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Register a window as sticky. Called by scratchpads when creating a sticky
/// scratchpad, or by sticky plugin via IPC.
pub fn register_sticky_window(window_id: u64, cross_monitor: bool) {
    sticky_registry().lock().unwrap().insert(window_id, cross_monitor);
    debug!(
        "Sticky registry: registered window {} (cross={})",
        window_id, cross_monitor
    );
}

/// Unregister a sticky window.
pub fn unregister_sticky_window(window_id: u64) {
    sticky_registry().lock().unwrap().remove(&window_id);
    debug!("Sticky registry: unregistered window {}", window_id);
}

/// Snapshot of all registered sticky windows as (window_id, cross_monitor) pairs.
pub fn get_sticky_window_list() -> Vec<(u64, bool)> {
    sticky_registry()
        .lock()
        .unwrap()
        .iter()
        .map(|(&id, &cross)| (id, cross))
        .collect()
}

use crate::config::Direction;
use crate::niri::NiriIpc;
use crate::niri::Window;

/// Shared state to track programmatic focus changes (e.g., from auto_fill)
/// This prevents window_rule from executing focus_command during programmatic operations
static PROGRAMMATIC_FOCUS_TIME: OnceLock<StdMutex<Option<Instant>>> = OnceLock::new();

fn get_programmatic_focus_time() -> &'static StdMutex<Option<Instant>> {
    PROGRAMMATIC_FOCUS_TIME.get_or_init(|| StdMutex::new(None))
}

/// Mark that a programmatic focus change is starting
/// Focus changes within PROGRAMMATIC_FOCUS_WINDOW_MS will be ignored by window_rule
pub fn mark_programmatic_focus_start() {
    let time = get_programmatic_focus_time();
    let mut guard = time.lock().unwrap();
    *guard = Some(Instant::now());
}

/// Check if a focus change should be ignored (happened during programmatic operation)
pub fn should_ignore_focus_change() -> bool {
    const PROGRAMMATIC_FOCUS_WINDOW_MS: u64 = 500;
    let time = get_programmatic_focus_time();
    let guard = time.lock().unwrap();
    if let Some(start_time) = *guard {
        if start_time.elapsed().as_millis() < PROGRAMMATIC_FOCUS_WINDOW_MS as u128 {
            return true;
        }
    }
    false
}

/// Execute a shell command (generic function for all plugins)
/// This function spawns a command in the background without waiting for completion
pub fn execute_command(command: &str) -> Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to execute command: {}", command))?;
    Ok(())
}

/// Launch an application by executing a command
/// This is a convenience wrapper around execute_command
pub async fn launch_application(command: &str) -> Result<()> {
    debug!("Launching: {}", command);
    execute_command(command)
}

/// Focus a window by ID
pub async fn focus_window(niri: NiriIpc, window_id: u64) -> Result<()> {
    niri.focus_window(window_id).await
}

/// Try to refocus to the previous window if the target window is currently focused.
/// Returns true if refocus was performed, false otherwise.
/// This is a shared function used by both mark and scratchpads plugins.
pub async fn try_refocus_to_previous(
    niri: &NiriIpc,
    target_window_id: u64,
    previous_window: &mut Option<u64>,
) -> Result<bool> {
    // Get current focused window
    let current = match get_focused_window(niri).await {
        Ok(window) => window,
        Err(_) => return Ok(false),
    };

    // If current focused window is not the target, no refocus needed
    if current.id != target_window_id {
        return Ok(false);
    }

    // If there's a previous window and it exists, switch to it
    if let Some(prev_id) = *previous_window {
        if window_exists(niri, prev_id).await? {
            debug!(
                "Refocusing from window {} to previous window {}",
                target_window_id, prev_id
            );
            // Swap: set previous to current target window for next toggle
            *previous_window = Some(target_window_id);
            // Focus the previous window
            focus_window(niri.clone(), prev_id).await?;
            return Ok(true);
        }
    }

    Ok(false)
}

pub async fn get_focused_window(niri: &NiriIpc) -> Result<Window> {
    let focused_window_id = niri.get_focused_window_id().await?;
    let window_id = focused_window_id.ok_or_else(|| anyhow::anyhow!("No focused window found"))?;
    let windows = niri.get_windows().await?;
    windows
        .into_iter()
        .find(|w| w.id == window_id)
        .ok_or_else(|| anyhow::anyhow!("Window {} not found", window_id))
}

/// Get focused window from a pre-fetched window list (avoids redundant IPC call).
/// Falls back to IPC only for the focused window ID.
pub async fn get_focused_window_from_cache(niri: &NiriIpc, windows: &[Window]) -> Result<Window> {
    let focused_window_id = niri.get_focused_window_id().await?;
    let window_id = focused_window_id.ok_or_else(|| anyhow::anyhow!("No focused window found"))?;
    windows
        .iter()
        .find(|w| w.id == window_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Window {} not found", window_id))
}

/// Check if a window exists by window_id
pub async fn window_exists(niri: &NiriIpc, window_id: u64) -> Result<bool> {
    let windows = niri.get_windows_raw().await?;
    Ok(windows.iter().any(|w| w.id == window_id))
}

/// Check if a window exists in a pre-fetched list (avoids redundant IPC call).
pub fn window_exists_in_cache(windows: &[Window], window_id: u64) -> bool {
    windows.iter().any(|w| w.id == window_id)
}

/// Wait for a window to appear matching the given pattern
/// Returns the window if found, or error on timeout
pub async fn wait_for_window(
    niri: NiriIpc,
    window_match: &str,
    name: &str,
    max_attempts: u32,
    matcher_cache: &WindowMatcherCache,
) -> Result<Option<Window>> {
    let pattern = if window_match.chars().any(|c| ".+*?[]()".contains(c)) {
        window_match.to_string()
    } else {
        regex::escape(window_match)
    };

    let patterns = vec![pattern];
    let matcher = WindowMatcher::new(Some(&patterns), None);

    for attempt in 1..=max_attempts {
        tokio::time::sleep(Duration::from_millis(100)).await;

        if let Some(window) = find_window_by_matcher(niri.clone(), &matcher, matcher_cache).await? {
            return Ok(Some(window));
        }

        if attempt % 10 == 0 {
            debug!(
                "Still waiting for {} (attempt {}/{})...",
                name, attempt, max_attempts
            );
        }
    }

    // Timeout: Log all available windows to help debug matching issues
    warn!("Timeout waiting for {} (pattern: '{}')", name, window_match);
    if let Ok(windows) = niri.get_windows_raw().await {
        debug!("Available windows at timeout:");
        for window in windows {
            debug!(
                "  - ID: {}, app_id: {:?}, title: {}",
                window.id, window.app_id, window.title
            );
        }
    }

    anyhow::bail!(
        "Timeout waiting for window to appear for {} (pattern: '{}')",
        name,
        window_match
    );
}

/// Window matcher configuration for matching windows by app_id and/or title
#[derive(Debug, Clone)]
pub struct WindowMatcher<'a> {
    /// Optional regex patterns to match app_id (any one matches)
    pub app_id: Option<&'a [String]>,
    /// Optional regex patterns to match title (any one matches)
    pub title: Option<&'a [String]>,
}

impl<'a> WindowMatcher<'a> {
    /// Create a new window matcher
    pub fn new(app_id: Option<&'a [String]>, title: Option<&'a [String]>) -> Self {
        Self { app_id, title }
    }
}

/// Window matcher with regex cache for efficient pattern matching
pub struct WindowMatcherCache {
    regex_cache: Arc<StdMutex<HashMap<String, Arc<Regex>>>>,
}

impl WindowMatcherCache {
    /// Create a new window matcher cache
    pub fn new() -> Self {
        Self {
            regex_cache: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    /// Get or compile a regex pattern (with caching)
    fn get_regex(&self, pattern: &str) -> Result<Arc<Regex>> {
        {
            let cache = self.regex_cache.lock().unwrap();
            if let Some(regex) = cache.get(pattern) {
                return Ok(Arc::clone(regex));
            }
        }
        // Drop the lock before compiling (potentially slow)
        // Compile case-insensitive to handle app_id/title case variations
        // (e.g. "Alacritty" vs "alacritty")
        let regex = Arc::new(
            regex::RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("Failed to compile regex pattern: {}", pattern))?,
        );
        let mut cache = self.regex_cache.lock().unwrap();
        // Double-check after re-acquiring lock (another thread may have inserted)
        if let Some(existing) = cache.get(pattern) {
            return Ok(Arc::clone(existing));
        }
        cache.insert(pattern.to_string(), Arc::clone(&regex));
        Ok(regex)
    }

    /// Check if a window matches the matcher criteria
    /// Returns true if:
    /// - Any app_id pattern matches (if specified)
    /// - Any title pattern matches (if specified)
    /// - If both are specified, match if either matches (OR logic)
    /// - If only one is specified, it must match
    pub fn matches(
        &self,
        window_app_id: Option<&String>,
        window_title: Option<&String>,
        matcher: &WindowMatcher<'_>,
    ) -> Result<bool> {
        // Check app_id match (if specified) - any pattern in the list matches
        if let Some(app_id_patterns) = matcher.app_id {
            if let Some(window_app_id) = window_app_id {
                for pattern in app_id_patterns {
                    let regex = self.get_regex(pattern)?;
                    if regex.is_match(window_app_id) {
                        return Ok(true);
                    }
                }
            }
        }

        // Check title match (if specified) - any pattern in the list matches
        if let Some(title_patterns) = matcher.title {
            if let Some(window_title) = window_title {
                for pattern in title_patterns {
                    let regex = self.get_regex(pattern)?;
                    if regex.is_match(window_title) {
                        return Ok(true);
                    }
                }
            }
        }

        // If both app_id and title are specified, match if either matches (OR logic)
        // If only one is specified, it must match
        Ok(false)
    }

    /// Clear the regex cache (useful when config changes)
    pub fn clear_cache(&self) {
        let mut cache = self.regex_cache.lock().unwrap();
        cache.clear();
    }
}

impl Default for WindowMatcherCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Find a window using WindowMatcher (regex-based matching)
/// This is the unified method for finding windows by app_id and/or title
pub async fn find_window_by_matcher(
    niri: NiriIpc,
    matcher: &WindowMatcher<'_>,
    matcher_cache: &WindowMatcherCache,
) -> Result<Option<Window>> {
    let windows = niri.get_windows_raw().await?;

    for window in windows {
        let matches =
            matcher_cache.matches(window.app_id.as_ref(), Some(&window.title), matcher)?;

        if matches {
            return Ok(Some(window));
        }
    }

    Ok(None)
}

pub async fn get_focused_workspace_from_event(
    niri: &NiriIpc,
    workspace_id: u64,
) -> Result<Option<niri_ipc::Workspace>> {
    let workspaces = niri.get_workspaces().await?;
    Ok(workspaces.into_iter().find(|ws| ws.is_focused && ws.id == workspace_id))
}

pub async fn is_workspace_empty(niri: &NiriIpc, workspace_id: u64) -> Result<bool> {
    let windows = niri.get_windows_raw().await?;
    let workspace_windows: Vec<_> =
        windows.iter().filter(|w| w.workspace_id == Some(workspace_id)).collect();
    Ok(workspace_windows.is_empty())
}

/// Move a window to target workspace by name.
/// If target workspace does not exist, move to first empty workspace and rename it.
pub async fn move_window_to_named_workspace(
    niri: &NiriIpc,
    window: &niri_ipc::Window,
    target_workspace_name: &str,
) -> Result<()> {
    let (target_name, want_output) = target_workspace_name
        .split_once('@')
        .map(|(name, output)| (name, Some(output)))
        .unwrap_or((target_workspace_name, None));

    let workspaces = niri.get_workspaces_for_mapping().await?;
    let windows = niri.get_windows().await?;
    let focused_output = niri.get_focused_output().await.ok().map(|o| o.name);
    debug!(
        "Workspace target='{}' (name='{}', output={:?}), focused output={:?}",
        target_workspace_name, target_name, want_output, focused_output
    );

    let find_on_output = |output_name: &str| -> Option<&niri_ipc::Workspace> {
        workspaces.iter().find(|ws| {
            ws.output.as_deref().is_some_and(|o| {
                o == output_name || super::extract_display_prefix(o) == Some(output_name)
            }) && (ws.name.as_deref() == Some(target_name) || ws.idx.to_string() == target_name)
        })
    };

    let matched_workspace = if let Some(want) = want_output {
        find_on_output(want)
    } else {
        focused_output.as_deref().and_then(find_on_output).or_else(|| {
            workspaces.iter().find(|ws| {
                ws.name.as_deref() == Some(target_name) || ws.idx.to_string() == target_name
            })
        })
    };

    if let Some(target_workspace) = matched_workspace {
        let is_already_there = window.workspace_id == Some(target_workspace.id);
        if !is_already_there {
            info!(
                "Moving window {} to workspace id={} (idx={}, output={:?}, target='{}')",
                window.id,
                target_workspace.id,
                target_workspace.idx,
                target_workspace.output,
                target_workspace_name
            );
            niri.send_action(Action::MoveWindowToWorkspace {
                window_id: Some(window.id),
                reference: WorkspaceReferenceArg::Id(target_workspace.id),
                focus: false,
            })
            .await?;
            tokio::time::sleep(Duration::from_millis(100)).await;
            let _ = focus_window(niri.clone(), window.id).await;
        }
        return Ok(());
    }

    // Multi-monitor aware:
    // prefer empty workspace on the target output first.
    let prefer_output = want_output.or(focused_output.as_deref());
    let empty_on_preferred = prefer_output.and_then(|output_name| {
        workspaces.iter().find(|ws| {
            ws.output.as_deref().is_some_and(|o| {
                o == output_name || super::extract_display_prefix(o) == Some(output_name)
            }) && windows.iter().all(|w| w.workspace_id != Some(ws.id))
        })
    });

    let empty_workspace = empty_on_preferred.or_else(|| {
        workspaces
            .iter()
            .find(|ws| windows.iter().all(|w| w.workspace_id != Some(ws.id)))
    });

    let Some(empty_workspace) = empty_workspace else {
        info!(
            "No empty workspace found for '{}', skip moving window {}",
            target_workspace_name, window.id
        );
        return Ok(());
    };

    info!(
        "Workspace '{}' not found, moving window {} to empty workspace id={} (idx={}, output={:?}) and renaming it",
        target_workspace_name, window.id, empty_workspace.id, empty_workspace.idx, empty_workspace.output
    );
    niri.send_action(Action::MoveWindowToWorkspace {
        window_id: Some(window.id),
        reference: WorkspaceReferenceArg::Id(empty_workspace.id),
        focus: false,
    })
    .await?;
    niri.send_action(Action::SetWorkspaceName {
        name: target_name.to_string(),
        workspace: Some(WorkspaceReferenceArg::Id(empty_workspace.id)),
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = focus_window(niri.clone(), window.id).await;

    Ok(())
}

/// Check if a window is in the current workspace
pub fn is_window_in_workspace(window: &Window, workspace: &crate::niri::Workspace) -> bool {
    window.workspace_id == Some(workspace.id)
}

/// Get current workspace and all windows (commonly used together)
pub async fn get_workspace_and_windows(
    niri: &NiriIpc,
) -> Result<(crate::niri::Workspace, Vec<Window>)> {
    let current_workspace = niri.get_focused_workspace().await?;
    let windows = niri.get_windows().await?;
    Ok((current_workspace, windows))
}

/// Calculate position based on direction (for visible positions)
/// Returns (x, y) coordinates
pub fn calculate_position(
    direction: Direction,
    output_width: u32,
    output_height: u32,
    window_width: u32,
    window_height: u32,
    margin: u32,
) -> (i32, i32) {
    match direction {
        Direction::Top => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = margin as i32;
            (x, y)
        }
        Direction::Bottom => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = (output_height - window_height - margin) as i32;
            (x, y)
        }
        Direction::Left => {
            let x = margin as i32;
            let y = ((output_height - window_height) / 2) as i32;
            (x, y)
        }
        Direction::Right => {
            let x = (output_width - window_width - margin) as i32;
            let y = ((output_height - window_height) / 2) as i32;
            (x, y)
        }
    }
}

/// Extract margin from current position based on direction
pub fn extract_margin(
    direction: Direction,
    output_width: u32,
    output_height: u32,
    window_width: u32,
    window_height: u32,
    x: i32,
    y: i32,
) -> u32 {
    let margin = match direction {
        Direction::Top => y,
        Direction::Bottom => output_height as i32 - window_height as i32 - y,
        Direction::Left => x,
        Direction::Right => output_width as i32 - window_width as i32 - x,
    };
    margin.max(0) as u32
}

/// Calculate off-screen position based on direction (for hidden positions)
/// Returns (x, y) coordinates where window is completely outside the screen
pub fn calculate_hide_position(
    direction: Direction,
    output_width: u32,
    output_height: u32,
    window_width: u32,
    window_height: u32,
    margin: u32,
) -> (i32, i32) {
    match direction {
        Direction::Top => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = -((window_height + margin) as i32);
            (x, y)
        }
        Direction::Bottom => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = (output_height + margin) as i32;
            (x, y)
        }
        Direction::Left => {
            let x = -((window_width + margin) as i32);
            let y = ((output_height - window_height) / 2) as i32;
            (x, y)
        }
        Direction::Right => {
            let x = (output_width + margin) as i32;
            let y = ((output_height - window_height) / 2) as i32;
            (x, y)
        }
    }
}

/// Move window from current position to target position
/// Automatically calculates the relative offset and moves the window
pub async fn move_window_to_position(
    niri: &NiriIpc,
    window_id: u64,
    current_x: i32,
    current_y: i32,
    target_x: i32,
    target_y: i32,
) -> Result<()> {
    let rel_x = target_x - current_x;
    let rel_y = target_y - current_y;

    debug!(
        "Moving window {} from ({}, {}) to ({}, {}) with relative movement ({}, {})",
        window_id, current_x, current_y, target_x, target_y, rel_x, rel_y
    );

    niri.move_window_relative(window_id, rel_x, rel_y).await?;
    Ok(())
}

/// Result of sticky_follow_workspace
pub enum StickyFollowResult {
    /// Window was moved or already in place, sticky state should be kept
    Keep,
    /// Window no longer exists, sticky state should be cleared
    WindowGone,
    /// Window is no longer floating, sticky state should be cleared
    NotFloating,
    /// Window moved to a different output; callers should resize/reposition
    OutputChanged {
        old_output_width: u32,
        old_output_height: u32,
        new_output_width: u32,
        new_output_height: u32,
    },
}

/// Shared sticky follow logic: move a floating window to the focused workspace.
/// Used by both StickyPlugin and ScratchpadManager.
/// Returns a StickyFollowResult indicating whether the sticky state should be kept or cleared.
pub async fn sticky_follow_workspace(
    niri: &NiriIpc,
    window_id: u64,
    cross_monitor: bool,
) -> Result<StickyFollowResult> {
    let windows = niri.get_windows_raw().await?;
    let window = match windows.iter().find(|w| w.id == window_id) {
        Some(w) => w,
        None => {
            warn!("Sticky window {} no longer exists", window_id);
            return Ok(StickyFollowResult::WindowGone);
        }
    };

    if !window.floating {
        warn!("Sticky window {} is no longer floating", window_id);
        return Ok(StickyFollowResult::NotFloating);
    }

    let window_workspace_id = window.workspace_id;

    if cross_monitor {
        let old_output_name = if let Some(ws_id) = window_workspace_id {
            let ws_list = niri.get_workspaces_for_mapping().await?;
            ws_list.iter().find(|ws| ws.id == ws_id).and_then(|ws| ws.output.clone())
        } else {
            None
        };

        // Get old output size
        let (old_ow, old_oh) = if let Some(ref name) = old_output_name {
            niri.get_output_size_by_name(name).unwrap_or((0, 0))
        } else {
            (0, 0)
        };

        // Move to focused output
        niri.move_floating_window(window_id).await?;

        // Get new output size (focused output is now the target)
        let (new_ow, new_oh) = niri.get_output_size().await?;

        // If output actually changed dimensions, signal caller
        if old_ow > 0 && old_oh > 0 && (old_ow != new_ow || old_oh != new_oh) {
            return Ok(StickyFollowResult::OutputChanged {
                old_output_width: old_ow,
                old_output_height: old_oh,
                new_output_width: new_ow,
                new_output_height: new_oh,
            });
        }

        return Ok(StickyFollowResult::Keep);
    }

    let focused_workspace = niri.get_focused_workspace().await?;
    let workspaces = niri.get_workspaces_for_mapping().await?;

    let target_workspace = workspaces
        .iter()
        .find(|ws| ws.idx.to_string() == focused_workspace.name)
        .context("Focused workspace not found in workspace list")?;

    if let Some(ws_id) = window_workspace_id {
        if let Some(source_workspace) = workspaces.iter().find(|ws| ws.id == ws_id) {
            if source_workspace.output != target_workspace.output {
                debug!(
                    "Sticky skip move: cross=false and output differs (from {:?} to {:?})",
                    source_workspace.output, target_workspace.output
                );
                return Ok(StickyFollowResult::Keep);
            }
        }
    }

    niri.move_window_to_workspace(window_id, &focused_workspace.name).await?;
    Ok(StickyFollowResult::Keep)
}

/// Check if a window matches the given matcher (with optional exclude patterns)
/// This is a generic window matching function that supports both include and exclude patterns
pub fn matches_window(
    window: &Window,
    app_id_patterns: Option<&Vec<String>>,
    title_patterns: Option<&Vec<String>>,
    exclude_app_id_patterns: Option<&Vec<String>>,
    exclude_title_patterns: Option<&Vec<String>>,
    matcher_cache: &WindowMatcherCache,
) -> Result<bool> {
    // First check exclude rules
    if let Some(exclude_patterns) = exclude_app_id_patterns {
        let exclude_matcher = WindowMatcher::new(Some(exclude_patterns.as_slice()), None);
        if matcher_cache.matches(
            window.app_id.as_ref(),
            Some(&window.title),
            &exclude_matcher,
        )? {
            return Ok(false);
        }
    }

    if let Some(exclude_patterns) = exclude_title_patterns {
        let exclude_matcher = WindowMatcher::new(None, Some(exclude_patterns.as_slice()));
        if matcher_cache.matches(
            window.app_id.as_ref(),
            Some(&window.title),
            &exclude_matcher,
        )? {
            return Ok(false);
        }
    }

    // If no include patterns specified, match all (unless excluded)
    if app_id_patterns.is_none() && title_patterns.is_none() {
        return Ok(true);
    }

    // Check include patterns
    let matcher = WindowMatcher::new(
        app_id_patterns.map(|v| v.as_slice()),
        title_patterns.map(|v| v.as_slice()),
    );
    matcher_cache.matches(window.app_id.as_ref(), Some(&window.title), &matcher)
}

/// Try to find parent window using PID-based matching.
/// Checks if any window's PID is in the child window's ancestor process tree.
pub async fn try_pid_matching(
    child_window: &Window,
    windows: &[Window],
    window_pid_map: Arc<Mutex<HashMap<u32, Vec<u64>>>>,
) -> Result<Option<Window>> {
    let child_pid = match child_window.pid {
        Some(pid) => {
            let mut map = window_pid_map.lock().await;
            map.entry(pid).or_insert_with(Vec::new).push(child_window.id);
            pid
        }
        None => {
            debug!("No PID found for child window {}", child_window.id);
            return Ok(None);
        }
    };

    debug!(
        "Trying PID matching: child window {} (app_id={:?}, title={}) has PID {}",
        child_window.id, child_window.app_id, child_window.title, child_pid
    );

    // Build ancestor process tree using blocking I/O in a dedicated thread.
    // /proc reads are fast but should not block the async runtime.
    let ancestor_pids: HashSet<u32> = tokio::task::spawn_blocking(move || {
        let mut ancestor_pids = HashSet::new();
        let mut current_pid = child_pid;

        loop {
            let stat_path = format!("/proc/{}/stat", current_pid);
            let stat = match std::fs::read_to_string(&stat_path) {
                Ok(s) => s,
                Err(_) => break,
            };

            let fields: Vec<&str> = stat.split_whitespace().collect();
            if fields.len() < 4 {
                break;
            }

            let p_pid = match fields[3].parse::<u32>() {
                Ok(pid) => pid,
                Err(_) => break,
            };

            if p_pid == 0 || p_pid == 1 {
                break;
            }

            ancestor_pids.insert(p_pid);
            current_pid = p_pid;
        }

        ancestor_pids
    })
    .await
    .unwrap_or_default();

    if log::log_enabled!(log::Level::Debug) && !ancestor_pids.is_empty() {
        let pids = ancestor_pids.clone();
        let child_id = child_window.id;
        let log_parts: Vec<String> = tokio::task::spawn_blocking(move || {
            pids.iter()
                .map(|&pid| {
                    let comm = std::fs::read_to_string(format!("/proc/{}/comm", pid))
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "unknown".to_string());
                    format!("{} ({})", pid, comm)
                })
                .collect()
        })
        .await
        .unwrap_or_default();
        debug!(
            "Process tree PIDs for child {}: {}",
            child_id,
            log_parts.join(" -> ")
        );
    }

    // Search for parent window whose PID is in the ancestor tree
    // Batch all pid_map updates into a single lock acquisition
    {
        let mut map = window_pid_map.lock().await;
        for window in windows {
            if window.id == child_window.id {
                continue;
            }

            let Some(window_pid) = window.pid else {
                continue;
            };

            map.entry(window_pid).or_insert_with(Vec::new).push(window.id);

            if ancestor_pids.contains(&window_pid) {
                debug!(
                    "Found parent window {} (app_id={:?}, title={}) in process tree (PID: {})",
                    window.id, window.app_id, window.title, window_pid
                );
                return Ok(Some(window.clone()));
            }
        }
    }

    Ok(None)
}

/// Perform swallow operation on a parent window
/// This function handles the entire swallow process including:
/// - Focusing the parent window
/// - Ensuring child window is not floating
/// - Moving child window to parent's workspace if needed
/// - Consuming child window into parent's column
/// - Focusing the child window
pub async fn perform_swallow(
    niri: &NiriIpc,
    parent_window: &Window,
    child_window: &Window,
    child_window_id: u64,
    column_display: ColumnDisplay,
) -> Result<()> {
    // Prepare workspace reference if needed
    let workspace_ref = if let Some(workspace_id) = parent_window.workspace_id {
        if child_window.workspace_id != Some(workspace_id) {
            let workspaces = niri.get_workspaces_for_mapping().await?;
            workspaces.iter().find(|ws| ws.id == workspace_id).map(|workspace| {
                workspace.name.as_ref().cloned().unwrap_or_else(|| workspace.idx.to_string())
            })
        } else {
            None
        }
    } else {
        None
    };

    // Copy values needed in the closure to avoid lifetime issues
    let parent_window_id = parent_window.id;
    let child_is_floating = child_window.floating;

    // Batch all actions together for faster execution
    niri.execute_batch(move |socket| {
        // 1. Focus parent window first
        match socket.send(Request::Action(Action::FocusWindow {
            id: parent_window_id,
        }))? {
            Reply::Ok(_) => {}
            Reply::Err(err) => anyhow::bail!("Failed to focus parent window: {}", err),
        }

        // 2. Set column display (Tabbed or Normal)
        let _ = socket.send(Request::Action(Action::SetColumnDisplay {
            display: column_display,
        }))?;

        // 3. Ensure child window is not floating (floating windows cannot be swallowed into columns)
        if child_is_floating {
            let _ = socket.send(Request::Action(Action::MoveWindowToTiling {
                id: Some(child_window_id),
            }))?;
        }

        // 4. Move child window to parent's workspace if needed
        // To ensure they are neighbors (required for ConsumeOrExpelWindowLeft)
        if let Some(workspace_ref_str) = workspace_ref.as_ref() {
            let workspace_ref_arg = if let Ok(idx) = workspace_ref_str.parse::<u8>() {
                WorkspaceReferenceArg::Index(idx)
            } else if let Ok(id) = workspace_ref_str.parse::<u64>() {
                WorkspaceReferenceArg::Id(id)
            } else {
                WorkspaceReferenceArg::Name(workspace_ref_str.clone())
            };
            let _ = socket.send(Request::Action(Action::MoveWindowToWorkspace {
                window_id: Some(child_window_id),
                reference: workspace_ref_arg,
                focus: false,
            }))?;
        }

        // 5. Consume child window into parent's column
        let _ = socket.send(Request::Action(Action::ConsumeOrExpelWindowLeft {
            id: Some(child_window_id),
        }))?;

        // 6. Focus child window
        let _ = socket.send(Request::Action(Action::FocusWindow {
            id: child_window_id,
        }))?;

        Ok::<(), anyhow::Error>(())
    })
    .await?;

    Ok(())
}
