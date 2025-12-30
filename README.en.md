# Piri

**English** | [中文](README.md)

---

Welcome to Piri, your gateway to extending the capabilities of niri compositor. Piri offers an extensible command system designed for simplicity and efficiency, allowing you to supercharge your productivity and customize your user experience.

You can think of it as a similar tool but for niri users (involves editing text files). With a command-based architecture, Piri is designed to be lightweight and easy to use.

Note that usage of Rust and daemon architecture encourages using many features with little impact on the footprint and performance.

Contributions, suggestions, bug reports and comments are welcome.

> **Note**: This project was entirely developed with the assistance of [Cursor](https://cursor.sh/) AI code editor.

## Features

- 🚀 **Daemon Mode**: Can run as a daemon to provide continuous service
- 🔧 **Niri IPC Wrapper**: Complete niri IPC command wrapper for easy interaction with niri
- 📝 **TOML Configuration**: TOML-formatted configuration files that are easy to read and edit
- 🎯 **Extensible Command System**: Supports `piri sub_command` format for easy addition of new features
- 📦 **Scratchpads**: Powerful window management feature for quick access to frequently used applications (see [Scratchpads](#scratchpads) section for details)

## Installation

### Using Install Script (Recommended)

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

### Using Cargo

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

## Configuration

Copy the example configuration file to the config directory:

```bash
mkdir -p ~/.config/niri
cp config.example.toml ~/.config/niri/piri.toml
```

Then edit `~/.config/niri/piri.toml` to configure your features.

### Basic Configuration Example

```toml
[niri]
# socket_path = "/tmp/niri"  # Optional, defaults to $XDG_RUNTIME_DIR/niri

[piri.scratchpad]
# Default size and margin for dynamically added scratchpads
default_size = "40% 60%"  # Default size, format: "width% height%"
default_margin = 50        # Default margin (pixels)
```

For more configuration options, please refer to the detailed documentation for each feature module.

## Usage

### Starting the Daemon

```bash
# Start daemon (runs in foreground)
piri daemon
```

### Reloading Configuration

```bash
# Reload configuration file (no need to restart daemon)
piri reload
```

Note: After reloading, new configuration takes effect immediately. Existing scratchpad windows will continue using old configuration, while newly launched scratchpads will use the new configuration.

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

## Scratchpads

Scratchpads is a powerful window management feature that allows you to quickly show and hide windows of frequently used applications. It supports cross-workspace and cross-monitor functionality, so you can quickly access your scratchpad windows regardless of which workspace or monitor you're on.

### Demo Video

<video src="assets/scratchpads.mp4" controls width="100%"></video>

### Configuration

Add `[scratchpads.{name}]` sections to your configuration file to configure scratchpads. Each scratchpad requires a unique name.

#### Configuration Example

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

#### Configuration Parameters

- `direction` (required): Direction from which the window appears
  - `fromTop`: Slide in from top
  - `fromBottom`: Slide in from bottom
  - `fromLeft`: Slide in from left
  - `fromRight`: Slide in from right

- `command` (required): Full command string to launch the application, can include environment variables and arguments

- `app_id` (required): Application ID used to match windows. This is the key identifier that niri uses to identify windows

- `size` (required): Window size in format `"width% height%"`, e.g., `"40% 60%"` means 40% of screen width and 60% of screen height

- `margin` (required): Margin from screen edge in pixels

### Usage

#### Toggle Scratchpad Visibility

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

#### Add Current Window as Scratchpad

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

#### How It Works

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

### Features

- ✅ **Cross-workspace support**: Access your scratchpad from any workspace
- ✅ **Cross-monitor support**: Works in multi-monitor setups, scratchpad automatically appears on the currently focused monitor
- ✅ **Smart focus management**: Automatically focuses when showing, intelligently restores previous focus when hiding
- ✅ **Flexible configuration**: Customize window size, position, and animation direction
- ✅ **Dynamic addition**: Quickly add the currently focused window as a scratchpad without editing configuration files

## How It Works

### Architecture

The project uses a modular design for easy extension:

- `config.rs`: Configuration management module
- `niri.rs`: Niri IPC wrapper module
- `commands.rs`: Command processing system
- `scratchpads.rs`: Scratchpads feature implementation
- `daemon.rs`: Daemon management
- `ipc.rs`: Inter-process communication (for client-daemon communication)

## Extensibility

### Adding New Subcommands

1. Add a new command to the `Commands` enum in `src/main.rs`
2. Add a handler method in `CommandHandler` in `src/commands.rs`
3. Implement the corresponding feature module
4. Add corresponding IPC message types in `src/ipc.rs` (if needed)

### Adding New Configuration Options

1. Add fields to the `Config` struct in `src/config.rs`
2. Update the TOML configuration file example

## Development

### Code Formatting

The project uses `rustfmt` for code formatting. The configuration file is `rustfmt.toml`.

#### Installing rustfmt

```bash
rustup component add rustfmt
```

#### Formatting Code

```bash
# Format all code
cargo fmt

# Check code format (without modifying files)
cargo fmt -- --check
```

## Dependencies

- `clap`: Command-line argument parsing
- `serde` / `toml`: Configuration serialization/deserialization
- `tokio`: Async runtime
- `anyhow`: Error handling
- `log` / `env_logger`: Logging system
- `niri-ipc`: Niri IPC client library

## License

MIT License

## References

This project is inspired by [Pyprland](https://github.com/hyprland-community/pyprland). Pyprland is an excellent project that provides extension capabilities for the Hyprland compositor, offering a plethora of plugins to enhance user experience. If you use Hyprland, we highly recommend trying Pyprland.
