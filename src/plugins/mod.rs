pub mod empty;
pub mod scratchpads;
pub mod window_rule;

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
