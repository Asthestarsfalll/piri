use anyhow::Result;
use log::info;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::PluginManager;

/// Command handler for processing different commands
pub struct CommandHandler {
    config: Config,
    config_path: PathBuf,
    niri: NiriIpc,
    plugin_manager: Arc<Mutex<PluginManager>>,
}

impl CommandHandler {
    pub fn new(config: Config) -> Self {
        Self::with_config_path(config, PathBuf::from(""))
    }

    pub fn with_config_path(config: Config, config_path: PathBuf) -> Self {
        let niri =
            NiriIpc::new(config.niri.socket_path.clone()).expect("Failed to initialize niri IPC");

        // Create plugin manager (will be initialized in daemon)
        let plugin_manager = Arc::new(Mutex::new(PluginManager::new()));

        Self {
            config,
            config_path,
            niri,
            plugin_manager,
        }
    }

    /// Handle IPC request through plugins
    pub async fn handle_ipc_request_through_plugins(
        &mut self,
        request: &IpcRequest,
    ) -> Option<Result<()>> {
        let mut pm = self.plugin_manager.lock().await;
        match pm.handle_ipc_request(request).await {
            Ok(Some(result)) => Some(result),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }

    /// Set plugin manager (called by daemon after initialization)
    pub fn set_plugin_manager(&mut self, plugin_manager: Arc<Mutex<PluginManager>>) {
        self.plugin_manager = plugin_manager;
    }

    /// Get niri IPC instance (for future extensions)
    pub fn niri(&self) -> &NiriIpc {
        &self.niri
    }

    /// Get plugin manager (for future extensions)
    pub fn plugin_manager(&self) -> &Arc<Mutex<PluginManager>> {
        &self.plugin_manager
    }

    /// Get config (for future extensions)
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get config path
    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    /// Reload configuration from file
    pub async fn reload_config(&mut self, config_path: &PathBuf) -> Result<()> {
        info!("Reloading configuration from {:?}", config_path);

        let new_config = Config::load(config_path)?;
        info!("Configuration reloaded successfully");

        // Update config
        self.config = new_config;

        // Note: Plugins will use the updated config on next request
        // Existing scratchpads will continue to work with old config
        // New scratchpads will use the new config

        Ok(())
    }
}
