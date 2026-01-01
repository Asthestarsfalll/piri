use anyhow::{Context, Result};
use log::{debug, info, warn};
use niri_ipc::{Action, Event, Reply, Request};

use crate::niri::NiriIpc;
pub struct AutofillPlugin {
    niri: NiriIpc,
}

impl AutofillPlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None).expect("Failed to initialize niri IPC"),
        }
    }

    /// Handle a single event - all events trigger the same alignment check
    async fn handle_event_internal(&self, event: &Event, niri: &NiriIpc) -> Result<()> {
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

    /// Align columns in current workspace by focusing first column then last column
    async fn check_and_align_last_column(niri: &NiriIpc) -> Result<()> {
        info!("Aligning columns in current workspace");

        // Save the currently focused window before alignment
        let niri_clone = niri.clone();
        let focused_window_id =
            tokio::task::spawn_blocking(move || niri_clone.get_focused_window_id())
                .await
                .context("Task join error")?
                .ok()
                .flatten();

        if let Some(window_id) = focused_window_id {
            debug!("Saving focused window ID: {}", window_id);
        } else {
            debug!("No focused window to save");
        }

        // First, focus column first
        let focus_first_result = tokio::task::spawn_blocking(move || {
            let mut socket =
                niri_ipc::socket::Socket::connect().context("Failed to connect to niri socket")?;
            let action = Action::FocusColumnFirst {};
            let request = Request::Action(action);
            match socket.send(request)? {
                Reply::Ok(_) => Ok(()),
                Reply::Err(err) => Err(anyhow::anyhow!("Failed to focus column first: {}", err)),
            }
        })
        .await
        .context("Task join error")?;

        if let Err(e) = focus_first_result {
            warn!("Failed to focus column first: {}", e);
            return Err(e);
        }

        // Then, focus column last (aligns to rightmost position)
        let focus_last_result = tokio::task::spawn_blocking(move || {
            let mut socket =
                niri_ipc::socket::Socket::connect().context("Failed to connect to niri socket")?;
            let action = Action::FocusColumnLast {};
            let request = Request::Action(action);
            match socket.send(request)? {
                Reply::Ok(_) => Ok(()),
                Reply::Err(err) => Err(anyhow::anyhow!("Failed to focus column last: {}", err)),
            }
        })
        .await
        .context("Task join error")?;

        if let Err(e) = focus_last_result {
            warn!("Failed to focus column last: {}", e);
            return Err(e);
        }

        // Restore focus to the previously focused window if it existed
        if let Some(window_id) = focused_window_id {
            debug!("Restoring focus to window ID: {}", window_id);
            let restore_focus_result = tokio::task::spawn_blocking(move || {
                let mut socket = niri_ipc::socket::Socket::connect()
                    .context("Failed to connect to niri socket")?;
                let action = Action::FocusWindow { id: window_id };
                let request = Request::Action(action);
                match socket.send(request)? {
                    Reply::Ok(_) => Ok(()),
                    Reply::Err(err) => Err(anyhow::anyhow!(
                        "Failed to restore focus to window: {}",
                        err
                    )),
                }
            })
            .await
            .context("Task join error")?;

            if let Err(e) = restore_focus_result {
                warn!("Failed to restore focus to window: {}", e);
                // Don't return error, alignment was successful
            } else {
                debug!("Focus restored successfully");
            }
        }

        info!("Columns aligned successfully");
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for AutofillPlugin {
    fn name(&self) -> &str {
        "autofill"
    }

    async fn init(&mut self, niri: NiriIpc, _config: &crate::config::Config) -> Result<()> {
        self.niri = niri;
        info!("Autofill plugin initialized");
        // Event listener is now handled by PluginManager
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        // Event-driven plugin, no polling needed
        // The event listener is started in init() and runs in a separate task
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Autofill plugin shutdown");
        Ok(())
    }

    async fn handle_event(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
        self.handle_event_internal(event, niri).await
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowClosed { .. } | Event::WindowLayoutsChanged { .. }
        )
    }

    async fn update_config(
        &mut self,
        niri: NiriIpc,
        _config: &crate::config::Config,
    ) -> Result<()> {
        info!("Updating autofill plugin configuration");
        self.niri = niri;
        Ok(())
    }
}
