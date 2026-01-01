use anyhow::Result;
use log::{debug, info, warn};
use niri_ipc::{Action, Event, Reply, Request};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::niri::NiriIpc;

pub struct AutofillPlugin {
    niri: NiriIpc,
    // Store handle for debouncing
    debounce_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl AutofillPlugin {
    pub fn new() -> Self {
        Self {
            niri: NiriIpc::new(None),
            debounce_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Handle a single event - all events trigger the same alignment check with debouncing
    async fn handle_event_internal(&self, event: &Event, niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowClosed { .. } | Event::WindowLayoutsChanged { .. } => {
                let niri_clone = niri.clone();
                let debounce_handle = self.debounce_handle.clone();

                let mut guard = debounce_handle.lock().await;
                // Cancel previous pending task
                if let Some(handle) = guard.take() {
                    handle.abort();
                }

                *guard = Some(tokio::spawn(async move {
                    if let Err(e) = Self::check_and_align_last_column(&niri_clone).await {
                        if !e.to_string().contains("canceled") {
                            warn!("Autofill alignment failed: {}", e);
                        }
                    }
                }));
            }
            _ => {}
        }

        Ok(())
    }

    /// Align columns in current workspace by focusing first then last column
    async fn check_and_align_last_column(niri: &NiriIpc) -> Result<()> {
        debug!("Aligning columns in current workspace (batched original logic)");

        niri.execute_batch(|socket| {
            // 1. Get currently focused window ID
            let reply = socket.send(Request::FocusedWindow)?;
            let focused_window_id = match reply {
                Reply::Ok(niri_ipc::Response::FocusedWindow(Some(w))) => Some(w.id),
                _ => None,
            };

            // 2. Focus column first
            let _ = socket.send(Request::Action(Action::FocusColumnFirst {}))?;

            // 3. If focused window exists, restore focus to it; otherwise focus last column
            if let Some(window_id) = focused_window_id {
                let _ = socket.send(Request::Action(Action::FocusWindow { id: window_id }))?;
            } else {
                let _ = socket.send(Request::Action(Action::FocusColumnLast {}))?;
            }

            Ok(())
        })
        .await
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
