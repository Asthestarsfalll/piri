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
mod plugins;
mod utils;

use commands::CommandHandler;
use config::Config;
use ipc::{IpcClient, IpcRequest, IpcResponse};
use utils::send_notification;

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
    /// Singleton management
    Singleton {
        /// Singleton name
        name: String,
        /// Action to perform
        #[command(subcommand)]
        action: SingletonAction,
    },
    /// Window order management
    WindowOrder {
        /// Action to perform
        #[command(subcommand)]
        action: WindowOrderAction,
    },
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

#[derive(Subcommand)]
enum SingletonAction {
    /// Toggle singleton (focus if exists, launch if not)
    Toggle,
}

#[derive(Subcommand)]
enum WindowOrderAction {
    /// Toggle window order (reorder windows in current workspace)
    Toggle,
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
fn create_runtime() -> tokio::runtime::Runtime {
    // Create runtime with thread name
    tokio::runtime::Builder::new_multi_thread()
        .thread_name("piri")
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime")
}

fn main() -> Result<()> {
    // Set up panic hook to ensure errors are visible in daemon mode
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        eprintln!("Panic occurred: {:?}", panic_info);
        original_hook(panic_info);
    }));

    let rt = create_runtime();
    let result = rt.block_on(async_main());

    // Shutdown the runtime to ensure all tasks are dropped
    rt.shutdown_background();

    if let Err(e) = result {
        eprintln!("Error in main: {}", e);
        eprintln!("Error chain: {:?}", e);
        std::process::exit(1);
    }
    Ok(())
}

async fn async_main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logger
    let log_level = if cli.debug { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

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
                send_notification("piri", &format!("Start failed: {}", e));
                eprintln!("Failed to start daemon: {}", e);
                eprintln!("Error chain: {:?}", e);
                return Err(e);
            }
        }
        Commands::Scratchpads { name, action } => {
            let client = IpcClient::new(None);
            match action {
                ScratchpadAction::Toggle => {
                    handle_ipc_response(
                        client
                            .send_request(IpcRequest::ScratchpadToggle { name: name.clone() })
                            .await,
                        &format!("Scratchpad '{}' toggled", name),
                        "Failed to toggle scratchpad",
                    )?;
                }
                ScratchpadAction::Add { direction } => {
                    handle_ipc_response(
                        client
                            .send_request(IpcRequest::ScratchpadAdd {
                                name: name.clone(),
                                direction: direction.clone(),
                            })
                            .await,
                        &format!("Scratchpad '{}' added with direction '{}'", name, direction),
                        "Failed to add scratchpad",
                    )?;
                }
            }
        }
        Commands::Singleton { name, action } => {
            let client = IpcClient::new(None);
            match action {
                SingletonAction::Toggle => {
                    handle_ipc_response(
                        client
                            .send_request(IpcRequest::SingletonToggle { name: name.clone() })
                            .await,
                        &format!("Singleton '{}' toggled", name),
                        "Failed to toggle singleton",
                    )?;
                }
            }
        }
        Commands::WindowOrder { action } => {
            let client = IpcClient::new(None);
            match action {
                WindowOrderAction::Toggle => {
                    handle_ipc_response(
                        client.send_request(IpcRequest::WindowOrderToggle).await,
                        "Window order toggled",
                        "Failed to toggle window order",
                    )?;
                }
            }
        }
        Commands::Stop => {
            let client = IpcClient::new(None);
            handle_ipc_response(
                client.send_request(IpcRequest::Shutdown).await,
                "Daemon stopped",
                "Failed to stop daemon",
            )?;
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

fn handle_ipc_response(
    result: Result<IpcResponse>,
    success_msg: &str,
    error_prefix: &str,
) -> Result<()> {
    match result {
        Ok(IpcResponse::Success) => {
            println!("{}", success_msg);
            Ok(())
        }
        Ok(IpcResponse::Error(e)) => {
            send_notification("piri", &e);
            anyhow::bail!("{}: {}", error_prefix, e);
        }
        Ok(IpcResponse::Pong) => {
            println!("Pong");
            Ok(())
        }
        Err(e) => {
            send_notification("piri", &format!("Connection failed: {}", e));
            Err(e)
        }
    }
}
