# Window Rule Plugin

The Window Rule plugin automatically moves windows to specified workspaces based on their `app_id` or `title`. This is very useful for automating window management, such as automatically assigning specific applications to specific workspaces.

> **Reference**: This functionality is similar to [Hyprland's window rules](https://wiki.hypr.land/Configuring/Window-Rules/).

## Configuration

Use the `[[window_rule]]` format in the configuration file to configure window rules:

```toml
# Match by app_id
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# Match by title
[[window_rule]]
title = "^kitty"
open_on_workspace = "browser"

# Specify both app_id and title (either match works)
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"
```

## Configuration Fields

Each window rule contains the following fields:

- **`app_id`** (optional): Regular expression pattern to match window `app_id`
- **`title`** (optional): Regular expression pattern to match window title
- **`open_on_workspace`** (required): Target workspace identifier (name or index)

**Note**: At least one of `app_id` or `title` must be specified. If both are specified, either match works (OR logic).

## Workspace Identifiers

The Window Rule plugin supports two types of workspace identifiers:

1. **name**: Workspace name (string), e.g., `"main"`, `"work"`, `"browser"`
2. **idx**: Workspace index number, typically 1-based, e.g., `"1"`, `"2"`, `"3"`

**Matching Order**: The plugin matches in the order of **name first, then idx**. This is consistent with the Empty plugin.

The plugin automatically identifies identifier types and supports cross-type matching (e.g., if the config uses name, the plugin will try name matching first, then idx matching).

## How It Works

The Window Rule plugin uses a **pure event-driven** approach to listen to niri compositor events in real-time:

1. **Event Stream Listening**: The plugin listens to window events through niri IPC's `EventStream`
2. **Real-time Response**: When a `WindowOpenedOrChanged` event is received (indicating a window has been created or changed), it immediately checks the window information
3. **Regex Matching**: Uses configured regular expression patterns to match window `app_id` or `title`
4. **Auto Move**: If the window matches a rule, automatically moves the window to the specified workspace

## Features

- ✅ **Pure Event-Driven**: Uses niri event stream for real-time listening, automatically handles windows when created
- ✅ **Regular Expression Support**: Supports powerful regular expression pattern matching
- ✅ **Flexible Matching**: Supports matching by `app_id` or `title`, or both combined (OR logic)
- ✅ **Regex Caching**: Compiled regular expressions are cached for better performance
- ✅ **Workspace Matching**: Supports both name and idx identifier types, matching order is name first, then idx
- ✅ **Auto Reconnect**: Automatically reconnects when connection is lost, ensuring continuous service
- ✅ **Hot Config Reload**: Supports configuration updates without restarting the daemon

## Matching Logic

1. **Rule Order**: Rules are checked in the order they appear in the configuration file, **the first matching rule is applied**
2. **Matching Conditions**:
   - If only `app_id` is specified, only `app_id` is matched
   - If only `title` is specified, only `title` is matched
   - If both are specified, either match works (OR logic)
3. **Regular Expressions**: Uses Rust's `regex` crate, supports full regular expression syntax

## Usage Examples

```toml
# Move all Firefox windows to workspace 2
[[window_rule]]
app_id = ".*firefox.*"
open_on_workspace = "2"

# Move all Chrome windows to workspace 3
[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "3"

# Move VS Code or Code windows to development workspace
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# Move specific terminal to workspace 1
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# Move windows with title containing "kitty" to browser workspace
[[window_rule]]
title = "^kitty"
open_on_workspace = "browser"
```

## Regular Expression Examples

```toml
# Match app_id starting with "firefox"
[[window_rule]]
app_id = "^firefox"
open_on_workspace = "2"

# Match title containing "chrome" (case-insensitive requires special handling)
[[window_rule]]
title = ".*[Cc]hrome.*"
open_on_workspace = "3"

# Match exact app_id
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev"

# Match multiple possible app_ids (using | operator)
[[window_rule]]
app_id = "^(code|vscode|Code)$"
open_on_workspace = "dev"
```

## Notes

1. **Rule Order Matters**: The first matching rule is applied, subsequent rules are not checked
2. **Regex Performance**: Complex regular expressions may impact performance, recommend using simple and clear patterns
3. **Window Creation Timing**: The plugin processes windows immediately when created or changed, ensuring windows are correctly moved to target workspace
4. **Non-existent Workspace**: If the specified workspace doesn't exist, a warning is logged but no error is raised
