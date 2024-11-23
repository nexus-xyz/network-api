use std::sync::Arc;
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

// // For testing: mock latest tag
// static mut MOCK_LATEST_TAG: Option<String> = None;

//constant for update interval
const UPDATE_INTERVAL: u64 = 20; // 20 seconds

// Add at the top with other constants
const BLUE: &str = "\x1b[34m"; // Normal blue
                               // or use "\x1b[1;34m" for bright blue
const RESET: &str = "\x1b[0m";

pub fn start_periodic_updates() {
    println!(
        "{}[auto-updater]{} Starting periodic CLI updates...",
        BLUE, RESET
    );

    // Initialize version counter (0.3.5 -> 305, 0.9.9 -> 909)
    let current_version = Arc::new(AtomicU64::new(version_to_number("0.3.5")));

    // Clone Arc for the thread
    let version_for_thread = current_version.clone();

    thread::spawn(move || {
        println!(
            "{}[auto-updater]{} Update checker thread started!",
            BLUE, RESET
        );
        loop {
            if let Err(e) = check_and_update(&version_for_thread) {
                eprintln!("{}[auto-updater]{} Update check failed: {}", BLUE, RESET, e);
            }
            println!(
                "{}[auto-updater]{} Checking for updates in {} seconds...",
                BLUE, RESET, UPDATE_INTERVAL
            );
            thread::sleep(Duration::from_secs(UPDATE_INTERVAL));
        }
    });
}

fn version_to_number(version: &str) -> u64 {
    // Convert "0.3.5" to 305
    let parts: Vec<&str> = version.split('.').collect();
    let major: u64 = parts[0].parse().unwrap_or(0);
    let minor: u64 = parts[1].parse().unwrap_or(0);
    let patch: u64 = parts[2].parse().unwrap_or(0);
    major * 100_000 + minor * 1_000 + patch
}

fn number_to_version(num: u64) -> String {
    // Convert 305 back to "0.3.5"
    let major = num / 100_000;
    let minor = (num % 100_000) / 1_000;
    let patch = num % 1_000;
    format!("{}.{}.{}", major, minor, patch)
}

pub fn check_and_update(
    current_version: &Arc<AtomicU64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (repo_path, _) = get_paths()?;
    println!(
        "{}[auto-updater]{} Checking git repo at: {}",
        BLUE, RESET, repo_path
    );

    // Get current version from memory
    let current_num = current_version.load(Ordering::Relaxed);
    let current = number_to_version(current_num);
    println!(
        "{}[auto-updater]{} Current version is {}",
        BLUE, RESET, current
    );

    // Get latest version from git
    let latest = get_git_version()?;
    let latest_num = version_to_number(&latest);
    println!(
        "{}[auto-updater]{} Latest version is {}",
        BLUE, RESET, latest
    );

    if current_num != latest_num {
        println!(
            "{}[auto-updater]{} Update needed! {} -> {}",
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
            "{}[auto-updater]{} Rebuilding and running new version...",
            BLUE, RESET
        );

        // Get original args to pass to new process
        let args: Vec<String> = std::env::args().skip(1).collect();

        // Build and run new version
        Command::new("cargo")
            .args(["run", "--release", "--", "beta.orchestrator.nexus.xyz"])
            .args(args) // Pass along original arguments
            .current_dir(format!("{}/clients/cli", repo_path))
            .spawn()?;

        // Exit the current process
        std::process::exit(0); // This will stop the main thread
    }

    Ok(())
}

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

fn get_git_version() -> Result<String, Box<dyn std::error::Error>> {
    let (repo_path, _) = get_paths()?;

    // Get latest tag from git
    let output = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(&repo_path)
        .output()?;

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
