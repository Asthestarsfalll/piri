use anyhow::Result;
use async_trait::async_trait;
use log::info;

use crate::config::Config;
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::scratchpads::ScratchpadManager;

/// Scratchpads plugin that wraps ScratchpadManager
pub struct ScratchpadsPlugin {
    manager: ScratchpadManager,
    config: Config,
}

impl ScratchpadsPlugin {
    pub fn new() -> Self {
        Self {
            manager: ScratchpadManager::new(
                NiriIpc::new(None).expect("Failed to initialize niri IPC"),
            ),
            config: Config::default(),
        }
    }
}

#[async_trait]
impl crate::plugins::Plugin for ScratchpadsPlugin {
    fn name(&self) -> &str {
        "scratchpads"
    }

    async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
        self.config = config.clone();
        self.manager = ScratchpadManager::new(niri);
        info!(
            "Scratchpads plugin initialized with {} scratchpads",
            config.scratchpads.len()
        );
        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::ScratchpadToggle { name } => {
                info!("Handling scratchpad toggle for: {}", name);

                // Try to get config from file first, then from dynamic configs
                let scratchpad_config = if let Some(config) = self.config.get_scratchpad(name) {
                    // Use config from file
                    config
                } else if let Some(config) = self.manager.get_config(name, None) {
                    // Use dynamic config (need to store it temporarily since we need a reference)
                    // We'll pass it directly to toggle
                    self.manager.toggle(name, &config).await?;
                    return Ok(Some(Ok(())));
                } else {
                    anyhow::bail!("Scratchpad '{}' not found. Use 'piri scratchpad {} add <direction>' to add it first.", name, name);
                };

                self.manager.toggle(name, scratchpad_config).await?;
                Ok(Some(Ok(())))
            }
            IpcRequest::ScratchpadAdd { name, direction } => {
                info!(
                    "Handling scratchpad add for: {} with direction: {}",
                    name, direction
                );

                // Validate direction
                match direction.as_str() {
                    "fromTop" | "fromBottom" | "fromLeft" | "fromRight" => {}
                    _ => anyhow::bail!(
                        "Invalid direction: {}. Must be one of: fromTop, fromBottom, fromLeft, fromRight",
                        direction
                    ),
                }

                // Get default size and margin from config
                let default_size = &self.config.piri.scratchpad.default_size;
                let default_margin = self.config.piri.scratchpad.default_margin;

                self.manager
                    .add_current_window(name, direction, default_size, default_margin)
                    .await?;

                Ok(Some(Ok(())))
            }
            _ => Ok(None), // Not handled by this plugin
        }
    }
}
