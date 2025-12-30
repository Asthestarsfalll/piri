# Architecture

## Project Structure

The project uses a modular design for easy extension:

- `config.rs`: Configuration management module
- `niri.rs`: Niri IPC wrapper module
- `commands.rs`: Command processing system
- `scratchpads.rs`: Scratchpads feature implementation
- `plugins/`: Plugin system directory
  - `mod.rs`: Plugin system core
  - `empty.rs`: Empty plugin implementation
  - `scratchpads.rs`: Scratchpads plugin implementation
- `daemon.rs`: Daemon management
- `ipc.rs`: Inter-process communication (for client-daemon communication)

## How It Works

### Daemon Architecture

Piri uses a daemon architecture to provide continuous service. The daemon runs in the background, listening to niri events and executing corresponding operations.

### IPC Communication

Clients communicate with the daemon through IPC, sending commands and receiving responses. This allows operations to be performed without restarting the daemon.

### Plugin System

The plugin system allows extending functionality. Each plugin implements the `Plugin` trait and is automatically loaded when the daemon starts.

