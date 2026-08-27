# Piri

**English** | [中文](README.md)

---

Piri is a high-performance [Niri](https://github.com/YaLTeR/niri) compositor extension built with Rust, providing a robust state-driven plugin system via efficient IPC and unified event distribution.

## Core Plugins

- 📦 **Scratchpads**: Intelligent hide/show windows, cross-workspace/monitor support (see [Docs](docs/en/plugins/scratchpads.md))
- 🔌 **Empty**: Empty workspace automation, auto-executes commands on switch (see [Docs](docs/en/plugins/empty.md))
- 🎯 **Window Rule**: Rule engine with regex matching and focus-triggered commands (see [Docs](docs/en/plugins/window_rule.md))
- 📐 **Workspace Rule**: Layout management with auto-width, tiling, alignment, maximization, and EdgePulse indicators (see [Docs](docs/en/plugins/workspace_rule.md), [EdgePulse](docs/en/plugins/edge_pulse.md))
- 🔒 **Singleton**: Single-instance assurance for global uniqueness (see [Docs](docs/en/plugins/singleton.md))
- 📌 **Mark**: Named window marks for quick focus, in-memory bindings (see [Docs](docs/en/plugins/mark.md))
- 📍 **Sticky**: Floating window follower with cross-monitor support (see [Docs](docs/en/plugins/sticky.md))
- 📋 **Window Order**: Weight-based window reordering, minimizes movement (see [Docs](docs/en/plugins/window_order.md))
- 🍽️ **Swallow**: Window swallowing, child replaces parent in layout (see [Docs](docs/en/plugins/swallow.md))

## Core Features

- **High Performance with Rust**: Built with Rust, low memory footprint, async event handling
- **Efficient IPC Communication**: Communicates with Niri compositor via Unix Socket, automatic reconnect, batch command execution
- **Unified Event Distribution**: Single event listener receives all Niri events and distributes to plugins based on interest
- **Fine-grained Event Splitting**: Automatically splits coarse-grained Niri events into `WindowOpened`, `WindowChanged`, `WindowToggleFloating` and other sub-events, so plugins don't need to track window state themselves (see [Docs](docs/en/event_normalization.md))
- **State-driven Plugin System**: Each plugin implements a unified `Plugin` trait, supporting config hot-reload, event filtering, and IPC command handling
- **Hot Config Reload**: Changes to `piri.toml` take effect immediately without restarting the daemon

## Window Matching

Piri uses a unified window matching mechanism: regex on `app_id` and/or `title`. Plugins such as `window_rule`, `singleton`, and `scratchpads` use it to find windows.

**Supported matching**:
- Full regular expression syntax
- Match `app_id` and/or `title`
- If both are set, **either** match can satisfy the rule (OR)

> **Note**: The Window Rule plugin also supports list matching for `app_id` and `title`; see [Window Rule Docs](docs/en/plugins/window_rule.md).

**Details**: [Window matching](docs/en/window_matching.md)

## Quick Start

### Installation

#### Using Install Script (Recommended)

The easiest way is to use the provided install script:

```bash
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

https://github.com/user-attachments/assets/1c40d75b-310a-4503-9d59-92f0479a54d5

Quick show/hide windows, cross-workspace/monitor. Features: **dynamic addition**, **retains manual adjustments**, **auto-move on hide**, **swallow to focus** (`swallow_to_focus`), **sticky follow** (delegated to sticky plugin), **auto-hide on focus loss** (`auto_hide_on_focus_loss`, floating only), **non-floating direct focus**.

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

[scratchpads.note]
direction = "fromTop"
command = "gnome-text-editor"
app_id = "org.gnome.TextEditor"
size = "50% 40%"
margin = 100
sticky = true  # Follow focused workspace (handled by sticky plugin)

[scratchpads.calc]
direction = "fromBottom"
command = "gnome-calculator"
app_id = "org.gnome.Calculator"
size = "30% 40%"
margin = 50
auto_hide_on_focus_loss = true  # Auto-hide when window loses focus
```

> **Note**: `sticky` and `auto_hide_on_focus_loss` cannot both be enabled for the same scratchpad.

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

Regex-based window placement to specified workspaces, with focus-triggered command execution.

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
- Regex matching (`app_id`/`title`, lists supported, OR logic)
- Workspace name/index matching
- Workspace locking (`!` suffix), prevents windows from being moved away
- Focus-triggered commands with de-duplication
- `focus_command_once`: per-rule single execution ([issue #1](https://github.com/Asthestarsfalll/piri/issues/1))
- Pure event-driven

For detailed documentation, please refer to the [Window Rule documentation](docs/en/plugins/window_rule.md).

### Workspace Rule

https://github.com/user-attachments/assets/092a383c-993b-42b5-9b89-b78b0807b436

https://github.com/user-attachments/assets/b48b0465-0101-4298-9935-22f46d1a2658

https://github.com/user-attachments/assets/3b8dc835-473c-4b6e-b684-02a6951b63f9

https://github.com/user-attachments/assets/2e9e2c86-b9ef-44f1-a896-46473b93b417

Workspace layout management: auto-width, tiling, alignment, maximization. Built-in EdgePulse edge indicators (animated) render visual hints when focused column reaches workspace edge.

**Configuration Example**:
```toml
[piri.plugins]
workspace_rule = true

# Default configuration (applies to all workspaces)
[piri.workspace_rule]
auto_width = ["100%", "50%", "33.33%", "25%", "20%"]
auto_fill = true  # Enable automatic alignment
auto_maximize = true  # Automatic maximization

# EdgePulse edge indicator (with animation support)
[piri.workspace_rule.edge_pulse]
enabled = true
animation_enabled = true  # Enable animation
animation_style = "pulse"  # "pulse" for breathing, "fade" for fade-in
animation_duration = 600  # Animation cycle duration (milliseconds)
animation_amplitude = 0.8  # Animation intensity
animation_repeat = 3  # Repeat count per trigger (0 = infinite)

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
- Automatic alignment: Automatically align to rightmost position after closing windows
- Automatic maximization: Automatically maximize when only one window, unmaximize when multiple windows
- EdgePulse edge indicators: Render visual hints when the focused column reaches the workspace edge
- Workspace-aware: Each workspace can be configured independently
- Flexible configuration: Supports default and workspace-specific configuration

For detailed documentation, please refer to the [Workspace Rule documentation](docs/en/plugins/workspace_rule.md).

### Singleton

Manages single-instance windows for global uniqueness. Toggle focuses existing or launches new. Ideal for browsers, terminals, etc.

**Features**:
- Smart detection with auto App ID extraction
- Window registry for fast lookup
- Supports post-creation commands (`on_created_command`)

For detailed documentation, please refer to the [Singleton documentation](docs/en/plugins/singleton.md).

### Mark

Assign **named marks** (e.g. letters `a`, `b`) to windows for quick focus. Marks are kept in the daemon’s memory and are **cleared when the daemon restarts**. You only enable the plugin in `piri.toml` and bind `spawn` commands in Niri for marks you use often.

**Configuration example**:

```toml
[piri.plugins]
mark = true
```

**Quick usage**:

```bash
# No valid binding: bind focused window to name; binding exists and window lives: focus it
piri mark {name} toggle

# Force-bind focused window to name (overwrites previous binding)
piri mark {name} add

# Remove this mark
piri mark {name} delete
```

**Note**: Piri cannot capture the “next key” globally. To save shortcut slots, you can use a launcher (e.g. `fuzzel`) to pick a letter, then run the commands above. If Niri adds multi-key sequences or binding modes, you can group `piri mark …` calls under one prefix.

For detailed documentation, see the [Mark documentation](docs/en/plugins/mark.md).

### Sticky

Pin floating window to follow focused workspace. Ideal for utility windows (dictionary, translator, logs, media control).

**Configuration example**:

```toml
[piri.plugins]
sticky = true
```

**Quick usage**:

```bash
# Set sticky (same-monitor follow only)
piri sticky add

# Set sticky (allow cross-monitor follow)
piri sticky add --cross

# Remove sticky binding
piri sticky delete
```

For details, see the [Sticky documentation](docs/en/plugins/sticky.md).

### Window Order

https://github.com/user-attachments/assets/2c9cbbb4-7001-44ce-acfd-afb51dfbc372

https://github.com/user-attachments/assets/9818d478-3a33-456b-8367-548bb8ab7da7

Weight-based window reordering. Larger weight = further left.

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
- Intelligent sorting, minimizes moves
- Manual/event-driven trigger support
- Workspace filtering
- Preserves relative order for same weight
- Supports `app_id` partial matching

For detailed documentation, please refer to the [Window Order documentation](docs/en/plugins/window_order.md).

### Swallow

https://github.com/user-attachments/assets/9968e97f-fdea-4211-a007-717edf703e93

https://github.com/user-attachments/assets/51567d89-8ca8-4f4a-b2ca-732dfc0741c9

Hides parent windows when child opens, replacing parent position. Ideal for terminal-spawned viewers/players.

**Configuration Example**:
```toml
[piri.plugins]
swallow = true

[piri.swallow]
use_pid_matching = true  # Enable PID-based parent-child process matching (default: true)
swallow_on_change = false  # Re-check swallow rules when title/app_id changes (default: false)

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
- PID-based parent-child matching (default)
- Rule-based matching (`app_id`/`title`/`pid`)
- Global/rule-level exclude rules
- Intelligent focus window queue
- Auto-handles workspace movement and floating conversion

For detailed documentation, please refer to the [Swallow documentation](docs/en/plugins/swallow.md).

## Documentation

- [Architecture](docs/en/architecture.md) - Project architecture and how it works
- [Fine-grained Event Splitting](docs/en/event_normalization.md) - Event splitting mechanism explained
- [Plugin System](docs/en/plugins/plugins.md) - Detailed plugin system documentation
- [Development Guide](docs/en/development.md) - Development, extension, and contribution guide

## License

MIT License

## References

This project is inspired by [Pyprland](https://github.com/hyprland-community/pyprland). Pyprland is an excellent project that provides extension capabilities for the Hyprland compositor, offering a plethora of plugins to enhance user experience.
