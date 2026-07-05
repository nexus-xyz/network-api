// Copyright (c) 2025 Nexus. All rights reserved.

mod analytics;
mod cli_messages;
mod config;
mod consts;
mod environment;
mod events;
mod keys;
mod logging;
mod network;
#[path = "proto/nexus.orchestrator.rs"]
mod nexus_orchestrator;
mod orchestrator;
mod prover;
mod register;
mod runtime;
mod session;
pub mod system;
mod task;
mod ui;
mod version;
mod workers;

use crate::config::{Config, get_config_path};
use crate::environment::Environment;
use crate::orchestrator::OrchestratorClient;
use crate::prover::engine::ProvingEngine;
use crate::register::{register_node, register_user};
use crate::session::{run_headless_mode, run_tui_mode, setup_session};
use crate::version::manager::validate_version_requirements;
use clap::{ArgAction, Parser, Subcommand};
use postcard::to_allocvec;
use std::error::Error;
use std::io::Write;
use std::process::exit;

/// All available difficulty levels as (name, enum_value) pairs
const DIFFICULTY_LEVELS: &[(&str, crate::nexus_orchestrator::TaskDifficulty)] = &[
    ("SMALL", crate::nexus_orchestrator::TaskDifficulty::Small),
    (
        "SMALL_MEDIUM",
        crate::nexus_orchestrator::TaskDifficulty::SmallMedium,
    ),
    ("MEDIUM", crate::nexus_orchestrator::TaskDifficulty::Medium),
    ("LARGE", crate::nexus_orchestrator::TaskDifficulty::Large),
    (
        "EXTRA_LARGE",
        crate::nexus_orchestrator::TaskDifficulty::ExtraLarge,
    ),
    (
        "EXTRA_LARGE_2",
        crate::nexus_orchestrator::TaskDifficulty::ExtraLarge2,
    ),
    (
        "EXTRA_LARGE_3",
        crate::nexus_orchestrator::TaskDifficulty::ExtraLarge3,
    ),
    (
        "EXTRA_LARGE_4",
        crate::nexus_orchestrator::TaskDifficulty::ExtraLarge4,
    ),
    (
        "EXTRA_LARGE_5",
        crate::nexus_orchestrator::TaskDifficulty::ExtraLarge5,
    ),
];

/// Helper function to validate difficulty string and return parsed enum
fn validate_difficulty(difficulty_str: &str) -> Option<crate::nexus_orchestrator::TaskDifficulty> {
    let upper = difficulty_str.trim().to_ascii_uppercase();
    DIFFICULTY_LEVELS
        .iter()
        .find(|(name, _)| *name == upper)
        .map(|(_, difficulty)| *difficulty)
}

/// Helper function to print available difficulty levels dynamically from the enum
fn print_available_difficulties() {
    eprintln!("Valid difficulty levels are:");
    for (name, _) in DIFFICULTY_LEVELS {
        eprintln!("  {}", name);
    }
}

