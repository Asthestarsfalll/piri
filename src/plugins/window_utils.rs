use anyhow::{Context, Result};
use log::{debug, warn};
use regex::Regex;
use std::collections::HashMap;
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
