use anyhow::{Context, Result};
use log::info;
use std::ffi::CString;
use std::os::unix::process::CommandExt;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;

use crate::commands::CommandHandler;
use crate::ipc::{handle_request, IpcServer};

/// Set the process name (for Linux)
/// This sets the thread name, which is what shows up in `ps -o comm=`
#[cfg(target_os = "linux")]
fn set_process_name(name: &str) {
    use std::os::raw::c_char;

    // PR_SET_NAME is 15 on Linux
    const PR_SET_NAME: libc::c_int = 15;

    // Truncate to 15 bytes (16 bytes including null terminator) as per prctl limitation
    let truncated = if name.len() > 15 { &name[..15] } else { name };

    let name_cstr = match CString::new(truncated) {
        Ok(s) => s,
        Err(_) => return, // Invalid name, skip
    };

    unsafe {
        libc::prctl(PR_SET_NAME, name_cstr.as_ptr() as *const c_char, 0, 0, 0);
    }
}

#[cfg(not(target_os = "linux"))]
fn set_process_name(_name: &str) {
    // No-op on non-Linux systems
}

/// Run daemon main loop (internal function)
async fn run_daemon_loop(ipc_server: IpcServer, handler: CommandHandler) -> Result<()> {
    eprintln!("[DAEMON] run_daemon_loop: Starting...");

    // Wrap handler in Arc<Mutex<>> for shared access
    let handler = Arc::new(Mutex::new(handler));

    // Shared shutdown flag
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();

    eprintln!("[DAEMON] run_daemon_loop: Setting up signal handlers...");
    // Setup signal handlers
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())?;

    eprintln!("[DAEMON] run_daemon_loop: Entering main loop, waiting for connections...");
    // Main daemon loop
    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down...");
                break;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, shutting down...");
                break;
            }
            _ = shutdown.notified() => {
                info!("Received shutdown request via IPC, shutting down...");
                break;
            }
            stream_result = ipc_server.accept() => {
                match stream_result {
                    Ok(stream) => {
                        let handler_clone = handler.clone();
                        let shutdown_flag = shutdown_clone.clone();
                        // Spawn request handling to avoid blocking the main loop
                        // This allows concurrent request handling
                        tokio::spawn(async move {
                            if let Err(e) = handle_request(stream, handler_clone, Some(shutdown_flag)).await {
                                log::error!("Error handling IPC request: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Error accepting IPC connection: {}", e);
                    }
                }
            }
        }
    }

    // Cleanup socket
    ipc_server.cleanup();
    info!("Daemon stopped");
    Ok(())
}

/// Run daemon (internal function, can be called with or without daemonizing)
async fn run_daemon(handler: CommandHandler) -> Result<()> {
    eprintln!("[DAEMON] run_daemon: Creating IPC server...");
    info!("Creating IPC server...");

    // Create IPC server
    // If this fails, error will be visible on stderr (which is still open in daemon mode)
    let ipc_server = match IpcServer::new(None).await {
        Ok(server) => {
            eprintln!("[DAEMON] run_daemon: IPC server created successfully");
            info!("IPC server created successfully");
            server
        }
        Err(e) => {
            let error_msg = format!("Failed to create IPC server: {}. Check permissions for socket directory and ensure no other daemon is running.", e);
            eprintln!("[DAEMON] ERROR: {}", error_msg);
            eprintln!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }
    };

    eprintln!("[DAEMON] run_daemon: Setting up signal handlers...");
    info!("Setting up signal handlers...");

    // If we're running as a daemon (marked by PIRI_DAEMON env var),
    // DON'T close stderr - keep it open for debugging
    // We'll keep stderr open so errors are always visible
    if std::env::var("PIRI_DAEMON").is_ok() {
        eprintln!("[DAEMON] run_daemon: IPC server started successfully, keeping stderr open for debugging");
        info!("IPC server started successfully");
        // Don't close stderr - keep it for debugging
        // unsafe {
        //     let _ = libc::close(2); // stderr
        // }
    }

    eprintln!("[DAEMON] run_daemon: Starting daemon main loop...");
    info!("Starting daemon main loop...");

    // Set process name again before entering main loop
    // This ensures the name is set even if tokio changed it
    set_process_name("piri");

    eprintln!("[DAEMON] run_daemon: About to enter run_daemon_loop");
    let result = run_daemon_loop(ipc_server, handler).await;
    eprintln!(
        "[DAEMON] run_daemon: run_daemon_loop returned: {:?}",
        result
    );
    result
}

