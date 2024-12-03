//! Auto-updater implementation for the CLI
//!
//! This module handles automatic updates by running a background thread that:
//! - Periodically checks for new versions
//! - Downloads and applies updates when available
//! - Restarts the CLI with the new version
//!
//! The updater runs in a separate thread to avoid blocking the main CLI operations,
//! allowing users to continue using the CLI while update checks happen in the background.

use semver::Version;
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
            BLUE,
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

pub fn spawn_auto_update_thread(
    updater_config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let version_manager = Arc::new(VersionManager::new(updater_config.clone())?);
    let version_manager_thread = version_manager.clone();
    let update_interval = updater_config.update_interval;

    let current_version = match version_manager_thread.get_current_version() {
        Ok(version) => version,
        Err(_) => Version::parse(crate::VERSION).unwrap(),
    };

    // println!(
    //     "{}[auto-updater]{} Update checker thread started (current version: {})",
    //     BLUE, RESET, current_version
    // );

    thread::spawn(move || loop {
        match version_manager_thread.update_version_status() {
            Ok(VersionStatus::UpdateAvailable(new_version)) => {
                let current_version = match version_manager_thread.get_current_version() {
                    Ok(version) => version,
                    Err(_) => Version::parse(crate::VERSION).unwrap(),
                };

                println!(
                    "{}[auto-updater]{} New version {} available (current: {}) - downloading new binary...",
                    BLUE, RESET, new_version, current_version
                );

                if let Err(e) = version_manager_thread.apply_update(&new_version) {
                    error!("Failed to update CLI: {}", e);
                } else {
                    println!(
                        "{}[auto-updater]{} ✅ Successfully updated CLI to version {}",
                        BLUE, RESET, new_version
                    );
                }
            }
            Ok(VersionStatus::UpToDate) => {
                let current_version = match version_manager_thread.get_current_version() {
                    Ok(version) => version,
                    Err(_) => Version::parse(crate::VERSION).unwrap(),
                };

                println!(
                    "{}[auto-updater]{} CLI is up to date (version: {})",
                    BLUE, RESET, current_version
                );
            }
            Err(e) => error!("Failed to check version: {}", e),
        }
        thread::sleep(Duration::from_secs(update_interval));
    });

    Ok(())
}
