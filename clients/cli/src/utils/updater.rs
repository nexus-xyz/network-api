//! Core auto-updater functionality and version management
//!
//! This module provides the underlying implementation for version checking and updates
//! using the self_update crate to handle version management and updates from GitHub releases.

use self_update::{cargo_crate_version, self_replace, ArchiveKind, Compression, Extract};
use semver::Version;
use std::os::unix::process::CommandExt;
use std::path::Path;
// use std::process::Command;

// ANSI escape codes for colors for pretty printing
pub const GREEN: &str = "\x1b[32m"; // Used to test if binary is replaced
pub const BLUE: &str = "\x1b[34m";

// UPDATER_COLOR is commented out because it is used only to show the updater
//is properly updated the source code vs binary
// used for 0.3.6
// pub const UPDATER_COLOR: &str = GREEN;
// used for 0.3.7
pub const UPDATER_COLOR: &str = BLUE;

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

        println!(
            "{}[auto-updater]{} Checking for updates in {} seconds",
            UPDATER_COLOR, RESET, config.update_interval
        );

        config
    }
}

pub enum VersionStatus {
    UpdateAvailable(Version),
    UpToDate,
}

pub struct VersionManager {
    // version_file: std::path::PathBuf,
}

impl VersionManager {
    pub fn new(_config: UpdaterConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Return an instance
        Ok(Self {})
    }

    // Checks GitHub for available updates by comparing the current version against the latest target release
    pub fn update_version_status(&self) -> Result<VersionStatus, Box<dyn std::error::Error>> {
        println!(
            "{}[auto-updater]{} Checking for updates...",
            UPDATER_COLOR, RESET
        );

        let current_version = self.get_current_version()?;
        let status = tokio::task::block_in_place(|| {
            let mut config = self_update::backends::github::Update::configure();
            let target = self_update::get_target();

            let update_builder = config
                .repo_owner("nexus-xyz")
                .repo_name("network-api")
                .bin_name("prover")
                .current_version(cargo_crate_version!())
                .target(target)
                .no_confirm(true);

            match update_builder.build()?.get_latest_release() {
                Ok(release) => Ok(release),
                Err(e) => {
                    println!(
                        "{}[auto-updater]{} No updates available for your platform ({}).\nError: {}",
                        UPDATER_COLOR, RESET, target, e
                    );
                    println!(
                        "{}[auto-updater]{} Please stop the CLI and run `curl https://cli.nexus.xyz/ | sh` to update the CLI manually",
                        UPDATER_COLOR, RESET
                    );
                    Err(e)
                }
            }
        })?;

        // Compare versions
        if current_version.to_string() == status.version {
            println!(
                "{}[auto-updater]{} Versions match - no update needed",
                UPDATER_COLOR, RESET
            );
            Ok(VersionStatus::UpToDate)
        } else {
            println!(
                "{}[auto-updater]{} Update available: {} -> {}",
                UPDATER_COLOR, RESET, current_version, status.version
            );
            Ok(VersionStatus::UpdateAvailable(Version::parse(
                &status.version,
            )?))
        }
    }

    pub fn apply_update(
        &self,
        new_version: &Version,
    ) -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
        println!(
            "{}[auto-updater]{} \t\t 1. Inspecting update version {}...",
            UPDATER_COLOR, RESET, new_version
        );

        // Create a temporary directory for extraction
        let temp_dir = tempfile::Builder::new()
            .prefix("prover-update")
            .tempdir()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        // Download the new precompiled binary from GitHub
        let mut config = self_update::backends::github::Update::configure();
        let mut update_builder = config.repo_owner("nexus-xyz");
        update_builder = update_builder
            .repo_name("network-api")
            .bin_name("prover")
            .current_version(cargo_crate_version!())
            .target(self_update::get_target())
            .no_confirm(true);

