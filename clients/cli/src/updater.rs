//! Auto-updater implementation for the CLI
//!
//! This module provides two main functions:
//! - `check_and_use_binary`: Validates the binary location and handles process spawning
//! - `spawn_auto_update_thread`: Manages the background update process
//!
//! The update process:
//! 1. Verifies the binary location and handles process respawning if needed
//! 2. Runs version checks in a background thread at configured intervals
//! 3. Downloads and applies updates automatically when new versions are found
//! 4. Handles process replacement with the new version
//!
//! The updater uses environment flags to prevent recursive spawning and supports
//! both default and custom binary locations.

use semver::Version;
use std::process::Command;
use std::sync::Arc;
use std::{thread, time::Duration};
use tracing::error;

use crate::utils::updater::{
    get_binary_path, UpdaterConfig, VersionManager, VersionStatus, RESET, UPDATER_COLOR,
};

/// Manages binary location and process spawning for auto-updates
///
/// If running from default location (~/.nexus/bin/prover):
/// - Returns Ok(None)
///
/// If running from a custom location:
/// 1. Spawns a new process from the same location
/// 2. Sets PROVER_SPAWNED=1 to prevent infinite spawning
/// 3. Exits with the new process's exit code
pub async fn check_and_use_binary(
    updater_config: &UpdaterConfig,
) -> Result<Option<std::process::ExitStatus>, Box<dyn std::error::Error>> {
    // Create version manager to get runtime version
    let version_manager = VersionManager::new(updater_config.clone())?;
    let current_version = version_manager.get_current_version()?;

    println!(
        "{}[auto-updater]{} Starting prover v{} (runtime) at {}",
        UPDATER_COLOR,
        RESET,
        current_version,
        chrono::Local::now().format("%H:%M:%S")
    );

    // Check if we were spawned by another instance
    if std::env::var("PROVER_SPAWNED").is_ok() {
        return Ok(None);
    }

    let binary_path = get_binary_path().join("prover");
    let current_exe = std::env::current_exe()?;

    // Check if we're running from the default binary path
    if current_exe != binary_path {
        println!(
            "{}[auto-updater]{} Running from custom location ({}), proceeding with update checks",
            UPDATER_COLOR,
            RESET,
            current_exe.display()
        );
        // Spawn new process with environment flag
        let status = Command::new(&current_exe)
            .args([&updater_config.hostname])
            .env("PROVER_SPAWNED", "1")
            .status()?;
        std::process::exit(status.code().unwrap_or(0));
    }

    Ok(None)
}

/// Spawns a background thread to check for and apply updates
pub fn spawn_auto_update_thread(
    updater_config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let version_manager = Arc::new(VersionManager::new(updater_config.clone())?);
    let version_manager_thread = version_manager.clone();
    let update_interval = updater_config.update_interval;

    // Spawn a new thread to periodically check for and apply updates
    // This thread will run indefinitely until the process is killed
    thread::spawn(move || loop {
        match version_manager_thread.update_version_status() {
            // If a new version is available, download and apply it...
            Ok(VersionStatus::UpdateAvailable(new_version)) => {
                // get the current version running
                let current_version = match version_manager_thread.get_current_version() {
                    Ok(version) => version,
                    Err(_) => Version::parse(crate::VERSION).unwrap(),
                };

                println!(
                    "{}[auto-updater]{} New version {} available (current: {}) - downloading new binary...\n",
                    UPDATER_COLOR, RESET, new_version, current_version
                );

                // Apply the update
                if let Err(e) = version_manager_thread.apply_update(&new_version) {
                    error!("Failed to update CLI: {}", e);
                } else {
                    println!(
                        "{}[auto-updater]{}\t\t 6. ✅ Successfully updated CLI to version {}",
                        UPDATER_COLOR, RESET, new_version
                    );
                }
            }
            // If we're up to date, just print a message
            Ok(VersionStatus::UpToDate) => {
                let current_version = match version_manager_thread.get_current_version() {
                    Ok(version) => version,
                    Err(_) => Version::parse(crate::VERSION).unwrap(),
                };

                println!(
                    "{}[auto-updater]{} CLI is up to date (version: {})",
                    UPDATER_COLOR, RESET, current_version
                );
            }
            // If there's an error, print it
            Err(e) => error!("Failed to check version: {}", e),
        }

        // Sleep for the update interval
        thread::sleep(Duration::from_secs(update_interval));
    });

    Ok(())
}
