// Copyright (c) 2024 Nexus. All rights reserved.

// mod analytics;
mod config;
// mod prover;
mod flops;
#[path = "proto/nexus.orchestrator.rs"]
mod nexus_orchestrator;
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
// use serde_json;
use directories::ProjectDirs;
use rand::Rng;
use serde_json::json;
use sha3::{Digest, Keccak256};
use std::fs::File;
use std::io::Write;

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
    #[arg(long)]
    precompute: bool,
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
    } else if cli.precompute {
        let result = precompute_proof_hashes();
        println!("Result: {:?}", result);
    } else {
        println!("No command specified. Use --start, --logout, or --precompute");
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
        .submit_proof(&node_id, &proof_hash, proof_bytes)
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

fn precompute_proof_hashes() -> Result<(), Box<dyn std::error::Error>> {
    //number of inputs to precompute
    let num_inputs = 100;

    println!("Precomputing {} proofs and proof hashes...", num_inputs);

    // Get directories set up first
    let proj_dirs = ProjectDirs::from("xyz", "nexus", "cli").ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found")
    })?;
    let data_dir = proj_dirs.data_dir();
    let proofs_dir = data_dir.join("proofs");
    std::fs::create_dir_all(&proofs_dir)?;

    println!("\nRaw proofs will be saved to: {}", proofs_dir.display());

    //loop for each input
    for i in 0..num_inputs {
        println!("\nStarting iteration #{}", i);

        //1. compile the guest program
        println!("\tCompiling guest program...");
        let mut prover_compiler = Compiler::<CargoPackager>::new("example");
        let prover: Stwo<Local> =
            Stwo::compile(&mut prover_compiler).expect("failed to compile guest program");

        //2. Generate a random input and check if it exists
        let public_input: u32 = rand::thread_rng().gen_range(0..num_inputs);

        // Read existing data or create new if file doesn't exist
        let proj_dirs = ProjectDirs::from("xyz", "nexus", "cli").ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found")
        })?;
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;
        let file_path = data_dir.join("proof_hashes2.json");
        let mut proof_hashes = if file_path.exists() {
            let content = std::fs::read_to_string(&file_path)?;
            if content.starts_with("const PROOF_HASHES = ") {
                let json_str = &content["const PROOF_HASHES = ".len()..content.len() - 2];
                let json: serde_json::Value = serde_json::from_str(json_str)?;
                println!(
                    "\tLoaded {} existing proofs",
                    json["fast-fib"].as_object().unwrap().len()
                );
                json
            } else {
                serde_json::from_str(&content)?
            }
        } else {
            json!({
                "fast-fib": {}
            })
        };

        if proof_hashes["fast-fib"]
            .as_object()
            .unwrap()
            .contains_key(&public_input.to_string())
        {
            println!("Input {} already computed, skipping...", public_input);
            continue;
        }

        //3. Run the prover with the random input
        println!("\tRunning prover with random input {}...", public_input);
        let (view, proof) = prover
            .prove_with_input::<(), u32>(&(), &public_input)
            .expect("Failed to run prover");

        assert_eq!(view.exit_code().expect("failed to retrieve exit code"), 0);

        //get proof and proof hash
        let proof_bytes = serde_json::to_vec(&proof)?;
        let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

        //3. print the input and proof hash to a file so that it loooks like this:
        // const PROOF_HASHES = {
        //     "fast-fib": {
        //       "9": "84d767c4464f4a957482c1e8b9df447bbca622f3159b1f308433d1ef22c78250"
        //     }
        //   }

        // Add new proof hash to existing data
        if let Some(fast_fib) = proof_hashes["fast-fib"].as_object_mut() {
            fast_fib.insert(public_input.to_string(), json!(proof_hash));
        }

        // Write updated data back to file
        let json_string = serde_json::to_string_pretty(&proof_hashes)?;
        let mut file = File::create(&file_path)?;
        file.write_all(format!("const PROOF_HASHES = {};\n", json_string).as_bytes())?;

        println!("\tProof hash #{} saved to: {}", i, file_path.display());

        //wait for 2 seconds
        std::thread::sleep(std::time::Duration::from_secs(2));

        // After writing
        println!(
            "\tNow have {} total proof hashes",
            proof_hashes["fast-fib"].as_object().unwrap().len()
        );

        // Check if proof file already exists
        let proof_file_path = proofs_dir.join(format!("fast-fib-{}", public_input));
        if proof_file_path.exists() {
            println!(
                "Proof file for input {} already exists, skipping...",
                public_input
            );
            continue;
        }

        // Save the proof to a file
        std::fs::write(&proof_file_path, &proof_bytes)?;
        println!("\tProof saved to: {}", proof_file_path.display());
    }

    //sort the proof hashes by the keys
    println!("\nSorting final proof hashes...");
    let proj_dirs = ProjectDirs::from("xyz", "nexus", "cli").ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found")
    })?;
    let data_dir = proj_dirs.data_dir();
    let file_path = data_dir.join("proof_hashes2.json");
    let content = std::fs::read_to_string(&file_path)?;
    let json_str = &content["const PROOF_HASHES = ".len()..content.len() - 2];
    let mut proof_hashes: serde_json::Value = serde_json::from_str(json_str)?;

    //sort the proof hashes by the keys
    if let Some(fast_fib) = proof_hashes["fast-fib"].as_object_mut() {
        let mut entries: Vec<_> = fast_fib.iter().collect();
        entries.sort_by(|a, b| {
            // Parse strings to numbers and compare numerically
            let a_num = a.0.parse::<u32>().unwrap_or(0);
            let b_num = b.0.parse::<u32>().unwrap_or(0);
            a_num.cmp(&b_num)
        });

        let sorted = json!({
            "fast-fib": entries.into_iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>()
        });

        let json_string = serde_json::to_string_pretty(&sorted)?;
        let mut file = File::create(&file_path)?;
        file.write_all(format!("const PROOF_HASHES = {};\n", json_string).as_bytes())?;

        println!("Final sorted file saved to: {}", file_path.display());
    }

    println!("\nCompleted precomputation:");
    println!("- Proof hashes saved to: {}", file_path.display());
    println!("- Raw proofs saved to: {}", proofs_dir.display());
    println!(
        "- Total proofs generated: {}",
        proof_hashes["fast-fib"].as_object().unwrap().len()
    );

    Ok(())
}
