use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

// Constants

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m"; // Normal blue
pub const RESET: &str = "\x1b[0m";

// The file to store the current version in
pub const VERSION_FILE: &str = ".current_version";
pub const REMOTE_REPO: &str = "https://github.com/nexus-xyz/network-api";
pub const FALLBACK_VERSION: &str = "0.3.5";

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

pub enum VersionStatus {
    UpdateAvailable(u64),
    UpToDate,
}

// Version conversion utilities
pub fn semver_to_num(version: &str) -> u64 {
    // Convert "0.3.5" to 305
    let parts: Vec<&str> = version.split('.').collect();
    let major: u64 = parts[0].parse().unwrap_or(0);
    let minor: u64 = parts[1].parse().unwrap_or(0);
    let patch: u64 = parts[2].parse().unwrap_or(0);
    major * 100_000 + minor * 1_000 + patch
}

/// Convert a version number to a string using the "0.3.5" semver format
pub fn num_to_semver(num: u64) -> String {
    // Convert 305 back to "0.3.5"
    let major = num / 100_000;
    let minor = (num % 100_000) / 1_000;
    let patch = num % 1_000;
    format!("{}.{}.{}", major, minor, patch)
}

/// function to read the current git tag version from a file
pub fn read_version_from_file() -> Result<u64, Box<dyn std::error::Error>> {
    let version_str = fs::read_to_string(VERSION_FILE)?;
    Ok(semver_to_num(&version_str))
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

pub fn update_code_to_new_git_version(
    version: u64,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match config.mode {
        AutoUpdaterMode::Test => {
            // Test mode: use local repo
            Command::new("git")
                .args(["fetch", "--tags"])
                .current_dir(&config.repo_path)
                .output()?;

            Command::new("git")
                .args(["checkout", &num_to_semver(version)])
                .current_dir(&config.repo_path)
                .output()?;
        }
        AutoUpdaterMode::Production => {
            // Production mode: pull from remote repo
            Command::new("git")
                .args(["fetch", "--tags", REMOTE_REPO])
                .output()?;

            Command::new("git")
                .args(["checkout", &num_to_semver(version)])
                .output()?;
        }
    }

    Ok(())
}

pub fn restart_cli_process_with_new_version(
    new_version: u64,
    current_version: &Arc<AtomicU64>,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update version tracking
    current_version.store(new_version, Ordering::Relaxed);
    write_version_to_file(&num_to_semver(new_version))?;

    // Build and restart as a new detached process
    let program = std::env::args().next().unwrap();
    let child = Command::new("cargo")
        .args(["run", "--release", "--"])
        .arg(program)
        .current_dir(format!("{}/clients/cli", config.repo_path))
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
    println!(
        "{}[auto-updater thread]{} Restarting with new version...",
        BLUE, RESET
    );

    std::process::exit(0); // This will stop the main thread
}

pub fn get_latest_available_version(
    current_version: &Arc<AtomicU64>,
    updater_config: &UpdaterConfig,
) -> Result<VersionStatus, Box<dyn std::error::Error>> {
    let current_num = current_version.load(Ordering::Relaxed);
    let latest_version = get_and_save_version(&updater_config)?;

    println!(
        "{}[auto-updater thread]{} Current: {} | Latest: {}",
        BLUE,
        RESET,
        num_to_semver(current_num),
        num_to_semver(latest_version)
    );

    if current_num == latest_version {
        Ok(VersionStatus::UpToDate)
    } else {
        Ok(VersionStatus::UpdateAvailable(latest_version))
    }
}

// function to get the current git tag version from the file or git
pub fn get_and_save_version(
    updater_config: &UpdaterConfig,
) -> Result<u64, Box<dyn std::error::Error>> {
    //1. Get the current git tag version (which depends on the updater mode)
    let git_version = get_git_version(updater_config)?;

    //2. Convert the semver to a number and write it to a file (so it can persist across updates)
    let version_num = semver_to_num(&git_version);
    write_version_to_file(&git_version)?;

    println!(
        "{}[auto-updater thread]{} Wrote version to file: {}",
        BLUE,
        RESET,
        num_to_semver(version_num)
    );

    Ok(version_num)
}

// pub fn apply_update(
//     latest_version: u64,
//     current_version: &Arc<AtomicU64>,
//     updater_config: &UpdaterConfig,
// ) -> Result<(), Box<dyn std::error::Error>> {
//     let current_num = current_version.load(Ordering::Relaxed);

//     println!(
//         "{}[auto-updater thread]{} Current: {} | Latest: {}",
//         BLUE,
//         RESET,
//         num_to_semver(current_num),
//         num_to_semver(latest_version)
//     );

//     // If the current version is already the latest version, do nothing
//     if current_num == latest_version {
//         return Ok(());
//     }

//     // If the current version is not the latest version, update the git version and restart the process
//     println!(
//         "{}[auto-updater thread]{} Update needed! {} -> {}",
//         BLUE,
//         RESET,
//         num_to_semver(current_num),
//         num_to_semver(latest_version)
//     );

//     // Pull down the new code
//     update_code_to_new_git_version(latest_version, updater_config)?;

//     // Restart the CLI process with the new version
//     restart_cli_process_with_new_version(latest_version, current_version, updater_config)?;

//     Ok(())
// }

pub fn download_and_apply_update(
    new_version: u64,
    current_version: &Arc<AtomicU64>,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}[auto-updater thread]{} Downloading update {} -> {}",
        BLUE,
        RESET,
        num_to_semver(current_version.load(Ordering::Relaxed)),
        num_to_semver(new_version)
    );

    //1. Download new version
    update_code_to_new_git_version(new_version, config)?;

    println!(
        "{}[auto-updater thread]{} Building new version...",
        BLUE, RESET
    );

    // 2. Build new version
    Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(format!("{}/clients/cli", config.repo_path))
        .output()?;

    println!(
        "{}[auto-updater thread]{} Build complete, restarting CLI...",
        BLUE, RESET
    );

    // 3. Restart CLI with new version
    restart_cli_process_with_new_version(new_version, current_version, config)?;

    Ok(())
}
