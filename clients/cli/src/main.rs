// Copyright (c) 2024 Nexus. All rights reserved.

// mod analytics;
mod config;
// mod prover;
mod flops;
#[path = "proto/nexus.orchestrator.rs"]
mod nexus_orchestrator;
mod node_id_manager;
mod orchestrator_client;
mod setup;
mod utils;

// use setup::SetupResult;

// Use high performance STWO
use nexus_sdk::{
    compile::{cargo::CargoPackager, Compile, Compiler},
    stwo::seq::Stwo,
    ByGuestCompilation, Local, Prover, Viewable,
};

// Update the import path to use the proto module
use orchestrator_client::OrchestratorClient;

use clap::Parser;
use colored::Colorize;
use sha3::{Digest, Keccak256};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Environment flags
    #[arg(long, group = "env")]
    local: bool,
    #[arg(long, group = "env")]
    dev: bool,
    #[arg(long, group = "env")]
    staging: bool,
    #[arg(long, group = "env")]
    beta: bool,

    /// Command to execute
    #[arg(long)]
    start: bool,
    #[arg(long)]
    logout: bool,
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

    // Use if/else instead of match for commands
    if cli.start {
        // Print the banner at startup
        utils::cli_branding::print_banner();

        // Set environment once at the start
        let environment = config::Environment::from_args(cli.local, cli.dev, cli.staging, cli.beta);

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
                    println!("\nStarting proof #{}...\n", proof_count);
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
                println!("Node computational capacity: {:.2} FLOPS", flops);
                println!("You are proving with the following node ID: {}", node_id);

                let mut proof_count = 1;
                loop {
                    println!("\n================================================");
                    println!("\nStarting proof #{}...\n", proof_count);

                    match authenticated_proving(&node_id, &environment).await {
                        Ok(_) => (),
                        Err(e) => println!("Error in authenticated proving: {}", e),
                    }
                    proof_count += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                }
            }
            setup::SetupResult::Invalid => {
                return Err("Invalid setup option selected".into());
            }
        };
    } else if cli.logout {
        setup::clear_user_config()?;
        println!("Successfully logged out");
    } else {
        println!("No command specified. Use --start, --logout");
    }
    Ok(())
}

async fn authenticated_proving(
    node_id: &str,
    environment: &config::Environment,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = OrchestratorClient::new(environment.clone());

    let proof_task = client.get_proof_task(node_id).await?;
    println!("1. Received proof task from Nexus Orchestrator...");

    let public_input: u32 = proof_task.public_inputs[0] as u32;

    //print inputs
    println!("2. Compiling guest program...");
    let mut prover_compiler = Compiler::<CargoPackager>::new("example");
    let prover: Stwo<Local> =
        Stwo::compile(&mut prover_compiler).expect("failed to compile guest program");

    println!("3. Creating proof with inputs...");

    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), &public_input)
        .expect("Failed to run prover");

    assert_eq!(view.exit_code().expect("failed to retrieve exit code"), 0);

    //REAL PROOF VERSION (DOES NOT WORK BECAUSE OF THE SIZE OF THE PROOF AT 157KB)
    let proof_bytes = serde_json::to_vec(&proof)?;
    let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

    println!("\tProof size: {} bytes", proof_bytes.len());
    println!("4. Submitting proof...");
    client
        .submit_proof(node_id, &proof_hash, proof_bytes)
        .await?;
    println!("{}", "5. Proof successfully submitted".green());

    Ok(())
}

fn anonymous_proving() -> Result<(), Box<dyn std::error::Error>> {
    //1. Instead of fetching the proof task from the orchestrator, we will use hardcoded input program and values

    // The 10th term of the Fibonacci sequence is 55
    let public_input: u32 = 9;

    //2. Compile the guest program
    println!("1. Compiling guest program...");
    let mut prover_compiler = Compiler::<CargoPackager>::new("example");
    let prover: Stwo<Local> =
        Stwo::compile(&mut prover_compiler).expect("failed to compile guest program");

    //3. Run the prover
    println!("2. Creating proof...");
    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), &public_input)
        .expect("Failed to run prover");

    assert_eq!(view.exit_code().expect("failed to retrieve exit code"), 0);

    let proof_bytes = serde_json::to_vec(&proof)?;

    println!(
        "{}",
        format!(
            "3. Proof successfully created with size: {} bytes",
            proof_bytes.len()
        )
        .green(),
    );
    println!("{}", "\nProof successfully created".green());
    Ok(())
}
