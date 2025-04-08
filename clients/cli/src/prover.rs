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
fn run_prover(
    node_id: &str,
    environment: &config::Environment,
    speed: &crate::ProvingSpeed,
    public_input: u32,
    is_anonymous: bool,
) -> Result<(Vec<u8>, String), Box<dyn StdError + Send + Sync>> {
    // Set thread count based on speed
    let num_threads = match speed {
        crate::ProvingSpeed::Low => {
            let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
            (total_cores + 3) / 4 // Use 25% of cores, rounded up
        }
        crate::ProvingSpeed::Medium => {
            let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
            (total_cores + 1) / 2 // Use 50% of cores, rounded up
        }
        crate::ProvingSpeed::High => {
            let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
            (total_cores * 3 + 3) / 4 // Use 75% of cores, rounded up
        }
    };

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
            return anonymous_proving(speed);
        }
    };

    let public_input: u32 = proof_task
        .public_inputs
        .first()
        .cloned()
        .unwrap_or_default() as u32;

    let (proof_bytes, proof_hash) = run_prover(node_id, environment, speed, public_input, false)?;

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

fn anonymous_proving(speed: &crate::ProvingSpeed) -> Result<(), Box<dyn StdError + Send + Sync>> {
    // The 10th term of the Fibonacci sequence is 55
    let public_input: u32 = 9;
    let environment = config::Environment::Local;
    let node_id = "anonymous";

    let (proof_bytes, _) = run_prover(node_id, &environment, speed, public_input, true)?;

    // Track analytics for anonymous proving
    analytics::track(
        "cli_proof_node_anon".to_string(),
        "Completed anonymous proof".to_string(),
        serde_json::json!({
            "node_id": "anonymous",
            "speed": format!("{:?}", speed),
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

    // Print the selected speed setting
    println!(
        "{}: {}",
        "Proving speed".bold(),
        format!("{:?}", speed).bright_cyan()
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
            let speed_clone = speed.clone();

            // Set thread count based on speed
            let num_threads = match speed {
                crate::ProvingSpeed::Low => {
                    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
                    (total_cores + 3) / 4 // Use 25% of cores, rounded up
                }
                crate::ProvingSpeed::Medium => {
                    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
                    (total_cores + 1) / 2 // Use 50% of cores, rounded up
                }
                crate::ProvingSpeed::High => {
                    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
                    (total_cores * 3 + 3) / 4 // Use 75% of cores, rounded up
                }
            };

            println!(
                "{}: {}",
                "Number of parallel proof workers".bold(),
                num_threads.to_string().bright_cyan()
            );

            // Create a shared atomic counter for proofs
            let proof_counter = Arc::new(AtomicU32::new(1));

            // Create a multi-progress bar for all threads
            let mut stdout = stdout();
            let (tx, mut rx): MessageChannel = mpsc::channel(100);

            // Initialize progress bars with empty messages
            let mut progress_bars = vec![(0, String::new()); num_threads];
            execute!(&mut stdout, Hide)?;

            // Create a single thread to handle progress bar updates
            let progress_handle = tokio::spawn(async move {
                let mut stdout = std::io::stdout();
                let mut error_messages: Vec<(usize, String)> = Vec::new();
                let mut last_messages: Vec<String> = vec![String::new(); num_threads];

                while let Some((worker_id, position, message)) = rx.recv().await {
                    if worker_id < progress_bars.len() {
                        // Update progress bar or error message
                        if message.contains("Error") {
                            // Only keep the most recent error for each worker
                            if let Some(pos) =
                                error_messages.iter().position(|(id, _)| *id == worker_id)
                            {
                                error_messages.remove(pos);
                            }
                            error_messages.push((worker_id, message.clone()));
                            // Keep only the most recent 7 errors
                            if error_messages.len() > 7 {
                                error_messages.remove(0);
                            }
                        } else {
                            progress_bars[worker_id] = (position, message.clone());
                            last_messages[worker_id] = message;
                        }

                        // Get terminal dimensions
                        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
                        // Calculate start row to ensure all rows fit
                        let table_height = num_threads + 4; // Header (2) + rows (num_threads) + footer (1) + spacing (1)
                        let start_row = height.saturating_sub(table_height as u16);

                        // Calculate table width based on terminal width
                        let table_width = width.min(80);
                        let status_width = table_width.saturating_sub(12); // Account for worker column and borders

                        // Clear only the progress bar area
                        execute!(
                            &mut stdout,
                            MoveTo(0, start_row),
                            Clear(ClearType::FromCursorDown)
                        )?;

                        // Draw table header
                        execute!(
                            &mut stdout,
                            MoveTo(0, start_row),
                            Print(format!(
                                "╔{}╤{}╗",
                                "═".repeat(8),
                                "═".repeat(status_width as usize - 7)
                            )),
                            MoveTo(0, start_row + 1),
                            Print(format!(
                                "║ Worker │ Status{}║",
                                " ".repeat(status_width as usize - 14)
                            )),
                            MoveTo(0, start_row + 2),
                            Print(format!(
                                "╠{}╪{}╣",
                                "═".repeat(8),
                                "═".repeat(status_width as usize - 7)
                            ))
                        )?;

                        // Draw all progress bars, including inactive ones
                        for (i, (_pos, msg)) in progress_bars.iter().enumerate().take(num_threads) {
                            let dot_color = if msg.contains("Error") || msg.contains("Retrying") {
                                Color::Red
                            } else if msg.contains("Fetching") {
                                Color::Yellow
                            } else if msg.is_empty() {
                                Color::White // Use white for inactive workers
                            } else {
                                Color::Green
                            };

                            let display_msg = if msg.is_empty() {
                                format!("[Worker {:02}] Waiting for task...", i)
                            } else if msg.contains("Error") {
                                msg.split(" at ").next().unwrap_or(msg).to_string()
                            } else {
                                msg.to_string()
                            };

                            // Truncate message to fit in available width
                            let truncated_msg = if display_msg.len() > status_width as usize - 2 {
                                format!("{}...", &display_msg[..status_width as usize - 5])
                            } else {
                                display_msg
                            };

                            execute!(
                                &mut stdout,
                                MoveTo(0, start_row + 3 + i as u16),
                                Print("║ "),
                                SetForegroundColor(dot_color),
                                Print("● "),
                                ResetColor,
                                Print(format!("{:02}   │ ", i)),
                                Print(format!(
                                    "{:<width$}",
                                    truncated_msg,
                                    width = status_width as usize - 8
                                )),
                                Print("║")
                            )?;
                        }

                        // Draw table footer
                        execute!(
                            &mut stdout,
                            MoveTo(0, start_row + 3 + num_threads as u16),
                            Print(format!(
                                "╚{}╧{}╝",
                                "═".repeat(8),
                                "═".repeat(status_width as usize - 7)
                            ))
                        )?;

                        stdout.flush()?;
                    }
                }
                Ok::<_, std::io::Error>(())
            });

            let mut handles = Vec::new();

            // Spawn worker threads
            for worker_id in 0..num_threads {
                let tx = tx.clone();
                let proof_counter = Arc::clone(&proof_counter);
                let client_id = client_id.clone();
                let speed_clone = speed_clone.clone();
                let environment = environment.clone();
                let node_id = "anonymous".to_string(); // Add node_id for anonymous mode

                let handle: tokio::task::JoinHandle<Result<(), Box<ProverError>>> = tokio::spawn(
                    async move {
                        loop {
                            let client = OrchestratorClient::new(environment.clone());

                            // Create a new thread pool for this worker
                            let pool = match ThreadPoolBuilder::new()
                                .num_threads(1) // Each worker gets its own single-threaded pool
                                .build()
                            {
                                Ok(pool) => pool,
                                Err(e) => {
                                    let _ = tx
                                        .send((
                                            worker_id,
                                            0,
                                            format!("Error: Failed to create thread pool - {}", e),
                                        ))
                                        .await;
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    return Ok(());
                                }
                            };

                            let current_proof = proof_counter.load(Ordering::SeqCst);
                            // Send initial status with current proof count
                            if let Err(e) = tx.send((worker_id, 0, "Fetching task...".to_string())).await {
                                error!("Worker {} channel error: {}", worker_id, e);
                                return Ok(());
                            }

                            // Create a timeout future for task fetching
                            let fetch_timeout = tokio::time::sleep(Duration::from_secs(5));
                            let fetch_task = client.get_proof_task(&node_id);

                            match tokio::select! {
                                _ = fetch_timeout => {
                                    if let Err(e) = tx.send((worker_id, 0, "Error: Task fetch timeout".to_string())).await {
                                        error!("Worker {} channel error: {}", worker_id, e);
                                        return Ok(());
                                    }
                                    // Show red dot during backoff
                                    for i in 0..5 {
                                        if let Err(e) = tx.send((worker_id, 0, format!("Retrying in {}s...", 2u64.pow(i)))).await {
                                            error!("Worker {} channel error: {}", worker_id, e);
                                            break;
                                        }
                                        tokio::time::sleep(Duration::from_secs(2u64.pow(i))).await;
                                    }
                                    return Ok(());
                                }
                                result = fetch_task => result
                            } {
                                Ok(proof_task) => {
                                    let public_input: u32 = proof_task
                                        .public_inputs
                                        .first()
                                        .cloned()
                                        .unwrap_or_default()
                                        as u32;
                                    let task_id = if proof_task.task_id > 0 {
                                        format!("task {}", proof_task.task_id)
                                    } else {
                                        format!("proof #{}", current_proof)
                                    };

                                    // Process the proof task using the thread pool
                                    let result = pool.install(|| -> Result<(Vec<u8>, String), ProverError> {
                                    let start_time = Instant::now();

                                    // Create a channel for progress updates
                                    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<()>();
                                    
                                    // Clone tx for the progress thread
                                    let tx_for_progress = tx.clone();
                                    let proof_counter_for_progress = Arc::clone(&proof_counter);
                                    
                                    // Spawn a thread to handle progress updates
                                    let progress_thread = std::thread::spawn(move || {
                                        let mut cycle_count = 0;
                                        let mut total_cycles = 0;
                                        let mut last_update = Instant::now();
                                        let start_time = Instant::now();

                                        // Send initial status
                                        let _current_proof = proof_counter_for_progress.load(Ordering::SeqCst);
                                        if let Err(e) = tx_for_progress.blocking_send((worker_id, 0, "Loading guest program...".to_string())) {
                                            error!("Worker {} progress channel error: {}", worker_id, e);
                                            return;
                                        }

                                        while progress_rx.recv().is_ok() {
                                            cycle_count += 1;
                                            total_cycles += 1;
                                            let now = Instant::now();
                                            let elapsed = now.duration_since(last_update);
                                            if elapsed.as_millis() >= 100 { // Update every 100ms
                                                let cycles_per_sec = cycle_count as f64 / elapsed.as_secs_f64();
                                                let total_time = now.duration_since(start_time);
                                                // Always get the latest proof count
                                                let _current_proof = proof_counter_for_progress.load(Ordering::SeqCst);
                                                if let Err(e) = tx_for_progress.blocking_send((worker_id, 0, 
                                                    format!("Proving at {:.2} cycles/sec ({} cycles in {:.1}s)", 
                                                        cycles_per_sec, total_cycles, total_time.as_secs_f64()))) {
                                                    error!("Worker {} progress channel error: {}", worker_id, e);
                                                    return;
                                                }
                                                cycle_count = 0;
                                                last_update = now;
                                            }
                                        }
                                    });

                                    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                        .join("assets")
                                        .join("fib_input");

                                    let prover = Stwo::<Local>::new_from_file(&elf_file_path)
                                        .map_err(|e| ProverError::from(format!("Failed to load guest program: {}", e)))?;

                                    // Start sending progress updates
                                    let progress_tx_clone = progress_tx.clone();
                                    let update_thread = std::thread::spawn(move || {
                                        loop {
                                            if progress_tx_clone.send(()).is_err() {
                                                break;
                                            }
                                            std::thread::sleep(Duration::from_millis(1));
                                        }
                                    });

                                    let (view, proof) = prover
                                        .prove_with_input::<(), u32>(&(), &public_input)
                                        .map_err(|e| ProverError::from(format!("Failed to run prover: {}", e)))?;

                                    // Signal the progress threads to stop
                                    drop(progress_tx);
                                    let _ = progress_thread.join();
                                    let _ = update_thread.join();

                                    let code = view.exit_code()
                                        .map(|u| u as i32)
                                        .unwrap_or_else(|_err| -1);
                                    
                                    if code != 0 {
                                        return Err(ProverError::from(format!("Unexpected exit code: {}", code)));
                                    }

                                    if let Err(e) = tx.blocking_send((worker_id, 0, format!("Serializing proof for {}...", task_id))) {
                                        error!("Worker {} channel error: {}", worker_id, e);
                                        return Err(ProverError::from("Channel closed".to_string()));
                                    }

                                    let proof_bytes = serde_json::to_vec(&proof)
                                        .map_err(ProverError::from)?;
                                    let proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

                                    let duration = start_time.elapsed();
                                    if let Err(e) = tx.blocking_send((worker_id, 0, format!("Completed {} in {:.2?}", task_id, duration))) {
                                        error!("Worker {} channel error: {}", worker_id, e);
                                        return Err(ProverError::from("Channel closed".to_string()));
                                    }

                                    Ok((proof_bytes, proof_hash))
                                });

                                    match result {
                                        Ok((proof_bytes, _proof_hash)) => {
                                            // Show preparing to submit message
                                            if let Err(e) = tx
                                                .send((
                                                    worker_id,
                                                    0,
                                                    "Preparing to submit proof...".to_string(),
                                                ))
                                                .await
                                            {
                                                error!(
                                                    "Worker {} channel error: {}",
                                                    worker_id, e
                                                );
                                                return Ok(());
                                            }

                                            // Submit the proof to orchestrator
                                            match client
                                                .submit_proof(
                                                    &node_id,
                                                    &_proof_hash,
                                                    proof_bytes,
                                                    proof_task.task_id,
                                                )
                                                .await
                                            {
                                                Ok(_) => {
                                                    // Increment the counter after successful submission
                                                    let _next_proof = proof_counter
                                                        .fetch_add(1, Ordering::SeqCst)
                                                        + 1;

                                                    // Show success message
                                                    if let Err(e) = tx
                                                        .send((
                                                            worker_id,
                                                            0,
                                                            "Successfully submitted proof".to_string(),
                                                        ))
                                                        .await
                                                    {
                                                        error!(
                                                            "Worker {} channel error: {}",
                                                            worker_id, e
                                                        );
                                                        return Ok(());
                                                    }

                                                    // Pause for 2 seconds
                                                    tokio::time::sleep(Duration::from_secs(2))
                                                        .await;

                                                    // Show waiting message with next proof number
                                                    if let Err(e) = tx
                                                        .send((
                                                            worker_id,
                                                            0,
                                                            "Waiting before next proof...".to_string(),
                                                        ))
                                                        .await
                                                    {
                                                        error!(
                                                            "Worker {} channel error: {}",
                                                            worker_id, e
                                                        );
                                                        return Ok(());
                                                    }

                                                    // Track analytics with the current proof count
                                                    analytics::track(
                                                        EVENT_NAME.to_string(),
                                                        format!(
                                                            "Completed proof iteration #{}",
                                                            current_proof
                                                        ),
                                                        serde_json::json!({
                                                            "node_id": node_id,
                                                            "proof_count": current_proof,
                                                            "speed": format!("{:?}", speed_clone),
                                                            "worker_id": worker_id,
                                                            "task_id": task_id,
                                                        }),
                                                        false,
                                                        &environment,
                                                        client_id.clone(),
                                                    );
                                                }
                                                Err(e) => {
                                                    tx.send((worker_id, 0, format!("Error submitting proof for {} - {}", task_id, e))).await.unwrap();
                                                    // Show red dot during backoff
                                                    for i in 0..5 {
                                                        tx.send((
                                                            worker_id,
                                                            0,
                                                            format!(
                                                                "Retrying in {}s...",
                                                                2u64.pow(i)
                                                            ),
                                                        ))
                                                        .await
                                                        .unwrap();
                                                        tokio::time::sleep(
                                                            Duration::from_secs(2u64.pow(i)),
                                                        )
                                                        .await;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tx.send((
                                                worker_id,
                                                0,
                                                format!(
                                                    "Error processing task {} - {}",
                                                    task_id, e
                                                ),
                                            ))
                                            .await
                                            .unwrap();
                                            return Ok(());
                                        }
                                    }
                                }
                                Err(e) => {
                                    tx.send((
                                        worker_id,
                                        0,
                                        format!("Error fetching task - {}", e),
                                    ))
                                    .await
                                    .unwrap();
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                    return Ok(());
                                }
                            }
                        }
                    },
                );

                handles.push(handle);
            }

            // Wait for all workers to complete (they won't, but this keeps the main thread alive)
            for handle in handles {
                let _ = handle.await?;
            }
            let _ = progress_handle.await?;
            execute!(&mut stdout, Show)?;
            Ok(())
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

            let client_id = format!("{:x}", md5::compute(node_id.as_bytes()));
            let speed_clone = speed.clone();

            // Set thread count based on speed
            let num_threads = match speed {
                crate::ProvingSpeed::Low => {
                    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
                    (total_cores + 3) / 4 // Use 25% of cores, rounded up
                }
                crate::ProvingSpeed::Medium => {
                    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
                    (total_cores + 1) / 2 // Use 50% of cores, rounded up
                }
                crate::ProvingSpeed::High => {
                    let total_cores = thread::available_parallelism().map_or(1, |n| n.get());
                    (total_cores * 3 + 3) / 4 // Use 75% of cores, rounded up
                }
            };

            println!(
                "{}: {}",
                "Number of parallel proof workers".bold(),
                num_threads.to_string().bright_cyan()
            );

            // Create a shared atomic counter for proofs
            let proof_counter = Arc::new(AtomicU32::new(1));

            // Create a multi-progress bar for all threads
            let mut stdout = stdout();
            let (tx, mut rx): MessageChannel = mpsc::channel(100);

            // Initialize progress bars with empty messages
            let mut progress_bars = vec![(0, String::new()); num_threads];
            execute!(&mut stdout, Hide)?;

            // Create a single thread to handle progress bar updates
            let progress_handle = tokio::spawn(async move {
                let mut stdout = std::io::stdout();
                let mut error_messages: Vec<(usize, String)> = Vec::new();
                let mut last_messages: Vec<String> = vec![String::new(); num_threads];

                while let Some((worker_id, position, message)) = rx.recv().await {
                    if worker_id < progress_bars.len() {
                        // Update progress bar or error message
                        if message.contains("Error") {
                            // Only keep the most recent error for each worker
                            if let Some(pos) =
                                error_messages.iter().position(|(id, _)| *id == worker_id)
                            {
                                error_messages.remove(pos);
                            }
                            error_messages.push((worker_id, message.clone()));
                            // Keep only the most recent 7 errors
                            if error_messages.len() > 7 {
                                error_messages.remove(0);
                            }
                        } else {
                            progress_bars[worker_id] = (position, message.clone());
                            last_messages[worker_id] = message;
                        }

                        // Get terminal dimensions
                        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
                        // Calculate start row to ensure all rows fit
                        let table_height = num_threads + 4; // Header (2) + rows (num_threads) + footer (1) + spacing (1)
                        let start_row = height.saturating_sub(table_height as u16);

                        // Calculate table width based on terminal width
                        let table_width = width.min(80);
                        let status_width = table_width.saturating_sub(12); // Account for worker column and borders

                        // Clear only the progress bar area
                        execute!(
                            &mut stdout,
                            MoveTo(0, start_row),
                            Clear(ClearType::FromCursorDown)
                        )?;

                        // Draw table header
                        execute!(
                            &mut stdout,
                            MoveTo(0, start_row),
                            Print(format!(
                                "╔{}╤{}╗",
                                "═".repeat(8),
                                "═".repeat(status_width as usize - 7)
                            )),
                            MoveTo(0, start_row + 1),
                            Print(format!(
                                "║ Worker │ Status{}║",
                                " ".repeat(status_width as usize - 14)
                            )),
                            MoveTo(0, start_row + 2),
                            Print(format!(
                                "╠{}╪{}╣",
                                "═".repeat(8),
                                "═".repeat(status_width as usize - 7)
                            ))
                        )?;

                        // Draw all progress bars, including inactive ones
                        for (i, (_pos, msg)) in progress_bars.iter().enumerate().take(num_threads) {
                            let dot_color = if msg.contains("Error") || msg.contains("Retrying") {
                                Color::Red
                            } else if msg.contains("Fetching") {
                                Color::Yellow
                            } else if msg.is_empty() {
                                Color::White // Use white for inactive workers
                            } else {
                                Color::Green
                            };

                            let display_msg = if msg.is_empty() {
                                format!("[Worker {:02}] Waiting for task...", i)
                            } else if msg.contains("Error") {
                                msg.split(" at ").next().unwrap_or(msg).to_string()
                            } else {
                                msg.to_string()
                            };

                            // Truncate message to fit in available width
                            let truncated_msg = if display_msg.len() > status_width as usize - 2 {
                                format!("{}...", &display_msg[..status_width as usize - 5])
                            } else {
                                display_msg
                            };

                            execute!(
                                &mut stdout,
                                MoveTo(0, start_row + 3 + i as u16),
                                Print("║ "),
                                SetForegroundColor(dot_color),
                                Print("● "),
                                ResetColor,
                                Print(format!("{:02}   │ ", i)),
                                Print(format!(
                                    "{:<width$}",
                                    truncated_msg,
                                    width = status_width as usize - 8
                                )),
                                Print("║")
                            )?;
                        }

                        // Draw table footer
                        execute!(
                            &mut stdout,
                            MoveTo(0, start_row + 3 + num_threads as u16),
                            Print(format!(
                                "╚{}╧{}╝",
                                "═".repeat(8),
                                "═".repeat(status_width as usize - 7)
                            ))
                        )?;

                        stdout.flush()?;
                    }
                }
                Ok::<_, std::io::Error>(())
            });

            let mut handles = Vec::new();

            // Spawn worker threads
            for worker_id in 0..num_threads {
                let node_id = node_id.clone();
                let environment = environment.clone();
                let tx = tx.clone();
                let proof_counter = Arc::clone(&proof_counter);
                let client_id = client_id.clone();
                let speed_clone = speed_clone.clone();

                let handle: tokio::task::JoinHandle<Result<(), Box<ProverError>>> = tokio::spawn(
                    async move {
                        loop {
                            let client = OrchestratorClient::new(environment.clone());

                            // Create a new thread pool for this worker
                            let pool = match ThreadPoolBuilder::new()
                                .num_threads(1) // Each worker gets its own single-threaded pool
                                .build()
                            {
                                Ok(pool) => pool,
                                Err(e) => {
                                    let _ = tx
                                        .send((
                                            worker_id,
                                            0,
                                            format!("Error: Failed to create thread pool - {}", e),
                                        ))
                                        .await;
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    return Ok(());
                                }
                            };

                            let current_proof = proof_counter.load(Ordering::SeqCst);
                            // Send initial status with current proof count
                            if let Err(e) = tx.send((worker_id, 0, "Fetching task...".to_string())).await {
                                error!("Worker {} channel error: {}", worker_id, e);
                                return Ok(());
                            }

                            // Create a timeout future for task fetching
                            let fetch_timeout = tokio::time::sleep(Duration::from_secs(5));
                            let fetch_task = client.get_proof_task(&node_id);

                            match tokio::select! {
                                _ = fetch_timeout => {
                                    if let Err(e) = tx.send((worker_id, 0, "Error: Task fetch timeout".to_string())).await {
                                        error!("Worker {} channel error: {}", worker_id, e);
                                        return Ok(());
                                    }
                                    // Show red dot during backoff
                                    for i in 0..5 {
                                        if let Err(e) = tx.send((worker_id, 0, format!("Retrying in {}s...", 2u64.pow(i)))).await {
                                            error!("Worker {} channel error: {}", worker_id, e);
                                            break;
                                        }
                                        tokio::time::sleep(Duration::from_secs(2u64.pow(i))).await;
                                    }
                                    return Ok(());
                                }
                                result = fetch_task => result
                            } {
                                Ok(proof_task) => {
                                    let public_input: u32 = proof_task
                                        .public_inputs
                                        .first()
                                        .cloned()
                                        .unwrap_or_default()
                                        as u32;
                                    let task_id = if proof_task.task_id > 0 {
                                        format!("task {}", proof_task.task_id)
                                    } else {
                                        format!("proof #{}", current_proof)
                                    };

                                    // Process the proof task using the thread pool
                                    let result = pool.install(|| -> Result<(Vec<u8>, String), ProverError> {
                                    let start_time = Instant::now();

                                    // Create a channel for progress updates
                                    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<()>();
                                    
                                    // Clone tx for the progress thread
                                    let tx_for_progress = tx.clone();
                                    let proof_counter_for_progress = Arc::clone(&proof_counter);
                                    
                                    // Spawn a thread to handle progress updates
                                    let progress_thread = std::thread::spawn(move || {
                                        let mut cycle_count = 0;
                                        let mut total_cycles = 0;
                                        let mut last_update = Instant::now();
                                        let start_time = Instant::now();

                                        // Send initial status
                                        let _current_proof = proof_counter_for_progress.load(Ordering::SeqCst);
                                        if let Err(e) = tx_for_progress.blocking_send((worker_id, 0, "Loading guest program...".to_string())) {
                                            error!("Worker {} progress channel error: {}", worker_id, e);
                                            return;
                                        }

                                        while progress_rx.recv().is_ok() {
                                            cycle_count += 1;
                                            total_cycles += 1;
                                            let now = Instant::now();
                                            let elapsed = now.duration_since(last_update);
                                            if elapsed.as_millis() >= 100 { // Update every 100ms
                                                let cycles_per_sec = cycle_count as f64 / elapsed.as_secs_f64();
                                                let total_time = now.duration_since(start_time);
                                                // Always get the latest proof count
                                                let _current_proof = proof_counter_for_progress.load(Ordering::SeqCst);
                                                if let Err(e) = tx_for_progress.blocking_send((worker_id, 0, 
                                                    format!("Proving at {:.2} cycles/sec ({} cycles in {:.1}s)", 
                                                        cycles_per_sec, total_cycles, total_time.as_secs_f64()))) {
                                                    error!("Worker {} progress channel error: {}", worker_id, e);
                                                    return;
                                                }
                                                cycle_count = 0;
                                                last_update = now;
                                            }
                                        }
                                    });

                                    let elf_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                        .join("assets")
                                        .join("fib_input");

                                    let prover = Stwo::<Local>::new_from_file(&elf_file_path)
                                        .map_err(|e| ProverError::from(format!("Failed to load guest program: {}", e)))?;

                                    // Start sending progress updates
                                    let progress_tx_clone = progress_tx.clone();
                                    let update_thread = std::thread::spawn(move || {
                                        loop {
                                            if progress_tx_clone.send(()).is_err() {
                                                break;
                                            }
                                            std::thread::sleep(Duration::from_millis(1));
                                        }
                                    });

                                    let (view, proof) = prover
                                        .prove_with_input::<(), u32>(&(), &public_input)
                                        .map_err(|e| ProverError::from(format!("Failed to run prover: {}", e)))?;

                                    // Signal the progress threads to stop
                                    drop(progress_tx);
                                    let _ = progress_thread.join();
                                    let _ = update_thread.join();

                                    let code = view.exit_code()
                                        .map(|u| u as i32)
                                        .unwrap_or_else(|_err| -1);
                                    
                                    if code != 0 {
                                        return Err(ProverError::from(format!("Unexpected exit code: {}", code)));
                                    }

                                    if let Err(e) = tx.blocking_send((worker_id, 0, format!("Serializing proof for {}...", task_id))) {
                                        error!("Worker {} channel error: {}", worker_id, e);
                                        return Err(ProverError::from("Channel closed".to_string()));
                                    }

                                    let proof_bytes = serde_json::to_vec(&proof)
                                        .map_err(ProverError::from)?;
                                    let _proof_hash = format!("{:x}", Keccak256::digest(&proof_bytes));

                                    let duration = start_time.elapsed();
                                    if let Err(e) = tx.blocking_send((worker_id, 0, format!("Completed {} in {:.2?}", task_id, duration))) {
                                        error!("Worker {} channel error: {}", worker_id, e);
                                        return Err(ProverError::from("Channel closed".to_string()));
                                    }

                                    Ok((proof_bytes, _proof_hash))
                                });

                                    match result {
                                        Ok((proof_bytes, _proof_hash)) => {
                                            // Show preparing to submit message
                                            if let Err(e) = tx
                                                .send((
                                                    worker_id,
                                                    0,
                                                    "Preparing to submit proof...".to_string(),
                                                ))
                                                .await
                                            {
                                                error!(
                                                    "Worker {} channel error: {}",
                                                    worker_id, e
                                                );
                                                return Ok(());
                                            }

                                            // Submit the proof to orchestrator
                                            match client
                                                .submit_proof(
                                                    &node_id,
                                                    &_proof_hash,
                                                    proof_bytes,
                                                    proof_task.task_id,
                                                )
                                                .await
                                            {
                                                Ok(_) => {
                                                    // Increment the counter after successful submission
                                                    let _next_proof = proof_counter
                                                        .fetch_add(1, Ordering::SeqCst)
                                                        + 1;

                                                    // Show success message
                                                    if let Err(e) = tx
                                                        .send((
                                                            worker_id,
                                                            0,
                                                            "Successfully submitted proof".to_string(),
                                                        ))
                                                        .await
                                                    {
                                                        error!(
                                                            "Worker {} channel error: {}",
                                                            worker_id, e
                                                        );
                                                        return Ok(());
                                                    }

                                                    // Pause for 2 seconds
                                                    tokio::time::sleep(Duration::from_secs(2))
                                                        .await;

                                                    // Show waiting message with next proof number
                                                    if let Err(e) = tx
                                                        .send((
                                                            worker_id,
                                                            0,
                                                            "Waiting before next proof...".to_string(),
                                                        ))
                                                        .await
                                                    {
                                                        error!(
                                                            "Worker {} channel error: {}",
                                                            worker_id, e
                                                        );
                                                        return Ok(());
                                                    }

                                                    // Track analytics with the current proof count
                                                    analytics::track(
                                                        EVENT_NAME.to_string(),
                                                        format!(
                                                            "Completed proof iteration #{}",
                                                            current_proof
                                                        ),
                                                        serde_json::json!({
                                                            "node_id": node_id,
                                                            "proof_count": current_proof,
                                                            "speed": format!("{:?}", speed_clone),
                                                            "worker_id": worker_id,
                                                            "task_id": task_id,
                                                        }),
                                                        false,
                                                        &environment,
                                                        client_id.clone(),
                                                    );
                                                }
                                                Err(e) => {
                                                    tx.send((worker_id, 0, format!("Error submitting proof for {} - {}", task_id, e))).await.unwrap();
                                                    // Show red dot during backoff
                                                    for i in 0..5 {
                                                        tx.send((
                                                            worker_id,
                                                            0,
                                                            format!(
                                                                "Retrying in {}s...",
                                                                2u64.pow(i)
                                                            ),
                                                        ))
                                                        .await
                                                        .unwrap();
                                                        tokio::time::sleep(
                                                            Duration::from_secs(2u64.pow(i)),
                                                        )
                                                        .await;
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tx.send((
                                                worker_id,
                                                0,
                                                format!(
                                                    "Error processing task {} - {}",
                                                    task_id, e
                                                ),
                                            ))
                                            .await
                                            .unwrap();
                                            return Ok(());
                                        }
                                    }
                                }
                                Err(e) => {
                                    tx.send((
                                        worker_id,
                                        0,
                                        format!("Error fetching task - {}", e),
                                    ))
                                    .await
                                    .unwrap();
                                    tokio::time::sleep(Duration::from_secs(2)).await;
                                    return Ok(());
                                }
                            }
                        }
                    },
                );

                handles.push(handle);
            }

            // Wait for all workers to complete (they won't, but this keeps the main thread alive)
            for handle in handles {
                let _ = handle.await?;
            }
            let _ = progress_handle.await?;
            execute!(&mut stdout, Show)?;
            Ok(())
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
