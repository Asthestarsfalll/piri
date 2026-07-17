use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::plugins::empty::EmptyPluginConfig;

/// Direction from which the scratchpad appears
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

impl std::str::FromStr for Direction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "fromTop" => Ok(Direction::Top),
            "fromBottom" => Ok(Direction::Bottom),
            "fromLeft" => Ok(Direction::Left),
            "fromRight" => Ok(Direction::Right),
            _ => anyhow::bail!(
                "Invalid direction: {}. Must be one of: fromTop, fromBottom, fromLeft, fromRight",
                s
            ),
        }
    }
}

impl Direction {
    /// Convert Direction to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Top => "fromTop",
            Direction::Bottom => "fromBottom",
            Direction::Left => "fromLeft",
            Direction::Right => "fromRight",
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
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(default)]
    pub workspace_rule: HashMap<String, WorkspaceRuleConfig>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NiriConfig {
    /// Path to niri socket (default: $XDG_RUNTIME_DIR/niri or /tmp/niri)
    pub socket_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PiriConfig {
    #[serde(default)]
    pub scratchpad: ScratchpadDefaults,
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub window_order: WindowOrderSection,
    #[serde(default)]
    pub swallow: SwallowSection,
    #[serde(default)]
    pub workspace_rule: WorkspaceRuleSection,
    #[serde(default)]
    pub mark: MarkSection,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarkSection {
    /// If true, toggling a mark that is already focused will return to the previous window
    #[serde(default)]
    pub refocus: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(default)]
    pub workspace_rule: Option<bool>,
    #[serde(default)]
    pub mark: Option<bool>,
    #[serde(default)]
    pub sticky: Option<bool>,
    #[serde(rename = "empty_config", default)]
    pub empty_config: Option<EmptyPluginConfig>,
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
    /// If true, scratchpad will follow the focused workspace (delegated to sticky plugin)
    #[serde(default)]
    pub sticky: bool,
    /// If true, scratchpad will automatically hide when it loses focus
    #[serde(default)]
    pub auto_hide_on_focus_loss: bool,
    /// If true, when the scratchpad is visible and focused, toggle will refocus to the previous window
    #[serde(default)]
    pub refocus: bool,
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
            "singleton" => self.singleton.unwrap_or(false),
            "window_order" => self.window_order.unwrap_or(false),
            "swallow" => self.swallow.unwrap_or(false),
            "workspace_rule" => self.workspace_rule.unwrap_or(false),
            "mark" => self.mark.unwrap_or(false),
            "sticky" => self.sticky.unwrap_or(false),
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

/// Helper type to deserialize String or Vec<String> for auto_width
/// This allows both "50%" and ["45%", "55%"] formats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum WidthValue {
    String(String),
    Vec(Vec<String>),
}

impl WidthValue {
    /// Convert to Vec<String>, expanding single string to vec
    fn into_vec(self) -> Vec<String> {
        match self {
            WidthValue::String(s) => vec![s],
            WidthValue::Vec(v) => v,
        }
    }
}

/// Custom deserializer for auto_width array
/// Handles nested arrays: ["100%", "50%"] or ["100%", ["45%", "55%"]]
fn deserialize_auto_width<'de, D>(deserializer: D) -> Result<Vec<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::Deserialize;

    // Deserialize as Vec<WidthValue>
    let values: Vec<WidthValue> = Vec::deserialize(deserializer)?;

    // Convert each element to Vec<String>
    let result: Vec<Vec<String>> = values.into_iter().map(|v| v.into_vec()).collect();

    Ok(result)
}

/// Workspace rule configuration for a specific workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRuleConfig {
    /// Auto width configuration: array where index corresponds to window count (1-based)
    /// Each element can be a string (all windows same width) or array (different widths per window)
    /// Examples:
    ///   ["100%", "50%"] - 1 window: 100%, 2 windows: each 50%
    ///   ["100%", ["45%", "55%"]] - 1 window: 100%, 2 windows: 45% and 55%
    #[serde(deserialize_with = "deserialize_auto_width", default)]
    pub auto_width: Vec<Vec<String>>,
    /// If true, automatically tile windows: allow up to 2 windows per column (except first column)
    #[serde(default)]
    pub auto_tile: bool,
    /// If true, automatically align last column (autofill)
    #[serde(default, rename = "auto_fill")]
    pub auto_fill: bool,
    /// If true, automatically maximize window when there's only one window, and unmaximize when there are multiple windows
    #[serde(default)]
    pub auto_maximize: bool,
    /// EdgePulse indicator config for this workspace.
    #[serde(default)]
    pub edge_pulse: EdgePulseConfig,
}

