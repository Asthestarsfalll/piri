use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

/// IPC message types for communication between client and daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcRequest {
    ScratchpadToggle { name: String },
    ScratchpadAdd { name: String, direction: String },
    SingletonToggle { name: String },
    WindowOrderToggle,
    Reload,
    Ping,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    Success,
    Error(String),
    Pong,
}

/// Get the default socket path for piri daemon
pub fn get_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("piri.sock")
    } else {
        PathBuf::from("/tmp/piri.sock")
    }
}

/// IPC server for daemon
pub struct IpcServer {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl IpcServer {
    /// Create a new IPC server
    pub async fn new(socket_path: Option<PathBuf>) -> Result<Self> {
        let socket_path = socket_path.unwrap_or_else(get_socket_path);

        // Remove existing socket if it exists
        if socket_path.exists() {
            std::fs::remove_file(&socket_path).context("Failed to remove existing socket")?;
        }

        // Create parent directory if needed
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create socket directory")?;
        }

        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("Failed to bind to socket: {:?}", socket_path))?;

        log::info!("IPC server listening on {:?}", socket_path);

        Ok(Self {
            listener,
            socket_path,
        })
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Accept a new connection
    pub async fn accept(&self) -> Result<UnixStream> {
        let (stream, _) = self.listener.accept().await.context("Failed to accept connection")?;
        Ok(stream)
    }

    /// Clean up socket file on drop
    pub fn cleanup(&self) {
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// IPC client for subcommands
pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    /// Create a new IPC client
    pub fn new(socket_path: Option<PathBuf>) -> Self {
        let socket_path = socket_path.unwrap_or_else(get_socket_path);
        Self { socket_path }
    }

    /// Send a request to the daemon and get a response
    pub async fn send_request(&self, request: IpcRequest) -> Result<IpcResponse> {
        // Add timeout to prevent hanging
        let connect_future = UnixStream::connect(&self.socket_path);
        let mut stream = tokio::time::timeout(std::time::Duration::from_secs(5), connect_future)
            .await
            .with_context(|| {
                format!(
                    "Connection timeout to daemon socket: {:?}",
                    self.socket_path
                )
            })?
            .with_context(|| {
                format!(
                    "Failed to connect to daemon socket: {:?}. Is the daemon running?",
                    self.socket_path
                )
            })?;

        // Serialize request
        let request_json =
            serde_json::to_string(&request).context("Failed to serialize request")?;

        // Send request length and data
        let request_bytes = request_json.as_bytes();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.write_u32(request_bytes.len() as u32),
        )
        .await
        .context("Timeout writing request length")?
        .context("Failed to write request length")?;

        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.write_all(request_bytes),
        )
        .await
        .context("Timeout writing request data")?
        .context("Failed to write request data")?;

        // Read response length
        let response_len =
            tokio::time::timeout(std::time::Duration::from_secs(5), stream.read_u32())
                .await
                .context("Timeout reading response length")?
                .context("Failed to read response length")?;

        // Read response data
        let mut response_bytes = vec![0u8; response_len as usize];
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.read_exact(&mut response_bytes),
        )
        .await
        .context("Timeout reading response data")?
        .context("Failed to read response data")?;

        // Deserialize response
        let response: IpcResponse =
            serde_json::from_slice(&response_bytes).context("Failed to deserialize response")?;

        Ok(response)
    }
}

/// Helper function to send error response
async fn send_error_response(stream: &mut UnixStream, error: &str) {
    let response = IpcResponse::Error(error.to_string());
    if let Ok(response_json) = serde_json::to_string(&response) {
        let response_bytes = response_json.as_bytes();
        let _ = stream.write_u32(response_bytes.len() as u32).await;
        let _ = stream.write_all(response_bytes).await;
    }
}

