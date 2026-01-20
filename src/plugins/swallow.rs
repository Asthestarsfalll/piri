use anyhow::Result;
use async_trait::async_trait;
use log::{debug, info, warn};
use niri_ipc::Event;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{deserialize_string_or_vec, Config};
use crate::niri::NiriIpc;
use crate::plugins::window_utils::{
    get_focused_window, matches_window, perform_swallow, try_pid_matching, WindowMatcherCache,
};
use crate::plugins::FromConfig;
use crate::utils::send_notification;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwallowExclude {
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub app_id: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub title: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwallowRule {
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub parent_app_id: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub parent_title: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub child_app_id: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub child_title: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwallowPluginConfig {
    pub rules: Vec<SwallowRule>,
    #[serde(default = "default_true")]
    pub use_pid_matching: bool,
    #[serde(default)]
    pub exclude: Option<SwallowExclude>,
}

fn default_true() -> bool {
    true
}

impl Default for SwallowPluginConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            use_pid_matching: true,
            exclude: None,
        }
    }
}

impl FromConfig for SwallowPluginConfig {
    fn from_config(config: &Config) -> Option<Self> {
        // Only read from top-level [[swallow]] array
        Some(Self {
            rules: config.swallow.clone(),
            use_pid_matching: config.piri.swallow.use_pid_matching,
            exclude: config.piri.swallow.exclude.clone(),
        })
    }
}

pub struct SwallowPlugin {
    niri: NiriIpc,
    config: SwallowPluginConfig,
    matcher_cache: Arc<WindowMatcherCache>,
    window_pid_map: Arc<Mutex<HashMap<u32, Vec<u64>>>>,
    focused_window_queue: VecDeque<u64>,
}

impl SwallowPlugin {
    fn new(niri: NiriIpc, config: SwallowPluginConfig) -> Self {
        info!(
            "Swallow plugin initialized with {} rules",
            config.rules.len()
        );
        let window_pid_map = Arc::new(Mutex::new(HashMap::new()));
        let window_pid_map_clone = window_pid_map.clone();
        let niri_clone = niri.clone();

        // Perform initial scan in background task on plugin startup
        tokio::spawn(async move {
            info!("Performing initial scan for swallow plugin on startup");
            if let Err(e) = Self::perform_initial_scan(niri_clone, window_pid_map_clone).await {
                warn!("Failed to perform initial scan for swallow plugin: {}", e);
            } else {
                debug!("Initial scan completed for swallow plugin");
            }
        });

        Self {
            niri,
            config,
            matcher_cache: Arc::new(WindowMatcherCache::new()),
            window_pid_map,
            focused_window_queue: VecDeque::with_capacity(5),
        }
    }

    async fn perform_initial_scan(
        niri: NiriIpc,
        window_pid_map: Arc<Mutex<HashMap<u32, Vec<u64>>>>,
    ) -> Result<()> {
        debug!("Performing initial window scan for swallow plugin");
        let windows = niri.get_windows().await?;
        let mut map = window_pid_map.lock().await;
        for window in windows {
            match window.pid {
                Some(pid) => {
                    map.entry(pid).or_insert_with(Vec::new).push(window.id);
                }
                None => {
                    warn!("No PID found for window {}", window.id);
                    send_notification("piri", &format!("No PID found for window {}", window.id));
                }
            }
        }
        Ok(())
    }

    /// Check if a window matches the exclude rule
    async fn check_window_matches_exclude(
        &self,
        window: &crate::niri::Window,
        exclude: &SwallowExclude,
    ) -> Result<bool> {
        // If no conditions specified, exclude nothing
        if exclude.app_id.is_none() && exclude.title.is_none() {
            return Ok(false);
        }

        // Check if window matches exclude app_id and title
        matches_window(
            window,
            exclude.app_id.as_ref(),
            exclude.title.as_ref(),
            None,
            None,
            &self.matcher_cache,
        )
        .await
    }

