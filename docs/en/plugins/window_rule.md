# Window Rule Plugin

The Window Rule plugin automatically moves windows to specified workspaces based on their `app_id` or `title` using regular expression matching.

## Configuration

Use the `[[window_rule]]` format to configure window rules:

```toml
[piri.plugins]
window_rule = true

# Match by app_id
[[window_rule]]
app_id = "ghostty"
open_on_workspace = "1"

# Match by title
[[window_rule]]
title = ".*Chrome.*"
open_on_workspace = "browser"
focus_command = "notify-send 'Focusing on Chrome'"

# Specify both app_id and title (either match works)
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# Only focus_command, don't move window
[[window_rule]]
title = ".*Chrome.*"
focus_command = "notify-send 'Chrome focused'"

# Regex example: match app_id starting with "firefox"
[[window_rule]]
app_id = "^firefox"
open_on_workspace = "2"

# Match exact app_id
[[window_rule]]
app_id = "^code$"
open_on_workspace = "dev"
```

## Configuration Fields

- **`app_id`** (optional): Regular expression pattern to match window `app_id`
- **`title`** (optional): Regular expression pattern to match window title
- **`open_on_workspace`** (optional): Target workspace identifier (name or index)
- **`focus_command`** (optional): Command to execute when the window gains focus

**Note**: 
- At least one of `app_id` or `title` must be specified
- At least one of `open_on_workspace` or `focus_command` must be specified
- If both `app_id` and `title` are specified, either match works (OR logic)

> **Reference**: For detailed information about the window matching mechanism, see [Window Matching Mechanism](../window_matching.md)

## Workspace Identifiers

Supports two types:

- **name**: Workspace name, e.g., `"main"`, `"browser"`
- **idx**: Workspace index (1-based), e.g., `"1"`, `"2"`

**Matching Order**: Name first, then idx.

## How It Works

The plugin listens for `WindowOpenedOrChanged` events:

1. Uses configured regular expressions to match window `app_id` or `title`
2. If matched, automatically moves the window to the specified workspace
3. Rules are checked in configuration order, **the first matching rule is applied**

## Features

- ✅ **Regular Expressions**: Supports full regular expression syntax
- ✅ **Flexible Matching**: Supports `app_id` or `title`, or both combined (OR logic)
- ✅ **Regex Caching**: Compiled regular expressions are cached for better performance
- ✅ **Hot Config Reload**: Supports configuration updates without restarting the daemon

## Notes

1. **Rule Order Matters**: The first matching rule is applied, subsequent rules are not checked
2. **Non-existent Workspace**: If the specified workspace doesn't exist, a warning is logged but no error is raised
3. **Regex Performance**: Recommend using simple and clear patterns for better performance
