# Empty Plugin

The Empty plugin automatically executes configured commands when switching to empty workspaces, useful for automating workflows.

> **Reference**: This functionality is similar to [Hyprland's `on-created-empty` workspace rule](https://wiki.hypr.land/Configuring/Workspace-Rules/#rules).

## Configuration

Use the `[empty.{workspace}]` format to configure workspace rules:

```toml
[piri.plugins]
empty = true

# Execute command when switching to workspace 1 if it's empty
[empty.1]
command = "alacritty"

# Use workspace name
[empty.main]
command = "firefox"

# Automatically launch editor in empty workspace
[empty.dev]
command = "code"
```

## Workspace Identifiers

Supports two identifier types:

- **name**: Workspace name, e.g., `"main"`, `"work"`
- **idx**: Workspace index (1-based), e.g., `"1"`, `"2"`

**Matching Order**: Name first, then idx. The plugin automatically identifies types and supports cross-type matching.

## How It Works

The plugin listens for `WorkspaceActivated` events, and when a workspace switch occurs:

1. Checks if the current workspace is empty (via `active_window_id` field)
2. If empty and matches a configuration rule, immediately executes the command

## Features

- ✅ **Event-Driven**: Real-time listening for workspace switches
- ✅ **Flexible Matching**: Supports both name and idx identifiers
- ✅ **Auto Detection**: No manual triggering needed

## Use Cases

- Automatically launch frequently used applications (terminal, browser, editor, etc.) in empty workspaces
- Set different default applications for different workspaces
- Automate workflows
