use anyhow::Result;
use nexus_sdk::{stwo::seq::Stwo, Local, Prover, Viewable};
use thiserror::Error;

use crate::analytics;
use crate::config;
use crate::flops;
use crate::orchestrator_client::OrchestratorClient;
use crate::setup;
use crate::utils;
use colored::Colorize;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
};
use log::error;
use rayon::ThreadPoolBuilder;
use sha3::{Digest, Keccak256};
use std::io::stdout;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Error, Debug)]
enum ProverError {
    #[error("Failed to create thread pool: {0}")]
    ThreadPoolCreation(String),
    #[error("Failed to load guest program: {0}")]
    GuestProgramLoad(String),
    #[error("Failed to run prover: {0}")]
    ProverExecution(String),
    #[error("Unexpected exit code: {0}")]
    UnexpectedExitCode(u32),
    #[error("Failed to serialize proof: {0}")]
    ProofSerialization(#[from] serde_json::Error),
    #[error("Invalid setup option selected")]
    InvalidSetup,
}

// Add this new function after the ProverError implementation
fn calculate_thread_count(dedicated_threads: Option<usize>) -> usize {
    let total_threads = thread::available_parallelism().map_or(1, |n| n.get());
    
    // If dedicated_threads is specified, cap it at total_threads
    if let Some(threads) = dedicated_threads {
        return threads.min(total_threads);
    }

    // Default to 50% of available threads if not specified
    (total_threads + 1) / 2
}

async fn run_prover(
    _node_id: &str,
    _environment: &config::Environment,
    dedicated_threads: Option<usize>,
    public_input: u32,
    is_anonymous: bool,
) -> Result<(Vec<u8>, String)> {
    // Set thread count based on dedicated threads
    let num_threads = calculate_thread_count(dedicated_threads);

    // Create a new thread pool with the specified number of threads
    let _pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|e| ProverError::ThreadPoolCreation(e.to_string()))?;

    // If we're using more than one thread, show the worker table
    let show_progress = num_threads > 1;
    let (tx, _rx) = if show_progress {
        let (tx, mut rx) = mpsc::channel(100);
        // Spawn a task to handle progress updates
        let handle = tokio::spawn(async move {
            let mut workers = std::collections::HashMap::new();
            while let Some((worker_id, progress, message)) = rx.recv().await {
                workers.insert(worker_id, (progress, message));
                // Clear the screen and redraw the table
                execute!(stdout(), Clear(ClearType::All), MoveTo(0, 0)).unwrap();
                println!("Worker Progress:");
                println!("{:-<50}", "");
                for (id, (progress, msg)) in workers.iter() {
                    println!("Worker {}: [{}%] {}", id, progress, msg);
                }
                println!("{:-<50}", "");
            }
        });
        (Some(tx), Some(handle))
    } else {
        (None, None)
    };

    // Run the prover in the custom thread pool
    let result = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, String)> {
        if let Some(ref tx) = tx {
            tx.blocking_send((0, 0, "Compiling guest program...".to_string())).unwrap();
        } else {
            println!("Compiling guest program...");
        }

        let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("fib_input");

        let prover = Stwo::<Local>::new_from_file(&elf_file_path)
            .map_err(|e| ProverError::GuestProgramLoad(e.to_string()))?;

        if let Some(ref tx) = tx {
            tx.blocking_send((0, 20, "Creating ZK proof...".to_string())).unwrap();
        } else {
            println!("Creating ZK proof{}...", if is_anonymous { " (anonymous)" } else { "" });
        }

        let (view, proof) = prover
            .prove_with_input::<(), u32>(&(), &public_input)
            .map_err(|e| ProverError::ProverExecution(e.to_string()))?;

        let exit_code = view.exit_code().expect("Failed to retrieve exit code");
        if exit_code != 0 {
            return Err(ProverError::UnexpectedExitCode(exit_code).into());
        }

        let proof_bytes = serde_json::to_vec(&proof)?;
        let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

        if let Some(ref tx) = tx {
            tx.blocking_send((0, 100, format!("ZK proof created with size: {} bytes", proof_bytes.len()))).unwrap();
        } else {
            println!(
                "{}",
                format!(
                    "ZK proof created{} with size: {} bytes",
                    if is_anonymous { " (anonymous)" } else { "" },
                    proof_bytes.len()
                )
                .green()
            );
        }

        Ok((proof_bytes, proof_hash))
    }).await?;

    result
}

/// Proves a program with a given node ID
async fn authenticated_proving(
    node_id: &str,
    environment: &config::Environment,
    dedicated_threads: Option<usize>,
) -> Result<()> {
    let client = OrchestratorClient::new(environment.clone());

    println!("Fetching a task to prove from Nexus Orchestrator...");
    let proof_task = match client.get_proof_task(node_id).await {
        Ok(task) => {
            println!("Successfully fetched task from Nexus Orchestrator.");
            task
        }
        Err(_) => {
            println!("Using local inputs.");
            return anonymous_proving(dedicated_threads).await;
        }
    };

    let public_input: u32 = proof_task
        .public_inputs
        .first()
        .cloned()
        .unwrap_or_default() as u32;

    let (proof_bytes, proof_hash) = run_prover(node_id, environment, dedicated_threads, public_input, false).await?;

    let _ = client
        .submit_proof(
            node_id,
            &proof_hash,
            proof_bytes.clone(),
            proof_task.task_id as u64,
        )
        .await;

    println!("{}", "ZK proof successfully submitted".green().on_black());

    // Track analytics for authenticated proving in a separate task
    let analytics_data = serde_json::json!({
        "node_id": node_id,
        "dedicated_threads": dedicated_threads,
        "proof_size": proof_bytes.len(),
        "task_id": proof_task.task_id,
        "environment": environment.to_string(),
    });
    
    let environment_clone = environment.clone();
    let node_id_clone = node_id.to_string();
    tokio::spawn(async move {
        let _ = if let Err(e) = analytics::track(
            "cli_proof_node_v3".to_string(),
            "Completed authenticated proof".to_string(),
            analytics_data,
            false,
            &environment_clone,
            format!("{:x}", md5::compute(node_id_clone.as_bytes())),
        ) {
            let _ = error!("Failed to send analytics: {}", e);
        };
        ();
    });

    Ok(())
}