/// Workspace rule section in piri config (default settings)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceRuleSection {
    /// Default auto width configuration
    #[serde(deserialize_with = "deserialize_auto_width", default)]
    pub auto_width: Vec<Vec<String>>,
    /// If true, automatically tile windows: allow up to 2 windows per column (except first column)
    #[serde(default)]
    pub auto_tile: bool,
    /// If true, automatically align last column (autofill)
    #[serde(default, rename = "auto_fill")]
    pub auto_fill: bool,
    /// If true, automatically maximize window when there's only one window, and unmaximize when there are multiple windows
    #[serde(default)]
    pub auto_maximize: bool,
    /// Default EdgePulse indicator config for all workspaces.
    #[serde(default)]
    pub edge_pulse: EdgePulseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgePulseConfig {
    /// Enable left/right missing-neighbor indicator.
    #[serde(default)]
    pub enabled: bool,
    /// Show left-side indicator when there is no left neighbor.
    #[serde(default = "default_true")]
    pub show_left: bool,
    /// Show right-side indicator when there is no right neighbor.
    #[serde(default = "default_true")]
    pub show_right: bool,
    /// Indicator width in pixels.
    #[serde(default = "default_edge_pulse_width")]
    pub width: u32,
    /// Indicator height ratio to output height, range 0.0-1.0.
    #[serde(default = "default_edge_pulse_height_ratio")]
    pub height_ratio: f64,
    /// Gradient start color for left edge.
    #[serde(default = "default_left_start")]
    pub left_gradient_start: String,
    /// Gradient end color for left edge.
    #[serde(default = "default_left_end")]
    pub left_gradient_end: String,
    /// Gradient start color for right edge.
    #[serde(default = "default_right_start")]
    pub right_gradient_start: String,
    /// Gradient end color for right edge.
    #[serde(default = "default_right_end")]
    pub right_gradient_end: String,
    /// Global alpha 0.0-1.0.
    #[serde(default = "default_edge_pulse_alpha")]
    pub alpha: f64,
    /// Enable animation effect (pulse/fade).
    #[serde(default)]
    pub animation_enabled: bool,
    /// Animation style: "pulse" | "fade".
    #[serde(default = "default_animation_style")]
    pub animation_style: String,
    /// Animation duration in milliseconds per cycle.
    #[serde(default = "default_animation_duration")]
    pub animation_duration: f64,
    /// Animation amplitude 0.0-1.0, controls intensity.
    #[serde(default = "default_animation_amplitude")]
    pub animation_amplitude: f64,
    /// Number of animation repeats (0 = infinite loop until state changes).
    #[serde(default = "default_animation_repeat")]
    pub animation_repeat: u32,
}

impl Default for EdgePulseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_left: true,
            show_right: true,
            width: default_edge_pulse_width(),
            height_ratio: default_edge_pulse_height_ratio(),
            left_gradient_start: default_left_start(),
            left_gradient_end: default_left_end(),
            right_gradient_start: default_right_start(),
            right_gradient_end: default_right_end(),
            alpha: default_edge_pulse_alpha(),
            animation_enabled: false,
            animation_style: default_animation_style(),
            animation_duration: default_animation_duration(),
            animation_amplitude: default_animation_amplitude(),
            animation_repeat: default_animation_repeat(),
        }
    }
}

fn default_edge_pulse_width() -> u32 {
    14
}

fn default_edge_pulse_height_ratio() -> f64 {
    0.42
}

fn default_edge_pulse_alpha() -> f64 {
    0.85
}

fn default_animation_style() -> String {
    "pulse".to_string()
}

fn default_animation_duration() -> f64 {
    600.0
}

