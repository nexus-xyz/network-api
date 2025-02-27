use nexus_sdk::{stwo::seq::Stwo, Local, Prover, Viewable};

use crate::analytics;
use crate::config;
use crate::flops;
use crate::orchestrator_client::OrchestratorClient;
use crate::setup;
use crate::utils;
use colored::Colorize;
use sha3::{Digest, Keccak256};
use log::{error, warn};
use std::time::Duration;

/// Proves a program with a given node ID
#[allow(dead_code)]
async fn authenticated_proving(
    node_id: &str,
    environment: &config::Environment,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = OrchestratorClient::new(environment.clone());

    println!("Fetching a task to prove from Nexus Orchestrator...");
    let proof_task = match client.get_proof_task(node_id).await {
        Ok(task) => {
            println!("Successfully fetched task from Nexus Orchestrator.");
            task
        },
        Err(_) => {
            println!("Using local inputs.");
            return anonymous_proving();
        },
    };

    let public_input: u32 = proof_task.public_inputs.first().cloned().unwrap_or_default() as u32;

    println!("Compiling guest program...");
    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("fib_input");
    let prover =
        match Stwo::<Local>::new_from_file(&elf_file_path){
            Ok(prover) => prover,
            Err(e) => {
                error!("Failed to load guest program: {}", e);
                return Err(e.into())
            }
        };

    println!("Creating ZK proof with inputs...");
    let (view, proof) = match prover
        .prove_with_input::<(), u32>(&(), &public_input){
            Ok(result) => result,
            Err(e) => {
                error!("Failed to run prover: {}", e);
                return Err(e.into());
            }
        };

    let code = view.exit_code()
        .map(|u| u as i32) // convert on success
        .unwrap_or_else(|_err| {
            eprintln!("Failed to retrieve exit code: {:?}", _err);
            -1
        });
    
    assert_eq!(code, 0, "Unexpected exit code!");

    let proof_bytes = match serde_json::to_vec(&proof) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to serialize proof: {}", e);
            return Err(e.into());
        }
    };
    let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

    println!("Submitting ZK proof to Nexus Orchestrator...");
    if let Err(e) = client
        .submit_proof(node_id, &proof_hash, proof_bytes)
        .await{
            error!("Failed to submit proof: {}", e);
            return Err(e);
        }
    
    println!("{}", "ZK proof successfully submitted".green());
    Ok(())
}

fn anonymous_proving() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Instead of fetching the proof task from the orchestrator, we will use hardcoded input program and values

    // The 10th term of the Fibonacci sequence is 55
    let public_input: u32 = 9;
    println!("Compiling guest program...");
    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("fib_input");

    let prover = Stwo::<Local>::new_from_file(&elf_file_path)
        .map_err(|e| {
            error!("Failed to load guest program: {}", e);
            e
        })?;

    //3. Run the prover
    println!("Creating ZK proof (anonymous)...");
    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), &public_input)
        .map_err(|e| {
            error!("Failed to run prover: {}", e);
            e
    })?;

    let exit_code = view.exit_code().expect("Failed to retrieve exit code");
    if exit_code != 0 {
        error!("Unexpected exit code: {} (expected 0)", exit_code);
        return Err(format!("Exit code was {}", exit_code).into());
    }

    let proof_bytes = serde_json::to_vec(&proof)?;
    println!(
        "{}",
        format!(
            "ZK proof created (anonymous) with size: {} bytes",
            proof_bytes.len()
        )
        .green()
    );
    Ok(())
}

/// Starts the prover, which can be anonymous or connected to the Nexus Orchestrator
pub async fn start_prover(
    environment: &config::Environment,
) -> Result<(), Box<dyn std::error::Error>> {
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
        // If the user selected "anonymous"
        setup::SetupResult::Anonymous => {
            println!(
                "\n===== {} =====\n",
                "Starting Anonymous proof generation for programs"
                    .bold()
                    .underline()
                    .bright_cyan()
            );
            let client_id = format!("{:x}", md5::compute(b"anonymous"));
            let mut proof_count = 1;

            loop {
                println!("\n================================================");
                println!(
                    "{}",
                    format!("\nStarting proof #{} (anonymous) ...\n", proof_count).yellow()
                );

                // We'll do a few attempts (e.g. 3) in case of transient failures
                let max_attempts = 3;
                let mut attempt = 1;
                let mut success = false;

                while attempt <= max_attempts {
                    println!("Attempt #{} for anonymous proving", attempt);
                    match anonymous_proving() {
                        Ok(_) => {
                            println!("Anonymous proving succeeded on attempt #{attempt}!");
                            success = true;
                            break;
                        }
                        Err(e) => {
                            warn!("Attempt #{attempt} failed: {e}");
                            attempt += 1;
                            if attempt <= max_attempts {
                                warn!("Retrying anonymous proving in 2s...");
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                }

                if !success {
                    error!(
                        "All {} attempts to prove anonymously failed. Moving on to next proof iteration.",
                        max_attempts
                    );
                }

                proof_count += 1;
                analytics::track(
                    "cli_proof_anon_v2".to_string(),
                    format!("Completed anon proof iteration #{}", proof_count),
                    serde_json::json!({
                        "node_id": "anonymous",
                        "proof_count": proof_count,
                    }),
                    false,
                    environment,
                    client_id.clone(),
                );

                // Sleep before next proof
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            }
        }

        // If the user selected "connected"
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
            println!(
                "{}: {}",
                "Environment".bold(),
                environment.to_string().bright_cyan()
            );

            let client_id = format!("{:x}", md5::compute(node_id.as_bytes()));
            let mut proof_count = 1;

            loop {
                println!("\n================================================");
                println!(
                    "{}",
                    format!(
                        "\n[node: {}] Starting proof #{} (connected) ...\n",
                        node_id, proof_count
                    )
                    .yellow()
                );

                // Retry logic for authenticated_proving
                let max_attempts = 3;
                let mut attempt = 1;
                let mut success = false;

                while attempt <= max_attempts {
                    println!("Attempt #{} for authenticated proving (node_id={})", attempt, node_id);
                    match authenticated_proving(&node_id, environment).await {
                        Ok(_) => {
                            println!("Proving succeeded on attempt #{attempt}!");
                            success = true;
                            break;
                        }
                        Err(e) => {
                            warn!("Attempt #{attempt} failed with error: {e}");
                            attempt += 1;
                            if attempt <= max_attempts {
                                warn!("Retrying in 2s...");
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                }

                if !success {
                    error!(
                        "All {} attempts to prove with node {} failed. Continuing to next proof iteration.",
                        max_attempts, node_id
                    );
                }

                proof_count += 1;
                analytics::track(
                    "cli_proof_node_v2".to_string(),
                    format!("Completed proof iteration #{}", proof_count),
                    serde_json::json!({
                        "node_id": node_id,
                        "proof_count": proof_count,
                    }),
                    false,
                    environment,
                    client_id.clone(),
                );
            }
        }

        // If setup is invalid
        setup::SetupResult::Invalid => {
            error!("Invalid setup option selected.");
            Err("Invalid setup option selected".into())
        }
    }
}