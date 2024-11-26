use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use crate::utils::updater::{
    get_git_version, get_repo_path, get_update_interval, is_test_mode, number_to_version,
    version_to_number, write_version_to_file, BLUE, REMOTE_REPO, RESET,
};

// function to get the current git tag version from the file or git
fn get_current_version() -> Result<u64, Box<dyn std::error::Error>> {
    let git_version = get_git_version()?;

    println!(
        "{}[auto-updater thread]{} Current version from git: {}",
        BLUE, RESET, git_version
    );

    let version_num = version_to_number(&git_version);
    write_version_to_file(&git_version)?;

    println!(
        "{}[auto-updater thread]{} Wrote version to file: {}",
        BLUE,
        RESET,
        number_to_version(version_num)
    );

    Ok(version_num)
}

// function to start the periodic update checker thread
// This is the function that is called by the main thread in prover.rs
pub fn start_periodic_updates() {
    println!(
        "{}[auto-updater thread]{} Starting periodic CLI updates...",
        BLUE, RESET
    );

    // Initialize the CLI version that will be shared between:
    // 1. Main thread (which runs the CLI's core functionality)
    // 2. Update checker thread (which periodically checks for and applies updates)
    let cli_version_shared_by_threads = Arc::new(AtomicU64::new(
        get_current_version().unwrap_or_else(|_| version_to_number("0.3.5")),
    ));

    // Clone Arc for the update checker thread
    let update_checker_version = cli_version_shared_by_threads.clone();

    thread::spawn(move || {
        println!(
            "{}[auto-updater thread]{} Update checker thread started!",
            BLUE, RESET
        );
        loop {
            if let Err(e) = check_if_update_needed_and_update(&update_checker_version) {
                eprintln!(
                    "{}[auto-updater thread]{} Update check failed: {}",
                    BLUE, RESET, e
                );
            }
            let update_interval = get_update_interval();
            // Sleep for the update interval
            println!(
                "{}[auto-updater thread]{} Checking for new CLI updated version in {} seconds...",
                BLUE, RESET, update_interval
            );
            thread::sleep(Duration::from_secs(update_interval));
        }
    });
}

/// function to check for updates and apply them if needed
fn check_if_update_needed_and_update(
    current_version: &Arc<AtomicU64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let current_num = current_version.load(Ordering::Relaxed);
    let current = number_to_version(current_num);
    println!(
        "{}[auto-updater thread]{} Current version is {}",
        BLUE, RESET, current
    );

    let latest = get_git_version()?;
    let latest_num = version_to_number(&latest);
    println!(
        "{}[auto-updater thread]{} Latest version is {}",
        BLUE, RESET, latest
    );

    if current_num != latest_num {
        println!(
            "{}[auto-updater thread]{} Update needed! {} -> {}",
            BLUE, RESET, current, latest
        );

        if is_test_mode() {
            // Test mode: use local repo
            let repo_path = get_repo_path();
            Command::new("git")
                .args(["fetch", "--tags"])
                .current_dir(&repo_path)
                .output()?;

            Command::new("git")
                .args(["checkout", &latest])
                .current_dir(&repo_path)
                .output()?;
        } else {
            // Production mode: pull from remote repo
            Command::new("git")
                .args(["fetch", "--tags", REMOTE_REPO])
                .output()?;

            Command::new("git").args(["checkout", &latest]).output()?;
        }

        current_version.store(latest_num, Ordering::Relaxed);
        write_version_to_file(&latest)?;

        // Build and restart as a new detached process
        // By making it a separate process (not just a thread), it will survive when the parent process exits
        let repo_path = get_repo_path();
        let child = Command::new("cargo")
            .args(["run", "--release", "--"])
            .arg(&args[0])
            .current_dir(format!("{}/clients/cli", repo_path))
            .process_group(0) // Create new process group
            .spawn()?;

        // Write the new PID to a file (so it can be read by bash script)
        std::fs::write(".prover.pid", child.id().to_string())?;

        println!(
            "{}[auto-updater thread]{} Started new process with PID: {}",
            BLUE,
            RESET,
            child.id()
        );

        // Exit the current process
        println!(
            "{}[auto-updater thread]{} Restarting with new version...",
            BLUE, RESET
        );
        std::process::exit(0); // This will stop the main thread
    }

    Ok(())
}
