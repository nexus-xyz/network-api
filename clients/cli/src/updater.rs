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
use tracing::{error, info};

use crate::utils::updater::{get_binary_path, UpdaterConfig, VersionManager, VersionStatus};

pub async fn check_and_use_binary(
    updater_config: &UpdaterConfig,
) -> Result<Option<std::process::ExitStatus>, Box<dyn std::error::Error>> {
    let binary_path = get_binary_path().join("prover");

    if !binary_path.exists() {
        println!("No installed binary found, using cargo run");
        return Ok(None);
    }

    let version_manager = VersionManager::new(updater_config.clone())?;
    match version_manager.update_version_status()? {
        VersionStatus::UpdateAvailable(new_version) => {
            println!("Update available - downloading version {}", new_version);
            if let Err(e) = version_manager.apply_update(&new_version) {
                error!("Failed to update CLI: {}", e);
                info!("Falling back to cargo run");
                Ok(None)
            } else {
                // After successful update, spawn new binary and exit current process
                println!("Update complete, launching new binary");
                let status = Command::new(&binary_path)
                    .args(std::env::args().skip(1)) // Forward all CLI args except program name
                    .status()?;
                std::process::exit(status.code().unwrap_or(0)); // Exit current process
            }
        }
        VersionStatus::UpToDate => {
            println!("Using installed binary (latest version)");
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

    thread::spawn(move || {
        println!("Update checker thread started");
        loop {
            match version_manager_thread.update_version_status() {
                Ok(VersionStatus::UpdateAvailable(new_version)) => {
                    println!("New version {} available - downloading update", new_version);
                    if let Err(e) = version_manager_thread.apply_update(&new_version) {
                        error!("Failed to update CLI: {}", e);
                    }
                }
                Ok(VersionStatus::UpToDate) => {
                    println!("CLI is up to date");
                }
                Err(e) => error!("Failed to check version: {}", e),
            }
            thread::sleep(Duration::from_secs(update_interval));
        }
    });

    Ok(())
}
