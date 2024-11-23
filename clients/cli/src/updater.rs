use std::sync::Arc;
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

// For testing: mock latest tag
static mut MOCK_LATEST_TAG: Option<String> = None;

//constant for update interval
const UPDATE_INTERVAL: u64 = 20; // 20 seconds

pub fn start_periodic_updates() {
    println!("\t[auto-updater] Starting periodic CLI updates...");

    // Initialize version counter (0.3.5 -> 305, 0.9.9 -> 909)
    let current_version = Arc::new(AtomicU64::new(version_to_number("0.3.5")));

    // Clone Arc for the thread
    let version_for_thread = current_version.clone();

    thread::spawn(move || {
        println!("\t[auto-updater] Update checker thread started!");
        loop {
            if let Err(e) = check_and_update(&version_for_thread) {
                eprintln!("\t[auto-updater] Update check failed: {}", e);
            }
            println!(
                "\t[auto-updater] Sleeping for {} seconds...",
                UPDATE_INTERVAL
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
    println!("[auto-updater] Checking git repo at: {}", repo_path);

    // Get current version from memory
    let current_num = current_version.load(Ordering::Relaxed);
    let current = number_to_version(current_num);
    println!("[auto-updater] Current version is {}", current);

    // Get latest version from git
    let latest = get_git_version()?;
    let latest_num = version_to_number(&latest);
    println!("[auto-updater] Latest version is {}", latest);

    if current_num != latest_num {
        println!("[auto-updater] Update needed! {} -> {}", current, latest);
        println!("[auto-updater] Update found! Rebuilding...");

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

        // Rebuild the project
        println!("[auto-updater] Rebuilding project...");
        Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&repo_path)
            .output()?;

        // Update the version before restarting
        current_version.store(latest_num, Ordering::Relaxed);
        println!("[auto-updater] Updated version to: {}", latest);

        // Restart self
        let current_exe = std::env::current_exe()?;
        let _ = Command::new(current_exe).spawn()?;
        std::process::exit(0);
    }

    Ok(())
}

fn get_paths() -> Result<(String, String), Box<dyn std::error::Error>> {
    let repo_path = std::env::current_dir()?.to_string_lossy().to_string();
    let cli_path = format!("{}/target/release/cli", repo_path);
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
