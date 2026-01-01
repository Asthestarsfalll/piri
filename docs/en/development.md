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
   use niri_ipc::Event;
   
   pub struct MyPlugin {
       niri: NiriIpc,
       // Plugin state
   }
   
   #[async_trait]
   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "myplugin" }
       
       async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> {
           self.niri = niri;
           // Initialize plugin
           Ok(())
       }
       
       async fn run(&mut self) -> Result<()> {
           // Periodic tasks (if needed)
           Ok(())
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
   }
   ```
3. Register the plugin in `src/plugins/mod.rs`
4. Add plugin configuration structure in `src/config.rs`
5. Update the configuration file example

#### Event-Driven Plugins

If you need to create an event-based plugin (e.g., listening to window events, workspace switches, etc.), simply implement the `handle_event` method. **You don't need to create your own event listener loop** because Piri uses a unified event distribution mechanism:

- All events are listened to by `PluginManager` in a unified way
- Events are distributed to plugins via the `handle_event` method
- Plugins only need to focus on event types they're interested in

This greatly simplifies plugin development and ensures efficient resource usage.

### Adding New Subcommands

1. Add a new command to the `Commands` enum in `src/main.rs`
2. Add a handler method in `CommandHandler` in `src/commands.rs`
3. Implement the corresponding feature module
4. Add corresponding IPC message types in `src/ipc.rs` (if needed)

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