        // Get the latest release from GitHub
        let release = update_builder
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
            .get_latest_release()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        // Get the asset that matches the target platform
        let target = self_update::get_target();
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == format!("{}.tar.gz", target))
            .ok_or_else(|| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "No binary available for your platform ({}). Please check https://github.com/nexus-xyz/network-api/releases for supported platforms.",
                        target
                    ),
                )) as Box<dyn std::error::Error + Send>
            })?;

        println!(
            "{}[auto-updater]{}\t\t 2. Downloading archive: {}",
            UPDATER_COLOR, RESET, asset.name
        );

        // Download to temp file
        let download_path = temp_dir.path().join(&asset.name);
        let request = reqwest::blocking::Client::new()
            .get(&asset.download_url)
            .header("Accept", "application/octet-stream")
            // Add a user agent to identify the updater (necessary for GitHub API not throttling requests)
            .header("User-Agent", "NexusUpdater/0.3.7");

        let response = request.send().map_err(|e| {
            eprintln!("Failed to send request: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;

        // Check if the response is successful
        if !response.status().is_success() {
            eprintln!("Failed to download file: HTTP {}", response.status());
            let error_body = response
                .text()
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            eprintln!("Error body: {}", error_body);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to download file",
            )) as Box<dyn std::error::Error + Send>);
        }

        let mut file = std::fs::File::create(&download_path).map_err(|e| {
            eprintln!("Failed to create file: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;
        std::io::copy(
            &mut response
                .bytes()
                .map_err(|e| {
                    eprintln!("Failed to read response bytes: {:?}", e);
                    Box::new(e) as Box<dyn std::error::Error + Send>
                })?
                .as_ref(),
            &mut file,
        )
        .map_err(|e| {
            eprintln!("Failed to write to file: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;

        // Get the file size
        let file_size = std::fs::metadata(&download_path)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
            .len();

        println!(
            "{}[auto-updater]{}\t\t 3. Archive downloaded to: {:?} (size: {} bytes)",
            UPDATER_COLOR, RESET, download_path, file_size
        );

        // Use the existing temporary directory for extraction
        let extract_path = temp_dir.path();

        // Extract and inspect the contents
        match extract_and_prepare_update(&download_path, extract_path) {
            Ok(_) => {
                // Define the path to the new executable
                let new_exe_path = extract_path.join("prover"); // Adjust "prover" to the actual executable name

                // Apply the update
                println!(
                    "{}[auto-updater]{}\t\t 4.Attempting to replace binary at {:?}",
                    UPDATER_COLOR,
                    RESET,
                    std::env::current_exe().unwrap_or_default()
                );

                if let Err(e) = self_replace::self_replace(&new_exe_path) {
                    eprintln!(
                        "{}[auto-updater]{} Failed to apply update: {}",
                        UPDATER_COLOR, RESET, e
                    );
                    return Err(Box::new(e) as Box<dyn std::error::Error + Send>);
                }

                // Replace current process with new binary
                if let Ok(current_exe) = std::env::current_exe() {
                    let args: Vec<String> = std::env::args().skip(1).collect();
                    println!(
                        "{}[auto-updater]{}\t\t 5.Replacing current process with a new one using the args: {:?}",
                        UPDATER_COLOR, RESET, args
                    );

                    std::process::Command::new(current_exe).args(args).exec(); // Replace current process entirely
                }

                // This line will only be reached if exec fails
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!(
                    "{}[auto-updater]{} Failed to extract and inspect: {}",
                    UPDATER_COLOR, RESET, e
                );
                Err(e)
            }
        }
    }

    pub fn get_current_version(&self) -> Result<Version, Box<dyn std::error::Error>> {
        let version = cargo_crate_version!();
        Ok(Version::parse(version)?)
    }
}

pub fn get_binary_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{}/.nexus/bin", home))
}

fn extract_and_prepare_update(
    archive_path: &Path,
    extract_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send>> {
    // Attempt to extract the archive
    match Extract::from_source(archive_path)
        .archive(ArchiveKind::Tar(Some(Compression::Gz)))
        .extract_into(extract_path)
    {
        Ok(_) => {
            // Print the extracted files for inspection
            for entry in std::fs::read_dir(extract_path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
            {
                let entry = entry.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;
                println!(
                    "{}[auto-updater]{} \t\t\tExtracted file: {:?}",
                    UPDATER_COLOR,
                    RESET,
                    entry.path()
                );
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to extract archive: {:?}", e);
            Err(Box::new(e) as Box<dyn std::error::Error + Send>)
        }
    }
}
