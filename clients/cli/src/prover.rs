use nexus_sdk::{stwo::seq::Stwo, Local, Prover, Viewable};

use crate::analytics;
use crate::config;
use crate::flops;
use crate::orchestrator_client::OrchestratorClient;
use crate::setup;
use crate::utils;
use colored::Colorize;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use log::error;
use rayon::ThreadPoolBuilder;
use sha3::{Digest, Keccak256};
use std::error::Error as StdError;
use std::fmt;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug)]
struct ProverError {
    message: String,
}

impl fmt::Display for ProverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl StdError for ProverError {}

impl From<String> for ProverError {
    fn from(message: String) -> Self {
        ProverError { message }
    }
}

impl From<serde_json::Error> for ProverError {
    fn from(e: serde_json::Error) -> Self {
        ProverError {
            message: format!("Serde error: {}", e),
        }
    }
}

// Make ProverError Send + Sync
unsafe impl Send for ProverError {}
unsafe impl Sync for ProverError {}

// At the start of the file, add type definition for message channel
type MessageChannel = (mpsc::Sender<(usize, u64, String)>, mpsc::Receiver<(usize, u64, String)>);

// Add this new function after the ProverError implementation
fn calculate_thread_count(
    speed: &crate::ProvingSpeed,
    dedicated_cores: Option<usize>,
) -> usize {
    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
    
    // If dedicated_cores is specified, cap it at total_cores
    if let Some(cores) = dedicated_cores {
        return cores.min(total_cores);
    }

    // Otherwise, use the speed setting to determine percentage of available cores
    match speed {
        crate::ProvingSpeed::Low => (total_cores + 3) / 4, // Use 25% of cores, rounded up
        crate::ProvingSpeed::Medium => (total_cores + 1) / 2, // Use 50% of cores, rounded up
        crate::ProvingSpeed::High => (total_cores * 3 + 3) / 4, // Use 75% of cores, rounded up
    }
}

fn run_prover(
    node_id: &str,
    environment: &config::Environment,
    speed: &crate::ProvingSpeed,
    dedicated_cores: Option<usize>,
    public_input: u32,
    is_anonymous: bool,
) -> Result<(Vec<u8>, String), Box<dyn StdError + Send + Sync>> {
    // Set thread count based on speed and dedicated cores
    let num_threads = calculate_thread_count(speed, dedicated_cores);

    // Create a new thread pool with the specified number of threads
    let pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .map_err(|e| {
            Box::new(ProverError::from(format!(
                "Failed to create thread pool: {}",
                e
            ))) as Box<dyn StdError + Send + Sync>
        })?;

    // Run the prover in the custom thread pool
    pool.install(|| -> Result<(Vec<u8>, String), Box<dyn StdError + Send + Sync>> {
        println!("Compiling guest program...");
        let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("fib_input");

        let prover = Stwo::<Local>::new_from_file(&elf_file_path).map_err(|e| {
            Box::new(ProverError::from(format!(
                "Failed to load guest program: {}",
                e
            ))) as Box<dyn StdError + Send + Sync>
        })?;

        println!("Creating ZK proof{}...", if is_anonymous { " (anonymous)" } else { "" });
        let (view, proof) = prover
            .prove_with_input::<(), u32>(&(), &public_input)
            .map_err(|e| {
                Box::new(ProverError::from(format!("Failed to run prover: {}", e)))
                    as Box<dyn StdError + Send + Sync>
            })?;

        let exit_code = view.exit_code().expect("Failed to retrieve exit code");
        if exit_code != 0 {
            error!("Unexpected exit code: {} (expected 0)", exit_code);
            return Err(
                Box::new(ProverError::from(format!("Exit code was {}", exit_code)))
                    as Box<dyn StdError + Send + Sync>,
            );
        }

        let proof_bytes = serde_json::to_vec(&proof)
            .map_err(ProverError::from)?;
        let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

        println!(
            "{}",
            format!(
                "ZK proof created{} with size: {} bytes",
                if is_anonymous { " (anonymous)" } else { "" },
                proof_bytes.len()
            )
            .green()
        );

        Ok((proof_bytes, proof_hash))
    })
}

