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

#### Unified Event Distribution

Piri uses a unified event distribution mechanism to optimize performance and resource usage:

- **Single Socket Connection**: All event-based plugins share one niri event stream socket connection, instead of each plugin having its own connection
- **Efficient Event Distribution**: Events are read only once, then distributed to all plugins that need to handle them via a channel
- **Smart Event Filtering**: Plugins can declare which event types they're interested in, and only relevant plugins receive events, avoiding unnecessary processing
- **Performance Optimization**: Reduces the number of socket connections and lowers system resource consumption
- **Easy to Extend**: New event-driven plugins only need to implement the `handle_event` and `is_interested_in_event` methods without managing their own event listener loops

This design ensures:
- Better resource utilization (only one socket connection)
- Higher event processing efficiency (events are read only once, and distributed only to interested plugins)
- Cleaner plugin code (plugins only focus on event handling logic)
- Better performance (plugins that don't care about an event are not called)

