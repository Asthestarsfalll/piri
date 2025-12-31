# Plugin System

Piri supports a plugin system that allows you to extend functionality. Plugins run automatically in daemon mode.

## Plugin Control

You can control which plugins are enabled or disabled in the configuration file:

```toml
[piri.plugins]
scratchpads = true
empty = true
window_rule = true
```

**Default Behavior**:
- If not explicitly specified, plugins are **disabled** by default (`false`)
- You must explicitly set `scratchpads = true`, `empty = true`, or `window_rule = true` to enable plugins
- Exception: `window_rule` plugin is enabled by default if window rules are configured (unless explicitly set to `false`)

## Empty Plugin

The Empty plugin executes commands when switching to specific empty workspaces. This is very useful for automating workflows, such as automatically launching applications in empty workspaces.

### Configuration

Use the `[empty.{workspace}]` format in the configuration file to configure workspace rules:

```toml
# Execute command when switching to workspace 1 if it's empty
[empty.1]
command = "notify-send 'Workspace 1 is empty'"

# Execute command when switching to workspace 2 if it's empty
[empty.2]
command = "echo 'Workspace 2 is empty' > /tmp/ws2.log"

# Use workspace name
[empty.main]
command = "firefox"
```

### Workspace Identifiers

The Empty plugin supports two types of workspace identifiers:

1. **name**: Workspace name (string), e.g., `"main"`, `"work"`
2. **idx**: Workspace index number, typically 1-based, e.g., `"1"`, `"2"`, `"3"`

**Matching Order**: The plugin matches in the order of **name first, then idx**. ID (u64) matching is not supported.

The plugin automatically identifies identifier types and supports cross-type matching (e.g., if the current workspace is idx and the config uses name, the plugin will try name matching first, then idx matching).

### How It Works

The Empty plugin uses a **pure event-driven** approach to listen to niri compositor events in real-time:

1. **Event Stream Listening**: The plugin listens to workspace activation events through niri IPC's `EventStream`
2. **Real-time Response**: When a `WorkspaceActivated` event is received (indicating workspace has switched), it immediately queries the current workspace state
3. **Detect Workspace State**: Queries whether the workspace is empty (via the `active_window_id` field)
4. **Execute Command**: If the workspace is empty and matches a configuration rule, immediately executes the command

### Features

- ✅ **Pure Event-Driven**: Uses niri event stream for real-time listening, `read_event()` blocks waiting for events, automatically wakes up when events arrive
- ✅ **Auto Detection**: Automatically detects workspace switches, no manual triggering needed
- ✅ **Flexible Matching**: Supports both name and idx identifier types, matching order is name first, then idx
- ✅ **Auto Reconnect**: Automatically reconnects when connection is lost, ensuring continuous service

### Usage Examples

```toml
# Automatically launch terminal in empty workspace 1
[empty.1]
command = "alacritty"

# Automatically launch browser in empty workspace 2
[empty.2]
command = "firefox"

# Automatically launch editor in empty workspace 3
[empty.3]
command = "code"

# Use name identifier
[empty.work]
command = "emacs"
```

## Scratchpads Plugin

The Scratchpads feature is implemented through the plugin system. For detailed documentation, please refer to the [Scratchpads documentation](scratchpads.md).

## Window Rule Plugin

The Window Rule plugin automatically moves windows to specified workspaces based on their `app_id` or `title`. This is very useful for automating window management.

### Configuration

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

### Features

- ✅ **Pure Event-Driven**: Uses niri event stream for real-time listening, automatically handles windows when created
- ✅ **Regular Expression Support**: Supports powerful regular expression pattern matching
- ✅ **Flexible Matching**: Supports matching by `app_id` or `title`, or both combined (OR logic)
- ✅ **Workspace Matching**: Supports both name and idx identifier types, matching order is name first, then idx

For detailed documentation, please refer to the [Window Rule documentation](window_rule.md).

