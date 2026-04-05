# Refocus Plugin

The Refocus plugin closes the focused window and restores focus to the previously focused window on the same workspace. It solves the issue where Niri focuses the geometrically adjacent window instead of the one you were working on before.

## Configuration

```toml
[piri.plugins]
refocus = true
```

No additional configuration is needed. The plugin starts tracking focus history as soon as it is enabled.

## Usage

```bash
# Close the focused window and refocus the previous one
piri close-refocus
```

### Niri Keybinding

Add a keybinding in your niri config (`~/.config/niri/config.kdl`):

```kdl
binds {
    Mod+Q { spawn "piri" "close-refocus"; }
}
```

## How It Works

1. **Focus Tracking**: The plugin passively listens to focus change events and maintains a global history of the last 50 focused windows
2. **Close and Refocus**: When `close-refocus` is invoked:
   - Identifies the currently focused window and its workspace
   - Searches the history for the most recently focused window that still exists on the same workspace
   - Closes the current window
   - Focuses the target window
3. **Fallback**: If no suitable target is found in the history (e.g., it's the only window on the workspace), the window is closed and Niri handles focus as usual

## Features

- **Same-workspace filtering**: Only refocuses windows on the current workspace, never switches workspaces unexpectedly
- **Stale entry cleanup**: Closed windows are automatically removed from the history
- **Deduplication**: Each window appears at most once in the history; refocusing a window moves it to the front
- **Deep history**: Tracks 50 entries, covering workflows with many workspaces and frequent navigation between them
- **Zero overhead when unused**: The plugin only processes lightweight focus/close events; the actual window query only happens when the command is invoked

## Use Cases

- Close an ephemeral window (file picker, dialog, quick lookup) and return to the window you were working on
- Chain multiple close-refocus calls: open A → B → C, close C → back to B, close B → back to A

## Notes

1. **Explicit command**: This is not automatic behavior on every window close. Use your regular close binding for normal closes, and `close-refocus` when you want to return to the previous window
2. **History capacity**: The history holds 50 entries. If the previously focused window has been evicted from the history, the plugin falls back to Niri's default focus behavior
3. **Floating windows**: The plugin tracks all windows regardless of tiling/floating state
