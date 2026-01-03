use anyhow::Result;
use log::{debug, info, warn};
use niri_ipc::{Action, Event, Reply, Request};

use crate::niri::NiriIpc;
use crate::utils::send_notification;

pub struct AutofillPlugin;

impl AutofillPlugin {
    async fn handle_event_internal(&self, _event: &Event, niri: &NiriIpc) -> Result<()> {
        if let Err(e) = Self::check_and_align_last_column(niri).await {
            warn!("Autofill alignment failed: {}", e);
            send_notification("piri", &format!("Autofill alignment failed: {}", e));
        }
        Ok(())
    }

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
    type Config = ();

    fn new(_niri: NiriIpc, _config: ()) -> Self {
        info!("Autofill plugin initialized");
        Self
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
}
