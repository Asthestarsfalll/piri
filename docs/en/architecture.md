# Architecture

## Project Structure

The project uses a modular design for easy extension:

### Core Modules

- `main.rs`: Main entry point, handles CLI command parsing and routing
- `lib.rs`: Library entry point, exports all public modules
- `config.rs`: Configuration management module, responsible for loading and parsing TOML configuration files
- `niri.rs`: Niri IPC wrapper module, provides interface for communicating with niri compositor
- `commands.rs`: Command processing system, contains `CommandHandler` for handling IPC requests
- `daemon.rs`: Daemon management, responsible for daemon startup, event loop, and lifecycle management
- `ipc.rs`: Inter-process communication module, implements Unix socket communication between client and daemon
- `utils.rs`: Utility functions module

### Plugin System

- `plugins/mod.rs`: Plugin system core, contains `Plugin` trait and `PluginManager`
- `plugins/scratchpads.rs`: Scratchpads plugin implementation
- `plugins/empty.rs`: Empty plugin implementation
- `plugins/window_rule.rs`: Window Rule plugin implementation
- `plugins/autofill.rs`: Autofill plugin implementation
- `plugins/singleton.rs`: Singleton plugin implementation
- `plugins/window_order.rs`: Window Order plugin implementation
- `plugins/window_utils.rs`: Window matching and utility functions

## How It Works

### Daemon Architecture

Piri uses a daemon architecture to provide continuous service. The daemon runs in the background, listening to niri events and executing corresponding operations.

**Daemon Startup Flow**:
1. `main.rs` parses CLI commands, starts daemon if `daemon` command is issued
2. `daemon.rs` creates `CommandHandler` and `PluginManager`
3. Initializes all enabled plugins
4. Starts unified event listener
5. Starts IPC server, listening for client connections
6. Enters main event loop, handling IPC requests and niri events

### IPC Communication

Clients communicate with the daemon through Unix socket, sending commands and receiving responses. This allows operations to be performed without restarting the daemon.

**IPC Communication Flow**:
1. Client connects to daemon's Unix socket via `IpcClient`
2. Sends serialized `IpcRequest` request
3. Daemon's `IpcServer` receives the request
4. `CommandHandler` processes the request, possibly routing through plugin system
5. Returns `IpcResponse` response to client

### Plugin System

The plugin system allows extending functionality. Each plugin implements the `Plugin` trait and is automatically loaded when the daemon starts.

**Plugin Lifecycle**:
1. **Initialization**: When daemon starts, `PluginManager` initializes all enabled plugins based on configuration
2. **Event Handling**: Plugins receive niri events through `handle_event` method
3. **IPC Request Handling**: Plugins can handle client requests through `handle_ipc_request` method
4. **Configuration Updates**: Supports hot-reloading configuration, plugins update config through `update_config` method
5. **Shutdown**: When daemon shuts down, calls plugin's `shutdown` method for cleanup

#### Unified Event Distribution

Piri uses a unified event distribution mechanism to optimize performance and resource usage:

- **Single Socket Connection**: All event-based plugins share one niri event stream socket connection, instead of each plugin having its own connection
- **Efficient Event Distribution**: Events are read only once, then distributed to all plugins that need to handle them via `mpsc::UnboundedChannel`
- **Smart Event Filtering**: Plugins can declare which event types they're interested in, and only relevant plugins receive events, avoiding unnecessary processing
- **Performance Optimization**: Reduces the number of socket connections and lowers system resource consumption
- **Easy to Extend**: New event-driven plugins only need to implement the `handle_event` and `is_interested_in_event` methods without managing their own event listener loops

**Event Distribution Flow**:
1. `PluginManager` starts unified event listener task
2. Event listener creates event stream connection via `NiriIpc::create_event_stream_socket`
3. Events are sent to daemon main loop through channel
4. Main loop calls `PluginManager::distribute_event` to distribute events
5. Only interested plugins receive event notifications

This design ensures:
- Better resource utilization (only one socket connection)
- Higher event processing efficiency (events are read only once, and distributed only to interested plugins)
- Cleaner plugin code (plugins only focus on event handling logic)
- Better performance (plugins that don't care about an event are not called)

### Configuration Hot Reload

Piri supports configuration file hot reloading without restarting the daemon:

- **File Watching**: Uses `notify` crate to watch for configuration file changes
- **Automatic Reload**: Automatically triggers reload when configuration file is modified
- **Plugin Updates**: After reloading config, all plugins update configuration through `update_config` method
- **Error Handling**: Sends notification on reload failure, but doesn't affect daemon operation

