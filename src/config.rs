use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::plugins::empty::EmptyPluginConfig;

/// Direction from which the scratchpad appears
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    FromTop,
    FromBottom,
    FromLeft,
    FromRight,
}

impl Direction {
    /// Convert string to Direction
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "fromTop" => Ok(Direction::FromTop),
            "fromBottom" => Ok(Direction::FromBottom),
            "fromLeft" => Ok(Direction::FromLeft),
            "fromRight" => Ok(Direction::FromRight),
            _ => anyhow::bail!(
                "Invalid direction: {}. Must be one of: fromTop, fromBottom, fromLeft, fromRight",
                s
            ),
        }
    }

    /// Convert Direction to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::FromTop => "fromTop",
            Direction::FromBottom => "fromBottom",
            Direction::FromLeft => "fromLeft",
            Direction::FromRight => "fromRight",
        }
    }
}

impl Serialize for Direction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Direction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Direction::from_str(&s).map_err(serde::de::Error::custom)
    }
}

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
    #[serde(default)]
    pub swallow: Vec<crate::plugins::swallow::SwallowRule>,
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
pub struct SwallowSection {
    #[serde(default)]
    pub rules: Vec<crate::plugins::swallow::SwallowRule>,
    #[serde(default = "default_true")]
    pub use_pid_matching: bool,
    #[serde(default)]
    pub exclude: Option<crate::plugins::swallow::SwallowExclude>,
}

fn default_true() -> bool {
    true
}

impl Default for SwallowSection {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            use_pid_matching: default_true(),
            exclude: None,
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
    #[serde(default)]
    pub swallow: SwallowSection,
}

impl Default for PiriConfig {
    fn default() -> Self {
        Self {
            scratchpad: ScratchpadDefaults::default(),
            plugins: PluginsConfig::default(),
            window_order: WindowOrderSection::default(),
            swallow: SwallowSection::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub scratchpads: Option<bool>,
    #[serde(default)]
    pub empty: Option<bool>,
    #[serde(default)]
    pub window_rule: Option<bool>,
    #[serde(default)]
    pub autofill: Option<bool>,
    #[serde(default)]
    pub singleton: Option<bool>,
    #[serde(default)]
    pub window_order: Option<bool>,
    #[serde(default)]
    pub swallow: Option<bool>,
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
            swallow: None,
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
    /// Optional command to execute after the window is created (only executed when window is newly created)
    #[serde(default)]
    pub on_created_command: Option<String>,
}

/// Helper type to deserialize String or Vec<String>
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum StringOrVec {
    String(String),
    Vec(Vec<String>),
}

impl StringOrVec {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringOrVec::String(s) => vec![s],
            StringOrVec::Vec(v) => v,
        }
    }
}

/// Window rule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRuleConfig {
    /// Regex pattern(s) to match app_id (optional, can be a string or list of strings)
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub app_id: Option<Vec<String>>,
    /// Regex pattern(s) to match title (optional, can be a string or list of strings)
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub title: Option<Vec<String>>,
    /// Workspace to move matching windows to (name or idx, optional if focus_command is specified)
    pub open_on_workspace: Option<String>,
    /// Command to execute when a matching window is focused (optional)
    pub focus_command: Option<String>,
    /// If true, focus_command will only execute on the first focus (default: false)
    #[serde(default)]
    pub focus_command_once: bool,
}

pub(crate) fn deserialize_string_or_vec<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    // Handle missing field case - deserialize as Option first
    let opt: Option<StringOrVec> = Option::deserialize(deserializer)?;
    Ok(opt.map(|sov| sov.into_vec()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadDefaults {
    /// Default size for dynamically added scratchpads (e.g., "40% 60%")
    #[serde(default = "default_size")]
    pub default_size: String,
    /// Default margin for dynamically added scratchpads (pixels)
    #[serde(default = "default_margin")]
    pub default_margin: u32,
    /// Optional workspace to move scratchpads to when hidden
    #[serde(default)]
    pub move_to_workspace: Option<String>,
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
            move_to_workspace: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchpadConfig {
    /// Direction from which the scratchpad appears
    pub direction: Direction,
    /// Command to execute the application (can include environment variables and arguments)
    pub command: String,
    /// Explicit app_id to match windows (required)
    pub app_id: String,
    /// Size of the scratchpad (e.g., "75% 60%")
    pub size: String,
    /// Margin from the edge in pixels
    pub margin: u32,
    /// If true, swallow the scratchpad window to the focused window when shown
    #[serde(default)]
    pub swallow_to_focus: bool,
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
    /// Load configuration from file
    /// This is the only method that should be used to load config
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
            "swallow" => self.swallow.unwrap_or(false),
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
            swallow: Vec::new(),
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
            .ok_or_else(|| anyhow::anyhow!("Missing 'direction' field"))
            .and_then(|s| Direction::from_str(s))?;

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

        let swallow_to_focus =
            table.get("swallow_to_focus").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(ScratchpadConfig {
            direction,
            command,
            app_id,
            size,
            margin,
            swallow_to_focus,
        })
    }
}
