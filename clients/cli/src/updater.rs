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
// use std::process::Command;
use std::sync::Arc;
use std::{thread, time::Duration};
use tracing::error;

use crate::utils::updater::{
    // get_binary_path,
    UpdaterConfig,
    VersionManager,
    VersionStatus,
    RESET,
    UPDATER_COLOR,
};

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
