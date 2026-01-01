use anyhow::Result;
use async_trait::async_trait;
use log::{info, warn};
use std::collections::HashMap;

use crate::config::{Config, SingletonConfig};
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use std::sync::Arc;

/// Manages singleton windows (windows that should only have one instance)
struct SingletonManager {
    niri: NiriIpc,
    /// Map of singleton name to window ID (for tracking)
    singletons: HashMap<String, u64>,
    /// Window matcher cache for regex pattern matching
    matcher_cache: Arc<WindowMatcherCache>,
}

impl SingletonManager {
    fn new(niri: NiriIpc) -> Self {
        Self {
            niri,
            singletons: HashMap::new(),
            matcher_cache: Arc::new(WindowMatcherCache::new()),
        }
    }

    /// Extract app_id pattern from command
    /// For example, "google-chrome-stable" from "/usr/bin/google-chrome-stable" or "google-chrome-stable --some-arg"
    fn extract_app_id_from_command(command: &str) -> String {
        // Split by whitespace and take the first part
        let cmd = command.split_whitespace().next().unwrap_or(command);
        // Extract just the executable name (without path)
        cmd.split('/').last().unwrap_or(cmd).to_string()
    }

    /// Get window match pattern from config (app_id if specified, otherwise extract from command)
    fn get_window_match_pattern(config: &SingletonConfig) -> String {
        config
            .app_id
            .clone()
            .unwrap_or_else(|| Self::extract_app_id_from_command(&config.command))
    }

    /// Toggle singleton: if window exists, focus it; otherwise launch command
    async fn toggle(&mut self, name: &str, config: &SingletonConfig) -> Result<()> {
        info!("Toggling singleton: {}", name);

        // Check if we already have this singleton registered
        if let Some(&window_id) = self.singletons.get(name) {
            // Check if the registered window still exists by checking window list
            let windows = self.niri.get_windows().await?;
            if let Some(window) = windows.iter().find(|w| w.id == window_id) {
                // Window exists, focus it
                info!(
                    "Singleton {} window exists (ID: {}), focusing it",
                    name, window_id
                );
                window_utils::focus_window(self.niri.clone(), window.id).await?;
                return Ok(());
            }
            // Window doesn't exist anymore, remove from registry
            warn!(
                "Singleton window {} (ID: {}) not found, removing from registry",
                name, window_id
            );
            self.singletons.remove(name);
        }

        // Get window match pattern (use app_id from config if specified, otherwise extract from command)
        let window_match = Self::get_window_match_pattern(config);
        info!("Using window match pattern: {}", window_match);

        // Try to find existing window using pattern matching
        let matcher = WindowMatcher::new(Some(window_match.clone()), None);
        if let Some(window) =
            window_utils::find_window_by_matcher(self.niri.clone(), &matcher, &self.matcher_cache)
                .await?
        {
            info!(
                "Found existing window for singleton {} (ID: {})",
                name, window.id
            );
            // Focus the window
            window_utils::focus_window(self.niri.clone(), window.id).await?;
            // Register the window ID
            self.singletons.insert(name.to_string(), window.id);
            return Ok(());
        }

        // Launch application
        info!("Launching application for singleton {}", name);
        info!("Looking for window matching pattern: {}", window_match);

        window_utils::launch_application(&config.command).await?;

        // Wait for window to appear
        let window = window_utils::wait_for_window(
            self.niri.clone(),
            &window_match,
            name,
            50, // max_attempts: 5 seconds with 100ms intervals
            &self.matcher_cache,
        )
        .await?;

        if let Some(window) = window {
            info!(
                "Window appeared for singleton {} (ID: {}, app_id: {:?}, title: {})",
                name, window.id, window.app_id, window.title
            );
            // Focus the window
            window_utils::focus_window(self.niri.clone(), window.id).await?;
            // Register the window ID
            self.singletons.insert(name.to_string(), window.id);
        }

        Ok(())
    }
}

/// Singleton plugin that wraps SingletonManager
pub struct SingletonPlugin {
    manager: SingletonManager,
    config: Config,
}

impl SingletonPlugin {
    pub fn new() -> Self {
        Self {
            manager: SingletonManager::new(NiriIpc::new(None)),
            config: Config::default(),
        }
    }
}

#[async_trait]
impl crate::plugins::Plugin for SingletonPlugin {
    fn name(&self) -> &str {
        "singleton"
    }

    async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
        self.config = config.clone();
        self.manager = SingletonManager::new(niri);
        info!(
            "Singleton plugin initialized with {} singletons",
            config.singleton.len()
        );
        Ok(())
    }

    async fn update_config(&mut self, _niri: NiriIpc, config: &Config) -> Result<()> {
        info!("Updating singleton plugin configuration");

        let old_count = self.config.singleton.len();
        self.config = config.clone();
        let new_count = self.config.singleton.len();

        // Update niri instance in manager (if needed)
        // Note: We keep the existing manager to preserve registered singletons
        // The manager will use the new config for new operations

        info!(
            "Singleton plugin config updated: {} -> {} singletons",
            old_count, new_count
        );

        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::SingletonToggle { name } => {
                info!("Handling singleton toggle for: {}", name);

                let singleton_config = self.config.get_singleton(name).ok_or_else(|| {
                    anyhow::anyhow!("Singleton '{}' not found in configuration", name)
                })?;

                self.manager.toggle(name, singleton_config).await?;
                Ok(Some(Ok(())))
            }
            _ => Ok(None), // Not handled by this plugin
        }
    }
}
