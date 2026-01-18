use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info};
use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::config::{Config, SingletonConfig};
use crate::ipc::IpcRequest;
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{self, WindowMatcher, WindowMatcherCache};
use crate::plugins::FromConfig;

/// Singleton plugin config (for internal use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingletonPluginConfig {
    /// Map of singleton name to config
    pub singletons: HashMap<String, SingletonConfig>,
}

impl Default for SingletonPluginConfig {
    fn default() -> Self {
        Self {
            singletons: HashMap::new(),
        }
    }
}

impl FromConfig for SingletonPluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        if config.singleton.is_empty() {
            None
        } else {
            Some(Self {
                singletons: config.singleton.clone(),
            })
        }
    }
}

#[derive(Debug, Clone)]
struct SingletonState {
    window_id: Option<u64>,
    config: SingletonConfig,
}

/// Manages singleton windows (windows that should only have one instance)
struct SingletonManager {
    niri: NiriIpc,
    states: HashMap<String, SingletonState>,
    matcher_cache: Arc<WindowMatcherCache>,
}

impl SingletonManager {
    fn new(niri: NiriIpc) -> Self {
        Self {
            niri,
            states: HashMap::new(),
            matcher_cache: Arc::new(WindowMatcherCache::new()),
        }
    }

    fn extract_app_id_from_command(command: &str) -> String {
        let cmd = command.split_whitespace().next().unwrap_or(command);
        cmd.split('/').last().unwrap_or(cmd).to_string()
    }

    fn get_window_match_pattern(config: &SingletonConfig) -> String {
        config
            .app_id
            .clone()
            .unwrap_or_else(|| Self::extract_app_id_from_command(&config.command))
    }

    async fn ensure_window_id(&mut self, name: &str) -> Result<u64> {
        let state = self.states.get_mut(name).context("Singleton state not found")?;

        if let Some(window_id) = state.window_id {
            if window_utils::window_exists(&self.niri, window_id).await? {
                return Ok(window_id);
            }
            debug!(
                "Singleton window {} (name: {}) no longer exists, clearing ID",
                window_id, name
            );
            state.window_id = None;
        }

        let config = state.config.clone();
        let window_match = Self::get_window_match_pattern(&config);
        let matcher = WindowMatcher::new(Some(vec![window_match.clone()]), None);

        let window_id = if let Some(window) =
            window_utils::find_window_by_matcher(self.niri.clone(), &matcher, &self.matcher_cache)
                .await?
        {
            window.id
        } else {
            info!("Launching application for singleton {}", name);
            window_utils::launch_application(&config.command).await?;
            let window = window_utils::wait_for_window(
                self.niri.clone(),
                &window_match,
                name,
                50,
                &self.matcher_cache,
            )
            .await?
            .context("Failed to launch/find singleton window")?;

            // Execute on_created_command if specified (only when window is newly created)
            if let Some(ref on_created_command) = config.on_created_command {
                info!(
                    "Executing on_created_command for singleton {}: {}",
                    name, on_created_command
                );
                window_utils::execute_command(on_created_command).with_context(|| {
                    format!(
                        "Failed to execute on_created_command: {}",
                        on_created_command
                    )
                })?;
            }

            window.id
        };

        let state = self.states.get_mut(name).unwrap();
        state.window_id = Some(window_id);
        Ok(window_id)
    }

    async fn toggle(&mut self, name: &str) -> Result<()> {
        info!("Toggling singleton: {}", name);
        let window_id = self.ensure_window_id(name).await?;
        window_utils::focus_window(self.niri.clone(), window_id).await?;
        Ok(())
    }

    async fn clear_cache(&self) {
        self.matcher_cache.clear_cache().await;
    }
}

/// Singleton plugin that wraps SingletonManager
pub struct SingletonPlugin {
    manager: SingletonManager,
    config: SingletonPluginConfig,
}

#[async_trait]
impl crate::plugins::Plugin for SingletonPlugin {
    type Config = SingletonPluginConfig;

    fn new(niri: NiriIpc, config: SingletonPluginConfig) -> Self {
        let count = config.singletons.len();
        info!("Singleton plugin initialized with {} singletons", count);

        let mut manager = SingletonManager::new(niri);
        for (name, s_config) in &config.singletons {
            manager.states.insert(
                name.clone(),
                SingletonState {
                    window_id: None,
                    config: s_config.clone(),
                },
            );
        }

        Self { manager, config }
    }

    async fn update_config(&mut self, config: SingletonPluginConfig) -> Result<()> {
        info!("Updating singleton plugin configuration");

        for (name, s_config) in &config.singletons {
            if let Some(state) = self.manager.states.get_mut(name) {
                state.config = s_config.clone();
            } else {
                self.manager.states.insert(
                    name.clone(),
                    SingletonState {
                        window_id: None,
                        config: s_config.clone(),
                    },
                );
            }
        }

        self.manager.states.retain(|name, _| config.singletons.contains_key(name));

        self.config = config;
        self.manager.clear_cache().await;

        Ok(())
    }

    async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
        match request {
            IpcRequest::SingletonToggle { name } => {
                info!("Handling singleton toggle for: {}", name);
                self.manager.toggle(name).await?;
                Ok(Some(Ok(())))
            }
            _ => Ok(None),
        }
    }
}
