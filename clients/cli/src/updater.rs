use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

//constant for update interval
const UPDATE_INTERVAL_IN_SECONDS: u64 = 20; // 20 seconds

// ANSI escape codes for colors for pretty printing
const BLUE: &str = "\x1b[34m"; // Normal blue
const RESET: &str = "\x1b[0m";

// The file to store the current version in
const VERSION_FILE: &str = ".current_version";

// function to get the current git tag version from the file or git
fn get_current_version() -> Result<u64, Box<dyn std::error::Error>> {
    // Try reading from file first
    match read_version_from_file() {
        Ok(version) => {
            println!(
                "{}[auto-updater thread]{} Read version from file: {}",
                BLUE,
                RESET,
                number_to_version(version)
            );
            Ok(version)
        }
        Err(_) => {
            // If file doesn't exist, try getting from git
            let git_version = get_git_version()?;
            println!(
                "{}[auto-updater thread]{} Read version from git: {}",
                BLUE, RESET, git_version
            );
            let version_num = version_to_number(&git_version);
            // Save it to file for next time
            write_version_to_file(&git_version)?;
            println!(
                "{}[auto-updater thread]{} Wrote git_version version to file: {}",
                BLUE,
                RESET,
                number_to_version(version_num)
            );
            Ok(version_num)
        }
    }
}

// function to start the periodic update checker thread
// This is the function that is called by the main thread in prover.rs
pub fn start_periodic_updates() {
    println!(
        "{}[auto-updater thread]{} Starting periodic CLI updates...",
        BLUE, RESET
    );

    // Initialize version counter from file or git
    let current_version = Arc::new(AtomicU64::new(
        get_current_version().unwrap_or_else(|_| version_to_number("0.3.5")),
    ));

    // Clone Arc for the thread
    let version_for_thread = current_version.clone();

    thread::spawn(move || {
        println!(
            "{}[auto-updater thread]{} Update checker thread started!",
            BLUE, RESET
        );
        loop {
            if let Err(e) = check_and_update(&version_for_thread) {
                eprintln!(
                    "{}[auto-updater thread]{} Update check failed: {}",
                    BLUE, RESET, e
                );
            }
            println!(
                "{}[auto-updater thread]{} Checking for new CLI updated version in {} seconds...",
                BLUE, RESET, UPDATE_INTERVAL_IN_SECONDS
            );
            thread::sleep(Duration::from_secs(UPDATE_INTERVAL_IN_SECONDS));
        }
    });
}

/// function to convert a version string to a number
fn version_to_number(version: &str) -> u64 {
    // Convert "0.3.5" to 305
    let parts: Vec<&str> = version.split('.').collect();
    let major: u64 = parts[0].parse().unwrap_or(0);
    let minor: u64 = parts[1].parse().unwrap_or(0);
    let patch: u64 = parts[2].parse().unwrap_or(0);
    major * 100_000 + minor * 1_000 + patch
}

/// function to convert a number to a version string
fn number_to_version(num: u64) -> String {
    // Convert 305 back to "0.3.5"
    let major = num / 100_000;
    let minor = (num % 100_000) / 1_000;
    let patch = num % 1_000;
    format!("{}.{}.{}", major, minor, patch)
}

/// function to check for updates and apply them if needed
pub fn check_and_update(
    current_version: &Arc<AtomicU64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (repo_path, _) = get_paths()?;
    println!(
        "{}[auto-updater thread]{} Checking git repo at: {}",
        BLUE, RESET, repo_path
    );

    // Get current version from memory
    let current_num = current_version.load(Ordering::Relaxed);
    let current = number_to_version(current_num);
    println!(
        "{}[auto-updater thread]{} Current version is {}",
        BLUE, RESET, current
    );

    // Get latest version from git
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

        // Pull latest changes
        Command::new("git")
            .args(["fetch", "--tags"])
            .current_dir(&repo_path)
            .output()?;

        // Checkout the latest tag
        Command::new("git")
            .args(["checkout", &latest])
            .current_dir(&repo_path)
            .output()?;

        println!(
            "{}[auto-updater thread]{} Rebuilding and running new version...",
            BLUE, RESET
        );

        // Get original args to pass to new process
        let args: Vec<String> = std::env::args().skip(1).collect();

        // Write new version to file before restarting
        write_version_to_file(&latest)?;

        // Build and restart as a new detached process
        // By making it a separate process (not just a thread), it will survive when the parent process exits
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

/// function to get the paths to the repo and the cli directory
fn get_paths() -> Result<(String, String), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    // Navigate up from 'clients/cli' to repo root
    let repo_path = current_dir
        .parent() // up from cli
        .and_then(|p| p.parent()) // up from clients
        .ok_or("Could not find repository root")?
        .to_string_lossy()
        .to_string();

    let cli_path = format!("{}/clients/cli", repo_path);
    Ok((repo_path, cli_path))
}

/// function to read the current git tag version from git
fn get_git_version() -> Result<String, Box<dyn std::error::Error>> {
    let (repo_path, _) = get_paths()?;

    // Get latest tag from git
    let output = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(&repo_path)
        .output()?;

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

/// function to read the current git tag version from a file
fn read_version_from_file() -> Result<u64, Box<dyn std::error::Error>> {
    let version_str = fs::read_to_string(VERSION_FILE)?;
    Ok(version_to_number(&version_str))
}

/// function to write the current git tagversion to a file so it can be read by the updater thread
fn write_version_to_file(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(VERSION_FILE, version)?;
    Ok(())
}
