use anyhow::{Context, Result};
use log::{debug, info};
use niri_ipc::Event;
use std::collections::VecDeque;

use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;

const HISTORY_CAPACITY: usize = 50;

pub struct RefocusPlugin {
    niri: NiriIpc,
    focus_history: VecDeque<u64>,
}

impl RefocusPlugin {
    fn push_focus(&mut self, window_id: u64) {
        // Deduplicate: remove if already present
        self.focus_history.retain(|&id| id != window_id);
        self.focus_history.push_front(window_id);
        if self.focus_history.len() > HISTORY_CAPACITY {
            self.focus_history.pop_back();
        }
        debug!("Focus history updated: {:?}", self.focus_history);
    }

    fn remove_window(&mut self, window_id: u64) {
        self.focus_history.retain(|&id| id != window_id);
    }

    async fn close_and_refocus(&mut self) -> Result<()> {
        let focused_id = self.niri.get_focused_window_id().await?.context("No focused window")?;

        let windows = self.niri.get_windows().await?;
        let focused_workspace =
            windows.iter().find(|w| w.id == focused_id).and_then(|w| w.workspace_id);

        // Find the best target: first history entry that exists and is on the same workspace
        let target_id = self.focus_history.iter().find(|&&id| {
            id != focused_id
                && windows.iter().any(|w| w.id == id && w.workspace_id == focused_workspace)
        });

        self.niri.close_window(focused_id).await?;

        if let Some(&target) = target_id {
            debug!("Refocusing window {}", target);
            self.niri.focus_window(target).await?;
        } else {
            debug!("No refocus target found, letting niri handle focus");
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::plugins::Plugin for RefocusPlugin {
    type Config = ();

    fn new(niri: NiriIpc, _config: ()) -> Self {
        info!("Refocus plugin initialized");
        Self {
            niri,
            focus_history: VecDeque::with_capacity(HISTORY_CAPACITY),
        }
    }

    async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowFocusChanged { id: Some(id) } => {
                self.push_focus(*id);
            }
            Event::WindowClosed { id } => {
                self.remove_window(*id);
            }
            _ => {}
        }
        Ok(())
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowFocusChanged { .. } | Event::WindowClosed { .. }
        )
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::CloseRefocus => Ok(Some(self.close_and_refocus().await)),
            _ => Ok(None),
        }
    }

    async fn update_config(&mut self, _config: ()) -> Result<()> {
        Ok(())
    }
}
