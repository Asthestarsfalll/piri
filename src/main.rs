use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, shells};
use log::info;
use std::io;
use std::path::PathBuf;

mod commands;
mod config;
mod daemon;
mod ipc;
mod niri;
mod scratchpads;

use commands::CommandHandler;
use config::Config;

#[derive(Parser)]
#[command(name = "piri")]
#[command(about = "A daemon for managing niri compositor", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Configuration file path
    #[arg(short, long, default_value = "~/.config/niri/piri.toml")]
    config: String,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start piri as a daemon
    Daemon,
    /// Scratchpads management
    Scratchpads {
        /// Scratchpad name
        name: String,
        /// Action to perform
        #[command(subcommand)]
        action: ScratchpadAction,
    },
    /// Reload configuration
    Reload,
    /// Stop the daemon
    Stop,
    /// Generate shell completion script
    Completion {
        /// Shell type
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum ScratchpadAction {
    /// Toggle scratchpad visibility
    Toggle,
    /// Add current focused window as scratchpad
    Add {
        /// Direction from which the scratchpad appears (e.g., "fromTop", "fromBottom", "fromLeft", "fromRight")
        direction: String,
    },
}

#[derive(Clone, ValueEnum)]
enum Shell {
    /// Bash completion script
    Bash,
    /// Zsh completion script
    Zsh,
    /// Fish completion script
    Fish,
    /// PowerShell completion script
    PowerShell,
    /// Elvish completion script
    Elvish,
}

// Custom tokio runtime with process name setting
#[cfg(target_os = "linux")]
fn create_runtime() -> tokio::runtime::Runtime {
    use std::ffi::CString;
    use std::os::raw::c_char;
    const PR_SET_NAME: libc::c_int = 15;

    // Set process name before creating runtime
    if let Ok(name) = CString::new("piri") {
        unsafe {
            libc::prctl(PR_SET_NAME, name.as_ptr() as *const c_char, 0, 0, 0);
        }
    }

    // Create runtime with thread name
    tokio::runtime::Builder::new_multi_thread()
        .thread_name("piri")
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
}

#[cfg(not(target_os = "linux"))]
fn create_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
}

fn main() -> Result<()> {
    // Set up panic hook to ensure errors are visible in daemon mode
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        eprintln!("Panic occurred: {:?}", panic_info);
        original_hook(panic_info);
    }));

    let rt = create_runtime();
    if let Err(e) = rt.block_on(async_main()) {
        eprintln!("Error in main: {}", e);
        eprintln!("Error chain: {:?}", e);
        std::process::exit(1);
    }
    Ok(())
}

async fn async_main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    // In daemon mode (PIRI_DAEMON env var set), ensure logs go to stderr
    // since stdout is closed
    let log_level = if cli.debug { "debug" } else { "info" };
    let mut logger_builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level));

    // If we're in daemon mode, force output to stderr
    if std::env::var("PIRI_DAEMON").is_ok() {
        logger_builder.target(env_logger::Target::Stderr);
    }

    logger_builder.init();

    match cli.command {
        Commands::Daemon => {
            // Only load config when starting daemon
            let config_path = shellexpand::full(&cli.config)
                .map(|s| PathBuf::from(s.as_ref()))
                .unwrap_or_else(|_| PathBuf::from(&cli.config));

            let config = Config::load(&config_path)?;
            info!("Loaded configuration from {:?}", config_path);

            let handler = CommandHandler::with_config_path(config, config_path);

            info!("Starting daemon");
            if let Err(e) = daemon::run(handler).await {
                eprintln!("Failed to start daemon: {}", e);
                eprintln!("Error chain: {:?}", e);
                return Err(e);
            }
        }
        Commands::Scratchpads { name, action } => {
            // Send command to daemon via IPC
            use crate::ipc::{IpcClient, IpcRequest, IpcResponse};

            let client = IpcClient::new(None);

            match action {
                ScratchpadAction::Toggle => {
                    let response = client
                        .send_request(IpcRequest::ScratchpadToggle { name: name.clone() })
                        .await?;
                    match response {
                        IpcResponse::Success => {
                            info!("Scratchpad '{}' toggled", name);
                        }
                        IpcResponse::Error(e) => {
                            anyhow::bail!("Failed to toggle scratchpad: {}", e);
                        }
                        _ => {
                            anyhow::bail!("Unexpected response from daemon");
                        }
                    }
                }
                ScratchpadAction::Add { direction } => {
                    let response = client
                        .send_request(IpcRequest::ScratchpadAdd {
                            name: name.clone(),
                            direction: direction.clone(),
                        })
                        .await?;
                    match response {
                        IpcResponse::Success => {
                            info!("Scratchpad '{}' added with direction '{}'", name, direction);
                        }
                        IpcResponse::Error(e) => {
                            anyhow::bail!("Failed to add scratchpad: {}", e);
                        }
                        _ => {
                            anyhow::bail!("Unexpected response from daemon");
                        }
                    }
                }
            }
        }
        Commands::Reload => {
            // Send reload command to daemon via IPC
            use crate::ipc::{IpcClient, IpcRequest, IpcResponse};

            let client = IpcClient::new(None);
            let response = client.send_request(IpcRequest::Reload).await?;

            match response {
                IpcResponse::Success => {
                    info!("Configuration reloaded");
                }
                IpcResponse::Error(e) => {
                    anyhow::bail!("Failed to reload configuration: {}", e);
                }
                _ => {
                    anyhow::bail!("Unexpected response from daemon");
                }
            }
        }
        Commands::Stop => {
            // Send stop command to daemon via IPC
            use crate::ipc::{IpcClient, IpcRequest, IpcResponse};

            let client = IpcClient::new(None);
            let response = client.send_request(IpcRequest::Shutdown).await?;

            match response {
                IpcResponse::Success => {
                    info!("Daemon stopped");
                }
                IpcResponse::Error(e) => {
                    anyhow::bail!("Failed to stop daemon: {}", e);
                }
                _ => {
                    anyhow::bail!("Unexpected response from daemon");
                }
            }
        }
        Commands::Completion { shell } => {
            let mut cmd = Cli::command();
            match shell {
                Shell::Bash => generate(shells::Bash, &mut cmd, "piri", &mut io::stdout()),
                Shell::Zsh => generate(shells::Zsh, &mut cmd, "piri", &mut io::stdout()),
                Shell::Fish => generate(shells::Fish, &mut cmd, "piri", &mut io::stdout()),
                Shell::PowerShell => {
                    generate(shells::PowerShell, &mut cmd, "piri", &mut io::stdout())
                }
                Shell::Elvish => generate(shells::Elvish, &mut cmd, "piri", &mut io::stdout()),
            }
        }
    }

    Ok(())
}
