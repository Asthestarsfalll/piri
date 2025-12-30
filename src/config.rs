use anyhow::{Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub niri: NiriConfig,
    #[serde(default)]
    pub piri: PiriConfig,
    #[serde(flatten)]
    pub scratchpads: HashMap<String, ScratchpadConfig>,
    #[serde(flatten)]
    pub empty: HashMap<String, EmptyWorkspaceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NiriConfig {
    /// Path to niri socket (default: $XDG_RUNTIME_DIR/niri or /tmp/niri)
    pub socket_path: Option<String>,
}

impl Default for NiriConfig {
    fn default() -> Self {
        Self { socket_path: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiriConfig {
    #[serde(default)]
    pub scratchpad: ScratchpadDefaults,
    #[serde(default)]
    pub plugins: PluginsConfig,
}

impl Default for PiriConfig {
    fn default() -> Self {
        Self {
            scratchpad: ScratchpadDefaults::default(),
            plugins: PluginsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Enable/disable scratchpads plugin (default: true if scratchpads are configured)
    #[serde(default)]
    pub scratchpads: Option<bool>,
    /// Enable/disable empty plugin (default: true if empty workspace rules are configured)
    #[serde(default)]
    pub empty: Option<bool>,
    /// Empty plugin configuration (for backward compatibility)
    #[serde(rename = "empty_config", default)]
    pub empty_config: Option<EmptyPluginConfig>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            scratchpads: None,
            empty: None,
            empty_config: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyWorkspaceConfig {
    /// Command to execute when switching to this empty workspace
    pub command: String,
}

/// Empty plugin config (for backward compatibility and internal use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyPluginConfig {
    /// Map of workspace identifier to command to execute when workspace is empty
    pub workspaces: std::collections::HashMap<String, String>,
}

impl Default for EmptyPluginConfig {
    fn default() -> Self {
        Self {
            workspaces: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadDefaults {
    /// Default size for dynamically added scratchpads (e.g., "40% 60%")
    #[serde(default = "default_size")]
    pub default_size: String,
    /// Default margin for dynamically added scratchpads (pixels)
    #[serde(default = "default_margin")]
    pub default_margin: u32,
}

fn default_size() -> String {
    "75% 60%".to_string()
}

fn default_margin() -> u32 {
    50
}

impl Default for ScratchpadDefaults {
    fn default() -> Self {
        Self {
            default_size: default_size(),
            default_margin: default_margin(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadConfig {
    /// Direction from which the scratchpad appears (e.g., "fromTop", "fromBottom", "fromLeft", "fromRight")
    pub direction: String,
    /// Command to execute the application (can include environment variables and arguments)
    pub command: String,
    /// Explicit app_id to match windows (required)
    pub app_id: String,
    /// Size of the scratchpad (e.g., "75% 60%")
    pub size: String,
    /// Margin from the edge in pixels
    pub margin: u32,
}

impl ScratchpadConfig {
    /// Parse size string (e.g., "75% 60%") into width and height percentages
    pub fn parse_size(&self) -> Result<(f64, f64)> {
        let parts: Vec<&str> = self.size.split_whitespace().collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "Size must be in format 'width% height%', got: {}",
                self.size
            );
        }

        let width = parts[0]
            .strip_suffix('%')
            .ok_or_else(|| anyhow::anyhow!("Width must end with %, got: {}", parts[0]))?
            .parse::<f64>()
            .context("Failed to parse width")?;

        let height = parts[1]
            .strip_suffix('%')
            .ok_or_else(|| anyhow::anyhow!("Height must end with %, got: {}", parts[1]))?
            .parse::<f64>()
            .context("Failed to parse height")?;

        Ok((width / 100.0, height / 100.0))
    }
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Create default config if file doesn't exist
        if !path.exists() {
            let default_config = Config::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).context("Failed to create config directory")?;
            }
            let toml = toml::to_string_pretty(&default_config)
                .context("Failed to serialize default config")?;
            fs::write(path, toml).context("Failed to write default config")?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        // Parse TOML manually to handle [scratchpads.term] format
        let doc: toml::Table = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        let mut config = Config {
            niri: NiriConfig::default(),
            piri: PiriConfig::default(),
            scratchpads: HashMap::new(),
            empty: HashMap::new(),
        };

        // Extract niri config
        if let Some(niri_table) = doc.get("niri") {
            if let Some(niri_map) = niri_table.as_table() {
                if let Some(socket_path) = niri_map.get("socket_path") {
                    if let Some(path_str) = socket_path.as_str() {
                        config.niri.socket_path = Some(path_str.to_string());
                    }
                }
            }
        }

        // Extract piri config
        if let Some(piri_table) = doc.get("piri") {
            if let Some(piri_map) = piri_table.as_table() {
                if let Some(scratchpad_table) = piri_map.get("scratchpad") {
                    if let Some(scratchpad_map) = scratchpad_table.as_table() {
                        if let Some(default_size) = scratchpad_map.get("default_size") {
                            if let Some(size_str) = default_size.as_str() {
                                config.piri.scratchpad.default_size = size_str.to_string();
                            }
                        }
                        if let Some(default_margin) = scratchpad_map.get("default_margin") {
                            if let Some(margin_int) = default_margin.as_integer() {
                                config.piri.scratchpad.default_margin = margin_int as u32;
                            }
                        }
                    }
                }

                // Extract plugins config
                if let Some(plugins_table) = piri_map.get("plugins") {
                    if let Some(plugins_map) = plugins_table.as_table() {
                        // Extract plugin enable/disable flags
                        if let Some(scratchpads_enabled) = plugins_map.get("scratchpads") {
                            if let Some(enabled) = scratchpads_enabled.as_bool() {
                                config.piri.plugins.scratchpads = Some(enabled);
                            }
                        }
                        if let Some(empty_value) = plugins_map.get("empty") {
                            // Check if it's a boolean (enable/disable flag)
                            if let Some(enabled) = empty_value.as_bool() {
                                config.piri.plugins.empty = Some(enabled);
                            }
                            // Check if it's a table (old format: [piri.plugins.empty.workspaces])
                            else if let Some(empty_map) = empty_value.as_table() {
                                if let Some(workspaces_table) = empty_map.get("workspaces") {
                                    if let Some(workspaces_map) = workspaces_table.as_table() {
                                        let mut empty_config = EmptyPluginConfig::default();
                                        for (key, value) in workspaces_map.iter() {
                                            if let Some(cmd_str) = value.as_str() {
                                                empty_config
                                                    .workspaces
                                                    .insert(key.clone(), cmd_str.to_string());
                                            }
                                        }
                                        config.piri.plugins.empty_config = Some(empty_config);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Extract empty plugin config (new format: [empty.{workspace}])
        // In TOML, [empty.1] creates a nested structure: { "empty": { "1": { ... } } }
        if let Some(empty_table) = doc.get("empty") {
            if let Some(empty_map) = empty_table.as_table() {
                for (workspace, value) in empty_map.iter() {
                    if let Some(workspace_table) = value.as_table() {
                        if let Some(command) = workspace_table.get("command") {
                            if let Some(cmd_str) = command.as_str() {
                                config.empty.insert(
                                    workspace.clone(),
                                    EmptyWorkspaceConfig {
                                        command: cmd_str.to_string(),
                                    },
                                );
                                log::debug!(
                                    "Parsed empty workspace config: {} -> {}",
                                    workspace,
                                    cmd_str
                                );
                            }
                        }
                    }
                }
                log::info!(
                    "Parsed {} empty workspace configurations",
                    config.empty.len()
                );
            }
        }

        // Extract scratchpads (format: [scratchpads.term])
        // In TOML, [scratchpads.term] creates a nested structure: { "scratchpads": { "term": { ... } } }
        if let Some(scratchpads_table) = doc.get("scratchpads") {
            if let Some(scratchpads_map) = scratchpads_table.as_table() {
                for (name, value) in scratchpads_map.iter() {
                    if let Some(scratchpad_table) = value.as_table() {
                        match scratchpad_table.clone().try_into() {
                            Ok(scratchpad) => {
                                config.scratchpads.insert(name.clone(), scratchpad);
                            }
                            Err(e) => {
                                warn!("Failed to parse scratchpad config for '{}': {}", name, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(config)
    }

    pub fn get_scratchpad(&self, name: &str) -> Option<&ScratchpadConfig> {
        self.scratchpads.get(name)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            niri: NiriConfig::default(),
            piri: PiriConfig::default(),
            scratchpads: HashMap::new(),
            empty: HashMap::new(),
        }
    }
}

impl Config {
    /// Get empty plugin config (converts new format to old format for plugin compatibility)
    pub fn get_empty_plugin_config(&self) -> Option<EmptyPluginConfig> {
        // Check new format first
        if !self.empty.is_empty() {
            let mut workspaces = std::collections::HashMap::new();
            for (workspace, config) in &self.empty {
                workspaces.insert(workspace.clone(), config.command.clone());
            }
            log::debug!(
                "Empty plugin: found {} workspaces in new format",
                workspaces.len()
            );
            return Some(EmptyPluginConfig { workspaces });
        }

        // Fallback to old format
        if let Some(ref old_config) = self.piri.plugins.empty_config {
            log::debug!(
                "Empty plugin: found {} workspaces in old format",
                old_config.workspaces.len()
            );
            return Some(old_config.clone());
        }

        log::debug!("Empty plugin: no configuration found");
        None
    }

    /// Check if scratchpads plugin should be enabled
    pub fn is_scratchpads_enabled(&self) -> bool {
        // If explicitly set, use that value
        // Otherwise, default to false (disabled)
        self.piri.plugins.scratchpads.unwrap_or(false)
    }

    /// Check if empty plugin should be enabled
    pub fn is_empty_enabled(&self) -> bool {
        // If explicitly set, use that value
        // Otherwise, default to false (disabled)
        self.piri.plugins.empty.unwrap_or(false)
    }
}

// Helper to convert TOML table to ScratchpadConfig
impl TryFrom<toml::Table> for ScratchpadConfig {
    type Error = anyhow::Error;

    fn try_from(table: toml::Table) -> Result<Self> {
        let direction = table
            .get("direction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'direction' field"))?
            .to_string();

        let command = table
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' field"))?
            .to_string();

        let size = table
            .get("size")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'size' field"))?
            .to_string();

        let margin = table
            .get("margin")
            .and_then(|v| v.as_integer())
            .ok_or_else(|| anyhow::anyhow!("Missing 'margin' field"))? as u32;

        let app_id = table
            .get("app_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'app_id' field"))?
            .to_string();

        Ok(ScratchpadConfig {
            direction,
            command,
            app_id,
            size,
            margin,
        })
    }
}
