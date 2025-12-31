use anyhow::{Context, Result};
use niri_ipc::{
    socket::Socket, Action, PositionChange, Reply, Request, Response, SizeChange,
    WorkspaceReferenceArg,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Wrapper for niri IPC communication
pub struct NiriIpc {
    socket_path: Option<PathBuf>,
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
    pub fn new(socket_path: Option<String>) -> Result<Self> {
        let path = if let Some(path) = socket_path {
            Some(PathBuf::from(path))
        } else {
            None
        };

        Ok(Self { socket_path: path })
    }

    /// Connect to niri socket
    fn connect(&self) -> Result<Socket> {
        let socket = if let Some(ref path) = self.socket_path {
            Socket::connect_to(path).context("Failed to connect to niri socket")?
        } else {
            Socket::connect().context("Failed to connect to niri socket")?
        };
        Ok(socket)
    }

    /// Get all windows
    pub fn get_windows(&self) -> Result<Vec<Window>> {
        let mut socket = self.connect()?;
        let request = Request::Windows;
        match socket.send(request)? {
            Reply::Ok(Response::Windows(niri_windows)) => {
                // Get workspaces to map workspace_id to workspace name/index
                let workspaces = self.get_workspaces_for_mapping()?;

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
                            }),
                        }
                    })
                    .collect();
                Ok(windows)
            }
            Reply::Ok(_) => {
                anyhow::bail!("Unexpected response type for Windows request");
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to get windows: {}", err);
            }
        }
    }

    /// Helper function to get workspaces for mapping
    pub fn get_workspaces_for_mapping(&self) -> Result<Vec<niri_ipc::Workspace>> {
        let mut socket = self.connect()?;
        let request = Request::Workspaces;
        match socket.send(request)? {
            Reply::Ok(Response::Workspaces(workspaces)) => Ok(workspaces),
            Reply::Ok(_) => {
                anyhow::bail!("Unexpected response type for Workspaces request");
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to get workspaces: {}", err);
            }
        }
    }

    /// Get all workspaces (public method for plugins)
    pub fn get_workspaces(&self) -> Result<Vec<niri_ipc::Workspace>> {
        self.get_workspaces_for_mapping()
    }

    /// Get focused output
    pub fn get_focused_output(&self) -> Result<Output> {
        let mut socket = self.connect()?;
        let request = Request::FocusedOutput;
        match socket.send(request)? {
            Reply::Ok(Response::FocusedOutput(Some(niri_output))) => {
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
            Reply::Ok(Response::FocusedOutput(None)) => {
                anyhow::bail!("No focused output found");
            }
            Reply::Ok(_) => {
                anyhow::bail!("Unexpected response type for FocusedOutput request");
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to get focused output: {}", err);
            }
        }
    }

    /// Get focused workspace
    pub fn get_focused_workspace(&self) -> Result<Workspace> {
        let mut socket = self.connect()?;
        let request = Request::Workspaces;
        match socket.send(request)? {
            Reply::Ok(Response::Workspaces(niri_workspaces)) => {
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
                let windows = self.get_windows()?;
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
            Reply::Ok(_) => {
                anyhow::bail!("Unexpected response type for Workspaces request");
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to get workspaces: {}", err);
            }
        }
    }

    /// Get currently focused window ID
    pub fn get_focused_window_id(&self) -> Result<Option<u64>> {
        let mut socket = self.connect()?;
        let request = Request::FocusedWindow;
        match socket.send(request)? {
            Reply::Ok(Response::FocusedWindow(Some(window))) => {
                log::debug!("Focused window ID: {}", window.id);
                Ok(Some(window.id))
            }
            Reply::Ok(Response::FocusedWindow(None)) => {
                log::debug!("No focused window found");
                Ok(None)
            }
            Reply::Ok(_) => {
                anyhow::bail!("Unexpected response type for FocusedWindow request");
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to get focused window: {}", err);
            }
        }
    }

    /// Focus a window by ID
    pub fn focus_window(&self, window_id: u64) -> Result<()> {
        log::info!("Focusing window {}", window_id);
        let mut socket = self.connect()?;
        let action = Action::FocusWindow { id: window_id };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => {
                log::debug!("Successfully focused window {}", window_id);
                Ok(())
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to focus window: {}", err);
            }
        }
    }

    /// Switch to a workspace by name/index
    pub fn switch_to_workspace(&self, workspace: &str) -> Result<()> {
        log::info!("Switching to workspace {}", workspace);
        let mut socket = self.connect()?;

        // Parse workspace reference - try as index first, then as name
        let workspace_ref = if let Ok(idx) = workspace.parse::<u8>() {
            WorkspaceReferenceArg::Index(idx)
        } else if let Ok(id) = workspace.parse::<u64>() {
            WorkspaceReferenceArg::Id(id)
        } else {
            WorkspaceReferenceArg::Name(workspace.to_string())
        };

        let action = Action::FocusWorkspace {
            reference: workspace_ref,
        };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => {
                log::debug!("Successfully switched to workspace {}", workspace);
                Ok(())
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to switch workspace: {}", err);
            }
        }
    }

    /// Get workspace idx from workspace id
    /// Returns the idx (index) of a workspace given its id
    pub fn get_workspace_idx_from_id(&self, workspace_id: u64) -> Result<Option<u64>> {
        let workspaces = self.get_workspaces_for_mapping()?;
        for workspace in workspaces {
            if workspace.id == workspace_id {
                log::debug!(
                    "Found workspace idx {} for workspace id {}",
                    workspace.idx,
                    workspace_id
                );
                return Ok(Some(workspace.idx as u64));
            }
        }
        log::debug!("No workspace found with id {}", workspace_id);
        Ok(None)
    }

    /// Move window to focused monitor
    /// This moves the window to the current focused output/monitor
    pub fn move_window_to_monitor(&self, window_id: u64) -> Result<()> {
        // Get the focused output name
        let focused_output = self.get_focused_output()?;

        // Move window to the focused monitor using niri_ipc
        let mut socket = self.connect()?;
        let action = Action::MoveWindowToMonitor {
            id: Some(window_id),
            output: focused_output.name,
        };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => Ok(()),
            Reply::Err(err) => {
                anyhow::bail!("Failed to move window to monitor: {}", err);
            }
        }
    }

    /// Move floating window to focused output and workspace
    /// This moves the window to the current focused workspace and monitor
    pub fn move_floating_window(&self, window_id: u64) -> Result<()> {
        // First, move window to the focused monitor
        self.move_window_to_monitor(window_id)?;

        // Small delay to ensure monitor change completes
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Get the focused workspace name or index
        let focused_workspace = self.get_focused_workspace()?;

        // Parse workspace reference
        let workspace_ref = if let Ok(idx) = focused_workspace.name.parse::<u8>() {
            WorkspaceReferenceArg::Index(idx)
        } else if let Ok(id) = focused_workspace.name.parse::<u64>() {
            WorkspaceReferenceArg::Id(id)
        } else {
            WorkspaceReferenceArg::Name(focused_workspace.name.clone())
        };

        // Move window to the focused workspace using niri_ipc
        let mut socket = self.connect()?;
        let action = Action::MoveWindowToWorkspace {
            window_id: Some(window_id),
            reference: workspace_ref,
            focus: false, // Don't change focus, just move the window
        };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => Ok(()),
            Reply::Err(err) => {
                anyhow::bail!("Failed to move window to workspace: {}", err);
            }
        }
    }

    /// Move window to a specific workspace by identifier (name or idx)
    pub fn move_window_to_workspace(&self, window_id: u64, workspace: &str) -> Result<()> {
        log::info!("Moving window {} to workspace {}", window_id, workspace);
        let mut socket = self.connect()?;

        // Parse workspace reference - try as index first, then as name
        let workspace_ref = if let Ok(idx) = workspace.parse::<u8>() {
            WorkspaceReferenceArg::Index(idx)
        } else if let Ok(id) = workspace.parse::<u64>() {
            WorkspaceReferenceArg::Id(id)
        } else {
            WorkspaceReferenceArg::Name(workspace.to_string())
        };

        let action = Action::MoveWindowToWorkspace {
            window_id: Some(window_id),
            reference: workspace_ref,
            focus: false, // Don't change focus, just move the window
        };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => {
                log::debug!(
                    "Successfully moved window {} to workspace {}",
                    window_id,
                    workspace
                );
                Ok(())
            }
            Reply::Err(err) => {
                anyhow::bail!("Failed to move window to workspace: {}", err);
            }
        }
    }

    /// Set window to floating
    pub fn set_window_floating(&self, window_id: u64, floating: bool) -> Result<()> {
        let mut socket = self.connect()?;
        let action = if floating {
            Action::MoveWindowToFloating {
                id: Some(window_id),
            }
        } else {
            Action::MoveWindowToTiling {
                id: Some(window_id),
            }
        };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => Ok(()),
            Reply::Err(err) => {
                anyhow::bail!("Failed to set window floating state: {}", err);
            }
        }
    }

    /// Move window using relative movement
    /// x and y are relative offsets (positive or negative)
    pub fn move_window_relative(&self, window_id: u64, x: i32, y: i32) -> Result<()> {
        let mut socket = self.connect()?;
        let action = Action::MoveFloatingWindow {
            id: Some(window_id),
            x: PositionChange::AdjustFixed(x as f64),
            y: PositionChange::AdjustFixed(y as f64),
        };
        let request = Request::Action(action);
        match socket.send(request)? {
            Reply::Ok(_) => Ok(()),
            Reply::Err(err) => {
                anyhow::bail!("Failed to move window: {}", err);
            }
        }
    }

    /// Resize floating window using set-window-width and set-window-height
    pub fn resize_floating_window(&self, window_id: u64, width: u32, height: u32) -> Result<()> {
        let mut socket = self.connect()?;

        // Set window width
        let width_action = Action::SetWindowWidth {
            id: Some(window_id),
            change: SizeChange::SetFixed(width as i32),
        };
        let request = Request::Action(width_action);
        match socket.send(request)? {
            Reply::Ok(_) => {}
            Reply::Err(err) => {
                anyhow::bail!("Failed to set window width: {}", err);
            }
        }

        // Set window height
        let height_action = Action::SetWindowHeight {
            id: Some(window_id),
            change: SizeChange::SetFixed(height as i32),
        };
        let request = Request::Action(height_action);
        match socket.send(request)? {
            Reply::Ok(_) => Ok(()),
            Reply::Err(err) => {
                anyhow::bail!("Failed to set window height: {}", err);
            }
        }
    }

    /// Get output dimensions (width and height) for focused output
    pub fn get_output_dimensions(&self) -> Result<(u32, u32)> {
        match self.get_focused_output() {
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

    /// Find window by class, title, or app_id
    /// Uses exact match for app_id, partial match for class and title
    pub fn find_window(&self, pattern: &str) -> Result<Option<Window>> {
        let windows = self.get_windows()?;

        for window in windows {
            // Exact match for app_id (most reliable)
            if let Some(ref app_id) = window.app_id {
                if app_id == pattern {
                    return Ok(Some(window));
                }
            }
            // Partial match for class
            if let Some(ref class) = window.class {
                if class.contains(pattern) {
                    return Ok(Some(window));
                }
            }
            // Partial match for title
            if window.title.contains(pattern) {
                return Ok(Some(window));
            }
        }

        Ok(None)
    }

    /// Find window by class or title (async version)
    pub async fn find_window_async(&self, pattern: &str) -> Result<Option<Window>> {
        // Run in blocking thread pool since Command is blocking
        let pattern = pattern.to_string();
        let niri = self.clone();

        tokio::task::spawn_blocking(move || niri.find_window(&pattern))
            .await
            .context("Task join error")?
    }

    /// Get window position and size
    /// Returns (x, y, width, height) if available
    /// For floating windows, extracts position from layout.tile_pos_in_workspace_view
    /// and size from layout.window_size
    pub fn get_window_position(&self, window_id: u64) -> Result<Option<(i32, i32, u32, u32)>> {
        let windows = self.get_windows()?;

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
        let niri = self.clone();

        tokio::task::spawn_blocking(move || niri.get_window_position(window_id))
            .await
            .context("Task join error")?
    }

    /// Create an event stream socket for listening to niri events
    /// This returns a socket that has already requested the event stream
    pub fn create_event_stream_socket(&self) -> Result<Socket> {
        let mut socket = self.connect()?;

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
        }
    }
}