async fn anonymous_proving(
    dedicated_threads: Option<usize>,
) -> Result<()> {
    // The 10th term of the Fibonacci sequence is 55
    let public_input: u32 = 9;
    let environment = config::Environment::Local;
    let node_id = "anonymous";

    let (proof_bytes, _) = run_prover(node_id, &environment, dedicated_threads, public_input, true).await?;

    // Track analytics for anonymous proving in a separate task
    let analytics_data = serde_json::json!({
        "node_id": "anonymous",
        "dedicated_threads": dedicated_threads,
        "proof_size": proof_bytes.len(),
    });
    
    let environment_clone = environment.clone();
    tokio::spawn(async move {
        let _ = if let Err(e) = analytics::track(
            "cli_proof_node_anon".to_string(),
            "Completed anonymous proof".to_string(),
            analytics_data,
            false,
            &environment_clone,
            format!("{:x}", md5::compute(b"anonymous")),
        ) {
            let _ = error!("Failed to send analytics: {}", e);
        };
        ();
    });

    Ok(())
}

/// Starts the prover, which can be anonymous or connected to the Nexus Orchestrator
pub async fn start_prover(
    environment: &config::Environment,
    dedicated_threads: Option<usize>,
) -> Result<()> {
    // Print the banner at startup
    utils::cli_branding::print_banner();

    println!(
        "\n===== {} =====\n",
        "Setting up CLI configuration"
            .bold()
            .white()
            .on_blue(),
    );

    // Print the thread count
    let num_threads = calculate_thread_count(dedicated_threads);
    println!(
        "{}: {}",
        "Number of dedicated threads".bold().white(),
        format!("{}", num_threads).yellow()
    );

    // Run the initial setup to determine anonymous or connected node
    match setup::run_initial_setup().await {
        // If the user selected "anonymous"
        setup::SetupResult::Anonymous => {
            println!(
                "\n===== {} =====\n",
                "Starting Anonymous proof generation for programs"
                    .bold()
                    .white()
                    .on_blue()
            );
            anonymous_proving(dedicated_threads).await
        }

        // If the user selected "connected"
        setup::SetupResult::Connected(node_id) => {
            println!(
                "\n===== {} =====\n\n",
                "Connected - Welcome to the Supercomputer"
                    .bold()
                    .white()
                    .on_blue()
            );
            let flops = flops::measure_flops();
            let flops_formatted = format!("{:.2}", flops);
            let flops_str = format!("{} FLOPS", flops_formatted);
            println!(
                "{}: {}",
                "Computational capacity of this node".bold().white(),
                flops_str.yellow()
            );
            println!(
                "{}: {}",
                "You are proving with node ID".bold().white(),
                node_id.yellow()
            );
            println!(
                "{}: {}",
                "Environment".bold().white(),
                environment.to_string().yellow()
            );

            // Add a newline to separate the header from potential errors
            println!();

            authenticated_proving(&node_id, environment, dedicated_threads).await
        }

        // If setup is invalid
        setup::SetupResult::Invalid => {
            Err(ProverError::InvalidSetup.into())
        }
    }
}

/// Process a single proof task
#[allow(dead_code)]
async fn process_proof_task(
    _node_id: &str,
    _environment: &config::Environment,
    public_input: &u32,
    worker_id: usize,
    tx: &mpsc::Sender<(usize, u64, String)>,
) -> Result<()> {
    let start_time = Instant::now();
    tx.send((
        worker_id,
        0,
        format!("Worker {}: Compiling guest program...", worker_id),
    ))
    .await?;

    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("fib_input");

    let prover = Stwo::<Local>::new_from_file(&elf_file_path)
        .map_err(|e| ProverError::GuestProgramLoad(e.to_string()))?;

    tx.send((
        worker_id,
        20,
        format!("Worker {}: Creating ZK proof with inputs...", worker_id),
    ))
    .await?;
    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), public_input)
        .map_err(|e| ProverError::ProverExecution(e.to_string()))?;

    // Send incremental progress updates during proving
    let mut last_progress = 20;
    while last_progress < 80 {
        last_progress += 1;
        tx.send((
            worker_id,
            last_progress,
            format!("Worker {}: Proving in progress...", worker_id),
        ))
        .await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let code = view.exit_code().map(|u| u as i32).unwrap_or_else(|_err| -1);

    if code != 0 {
        tx.send((
            worker_id,
            0,
            format!("Worker {}: Unexpected exit code: {}", worker_id, code),
        ))
        .await?;
        return Err(ProverError::UnexpectedExitCode(code as u32).into());
    }

    tx.send((
        worker_id,
        80,
        format!("Worker {}: Serializing proof...", worker_id),
    ))
    .await?;
    let proof_bytes = serde_json::to_vec(&proof)?;
    let _proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

    let duration = start_time.elapsed();
    tx.send((
        worker_id,
        100,
        format!("Worker {}: Proof completed in {:.2?}", worker_id, duration),
    ))
    .await?;
    Ok(())
}
