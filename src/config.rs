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
    #[serde(flatten)]
    pub singleton: HashMap<String, SingletonConfig>,
    #[serde(default)]
    pub window_rule: Vec<WindowRuleConfig>,
    #[serde(flatten)]
    pub window_order: HashMap<String, u32>,
    #[serde(default)]
    pub window_order_config: WindowOrderConfig,
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

        // Parse TOML manually to handle [scratchpads.term] format
        let doc: toml::Table = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        let mut config = Config {
            niri: NiriConfig::default(),
            piri: PiriConfig::default(),
            scratchpads: HashMap::new(),
            empty: HashMap::new(),
            singleton: HashMap::new(),
            window_rule: Vec::new(),
            window_order: HashMap::new(),
            window_order_config: WindowOrderConfig::default(),
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
                        if let Some(window_rule_enabled) = plugins_map.get("window_rule") {
                            if let Some(enabled) = window_rule_enabled.as_bool() {
                                config.piri.plugins.window_rule = Some(enabled);
                            }
                        }
                        if let Some(autofill_enabled) = plugins_map.get("autofill") {
                            if let Some(enabled) = autofill_enabled.as_bool() {
                                config.piri.plugins.autofill = Some(enabled);
                            }
                        }
                        if let Some(singleton_enabled) = plugins_map.get("singleton") {
                            if let Some(enabled) = singleton_enabled.as_bool() {
                                config.piri.plugins.singleton = Some(enabled);
                            }
                        }
                        if let Some(window_order_enabled) = plugins_map.get("window_order") {
                            if let Some(enabled) = window_order_enabled.as_bool() {
                                config.piri.plugins.window_order = Some(enabled);
                            }
                        }
                    }
                }

                // Extract piri.window_order config
                if let Some(piri_window_order_table) = piri_map.get("window_order") {
                    if let Some(piri_window_order_map) = piri_window_order_table.as_table() {
                        if let Some(enable_listener) =
                            piri_window_order_map.get("enable_event_listener")
                        {
                            if let Some(enabled) = enable_listener.as_bool() {
                                config.window_order_config.enable_event_listener = enabled;
                            }
                        }
                        if let Some(default_w) = piri_window_order_map.get("default_weight") {
                            if let Some(weight) = default_w.as_integer() {
                                config.window_order_config.default_weight = weight as u32;
                            }
                        }
                        if let Some(workspaces) = piri_window_order_map.get("workspaces") {
                            if let Some(workspaces_array) = workspaces.as_array() {
                                config.window_order_config.workspaces = workspaces_array
                                    .iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect();
                                log::debug!(
                                    "piri.window_order.workspaces = {:?}",
                                    config.window_order_config.workspaces
                                );
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

        // Extract singleton configs (format: [singleton.{name}])
        // In TOML, [singleton.browser] creates a nested structure: { "singleton": { "browser": { ... } } }
        if let Some(singleton_table) = doc.get("singleton") {
            if let Some(singleton_map) = singleton_table.as_table() {
                for (name, value) in singleton_map.iter() {
                    if let Some(singleton_table) = value.as_table() {
                        let mut singleton_config = SingletonConfig {
                            command: String::new(),
                            app_id: None,
                        };

                        if let Some(command_value) = singleton_table.get("command") {
                            if let Some(cmd_str) = command_value.as_str() {
                                singleton_config.command = cmd_str.to_string();
                            } else {
                                warn!("singleton.{}: command must be a string", name);
                                continue;
                            }
                        } else {
                            warn!("singleton.{}: missing required field 'command'", name);
                            continue;
                        }

                        if let Some(app_id_value) = singleton_table.get("app_id") {
                            if let Some(app_id_str) = app_id_value.as_str() {
                                singleton_config.app_id = Some(app_id_str.to_string());
                            } else {
                                warn!("singleton.{}: app_id must be a string", name);
                            }
                        }

                        config.singleton.insert(name.clone(), singleton_config);
                        log::debug!(
                            "Parsed singleton config: {} -> command: {}, app_id: {:?}",
                            name,
                            config.singleton[&name.clone()].command,
                            config.singleton[&name.clone()].app_id
                        );
                    }
                }
                log::info!("Parsed {} singleton configurations", config.singleton.len());
            }
        }

        // Extract window_rule config (format: [[window_rule]])
        // In TOML, [[window_rule]] creates an array of tables
        if let Some(window_rule_array) = doc.get("window_rule") {
            if let Some(window_rule_items) = window_rule_array.as_array() {
                for item in window_rule_items {
                    if let Some(rule_table) = item.as_table() {
                        let mut rule = WindowRuleConfig {
                            app_id: None,
                            title: None,
                            open_on_workspace: None,
                            focus_command: None,
                        };

                        if let Some(app_id_value) = rule_table.get("app_id") {
                            if let Some(app_id_str) = app_id_value.as_str() {
                                rule.app_id = Some(app_id_str.to_string());
                            }
                        }

                        if let Some(title_value) = rule_table.get("title") {
                            if let Some(title_str) = title_value.as_str() {
                                rule.title = Some(title_str.to_string());
                            }
                        }

                        if let Some(workspace_value) = rule_table.get("open_on_workspace") {
                            if let Some(workspace_str) = workspace_value.as_str() {
                                rule.open_on_workspace = Some(workspace_str.to_string());
                            } else {
                                warn!("window_rule: open_on_workspace must be a string");
                                continue;
                            }
                        }

                        if let Some(focus_command_value) = rule_table.get("focus_command") {
                            if let Some(focus_command_str) = focus_command_value.as_str() {
                                rule.focus_command = Some(focus_command_str.to_string());
                            }
                        }

                        // At least one of app_id or title must be specified
                        if rule.app_id.is_none() && rule.title.is_none() {
                            warn!("window_rule: at least one of 'app_id' or 'title' must be specified");
                            continue;
                        }

                        // At least one of open_on_workspace or focus_command must be specified
                        if rule.open_on_workspace.is_none() && rule.focus_command.is_none() {
                            warn!("window_rule: at least one of 'open_on_workspace' or 'focus_command' must be specified");
                            continue;
                        }

                        let app_id_clone = rule.app_id.clone();
                        let title_clone = rule.title.clone();
                        let workspace_clone = rule.open_on_workspace.clone();
                        let focus_command_clone = rule.focus_command.clone();
                        config.window_rule.push(rule);
                        log::debug!(
                            "Parsed window rule: app_id={:?}, title={:?}, workspace={:?}, focus_command={:?}",
                            app_id_clone,
                            title_clone,
                            workspace_clone,
                            focus_command_clone
                        );
                    }
                }
                log::info!(
                    "Parsed {} window rule configurations",
                    config.window_rule.len()
                );
            }
        }

        // Extract window_order config (format: [window_order])
        // In TOML, [window_order] creates a table: { "window_order": { "app_id": value, ... } }
        // Special keys: enable_event_listener (bool), default_weight (integer)
        if let Some(window_order_table) = doc.get("window_order") {
            if let Some(window_order_map) = window_order_table.as_table() {
                for (key, value) in window_order_map.iter() {
                    // Handle special configuration keys
                    if key == "enable_event_listener" {
                        if let Some(enabled) = value.as_bool() {
                            config.window_order_config.enable_event_listener = enabled;
                            log::debug!("window_order.enable_event_listener = {}", enabled);
                        } else {
                            warn!("window_order.enable_event_listener must be a boolean");
                        }
                        continue;
                    }
                    if key == "default_weight" {
                        if let Some(weight) = value.as_integer() {
                            config.window_order_config.default_weight = weight as u32;
                            log::debug!("window_order.default_weight = {}", weight);
                        } else {
                            warn!("window_order.default_weight must be an integer");
                        }
                        continue;
                    }
                    if key == "workspaces" {
                        if let Some(workspaces_array) = value.as_array() {
                            config.window_order_config.workspaces = workspaces_array
                                .iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect();
                            log::debug!(
                                "window_order.workspaces = {:?}",
                                config.window_order_config.workspaces
                            );
                        } else {
                            warn!("window_order.workspaces must be an array of strings");
                        }
                        continue;
                    }
                    // Handle app_id -> weight mappings
                    if let Some(order_value) = value.as_integer() {
                        config.window_order.insert(key.clone(), order_value as u32);
                        log::debug!("Parsed window_order config: {} -> {}", key, order_value);
                    } else {
                        warn!("window_order.{}: value must be an integer", key);
                    }
                }
                log::info!(
                    "Parsed {} window_order configurations (event_listener: {}, default_weight: {}, workspaces: {:?})",
                    config.window_order.len(),
                    config.window_order_config.enable_event_listener,
                    config.window_order_config.default_weight,
                    if config.window_order_config.workspaces.is_empty() {
                        "all".to_string()
                    } else {
                        format!("{:?}", config.window_order_config.workspaces)
                    }
                );
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
            singleton: HashMap::new(),
            window_rule: Vec::new(),
            window_order: HashMap::new(),
            window_order_config: WindowOrderConfig::default(),
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

    /// Get window rule plugin config
    pub fn get_window_rule_plugin_config(&self) -> Option<WindowRulePluginConfig> {
        if !self.window_rule.is_empty() {
            return Some(WindowRulePluginConfig {
                rules: self.window_rule.clone(),
            });
        }
        None
    }

    /// Check if window_rule plugin should be enabled
    pub fn is_window_rule_enabled(&self) -> bool {
        // If explicitly set, use that value
        // Otherwise, default to true if rules are configured
        self.piri.plugins.window_rule.unwrap_or(!self.window_rule.is_empty())
    }

    /// Check if autofill plugin should be enabled
    pub fn is_autofill_enabled(&self) -> bool {
        // If explicitly set, use that value
        // Otherwise, default to false (disabled)
        self.piri.plugins.autofill.unwrap_or(false)
    }

    /// Get singleton config by name
    pub fn get_singleton(&self, name: &str) -> Option<&SingletonConfig> {
        self.singleton.get(name)
    }

    /// Check if singleton plugin should be enabled
    pub fn is_singleton_enabled(&self) -> bool {
        // If explicitly set, use that value
        // Otherwise, default to true if singleton configs are configured
        self.piri.plugins.singleton.unwrap_or(!self.singleton.is_empty())
    }

    /// Get window order value for an app_id
    /// Returns configured value if exists, otherwise returns default_weight from config
    /// Supports partial matching: if exact match fails, tries to match if config key is contained in app_id
    pub fn get_window_order(&self, app_id: &str) -> u32 {
        // First try exact match
        if let Some(&order) = self.window_order.get(app_id) {
            return order;
        }

        // Then try partial match: check if any config key is contained in app_id
        // or if app_id is contained in any config key
        // This allows "ghostty" to match "com.mitchellh.ghostty"
        for (config_key, &order) in &self.window_order {
            // Check if config key is contained in app_id (e.g., "ghostty" in "com.mitchellh.ghostty")
            if app_id.contains(config_key) {
                log::debug!(
                    "Matched window_order: app_id '{}' contains config key '{}' -> order {}",
                    app_id,
                    config_key,
                    order
                );
                return order;
            }
            // Also check if app_id is contained in config key (e.g., "google-chrome" in "google-chrome-stable")
            if config_key.contains(app_id) {
                log::debug!(
                    "Matched window_order: config key '{}' contains app_id '{}' -> order {}",
                    config_key,
                    app_id,
                    order
                );
                return order;
            }
        }

        // No match found, return default weight
        self.window_order_config.default_weight
    }

    /// Check if window_order plugin should be enabled
    pub fn is_window_order_enabled(&self) -> bool {
        // If explicitly set, use that value
        // Otherwise, default to true if window_order configs are configured
        self.piri.plugins.window_order.unwrap_or(!self.window_order.is_empty())
    }

    /// Check if window_order event listener is enabled
    pub fn is_window_order_event_listener_enabled(&self) -> bool {
        self.window_order_config.enable_event_listener
    }
}

/// Window order plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowOrderConfig {
    /// Enable/disable event listener for automatic reordering
    /// When enabled, windows will be automatically reordered on WindowLayoutsChanged and WindowClosed events
    #[serde(default = "default_enable_event_listener")]
    pub enable_event_listener: bool,
    /// Default weight for windows not specified in window_order config
    /// Windows with higher weight values will be positioned more to the left
    #[serde(default = "default_window_order_weight")]
    pub default_weight: u32,
    /// Workspaces where window ordering should be applied
    /// If empty or not specified, applies to all workspaces
    /// Can be workspace names or indices (as strings)
    #[serde(default)]
    pub workspaces: Vec<String>,
}

fn default_enable_event_listener() -> bool {
    false // Default: event listener disabled
}

fn default_window_order_weight() -> u32 {
    0 // Default: unconfigured windows have weight 0 (rightmost)
}

impl Default for WindowOrderConfig {
    fn default() -> Self {
        Self {
            enable_event_listener: default_enable_event_listener(),
            default_weight: default_window_order_weight(),
            workspaces: Vec::new(),
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
