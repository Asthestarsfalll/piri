# Development Guide

## Extensibility

### Adding New Plugins

1. Create a new plugin file in the `src/plugins/` directory (e.g., `myplugin.rs`)
2. Implement the `Plugin` trait:
   ```rust
   use async_trait::async_trait;
   use crate::plugins::Plugin;
   use crate::config::Config;
   use crate::niri::NiriIpc;
   use crate::ipc::IpcRequest;
   use niri_ipc::Event;
   use anyhow::Result;
   
   pub struct MyPlugin {
       niri: NiriIpc,
       // Plugin state
   }
   
   impl MyPlugin {
       pub fn new() -> Self {
           Self {
               niri: NiriIpc::new(None),
           }
       }
   }
   
   #[async_trait]
   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "myplugin" }
       
       async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
           self.niri = niri;
           // Initialize plugin, read configuration, etc.
           Ok(())
       }
       
       // Handle IPC requests (optional, if plugin needs to respond to client commands)
       async fn handle_ipc_request(&mut self, request: &IpcRequest) -> Result<Option<Result<()>>> {
           // If request is handled, return Some(Ok(()))
           // If not handled, return Ok(None)
           Ok(None)
       }
       
       // Handle niri events (optional, only for event-driven plugins)
       async fn handle_event(&mut self, event: &Event, niri: &NiriIpc) -> Result<()> {
           match event {
               Event::WindowOpenedOrChanged { window } => {
                   // Handle window opened or changed event
               }
               _ => {
                   // Ignore other events
               }
           }
           Ok(())
       }
       
       // Declare which event types the plugin is interested in (for event filtering)
       fn is_interested_in_event(&self, event: &Event) -> bool {
           matches!(event, Event::WindowOpenedOrChanged { .. })
       }
       
       // Update configuration (optional, supports hot reload)
       async fn update_config(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
           // Update plugin configuration
           Ok(())
       }

       // Shutdown plugin (optional, for resource cleanup)
       async fn shutdown(&mut self) -> Result<()> {
           // Cleanup resources
           Ok(())
       }
   }
   ```
3. Register the plugin in `src/plugins/mod.rs`:
   - Add `pub mod myplugin;` at the top of the file
   - Add plugin initialization logic in `PluginManager::init` method
4. Add plugin configuration structure in `src/config.rs`:
   - Add plugin enable/disable option in `PluginsConfig`
   - Add plugin-specific configuration structures (if needed)
5. Add IPC request types in `src/ipc.rs` (if plugin needs to respond to client commands)
6. Add CLI commands in `src/main.rs` (if plugin needs command-line interface)
7. Update configuration file example `config.example.toml`

#### Event-Driven Plugins

If you need to create an event-based plugin (e.g., listening to window events, workspace switches, etc.), simply implement the `handle_event` method. **You don't need to create your own event listener loop** because Piri uses a unified event distribution mechanism:

- All events are listened to by `PluginManager` in a unified way
- Events are distributed to plugins via the `handle_event` method
- Plugins only need to focus on event types they're interested in

This greatly simplifies plugin development and ensures efficient resource usage.

### Adding New Subcommands

1. Add a new command to the `Commands` enum in `src/main.rs`
2. Add command handling logic in the `async_main` function in `src/main.rs`
3. If the command needs to communicate with the daemon:
   - Add request type to `IpcRequest` enum in `src/ipc.rs`
   - Add request handling logic in `handle_request` function in `src/ipc.rs`
   - Or handle through plugin system (if command belongs to a plugin)
4. If the command needs direct access to niri:
   - Create `NiriIpc` instance
   - Call corresponding niri IPC methods

### Adding New Configuration Options

1. Add fields to the `Config` struct in `src/config.rs`
2. Update the TOML configuration file example

## Code Formatting

The project uses `rustfmt` for code formatting. The configuration file is `rustfmt.toml`.

### Installing rustfmt

```bash
rustup component add rustfmt
```

### Formatting Code

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

