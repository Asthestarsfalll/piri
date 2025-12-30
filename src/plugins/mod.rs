pub mod empty;
pub mod scratchpads;

use anyhow::Result;
use async_trait::async_trait;
use log::{info, warn};

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
}

/// Plugin manager that manages all plugins
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Initialize all plugins
    pub async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
        // Check if plugins already exist (for config reload)
        let has_plugins = !self.plugins.is_empty();

        // Initialize or update scratchpads plugin
        if config.is_scratchpads_enabled() {
            if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name() == "scratchpads") {
                // Plugin exists, update config
                info!("Updating scratchpads plugin configuration");
                plugin.update_config(niri.clone(), config).await?;
            } else {
                // Plugin doesn't exist, create new one
                let mut scratchpads_plugin = scratchpads::ScratchpadsPlugin::new();
                scratchpads_plugin.init(niri.clone(), config).await?;
                self.plugins.push(Box::new(scratchpads_plugin));
                log::info!("Scratchpads plugin enabled");
            }
        } else {
            // Remove plugin if it exists and is disabled
            if has_plugins {
                self.plugins.retain(|p| p.name() != "scratchpads");
            }
            log::debug!("Scratchpads plugin disabled by configuration");
        }

        // Initialize or update empty plugin
        if config.is_empty_enabled() {
            if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name() == "empty") {
                // Plugin exists, update config
                info!("Updating empty plugin configuration");
                plugin.update_config(niri.clone(), config).await?;
            } else {
                // Plugin doesn't exist, create new one
                let mut empty_plugin = empty::EmptyPlugin::new();
                empty_plugin.init(niri.clone(), config).await?;
                self.plugins.push(Box::new(empty_plugin));
                log::info!("Empty plugin enabled");
            }
        } else {
            // Remove plugin if it exists and is disabled
            if has_plugins {
                self.plugins.retain(|p| p.name() != "empty");
            }
            log::debug!("Empty plugin disabled by configuration");
        }

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