fn default_animation_amplitude() -> f64 {
    0.8
}

fn default_animation_repeat() -> u32 {
    3
}

fn default_left_start() -> String {
    "#68d8ff".to_string()
}

fn default_left_end() -> String {
    "#1f4fff".to_string()
}

fn default_right_start() -> String {
    "#ffd36a".to_string()
}

fn default_right_end() -> String {
    "#ff7a1f".to_string()
}

// Helper to convert TOML table to ScratchpadConfig
impl TryFrom<toml::Table> for ScratchpadConfig {
    type Error = anyhow::Error;

    fn try_from(table: toml::Table) -> Result<Self> {
        let direction = table
            .get("direction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'direction' field"))
            .and_then(Direction::from_str)?;

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

        let sticky = table.get("sticky").and_then(|v| v.as_bool()).unwrap_or(false);

        let auto_hide_on_focus_loss =
            table.get("auto_hide_on_focus_loss").and_then(|v| v.as_bool()).unwrap_or(false);

        let refocus = table.get("refocus").and_then(|v| v.as_bool()).unwrap_or(false);

        if sticky && auto_hide_on_focus_loss {
            anyhow::bail!(
                "'sticky' and 'auto_hide_on_focus_loss' cannot both be enabled for a scratchpad"
            );
        }

        Ok(ScratchpadConfig {
            direction,
            command,
            app_id,
            size,
            margin,
            swallow_to_focus,
            sticky,
            auto_hide_on_focus_loss,
            refocus,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Direction ====================

    #[test]
    fn test_direction_parse_valid() {
        assert_eq!("fromTop".parse::<Direction>().unwrap(), Direction::Top);
        assert_eq!(
            "fromBottom".parse::<Direction>().unwrap(),
            Direction::Bottom
        );
        assert_eq!("fromLeft".parse::<Direction>().unwrap(), Direction::Left);
        assert_eq!("fromRight".parse::<Direction>().unwrap(), Direction::Right);
    }

    #[test]
    fn test_direction_parse_invalid() {
        assert!("up".parse::<Direction>().is_err());
        assert!("".parse::<Direction>().is_err());
    }

    #[test]
    fn test_direction_as_str() {
        assert_eq!(Direction::Top.as_str(), "fromTop");
        assert_eq!(Direction::Bottom.as_str(), "fromBottom");
        assert_eq!(Direction::Left.as_str(), "fromLeft");
        assert_eq!(Direction::Right.as_str(), "fromRight");
    }

    #[test]
    fn test_direction_roundtrip() {
        for dir in [
            Direction::Top,
            Direction::Bottom,
            Direction::Left,
            Direction::Right,
        ] {
            let s = dir.as_str();
            let parsed: Direction = s.parse().unwrap();
            assert_eq!(parsed, dir);
        }
    }

    // ==================== NiriConfig ====================

    #[test]
    fn test_niri_config_default() {
        let config: NiriConfig = toml::from_str("").unwrap();
        assert!(config.socket_path.is_none());
    }

    #[test]
    fn test_niri_config_with_socket() {
        let toml = r#"socket_path = "/tmp/niri""#;
        let config: NiriConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.socket_path.as_deref(), Some("/tmp/niri"));
    }

    // ==================== PluginsConfig ====================

    #[test]
    fn test_plugins_config_default() {
        let config: PluginsConfig = toml::from_str("").unwrap();
        assert_eq!(config.is_enabled("scratchpads"), false);
        assert_eq!(config.is_enabled("empty"), false);
        assert_eq!(config.is_enabled("window_rule"), false);
        assert_eq!(config.is_enabled("singleton"), false);
        assert_eq!(config.is_enabled("window_order"), false);
        assert_eq!(config.is_enabled("swallow"), false);
        assert_eq!(config.is_enabled("workspace_rule"), false);
        assert_eq!(config.is_enabled("mark"), false);
        assert_eq!(config.is_enabled("sticky"), false);
        assert_eq!(config.is_enabled("unknown_plugin"), false);
    }

    #[test]
    fn test_plugins_config_all_enabled() {
        let toml = r#"
scratchpads = true
empty = true
window_rule = true
singleton = true
window_order = true
swallow = true
workspace_rule = true
mark = true
sticky = true
"#;
        let config: PluginsConfig = toml::from_str(toml).unwrap();
        assert!(config.is_enabled("scratchpads"));
        assert!(config.is_enabled("empty"));
        assert!(config.is_enabled("window_rule"));
        assert!(config.is_enabled("singleton"));
        assert!(config.is_enabled("window_order"));
        assert!(config.is_enabled("swallow"));
        assert!(config.is_enabled("workspace_rule"));
        assert!(config.is_enabled("mark"));
        assert!(config.is_enabled("sticky"));
    }

    // ==================== ScratchpadDefaults ====================

    #[test]
    fn test_scratchpad_defaults() {
        let config: ScratchpadDefaults = toml::from_str("").unwrap();
        assert_eq!(config.default_size, "75% 60%");
        assert_eq!(config.default_margin, 50);
        assert!(config.move_to_workspace.is_none());
    }

    #[test]
    fn test_scratchpad_defaults_custom() {
        let toml = r#"
default_size = "40% 80%"
default_margin = 100
move_to_workspace = "tmp"
"#;
        let config: ScratchpadDefaults = toml::from_str(toml).unwrap();
        assert_eq!(config.default_size, "40% 80%");
        assert_eq!(config.default_margin, 100);
        assert_eq!(config.move_to_workspace.as_deref(), Some("tmp"));
    }

    // ==================== ScratchpadConfig ====================

    #[test]
    fn test_scratchpad_config_minimal() {
        let toml = r#"
direction = "fromRight"
command = "ghostty"
app_id = "float.term"
size = "40% 60%"
margin = 50
"#;
        let config: ScratchpadConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.direction, Direction::Right);
        assert_eq!(config.command, "ghostty");
        assert_eq!(config.app_id, "float.term");
        assert_eq!(config.size, "40% 60%");
        assert_eq!(config.margin, 50);
        assert!(!config.swallow_to_focus);
        assert!(!config.sticky);
        assert!(!config.auto_hide_on_focus_loss);
        assert!(!config.refocus);
    }

