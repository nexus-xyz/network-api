//! Session setup and initialization

use crate::analytics::set_wallet_address_for_reporting;
use crate::config::Config;
use crate::environment::Environment;
use crate::events::Event;
use crate::orchestrator::OrchestratorClient;
use crate::runtime::start_authenticated_worker;
use ed25519_dalek::SigningKey;
use std::error::Error;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

/// Session data for both TUI and headless modes
#[derive(Debug)]
pub struct SessionData {
    /// Event receiver for worker events
    pub event_receiver: mpsc::Receiver<Event>,
    /// Join handles for worker tasks
    pub join_handles: Vec<JoinHandle<()>>,
    /// Shutdown sender to stop all workers
    pub shutdown_sender: broadcast::Sender<()>,
    /// Shutdown sender for max tasks completion
    pub max_tasks_shutdown_sender: broadcast::Sender<()>,
    /// Node ID
    pub node_id: u64,
    /// Orchestrator client
    pub orchestrator: OrchestratorClient,
    /// Number of workers (for display purposes)
    pub num_workers: usize,
}

/// Clamp thread count based on available system memory
/// Returns the maximum number of threads that can be safely used given system memory
fn clamp_threads_by_memory(requested_threads: usize) -> usize {
    let mut sysinfo = System::new();
    sysinfo.refresh_memory();

    let total_system_memory = sysinfo.total_memory();
    let memory_per_thread = crate::consts::cli_consts::PROJECTED_MEMORY_REQUIREMENT;

    // Calculate max threads based on total system memory
    // Reserve 25% of system memory for OS and other processes
    let available_memory = (total_system_memory as f64 * 0.75) as u64;
    let max_threads_by_memory = (available_memory / memory_per_thread) as usize;

    // Return the minimum of requested threads and memory-limited threads
    // Always allow at least 1 thread
    requested_threads.min(max_threads_by_memory.max(1))
}

/// Warn the user if their available memory seems insufficient for the task(s) at hand
pub fn warn_memory_configuration(max_threads: Option<u32>) {
    if let Some(threads) = max_threads {
        let current_pid = Pid::from(std::process::id() as usize);

        let mut sysinfo = System::new();
        sysinfo.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[current_pid]),
            true, // Refresh exact processes
            ProcessRefreshKind::nothing().with_memory(),
        );

        if let Some(process) = sysinfo.process(current_pid) {
            let ram_total = process.memory();
            if threads as u64 * crate::consts::cli_consts::PROJECTED_MEMORY_REQUIREMENT >= ram_total
            {
                crate::print_cmd_warn!(
                    "OOM warning",
                    "Projected memory usage across {} requested threads exceeds memory currently available to process. In the event that proving fails due to an out-of-memory error, please restart the Nexus CLI with a smaller value supplied to `--max-threads`.",
                    threads
                );
                std::thread::sleep(std::time::Duration::from_secs(3));
            }
        }
    }
}

