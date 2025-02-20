// Copyright (c) 2024 Nexus. All rights reserved.

// mod analytics;
mod config;
// mod prover;
mod flops;
mod memory_stats;
#[path = "proto/nexus.orchestrator.rs"]
mod nexus_orchestrator;
mod node_id_manager;
mod orchestrator_client;
mod prover;
mod setup;
mod utils;

// Update the import path to use the proto module
use clap::{Parser, Subcommand};

#[derive(clap::ValueEnum, Clone, Debug)]
enum Environment {
    Local,
    Dev,
    Staging,
    Beta,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Command to execute
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the prover
    Start {
        /// Environment to run in
        #[arg(long, value_enum)]
        env: Option<Environment>,
    },
    /// Logout from the current session
    Logout,
}

#[derive(Parser, Debug)]
struct Args {
    /// Hostname at which Orchestrator can be reached
    hostname: String,

    /// Port over which to communicate with Orchestrator
    #[arg(short, long, default_value_t = 443u16)]
    port: u16,

    /// Whether to hang up after the first proof
    #[arg(short, long, default_value_t = false)]
    just_once: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    //each arm of the match is a command
    match cli.command {
        Command::Start { env } => {
            match prover::start_prover(&config::Environment::from_args(env.as_ref())).await {
                Ok(_) => println!("Prover started successfully"),
                Err(e) => eprintln!("Failed to start prover: {}", e),
            }
        }
        Command::Logout => match setup::clear_node_id() {
            Ok(_) => println!("Successfully logged out"),
            Err(e) => eprintln!("Failed to logout: {}", e),
        },
    }

    Ok(())
}
