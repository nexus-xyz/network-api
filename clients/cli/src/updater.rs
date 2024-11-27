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
pub fn spawn_auto_update_thread(
    updater_config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}[auto-updater]{} Starting periodic CLI updates...",
        BLUE, RESET
    );

    // Create a thread-safe version manager that can be shared across threads
    let version_manager: Arc<VersionManager> = Arc::new(
        VersionManager::new(updater_config.clone()).expect("Failed to initialize version manager"),
    );

    // Create a reference for the new thread (original stays with main thread)
    let version_manager_thread: Arc<VersionManager> = version_manager.clone();

    let update_interval = updater_config.update_interval;

    // Spawn the update checker thread
    thread::spawn(move || {
        println!(
            "{}[auto-updater]{} Update checker thread started!",
            BLUE, RESET
        );

        // Infinite loop to check for updates
        loop {
            match version_manager_thread.as_ref().update_version_status() {
                // Got the latest version info with no error....
                Ok(version_info) => match version_info {
                    // ... there is an update available, try to apply it
                    VersionStatus::UpdateAvailable(new_version) => {
                        if let Err(e) = version_manager_thread.apply_update(&new_version) {
                            println!(
                                "{}[auto-updater]{} Failed to update CLI: {}",
                                BLUE, RESET, e
                            )
                        }
                    }
                    // ... No update needed
                    VersionStatus::UpToDate => {
                        println!("{}[auto-updater]{} CLI is up to date", BLUE, RESET);
                    }
                },
                Err(e) => {
                    eprintln!(
                        "{}[auto-updater]{} Failed to check version: {}",
                        BLUE, RESET, e
                    );
                }
            }

            // Wait for the next update check
            println!(
                "{}[auto-updater]{} Next update check in {} seconds...\n",
                BLUE, RESET, update_interval
            );
            thread::sleep(Duration::from_secs(update_interval));
        }
    });

    Ok(())
}
