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
        info!("No installed binary found, using cargo run");
        return Ok(None);
    }

    let version_manager = VersionManager::new(updater_config.clone())?;
    match version_manager.update_version_status()? {
        VersionStatus::UpdateAvailable(_) => {
            info!("Update available, using cargo run while downloading");
            Ok(None)
        }
        VersionStatus::UpToDate => {
            info!("Using installed binary (latest version)");
            let status = Command::new(&binary_path)
                .args([
                    &updater_config.hostname,
                    "--updater-mode",
                    match updater_config.mode {
                        crate::utils::updater::AutoUpdaterMode::Test => "test",
                        crate::utils::updater::AutoUpdaterMode::Production => "production",
                    },
                ])
                .status()?;
            Ok(Some(status))
        }
    }
}

pub fn spawn_auto_update_thread(
    updater_config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting periodic CLI updates...");

    let version_manager = Arc::new(VersionManager::new(updater_config.clone())?);
    let version_manager_thread = version_manager.clone();
    let update_interval = updater_config.update_interval;

    thread::spawn(move || {
        info!("Update checker thread started!");
        loop {
            match version_manager_thread.as_ref().update_version_status() {
                Ok(version_info) => match version_info {
                    VersionStatus::UpdateAvailable(new_version) => {
                        info!("New version {} available - downloading update", new_version);
                        if let Err(e) = version_manager_thread.apply_update(&new_version) {
                            error!("Failed to update CLI: {}", e);
                            continue;
                        }
                        info!("Update downloaded, restarting process...");

                        // Restart the process using the new binary
                        let binary_path = get_binary_path().join("prover");
                        if let Err(e) = Command::new(&binary_path)
                            .args(std::env::args().skip(1))
                            .spawn()
                        {
                            error!("Failed to restart process: {}", e);
                            continue;
                        }
                        std::process::exit(0);
                    }
                    VersionStatus::UpToDate => {
                        info!("CLI is up to date");
                    }
                },
                Err(e) => error!("Failed to check version: {}", e),
            }
            thread::sleep(Duration::from_secs(update_interval));
        }
    });

    Ok(())
}
