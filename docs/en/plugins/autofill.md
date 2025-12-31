# Autofill Plugin

The Autofill plugin automatically aligns the last column of windows to the rightmost position when windows are closed or layout changes. This helps maintain a clean and organized window layout, preventing gaps in your workspace after closing windows.

## Demo Video

![Autofill Demo Video](../assets/autofill.mp4)

## Configuration

The Autofill plugin requires no additional configuration. Simply enable it in your configuration file:

```toml
[piri.plugins]
autofill = true
```

## How It Works

The Autofill plugin uses a **pure event-driven** approach to listen to niri compositor events in real-time:

1. **Event Stream Listening**: The plugin listens to window events through niri IPC's `EventStream`
2. **Real-time Response**: When a `WindowClosed` or `WindowLayoutsChanged` event is received, it immediately checks the current workspace layout
3. **Last Column Detection**: Identifies the last column (rightmost column) in the current workspace
4. **Automatic Alignment**: Aligns the last column to the rightmost position by using niri's column navigation commands

## Alignment Algorithm

When a window is closed or layout changes, the plugin:

1. Gets the currently focused window to determine the workspace
2. Filters all non-floating windows in the current workspace
3. Finds the window in the last column (highest column number)
4. Focuses that window
5. Moves focus to the column left (to ensure we're not already at the edge)
6. Moves focus back to the column right (which aligns it to the rightmost position)

This ensures that after closing a window, the remaining windows automatically adjust to fill the space, keeping your workspace organized.

## Features

- ✅ **Pure Event-Driven**: Uses niri event stream for real-time listening, automatically handles layout changes when windows are closed
- ✅ **Zero Configuration**: Works out of the box, no configuration needed
- ✅ **Smart Detection**: Automatically detects when the last column needs alignment
- ✅ **Workspace-Aware**: Only affects windows in the current workspace where the change occurred
- ✅ **Floating Window Aware**: Ignores floating windows, only aligns tiled windows
- ✅ **Auto Reconnect**: Automatically reconnects when connection is lost, ensuring continuous service

## Usage

Simply enable the plugin in your configuration file:

```toml
[piri.plugins]
autofill = true
```

Then restart or reload the daemon:

```bash
# Restart daemon
piri daemon

# Or reload configuration if daemon is already running
piri reload
```

The plugin will automatically start working in the background. No commands or manual intervention needed!

## Use Cases

- **Clean Workspace Management**: Automatically maintain clean window layouts after closing windows
- **Multi-Column Layouts**: Keep multi-column window layouts organized and aligned
- **Productivity**: Focus on your work instead of manually rearranging windows

## Notes

1. **Floating Windows**: Floating windows are ignored by the plugin - only tiled windows are affected
2. **Single Window**: If there's only one window in a workspace, alignment is skipped (no need to align)
3. **No Focus Required**: The plugin automatically focuses windows as needed for alignment, you don't need to focus the last column manually
4. **Real-time Processing**: Alignment happens immediately when windows are closed or layout changes, ensuring a responsive experience
