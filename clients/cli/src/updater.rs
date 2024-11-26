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

    // Initialize an atomic version number shared between threads
    // let cli_version_shared_by_threads = Arc::new(RwLock::new(
    //     version_manager
    //         .fetch_and_persist_cli_version()
    //         .unwrap_or_else(|_| FALLBACK_VERSION),
    // ));

    // Clone Arcs for the update checker thread
    // let current_cli_version = cli_version_shared_by_threads.clone();
    let version_manager = version_manager.clone();
    let update_interval = updater_config.update_interval;

    thread::spawn(move || {
        println!(
            "{}[auto-updater thread]{} Update checker thread started!",
            BLUE, RESET
        );

        loop {
            match version_manager.as_ref().get_latest_available_version() {
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

            println!(
                "{}[auto-updater thread]{} Next update check in {} seconds...",
                BLUE, RESET, update_interval
            );
            thread::sleep(Duration::from_secs(update_interval));
        }
    });
}
