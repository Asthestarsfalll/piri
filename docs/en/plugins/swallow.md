# Swallow Plugin

The Swallow plugin automatically hides parent windows when child windows are spawned. This is useful for scenarios like terminals spawning image viewers or media players, where you want the child window to replace the parent window in the layout.

## How It Works

When a child window is opened:

1. **Child Window Matching**: The plugin checks if the new window matches any rule's child window criteria
2. **Parent Window Discovery**: It finds the parent window using two methods (in priority order):
   - **PID-based matching** (default): Traces the process tree to find if the child process was spawned from a parent process
   - **Rule-based matching**: Matches parent windows by `app_id`, `title`, or `pid` patterns
3. **Swallow Operation**: If a matching parent is found, the child window is "swallowed" into the parent's column position, effectively replacing it

## Configuration

Use the `[[swallow]]` format to configure rules (each rule is a separate configuration block), and use `[piri.swallow]` to configure plugin global settings:

```toml
[piri.plugins]
swallow = true

# Plugin global configuration
[piri.swallow]
# Enable PID-based parent-child process matching (default: true)
use_pid_matching = true

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

## Examples

### PID-based Matching Example

![Swallow - PID-based Matching](./assets/swallow_pid.mp4)

Using the default PID matching (`use_pid_matching = true`), the plugin automatically traces the process tree to find parent-child relationships.

```toml
[piri.swallow]
use_pid_matching = true

[[swallow]]
parent_app_id = [".*ghostty.*"]
child_app_id = [".*mpv.*"]
```

### Rule-based Matching Example

![Swallow - Rule-based Matching](./assets/swallow_rule.mp4)

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

### Complex Example: Multiple Patterns

```toml
[[swallow]]
parent_app_id = ["ghostty", "alacritty", "foot", "kitty"]
child_app_id = ["mpv", "imv", "feh", "sxiv"]
```

## Default Behavior

- If no rules are specified, the plugin is enabled but won't match any windows
- `use_pid_matching` defaults to `true` if not specified
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
2. Ensures child window is not floating (converts to tiling if needed)
3. Moves child window to parent's workspace (if different)
4. Executes `ConsumeOrExpelWindowLeft` action to swallow the child into parent's column
5. Focuses the child window

All operations are performed in a single batch for better performance and atomicity.

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

