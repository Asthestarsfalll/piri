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
2. **Real-time Response**: When a `WindowClosed` or `WindowLayoutsChanged` event is received, it immediately aligns columns in the current workspace
3. **Automatic Alignment**: Aligns columns to the rightmost position by using niri's column navigation commands (`focus-column-first` followed by `focus-column-last`)

## Alignment Algorithm

When a window is closed or layout changes, the plugin:

1. Focuses the first column in the current workspace
2. Focuses the last column in the current workspace (which aligns it to the rightmost position)

This simple approach ensures that after closing a window, the remaining windows automatically adjust to fill the space, keeping your workspace organized.

## Features

- ✅ **Pure Event-Driven**: Uses niri event stream for real-time listening, automatically handles layout changes when windows are closed
- ✅ **Zero Configuration**: Works out of the box, no configuration needed
- ✅ **Simple and Efficient**: Directly aligns columns without complex window detection logic
- ✅ **Workspace-Aware**: Only affects columns in the current workspace where the change occurred
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

1. **Workspace-Based**: The plugin operates on the current workspace where the event occurred
2. **Simple Operation**: The plugin simply focuses the first column then the last column, which automatically aligns all columns to the rightmost position
3. **Real-time Processing**: Alignment happens immediately when windows are closed or layout changes, ensuring a responsive experience