    /// Check if a child window matches a rule's child window conditions
    async fn check_child_window_matches_rule(
        &self,
        child_window: &crate::niri::Window,
        window_id: u64,
        rule: &SwallowRule,
    ) -> Result<bool> {
        debug!(
            "Checking if child window {} (app_id={:?}, title={}) matches rule child criteria",
            window_id, child_window.app_id, child_window.title
        );

        // Check if rule has child matching conditions
        let has_child_conditions = rule.child_app_id.is_some() || rule.child_title.is_some();

        debug!(
            "Rule child conditions: app_id={:?}, title={:?}, has_conditions={}",
            rule.child_app_id, rule.child_title, has_child_conditions
        );

        if !has_child_conditions {
            // If no child conditions specified, match all
            debug!("No child conditions specified, matching all windows");
            return Ok(true); // No conditions means match all
        }

        // Check if child window matches rule (app_id and title)
        debug!(
            "Checking child window against rule patterns: app_id={:?}, title={:?}",
            rule.child_app_id, rule.child_title
        );
        let matches_window_criteria = matches_window(
            child_window,
            rule.child_app_id.as_ref(),
            rule.child_title.as_ref(),
            None,
            None,
            &self.matcher_cache,
        )
        .await?;

        if !matches_window_criteria {
            return Ok(false);
        }
        debug!("Child window matches window criteria (app_id/title)");

        info!(
            "Child window {} (app_id={:?}, title={}) matches rule child criteria",
            window_id, child_window.app_id, child_window.title
        );
        Ok(true)
    }

    /// Check if the currently focused window matches the parent window rule
    /// If focused window is the child window, use the last focused window instead
    async fn check_focused_window_matches_parent_rule(
        &self,
        rule: &SwallowRule,
        child_window_id: u64,
    ) -> Result<Option<crate::niri::Window>> {
        // Get currently focused window
        info!("Checking focused window for parent rule matching...");
        let focused_window = match get_focused_window(&self.niri).await {
            Ok(window) => {
                debug!(
                    "Current focused window: id={}, app_id={:?}, title={}, pid={:?}",
                    window.id, window.app_id, window.title, window.pid
                );
                window
            }
            Err(e) => {
                warn!("No focused window found: {}", e);
                return Ok(None);
            }
        };

        // Check if rule has parent matching conditions
        let has_rule_conditions = rule.parent_app_id.is_some() || rule.parent_title.is_some();

        // If focused window is the child window, search queue for a matching parent window
        if focused_window.id == child_window_id {
            debug!(
                "Focused window {} is the child window, searching queue for matching parent (queue length: {})",
                child_window_id, self.focused_window_queue.len()
            );
            // Search queue from newest to oldest, find first window that matches parent rule
            let windows = self.niri.get_windows().await?;
            for &prev_focused_id in self.focused_window_queue.iter().rev() {
                // Skip child window itself
                if prev_focused_id == child_window_id {
                    continue;
                }

                // Get the window from all windows
                let Some(prev_window) = windows.iter().find(|w| w.id == prev_focused_id) else {
                    continue;
                };
                let prev_window = prev_window.clone();

                // If no parent conditions, match any non-child window
                if !has_rule_conditions {
                    info!(
                        "Found previous focused window (no rule conditions): id={}, app_id={:?}, title={}, pid={:?}",
                        prev_window.id, prev_window.app_id, prev_window.title, prev_window.pid
                    );
                    return Ok(Some(prev_window));
                }

                // Check if this window matches parent criteria
                let matches_window_criteria = matches_window(
                    &prev_window,
                    rule.parent_app_id.as_ref(),
                    rule.parent_title.as_ref(),
                    None,
                    None,
                    &self.matcher_cache,
                )
                .await?;

                if !matches_window_criteria {
                    debug!(
                        "Previous focused window {} (app_id={:?}, title={}) does not match parent criteria, trying next",
                        prev_window.id, prev_window.app_id, prev_window.title
                    );
                    continue;
                }

                // Found matching parent window
                info!(
                    "Found matching previous focused window: id={}, app_id={:?}, title={}, pid={:?}",
                    prev_window.id, prev_window.app_id, prev_window.title, prev_window.pid
                );
                return Ok(Some(prev_window));
            }

            // No matching parent found in queue
            warn!(
                "Focused window {} is the child window but no matching parent window found in queue (checked {} windows)",
                child_window_id, self.focused_window_queue.len()
            );
            return Ok(None);
        }

        // Current focused window is not child window, check if it matches parent rule
        if !has_rule_conditions {
            // If no parent conditions, match any focused window
            return Ok(Some(focused_window));
        }

        // Check if focused window matches parent criteria
        debug!(
            "Checking if focused window {} matches parent criteria (app_id={:?}, title={:?})",
            focused_window.id, rule.parent_app_id, rule.parent_title
        );
        let matches_window_criteria = matches_window(
            &focused_window,
            rule.parent_app_id.as_ref(),
            rule.parent_title.as_ref(),
            None,
            None,
            &self.matcher_cache,
        )
        .await?;

        if !matches_window_criteria {
            warn!(
                "Focused window {} (app_id={:?}, title={}) does not match parent window criteria",
                focused_window.id, focused_window.app_id, focused_window.title
            );
            return Ok(None);
        }
        debug!("Focused window matches window criteria (app_id/title)");

        // Found matching focused window
        info!(
            "Focused window {} (app_id={:?}, title={}, pid={:?}) matches parent rule",
            focused_window.id, focused_window.app_id, focused_window.title, focused_window.pid
        );
        Ok(Some(focused_window))
    }