    #[test]
    fn test_scratchpad_config_all_options() {
        let toml = r#"
direction = "fromTop"
command = "gnome-text-editor"
app_id = "org.gnome.TextEditor"
size = "50% 40%"
margin = 100
swallow_to_focus = true
sticky = true
auto_hide_on_focus_loss = false
refocus = true
"#;
        let config: ScratchpadConfig = toml::from_str(toml).unwrap();
        assert!(config.swallow_to_focus);
        assert!(config.sticky);
        assert!(!config.auto_hide_on_focus_loss);
        assert!(config.refocus);
    }

    #[test]
    fn test_scratchpad_parse_size() {
        let config = ScratchpadConfig {
            direction: Direction::Right,
            command: "test".into(),
            app_id: "test".into(),
            size: "40% 60%".into(),
            margin: 50,
            swallow_to_focus: false,
            sticky: false,
            auto_hide_on_focus_loss: false,
            refocus: false,
        };
        let (w, h) = config.parse_size().unwrap();
        assert!((w - 0.4).abs() < f64::EPSILON);
        assert!((h - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scratchpad_parse_size_invalid() {
        let config = ScratchpadConfig {
            direction: Direction::Right,
            command: "test".into(),
            app_id: "test".into(),
            size: "40%".into(),
            margin: 50,
            swallow_to_focus: false,
            sticky: false,
            auto_hide_on_focus_loss: false,
            refocus: false,
        };
        assert!(config.parse_size().is_err());
    }

    // ==================== EmptyWorkspaceConfig ====================

    #[test]
    fn test_empty_workspace_config() {
        let toml = "command = \"notify-send empty\"";
        let config: EmptyWorkspaceConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.command, "notify-send empty");
    }

    // ==================== SingletonConfig ====================

    #[test]
    fn test_singleton_config_minimal() {
        let toml = r#"command = "google-chrome""#;
        let config: SingletonConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.command, "google-chrome");
        assert!(config.app_id.is_none());
        assert!(config.on_created_command.is_none());
    }

    #[test]
    fn test_singleton_config_all_fields() {
        let toml = r#"
command = "ghostty --class=singleton.term"
app_id = "singleton.term"
on_created_command = "echo created"
"#;
        let config: SingletonConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.command, "ghostty --class=singleton.term");
        assert_eq!(config.app_id.as_deref(), Some("singleton.term"));
        assert_eq!(config.on_created_command.as_deref(), Some("echo created"));
    }

