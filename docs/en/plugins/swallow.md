# Swallow Plugin

The Swallow plugin automatically hides parent windows when child windows are spawned. This is useful for scenarios like terminals spawning image viewers or media players, where you want the child window to replace the parent window in the layout.

## How It Works

When a child window is opened:

1. **Child Window Matching**: The plugin checks if the new window matches any rule's child window criteria
2. **Parent Window Discovery**: It finds the parent window using two methods (in priority order):
   - **PID-based matching** (default): Traces the process tree to find if the child process was spawned from a parent process
   - **Rule-based matching**: Matches parent windows by `app_id`, `title`, or `pid` patterns
3. **Swallow Operation**: If a matching parent is found, the child window is "swallowed" into the parent's column position, effectively replacing it
4. **Column Display Restore**: Before swallowing, the plugin remembers the parent column's display mode (`tabbed` or `normal`). When the swallowed child window later closes, the column's display mode is restored to what it was before the swallow, so the parent column doesn't permanently switch to tabbed mode

## Configuration

Use the `[[swallow]]` format to configure rules (each rule is a separate configuration block), and use `[piri.swallow]` to configure plugin global settings:

```toml
[piri.plugins]
swallow = true

# Plugin global configuration
[piri.swallow]
# Enable PID-based parent-child process matching (default: true)
use_pid_matching = true
# Re-check swallow rules when a window's title/app_id changes (default: false)
swallow_on_change = false

# Global exclude rule: windows matching these conditions will never be swallowed
[piri.swallow.exclude]
app_id = [".*dialog.*"]
title = [".*error.*"]

# Rules list (each rule is a separate configuration block)
# Example 1: Terminal swallows media players
[[swallow]]
parent_app_id = [".*terminal.*", ".*alacritty.*", ".*foot.*", ".*ghostty.*"]
child_app_id = [".*mpv.*", ".*imv.*", ".*feh.*"]

# Example 2: Editor swallows preview windows
[[swallow]]
parent_app_id = ["code", "nvim-qt"]
child_app_id = [".*preview.*", ".*markdown.*"]
```

### Global Configuration Parameters

The following global parameters can be configured in `[piri.swallow]`:

| Parameter | Type | Description |
| :--- | :--- | :--- |
| `use_pid_matching` | `bool` | Enable PID-based parent-child process matching (default: `true`) |
| `swallow_on_change` | `bool` | Re-check swallow rules when a window's `title` or `app_id` changes (default: `false`) |
| `exclude` | `SwallowExclude` | Global exclude rule, windows matching these conditions will never be swallowed (optional) |

### Rule Configuration Parameters

Each rule supports the following optional parameters:

| Parameter | Type | Description |
| :--- | :--- | :--- |
| `parent_app_id` | `Vec<String>` | Regex patterns to match parent window `app_id` |
| `parent_title` | `Vec<String>` | Regex patterns to match parent window `title` |
| `child_app_id` | `Vec<String>` | Regex patterns to match child window `app_id` |
| `child_title` | `Vec<String>` | Regex patterns to match child window `title` |

### Matching Logic

1. **Global Exclude Check**: First check if the child window matches the global `exclude` rule. If matched, skip immediately without performing any swallow operations.

2. **PID Matching** (when `use_pid_matching = true`, default, highest priority):
   - Traces the child process's process tree to find ancestor processes
   - Matches parent windows whose PID is an ancestor of the child process
   - If parent criteria (`parent_app_id`, `parent_title`) are specified, they are also checked
   - If no parent criteria are specified, any ancestor window will match

3. **Rule-based Matching** (fallback when PID matching fails or is disabled):
   - Matches parent windows using `app_id`, `title`, or `pid` patterns
   - Only used if PID matching fails or `use_pid_matching = false`
   - **Parent Window Discovery Mechanism**:
     - If the currently focused window is not the child window, use the currently focused window as the candidate parent window
     - If the currently focused window is the child window itself, search for a matching parent window from the focus window queue (maintains the last 5 focused windows)
     - The focus window queue is automatically updated when windows gain focus