    async fn handle_window_opened(&mut self, window: &niri_ipc::Window) -> Result<()> {
        let window_id = window.id;

        // If ID is already in the map, it's a Changed event, skip it.
        let should_skip = {
            let map = self.window_pid_map.lock().await;
            map.values().any(|window_ids| window_ids.contains(&window_id))
        };
        if should_skip {
            debug!(
                "Window {} already in map, skipping (Changed event)",
                window_id
            );
            return Ok(());
        }

        let child_window = self.niri.convert_window(window).await?;

        match child_window.pid {
            Some(pid) => {
                debug!(
                    "Stored PID {} for window {} (app_id={:?}, title={}) in window_pid_map",
                    pid, window_id, child_window.app_id, child_window.title
                );
                let mut map = self.window_pid_map.lock().await;
                map.entry(pid).or_insert_with(Vec::new).push(window_id);
            }
            None => {
                warn!("No PID found for window {}", window_id);
                send_notification("piri", &format!("No PID found for window {}", window_id));
            }
        }

        // Add new window to focused window queue
        // Remove the window ID from queue if it already exists (to avoid duplicates)
        self.focused_window_queue
            .retain(|&queue_window_id| queue_window_id != window_id);
        // Add to the back (newest)
        self.focused_window_queue.push_back(window_id);
        // Keep queue size at most 5
        while self.focused_window_queue.len() > 5 {
            self.focused_window_queue.pop_front(); // Remove oldest
        }
        debug!(
            "Added new window {} to focus queue: queue_length={}, queue={:?}",
            window_id,
            self.focused_window_queue.len(),
            self.focused_window_queue
        );

        // Check if child window matches exclude rule
        if let Some(ref exclude) = self.config.exclude {
            let matches_exclude = self.check_window_matches_exclude(&child_window, exclude).await?;
            if matches_exclude {
                debug!(
                    "Child window {} (app_id={:?}, title={}) matches exclude rule, skipping swallow",
                    window_id, child_window.app_id, child_window.title
                );
                return Ok(());
            }
        }

        // Priority 1: Try PID matching first (if enabled)
        if self.config.use_pid_matching {
            let windows = self.niri.get_windows().await?;
            if let Some(parent_window) =
                try_pid_matching(&child_window, &windows, self.window_pid_map.clone()).await?
            {
                perform_swallow(&self.niri, &parent_window, &child_window, window_id).await?;
                return Ok(());
            }
            debug!(
                "PID matching failed for child window {} (app_id={:?}, title={}), trying rule matching",
                window_id, child_window.app_id, child_window.title
            );
        }

        // Priority 2: Try rule-based matching (if PID matching failed or disabled)
        debug!(
            "Starting rule-based matching for child window {} (app_id={:?}, title={}), checking {} rules",
            window_id, child_window.app_id, child_window.title, self.config.rules.len()
        );
        for (rule_idx, rule) in self.config.rules.iter().enumerate() {
            debug!(
                "Checking rule {}: child_app_id={:?}, child_title={:?}, parent_app_id={:?}, parent_title={:?}",
                rule_idx, rule.child_app_id, rule.child_title, rule.parent_app_id, rule.parent_title
            );
            // Check if child window matches rule
            if !self.check_child_window_matches_rule(&child_window, window_id, rule).await? {
                debug!(
                    "Child window {} does not match rule {} criteria, skipping",
                    window_id, rule_idx
                );
                continue;
            }

            // If child window matches this rule, check if focused window matches parent rule
            debug!(
                "Child window {} (app_id={:?}, title={}) matches rule {} child criteria, checking if focused window matches parent rule",
                window_id, child_window.app_id, child_window.title, rule_idx
            );

            match self.check_focused_window_matches_parent_rule(rule, window_id).await? {
                Some(parent_window) => {
                    debug!(
                        "Found matching parent window {} for rule {}, performing swallow",
                        parent_window.id, rule_idx
                    );
                    perform_swallow(&self.niri, &parent_window, &child_window, window_id).await?;
                    return Ok(()); // Only apply first matching rule
                }
                None => {
                    warn!(
                        "Rule {} matched child window but focused window does not match parent rule, trying next rule",
                        rule_idx
                    );
                }
            }
        }

        info!(
            "No matching parent window found for child window {} (app_id={:?}, title={})",
            window_id, child_window.app_id, child_window.title
        );

        Ok(())
    }
}