/// Run daemon
pub async fn run(handler: CommandHandler) -> Result<()> {
    let is_daemon = std::env::var("PIRI_DAEMON").is_ok();

    // Set process name to "piri" so it shows correctly in process list
    set_process_name("piri");

    if is_daemon {
        eprintln!("[DAEMON] Starting piri daemon (daemonized mode)");
        info!("Starting piri daemon (daemonized mode)");
    } else {
        info!("Starting piri daemon");
    }

    eprintln!("[DAEMON] About to call run_daemon");
    match run_daemon(handler).await {
        Ok(()) => {
            eprintln!("[DAEMON] run_daemon returned successfully");
            Ok(())
        }
        Err(e) => {
            // In daemon mode, stderr is still open, so we can print errors
            eprintln!("[DAEMON] Daemon failed to start: {}", e);
            eprintln!("[DAEMON] Error chain: {:?}", e);
            Err(e)
        }
    }
}

/// Daemonize the process (synchronous version)
/// This should be called from a blocking thread, not from within tokio runtime
pub fn daemonize_sync(handler: CommandHandler) -> Result<()> {
    info!("Daemonizing piri...");

    // Get socket path to check if it exists later
    use crate::ipc::get_socket_path;
    let socket_path = get_socket_path();

    // Fork the process
    match unsafe { libc::fork() } {
        -1 => anyhow::bail!("Failed to fork process"),
        0 => {
            // Child process
            // Create a new session
            if unsafe { libc::setsid() } == -1 {
                anyhow::bail!("Failed to create new session");
            }

            // Fork again to ensure we're not a session leader
            match unsafe { libc::fork() } {
                -1 => anyhow::bail!("Failed to fork process again"),
                0 => {
                    // Second child - this is the actual daemon
                    // Change working directory
                    std::env::set_current_dir("/").unwrap();

                    // Close stdin and stdout, but keep stderr open for error reporting
                    // We'll close stderr after IPC server is successfully created
                    unsafe {
                        libc::close(0); // stdin
                        libc::close(1); // stdout
                                        // Don't close stderr yet - we need it for error messages
                    }

                    // Set environment variable to mark this as a daemon process
                    std::env::set_var("PIRI_DAEMON", "1");

                    // Re-execute the program
                    // This is the simplest and most reliable way to daemonize a tokio program
                    // The new process will have a clean tokio runtime
                    let exe = std::env::current_exe()
                        .unwrap_or_else(|_| std::path::PathBuf::from("piri"));

                    let config_path = handler.config_path().to_string_lossy().to_string();
                    let mut cmd = std::process::Command::new(&exe);
                    cmd.arg("daemon").arg("--config").arg(&config_path);

                    // Execute the new process (replaces current process)
                    // Stderr is still open, so errors can be logged
                    let _ = cmd.exec();
                    // This should never be reached, but just in case
                    std::process::exit(1);
                }
                _ => {
                    // First child exits
                    std::process::exit(0);
                }
            }
        }
        _ => {
            // Parent process - wait for socket to be created before exiting
            // This ensures the socket is ready when the parent exits
            let mut attempts = 0;
            while attempts < 50 {
                // Wait up to 5 seconds
                if socket_path.exists() {
                    info!("Socket created, parent exiting");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                attempts += 1;
            }
            std::process::exit(0);
        }
    }

    // This line is unreachable because all branches either exit or return an error
    // but we need to return a value to satisfy the function signature
    #[allow(unreachable_code)]
    Ok(())
}

/// Daemonize the process (async wrapper)
/// Note: This should be called from outside tokio runtime context
pub async fn daemonize(handler: CommandHandler) -> Result<()> {
    // We need to drop out of the async context before forking
    // Use spawn_blocking to move to a blocking thread
    tokio::task::spawn_blocking(move || daemonize_sync(handler))
        .await
        .context("Failed to spawn daemonize task")?
}
