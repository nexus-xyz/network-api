use std::{process::Command, thread, time::Duration};

// For testing: mock latest tag
static mut MOCK_LATEST_TAG: Option<String> = None;

//constant for update interval
const UPDATE_INTERVAL: u64 = 20; // 20 seconds

pub fn start_periodic_updates() {
    println!("Starting periodic CLI updates...");

    thread::spawn(|| {
        loop {
            if let Err(e) = check_and_update() {
                eprintln!("Update check failed: {}", e);
            }
            thread::sleep(Duration::from_secs(UPDATE_INTERVAL)); // 60 seconds (for testing)
        }
    });
}

pub fn check_and_update() -> Result<(), Box<dyn std::error::Error>> {
    let (repo_path, _) = get_paths()?;

    // Get current version
    let current = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(&repo_path)
        .output()?;
    let current = String::from_utf8_lossy(&current.stdout).trim().to_string();
    println!("Updater: Current version is {}", current); // Debug print

    // Get latest version (real or mocked)
    let latest = if cfg!(test) {
        unsafe { MOCK_LATEST_TAG.clone().unwrap_or(current.clone()) }
    } else {
        // Get latest tag name using git describe
        // Example repo state:
        //   * abc123 (tag: v2.0) Latest commit
        //   * def456 (tag: v1.1) Older commit
        //   * ghi789 (tag: v1.0) First commit
        //
        // Command will return: v2.0
        let latest = Command::new("git")
            .args(["describe", "--tags", "--abbrev=0"])
            .current_dir(&repo_path)
            .output()?;
        String::from_utf8_lossy(&latest.stdout).trim().to_string()
    };
    println!("Updater: Latest version is {}", latest); // Debug print

    if current != latest {
        println!("Updater: Update needed! {} -> {}", current, latest);
        println!("Update found! Rebuilding...");

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
