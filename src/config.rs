use anyhow::{Context, Result};
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
    #[serde(default)]
    pub scratchpads: HashMap<String, ScratchpadConfig>,
    #[serde(default)]
    pub empty: HashMap<String, EmptyWorkspaceConfig>,
    #[serde(default)]
    pub singleton: HashMap<String, SingletonConfig>,
    #[serde(default)]
    pub window_rule: Vec<WindowRuleConfig>,
    #[serde(default)]
    pub window_order: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowOrderSection {
    #[serde(default = "default_enable_event_listener")]
    pub enable_event_listener: bool,
    #[serde(default = "default_window_order_weight")]
    pub default_weight: u32,
    #[serde(default)]
    pub workspaces: Vec<String>,
}

impl Default for WindowOrderSection {
    fn default() -> Self {
        Self {
            enable_event_listener: default_enable_event_listener(),
            default_weight: default_window_order_weight(),
            workspaces: Vec::new(),
        }
    }
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
    #[serde(default)]
    pub window_order: WindowOrderSection,
}

impl Default for PiriConfig {
    fn default() -> Self {
        Self {
            scratchpad: ScratchpadDefaults::default(),
            plugins: PluginsConfig::default(),
            window_order: WindowOrderSection::default(),
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
    /// Enable/disable window_rule plugin (default: true if window rules are configured)
    #[serde(default)]
    pub window_rule: Option<bool>,
    /// Enable/disable autofill plugin (default: false)
    #[serde(default)]
    pub autofill: Option<bool>,
    /// Enable/disable singleton plugin (default: true if singleton configs are configured)
    #[serde(default)]
    pub singleton: Option<bool>,
    /// Enable/disable window_order plugin (default: true if window_order configs are configured)
    #[serde(default)]
    pub window_order: Option<bool>,
    /// Empty plugin configuration (for backward compatibility)
    #[serde(rename = "empty_config", default)]
    pub empty_config: Option<EmptyPluginConfig>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            scratchpads: None,
            empty: None,
            window_rule: None,
            autofill: None,
            singleton: None,
            window_order: None,
            empty_config: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyWorkspaceConfig {
    /// Command to execute when switching to this empty workspace
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingletonConfig {
    /// Command to execute the application (can include environment variables and arguments)
    pub command: String,
    /// Optional app_id pattern to match windows (if not specified, extracted from command)
    pub app_id: Option<String>,
}

/// Window rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRuleConfig {
    /// Regex pattern to match app_id (optional)
    pub app_id: Option<String>,
    /// Regex pattern to match title (optional)
    pub title: Option<String>,
    /// Workspace to move matching windows to (name or idx, optional if focus_command is specified)
    pub open_on_workspace: Option<String>,
    /// Command to execute when a matching window is focused (optional)
    pub focus_command: Option<String>,
}

/// Window rule plugin config (for internal use)
#[derive(Debug, Clone)]
pub struct WindowRulePluginConfig {
    /// List of window rules
    pub rules: Vec<WindowRuleConfig>,
}

impl Default for WindowRulePluginConfig {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
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

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        Ok(config)
    }

    pub fn get_scratchpad(&self, name: &str) -> Option<&ScratchpadConfig> {
        self.scratchpads.get(name)
    }

    pub fn get_singleton(&self, name: &str) -> Option<&SingletonConfig> {
        self.singleton.get(name)
    }

    pub fn get_window_order(&self, app_id: &str) -> u32 {
        // Check weights in top-level [window_order]
        if let Some(&order) = self.window_order.get(app_id) {
            return order;
        }

        // Check for partial matches
        for (config_key, &order) in &self.window_order {
            if app_id.contains(config_key) || config_key.contains(app_id) {
                return order;
            }
        }

        self.piri.window_order.default_weight
    }

    pub fn get_window_rule_plugin_config(&self) -> Option<WindowRulePluginConfig> {
        if !self.window_rule.is_empty() {
            return Some(WindowRulePluginConfig {
                rules: self.window_rule.clone(),
            });
        }
        None
    }

    pub fn get_empty_plugin_config(&self) -> Option<EmptyPluginConfig> {
        if !self.empty.is_empty() {
            let mut workspaces = std::collections::HashMap::new();
            for (workspace, config) in &self.empty {
                workspaces.insert(workspace.clone(), config.command.clone());
            }
            return Some(EmptyPluginConfig { workspaces });
        }
        self.piri.plugins.empty_config.clone()
    }

    pub fn is_window_order_event_listener_enabled(&self) -> bool {
        self.piri.window_order.enable_event_listener
    }

    pub fn get_window_order_default_weight(&self) -> u32 {
        self.piri.window_order.default_weight
    }

    pub fn get_window_order_workspaces(&self) -> Vec<String> {
        self.piri.window_order.workspaces.clone()
    }
}

impl PluginsConfig {
    pub fn is_enabled(&self, name: &str) -> bool {
        match name {
            "scratchpads" => self.scratchpads.unwrap_or(false),
            "empty" => self.empty.unwrap_or(false),
            "window_rule" => self.window_rule.unwrap_or(false),
            "autofill" => self.autofill.unwrap_or(false),
            "singleton" => self.singleton.unwrap_or(false),
            "window_order" => self.window_order.unwrap_or(false),
            _ => false,
        }
    }
}

fn default_enable_event_listener() -> bool {
    false // Default: event listener disabled
}

fn default_window_order_weight() -> u32 {
    0 // Default: unconfigured windows have weight 0 (rightmost)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            niri: NiriConfig::default(),
            piri: PiriConfig::default(),
            scratchpads: HashMap::new(),
            empty: HashMap::new(),
            singleton: HashMap::new(),
            window_rule: Vec::new(),
            window_order: HashMap::new(),
        }
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
