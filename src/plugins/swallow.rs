use anyhow::Result;
use async_trait::async_trait;
use log::{debug, info, warn};
use niri_ipc::{Action, ColumnDisplay, Reply, Request};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Maximum difference (in logical pixels) between window sizes still considered equal
/// when determining whether a column is tabbed.
const SWALLOW_SIZE_TOLERANCE: u32 = 2;
/// A column is considered tabbed when all of its windows share a size that spans at
/// least this fraction of the output height.
const TABBED_HEIGHT_RATIO: f64 = 0.6;

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
    /// Re-check swallow rules when a window's title or app_id changes
    /// (e.g. Firefox extension windows that set their real title after opening).
    #[serde(default)]
    pub swallow_on_change: bool,
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
            swallow_on_change: false,
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
            swallow_on_change: config.piri.swallow.swallow_on_change,
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
    /// Windows already swallowed into a parent; they are skipped when their
    /// title/app_id changes so the swallow is not performed twice.
    swallowed_windows: HashSet<u64>,
    /// Last (app_id, title) observed per window, used to detect identity
    /// changes that should trigger a swallow re-check.
    last_checked_state: HashMap<u64, (Option<String>, String)>,
    /// Column display modes to restore once active swallows complete, keyed by
    /// (workspace id, column index).
    pending_column_restores: HashMap<(Option<u64>, usize), PendingColumnRestore>,
}

