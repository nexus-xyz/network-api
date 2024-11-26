use std::{fs, process::Command};

// Constants

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m"; // Normal blue
pub const RESET: &str = "\x1b[0m";

// The file to store the current version in
pub const VERSION_FILE: &str = ".current_version";
pub const REMOTE_REPO: &str = "https://github.com/nexus-xyz/network-api";

// returns true if the UPDATER_MODE environment variable is set to "test"
pub fn is_test_mode() -> bool {
    match std::env::var("UPDATER_MODE") {
        Ok(val) => {
            println!(
                "{}[auto-updater thread]{} Running in test mode",
                BLUE, RESET
            );
            // return true if the UPDATER_MODE environment variable is set to "test"
            val == "test"
        }
        Err(_) => false,
    }
}

// The environment the CLI is running in
// this is used to change a few things depending on if it's running in production or test mode
// a. how often it checks for updates
// b. where it looks for the git repo (test looks in the current directory, production looks in the repo)

/// function to get the update interval based on the environment
pub fn get_update_interval() -> u64 {
    if is_test_mode() {
        println!(
            "{}[auto-updater thread]{} Update interval is 20 seconds",
            BLUE, RESET
        );
        20 // 20 seconds for test
    } else {
        println!(
            "{}[auto-updater thread]{} Update interval is 1 hour",
            BLUE, RESET
        );
        3600 // 1 hour for production
    }
}

/// function to get the repo path based on the environment
pub fn get_repo_path() -> String {
    if is_test_mode() {
        println!(
            "{}[auto-updater thread]{} Repo path is current directory",
            BLUE, RESET
        );
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        println!(
            "{}[auto-updater thread]{} Repo path is home directory",
            BLUE, RESET
        );
        dirs::home_dir()
            .map(|home| home.join(".nexus").join("network-api"))
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

// function to convert a version string to a number
pub fn version_to_number(version: &str) -> u64 {
    // Convert "0.3.5" to 305
    let parts: Vec<&str> = version.split('.').collect();
    let major: u64 = parts[0].parse().unwrap_or(0);
    let minor: u64 = parts[1].parse().unwrap_or(0);
    let patch: u64 = parts[2].parse().unwrap_or(0);
    major * 100_000 + minor * 1_000 + patch
}

/// function to convert a number to a version string
pub fn number_to_version(num: u64) -> String {
    // Convert 305 back to "0.3.5"
    let major = num / 100_000;
    let minor = (num % 100_000) / 1_000;
    let patch = num % 1_000;
    format!("{}.{}.{}", major, minor, patch)
}

/// function to read the current git tag version from a file
pub fn read_version_from_file() -> Result<u64, Box<dyn std::error::Error>> {
    let version_str = fs::read_to_string(VERSION_FILE)?;
    Ok(version_to_number(&version_str))
}

/// function to write the current git tagversion to a file so it can be read by the updater thread
pub fn write_version_to_file(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(VERSION_FILE, version)?;
    Ok(())
}

pub fn get_git_version() -> Result<String, Box<dyn std::error::Error>> {
    if is_test_mode() {
        // Test mode: use local repo
        let repo_path = get_repo_path();
        let output = Command::new("git")
            .args(["describe", "--tags", "--abbrev=0"])
            .current_dir(&repo_path)
            .output()?;
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    } else {
        // Production mode: poll remote repo
        let output = Command::new("git")
            .args(["ls-remote", "--tags", "--refs", REMOTE_REPO])
            .output()?;

        let tags = String::from_utf8(output.stdout)?;
        tags.lines()
            .last()
            .and_then(|line| line.split('/').last())
            .map(|v| v.to_string())
            .ok_or_else(|| "No tags found".into())
    }
}
