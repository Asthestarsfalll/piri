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
   
   pub struct MyPlugin {
       // Plugin state
   }
   
   #[async_trait]
   impl Plugin for MyPlugin {
       fn name(&self) -> &str { "myplugin" }
       async fn init(&mut self, niri: NiriIpc, config: &Config) -> Result<()> { /* ... */ }
       async fn run(&mut self) -> Result<()> { /* ... */ }
   }
   ```
3. Register the plugin in `src/plugins/mod.rs`
4. Add plugin configuration structure in `src/config.rs`
5. Update the configuration file example

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