#[async_trait]
impl crate::plugins::Plugin for SwallowPlugin {
    type Config = SwallowPluginConfig;

    fn new(niri: NiriIpc, config: SwallowPluginConfig) -> Self {
        Self::new(niri, config)
    }

    async fn update_config(&mut self, config: SwallowPluginConfig) -> Result<()> {
        info!(
            "Updating swallow plugin configuration: {} rules",
            config.rules.len()
        );
        self.config = config;
        Ok(())
    }

    fn is_interested_in_event(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::WindowOpenedOrChanged { .. }
                | Event::WindowClosed { .. }
                | Event::WindowFocusTimestampChanged { .. }
        )
    }

    async fn handle_event(&mut self, event: &Event, _niri: &NiriIpc) -> Result<()> {
        match event {
            Event::WindowOpenedOrChanged { window } => {
                self.handle_window_opened(window).await?;
            }
            Event::WindowClosed { id } => {
                // Remove window id from all pid entries
                {
                    let mut map = self.window_pid_map.lock().await;
                    map.values_mut().for_each(|window_ids| {
                        window_ids.retain(|&window_id| window_id != *id);
                    });
                    // Remove empty pid entries
                    map.retain(|_, window_ids| !window_ids.is_empty());
                }

                // Remove window id from focused window queue
                self.focused_window_queue.retain(|&window_id| window_id != *id);
            }
            Event::WindowFocusTimestampChanged { id, .. } => {
                // Add new focused window to queue
                // Remove the window ID from queue if it already exists (to avoid duplicates)
                self.focused_window_queue.retain(|&window_id| window_id != *id);
                // Add to the back (newest)
                self.focused_window_queue.push_back(*id);
                // Keep queue size at most 5
                while self.focused_window_queue.len() > 5 {
                    self.focused_window_queue.pop_front(); // Remove oldest
                }
                debug!(
                    "Window focus timestamp changed: new_focused_id={}, queue_length={}, queue={:?}",
                    id, self.focused_window_queue.len(), self.focused_window_queue
                );
            }
            _ => {}
        }
        Ok(())
    }
}
