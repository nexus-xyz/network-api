use std::{process::Command, thread, time::Duration};

// For testing: mock latest tag
static mut MOCK_LATEST_TAG: Option<String> = None;

//constant for update interval
const UPDATE_INTERVAL: u64 = 20; // 20 seconds

pub fn start_periodic_updates() {
    println!("\t[start_periodic_updates] Starting periodic CLI updates...");

    thread::spawn(|| {
        println!("\t[start_periodic_updates]Update checker thread started!");
        loop {
            if let Err(e) = check_and_update() {
                eprintln!("\t[start_periodic_updates] Update check failed: {}", e);
            }
            println!(
                "\t[start_periodic_updates] Sleeping for {} seconds...",
                UPDATE_INTERVAL
            );
            thread::sleep(Duration::from_secs(UPDATE_INTERVAL)); // 60 seconds (for testing)
        }
    });
}

pub fn check_and_update() -> Result<(), Box<dyn std::error::Error>> {
    let (repo_path, _) = get_paths()?;
    println!("[updater] Checking git repo at: {}", repo_path);

    // Get current version
    let current = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(&repo_path)
        .output()?;
    let current = String::from_utf8_lossy(&current.stdout).trim().to_string();
    println!("[updater] Current version is {}", current);

    // Get latest version
    let latest = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(&repo_path)
        .output()?;
    let latest = String::from_utf8_lossy(&latest.stdout).trim().to_string();
    println!("[updater] Latest version is {}", latest);

    if current != latest {
        println!("[updater] Update needed! {} -> {}", current, latest);
        println!("[updater] Update found! Rebuilding...");

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
        Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&repo_path)
            .output()?;

        // Restart self
        let current_exe = std::env::current_exe()?;
        let _ = Command::new(current_exe).spawn()?;
        std::process::exit(0); // Exit old process
    }

    Ok(())
}

fn get_paths() -> Result<(String, String), Box<dyn std::error::Error>> {
    let repo_path = std::env::current_dir()?.to_string_lossy().to_string();
    let cli_path = format!("{}/target/release/cli", repo_path);
    Ok((repo_path, cli_path))
}