/// Column display mode to restore after a swallow completes.
struct PendingColumnRestore {
    /// Window that was focused when the swallow happened; used to select the column
    /// when restoring.
    parent_id: u64,
    /// The column display mode observed before the swallow.
    display: ColumnDisplay,
    /// Child windows currently swallowed into this column.
    child_ids: Vec<u64>,
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
            swallowed_windows: HashSet::new(),
            last_checked_state: HashMap::new(),
            pending_column_restores: HashMap::new(),
        }
    }

    async fn perform_initial_scan(
        niri: NiriIpc,
        window_pid_map: Arc<Mutex<HashMap<u32, Vec<u64>>>>,
    ) -> Result<()> {
        debug!("Performing initial window scan for swallow plugin");
        let windows = niri.get_windows_raw().await?;
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
        )?;

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
            let windows = self.niri.get_windows_raw().await?;
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
                )?;

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
        )?;

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

    /// Determine the display mode (Tabbed/Normal) of the column containing `window`.
    ///
    /// niri's IPC does not expose the column display mode directly, so it is inferred
    /// from window geometry: windows in a tabbed column all occupy the same full-height
    /// tile (identical sizes), while windows in a normal column are stacked with
    /// different (or shorter) tiles.
    ///
    /// Returns `None` when the mode cannot be determined reliably, in which case the
    /// caller should not schedule a restore.
    async fn detect_column_display(
        &self,
        window: &crate::niri::Window,
    ) -> Result<Option<ColumnDisplay>> {
        let Some((col_idx, _)) = window.layout.as_ref().and_then(|l| l.pos_in_scrolling_layout)
        else {
            // Not in a tiled column (e.g. floating); nothing to restore.
            return Ok(None);
        };

        let windows = self.niri.get_windows_raw().await?;

        // All tiled windows in the same workspace and column as `window`.
        let column_windows: Vec<&crate::niri::Window> = windows
            .iter()
            .filter(|w| {
                !w.floating
                    && w.workspace_id == window.workspace_id
                    && w.layout
                        .as_ref()
                        .and_then(|l| l.pos_in_scrolling_layout)
                        .map(|(col, _)| col == col_idx)
                        .unwrap_or(false)
            })
            .collect();

        // A column with a single window is always displayed normally by niri.
        if column_windows.len() <= 1 {
            return Ok(Some(ColumnDisplay::Normal));
        }

        // Windows in a tabbed column all have the same size.
        let Some(first_size) = column_windows[0].layout.as_ref().and_then(|l| l.window_size) else {
            return Ok(None);
        };
        let all_same_size = column_windows.iter().all(|w| {
            w.layout.as_ref().and_then(|l| l.window_size).is_some_and(|size| {
                size[0].abs_diff(first_size[0]) <= SWALLOW_SIZE_TOLERANCE
                    && size[1].abs_diff(first_size[1]) <= SWALLOW_SIZE_TOLERANCE
            })
        });
        if !all_same_size {
            // Stacked windows in a normal column have different tile sizes.
            return Ok(Some(ColumnDisplay::Normal));
        }

        // All windows share one size: it is a tabbed column only if the shared height
        // spans most of the output height; an equally-split normal column is only about
        // half as tall.
        let Some(output_height) = self.output_height_for_window(window).await? else {
            return Ok(None);
        };
        let common_height = f64::from(first_size[1]);
        Ok(Some(
            if common_height >= output_height * TABBED_HEIGHT_RATIO {
                ColumnDisplay::Tabbed
            } else {
                ColumnDisplay::Normal
            },
        ))
    }

    /// Resolve the logical height of the output the given window's workspace is on.
    async fn output_height_for_window(&self, window: &crate::niri::Window) -> Result<Option<f64>> {
        let Some(workspace_id) = window.workspace_id else {
            return Ok(None);
        };
        let workspaces = self.niri.get_workspaces_for_mapping().await?;
        let Some(workspace) = workspaces.iter().find(|ws| ws.id == workspace_id) else {
            return Ok(None);
        };
        let Some(output_name) = workspace.output.as_deref() else {
            return Ok(None);
        };
        Ok(self
            .niri
            .get_output_size_by_name(output_name)
            .map(|(_, height)| f64::from(height)))
    }

    /// Remember the parent column's display mode so it can be restored once the
    /// swallowed child window closes. The display mode is only saved on the first
    /// swallow into a column; later swallows into the same column keep the original
    /// value (which was observed before any swallow).
    async fn record_column_display_for_swallow(
        &mut self,
        parent_window: &crate::niri::Window,
        child_window_id: u64,
    ) -> Result<()> {
        let Some((col_idx, _)) =
            parent_window.layout.as_ref().and_then(|l| l.pos_in_scrolling_layout)
        else {
            return Ok(());
        };

        let Some(display) = self.detect_column_display(parent_window).await? else {
            return Ok(());
        };

        debug!(
            "Saving column display {:?} before swallowing window {} into parent {}",
            display, child_window_id, parent_window.id
        );

        let key = (parent_window.workspace_id, col_idx);
        self.pending_column_restores
            .entry(key)
            .or_insert_with(|| PendingColumnRestore {
                parent_id: parent_window.id,
                display,
                child_ids: Vec::new(),
            })
            .child_ids
            .push(child_window_id);

        Ok(())
    }

    /// Restore the column display mode saved before a swallow once the swallowed child
    /// window closes. The restore is delayed until the last swallowed child of the
    /// column has closed, so windows that are still swallowed stay hidden as tabs.
    async fn restore_column_display_on_window_closed(&mut self, window_id: u64) -> Result<()> {
        let Some(key) = self
            .pending_column_restores
            .iter()
            .find_map(|(key, entry)| entry.child_ids.contains(&window_id).then_some(*key))
        else {
            return Ok(());
        };

        let should_restore = {
            let entry = self.pending_column_restores.get_mut(&key).unwrap();
            entry.child_ids.retain(|&id| id != window_id);
            entry.child_ids.is_empty()
        };
        if !should_restore {
            // Other swallowed children are still open in this column.
            return Ok(());
        }

        let PendingColumnRestore {
            parent_id, display, ..
        } = self.pending_column_restores.remove(&key).unwrap();

        // The parent window selects the column; if it is gone there is nothing to restore.
        let windows = self.niri.get_windows_raw().await?;
        if !windows.iter().any(|w| w.id == parent_id) {
            debug!(
                "Parent window {} for column display restore is gone, skipping",
                parent_id
            );
            return Ok(());
        }

        info!(
            "Restoring column display to {:?} after swallowed window {} closed",
            display, window_id
        );

        self.niri
            .execute_batch(move |socket| {
                // Focus the parent window to select its column, then restore the display.
                match socket.send(Request::Action(Action::FocusWindow { id: parent_id }))? {
                    Reply::Ok(_) => {}
                    Reply::Err(err) => anyhow::bail!("Failed to focus parent window: {}", err),
                }
                let _ = socket.send(Request::Action(Action::SetColumnDisplay { display }))?;
                Ok::<(), anyhow::Error>(())
            })
            .await?;

        Ok(())
    }

    async fn handle_window_opened(&mut self, window: &niri_ipc::Window) -> Result<()> {
        let window_id = window.id;

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

        // Record the window identity so later title/app_id changes can be detected
        // by handle_window_changed.
        self.last_checked_state.insert(
            window_id,
            (child_window.app_id.clone(), child_window.title.clone()),
        );

        if self.try_swallow_window(&child_window).await? {
            // Mark as swallowed so later property changes don't re-swallow it.
            self.swallowed_windows.insert(window_id);
        }

        Ok(())
    }

    /// Check whether the window matches any rule's child criteria.
    ///
    /// Used to guard PID matching: when rules exist, PID matching should only
    /// swallow windows that match at least one rule's child conditions, so
    /// windows opening with a generic title (e.g. Firefox extension popups
    /// showing "Mozilla Firefox" before updating to "Extension: ...") are not
    /// swallowed before their real identity is known.
    async fn window_matches_any_rule_child(
        &self,
        child_window: &crate::niri::Window,
    ) -> Result<bool> {
        for rule in &self.config.rules {
            if self
                .check_child_window_matches_rule(child_window, child_window.id, rule)
                .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Try to swallow the given child window into a matching parent window.
    ///
    /// Returns `true` if the window was swallowed. Used both when a window first
    /// opens and when its title/app_id changes afterwards.
    async fn try_swallow_window(&mut self, child_window: &crate::niri::Window) -> Result<bool> {
        let window_id = child_window.id;

        // Check if child window matches exclude rule
        if let Some(ref exclude) = self.config.exclude {
            let matches_exclude = self.check_window_matches_exclude(child_window, exclude).await?;
            if matches_exclude {
                debug!(
                    "Child window {} (app_id={:?}, title={}) matches exclude rule, skipping swallow",
                    window_id, child_window.app_id, child_window.title
                );
                return Ok(false);
            }
        }

        // Priority 1: Try PID matching first (if enabled).
        // Only swallow via PID matching when the child matches at least one
        // rule's child criteria (or when no rules are configured), so rule
        // conditions are respected even in the PID path.
        if self.config.use_pid_matching
            && (self.config.rules.is_empty()
                || self.window_matches_any_rule_child(child_window).await?)
        {
            let windows = self.niri.get_windows_raw().await?;
            if let Some(parent_window) =
                try_pid_matching(child_window, &windows, self.window_pid_map.clone()).await?
            {
                self.record_column_display_for_swallow(&parent_window, window_id).await?;
                perform_swallow(
                    &self.niri,
                    &parent_window,
                    child_window,
                    window_id,
                    ColumnDisplay::Tabbed,
                )
                .await?;
                return Ok(true);
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
            if !self.check_child_window_matches_rule(child_window, window_id, rule).await? {
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
                    self.record_column_display_for_swallow(&parent_window, window_id).await?;
                    perform_swallow(
                        &self.niri,
                        &parent_window,
                        child_window,
                        window_id,
                        ColumnDisplay::Tabbed,
                    )
                    .await?;
                    return Ok(true); // Only apply first matching rule
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

        Ok(false)
    }

    /// Re-check swallow rules when a window's title or app_id changes.
    ///
    /// Some applications (e.g. Firefox extension popups) open with a generic
    /// title ("Mozilla Firefox") and only set their real title ("Extension: ...")
    /// afterwards, so they do not match rule criteria at open time. With
    /// `swallow_on_change` enabled, the rules are re-evaluated whenever the
    /// window identity changes.
    async fn handle_window_changed(&mut self, window: &niri_ipc::Window) -> Result<()> {
        if !self.config.swallow_on_change {
            return Ok(());
        }

        let window_id = window.id;

        // A swallowed window should stay swallowed; ignore its property changes.
        if self.swallowed_windows.contains(&window_id) {
            return Ok(());
        }

        let child_window = self.niri.convert_window(window).await?;

        // Only act when the identity (app_id/title) actually changed, so unrelated
        // WindowOpenedOrChanged events (layout, workspace, etc.) don't re-check.
        let new_state = (child_window.app_id.clone(), child_window.title.clone());
        if self.last_checked_state.get(&window_id) == Some(&new_state) {
            return Ok(());
        }
        self.last_checked_state.insert(window_id, new_state);

        debug!(
            "Window {} (app_id={:?}, title={}) changed, re-checking swallow rules",
            window_id, child_window.app_id, child_window.title
        );

        if self.try_swallow_window(&child_window).await? {
            self.swallowed_windows.insert(window_id);
        }

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

    fn is_interested_in_event(&self, event: &crate::plugins::PiriEvent) -> bool {
        matches!(
            event,
            crate::plugins::PiriEvent::WindowOpened { .. }
                | crate::plugins::PiriEvent::WindowChanged { .. }
                | crate::plugins::PiriEvent::WindowClosed { .. }
                | crate::plugins::PiriEvent::WindowFocusTimestampChanged { .. }
        )
    }

    async fn handle_event(
        &mut self,
        event: &crate::plugins::PiriEvent,
        _niri: &NiriIpc,
    ) -> Result<()> {
        match event {
            crate::plugins::PiriEvent::WindowOpened { window } => {
                self.handle_window_opened(window).await?;
            }
            crate::plugins::PiriEvent::WindowChanged { window } => {
                self.handle_window_changed(window).await?;
            }
            crate::plugins::PiriEvent::WindowClosed { id } => {
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

                // Forget swallow/change tracking state for this window
                self.swallowed_windows.remove(id);
                self.last_checked_state.remove(id);

                // Restore the column display mode saved before the swallow, now that
                // the swallowed child window has closed.
                self.restore_column_display_on_window_closed(*id).await?;

                // Drop pending restores whose parent window was closed.
                self.pending_column_restores.retain(|_, entry| entry.parent_id != *id);
            }
            crate::plugins::PiriEvent::WindowFocusTimestampChanged { id, .. } => {
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
