use parking_lot::RwLock;
use std::sync::Arc;
use std::{thread, time::Duration};

use crate::utils::updater::{
    download_and_apply_update, fetch_and_persist_cli_version, get_latest_available_version,
    UpdaterConfig, VersionStatus, BLUE, FALLBACK_VERSION, RESET,
};

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

    // Initialize an atomic version number shared between threads that
    // tracks the currently installed CLI version
    let cli_version_shared_by_threads = Arc::new(RwLock::new(
        fetch_and_persist_cli_version(&updater_config).unwrap_or_else(|_| FALLBACK_VERSION),
    ));

    // Clone Arc for the update checker thread
    let current_cli_version = cli_version_shared_by_threads.clone();
    let update_interval = updater_config.update_interval;

    // Clone the udpater config before creating a new thread with it
    let updater_config_for_thread = updater_config.clone();

    thread::spawn(move || {
        println!(
            "{}[auto-updater thread]{} Update checker thread started!",
            BLUE, RESET
        );

        loop {
            match get_latest_available_version(&current_cli_version, &updater_config_for_thread) {
                // Got the latest version info with no error....
                Ok(version_info) => match version_info {
                    // ... there is an update available, try to apply it
                    VersionStatus::UpdateAvailable(new_version) => {
                        if let Err(e) = download_and_apply_update(
                            &new_version,
                            &current_cli_version,
                            &updater_config_for_thread,
                        ) {
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

            println!(
                "{}[auto-updater thread]{} Next update check in {} seconds...",
                BLUE, RESET, update_interval
            );
            thread::sleep(Duration::from_secs(update_interval));
        }
    });
}
