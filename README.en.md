# Piri

**English** | [中文](README.md)

---

Piri is a high-performance [Niri](https://github.com/YaLTeR/niri) extension tool built with Rust. It leverages efficient Niri IPC interaction and a unified event distribution mechanism to provide a robust, state-driven plugin system.

## Core Plugins

- 📦 **Scratchpads**: Intelligent hide/show windows. Supports auto-capturing existing windows or launching on-demand, following you seamlessly across workspaces and monitors (see [Scratchpads Docs](docs/en/plugins/scratchpads.md))
- 🔌 **Empty**: Automation for empty workspaces. Automatically triggers preset commands when switching to an empty workspace to get you into the flow faster (see [Empty Docs](docs/en/plugins/empty.md))
- 🎯 **Window Rule**: Powerful rule engine. Automatically places windows based on regex matching and provides focus-triggered command execution with a built-in de-duplication mechanism (see [Window Rule Docs](docs/en/plugins/window_rule.md))
- 📐 **Workspace Rule**: Workspace window layout management. Provides automatic width adjustment, automatic tiling, automatic alignment, and automatic maximization. Integrates original Autofill functionality (see [Workspace Rule Docs](docs/en/plugins/workspace_rule.md))
- 🔒 **Singleton**: Single-instance assurance. Ensures specific applications remain globally unique, supporting quick focus or automatic process launching (see [Singleton Docs](docs/en/plugins/singleton.md))
- 📋 **Window Order**: Intelligent reordering. Automatically reorders tiled windows based on configured weights, preserving relative positions for identical weights to minimize movement (see [Window Order Docs](docs/en/plugins/window_order.md))
- 🍽️ **Swallow**: Window swallowing mechanism. Automatically hides parent windows when child windows are opened, allowing child windows to replace parent windows in the layout (see [Swallow Docs](docs/en/plugins/swallow.md))
- 🖥️ **Fullscreen**: Fullscreen with position restore. Toggles fullscreen while saving and restoring the window's exact position in the scrolling layout, including shared columns (see [Fullscreen Docs](docs/en/plugins/fullscreen.md))
- 🔄 **Refocus**: Close and refocus. Closes the focused window and restores focus to the previously focused window on the same workspace, instead of the geometrically adjacent one (see [Refocus Docs](docs/en/plugins/refocus.md))


## Quick Start

### Installation

#### Using Install Script (Recommended)

The easiest way is to use the provided install script:

```bash
# Run the install script
./install.sh
```

The install script will automatically:
- Build the release version
- Install to `~/.local/bin/piri` (regular user) or `/usr/local/bin/piri` (root)
- Copy configuration file to `~/.config/niri/piri.toml`

If `~/.local/bin` is not in your PATH, the script will prompt you to add it.

#### Using Cargo

```bash
# Install to user directory (recommended, no root required)
cargo install --path .

# Or install to system directory (requires root)
sudo cargo install --path . --root /usr/local
```

After installation, if installed to user directory, make sure `~/.cargo/bin` is in your `PATH`:

```bash
export PATH="$PATH:$HOME/.cargo/bin"
```

You can add this command to your shell configuration file (e.g., `~/.bashrc` or `~/.zshrc`).

### Configuration

Copy the example configuration file to the config directory:

```bash
mkdir -p ~/.config/niri
cp config.example.toml ~/.config/niri/piri.toml
```

Then edit `~/.config/niri/piri.toml` to configure your features.

## Usage

### Starting the Daemon

#### Manual Start

```bash
# Start daemon (runs in foreground)
piri daemon
```

```bash
# More debug logs
piri --debug daemon
```

#### Auto-start (Recommended)

Add the following configuration to your niri config file to automatically start piri daemon when niri starts:

Edit `~/.config/niri/config.kdl`, add to the `spawn-at-startup` section:

```kdl
spawn-at-startup "bash" "-c" "/path/to/piri daemon > /dev/null 2>&1 &"
```

### Shell Completion

Generate shell completion scripts:

```bash
# Bash
piri completion bash > ~/.bash_completion.d/piri

# Zsh
piri completion zsh > ~/.zsh_completion.d/_piri

# Fish
piri completion fish > ~/.config/fish/completions/piri.fish
```

## Plugins

### Scratchpads

![Scratchpads](./assets/scratchpads.mp4)

Quickly show and hide windows of frequently used applications. Supports cross-workspace and cross-monitor, so you can quickly access your scratchpad windows regardless of which workspace or monitor you're on. Features **dynamic window addition**, **automatic retention of manual size and margin adjustments**, **automatic moving to a specific workspace when hidden**, and **swallowing window into the currently focused window** (`swallow_to_focus` option).

**Configuration Example**:
```toml
[piri.plugins]
scratchpads = true

[piri.scratchpad]
default_size = "40% 60%"
default_margin = 50
move_to_workspace = "tmp" # Automatically move to workspace tmp when hidden

[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50

[scratchpads.preview]
direction = "fromRight"
command = "imv"
app_id = "imv"
size = "60% 80%"
margin = 50
swallow_to_focus = true  # Automatically swallow into focused window when shown
```

**Quick Usage**:
```bash
# Toggle scratchpad show/hide
piri scratchpads {name} toggle

# Dynamically add current window as scratchpad
piri scratchpads {name} add {direction}
```

> **Tip**: Dynamically added windows only use default size and margin during initial registration. After that, you can manually resize or move the window, and the plugin will automatically maintain these adjustments.

For detailed documentation, please refer to [Scratchpads documentation](docs/en/plugins/scratchpads.md).

### Empty

Automatically execute commands when switching to empty workspaces, useful for automating workflows.

> **Reference**: This functionality is similar to [Hyprland's `on-created-empty` workspace rule](https://wiki.hypr.land/Configuring/Workspace-Rules/#rules).

**Configuration Example**:
```toml
[piri.plugins]
empty = true

# Execute command when switching to workspace 1 if it's empty
[empty.1]
command = "alacritty"

# Use workspace name
[empty.main]
command = "firefox"
```

**Workspace Identifiers**: Supports matching by workspace name (e.g., `"main"`) or index (e.g., `"1"`).

For detailed documentation, please refer to [Plugin System documentation](docs/en/plugins/empty.md).

### Window Rule

Automatically move windows to specified workspaces based on their `app_id` or `title` using regular expression matching. This is very useful for automating window management, such as automatically assigning specific applications to specific workspaces.

> **Reference**: This functionality is similar to [Hyprland's window rules](https://wiki.hypr.land/Configuring/Window-Rules/).

**Configuration Example**:
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

# Specify both app_id and title (either match works)
[[window_rule]]
app_id = "code"
title = ".*VS Code.*"
open_on_workspace = "dev"

# Only focus_command, don't move window
[[window_rule]]
title = ".*Chrome.*"
focus_command = "notify-send 'Chrome focused'"

# Execute focus_command only once per rule (rule-level, not window-level)
[[window_rule]]
app_id = "firefox"
focus_command = "notify-send 'Firefox focused'"
focus_command_once = true

# app_id as a list (any one matches)
[[window_rule]]
app_id = ["code", "code-oss", "codium"]
open_on_workspace = "dev"

# title as a list (any one matches)
[[window_rule]]
title = [".*Chrome.*", ".*Chromium.*", ".*Google Chrome.*"]
open_on_workspace = "browser"
```

**Features**:
- Regular expression pattern matching support
- Match by `app_id` or `title`, or both combined (OR logic)
- Support for lists of patterns: `app_id` and `title` can be lists, any one match triggers the rule
- Support workspace name or index matching
- Focus-triggered command execution with built-in de-duplication mechanism
- `focus_command_once` option: execute `focus_command` only once per rule globally (see [issue #1](https://github.com/Asthestarsfalll/piri/issues/1))
- Pure event-driven, real-time response to window creation

For detailed documentation, please refer to the [Window Rule documentation](docs/en/plugins/window_rule.md).

### Workspace Rule

![Workspace Rule](./assets/autofill.mp4)

Workspace window layout management plugin that provides automatic width adjustment, automatic tiling, automatic alignment, and automatic maximization. Integrates the original Autofill plugin functionality, providing comprehensive workspace window management capabilities.

**Configuration Example**:
```toml
[piri.plugins]
workspace_rule = true

# Default configuration (applies to all workspaces)
[piri.workspace_rule]
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]
auto_fill = true  # Automatic alignment (original Autofill functionality)
auto_maximize = true  # Automatic maximization

# Workspace-specific configuration
[workspace_rule.main]
auto_maximize = true

[workspace_rule.dev]
auto_width = ["100%", ["45%", "55%"], ["30%", "35%", "35%"]]
auto_tile = true  # Automatic tiling
```

**Features**:
- Automatic width adjustment: Automatically adjust window widths based on window count
- Automatic tiling: Automatically merge new windows into existing columns
- Automatic alignment: Automatically align to rightmost position after closing windows (original Autofill functionality)
- Automatic maximization: Automatically maximize when only one window, unmaximize when multiple windows
- Workspace-aware: Each workspace can be configured independently
- Flexible configuration: Supports default and workspace-specific configuration

**Migration from Autofill**:
```toml
# Old configuration
[piri.plugins]
autofill = true

# New configuration
[piri.plugins]
workspace_rule = true

[piri.workspace_rule]
auto_fill = true  # Enable original Autofill functionality
```

For detailed documentation, please refer to the [Workspace Rule documentation](docs/en/plugins/workspace_rule.md).

### Singleton

Manages singleton windows - windows that should only have one instance. When you toggle a singleton, if the window already exists, it will focus it; otherwise, it will launch the application. This is useful for applications like browsers, terminals, or other tools where you typically only want one instance running.

**Configuration Example**:
```toml
[piri.plugins]
singleton = true

[singleton.browser]
command = "google-chrome-stable"

[singleton.term]
command = "GTK_IM_MODULE=wayland ghostty --class=singleton.term"
app_id = "singleton.term"

[singleton.editor]
command = "code"
app_id = "code"
on_created_command = "notify-send 'Editor opened'"
```

**Quick Usage**:
```bash
# Toggle singleton (focus if exists, launch if not)
piri singleton {name} toggle
```

**Features**:
- Smart window detection, automatically detects existing windows
- Automatic App ID extraction, no manual specification needed
- Window registry for fast lookup of existing windows
- Automatically focuses existing windows, prevents duplicate instances
- Supports executing custom commands after window creation (`on_created_command`)

For detailed documentation, please refer to the [Singleton documentation](docs/en/plugins/singleton.md).

### Window Order

![Window Order - Manual Trigger](./assets/window_order.mp4)

![Window Order - Event-Driven Automatic Trigger](./assets/window_order_envent.mp4)

Automatically reorder windows in workspace based on configured priority weights. Larger weight values position windows further to the left.

**Configuration Example**:
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

**Quick Usage**:
```bash
# Manually trigger window reordering (works in any workspace)
piri window_order toggle
```

**Features**:
- Intelligent sorting algorithm that minimizes window moves
- Supports manual trigger and event-driven automatic trigger
- Supports workspace filtering (only for automatic trigger)
- Preserves relative order for windows with same weight
- Supports partial matching of `app_id`

For detailed documentation, please refer to the [Window Order documentation](docs/en/plugins/window_order.md).

### Swallow

![Swallow](./assets/autofill_1.mp4)

Automatically hides parent windows when child windows are opened, allowing child windows to replace parent windows in the layout. This is useful for scenarios like terminals spawning image viewers or media players.

**Configuration Example**:
```toml
[piri.plugins]
swallow = true

[piri.swallow]
use_pid_matching = true  # Enable PID-based parent-child process matching (default: true)

# Global exclude rule (optional)
[piri.swallow.exclude]
app_id = [".*dialog.*"]

# Rules list
[[swallow]]
parent_app_id = [".*terminal.*", ".*alacritty.*", ".*foot.*", ".*ghostty.*"]
child_app_id = [".*mpv.*", ".*imv.*", ".*feh.*"]
exclude_child_app_id = [".*dialog.*", ".*error.*"]

[[swallow]]
parent_app_id = ["code", "nvim-qt"]
child_app_id = [".*preview.*", ".*markdown.*"]
```

**Features**:
- Supports PID-based parent-child process matching (enabled by default)
- Supports rule-based matching (via `app_id`, `title`, or `pid` patterns)
- Supports global and rule-level exclude rules
- Intelligent focus window queue for automatic parent window discovery
- Automatically handles workspace movement and floating window conversion
- Smart floating window handling: Floating windows are not swallowed, but re-attempts swallowing when converting from floating to tiled

For detailed documentation, please refer to the [Swallow documentation](docs/en/plugins/swallow.md).

### Fullscreen

Toggle fullscreen while saving and restoring the window's exact position in the scrolling layout. Niri's built-in fullscreen does not remember where a window was — this plugin does, including windows that share a column with other windows.

**Configuration Example**:
```toml
[piri.plugins]
fullscreen = true
```

**Quick Usage**:
```bash
# Toggle fullscreen with position restore
piri fullscreen-toggle
```

**Niri Keybinding**:
```kdl
binds {
    Mod+F { spawn "piri" "fullscreen-toggle"; }
}
```

**Features**:
- Saves and restores column index and row position
- Handles shared columns intelligently (expel, move, consume back)
- Floating windows are fullscreened normally without position tracking
- Falls back to plain fullscreen if position data is unavailable

For detailed documentation, please refer to the [Fullscreen documentation](docs/en/plugins/fullscreen.md).

### Refocus

Close the focused window and restore focus to the previously focused window on the same workspace. Solves the common issue where Niri focuses the geometrically adjacent window instead of the one you were working on.

**Configuration Example**:
```toml
[piri.plugins]
refocus = true
```

**Quick Usage**:
```bash
# Close focused window and refocus the previous one
piri close-refocus
```

**Niri Keybinding**:
```kdl
binds {
    Mod+Q { spawn "piri" "close-refocus"; }
}
```

**Features**:
- Maintains a focus history (depth 50) across all workspaces
- Only refocuses windows on the same workspace
- Chainable: A → B → C, close C → B, close B → A
- Falls back to Niri's default behavior when no history target is available

For detailed documentation, please refer to the [Refocus documentation](docs/en/plugins/refocus.md).

## Documentation

- [Architecture](docs/en/architecture.md) - Project architecture and how it works
- [Plugin System](docs/en/plugins/plugins.md) - Detailed plugin system documentation
- [Development Guide](docs/en/development.md) - Development, extension, and contribution guide

## License

MIT License

## References

This project is inspired by [Pyprland](https://github.com/hyprland-community/pyprland). Pyprland is an excellent project that provides extension capabilities for the Hyprland compositor, offering a plethora of plugins to enhance user experience.
