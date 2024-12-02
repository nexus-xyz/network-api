//! Core auto-updater functionality and version management
//!
//! This module provides the underlying implementation for version checking and updates
//! using the self_update crate to handle version management and updates from GitHub releases.

use self_update::cargo_crate_version;
use semver::Version;
use std::os::unix::process::CommandExt;
use std::process::Command;

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m";
pub const RESET: &str = "\x1b[0m";

#[derive(Clone)]
pub struct UpdaterConfig {
    pub update_interval: u64,
    pub repo_path: String,
    pub remote_repo: String,
    pub hostname: String,
}

impl UpdaterConfig {
    pub fn new(hostname: String) -> Self {
        #[cfg(debug_assertions)]
        let config = Self {
            repo_path: std::env::current_dir()
                .expect("Failed to get current directory")
                .to_string_lossy()
                .into_owned(),
            remote_repo: String::from("."),
            update_interval: 30, // 30 seconds in debug mode
            hostname,
        };

        #[cfg(not(debug_assertions))]
        let config = Self {
            repo_path: format!(
                "{}/.nexus/network-api",
                std::env::var("HOME").unwrap_or_default()
            ),
            remote_repo: String::from("https://github.com/nexus-xyz/network-api.git"),
            update_interval: 3600, // 1 hour in release mode
            hostname,
        };

        config
    }
}

pub enum VersionStatus {
    UpdateAvailable(Version),
    UpToDate,
}

pub struct VersionManager {
    config: UpdaterConfig,
}

impl VersionManager {
    pub fn new(config: UpdaterConfig) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self { config })
    }

    pub fn update_version_status(&self) -> Result<VersionStatus, Box<dyn std::error::Error>> {
        let updater = self_update::backends::github::Update::configure()
            .repo_owner("nexus-xyz")
            .repo_name("network-api")
            .bin_name("nexus-cli")
            .current_version(cargo_crate_version!())
            .build()?;

        let latest_release = updater.get_latest_release()?;

        if cargo_crate_version!() == latest_release.version {
            Ok(VersionStatus::UpToDate)
        } else {
            Ok(VersionStatus::UpdateAvailable(Version::parse(
                &latest_release.version,
            )?))
        }
    }

    pub fn apply_update(&self, new_version: &Version) -> Result<(), Box<dyn std::error::Error>> {
        let status = self_update::backends::github::Update::configure()
            .repo_owner("nexus-xyz")
            .repo_name("network-api")
            .bin_name("nexus-cli")
            .current_version(cargo_crate_version!())
            .target_version_tag(&new_version.to_string())
            .build()?
            .update()?;

        println!(
            "{}[auto-updater]{} Update status: `{}`",
            BLUE,
            RESET,
            status.version()
        );

        // Restart the process
        let binary_path = get_binary_path().join("nexus-cli");
        let child = Command::new(binary_path)
            .arg(&self.config.hostname)
            .process_group(0)
            .spawn()?;

        std::fs::write(".prover.pid", child.id().to_string())?;
        std::process::exit(0);
    }
}

pub fn get_binary_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{}/.nexus/bin", home))
}
