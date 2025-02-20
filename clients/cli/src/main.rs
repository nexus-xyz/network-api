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
mod setup;
mod utils;

// Use high performance STWO
use nexus_sdk::{
    // compile::{cargo::CargoPackager, Compile, Compiler},
    stwo::seq::Stwo,
    // ByGuestCompilation,
    Local,
    Prover,
    Viewable,
};

// Update the import path to use the proto module
use orchestrator_client::OrchestratorClient;

use clap::{Parser, Subcommand};
use colored::Colorize;
use sha3::{Digest, Keccak256};

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
    /// Environment to run in
    #[arg(long, value_enum)]
    env: Option<Environment>,

    /// Command to execute
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the prover
    Start,
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

    // Set environment once at the start
    let environment = config::Environment::from_args(cli.env.as_ref());

    //each arm of the match is a command
    match cli.command {
        Command::Start => start_prover(&environment).await?,
        Command::Logout => {
            setup::clear_node_id()?;
            println!("Successfully logged out");
        }
    }

    Ok(())
}

async fn authenticated_proving(
    node_id: &str,
    environment: &config::Environment,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = OrchestratorClient::new(environment.clone());

    println!("1. Fetching a task to prove from Nexus Orchestrator...");
    let proof_task = client.get_proof_task(node_id).await?;
    println!("2. Received a task to prove from Nexus Orchestrator...");

    let public_input: u32 = proof_task.public_inputs[0] as u32;

    println!("3. Compiling guest program...");
    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("fib_input");
    let prover =
        Stwo::<Local>::new_from_file(&elf_file_path).expect("failed to load guest program");

    println!("4. Creating ZK proof with inputs...");
    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), &public_input)
        .expect("Failed to run prover");

    assert_eq!(view.exit_code().expect("failed to retrieve exit code"), 0);

    //REAL PROOF VERSION (DOES NOT WORK BECAUSE OF THE SIZE OF THE PROOF AT 157KB)
    let proof_bytes = serde_json::to_vec(&proof)?;
    let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

    println!("\tProof size: {} bytes", proof_bytes.len());
    println!("5. Submitting ZK proof to Nexus Orchestrator...");
    client
        .submit_proof(node_id, &proof_hash, proof_bytes)
        .await?;
    println!("{}", "6. ZK proof successfully submitted".green());

    Ok(())
}

fn anonymous_proving() -> Result<(), Box<dyn std::error::Error>> {
    //1. Instead of fetching the proof task from the orchestrator, we will use hardcoded input program and values

    // The 10th term of the Fibonacci sequence is 55
    let public_input: u32 = 9;

    //2. Compile the guest program
    println!("1. Compiling guest program...");
    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("fib_input");
    let prover =
        Stwo::<Local>::new_from_file(&elf_file_path).expect("failed to load guest program");

    //3. Run the prover
    println!("2. Creating ZK proof...");
    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), &public_input)
        .expect("Failed to run prover");

    assert_eq!(view.exit_code().expect("failed to retrieve exit code"), 0);

    let proof_bytes = serde_json::to_vec(&proof)?;

    println!(
        "{}",
        format!(
            "3. ZK proof successfully created with size: {} bytes",
            proof_bytes.len()
        )
        .green(),
    );
    Ok(())
}

async fn start_prover(environment: &config::Environment) -> Result<(), Box<dyn std::error::Error>> {
    // Print the banner at startup
    utils::cli_branding::print_banner();

    println!(
        "\n===== {} =====\n",
        "Setting up CLI configuration"
            .bold()
            .underline()
            .bright_cyan(),
    );

    // Run the initial setup to determine anonymous or connected node
    match setup::run_initial_setup().await {
        setup::SetupResult::Anonymous => {
            println!(
                "\n===== {} =====\n",
                "Starting Anonymous proof generation for programs"
                    .bold()
                    .underline()
                    .bright_cyan()
            );
            // Run the proof generation loop with anonymous proving
            let mut proof_count = 1;
            loop {
                println!("\n================================================");
                println!(
                    "{}",
                    format!("\nStarting proof #{} ...\n", proof_count).yellow()
                );
                match anonymous_proving() {
                    Ok(_) => (),
                    Err(e) => println!("Error in anonymous proving: {}", e),
                }
                proof_count += 1;
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            }
        }
        setup::SetupResult::Connected(node_id) => {
            println!(
                "\n===== {} =====\n",
                "Starting proof generation for programs"
                    .bold()
                    .underline()
                    .bright_cyan()
            );
            let flops = flops::measure_flops();
            let flops_formatted = format!("{:.2}", flops);
            let flops_str = format!("{} FLOPS", flops_formatted);
            println!(
                "{}: {}",
                "Computational capacity of this node".bold(),
                flops_str.bright_cyan()
            );
            println!(
                "{}: {}",
                "You are proving with node ID".bold(),
                node_id.bright_cyan()
            );

            let mut proof_count = 1;
            loop {
                println!("\n================================================");
                println!(
                    "{}",
                    format!("\nStarting proof #{} ...\n", proof_count).yellow()
                );

                match authenticated_proving(&node_id, &environment).await {
                    Ok(_) => (),
                    Err(e) => println!("\tError: {}", e),
                }
                proof_count += 1;
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            }
        }
        setup::SetupResult::Invalid => {
            return Err("Invalid setup option selected".into());
        }
    };
}