#[derive(Parser)]
#[command(author, version = concat!(env!("CARGO_PKG_VERSION"), " (build ", env!("BUILD_TIMESTAMP"), ")"), about, long_about = None)]
/// Command-line arguments
struct Args {
    /// Command to execute
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the prover
    Start {
        /// Node ID
        #[arg(long, value_name = "NODE_ID")]
        node_id: Option<u64>,

        /// Run without the terminal UI
        #[arg(long = "headless", action = ArgAction::SetTrue)]
        headless: bool,

        /// Maximum number of threads to use for proving. Capped at the number of CPU cores.
        #[arg(long = "max-threads", value_name = "MAX_THREADS")]
        max_threads: Option<u32>,

        /// Custom orchestrator URL (overrides environment setting)
        #[arg(long = "orchestrator-url", value_name = "URL")]
        orchestrator_url: Option<String>,

        /// Enable checking for risk of memory errors, may slow down CLI startup
        #[arg(long = "check-memory", default_value_t = false)]
        check_mem: bool,

        /// Enable background colors in the dashboard
        #[arg(long = "with-background", action = ArgAction::SetTrue)]
        with_background: bool,

        /// Maximum number of tasks to process before exiting (default: unlimited)
        #[arg(long = "max-tasks", value_name = "MAX_TASKS")]
        max_tasks: Option<u32>,

        /// Override max difficulty to request. Auto-promotion occurs when tasks complete in < 7 min
        #[arg(long = "max-difficulty", value_name = "DIFFICULTY")]
        max_difficulty: Option<String>,

        /// Ignore memory requirements per thread (allows running more threads than recommended)
        #[arg(long = "ignore-memory-requirement", default_value_t = false)]
        ignore_memory_requirement: bool,
    },
    /// Register a new user
    RegisterUser {
        /// User's public Ethereum wallet address. 42-character hex string starting with '0x'
        #[arg(long, value_name = "WALLET_ADDRESS")]
        wallet_address: String,
    },
    /// Register a new node to an existing user, or link an existing node to a user.
    RegisterNode {
        /// ID of the node to register. If not provided, a new node will be created.
        #[arg(long, value_name = "NODE_ID")]
        node_id: Option<u64>,
    },
    /// Clear the node configuration and logout.
    Logout,
    /// Hidden command for subprocess proof generation
    #[command(hide = true, name = "prove-fib-subprocess")]
    ProveFibSubprocess {
        /// Serialized inputs blob
        #[arg(long)]
        inputs: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Set up panic hook to prevent core dumps
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Panic occurred: {}", panic_info);
        std::process::exit(1);
    }));

    let nexus_environment_str = std::env::var("NEXUS_ENVIRONMENT").unwrap_or_default();
    let environment = nexus_environment_str
        .parse::<Environment>()
        .unwrap_or(Environment::default());

    let config_path = get_config_path()?;

    let args = Args::parse();
    match args.command {
        Command::Start {
            node_id,
            headless,
            max_threads,
            orchestrator_url,
            check_mem,
            with_background,
            max_tasks,
            max_difficulty,
            ignore_memory_requirement,
        } => {
            // If a custom orchestrator URL is provided, create a custom environment
            let final_environment = if let Some(url) = orchestrator_url {
                Environment::Custom {
                    orchestrator_url: url,
                }
            } else {
                environment
            };
            start(
                node_id,
                final_environment,
                config_path,
                headless,
                max_threads,
                check_mem,
                with_background,
                max_tasks,
                max_difficulty,
                ignore_memory_requirement,
            )
            .await
        }
        Command::Logout => {
            print_cmd_info!("Logging out", "Clearing node configuration file...");
            Config::clear_node_config(&config_path).map_err(Into::into)
        }
        Command::RegisterUser { wallet_address } => {
            print_cmd_info!("Registering user", "Wallet address: {}", wallet_address);
            let orchestrator = Box::new(OrchestratorClient::new(environment));
            register_user(&wallet_address, &config_path, orchestrator).await
        }
        Command::RegisterNode { node_id } => {
            let orchestrator = Box::new(OrchestratorClient::new(environment));
            register_node(node_id, &config_path, orchestrator).await
        }
        Command::ProveFibSubprocess { inputs } => {
            let inputs: (u32, u32, u32) = serde_json::from_str(&inputs)?;
            match ProvingEngine::prove_fib_subprocess(&inputs) {
                Ok(proof) => {
                    let bytes = to_allocvec(&proof)?;
                    let mut out = std::io::stdout().lock();
                    out.write_all(&bytes)?;
                    Ok(())
                }
                Err(e) => {
                    eprintln!("{}", e);
                    exit(consts::cli_consts::SUBPROCESS_INTERNAL_ERROR_CODE);
                }
            }
        }
    }
}