/// Handle an IPC request (used by daemon)
pub async fn handle_request(
    mut stream: UnixStream,
    handler: std::sync::Arc<tokio::sync::Mutex<crate::commands::CommandHandler>>,
    shutdown: Option<std::sync::Arc<tokio::sync::Notify>>,
) -> Result<()> {
    // Read request length
    let request_len = match stream.read_u32().await {
        Ok(len) => len,
        Err(e) => {
            log::warn!("Failed to read request length: {}", e);
            return Ok(()); // Connection closed, just return
        }
    };

    // Read request data
    let mut request_bytes = vec![0u8; request_len as usize];
    if let Err(e) = stream.read_exact(&mut request_bytes).await {
        log::error!("Failed to read request data: {}", e);
        send_error_response(&mut stream, &format!("Failed to read request data: {}", e)).await;
        return Ok(());
    }

    // Deserialize request
    let request: IpcRequest = match serde_json::from_slice(&request_bytes) {
        Ok(req) => req,
        Err(e) => {
            log::error!("Failed to deserialize request: {}", e);
            send_error_response(
                &mut stream,
                &format!("Failed to deserialize request: {}", e),
            )
            .await;
            return Ok(());
        }
    };

    // Handle request
    let response = {
        let mut handler = handler.lock().await;

        // Try to handle through plugins first
        if let Some(plugin_result) = handler.handle_ipc_request_through_plugins(&request).await {
            match plugin_result {
                Ok(()) => IpcResponse::Success,
                Err(e) => {
                    log::error!("Error handling request through plugins: {}", e);
                    IpcResponse::Error(e.to_string())
                }
            }
        } else {
            // Fallback to direct handler methods for non-plugin requests
            match request {
                IpcRequest::Reload => {
                    let config_path = handler.config_path().clone();
                    // Reload config and update plugin configs
                    match handler.reload_config(&config_path).await {
                        Ok(()) => {
                            // Update plugin configs after reload
                            let config = handler.config().clone();
                            let niri = handler.niri().clone();
                            let mut pm = handler.plugin_manager().lock().await;
                            // Reinitialize plugins with new config
                            if let Err(e) = pm.init(niri, &config).await {
                                log::warn!(
                                    "Failed to reinitialize plugins after config reload: {}",
                                    e
                                );
                            }
                            IpcResponse::Success
                        }
                        Err(e) => {
                            log::error!("Error reloading config: {}", e);
                            IpcResponse::Error(e.to_string())
                        }
                    }
                }
                IpcRequest::Ping => IpcResponse::Pong,
                IpcRequest::Shutdown => {
                    // Notify the daemon loop to shutdown
                    if let Some(ref shutdown) = shutdown {
                        shutdown.notify_one();
                    }
                    IpcResponse::Success
                }
                IpcRequest::ScratchpadToggle { .. } | IpcRequest::ScratchpadAdd { .. } => {
                    // Check if scratchpads plugin should be enabled but isn't
                    let config = handler.config();
                    if config.is_scratchpads_enabled() {
                        IpcResponse::Error("Scratchpads plugin is enabled but not initialized. Please restart the daemon.".to_string())
                    } else {
                        IpcResponse::Error("Scratchpads plugin is not enabled. Please enable it in the configuration file (piri.plugins.scratchpads = true).".to_string())
                    }
                }
                IpcRequest::SingletonToggle { name: _ } => {
                    // Check if singleton plugin should be enabled but isn't
                    let config = handler.config();
                    if config.is_singleton_enabled() {
                        IpcResponse::Error(format!("Singleton plugin is enabled but not initialized. Please restart the daemon."))
                    } else {
                        IpcResponse::Error(format!("Singleton plugin is not enabled. Please enable it in the configuration file (piri.plugins.singleton = true)."))
                    }
                }
                IpcRequest::WindowOrderToggle => {
                    // Check if window_order plugin should be enabled but isn't
                    let config = handler.config();
                    if config.is_window_order_enabled() {
                        IpcResponse::Error("WindowOrder plugin is enabled but not initialized. Please restart the daemon.".to_string())
                    } else {
                        IpcResponse::Error("WindowOrder plugin is not enabled. Please enable it in the configuration file (piri.plugins.window_order = true).".to_string())
                    }
                }
            }
        }
    };

    // Serialize response
    let response_json = match serde_json::to_string(&response) {
        Ok(json) => json,
        Err(e) => {
            log::error!("Failed to serialize response: {}", e);
            send_error_response(&mut stream, &format!("Failed to serialize response: {}", e)).await;
            return Ok(());
        }
    };
    let response_bytes = response_json.as_bytes();

    // Send response length and data
    if let Err(e) = stream.write_u32(response_bytes.len() as u32).await {
        log::error!("Failed to write response length: {}", e);
        return Ok(());
    }
    if let Err(e) = stream.write_all(response_bytes).await {
        log::error!("Failed to write response data: {}", e);
        return Ok(());
    }

    Ok(())
}
