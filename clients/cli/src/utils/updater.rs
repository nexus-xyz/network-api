//! Core auto-updater functionality and version management
//!
//! This module provides the underlying implementation for:
//! - Version tracking and persistence
//! - Git-based version detection
//! - Update application logic
//! - Process management for CLI restarts
//!
//! The code here is used by the auto-updater thread (./updater.rs) to handle the mechanics of
//! checking versions and applying updates in both test and production environments.

use parking_lot::RwLock;
use semver::Version;
use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::{fs, process::Command};

// Constants

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m"; // Normal blue
pub const RESET: &str = "\x1b[0m"; // Reset color

// The file to store the current version in
pub const VERSION_FILE: &str = ".current_version";
pub const REMOTE_REPO: &str = "https://github.com/nexus-xyz/network-api";
pub const FALLBACK_VERSION: Version = Version::new(0, 3, 6); // 0.3.6

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum AutoUpdaterMode {
    Production,
    Test,
}

/// Struct to manage the updater configuration
#[derive(Clone)]
pub struct UpdaterConfig {
    pub mode: AutoUpdaterMode,
    pub update_interval: u64,
    pub repo_path: String,
    pub remote_repo: String,
    pub hostname: String,
}

impl UpdaterConfig {
    pub fn new(mode: AutoUpdaterMode, hostname: String) -> Self {
        match mode {
            AutoUpdaterMode::Production => Self {
                mode,
                repo_path: format!(
                    "{}/.nexus/network-api",
                    std::env::var("HOME").unwrap_or_default()
                ),
                remote_repo: String::from("https://github.com/nexus-xyz/network-api.git"),
                update_interval: 3600, // check for updates every 1 hour (3600 seconds)
                hostname,
            },
            AutoUpdaterMode::Test => Self {
                mode,
                repo_path: std::env::current_dir()
                    .expect("Failed to get current directory")
                    .to_string_lossy()
                    .into_owned(),
                remote_repo: String::from("."),
                update_interval: 30, // check for updates every 30 seconds
                hostname,
            },
        }
    }
}

pub enum VersionStatus {
    UpdateAvailable(Version), // in case there is an update available, there is a semver `Version` type
    UpToDate,
}

// Struct to manage the version of the CLI it is running at all times
// This is also used to check for updates and apply them
pub struct VersionManager {
    current_version: Arc<RwLock<Version>>,
    config: UpdaterConfig,
}

impl VersionManager {
    /// Initialize the version manager
    pub fn new(config: UpdaterConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let current_version = Arc::new(RwLock::new(
            read_version_from_file().unwrap_or(FALLBACK_VERSION),
        ));
        Ok(Self {
            current_version,
            config,
        })
    }

    /// Fetch the current version of the CLI and persist it to a file
    pub fn fetch_and_persist_cli_version(&self) -> Result<Version, Box<dyn std::error::Error>> {
        // 1. Get the current git tag version (which depends on the updater mode)
        let current_git_version = self.get_cli_release_version(false)?;

        // 2. Convert the semver to a number and write it to a file (so it can persist across updates)
        write_version_to_file(&current_git_version)?;

        println!(
            "{}[auto-updater]{} Wrote version to file: {}",
            BLUE, RESET, current_git_version
        );

        Ok(current_git_version)
    }

    /// Get the latest release version of the CLI
    fn get_cli_release_version(
        &self,
        should_write: bool,
    ) -> Result<Version, Box<dyn std::error::Error>> {
        let version = match self.config.mode {
            AutoUpdaterMode::Test => {
                let output = Command::new("git")
                    .args(["describe", "--tags", "--abbrev=0"])
                    .current_dir(&self.config.repo_path)
                    .output()?;
                Version::parse(String::from_utf8(output.stdout)?.trim())?
            }
            AutoUpdaterMode::Production => {
                // Get only version tags (X.Y.Z format) from remote
                // This filters out non-release tags using git's pattern matching
                // Example matches: "1.2.3", "0.3.5"
                // Won't match: "latest", "stable", or other non-version tags
                let output = Command::new("git")
                    .args([
                        "ls-remote",
                        "--refs",
                        &self.config.remote_repo,
                        "refs/tags/[0-9]*.[0-9]*.[0-9]*", // Only match semantic version tags
                    ])
                    .output()?;

                let tags = String::from_utf8(output.stdout)?;

                // Process the version tags:
                // 1. Split each line and get the tag name
                // 2. Parse into semver Version type (validates format)
                // 3. Find the highest version number
                tags.lines()
                    .filter_map(|line| line.split('/').last())
                    .filter_map(|tag| Version::parse(tag).ok())
                    .max()
                    .ok_or("No release versions found")?
            }
        };

        // Optionally persist the version to disk
        if should_write {
            write_version_to_file(&version)?;
            println!(
                "{}[auto-updater]{} Wrote version to file: {}",
                BLUE, RESET, version
            );
        }

        Ok(version)
    }

