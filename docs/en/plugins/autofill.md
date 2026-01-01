# Autofill Plugin

The Autofill plugin automatically aligns the last column of windows to the rightmost position when windows are closed or layout changes. This helps maintain a clean and organized window layout, preventing gaps in your workspace after closing windows.

## Demo Videos

![Autofill Demo Video](../assets/autofill.mp4)

![Autofill Demo Video 1](../assets/autofill_1.mp4)

![Autofill Demo Video 2](../assets/autofill_2.mp4)

## Configuration

Simply enable the plugin in your configuration file, no additional configuration needed:

```toml
[piri.plugins]
autofill = true
```

## How It Works

The plugin listens for `WindowClosed` or `WindowLayoutsChanged` events, and when triggered:

1. Saves the currently focused window
2. Focuses the first column, then the last column (aligning all columns to the rightmost position)
3. Restores the previously focused window

## Features

- ✅ **Zero Configuration**: Works out of the box
- ✅ **Event-Driven**: Real-time response to window changes
- ✅ **Focus Preservation**: Automatically saves and restores focus without disrupting workflow
- ✅ **Workspace-Aware**: Only affects the current workspace

## Use Cases

- Maintain organized alignment in multi-column window layouts
- Automatically fill gaps after closing windows
- Automatically maintain clean workspace layouts
