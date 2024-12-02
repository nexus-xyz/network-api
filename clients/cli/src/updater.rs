//! Auto-updater implementation for the CLI
//!
//! This module handles automatic updates by running a background thread that:
//! - Periodically checks for new versions
//! - Downloads and applies updates when available
//! - Restarts the CLI with the new version
//!
//! The updater runs in a separate thread to avoid blocking the main CLI operations,
//! allowing users to continue using the CLI while update checks happen in the background.

use std::process::Command;
use std::sync::Arc;
use std::{thread, time::Duration};
use tracing::error;

use crate::utils::updater::{
    get_binary_path, UpdaterConfig, VersionManager, VersionStatus, BLUE, RESET,
};

pub async fn check_and_use_binary(
    updater_config: &UpdaterConfig,
) -> Result<Option<std::process::ExitStatus>, Box<dyn std::error::Error>> {
    let binary_path = get_binary_path().join("prover");

    if !binary_path.exists() {
        println!(
            "{}[auto-updater]{} No installed binary found, falling back to local build (auto-updates enabled)",
            BLUE, RESET
        );
        return Ok(None);
    }

    let version_manager = VersionManager::new(updater_config.clone())?;
    match version_manager.update_version_status()? {
        VersionStatus::UpdateAvailable(new_version) => {
            println!(
                "{}[auto-updater]{} Update available - downloading version {}",
                BLUE, RESET, new_version
            );
            if let Err(e) = version_manager.apply_update(&new_version) {
                println!(
                    "{}[auto-updater]{} Failed to update CLI: {}",
                    BLUE, RESET, e
                );
                println!("{}[auto-updater]{} Falling back to cargo run", BLUE, RESET);
                Ok(None)
            } else {
                // After successful update, spawn new binary and exit current process
                println!(
                    "{}[auto-updater]{} Successfully installed new binary version {}",
                    BLUE, RESET, new_version
                );
                println!(
                    "{}[auto-updater]{} Update complete, launching new binary from: {}",
                    BLUE,
                    RESET,
                    binary_path.display()
                );

                let status = Command::new(&binary_path)
                    .args(std::env::args().skip(1)) // Forward all CLI args except program name
                    .status()?;

                println!(
                    "{}[auto-updater]{} New binary launched successfully with status: {}",
                    BLUE, RESET, status
                );
                std::process::exit(status.code().unwrap_or(0));
            }
        }
        VersionStatus::UpToDate => {
            println!(
                "{}[auto-updater]{} Using installed binary (latest version)",
                BLUE, RESET
            );

            let status = Command::new(&binary_path)
                .args([&updater_config.hostname])
                .status()?;
            Ok(Some(status))
        }
    }
}

pub fn spawn_auto_update_thread(
    updater_config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let version_manager = Arc::new(VersionManager::new(updater_config.clone())?);
    let version_manager_thread = version_manager.clone();
    let update_interval = updater_config.update_interval;

    println!(
        "{}[auto-updater]{} Update checker thread started (current version: {})",
        BLUE,
        RESET,
        crate::VERSION
    );

    thread::spawn(move || loop {
        match version_manager_thread.update_version_status() {
            Ok(VersionStatus::UpdateAvailable(new_version)) => {
                println!(
                    "{}[auto-updater]{} New version {} available (current: {}) - downloading update",
                    BLUE, RESET, new_version, crate::VERSION
                );

                match version_manager_thread.apply_update(&new_version) {
                    Ok(_) => {
                        println!(
                            "{}[auto-updater]{} Successfully downloaded and applied update to version {}",
                            BLUE, RESET, new_version
                        );
                    }
                    Err(e) => error!("Failed to update CLI: {}", e),
                }
            }
            Ok(VersionStatus::UpToDate) => {
                println!(
                    "{}[auto-updater]{} CLI is up to date (version: {})",
                    BLUE,
                    RESET,
                    crate::VERSION
                );
            }
            Err(e) => error!("Failed to check version: {}", e),
        }
        thread::sleep(Duration::from_secs(update_interval));
    });

    Ok(())
}