/// Starts the Nexus CLI application.
///
/// # Arguments
/// * `node_id` - This client's unique identifier, if available.
/// * `env` - The environment to connect to.
/// * `config_path` - Path to the configuration file.
/// * `headless` - If true, runs without the terminal UI.
/// * `max_threads` - Optional maximum number of threads to use for proving.
/// * `check_mem` - Whether to check risky memory usage.
/// * `with_background` - Whether to use the alternate TUI background color.
/// * `max_tasks` - Optional maximum number of tasks to prove.
#[allow(clippy::too_many_arguments)]
async fn start(
    node_id: Option<u64>,
    env: Environment,
    config_path: std::path::PathBuf,
    headless: bool,
    max_threads: Option<u32>,
    check_mem: bool,
    with_background: bool,
    max_tasks: Option<u32>,
    max_difficulty: Option<String>,
    ignore_memory_requirement: bool,
) -> Result<(), Box<dyn Error>> {
    // 1. Version checking (will internally perform country detection without race)
    validate_version_requirements().await?;

    // 2. Configuration resolution
    let orchestrator_client = OrchestratorClient::new(env.clone());
    let config = Config::resolve(node_id, &config_path, &orchestrator_client).await?;

    // 3. Session setup (authenticated worker only)
    // Parse and validate difficulty override (case-insensitive)
    let max_difficulty_parsed = if let Some(difficulty_str) = &max_difficulty {
        match validate_difficulty(difficulty_str) {
            Some(difficulty) => Some(difficulty),
            None => {
                eprintln!(
                    "Error: Invalid difficulty level '{}'",
                    difficulty_str.trim()
                );
                print_available_difficulties();
                eprintln!();
                eprintln!("Note: Difficulty levels are case-insensitive.");
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    let session = setup_session(
        config,
        env,
        check_mem,
        max_threads,
        max_tasks,
        max_difficulty_parsed,
        ignore_memory_requirement,
    )
    .await?;

    // 4. Run appropriate mode
    if headless {
        run_headless_mode(session).await
    } else {
        run_tui_mode(session, with_background).await
    }
}

#[cfg(test)]
mod tests {
    use crate::nexus_orchestrator::TaskDifficulty;

    #[test]
    fn test_difficulty_validation() {
        // Test valid difficulty levels (case-insensitive)
        assert_eq!(validate_difficulty("small"), Some(TaskDifficulty::Small));
        assert_eq!(validate_difficulty("SMALL"), Some(TaskDifficulty::Small));
        assert_eq!(validate_difficulty("Small"), Some(TaskDifficulty::Small));

        assert_eq!(
            validate_difficulty("small_medium"),
            Some(TaskDifficulty::SmallMedium)
        );
        assert_eq!(
            validate_difficulty("SMALL_MEDIUM"),
            Some(TaskDifficulty::SmallMedium)
        );

        assert_eq!(validate_difficulty("medium"), Some(TaskDifficulty::Medium));
        assert_eq!(validate_difficulty("large"), Some(TaskDifficulty::Large));
        assert_eq!(
            validate_difficulty("extra_large"),
            Some(TaskDifficulty::ExtraLarge)
        );
        assert_eq!(
            validate_difficulty("extra_large_2"),
            Some(TaskDifficulty::ExtraLarge2)
        );

        // Test invalid difficulty levels
        assert_eq!(validate_difficulty("invalid"), None);
        assert_eq!(validate_difficulty("small medium"), None); // space instead of underscore
        assert_eq!(validate_difficulty(""), None);
        assert_eq!(validate_difficulty("   "), None);
        assert_eq!(validate_difficulty("SMALL_MEDIUM_EXTRA"), None);
        assert_eq!(validate_difficulty("123"), None);
    }

    fn validate_difficulty(difficulty_str: &str) -> Option<TaskDifficulty> {
        match difficulty_str.trim().to_ascii_uppercase().as_str() {
            "SMALL" => Some(TaskDifficulty::Small),
            "SMALL_MEDIUM" => Some(TaskDifficulty::SmallMedium),
            "MEDIUM" => Some(TaskDifficulty::Medium),
            "LARGE" => Some(TaskDifficulty::Large),
            "EXTRA_LARGE" => Some(TaskDifficulty::ExtraLarge),
            "EXTRA_LARGE_2" => Some(TaskDifficulty::ExtraLarge2),
            _ => None,
        }
    }
}
