# Window Order Plugin

The Window Order plugin automatically reorders windows in a workspace based on configured priority weights. Windows are sorted by their weight values, with larger values positioned further to the left.

## Configuration

Enable the plugin in the configuration file and set window weights:

```toml
[piri.plugins]
window_order = true

[piri.window_order]
enable_event_listener = true  # Enable event listening for automatic reordering
default_weight = 0            # Default weight for unconfigured windows
# workspaces = ["1", "2", "dev"]  # Optional: only apply to specific workspaces (empty = all)

[window_order]
google-chrome = 100
code = 80
ghostty = 70
```

### Configuration Options

- **`enable_event_listener`**: Whether to enable event listening. When enabled, windows are automatically reordered when layout changes or new windows open (only works in configured `workspaces`)
- **`default_weight`**: Default weight value for windows not configured in `[window_order]`
- **`workspaces`**: Optional, specify which workspaces to apply window ordering. Can be workspace names or indices (array of strings). If empty or not specified, applies to all workspaces
- **`[window_order]`**: Window weight configuration table, where keys are window `app_id` and values are weights (larger values go to the left)

### Weight Matching Rules

The plugin supports partial matching, for example:
- Config `ghostty = 70` can match `com.mitchellh.ghostty`
- Config `google-chrome = 100` can match `google-chrome-stable`

Matching priority:
1. Exact match
2. Config key is contained in `app_id`
3. `app_id` is contained in config key
4. If no match, use `default_weight`

## Usage

### Manual Trigger

Manually trigger window reordering via command line:

```bash
piri window_order toggle
```

**Note**: Manual trigger works in any workspace, regardless of `workspaces` configuration.

![Window Order - Manual Trigger](../../assets/window_order.mp4)

### Automatic Trigger

If `enable_event_listener` is enabled, the plugin automatically reorders windows when:
- Window layout changes (`WindowLayoutsChanged` event)
- New window opens (`WindowOpenedOrChanged` event)

**Note**: Automatic trigger only works in workspaces specified in the `workspaces` configuration. If `workspaces` is not configured or empty, applies to all workspaces.

![Window Order - Event-Driven Automatic Trigger](../../assets/window_order_envent.mp4)

## How It Works

The Window Order plugin uses an intelligent algorithm to minimize the number of window moves:

1. **Get Current State**: Get column positions of all tiled windows in the current workspace
2. **Calculate Target Positions**: Calculate target position for each window based on configured weights
3. **Optimize Move Sequence**: Use a greedy algorithm to find the solution with minimum moves
4. **Execute Moves**: Move windows to target positions sequentially

### Algorithm Features

- **Minimize Moves**: Prefer moves that get the most windows to correct positions
- **Minimize Distance**: Among moves with the same move count, choose the one with minimum distance
- **Prefer Focused Window**: If only one move is needed, prefer moving the currently focused window
- **Preserve Relative Order**: Windows with the same weight maintain their current relative order to avoid unnecessary moves

### Move Strategy

When moving a window from column A to column B:
- If A < B (moving right): Windows in columns (A, B] shift left by 1
- If A > B (moving left): Windows in columns [B, A) shift right by 1

The algorithm simulates the state after each move to select the optimal move sequence.

## Features

- ✅ **Smart Sorting**: Automatically sort windows based on configured weights
- ✅ **Minimize Moves**: Use optimized algorithm to reduce the number of window moves
- ✅ **Partial Matching**: Support partial matching of `app_id`
- ✅ **Event-Driven**: Optional automatic reordering feature
- ✅ **Focus Preservation**: Restore original focused window after moves
- ✅ **Stable Sort**: Windows with the same weight maintain relative order

## Use Cases

- **Workspace Organization**: Keep frequently used applications (browser, editor) fixed on the left side of the workspace
- **Multi-Window Management**: Maintain relative order when multiple windows of the same application exist
- **Automated Layout**: Automatically maintain window order through event listening

## Notes

1. **Tiled Windows Only**: Floating windows are not reordered
2. **Workspace Filtering**:
   - **Manual Trigger**: Works in any workspace, regardless of `workspaces` configuration
   - **Automatic Trigger**: Only works in workspaces specified in `workspaces` configuration. If not configured or empty, applies to all workspaces
3. **Larger Weight = Left**: A window with weight 100 will be to the left of a window with weight 80
4. **Same Weight Preserves Order**: Windows with the same weight don't change relative order to reduce unnecessary moves
5. **Workspace Matching**: `workspaces` supports workspace names or indices (as strings), e.g., `["1", "2", "dev"]`

