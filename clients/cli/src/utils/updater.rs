use std::{fs, path::PathBuf, process::Command};

// Constants

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m"; // Normal blue
pub const RESET: &str = "\x1b[0m";

// The file to store the current version in
pub const VERSION_FILE: &str = ".current_version";
pub const REMOTE_REPO: &str = "https://github.com/nexus-xyz/network-api";

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum AutoUpdaterMode {
    Production,
    Test,
}

#[derive(Clone)]
pub struct UpdaterConfig {
    pub mode: AutoUpdaterMode,
    pub update_interval: u64,
    pub repo_path: String,
    pub remote_repo: String,
}

impl UpdaterConfig {
    pub fn new(mode: AutoUpdaterMode) -> Self {
        match mode {
            AutoUpdaterMode::Production => Self {
                mode,
                repo_path: String::from("."), // Current directory for production
                remote_repo: String::from("https://github.com/nexus-labs/nexus-prover.git"),
                update_interval: 3600, // 1 hour
            },
            AutoUpdaterMode::Test => Self {
                mode,
                repo_path: std::env::current_dir()
                    .expect("Failed to get current directory")
                    .to_string_lossy()
                    .into_owned(),
                remote_repo: String::from("../nexus-prover"), // Local development path
                update_interval: 30,                          // 30 seconds
            },
        }
    }
}

// Version conversion utilities
pub fn version_to_number(version: &str) -> u64 {
    // Convert "0.3.5" to 305
    let parts: Vec<&str> = version.split('.').collect();
    let major: u64 = parts[0].parse().unwrap_or(0);
    let minor: u64 = parts[1].parse().unwrap_or(0);
    let patch: u64 = parts[2].parse().unwrap_or(0);
    major * 100_000 + minor * 1_000 + patch
}

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

/// function to write the current git tag version to a file so it can be read by the updater thread
/// We write to a file because storing the version in memory is not persistent across updates
pub fn write_version_to_file(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(VERSION_FILE, version)?;
    Ok(())
}

pub fn get_git_version(config: &UpdaterConfig) -> Result<String, Box<dyn std::error::Error>> {
    match config.mode {
        AutoUpdaterMode::Test => {
            // In test mode, we read the git tag directly from the local repository
            // This is useful during development when working with a local checkout
            let output = Command::new("git")
                .args(["describe", "--tags", "--abbrev=0"])
                .current_dir(&config.repo_path)
                .output()?;
            Ok(String::from_utf8(output.stdout)?.trim().to_string())
        }
        AutoUpdaterMode::Production => {
            // In production mode, we fetch tags from the remote repository
            // This ensures we get the latest version without needing a local git checkout
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
}
