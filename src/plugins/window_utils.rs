use anyhow::{Context, Result};
use log::{debug, warn};
use niri_ipc::{Action, Reply, Request, WorkspaceReferenceArg};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

use crate::config::Direction;
use crate::niri::NiriIpc;
use crate::niri::Window;

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

pub async fn get_focused_window(niri: &NiriIpc) -> Result<Window> {
    let focused_window_id = niri.get_focused_window_id().await?;
    let window_id = focused_window_id.ok_or_else(|| anyhow::anyhow!("No focused window found"))?;
    let windows = niri.get_windows().await?;
    windows
        .into_iter()
        .find(|w| w.id == window_id)
        .ok_or_else(|| anyhow::anyhow!("Window {} not found", window_id))
}

/// Check if a window exists by window_id
pub async fn window_exists(niri: &NiriIpc, window_id: u64) -> Result<bool> {
    let windows = niri.get_windows().await?;
    Ok(windows.iter().any(|w| w.id == window_id))
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

    let matcher = WindowMatcher::new(Some(vec![pattern]), None);

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
    if let Ok(windows) = niri.get_windows().await {
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
pub struct WindowMatcher {
    /// Optional regex patterns to match app_id (any one matches)
    pub app_id: Option<Vec<String>>,
    /// Optional regex patterns to match title (any one matches)
    pub title: Option<Vec<String>>,
}

impl WindowMatcher {
    /// Create a new window matcher
    pub fn new(app_id: Option<Vec<String>>, title: Option<Vec<String>>) -> Self {
        Self { app_id, title }
    }
}

/// Window matcher with regex cache for efficient pattern matching
pub struct WindowMatcherCache {
    regex_cache: Arc<Mutex<HashMap<String, Regex>>>,
}

impl WindowMatcherCache {
    /// Create a new window matcher cache
    pub fn new() -> Self {
        Self {
            regex_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get or compile a regex pattern (with caching)
    async fn get_regex(&self, pattern: &str) -> Result<Regex> {
        let mut cache = self.regex_cache.lock().await;
        if let Some(regex) = cache.get(pattern) {
            return Ok(regex.clone());
        }

        let regex = Regex::new(pattern)
            .with_context(|| format!("Failed to compile regex pattern: {}", pattern))?;
        cache.insert(pattern.to_string(), regex.clone());
        Ok(regex)
    }

    /// Check if a window matches the matcher criteria
    /// Returns true if:
    /// - Any app_id pattern matches (if specified)
    /// - Any title pattern matches (if specified)
    /// - If both are specified, match if either matches (OR logic)
    /// - If only one is specified, it must match
    pub async fn matches(
        &self,
        window_app_id: Option<&String>,
        window_title: Option<&String>,
        matcher: &WindowMatcher,
    ) -> Result<bool> {
        // Check app_id match (if specified) - any pattern in the list matches
        if let Some(ref app_id_patterns) = matcher.app_id {
            if let Some(window_app_id) = window_app_id {
                for pattern in app_id_patterns {
                    let regex = self.get_regex(pattern).await?;
                    if regex.is_match(window_app_id) {
                        return Ok(true);
                    }
                }
            }
        }

        // Check title match (if specified) - any pattern in the list matches
        if let Some(ref title_patterns) = matcher.title {
            if let Some(window_title) = window_title {
                for pattern in title_patterns {
                    let regex = self.get_regex(pattern).await?;
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
    pub async fn clear_cache(&self) {
        let mut cache = self.regex_cache.lock().await;
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
    matcher: &WindowMatcher,
    matcher_cache: &WindowMatcherCache,
) -> Result<Option<Window>> {
    let windows = niri.get_windows().await?;

    for window in windows {
        let matches = matcher_cache
            .matches(window.app_id.as_ref(), Some(&window.title), matcher)
            .await?;

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
    let windows = niri.get_windows().await?;
    let workspace_windows: Vec<_> =
        windows.iter().filter(|w| w.workspace_id == Some(workspace_id)).collect();
    Ok(workspace_windows.is_empty())
}

/// Match workspace by exact name or idx
/// Returns the workspace identifier (name if available, otherwise idx as string)
/// Matching order: 1. exact name match, 2. exact idx match
pub async fn match_workspace(target_workspace: &str, niri: NiriIpc) -> Result<Option<String>> {
    let workspaces = niri.get_workspaces_for_mapping().await?;

    // First pass: exact name match
    for workspace in &workspaces {
        if let Some(ref name) = workspace.name {
            if name == target_workspace {
                debug!(
                    "Matched workspace by name: {} -> {}",
                    target_workspace, name
                );
                return Ok(Some(name.clone()));
            }
        }
    }

    // Second pass: exact idx match
    if let Ok(target_idx) = target_workspace.parse::<u8>() {
        for workspace in &workspaces {
            if workspace.idx == target_idx {
                let result = workspace.name.clone().unwrap_or_else(|| workspace.idx.to_string());
                debug!(
                    "Matched workspace by idx: {} -> {}",
                    target_workspace, result
                );
                return Ok(Some(result));
            }
        }
    }

    debug!("No matching workspace found for: {}", target_workspace);
    Ok(None)
}

/// Check if a window is in the current workspace
pub fn is_window_in_workspace(window: &Window, workspace: &crate::niri::Workspace) -> bool {
    match (&window.workspace, &window.workspace_id) {
        (Some(ws), _) => ws == &workspace.name,
        (_, Some(ws_id)) => ws_id.to_string() == workspace.name,
        _ => false,
    }
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
        Direction::FromTop => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = margin as i32;
            (x, y)
        }
        Direction::FromBottom => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = (output_height - window_height - margin) as i32;
            (x, y)
        }
        Direction::FromLeft => {
            let x = margin as i32;
            let y = ((output_height - window_height) / 2) as i32;
            (x, y)
        }
        Direction::FromRight => {
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
        Direction::FromTop => y,
        Direction::FromBottom => output_height as i32 - window_height as i32 - y,
        Direction::FromLeft => x,
        Direction::FromRight => output_width as i32 - window_width as i32 - x,
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
        Direction::FromTop => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = -((window_height + margin) as i32);
            (x, y)
        }
        Direction::FromBottom => {
            let x = ((output_width - window_width) / 2) as i32;
            let y = (output_height + margin) as i32;
            (x, y)
        }
        Direction::FromLeft => {
            let x = -((window_width + margin) as i32);
            let y = ((output_height - window_height) / 2) as i32;
            (x, y)
        }
        Direction::FromRight => {
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

/// Check if a window matches the given matcher (with optional exclude patterns)
/// This is a generic window matching function that supports both include and exclude patterns
pub async fn matches_window(
    window: &Window,
    app_id_patterns: Option<&Vec<String>>,
    title_patterns: Option<&Vec<String>>,
    exclude_app_id_patterns: Option<&Vec<String>>,
    exclude_title_patterns: Option<&Vec<String>>,
    matcher_cache: &WindowMatcherCache,
) -> Result<bool> {
    // First check exclude rules
    if let Some(exclude_patterns) = exclude_app_id_patterns {
        let exclude_matcher = WindowMatcher::new(Some(exclude_patterns.clone()), None);
        if matcher_cache
            .matches(
                window.app_id.as_ref(),
                Some(&window.title),
                &exclude_matcher,
            )
            .await?
        {
            return Ok(false);
        }
    }

    if let Some(exclude_patterns) = exclude_title_patterns {
        let exclude_matcher = WindowMatcher::new(None, Some(exclude_patterns.clone()));
        if matcher_cache
            .matches(
                window.app_id.as_ref(),
                Some(&window.title),
                &exclude_matcher,
            )
            .await?
        {
            return Ok(false);
        }
    }

    // If no include patterns specified, match all (unless excluded)
    if app_id_patterns.is_none() && title_patterns.is_none() {
        return Ok(true);
    }

    // Check include patterns
    let matcher = WindowMatcher::new(app_id_patterns.cloned(), title_patterns.cloned());
    matcher_cache
        .matches(window.app_id.as_ref(), Some(&window.title), &matcher)
        .await
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

    // Build ancestor process tree set for O(1) lookup
    let mut ancestor_pids = HashSet::new();
    let mut current_pid = child_pid;
    let mut ancestor_list = Vec::new();

    loop {
        let stat_path = format!("/proc/{}/stat", current_pid);
        let stat = match tokio::fs::read_to_string(&stat_path).await {
            Ok(stat) => stat,
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
        ancestor_list.push(p_pid);
        current_pid = p_pid;
    }

    if !ancestor_list.is_empty() {
        let mut log_parts = Vec::new();
        for &pid in &ancestor_list {
            let comm = tokio::fs::read_to_string(format!("/proc/{}/comm", pid))
                .await
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            log_parts.push(format!("{} ({})", pid, comm));
        }
        debug!(
            "Process tree PIDs for child {}: {}",
            child_window.id,
            log_parts.join(" -> ")
        );
    }

    // Search for parent window whose PID is in the ancestor tree
    for window in windows {
        if window.id == child_window.id {
            continue;
        }

        let Some(window_pid) = window.pid else {
            continue;
        };

        {
            let mut map = window_pid_map.lock().await;
            map.entry(window_pid).or_insert_with(Vec::new).push(window.id);
        }

        if ancestor_pids.contains(&window_pid) {
            debug!(
                "Found parent window {} (app_id={:?}, title={}) in process tree (PID: {})",
                window.id, window.app_id, window.title, window_pid
            );
            return Ok(Some(window.clone()));
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
) -> Result<()> {
    // Prepare workspace reference if needed
    let workspace_ref = if let Some(workspace_id) = parent_window.workspace_id {
        if child_window.workspace_id != Some(workspace_id) {
            let workspaces = niri.get_workspaces_for_mapping().await?;
            if let Some(workspace) = workspaces.iter().find(|ws| ws.id == workspace_id) {
                Some(workspace.name.as_ref().cloned().unwrap_or_else(|| workspace.idx.to_string()))
            } else {
                None
            }
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

        // 2. Ensure child window is not floating (floating windows cannot be swallowed into columns)
        if child_is_floating {
            let _ = socket.send(Request::Action(Action::MoveWindowToTiling {
                id: Some(child_window_id),
            }))?;
        }

        // 3. Move child window to parent's workspace if needed
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

        // 4. Consume child window into parent's column
        let _ = socket.send(Request::Action(Action::ConsumeOrExpelWindowLeft {
            id: Some(child_window_id),
        }))?;

        // 5. Focus child window
        let _ = socket.send(Request::Action(Action::FocusWindow {
            id: child_window_id,
        }))?;

        Ok::<(), anyhow::Error>(())
    })
    .await?;

    Ok(())
}