    /// Apply an update to the CLI given a new version
    pub fn apply_update(&self, new_version: &Version) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "{}[auto-updater]{} Using repo path: {}",
            BLUE, RESET, self.config.repo_path
        );

        let repo_path = std::path::Path::new(&self.config.repo_path);
        if !repo_path.exists() {
            return Err(format!("Repository not found at: {}", self.config.repo_path).into());
        }

        match self.config.mode {
            AutoUpdaterMode::Test => {
                println!(
                    "{}[auto-updater]{} Building version {} from local repository...",
                    BLUE, RESET, new_version
                );
                let build_output = Command::new("cargo")
                    .args(["build", "--release"])
                    .current_dir(repo_path)
                    .output()?;

                if !build_output.status.success() {
                    return Err(format!(
                        "Build failed: {}",
                        String::from_utf8_lossy(&build_output.stderr)
                    )
                    .into());
                }
            }
            AutoUpdaterMode::Production => {
                if repo_path.read_dir()?.next().is_none() {
                    println!(
                        "{}[auto-updater]{} Cloning remote repository...",
                        BLUE, RESET
                    );
                    Command::new("git")
                        .args(["clone", &self.config.remote_repo, &self.config.repo_path])
                        .output()?;
                }

                println!("{}[auto-updater]{} Fetching updates...", BLUE, RESET);
                Command::new("git")
                    .args(["fetch", "--all", "--tags", "--prune"])
                    .current_dir(repo_path)
                    .output()?;

                println!(
                    "{}[auto-updater]{} Checking out version {}...",
                    BLUE, RESET, new_version
                );
                let checkout_output = Command::new("git")
                    .args(["checkout", &format!("tags/{}", new_version)])
                    .current_dir(repo_path)
                    .output()?;

                if !checkout_output.status.success() {
                    return Err(format!(
                        "Failed to checkout version: {}",
                        String::from_utf8_lossy(&checkout_output.stderr)
                    )
                    .into());
                }

                println!(
                    "{}[auto-updater]{} Building version {} from remote repository...",
                    BLUE, RESET, new_version
                );
                let build_output = Command::new("cargo")
                    .args(["build", "--release"])
                    .current_dir(repo_path)
                    .output()?;

                if !build_output.status.success() {
                    return Err(format!(
                        "Build failed: {}",
                        String::from_utf8_lossy(&build_output.stderr)
                    )
                    .into());
                }
            }
        }

        restart_cli_process_with_new_version(new_version, &self.current_version, &self.config)
    }

    /// update the version status of the CLI. is there an update available?
    pub fn update_version_status(&self) -> Result<VersionStatus, Box<dyn std::error::Error>> {
        let this_repo_version = self.current_version.read().clone();

        // debug output
        println!(
            "{}[auto-updater thread]{} Checking for updates from: {}",
            BLUE, RESET, self.config.remote_repo
        );

        let latest_version = match self.get_cli_release_version(false) {
            Ok(version) => version,
            Err(e) => {
                println!(
                    "{}[auto-updater thread]{} Version check failed: {}",
                    BLUE, RESET, e
                );
                return Ok(VersionStatus::UpToDate); // Gracefully handle error by assuming up-to-date
            }
        };

        println!(
            "{}[auto-updater]{} Current version of CLI: {} | Latest version of CLI: {}",
            BLUE, RESET, this_repo_version, latest_version
        );

        if this_repo_version == latest_version {
            Ok(VersionStatus::UpToDate)
        } else {
            Ok(VersionStatus::UpdateAvailable(latest_version))
        }
    }
}

/// function to read the current git tag version from a file
pub fn read_version_from_file() -> Result<Version, Box<dyn std::error::Error>> {
    let version_str = fs::read_to_string(VERSION_FILE)?;
    Ok(Version::parse(&version_str)?)
}

/// function to write the current git tag version to a file so it can be read by the updater thread
/// We write to a file because storing the version in memory is not persistent across updates
pub fn write_version_to_file(version: &Version) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(VERSION_FILE, version.to_string())?;
    Ok(())
}

/// Restart the CLI process with a new version
pub fn restart_cli_process_with_new_version(
    new_version: &Version,
    current_version: &Arc<RwLock<Version>>,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update version tracking
    *current_version.write() = new_version.clone();
    write_version_to_file(new_version)?;

    let cli_path = std::path::Path::new(&config.repo_path);

    let mode_arg = match config.mode {
        AutoUpdaterMode::Test => "test",
        AutoUpdaterMode::Production => "production",
    };

    let child = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            &config.hostname,
            "--updater-mode",
            mode_arg,
        ])
        .current_dir(cli_path)
        .process_group(0)
        .spawn()?;

    // Write the new PID to a file
    std::fs::write(".prover.pid", child.id().to_string())?;

    println!(
        "{}[auto-updater]{} Started new process with PID: {}",
        BLUE,
        RESET,
        child.id()
    );
    println!(
        "{}[auto-updater]{} Restarting with new version...",
        BLUE, RESET
    );

    std::process::exit(0);
}
