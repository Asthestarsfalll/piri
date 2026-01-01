use anyhow::{Context, Result};
use log::{debug, warn};
use regex::Regex;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

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

/// Helper function to run blocking operations with NiriIpc
pub async fn run_blocking<F, T>(niri: NiriIpc, f: F) -> Result<T>
where
    F: FnOnce(NiriIpc) -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || f(niri)).await.context("Task join error")?
}

/// Focus a window by ID
pub async fn focus_window(niri: NiriIpc, window_id: u64) -> Result<()> {
    run_blocking(niri, move |niri| niri.focus_window(window_id)).await
}

/// Switch to a workspace
pub async fn switch_to_workspace(niri: NiriIpc, workspace: &str) -> Result<()> {
    let workspace = workspace.to_string();
    run_blocking(niri, move |niri| niri.switch_to_workspace(&workspace)).await?;
    // Small delay to ensure workspace switch completes
    sleep(Duration::from_millis(100)).await;
    Ok(())
}

/// Wait for a window to appear matching the given pattern
/// Returns the window if found, or None if timeout
/// Uses WindowMatcher for regex-based matching
pub async fn wait_for_window(
    niri: NiriIpc,
    window_match: &str,
    name: &str,
    max_attempts: u32,
) -> Result<Option<Window>> {
    // Create a matcher from the pattern (treat as app_id pattern by default)
    // For backward compatibility, if pattern doesn't look like regex, escape it
    let pattern = if window_match
        .chars()
        .any(|c| c == '.' || c == '*' || c == '+' || c == '?' || c == '[' || c == '(')
    {
        window_match.to_string()
    } else {
        // Escape special regex characters for simple string matching
        regex::escape(window_match)
    };

    let matcher = WindowMatcher::new(Some(pattern), None);
    let matcher_cache = WindowMatcherCache::new();

    let mut attempts = 0;

    loop {
        sleep(Duration::from_millis(100)).await;
        attempts += 1;

        if let Some(window) = find_window_by_matcher(niri.clone(), &matcher, &matcher_cache).await?
        {
            return Ok(Some(window));
        }

        // Log available windows every 10 attempts (every second) for debugging
        if attempts % 10 == 0 {
            debug!(
                "Still waiting for window (attempt {}/{})...",
                attempts, max_attempts
            );
            if let Ok(windows) = run_blocking(niri.clone(), |niri| niri.get_windows()).await {
                debug!("Available windows: {}", windows.len());
                for window in windows.iter().take(5) {
                    debug!(
                        "  - ID: {}, app_id: {:?}, class: {:?}, title: {}",
                        window.id, window.app_id, window.class, window.title
                    );
                }
            }
        }

        if attempts >= max_attempts {
            // Before bailing, list all available windows for debugging
            warn!(
                "Timeout waiting for window matching '{}' for {}",
                window_match, name
            );
            if let Ok(windows) = run_blocking(niri.clone(), |niri| niri.get_windows()).await {
                warn!("Available windows at timeout ({} total):", windows.len());
                for window in windows.iter() {
                    warn!(
                        "  - ID: {}, app_id: {:?}, class: {:?}, title: {}",
                        window.id, window.app_id, window.class, window.title
                    );
                }
            }
            anyhow::bail!(
                "Timeout waiting for window to appear for {} (searched for pattern: '{}')",
                name,
                window_match
            );
        }
    }
}

/// Window matcher configuration for matching windows by app_id and/or title
#[derive(Debug, Clone)]
pub struct WindowMatcher {
    /// Optional regex pattern to match app_id
    pub app_id: Option<String>,
    /// Optional regex pattern to match title
    pub title: Option<String>,
}

impl WindowMatcher {
    /// Create a new window matcher
    pub fn new(app_id: Option<String>, title: Option<String>) -> Self {
        Self { app_id, title }
    }

    /// Check if at least one matching criteria is specified
    pub fn has_criteria(&self) -> bool {
        self.app_id.is_some() || self.title.is_some()
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
    /// - app_id pattern matches (if specified)
    /// - title pattern matches (if specified)
    /// - If both are specified, match if either matches (OR logic)
    /// - If only one is specified, it must match
    pub async fn matches(
        &self,
        window_app_id: Option<&String>,
        window_title: Option<&String>,
        matcher: &WindowMatcher,
    ) -> Result<bool> {
        // Check app_id match (if specified)
        if let Some(ref app_id_pattern) = matcher.app_id {
            if let Some(window_app_id) = window_app_id {
                let regex = self.get_regex(app_id_pattern).await?;
                if regex.is_match(window_app_id) {
                    return Ok(true);
                }
            }
        }

        // Check title match (if specified)
        if let Some(ref title_pattern) = matcher.title {
            if let Some(window_title) = window_title {
                let regex = self.get_regex(title_pattern).await?;
                if regex.is_match(window_title) {
                    return Ok(true);
                }
            }
        }

        // If both app_id and title are specified, match if either matches (OR logic)
        // If only one is specified, it must match
        Ok(false)
    }

    /// Clear the regex cache (useful when config changes)
    pub async fn clear_cache(&self) {
        self.regex_cache.lock().await.clear();
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
    let windows = run_blocking(niri, |niri| niri.get_windows()).await?;

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

/// Find a window by app_id pattern (convenience function)
/// Creates a WindowMatcher with app_id pattern and uses shared cache
pub async fn find_window_by_app_id(
    niri: NiriIpc,
    app_id_pattern: &str,
    matcher_cache: &WindowMatcherCache,
) -> Result<Option<Window>> {
    let matcher = WindowMatcher::new(Some(app_id_pattern.to_string()), None);
    find_window_by_matcher(niri, &matcher, matcher_cache).await
}

/// Match workspace by exact name or idx
/// Returns the workspace identifier (name if available, otherwise idx as string)
/// Matching order: 1. exact name match, 2. exact idx match
pub async fn match_workspace(target_workspace: &str, niri: NiriIpc) -> Result<Option<String>> {
    let workspaces = run_blocking(niri.clone(), |niri| niri.get_workspaces_for_mapping()).await?;

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