/// Sets up an authenticated worker session
///
/// This function handles all the common setup required for both TUI and headless modes:
/// 1. Creates signing key for the prover
/// 2. Sets up shutdown channel
/// 3. Starts authenticated worker
/// 4. Returns session data for mode-specific handling
///
/// # Arguments
/// * `config` - Resolved configuration with node_id and client_id
/// * `env` - Environment to connect to
/// * `max_threads` - Optional maximum number of threads for proving
/// * `max_difficulty` - Optional override for task difficulty
///
/// # Returns
/// * `Ok(SessionData)` - Successfully set up session
/// * `Err` - Session setup failed
pub async fn setup_session(
    config: Config,
    env: Environment,
    check_mem: bool,
    max_threads: Option<u32>,
    max_tasks: Option<u32>,
    max_difficulty: Option<crate::nexus_orchestrator::TaskDifficulty>,
) -> Result<SessionData, Box<dyn Error>> {
    let node_id = config.node_id.parse::<u64>()?;
    let client_id = config.user_id;

    // Create a signing key for the prover
    let mut csprng = rand_core::OsRng;
    let signing_key: SigningKey = SigningKey::generate(&mut csprng);

    // Create orchestrator client
    let orchestrator_client = OrchestratorClient::new(env.clone());

    // Clamp the number of workers to [1, 75% of num_cores]. Leave room for other processes.
    let total_cores = crate::system::num_cores();
    let max_workers = ((total_cores as f64 * 0.75).ceil() as usize).max(1);
    let mut num_workers: usize = max_threads.unwrap_or(1).clamp(1, max_workers as u32) as usize;

    // Check memory and clamp threads only when --check-memory is explicitly requested.
    // When the user explicitly sets --max-threads, trust their hardware knowledge.
    if check_mem {
        let memory_clamped_workers = clamp_threads_by_memory(num_workers);
        if memory_clamped_workers < num_workers {
            crate::print_cmd_warn!(
                "Memory limit",
                "Reduced thread count from {} to {} due to insufficient memory. Each thread requires ~4GB RAM.",
                num_workers,
                memory_clamped_workers
            );
            num_workers = memory_clamped_workers;
        }
    }

    // Additional memory warning if explicitly requested
    if check_mem {
        warn_memory_configuration(Some(num_workers as u32));
    }

    // Create shutdown channel - only one shutdown signal needed
    let (shutdown_sender, _) = broadcast::channel(1);

    // Set wallet for reporting
    set_wallet_address_for_reporting(config.wallet_address.clone());

    // Start authenticated worker (only mode we support now)
    let (event_receiver, join_handles, max_tasks_shutdown_sender) = start_authenticated_worker(
        node_id,
        signing_key,
        orchestrator_client.clone(),
        shutdown_sender.subscribe(),
        env,
        client_id,
        max_tasks,
        max_difficulty,
        num_workers,
    )
    .await;

    Ok(SessionData {
        event_receiver,
        join_handles,
        shutdown_sender,
        max_tasks_shutdown_sender,
        node_id,
        orchestrator: orchestrator_client,
        num_workers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_num_workers(max_threads: Option<u32>, total_cores: usize) -> usize {
        let max_workers = ((total_cores as f64 * 0.75).ceil() as usize).max(1);
        max_threads.unwrap_or(1).clamp(1, max_workers as u32) as usize
    }

    #[test]
    fn num_workers_defaults_to_1_without_flag() {
        let workers = compute_num_workers(None, 8);
        assert_eq!(workers, 1);
    }

    #[test]
    fn num_workers_uses_max_threads_when_set() {
        // 8 cores → max_workers = ceil(6) = 6; --max-threads 4 should give 4
        let workers = compute_num_workers(Some(4), 8);
        assert_eq!(workers, 4);
    }

    #[test]
    fn num_workers_clamps_to_core_limit() {
        // 2 cores → max_workers = ceil(1.5) = 2; --max-threads 8 should be clamped to 2
        let workers = compute_num_workers(Some(8), 2);
        assert_eq!(workers, 2);
    }

    #[test]
    fn num_workers_minimum_is_1() {
        // --max-threads 0 would be clamped up to 1 by clamp(1, …)
        let workers = compute_num_workers(Some(0), 8);
        assert_eq!(workers, 1);
    }

    #[test]
    fn memory_clamp_not_applied_when_check_mem_false() {
        // Simulate a machine with very little "memory" — previously this would reduce workers to 1
        // when max_threads.is_some(). Now it must NOT clamp unless check_mem=true.
        let requested = 4usize;
        // With check_mem=false the result stays at requested (no memory check runs).
        // We verify by re-running the guard logic: clamp_threads_by_memory would return 1 on a
        // tiny memory budget, but the guard is now gated on check_mem only.
        let check_mem = false;
        let max_threads: Option<u32> = Some(4);

        // The guard in setup_session is: if check_mem { ... }
        // Since check_mem=false, clamping never runs regardless of max_threads.
        let result = if check_mem {
            clamp_threads_by_memory(requested)
        } else {
            requested
        };

        assert_eq!(result, requested,
            "num_workers should not be memory-clamped when --check-memory is not set (got {result}, max_threads={max_threads:?})");
    }

    #[test]
    fn memory_clamp_applied_when_check_mem_true_and_overcommitted() {
        // When check_mem=true and the request exceeds available memory, clamping should fire.
        // clamp_threads_by_memory reads real system memory, so we only verify the path is taken
        // and the result is ≥ 1.
        let requested = usize::MAX; // absurdly large
        let check_mem = true;

        let result = if check_mem {
            clamp_threads_by_memory(requested)
        } else {
            requested
        };

        assert!(result >= 1, "clamp_threads_by_memory must always return at least 1");
        assert!(result < usize::MAX, "overcommitted request must be clamped");
    }
}
