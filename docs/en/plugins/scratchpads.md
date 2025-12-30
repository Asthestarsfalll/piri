# Scratchpads

Scratchpads is a powerful window management feature that allows you to quickly show and hide windows of frequently used applications. It supports cross-workspace and cross-monitor functionality, so you can quickly access your scratchpad windows regardless of which workspace or monitor you're on.

## Demo Video

![Scratchpads Demo Video](../assets/scratchpads.mp4)

## Configuration

Add `[scratchpads.{name}]` sections to your configuration file to configure scratchpads. Each scratchpad requires a unique name.

### Configuration Example

```toml
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

- `app_id` (required): Application ID used to match windows. This is the key identifier that niri uses to identify windows

- `size` (required): Window size in format `"width% height%"`, e.g., `"40% 60%"` means 40% of screen width and 60% of screen height

- `margin` (required): Margin from screen edge in pixels

## Usage

### Toggle Scratchpad Visibility

```bash
piri scratchpads {name} toggle
```

Examples:

```bash
# Toggle terminal scratchpad
piri scratchpads term toggle

# Toggle calculator scratchpad
piri scratchpads calc toggle
```

### Add Current Window as Scratchpad

You can quickly add the currently focused window as a scratchpad without editing the configuration file:

```bash
piri scratchpads {name} add {direction}
```

Parameters:
- `{name}`: Name of the scratchpad (unique identifier)
- `{direction}`: Direction from which the window appears, options:
  - `fromTop`: Slide in from top
  - `fromBottom`: Slide in from bottom
  - `fromLeft`: Slide in from left
  - `fromRight`: Slide in from right

Example:

```bash
# Add current window as a scratchpad named "mypad", sliding in from right
piri scratchpads mypad add fromRight
```

Dynamically added scratchpads will use the default size and margin set in the `[piri.scratchpad]` section of the configuration file. After adding, you can use `piri scratchpads {name} toggle` to show/hide it.

## How It Works

1. **First Launch**: When executing `piri scratchpads {name} toggle`, if the window doesn't exist, it launches the application specified in the configuration

2. **Window Registration**: After finding the window, it sets it to floating mode and moves it off-screen

3. **Show/Hide**: 
   - **Show**: 
     - Records the currently focused window
     - Moves the window to the currently focused output and workspace, positioning it according to configured direction and size
     - Transfers focus to the scratchpad window
     - **Cross-workspace and cross-monitor support**: Regardless of which workspace or monitor the scratchpad window was originally on, it will automatically move to the currently focused location
   - **Hide**: 
     - Moves the window off-screen
     - Restores focus:
       - If the previously focused window is in the current workspace, focus it
       - If not in the current workspace, and there are other windows in the current workspace, focus the middle window

## Features

- ✅ **Cross-workspace support**: Access your scratchpad from any workspace
- ✅ **Cross-monitor support**: Works in multi-monitor setups, scratchpad automatically appears on the currently focused monitor
- ✅ **Smart focus management**: Automatically focuses when showing, intelligently restores previous focus when hiding
- ✅ **Flexible configuration**: Customize window size, position, and animation direction
- ✅ **Dynamic addition**: Quickly add the currently focused window as a scratchpad without editing configuration files

