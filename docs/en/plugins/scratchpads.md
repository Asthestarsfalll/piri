# Scratchpads Plugin

Scratchpads allows you to quickly show and hide windows of frequently used applications, with support for cross-workspace and cross-monitor functionality.

## Demo Video

![Scratchpads Demo Video](../assets/scratchpads.mp4)

## Configuration

Use the `[scratchpads.{name}]` format to configure scratchpads:

```toml
[piri.plugins]
scratchpads = true

[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50

[scratchpads.calc]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "gnome-calculator"
size = "50% 40%"
margin = 100
```

### Configuration Parameters

- `direction` (required): Direction from which the window appears
  - `fromTop`: Slide in from top
  - `fromBottom`: Slide in from bottom
  - `fromLeft`: Slide in from left
  - `fromRight`: Slide in from right
- `command` (required): Full command string to launch the application, can include environment variables and arguments
- `app_id` (required): Application ID used to match windows (supports regular expressions)
- `size` (required): Window size in format `"width% height%"`
- `margin` (required): Margin from screen edge in pixels

> **Note**: `app_id` uses regular expression matching. If `app_id` contains special characters (such as `.`, `*`, etc.), they need to be escaped. For example: `app_id = "float\\.dropterm"`

> **Reference**: For detailed information about the window matching mechanism, see [Window Matching Mechanism](../window_matching.md)

## Usage

### Toggle Visibility

```bash
piri scratchpads {name} toggle

# Examples
piri scratchpads term toggle
piri scratchpads calc toggle
```

### Add Current Window

Quickly add the currently focused window as a scratchpad:

```bash
piri scratchpads {name} add {direction}

# Example
piri scratchpads mypad add fromRight
```

Dynamically added scratchpads will use the default size and margin set in the `[piri.scratchpad]` section.

> **Note**: Dynamically added windows are only resized and positioned once during initial registration. After that, you can manually resize or move the window, and the plugin will maintain your custom size and margin (position) during subsequent show/hide toggles without overriding it.

### Global Configuration

You can set global defaults in the `[piri.scratchpad]` section:

| Parameter | Description | Default |
| :--- | :--- | :--- |
| `default_size` | Default size for dynamic addition | `"75% 60%"` |
| `default_margin` | Default margin for dynamic addition | `50` |
| `move_to_workspace` | (Optional) Workspace to move windows to when hidden | `None` |

> **move_to_workspace**: If specified, hidden scratchpad windows will be moved to this workspace. This keeps hidden windows out of the current workspace's window stack. When shown, the window will still automatically move to the currently active workspace.

## How It Works

1. **First Launch**: If the window doesn't exist, launches the application specified in the configuration
2. **Window Registration**: After finding the window, sets it to floating mode and moves it off-screen
3. **Show**: Moves the window to the currently focused output and workspace, positions it according to configured direction and size, and focuses the window
4. **Hide**: Moves the window off-screen and intelligently restores previous focus

**Cross-workspace and cross-monitor**: Regardless of which workspace or monitor the scratchpad window was originally on, it will automatically move to the currently focused location.

## Features

- ✅ **Cross-workspace**: Quick access from any workspace
- ✅ **Cross-monitor**: Automatically appears on the currently focused monitor
- ✅ **Smart focus management**: Automatically focuses when showing, restores previous focus when hiding
- ✅ **Flexible configuration**: Customize window size, position, and animation direction
- ✅ **Dynamic addition**: Quickly add the currently focused window as a scratchpad