    // ==================== WindowRuleConfig ====================

    #[test]
    fn test_window_rule_config_app_id_string() {
        let toml = r#"
app_id = "firefox"
open_on_workspace = "2"
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.app_id, Some(vec!["firefox".to_string()]));
        assert!(config.title.is_none());
        assert_eq!(config.open_on_workspace.as_deref(), Some("2"));
        assert!(config.focus_command.is_none());
        assert!(!config.focus_command_once);
    }

    #[test]
    fn test_window_rule_config_app_id_vec() {
        let toml = r#"
app_id = ["code", "code-oss", "codium"]
open_on_workspace = "dev"
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            config.app_id,
            Some(vec![
                "code".to_string(),
                "code-oss".to_string(),
                "codium".to_string()
            ])
        );
    }

    #[test]
    fn test_window_rule_config_title_only() {
        let toml = r#"
title = ".*Chrome.*"
focus_command = "notify-send 'focused'"
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert!(config.app_id.is_none());
        assert_eq!(config.title, Some(vec![".*Chrome.*".to_string()]));
        assert!(config.open_on_workspace.is_none());
        assert_eq!(
            config.focus_command.as_deref(),
            Some("notify-send 'focused'")
        );
    }

    #[test]
    fn test_window_rule_config_focus_command_once() {
        let toml = r#"
app_id = "test"
focus_command = "echo hi"
focus_command_once = true
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert!(config.focus_command_once);
    }

    // ==================== WindowOrderSection ====================

    #[test]
    fn test_window_order_section_default() {
        let config: WindowOrderSection = toml::from_str("").unwrap();
        assert!(!config.enable_event_listener);
        assert_eq!(config.default_weight, 0);
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn test_window_order_section_custom() {
        let toml = r#"
enable_event_listener = true
default_weight = 10
workspaces = ["1", "2", "dev"]
"#;
        let config: WindowOrderSection = toml::from_str(toml).unwrap();
        assert!(config.enable_event_listener);
        assert_eq!(config.default_weight, 10);
        assert_eq!(config.workspaces, vec!["1", "2", "dev"]);
    }

    // ==================== MarkSection ====================

    #[test]
    fn test_mark_section_default() {
        let config: MarkSection = toml::from_str("").unwrap();
        assert!(!config.refocus);
    }

    #[test]
    fn test_mark_section_refocus() {
        let config: MarkSection = toml::from_str("refocus = true").unwrap();
        assert!(config.refocus);
    }

    // ==================== SwallowSection ====================

    #[test]
    fn test_swallow_section_default() {
        let config: SwallowSection = toml::from_str("").unwrap();
        assert!(config.use_pid_matching);
        assert!(config.rules.is_empty());
        assert!(config.exclude.is_none());
    }

    #[test]
    fn test_swallow_section_custom() {
        let toml = r#"
use_pid_matching = false

[[rules]]
child_app_id = ".*chrome.*"
parent_app_id = ".*ghostty.*"

[exclude]
app_id = ".*mpv.*"
"#;
        let config: SwallowSection = toml::from_str(toml).unwrap();
        assert!(!config.use_pid_matching);
        assert_eq!(config.rules.len(), 1);
        assert!(config.exclude.is_some());
    }

    // ==================== SwallowRule ====================

    #[test]
    fn test_swallow_rule_all_fields() {
        let toml = r#"
child_app_id = ".*chrome.*"
child_title = ".*Chrome.*"
parent_app_id = ".*ghostty.*"
parent_title = ".*Ghostty.*"
"#;
        let rule: crate::plugins::swallow::SwallowRule = toml::from_str(toml).unwrap();
        assert_eq!(rule.child_app_id, Some(vec![".*chrome.*".to_string()]));
        assert_eq!(rule.child_title, Some(vec![".*Chrome.*".to_string()]));
        assert_eq!(rule.parent_app_id, Some(vec![".*ghostty.*".to_string()]));
        assert_eq!(rule.parent_title, Some(vec![".*Ghostty.*".to_string()]));
    }

    #[test]
    fn test_swallow_rule_vec_app_id() {
        let toml = r#"
child_app_id = [".*chrome.*", ".*chromium.*"]
parent_app_id = ".*ghostty.*"
"#;
        let rule: crate::plugins::swallow::SwallowRule = toml::from_str(toml).unwrap();
        assert_eq!(
            rule.child_app_id,
            Some(vec![".*chrome.*".to_string(), ".*chromium.*".to_string()])
        );
    }

    // ==================== SwallowExclude ====================

    #[test]
    fn test_swallow_exclude() {
        let toml = r#"
app_id = ".*mpv.*"
title = ".*mpv.*"
"#;
        let exclude: crate::plugins::swallow::SwallowExclude = toml::from_str(toml).unwrap();
        assert_eq!(exclude.app_id, Some(vec![".*mpv.*".to_string()]));
        assert_eq!(exclude.title, Some(vec![".*mpv.*".to_string()]));
    }

    // ==================== WorkspaceRuleConfig ====================

    #[test]
    fn test_workspace_rule_config_minimal() {
        // auto_width is optional after the fix
        let toml = "auto_maximize = true";
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert!(config.auto_width.is_empty());
        assert!(!config.auto_tile);
        assert!(!config.auto_fill);
        assert!(config.auto_maximize);
    }

    #[test]
    fn test_workspace_rule_config_auto_width_strings() {
        let toml = r#"auto_width = ["100%", "50%", "33.33%"]"#;
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.auto_width.len(), 3);
        assert_eq!(config.auto_width[0], vec!["100%"]);
        assert_eq!(config.auto_width[1], vec!["50%"]);
        assert_eq!(config.auto_width[2], vec!["33.33%"]);
    }

    #[test]
    fn test_workspace_rule_config_auto_width_nested() {
        let toml = r#"auto_width = ["100%", ["45%", "55%"], "33.33%"]"#;
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.auto_width.len(), 3);
        assert_eq!(config.auto_width[0], vec!["100%"]);
        assert_eq!(config.auto_width[1], vec!["45%", "55%"]);
        assert_eq!(config.auto_width[2], vec!["33.33%"]);
    }

    #[test]
    fn test_workspace_rule_config_auto_width_deep_nested() {
        let toml = r#"auto_width = ["100%", "50%", ["30%", "35%", "35%"]]"#;
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.auto_width.len(), 3);
        assert_eq!(config.auto_width[2], vec!["30%", "35%", "35%"]);
    }

    #[test]
    fn test_workspace_rule_config_auto_tile_and_fill() {
        let toml = r#"
auto_tile = true
auto_fill = true
"#;
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert!(config.auto_tile);
        assert!(config.auto_fill);
    }

    // ==================== WorkspaceRuleSection ====================

    #[test]
    fn test_workspace_rule_section_default() {
        let config: WorkspaceRuleSection = toml::from_str("").unwrap();
        assert!(config.auto_width.is_empty());
        assert!(!config.auto_tile);
        assert!(!config.auto_fill);
        assert!(!config.auto_maximize);
    }

    #[test]
    fn test_workspace_rule_section_custom() {
        let toml = r#"
auto_width = ["100%", "50%"]
auto_maximize = true
"#;
        let config: WorkspaceRuleSection = toml::from_str(toml).unwrap();
        assert_eq!(config.auto_width.len(), 2);
        assert!(config.auto_maximize);
    }

    // ==================== EdgePulseConfig ====================

    #[test]
    fn test_edge_pulse_config_default() {
        let config: EdgePulseConfig = toml::from_str("").unwrap();
        assert!(!config.enabled);
        assert!(config.show_left);
        assert!(config.show_right);
        assert_eq!(config.width, 14);
        assert!((config.height_ratio - 0.42).abs() < f64::EPSILON);
        assert_eq!(config.left_gradient_start, "#68d8ff");
        assert_eq!(config.left_gradient_end, "#1f4fff");
        assert_eq!(config.right_gradient_start, "#ffd36a");
        assert_eq!(config.right_gradient_end, "#ff7a1f");
        assert!((config.alpha - 0.85).abs() < f64::EPSILON);
        assert!(!config.animation_enabled);
        assert_eq!(config.animation_style, "pulse");
        assert!((config.animation_duration - 600.0).abs() < f64::EPSILON);
        assert!((config.animation_amplitude - 0.8).abs() < f64::EPSILON);
        assert_eq!(config.animation_repeat, 3);
    }

    #[test]
    fn test_edge_pulse_config_custom() {
        let toml = "\
enabled = true
show_left = false
show_right = false
width = 20
height_ratio = 0.5
left_gradient_start = \"#ff0000\"
left_gradient_end = \"#00ff00\"
right_gradient_start = \"#0000ff\"
right_gradient_end = \"#ffff00\"
alpha = 1.0
animation_enabled = true
animation_style = \"fade\"
animation_duration = 300.0
animation_amplitude = 0.5
animation_repeat = 0
";
        let config: EdgePulseConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert!(!config.show_left);
        assert!(!config.show_right);
        assert_eq!(config.width, 20);
        assert!((config.height_ratio - 0.5).abs() < f64::EPSILON);
        assert_eq!(config.left_gradient_start, "#ff0000");
        assert_eq!(config.animation_style, "fade");
        assert_eq!(config.animation_repeat, 0);
    }

    // ==================== PiriConfig ====================

    #[test]
    fn test_piri_config_default() {
        let config: PiriConfig = toml::from_str("").unwrap();
        assert_eq!(config.swallow.use_pid_matching, true);
        assert!(config.mark.refocus == false);
    }

    // ==================== Config (root) ====================

    #[test]
    fn test_config_empty() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.scratchpads.is_empty());
        assert!(config.empty.is_empty());
        assert!(config.window_rule.is_empty());
        assert!(config.window_order.is_empty());
        assert!(config.swallow.is_empty());
        assert!(config.workspace_rule.is_empty());
    }

    #[test]
    fn test_config_example_parses() {
        let content = include_str!("../config.example.toml");
        let config: Config =
            toml::from_str(content).expect("config.example.toml should parse without errors");

        // workspace_rule.main should work with only auto_maximize (no auto_width required)
        let main_rule =
            config.workspace_rule.get("main").expect("workspace_rule.main should exist");
        assert!(main_rule.auto_maximize);
        assert!(main_rule.auto_width.is_empty());

        // workspace_rule.browser should have nested auto_width
        let browser_rule = config
            .workspace_rule
            .get("browser")
            .expect("workspace_rule.browser should exist");
        assert_eq!(browser_rule.auto_width.len(), 3);
        assert_eq!(browser_rule.auto_width[0], vec!["100%"]);
        assert_eq!(browser_rule.auto_width[1], vec!["45%", "55%"]);
        assert_eq!(browser_rule.auto_width[2], vec!["33.33%"]);
    }

    #[test]
    fn test_config_full_minimal() {
        let toml = r#"
[niri]
socket_path = "/tmp/niri"

[piri.plugins]
scratchpads = true

[scratchpads.term]
direction = "fromRight"
command = "ghostty"
app_id = "float.term"
size = "40% 60%"
margin = 50

[empty.1]
command = "notify-send empty"

[singleton.browser]
command = "google-chrome"

[[window_rule]]
app_id = "firefox"
open_on_workspace = "2"

[window_order]
firefox = 100

[[swallow]]
child_app_id = ".*chrome.*"
parent_app_id = ".*ghostty.*"

[workspace_rule.main]
auto_width = ["100%", "50%"]
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.niri.socket_path.as_deref(), Some("/tmp/niri"));
        assert!(config.scratchpads.contains_key("term"));
        assert!(config.empty.contains_key("1"));
        assert!(config.singleton.contains_key("browser"));
        assert_eq!(config.window_rule.len(), 1);
        assert_eq!(config.window_order.get("firefox"), Some(&100));
        assert_eq!(config.swallow.len(), 1);
        assert!(config.workspace_rule.contains_key("main"));
    }

    // ==================== deserialize_string_or_vec ====================

    #[test]
    fn test_deserialize_string_or_vec_string() {
        let toml = r#"
app_id = "firefox"
open_on_workspace = "2"
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.app_id, Some(vec!["firefox".to_string()]));
    }

    #[test]
    fn test_deserialize_string_or_vec_vec() {
        let toml = r#"
app_id = ["firefox", "chromium"]
open_on_workspace = "2"
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            config.app_id,
            Some(vec!["firefox".to_string(), "chromium".to_string()])
        );
    }

    #[test]
    fn test_deserialize_string_or_vec_none() {
        let toml = r#"
open_on_workspace = "2"
"#;
        let config: WindowRuleConfig = toml::from_str(toml).unwrap();
        assert!(config.app_id.is_none());
    }

    // ==================== deserialize_auto_width ====================

    #[test]
    fn test_auto_width_single_string() {
        let toml = r#"auto_width = ["100%"]"#;
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.auto_width, vec![vec!["100%"]]);
    }

    #[test]
    fn test_auto_width_mixed() {
        let toml = r#"auto_width = ["100%", ["45%", "55%"]]"#;
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.auto_width, vec![vec!["100%"], vec!["45%", "55%"]]);
    }

    #[test]
    fn test_auto_width_empty() {
        let toml = "auto_maximize = true";
        let config: WorkspaceRuleConfig = toml::from_str(toml).unwrap();
        assert!(config.auto_width.is_empty());
    }

    // ==================== ScratchpadConfig TryFrom ====================

    #[test]
    fn test_scratchpad_try_from_table() {
        let mut table = toml::Table::new();
        table.insert("direction".into(), "fromRight".into());
        table.insert("command".into(), "ghostty".into());
        table.insert("app_id".into(), "float.term".into());
        table.insert("size".into(), "40% 60%".into());
        table.insert("margin".into(), 50i64.into());
        table.insert("swallow_to_focus".into(), true.into());
        table.insert("sticky".into(), false.into());
        table.insert("auto_hide_on_focus_loss".into(), false.into());
        table.insert("refocus".into(), false.into());

        let config = ScratchpadConfig::try_from(table).unwrap();
        assert_eq!(config.direction, Direction::Right);
        assert!(config.swallow_to_focus);
    }

    #[test]
    fn test_scratchpad_try_from_sticky_conflict() {
        let mut table = toml::Table::new();
        table.insert("direction".into(), "fromRight".into());
        table.insert("command".into(), "ghostty".into());
        table.insert("app_id".into(), "float.term".into());
        table.insert("size".into(), "40% 60%".into());
        table.insert("margin".into(), 50i64.into());
        table.insert("sticky".into(), true.into());
        table.insert("auto_hide_on_focus_loss".into(), true.into());

        assert!(ScratchpadConfig::try_from(table).is_err());
    }

    // ==================== WorkspaceRuleConfig + EdgePulse nested ====================

    #[test]
    fn test_workspace_rule_with_edge_pulse() {
        let full_toml = r#"
[workspace_rule.main]
auto_width = ["100%", "50%"]

[workspace_rule.main.edge_pulse]
enabled = true
width = 20
"#;
        let config: Config = toml::from_str(full_toml).unwrap();
        let rule = config.workspace_rule.get("main").unwrap();
        assert!(rule.edge_pulse.enabled);
        assert_eq!(rule.edge_pulse.width, 20);
    }

    #[test]
    fn test_piri_workspace_rule_with_edge_pulse() {
        let full_toml = r#"
[piri.workspace_rule]
auto_width = ["100%", "50%"]

[piri.workspace_rule.edge_pulse]
enabled = true
width = 20
"#;
        let config: Config = toml::from_str(full_toml).unwrap();
        assert!(config.piri.workspace_rule.edge_pulse.enabled);
        assert_eq!(config.piri.workspace_rule.edge_pulse.width, 20);
    }
}
