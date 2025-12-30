# Piri

**English** | [中文](README.md)

---

Welcome to Piri, your gateway to extending the capabilities of niri compositor. Piri offers an extensible command system designed for simplicity and efficiency, allowing you to supercharge your productivity and customize your user experience.

You can think of it as a similar tool but for niri users (involves editing text files). With a command-based architecture, Piri is designed to be lightweight and easy to use.

Note that usage of Rust and daemon architecture encourages using many features with little impact on the footprint and performance.

Contributions, suggestions, bug reports and comments are welcome.

> **Note**: This project was entirely developed with the assistance of [Cursor](https://cursor.sh/) AI code editor.

## Supported Plugins

- 📦 **Scratchpads**: Powerful window management feature for quick access to frequently used applications (see [Scratchpads documentation](docs/en/plugins/scratchpads.md) for details)
- 🔌 **Empty**: Automatically execute commands when switching to empty workspaces, useful for automating workflows (see [Empty documentation](docs/en/plugins/empty.md) for details)

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

```bash
# Start daemon (runs in foreground)
piri daemon
```

```bash
# More debug logs
piri --debug daemon
```

### Reloading Configuration

```bash
# Reload configuration file (no need to restart daemon)
piri reload
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

![](./assets/scratchpads.mp4)

Quickly show and hide windows of frequently used applications. Supports cross-workspace and cross-monitor.

**Configuration Example**:
```toml
[piri.plugins]
scratchpads = true

[scratchpads.term]
direction = "fromRight"
command = "GTK_IM_MODULE=wayland ghostty --class=float.dropterm"
app_id = "float.dropterm"
size = "40% 60%"
margin = 50
```

**Quick Usage**:
```bash
# Toggle scratchpad show/hide
piri scratchpads {name} toggle

# Dynamically add current window as scratchpad
piri scratchpads {name} add {direction}
```

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

## Documentation

- [Architecture](docs/en/architecture.md) - Project architecture and how it works
- [Scratchpads](docs/en/plugins/scratchpads.md) - Detailed Scratchpads documentation
- [Plugin System](docs/en/plugins/plugins.md) - Detailed plugin system documentation
- [Development Guide](docs/en/development.md) - Development, extension, and contribution guide

## License

MIT License

## References

This project is inspired by [Pyprland](https://github.com/hyprland-community/pyprland). Pyprland is an excellent project that provides extension capabilities for the Hyprland compositor, offering a plethora of plugins to enhance user experience.
