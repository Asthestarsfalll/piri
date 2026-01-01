use anyhow::{Context, Result};
use niri_ipc::{
    socket::Socket, Action, PositionChange, Reply, Request, Response, SizeChange,
    WorkspaceReferenceArg,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Wrapper for niri IPC communication
pub struct NiriIpc {
    socket_path: Option<PathBuf>,
    socket: Arc<Mutex<Option<Socket>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
    pub id: u64,
    pub title: String,
    #[serde(rename = "app_id")]
    pub app_id: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(rename = "is_floating")]
    pub floating: bool,
    #[serde(rename = "workspace_id")]
    pub workspace_id: Option<u64>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub layout: Option<WindowLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowLayout {
    #[serde(rename = "tile_pos_in_workspace_view")]
    pub tile_pos: Option<[f64; 2]>,
    #[serde(rename = "window_size")]
    pub window_size: Option<[u32; 2]>,
    /// Position in scrolling layout: (column index, tile index in column), 1-based
    #[serde(rename = "pos_in_scrolling_layout")]
    pub pos_in_scrolling_layout: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    #[serde(default)]
    pub focused: bool,
    #[serde(rename = "logical")]
    pub logical: Option<OutputLogical>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputLogical {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub focused: bool,
}

impl NiriIpc {
    pub fn new(socket_path: Option<String>) -> Self {
        let path = socket_path.map(PathBuf::from);

        Self {
            socket_path: path,
            socket: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to niri socket
    fn connect_internal(&self) -> Result<Socket> {
        let socket = if let Some(ref path) = self.socket_path {
            Socket::connect_to(path).context("Failed to connect to niri socket")?
        } else {
            Socket::connect().context("Failed to connect to niri socket")?
        };
        Ok(socket)
    }

    /// Helper to send a request and get a response
    pub async fn send_request(&self, request: Request) -> Result<Response> {
        let niri = self.clone();
        tokio::task::spawn_blocking(move || -> Result<Response> {
            let mut guard = niri.socket.lock().map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
            if guard.is_none() {
                *guard = Some(niri.connect_internal()?);
            }
            let socket = guard.as_mut().unwrap();

            let request_clone = request.clone();

            match socket.send(request) {
                Ok(Reply::Ok(response)) => Ok(response),
                Ok(Reply::Err(err)) => anyhow::bail!("niri-ipc error: {}", err),
                Err(_) => {
                    // Try to reconnect once if send fails
                    *guard = Some(niri.connect_internal()?);
                    let socket = guard.as_mut().unwrap();
                    match socket.send(request_clone)? {
                        Reply::Ok(response) => Ok(response),
                        Reply::Err(err) => anyhow::bail!("niri-ipc error: {}", err),
                    }
                }
            }
        })
        .await
        .context("Task join error")?
    }

    /// Helper to send an action and expect Ok
    pub async fn send_action(&self, action: Action) -> Result<()> {
        self.send_request(Request::Action(action)).await?;
        Ok(())
    }

    /// Execute multiple IPC operations in a single blocking task to minimize latency
    /// and ensure they are processed sequentially without gaps.
    pub async fn execute_batch<F, T>(&self, f: F) -> Result<T>
    where
        F: Fn(&mut Socket) -> Result<T> + Send + Sync + 'static,
        T: Send + 'static,
    {
        let niri = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = niri.socket.lock().map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;

            // Ensure we have a connection
            if guard.is_none() {
                *guard = Some(niri.connect_internal()?);
            }

            let res = {
                let socket = guard.as_mut().unwrap();
                f(socket)
            };

            if res.is_ok() {
                res
            } else {
                // On error, try to reconnect once and retry the whole batch
                *guard = Some(niri.connect_internal()?);
                let socket = guard.as_mut().unwrap();
                f(socket)
            }
        })
        .await
        .context("Task join error")?
    }

    /// Get all windows
    pub async fn get_windows(&self) -> Result<Vec<Window>> {
        match self.send_request(Request::Windows).await? {
            Response::Windows(niri_windows) => {
                // Get workspaces to map workspace_id to workspace name/index
                let workspaces = self.get_workspaces_for_mapping().await?;

                // Convert niri_ipc::Window to our Window type
                let windows: Vec<Window> = niri_windows
                    .into_iter()
                    .map(|w| {
                        // Find workspace name from workspace_id
                        let workspace = w.workspace_id.and_then(|id| {
                            workspaces.iter().find(|ws| ws.id == id).map(|ws| ws.idx.to_string())
                        });

                        Window {
                            id: w.id,
                            title: w.title.unwrap_or_default(),
                            app_id: w.app_id,
                            class: None, // niri_ipc::Window doesn't have class field
                            floating: w.is_floating,
                            workspace_id: w.workspace_id,
                            workspace,
                            output: None, // niri_ipc::Window doesn't have output field directly
                            layout: Some(WindowLayout {
                                tile_pos: w.layout.tile_pos_in_workspace_view.map(|(x, y)| [x, y]),
                                window_size: Some([
                                    w.layout.window_size.0 as u32,
                                    w.layout.window_size.1 as u32,
                                ]),
                                pos_in_scrolling_layout: w.layout.pos_in_scrolling_layout,
                            }),
                        }
                    })
                    .collect();
                Ok(windows)
            }
            _ => anyhow::bail!("Unexpected response type for Windows request"),
        }
    }

    /// Helper function to get workspaces for mapping
    pub async fn get_workspaces_for_mapping(&self) -> Result<Vec<niri_ipc::Workspace>> {
        match self.send_request(Request::Workspaces).await? {
            Response::Workspaces(workspaces) => Ok(workspaces),
            _ => anyhow::bail!("Unexpected response type for Workspaces request"),
        }
    }

    /// Get all workspaces (public method for plugins)
    pub async fn get_workspaces(&self) -> Result<Vec<niri_ipc::Workspace>> {
        self.get_workspaces_for_mapping().await
    }

    /// Get focused output
    pub async fn get_focused_output(&self) -> Result<Output> {
        match self.send_request(Request::FocusedOutput).await? {
            Response::FocusedOutput(Some(niri_output)) => {
                // Convert niri_ipc::Output to our Output type
                // niri_ipc::Output doesn't have is_focused field, but we can assume it's focused if we got it
                Ok(Output {
                    name: niri_output.name,
                    focused: true, // If we got it from FocusedOutput, it's focused
                    logical: niri_output.logical.map(|l| OutputLogical {
                        width: l.width,
                        height: l.height,
                        x: l.x,
                        y: l.y,
                    }),
                })
            }
            Response::FocusedOutput(None) => anyhow::bail!("No focused output found"),
            _ => anyhow::bail!("Unexpected response type for FocusedOutput request"),
        }
    }

    /// Get focused workspace
    pub async fn get_focused_workspace(&self) -> Result<Workspace> {
        match self.send_request(Request::Workspaces).await? {
            Response::Workspaces(niri_workspaces) => {
                // Find the focused workspace
                for workspace in &niri_workspaces {
                    if workspace.is_focused {
                        // Use idx field as workspace identifier
                        return Ok(Workspace {
                            name: workspace.idx.to_string(),
                            focused: true,
                        });
                    }
                }

                // Fallback: try to get from windows if no focused workspace found
                let windows = self.get_windows().await?;
                for window in windows {
                    if let Some(workspace) = &window.workspace {
                        return Ok(Workspace {
                            name: workspace.clone(),
                            focused: true,
                        });
                    }
                    if let Some(workspace_id) = window.workspace_id {
                        return Ok(Workspace {
                            name: workspace_id.to_string(),
                            focused: true,
                        });
                    }
                }

                // Final fallback to default workspace
                Ok(Workspace {
                    name: "1".to_string(),
                    focused: true,
                })
            }
            _ => anyhow::bail!("Unexpected response type for Workspaces request"),
        }
    }

    /// Get currently focused window ID
    pub async fn get_focused_window_id(&self) -> Result<Option<u64>> {
        match self.send_request(Request::FocusedWindow).await? {
            Response::FocusedWindow(Some(window)) => {
                log::debug!("Focused window ID: {}", window.id);
                Ok(Some(window.id))
            }
            Response::FocusedWindow(None) => {
                log::debug!("No focused window found");
                Ok(None)
            }
            _ => anyhow::bail!("Unexpected response type for FocusedWindow request"),
        }
    }

    /// Focus a window by ID
    pub async fn focus_window(&self, window_id: u64) -> Result<()> {
        log::info!("Focusing window {}", window_id);
        self.send_action(Action::FocusWindow { id: window_id }).await
    }

    /// Move window to focused monitor
    /// This moves the window to the current focused output/monitor
    pub async fn move_window_to_monitor(&self, window_id: u64) -> Result<()> {
        // Get the focused output name
        let focused_output = self.get_focused_output().await?;

        // Move window to the focused monitor using niri_ipc
        self.send_action(Action::MoveWindowToMonitor {
            id: Some(window_id),
            output: focused_output.name,
        })
        .await
    }

    /// Move floating window to focused output and workspace
    /// This moves the window to the current focused workspace and monitor
    pub async fn move_floating_window(&self, window_id: u64) -> Result<()> {
        // First, move window to the focused monitor
        self.move_window_to_monitor(window_id).await?;

        // Small delay to ensure monitor change completes
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Get the focused workspace name or index
        let focused_workspace = self.get_focused_workspace().await?;

        // Parse workspace reference
        let workspace_ref = if let Ok(idx) = focused_workspace.name.parse::<u8>() {
            WorkspaceReferenceArg::Index(idx)
        } else if let Ok(id) = focused_workspace.name.parse::<u64>() {
            WorkspaceReferenceArg::Id(id)
        } else {
            WorkspaceReferenceArg::Name(focused_workspace.name.clone())
        };

        // Move window to the focused workspace using niri_ipc
        self.send_action(Action::MoveWindowToWorkspace {
            window_id: Some(window_id),
            reference: workspace_ref,
            focus: false, // Don't change focus, just move the window
        })
        .await
    }

    /// Move window to a specific workspace by identifier (name or idx)
    pub async fn move_window_to_workspace(&self, window_id: u64, workspace: &str) -> Result<()> {
        log::info!("Moving window {} to workspace {}", window_id, workspace);

        // Parse workspace reference - try as index first, then as name
        let workspace_ref = if let Ok(idx) = workspace.parse::<u8>() {
            WorkspaceReferenceArg::Index(idx)
        } else if let Ok(id) = workspace.parse::<u64>() {
            WorkspaceReferenceArg::Id(id)
        } else {
            WorkspaceReferenceArg::Name(workspace.to_string())
        };

        self.send_action(Action::MoveWindowToWorkspace {
            window_id: Some(window_id),
            reference: workspace_ref,
            focus: false, // Don't change focus, just move the window
        })
        .await
    }

    /// Set window to floating
    pub async fn set_window_floating(&self, window_id: u64, floating: bool) -> Result<()> {
        let action = if floating {
            Action::MoveWindowToFloating {
                id: Some(window_id),
            }
        } else {
            Action::MoveWindowToTiling {
                id: Some(window_id),
            }
        };
        self.send_action(action).await
    }

    /// Move window using relative movement
    /// x and y are relative offsets (positive or negative)
    pub async fn move_window_relative(&self, window_id: u64, x: i32, y: i32) -> Result<()> {
        self.send_action(Action::MoveFloatingWindow {
            id: Some(window_id),
            x: PositionChange::AdjustFixed(x as f64),
            y: PositionChange::AdjustFixed(y as f64),
        })
        .await
    }

    /// Resize floating window using set-window-width and set-window-height
    pub async fn resize_floating_window(
        &self,
        window_id: u64,
        width: u32,
        height: u32,
    ) -> Result<()> {
        // Set window width
        self.send_action(Action::SetWindowWidth {
            id: Some(window_id),
            change: SizeChange::SetFixed(width as i32),
        })
        .await?;

        // Set window height
        self.send_action(Action::SetWindowHeight {
            id: Some(window_id),
            change: SizeChange::SetFixed(height as i32),
        })
        .await
    }

    /// Get output dimensions (width and height) for focused output
    pub async fn get_output_dimensions(&self) -> Result<(u32, u32)> {
        match self.get_focused_output().await {
            Ok(output) => {
                if let Some(logical) = output.logical {
                    Ok((logical.width, logical.height))
                } else {
                    // Fallback to default dimensions
                    Ok((1920, 1080))
                }
            }
            Err(_) => {
                // Fallback to default dimensions if query fails
                Ok((1920, 1080))
            }
        }
    }
    /// Returns (x, y, width, height) if available
    /// For floating windows, extracts position from layout.tile_pos_in_workspace_view
    /// and size from layout.window_size
    pub async fn get_window_position(
        &self,
        window_id: u64,
    ) -> Result<Option<(i32, i32, u32, u32)>> {
        let windows = self.get_windows().await?;

        for window in windows {
            if window.id == window_id {
                // For floating windows, get position from layout
                if window.floating {
                    if let Some(layout) = &window.layout {
                        if let (Some(pos), Some(size)) = (layout.tile_pos, layout.window_size) {
                            return Ok(Some((
                                pos[0] as i32, // x
                                pos[1] as i32, // y
                                size[0],       // width
                                size[1],       // height
                            )));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Get window position and size (async version)
    pub async fn get_window_position_async(
        &self,
        window_id: u64,
    ) -> Result<Option<(i32, i32, u32, u32)>> {
        self.get_window_position(window_id).await
    }

    /// Create an event stream socket for listening to niri events
    /// This returns a socket that has already requested the event stream
    pub fn create_event_stream_socket(&self) -> Result<Socket> {
        let mut socket = self.connect_internal()?;

        // Request event stream
        match socket.send(Request::EventStream)? {
            Reply::Ok(_) => {}
            Reply::Err(err) => {
                anyhow::bail!("Failed to request event stream: {}", err);
            }
        }

        Ok(socket)
    }
}

// Make NiriIpc cloneable for async use
impl Clone for NiriIpc {
    fn clone(&self) -> Self {
        Self {
            socket_path: self.socket_path.clone(),
            socket: self.socket.clone(),
        }
    }
}