4. **Exclude Rules**: Exclude patterns take precedence - if a window matches an exclude pattern, it will not be matched even if it matches include patterns

5. **Pattern Lists**: When multiple patterns are provided (e.g., `parent_app_id = ["pattern1", "pattern2"]`), the rule matches if ANY pattern matches (OR logic)

### Niri Configuration Requirements

For a better experience, it is recommended to configure applications that may be replaced by child windows (such as `mpv`, `imv`, `feh`, etc.) using one of the following methods:

> For more detailed information about configuration, please refer to [GitHub Issue #2](https://github.com/Asthestarsfalll/piri/issues/2).

**Method 1: Use window-rule to set floating**

Configure child window applications with `open-floating=true` in the niri configuration:

```kdl
window-rule {
    app-id = "mpv"
    open-floating = true
}

window-rule {
    app-id = "imv"
    open-floating = true
}

window-rule {
    app-id = "feh"
    open-floating = true
}
```

**Method 2: Use workspace_rule functionality**

Enable the piri workspace_rule plugin and configure `auto_fill = true` to automatically handle the layout of these windows.

## Examples

### PID-based Matching Example

https://github.com/user-attachments/assets/51567d89-8ca8-4f4a-b2ca-732dfc0741c9

Using the default PID matching (`use_pid_matching = true`), the plugin automatically traces the process tree to find parent-child relationships.

```toml
[piri.swallow]
use_pid_matching = true

[[swallow]]
parent_app_id = [".*ghostty.*"]
child_app_id = [".*mpv.*"]
```

### Rule-based Matching Example

https://github.com/user-attachments/assets/9968e97f-fdea-4211-a007-717edf703e93

Using `app_id` and `title` patterns to match parent windows.

```toml
[piri.swallow]
use_pid_matching = true

[[swallow]]
child_app_id = '.*google-chrome.*'
parent_app_id = '.*ghostty.*'

[[swallow]]
child_app_id = '.*firefox*.'
parent_app_id = '.*ghostty.*'
```

### Basic Example: Terminal Swallows Media Players

```toml
[[swallow]]
parent_app_id = ["ghostty", "alacritty", "foot"]
child_app_id = ["mpv", "imv", "feh"]
```

When you launch `mpv` or `imv` from a terminal, the terminal window will be hidden and replaced by the media player.


### Global Exclude Example

```toml
[piri.swallow]
# Globally exclude all dialog windows
[piri.swallow.exclude]
app_id = [".*dialog.*", ".*error.*"]

[[swallow]]
parent_app_id = [".*terminal.*"]
child_app_id = [".*mpv.*"]
```

This way all dialog windows will never be swallowed, even if rules match.

### Disable PID Matching

```toml
[piri.swallow]
use_pid_matching = false

[[swallow]]
parent_app_id = [".*terminal.*"]
child_app_id = [".*mpv.*"]
```

This uses rule-based matching only, without checking process relationships.

### Match by Title

```toml
[[swallow]]
parent_title = [".*Terminal.*"]
child_title = [".*Video Player.*"]
```

### Swallow When the Window Title Changes

Some applications open with a generic title and only set their real title afterwards — for example Firefox extension windows (Bitwarden, aria2 downloader, etc.) open as `Mozilla Firefox` and update to `Extension: ...` once fully loaded. Since the rule only matches the final title, the window does not match at open time.

Set `swallow_on_change = true` to re-run the swallow rules whenever a window's `title` or `app_id` changes:

```toml
[piri.swallow]
use_pid_matching = true
swallow_on_change = true

[[swallow]]
child_title = ".*Extension:.*"
parent_title = ".*firefox.*"
```

The window opens as `Mozilla Firefox` (no match, not swallowed), and once its title updates to e.g. `Extension: Bitwarden`, the plugin re-checks the rules and swallows it into the matching Firefox parent. Normal Firefox windows are unaffected.

Notes:
- When rules are configured, PID matching only swallows windows that match at least one rule's child criteria, so the final title is respected instead of the window being swallowed at open time.
- Windows that were already swallowed are skipped on later changes, and unrelated property changes (layout, workspace, etc.) do not trigger re-checks.

### Complex Example: Multiple Patterns

```toml
[[swallow]]
parent_app_id = ["ghostty", "alacritty", "foot", "kitty"]
child_app_id = ["mpv", "imv", "feh", "sxiv"]
```

## Default Behavior

- If no rules are specified, the plugin is enabled but won't match any windows
- `use_pid_matching` defaults to `true` if not specified
- `swallow_on_change` defaults to `false` if not specified
- If `exclude` is not specified, no global exclusion is performed
- If no child conditions are specified, the rule will match any child window and look for parents
- If no parent conditions are specified (with PID matching enabled), any ancestor window will match
- The focus window queue maintains at most the last 5 focused windows, used to find parent windows when child windows are focused

## Technical Details

### Process Tree Tracing

When PID matching is enabled, the plugin:
1. Finds the PID of the child window's process
2. Traces up the process tree (up to PID 1) to find ancestor PIDs
3. Matches windows whose process PID is in the ancestor chain

### Focus Window Queue

The plugin maintains a focus queue of at most 5 windows to track recently focused windows:
- When a window gains focus (`WindowFocusTimestampChanged` event), the window ID is added to the end of the queue
- When a new window opens (`WindowOpenedOrChanged` event), the window ID is also added to the queue
- When a child window opens and the currently focused window is the child window itself, the plugin searches for a matching parent window from the queue (newest to oldest)
- The queue size is limited to 5, removing the oldest window ID when exceeded

### Window Matching

The plugin uses the same window matching mechanism as other plugins. For details, see [Window Matching Mechanism](../window_matching.md).

### IPC Calls

The plugin performs the following operations when swallowing:
1. Focus the parent window
2. Set column display to tabbed (ensures better layout when multiple windows are swallowed)
3. Ensures child window is not floating (converts to tiling if needed)
4. Moves child window to parent's workspace (if different)
5. Executes `ConsumeOrExpelWindowLeft` action to swallow the child into parent's column
6. Focuses the child window

All operations are performed in a single batch for better performance and atomicity.

### Column Display Restore

When a swallowed child window closes, the plugin restores the parent column's display mode to the value it had before the swallow:

1. Focuses the parent window
2. Sets the column display back to the saved mode (`normal` or `tabbed`)

This prevents the parent column from permanently switching to tabbed mode after a swallow completes (which would otherwise affect how new windows open in that column). If multiple child windows were swallowed into the same column, the restore happens only after the last one closes, so windows still swallowed stay hidden as tabs.

Since niri's IPC does not expose the column display mode directly, it is inferred from window geometry: windows in a tabbed column all occupy the same full-height tile (identical sizes), while windows in a normal column are stacked with different (or shorter) tiles. If the mode cannot be determined reliably, the plugin simply leaves the column as is (no restore).

## Use Cases

- **Terminals spawning media players**: Hide terminal when launching `mpv`, `imv`, or `feh`
- **Editors spawning previews**: Hide editor window when preview window opens
- **Applications with launcher windows**: Hide launcher when main application starts
- **Nested application workflows**: Automatically manage parent-child window relationships

## Limitations

- Floating windows cannot be swallowed (will be converted to tiling first)
- Parent and child windows must be in the same workspace (plugin handles this automatically)
- Process tree tracing goes all the way up to PID 1, which may impact performance if the process tree is very deep
- PID matching requires processes to have a parent-child relationship
- The focus window queue maintains at most 5 windows. If the parent window is not among the last 5 focused windows, rule-based matching may not find the parent window
