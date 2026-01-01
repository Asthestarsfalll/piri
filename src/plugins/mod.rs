pub mod autofill;
pub mod empty;
pub mod scratchpads;
pub mod singleton;
pub mod window_order;
pub mod window_rule;
pub mod window_utils;

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, info, warn};
use niri_ipc::Event;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;

/// Plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name
    fn name(&self) -> &str;

    /// Initialize the plugin
    async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()>;

    /// Run the plugin (called periodically in daemon loop)
    async fn run(&mut self) -> Result<()> {
        // Default implementation: do nothing
        Ok(())
    }

    /// Handle IPC request (optional, for plugins that need to handle IPC commands)
    async fn handle_ipc_request(&mut self, _request: &IpcRequest) -> Result<Option<Result<()>>> {
        // Default implementation: not handled
        Ok(None)
    }

    /// Shutdown the plugin (optional, for plugins that need cleanup)
    async fn shutdown(&mut self) -> Result<()> {
        // Default implementation: do nothing
        Ok(())
    }

    /// Update plugin configuration (optional, for plugins that support config updates)
    async fn update_config(&mut self, _niri: NiriIpc, _config: &Config) -> Result<()> {
        // Default implementation: do nothing
        Ok(())
    }

    /// Handle niri event (optional, for plugins that need to listen to events)
    async fn handle_event(&mut self, _event: &Event, _niri: &NiriIpc) -> Result<()> {
        // Default implementation: do nothing
        Ok(())
    }

    /// Check if plugin is interested in a specific event type
    /// This is used for event filtering to avoid calling plugins that don't care about the event
    /// Default implementation returns true (receive all events for backward compatibility)
    fn is_interested_in_event(&self, event: &Event) -> bool {
        let _ = event; // Suppress unused variable warning
        true // Default: interested in all events
    }
}

/// Plugin manager that manages all plugins
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
    /// Event listener task handle
    event_listener_handle: Option<tokio::task::JoinHandle<()>>,
    /// Channel sender for events (receiver is in the event listener loop)
    event_sender: Option<mpsc::UnboundedSender<Event>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            event_listener_handle: None,
            event_sender: None,
        }
    }

    /// Start unified event listener that sends events to channel
    pub async fn start_event_listener(
        &mut self,
        niri: NiriIpc,
    ) -> Result<mpsc::UnboundedReceiver<Event>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        self.event_sender = Some(tx);

        let niri_clone = niri.clone();
        let handle = tokio::spawn(async move {
            Self::event_listener_loop(niri_clone, tx_clone).await;
        });

        self.event_listener_handle = Some(handle);
        info!("Plugin manager unified event listener started");
        Ok(rx)
    }

    /// Unified event listener loop that reads events and sends them to channel
    async fn event_listener_loop(niri: NiriIpc, event_tx: mpsc::UnboundedSender<Event>) {
        info!("Plugin manager event listener started");

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

                // Send event to channel for distribution
                if event_tx.send(event).is_err() {
                    warn!("Event channel closed, stopping event listener");
                    return;
                }
            }

            // Connection closed or error - will reconnect in outer loop
            warn!("Event stream closed, reconnecting...");
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }

    /// Distribute event to all plugins (called from daemon loop)
    /// Only plugins that are interested in the event type will receive it
    pub async fn distribute_event(&mut self, event: &Event, niri: &NiriIpc) {
        for plugin in &mut self.plugins {
            // Check if plugin is interested in this event type
            if plugin.is_interested_in_event(event) {
                if let Err(e) = plugin.handle_event(event, niri).await {
                    log::warn!("Error handling event in plugin {}: {}", plugin.name(), e);
                }
            }
        }
    }

    /// Helper function to initialize or update a plugin
    async fn init_plugin<P, F>(
        &mut self,
        plugin_name: &str,
        enabled: bool,
        create_plugin: F,
        niri: NiriIpc,
        config: &Config,
    ) -> Result<()>
    where
        P: Plugin + 'static,
        F: FnOnce() -> P,
    {
        if enabled {
            if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name() == plugin_name) {
                // Plugin exists, update config
                info!("Updating {} plugin configuration", plugin_name);
                plugin.update_config(niri.clone(), config).await?;
            } else {
                // Plugin doesn't exist, create new one
                let mut new_plugin = create_plugin();
                new_plugin.init(niri.clone(), config).await?;
                self.plugins.push(Box::new(new_plugin));
                log::info!("{} plugin enabled", plugin_name);
            }
        } else {
            // Remove plugin if it exists and is disabled
            let had_plugin = self.plugins.iter().any(|p| p.name() == plugin_name);
            self.plugins.retain(|p| p.name() != plugin_name);
            if had_plugin {
                log::debug!("{} plugin disabled by configuration", plugin_name);
            }
        }
        Ok(())
    }

    /// Initialize all plugins
    pub async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
        // Initialize or update scratchpads plugin
        self.init_plugin(
            "scratchpads",
            config.is_scratchpads_enabled(),
            || scratchpads::ScratchpadsPlugin::new(),
            niri.clone(),
            config,
        )
        .await?;

        // Initialize or update empty plugin
        self.init_plugin(
            "empty",
            config.is_empty_enabled(),
            || empty::EmptyPlugin::new(),
            niri.clone(),
            config,
        )
        .await?;

        // Initialize or update window_rule plugin
        self.init_plugin(
            "window_rule",
            config.is_window_rule_enabled(),
            || window_rule::WindowRulePlugin::new(),
            niri.clone(),
            config,
        )
        .await?;

        // Initialize or update autofill plugin
        self.init_plugin(
            "autofill",
            config.is_autofill_enabled(),
            || autofill::AutofillPlugin::new(),
            niri.clone(),
            config,
        )
        .await?;

        // Initialize or update singleton plugin
        self.init_plugin(
            "singleton",
            config.is_singleton_enabled(),
            || singleton::SingletonPlugin::new(),
            niri.clone(),
            config,
        )
        .await?;

        // Initialize or update window_order plugin
        self.init_plugin(
            "window_order",
            config.is_window_order_enabled(),
            || window_order::WindowOrderPlugin::new(),
            niri.clone(),
            config,
        )
        .await?;

        Ok(())
    }

    /// Handle IPC request through plugins
    pub async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        for plugin in &mut self.plugins {
            match plugin.handle_ipc_request(request).await? {
                Some(result) => return Ok(Some(result)),
                None => continue,
            }
        }
        Ok(None)
    }

    /// Run all plugins
    pub async fn run(&mut self) -> Result<()> {
        for plugin in &mut self.plugins {
            if let Err(e) = plugin.run().await {
                log::error!("Error running plugin {}: {}", plugin.name(), e);
            }
        }
        Ok(())
    }

    /// Shutdown all plugins
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down plugins...");

        // Shutdown all plugins
        for plugin in &mut self.plugins {
            if let Err(e) = plugin.shutdown().await {
                warn!("Error shutting down plugin {}: {}", plugin.name(), e);
            }
        }

        Ok(())
    }
}
