use anyhow::Result;
use log::{error, info, warn};
use notify::{RecursiveMode, Watcher};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::Mutex;

use crate::commands::CommandHandler;
use crate::ipc::{handle_request, IpcServer};
use crate::niri::NiriIpc;
use crate::plugins::PluginManager;
use crate::utils::send_notification;
use niri_ipc::Event;
use tokio::sync::mpsc;

/// Start a config file watcher that triggers reload on change
async fn start_config_watcher(
    handler: Arc<Mutex<CommandHandler>>,
    plugin_manager: Arc<Mutex<PluginManager>>,
    niri: NiriIpc,
) -> Result<()> {
    let (tx, mut rx) = mpsc::channel(1);
    let config_path = {
        let h = handler.lock().await;
        h.config_path().clone()
    };

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                let _ = tx.blocking_send(());
            }
        }
    })?;

    watcher.watch(&config_path, RecursiveMode::NonRecursive)?;

    // Spawn a task to handle reload signals with debounce
    tokio::spawn(async move {
        // Keep watcher alive
        let _watcher = watcher;

        loop {
            // Wait for first event
            if rx.recv().await.is_none() {
                break;
            }

            // Debounce: wait for 300ms, if new events arrive, reset the timer
            let debounce_timer = tokio::time::sleep(tokio::time::Duration::from_millis(300));
            tokio::pin!(debounce_timer);
            loop {
                tokio::select! {
                    _ = debounce_timer.as_mut() => {
                        // No new events during debounce period, proceed with reload
                        break;
                    }
                    result = rx.recv() => {
                        if result.is_none() {
                            // Channel closed
                            return;
                        }
                        // New event arrived, reset debounce timer
                        debounce_timer.set(tokio::time::sleep(tokio::time::Duration::from_millis(300)));
                    }
                }
            }

            info!("Config file modified, reloading...");

            let mut h = handler.lock().await;
            let path = h.config_path().clone();
            if let Err(e) = h.reload_config(&path).await {
                error!("Failed to auto-reload config: {}", e);
                send_notification("piri", &format!("Auto-reload failed: {}", e));
            } else {
                let config = h.config().clone();
                // Update existing NiriIpc instance in case socket_path changed
                niri.update_socket_path(config.niri.socket_path.clone());

                let mut pm = plugin_manager.lock().await;
                if let Err(e) = pm.init(niri.clone(), &config).await {
                    error!("Failed to reinitialize plugins after auto-reload: {}", e);
                    send_notification("piri", &format!("Plugin reinit failed: {}", e));
                } else {
                    info!("Config auto-reloaded successfully");
                    send_notification("piri", "Configuration hot-reloaded successfully");
                }
            }
        }
    });

    Ok(())
}

/// Run daemon main loop (internal function)
async fn run_daemon_loop(
    ipc_server: IpcServer,
    handler: Arc<Mutex<CommandHandler>>,
    plugin_manager: Arc<Mutex<PluginManager>>,
    mut event_rx: mpsc::UnboundedReceiver<Event>,
    niri: NiriIpc,
) -> Result<()> {
    // Shared shutdown flag
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_clone = shutdown.clone();

    // Setup signal handlers
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())?;

    // Main daemon loop with unified event distribution
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
            event_result = event_rx.recv() => {
                match event_result {
                    Some(event) => {
                        let pm = plugin_manager.clone();
                        let niri_clone = niri.clone();
                        tokio::spawn(async move {
                            let mut pm = pm.lock().await;
                            pm.distribute_event(&event, &niri_clone).await;
                        });
                    }
                    None => {
                        // Channel closed, event listener stopped
                        warn!("Event channel closed, stopping daemon");
                        break;
                    }
                }
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
async fn run_daemon(mut handler: CommandHandler) -> Result<()> {
    info!("Creating IPC server...");

    // Create IPC server
    // If this fails, error will be visible on stderr (which is still open in daemon mode)
    let ipc_server = match IpcServer::new(None).await {
        Ok(server) => {
            info!("IPC server created successfully");
            server
        }
        Err(e) => {
            let error_msg = format!("Failed to create IPC server: {}. Check permissions for socket directory and ensure no other daemon is running.", e);
            return Err(anyhow::anyhow!(error_msg));
        }
    };

    info!("Initializing plugins...");

    // Initialize plugin manager
    let config = handler.config().clone();
    let niri = handler.niri().clone();
    let mut plugin_manager = PluginManager::new();
    if let Err(e) = plugin_manager.init(niri.clone(), &config).await {
        warn!("Failed to initialize plugins: {}", e);
    }

    // Start unified event listener
    let event_rx = match plugin_manager.start_event_listener(niri.clone()).await {
        Ok(rx) => rx,
        Err(e) => {
            warn!("Failed to start event listener: {}", e);
            return Err(anyhow::anyhow!("Failed to start event listener: {}", e));
        }
    };

    // Share plugin manager with handler
    let plugin_manager = Arc::new(Mutex::new(plugin_manager));
    handler.set_plugin_manager(plugin_manager.clone());

    // Wrap handler in Arc<Mutex<>> early to share with config watcher
    let handler = Arc::new(Mutex::new(handler));

    // Start config watcher for hot-reload
    if let Err(e) =
        start_config_watcher(handler.clone(), plugin_manager.clone(), niri.clone()).await
    {
        warn!("Failed to start config watcher: {}", e);
    }

    info!("Setting up signal handlers...");
    info!("Starting daemon main loop...");

    // Set process name again before entering main loop
    // This ensures the name is set even if tokio changed it
    // set_process_name("piri");

    run_daemon_loop(ipc_server, handler, plugin_manager, event_rx, niri).await
}

/// Run daemon
pub async fn run(handler: CommandHandler) -> Result<()> {
    // set_process_name("piri");
    info!("Starting piri daemon");

    run_daemon(handler).await
}
