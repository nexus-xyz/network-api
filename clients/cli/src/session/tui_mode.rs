//! TUI mode execution

use super::{
    SessionData,
    messages::{print_session_exit_success, print_session_shutdown, print_session_starting},
};
use crate::orchestrator::Orchestrator;
use crate::ui::{self, UIConfig};
use crate::version::checker::check_for_new_version;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{error::Error, io};

/// Runs the application in TUI mode
///
/// This function handles:
/// 1. Terminal setup and cleanup
/// 2. UI application initialization and execution
/// 3. Proper shutdown handling
///
/// # Arguments
/// * `session` - Session data from setup
/// * `with_background` - Whether to enable background colors
///
/// # Returns
/// * `Ok(())` - TUI mode completed successfully
/// * `Err` - TUI mode failed
pub async fn run_tui_mode(
    session: SessionData,
    with_background: bool,
) -> Result<(), Box<dyn Error>> {
    // Print session start message
    print_session_starting("TUI", session.node_id);

    // Check for new version and get version info
    let current_version = env!("CARGO_PKG_VERSION");
    let (version_update_available, latest_version) =
        if let Some(message) = check_for_new_version(current_version).await {
            // Extract version from message - format: "New version v0.10.3 is available..."
            let latest = message
                .split_whitespace()
                .nth(2) // "New version [VERSION] is available..."
                .map(|v| v.to_string());
            (true, latest)
        } else {
            (false, None)
        };

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, DisableMouseCapture)?;

    // Initialize the terminal with Crossterm backend
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create the application and run it
    let ui_config = UIConfig::new(
        with_background,
        session.num_workers,
        version_update_available,
        latest_version,
    );

    let app = ui::App::new(
        Some(session.node_id),
        session.orchestrator.environment().clone(),
        session.event_receiver,
        session.shutdown_sender.clone(),
        session.max_tasks_shutdown_sender.subscribe(),
        ui_config,
    );

    let result = ui::run(&mut terminal, app).await;

    // Clean up the terminal after running the application
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Handle the result
    result?;

    // Wait for workers to finish
    print_session_shutdown();
    for handle in session.join_handles {
        let _ = handle.await;
    }
    print_session_exit_success();

    Ok(())
}
