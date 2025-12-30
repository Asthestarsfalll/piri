use anyhow::Result;
use log::info;
use std::path::PathBuf;

use crate::config::Config;
use crate::niri::NiriIpc;
use crate::scratchpads::ScratchpadManager;

/// Command handler for processing different commands
pub struct CommandHandler {
    config: Config,
    config_path: PathBuf,
    niri: NiriIpc,
    scratchpad_manager: ScratchpadManager,
}

impl CommandHandler {
    pub fn new(config: Config) -> Self {
        Self::with_config_path(config, PathBuf::from(""))
    }

    pub fn with_config_path(config: Config, config_path: PathBuf) -> Self {
        let niri =
            NiriIpc::new(config.niri.socket_path.clone()).expect("Failed to initialize niri IPC");
        let scratchpad_manager = ScratchpadManager::new(niri.clone());

        Self {
            config,
            config_path,
            niri,
            scratchpad_manager,
        }
    }

    /// Handle scratchpad toggle command
    pub async fn handle_scratchpad_toggle(&mut self, name: &str) -> Result<()> {
        info!("Handling scratchpad toggle for: {}", name);

        // Try to get config from file first, then from dynamic configs
        let scratchpad_config = if let Some(config) = self.config.get_scratchpad(name) {
            // Use config from file
            config
        } else if let Some(config) = self.scratchpad_manager.get_config(name, None) {
            // Use dynamic config (need to store it temporarily since we need a reference)
            // We'll pass it directly to toggle
            self.scratchpad_manager.toggle(name, &config).await?;
            return Ok(());
        } else {
            anyhow::bail!("Scratchpad '{}' not found. Use 'piri scratchpad {} add <direction>' to add it first.", name, name);
        };

        self.scratchpad_manager.toggle(name, scratchpad_config).await?;

        Ok(())
    }

    /// Handle scratchpad add command
    pub async fn handle_scratchpad_add(&mut self, name: &str, direction: &str) -> Result<()> {
        info!(
            "Handling scratchpad add for: {} with direction: {}",
            name, direction
        );

        // Validate direction
        match direction {
            "fromTop" | "fromBottom" | "fromLeft" | "fromRight" => {}
            _ => anyhow::bail!(
                "Invalid direction: {}. Must be one of: fromTop, fromBottom, fromLeft, fromRight",
                direction
            ),
        }

        // Get default size and margin from config
        let default_size = &self.config.piri.scratchpad.default_size;
        let default_margin = self.config.piri.scratchpad.default_margin;

        self.scratchpad_manager
            .add_current_window(name, direction, default_size, default_margin)
            .await?;

        Ok(())
    }

    /// Get niri IPC instance (for future extensions)
    pub fn niri(&self) -> &NiriIpc {
        &self.niri
    }

    /// Get scratchpad manager (for future extensions)
    pub fn scratchpad_manager(&mut self) -> &mut ScratchpadManager {
        &mut self.scratchpad_manager
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

        // Note: Existing scratchpads will continue to work with old config
        // New scratchpads will use the new config

        Ok(())
    }
}
