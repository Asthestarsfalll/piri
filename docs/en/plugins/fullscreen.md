# Fullscreen Plugin

The Fullscreen plugin provides a fullscreen toggle that saves and restores the window's position in the scrolling layout. Niri's built-in fullscreen does not remember where a window was before going fullscreen — this plugin does.

## Configuration

```toml
[piri.plugins]
fullscreen = true
```

No additional configuration is needed.

## Usage

```bash
# Toggle fullscreen with position restore
piri fullscreen-toggle
```

### Niri Keybinding

Add a keybinding in your niri config (`~/.config/niri/config.kdl`):

```kdl
binds {
    Mod+F { spawn "piri" "fullscreen-toggle"; }
}
```

## How It Works

1. **Position Tracking**: The plugin continuously tracks the position (column and row) of all tiled windows in the scrolling layout
2. **Enter Fullscreen**: When toggling fullscreen on, the plugin saves the window's current column index, row index, and whether it was sharing a column with other windows
3. **Exit Fullscreen**: When toggling fullscreen off, the plugin restores the window to its saved position:
   - Moves the window back to its original column index
   - If the window was sharing a column, consumes it back into that column and restores its row position
4. **Floating Windows**: Floating windows are fullscreened normally without position tracking (they have no column position)

### Shared Column Handling

A key feature is the intelligent handling of shared columns (multiple windows stacked in a single column):

- Before fullscreen, the plugin detects whether the window shares its column with other windows
- On restore, if the column was shared, the window is expelled to its own column, moved to the correct index, then consumed back into the original column at the correct row

## Features

- **Transparent position tracking**: No user action needed, positions are tracked automatically via niri events
- **Shared column support**: Correctly restores windows that were stacked with other windows in the same column
- **Floating-aware**: Floating windows are fullscreened without attempting position restore
- **Robust**: Falls back to plain fullscreen toggle if position data is unavailable

## Use Cases

- Fullscreen a window for focused work, then restore it to its exact position in the layout
- Works seamlessly with complex multi-column, multi-row layouts

## Notes

1. **Use this instead of niri's built-in fullscreen** if you want position restoration. Niri's native `maximize-column` or `fullscreen-window` actions do not restore the previous layout position
2. **Column index is 1-based**: Positions follow niri's internal indexing
3. **Brief delays**: The plugin inserts small delays (50ms) between niri commands during restoration to ensure layout operations complete correctly