/// Proves a program with a given node ID
async fn authenticated_proving(
    node_id: &str,
    environment: &config::Environment,
    speed: &crate::ProvingSpeed,
    dedicated_cores: Option<usize>,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let client = OrchestratorClient::new(environment.clone());

    println!("Fetching a task to prove from Nexus Orchestrator...");
    let proof_task = match client.get_proof_task(node_id).await {
        Ok(task) => {
            println!("Successfully fetched task from Nexus Orchestrator.");
            task
        }
        Err(_) => {
            println!("Using local inputs.");
            return anonymous_proving(speed, dedicated_cores);
        }
    };

    let public_input: u32 = proof_task
        .public_inputs
        .first()
        .cloned()
        .unwrap_or_default() as u32;

    let (proof_bytes, proof_hash) = run_prover(node_id, environment, speed, dedicated_cores, public_input, false)?;

    let _ = client
        .submit_proof(
            node_id,
            &proof_hash,
            proof_bytes,
            proof_task.task_id as u64,
        )
        .await;

    println!("{}", "ZK proof successfully submitted".green());
    Ok(())
}

fn anonymous_proving(
    speed: &crate::ProvingSpeed,
    dedicated_cores: Option<usize>,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    // The 10th term of the Fibonacci sequence is 55
    let public_input: u32 = 9;
    let environment = config::Environment::Local;
    let node_id = "anonymous";

    let (proof_bytes, _) = run_prover(node_id, &environment, speed, dedicated_cores, public_input, true)?;

    // Track analytics for anonymous proving
    analytics::track(
        "cli_proof_node_anon".to_string(),
        "Completed anonymous proof".to_string(),
        serde_json::json!({
            "node_id": "anonymous",
            "speed": format!("{:?}", speed),
            "dedicated_cores": dedicated_cores,
            "proof_size": proof_bytes.len(),
        }),
        false,
        &config::Environment::Local,
        format!("{:x}", md5::compute(b"anonymous")),
    );

    Ok(())
}

/// Starts the prover, which can be anonymous or connected to the Nexus Orchestrator
pub async fn start_prover(
    environment: &config::Environment,
    speed: &crate::ProvingSpeed,
    dedicated_cores: Option<usize>,
) -> Result<(), Box<dyn StdError>> {
    // Print the banner at startup
    utils::cli_branding::print_banner();

    const EVENT_NAME: &str = "cli_proof_node_v3";

    println!(
        "\n===== {} =====\n",
        "Setting up CLI configuration"
            .bold()
            .underline()
            .bright_cyan(),
    );

    // Print the selected speed setting and core count
    let num_threads = calculate_thread_count(speed, dedicated_cores);
    println!(
        "{}: {}",
        "Proving speed".bold(),
        format!("{:?}", speed).bright_cyan()
    );
    println!(
        "{}: {}",
        "Number of dedicated cores".bold(),
        format!("{}", num_threads).bright_cyan()
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
            anonymous_proving(speed, dedicated_cores)
        }

        // If the user selected "connected"
        setup::SetupResult::Connected(node_id) => {
            println!(
                "\n===== {} =====\n\n",
                "Connected - Welcome to the Supercomputer"
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

            // Add a newline to separate the header from potential errors
            println!();

            authenticated_proving(&node_id, environment, speed, dedicated_cores).await
        }

        // If setup is invalid
        setup::SetupResult::Invalid => {
            error!("Invalid setup option selected.");
            Err("Invalid setup option selected".into())
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
) -> Result<(), Box<dyn StdError>> {
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
        .map_err(|e| ProverError::from(format!("Failed to load guest program: {}", e)))?;

    tx.send((
        worker_id,
        20,
        format!("Worker {}: Creating ZK proof with inputs...", worker_id),
    ))
    .await?;
    let (view, proof) = prover
        .prove_with_input::<(), u32>(&(), public_input)
        .map_err(|e| ProverError::from(format!("Failed to run prover: {}", e)))?;

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

    let code = view.exit_code().map(|u| u as i32).unwrap_or_else(|_err| {
        // We can't use await here, so just return -1 and let the error be handled below
        -1
    });

    if code != 0 {
        tx.send((
            worker_id,
            0,
            format!("Worker {}: Unexpected exit code: {}", worker_id, code),
        ))
        .await?;
        return Err(Box::new(ProverError::from(format!(
            "Unexpected exit code: {}",
            code
        ))));
    }

    tx.send((
        worker_id,
        80,
        format!("Worker {}: Serializing proof...", worker_id),
    ))
    .await?;
    let proof_bytes = serde_json::to_vec(&proof)
        .map_err(ProverError::from)?;
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
