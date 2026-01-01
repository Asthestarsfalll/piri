use anyhow::{Context, Result};
use log::{debug, warn};
use std::process::{Command, Stdio};
use tokio::time::{sleep, Duration};

use crate::niri::NiriIpc;
use crate::niri::Window;

/// Launch an application by executing a command
pub async fn launch_application(command: &str) -> Result<()> {
    debug!("Launching: {}", command);

    // Parse command - it may contain environment variables and arguments
    // Use shell to execute the full command
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to launch application: {}", command))?;

    Ok(())
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
pub async fn wait_for_window(
    niri: NiriIpc,
    window_match: &str,
    name: &str,
    max_attempts: u32,
) -> Result<Option<Window>> {
    let mut attempts = 0;

    loop {
        sleep(Duration::from_millis(100)).await;
        attempts += 1;

        if let Some(window) = niri.find_window_async(window_match).await? {
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
