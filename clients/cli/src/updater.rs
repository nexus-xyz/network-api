//! Auto-updater implementation for the CLI
//!
//! This module handles automatic updates by running a background thread that:
//! - Periodically checks for new versions
//! - Downloads and applies updates when available
//! - Restarts the CLI with the new version
//!
//! The updater runs in a separate thread to avoid blocking the main CLI operations,
//! allowing users to continue using the CLI while update checks happen in the background.

use std::sync::Arc;
use std::{thread, time::Duration};

use crate::utils::updater::{UpdaterConfig, VersionManager, VersionStatus, BLUE, RESET};

// We spawn a separate thread for periodic update checks because the auto-updater runs in an infinite loop
// that would otherwise block the main CLI process. By running in a background thread:

// 1. The update checker can continuously monitor for new versions without interrupting the main CLI operations
// 2. The main thread remains free to handle its primary responsibility (proving transactions)
// 3. Users don't have to wait for update checks to complete before using the CLI
pub fn spawn_auto_update_thread(updater_config: &UpdaterConfig) {
    println!(
        "{}[auto-updater thread]{} Starting periodic CLI updates...",
        BLUE, RESET
    );

    // Initialize version manager
    let version_manager = VersionManager::new(updater_config.clone()).unwrap();
    let version_manager = Arc::new(version_manager);

    // Clone Arcs for the update checker thread
    let version_manager = version_manager.clone();
    let update_interval = updater_config.update_interval;

    // Spawn the update checker thread
    thread::spawn(move || {
        println!(
            "{}[auto-updater thread]{} Update checker thread started!",
            BLUE, RESET
        );

        // Infinite loop to check for updates
        loop {
            match version_manager.as_ref().update_version_status() {
                // Got the latest version info with no error....
                Ok(version_info) => match version_info {
                    // ... there is an update available, try to apply it
                    VersionStatus::UpdateAvailable(new_version) => {
                        if let Err(e) = version_manager.apply_update(&new_version) {
                            eprintln!(
                                "{}[auto-updater thread]{} Failed to update CLI: {}",
                                BLUE, RESET, e
                            );
                        }
                    }
                    // ... No update needed
                    VersionStatus::UpToDate => {
                        println!("{}[auto-updater thread]{} CLI is up to date", BLUE, RESET);
                    }
                },
                Err(e) => {
                    eprintln!(
                        "{}[auto-updater thread]{} Failed to check version: {}",
                        BLUE, RESET, e
                    );
                }
            }

            // Wait for the next update check
            println!(
                "{}[auto-updater thread]{} Next update check in {} seconds...",
                BLUE, RESET, update_interval
            );
            thread::sleep(Duration::from_secs(update_interval));
        }
    });
}
