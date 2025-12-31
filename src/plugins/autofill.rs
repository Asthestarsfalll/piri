use anyhow::{Context, Result};
use log::{debug, info, warn};
use niri_ipc::{Action, Event, Reply, Request};
use tokio::time::Duration;

use crate::niri::NiriIpc;
pub struct AutofillPlugin {
    niri: NiriIpc,
    /// Event listener task handle
    event_listener_handle: Option<tokio::task::JoinHandle<()>>,
}

impl AutofillPlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None).expect("Failed to initialize niri IPC"),
            event_listener_handle: None,
        }
    }

    /// Event listener loop that listens to niri events
    async fn event_listener_loop(niri: NiriIpc) -> Result<()> {
        info!("Autofill plugin event listener started");

        // Outer loop: reconnect on connection failure
        loop {
            let socket = match niri.create_event_stream_socket() {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to create event stream: {}, retrying in 1s", e);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
            };

            let mut read_event = socket.read_events();
            info!("Event stream connected, waiting for events...");

            while let Ok(event) = read_event() {
                debug!("Raw event received: {:?}", event);
                if let Err(e) = Self::handle_event(event, &niri).await {
                    warn!("Error handling event: {}", e);
                }
            }

            // Connection closed or error - will reconnect in outer loop
            warn!("Event stream closed, reconnecting...");

            // Reconnect after error
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    /// Handle a single event - all events trigger the same alignment check
    async fn handle_event(event: Event, niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowClosed { .. } => {
                debug!("Received WindowClosed event, triggering alignment check");
                Self::check_and_align_last_column(niri).await?;
            }
            Event::WindowLayoutsChanged { .. } => {
                debug!("Received WindowLayoutsChanged event, triggering alignment check");
                Self::check_and_align_last_column(niri).await?;
            }
            other => {
                // Log other events for debugging
                debug!("Received other event: {:?}", other);
            }
        }

        Ok(())
    }

    /// Check if current column is the last one and align it to the right
    async fn check_and_align_last_column(niri: &NiriIpc) -> Result<()> {
        // Get current focused window to determine workspace
        let niri_clone = niri.clone();
        let focused_window_result =
            tokio::task::spawn_blocking(move || niri_clone.get_focused_window_id())
                .await
                .context("Task join error")?;

        let focused_window_id = match focused_window_result {
            Ok(Some(id)) => id,
            Ok(None) => {
                debug!("No focused window, skipping column alignment");
                return Ok(());
            }
            Err(e) => {
                debug!("Failed to get focused window: {}", e);
                return Ok(());
            }
        };

        // Get all windows to check column information
        let niri_clone = niri.clone();
        let windows_result = tokio::task::spawn_blocking(move || niri_clone.get_windows())
            .await
            .context("Task join error")?;

        let windows = match windows_result {
            Ok(windows) => windows,
            Err(e) => {
                debug!("Failed to get windows: {}", e);
                return Ok(());
            }
        };

        // Find the focused window to get its workspace ID
        let focused_window = windows.iter().find(|w| w.id == focused_window_id);
        let focused_workspace_id = match focused_window {
            Some(w) => match w.workspace_id {
                Some(id) => id,
                None => {
                    debug!("Focused window has no workspace ID, skipping");
                    return Ok(());
                }
            },
            None => {
                debug!("Focused window not found in windows list");
                return Ok(());
            }
        };

        // Get all non-floating windows in the current workspace
        let workspace_windows: Vec<_> = windows
            .iter()
            .filter(|w| w.workspace_id == Some(focused_workspace_id) && !w.floating)
            .collect();

        // If only one window, no need to align
        if workspace_windows.len() <= 1 {
            debug!(
                "Only {} window(s) in workspace, skipping alignment",
                workspace_windows.len()
            );
            return Ok(());
        }

        // Find the maximum column number and get a window from the last column
        let last_column_window = workspace_windows
            .iter()
            .filter_map(|w| {
                // Extract column number from layout
                w.layout
                    .as_ref()
                    .and_then(|l| l.pos_in_scrolling_layout)
                    .map(|(column, _tile)| (column, w.id))
            })
            .max_by_key(|(column, _)| *column)
            .map(|(_, window_id)| window_id);

        let last_column_window_id = match last_column_window {
            Some(id) => id,
            None => {
                debug!("No windows with column position found, skipping");
                return Ok(());
            }
        };

        info!(
            "Found last column window {}, aligning to rightmost position",
            last_column_window_id
        );
        Self::align_column_to_right(niri, last_column_window_id).await?;

        Ok(())
    }

    async fn align_column_to_right(niri: &NiriIpc, window_id: u64) -> Result<()> {
        info!(
            "Aligning column to rightmost position for window {}",
            window_id
        );

        // First, focus the window to ensure we're on the correct column
        let focus_result = tokio::task::spawn_blocking(move || {
            let mut socket =
                niri_ipc::socket::Socket::connect().context("Failed to connect to niri socket")?;
            let action = Action::FocusWindow { id: window_id };
            let request = Request::Action(action);
            match socket.send(request)? {
                Reply::Ok(_) => Ok(()),
                Reply::Err(err) => Err(anyhow::anyhow!("Failed to focus window: {}", err)),
            }
        })
        .await
        .context("Task join error")?;

        if let Err(e) = focus_result {
            warn!("Failed to focus window: {}", e);
            return Err(e);
        }

        // Then, focus column left (moves to second-to-last if currently last)
        let focus_left_result = tokio::task::spawn_blocking(move || {
            let mut socket =
                niri_ipc::socket::Socket::connect().context("Failed to connect to niri socket")?;
            let action = Action::FocusColumnLeft {};
            let request = Request::Action(action);
            match socket.send(request)? {
                Reply::Ok(_) => Ok(()),
                Reply::Err(err) => Err(anyhow::anyhow!("Failed to focus column left: {}", err)),
            }
        })
        .await
        .context("Task join error")?;

        if let Err(e) = focus_left_result {
            warn!("Failed to focus column left: {}", e);
            return Err(e);
        }

        // Finally, focus column right (moves to last, which is rightmost)
        let focus_right_result = tokio::task::spawn_blocking(move || {
            let mut socket =
                niri_ipc::socket::Socket::connect().context("Failed to connect to niri socket")?;
            let action = Action::FocusColumnRight {};
            let request = Request::Action(action);
            match socket.send(request)? {
                Reply::Ok(_) => Ok(()),
                Reply::Err(err) => Err(anyhow::anyhow!("Failed to focus column right: {}", err)),
            }
        })
        .await
        .context("Task join error")?;

        if let Err(e) = focus_right_result {
            warn!("Failed to focus column right: {}", e);
            return Err(e);
        }

        info!("Column aligned to rightmost position successfully");
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for AutofillPlugin {
    fn name(&self) -> &str {
        "autofill"
    }

    async fn init(&mut self, niri: NiriIpc, _config: &crate::config::Config) -> Result<()> {
        self.niri = niri.clone();

        info!("Autofill plugin initialized");

        // Start event listener task
        let niri_clone = niri.clone();

        let handle = tokio::spawn(async move {
            if let Err(e) = Self::event_listener_loop(niri_clone).await {
                log::error!("Autofill plugin event listener error: {}", e);
            }
        });

        self.event_listener_handle = Some(handle);

        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        // Event-driven plugin, no polling needed
        // The event listener is started in init() and runs in a separate task
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Shutdown is handled by runtime - when runtime shuts down, all tasks are cancelled
        // No need for plugin-specific shutdown logic
        info!("Autofill plugin shutdown (handled by runtime)");
        Ok(())
    }

    async fn update_config(
        &mut self,
        niri: NiriIpc,
        _config: &crate::config::Config,
    ) -> Result<()> {
        info!("Updating autofill plugin configuration");
        self.niri = niri.clone();
        Ok(())
    }
}
