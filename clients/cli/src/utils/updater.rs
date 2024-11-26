use parking_lot::RwLock;
use semver::Version;
use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::{
    fs,
    process::Command,
    // sync::atomic::{AtomicU64, Ordering},
};

// Constants

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m"; // Normal blue
pub const RESET: &str = "\x1b[0m"; // Reset color

// The file to store the current version in
pub const VERSION_FILE: &str = ".current_version";
pub const REMOTE_REPO: &str = "https://github.com/nexus-xyz/network-api";
pub const FALLBACK_VERSION: Version = Version::new(0, 3, 5); // 0.3.5

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum AutoUpdaterMode {
    Production,
    Test,
}

#[derive(Clone)]
pub struct UpdaterConfig {
    pub mode: AutoUpdaterMode,
    pub update_interval: u64,
    pub repo_path: String,
    pub remote_repo: String,
    pub hostname: String,
}

impl UpdaterConfig {
    pub fn new(mode: AutoUpdaterMode, hostname: String) -> Self {
        match mode {
            AutoUpdaterMode::Production => Self {
                mode,
                repo_path: format!(
                    "{}/.nexus/network-api",
                    std::env::var("HOME").unwrap_or_default()
                ),
                remote_repo: String::from("https://github.com/nexus-labs/nexus-prover.git"),
                update_interval: 3600, // 1 hour
                hostname,
            },
            AutoUpdaterMode::Test => Self {
                mode,
                repo_path: std::env::current_dir()
                    .expect("Failed to get current directory")
                    .to_string_lossy()
                    .into_owned(),
                remote_repo: String::from("."),
                update_interval: 30,
                hostname,
            },
        }
    }
}

pub enum VersionStatus {
    UpdateAvailable(Version), // in case there is an update available, there is a semver `Version` type
    UpToDate,
}

/// function to read the current git tag version from a file
pub fn read_version_from_file() -> Result<Version, Box<dyn std::error::Error>> {
    let version_str = fs::read_to_string(VERSION_FILE)?;
    Ok(Version::parse(&version_str)?)
}

/// function to write the current git tag version to a file so it can be read by the updater thread
/// We write to a file because storing the version in memory is not persistent across updates
pub fn write_version_to_file(version: &Version) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(VERSION_FILE, version.to_string())?;
    Ok(())
}

pub fn get_cli_version(config: &UpdaterConfig) -> Result<Version, Box<dyn std::error::Error>> {
    match config.mode {
        AutoUpdaterMode::Test => {
            // In test mode, we read the git tag directly from the local repository
            // This is useful during development when working with a local checkout
            let output = Command::new("git")
                .args(["describe", "--tags", "--abbrev=0"])
                .current_dir(&config.repo_path)
                .output()?;
            Ok(Version::parse(String::from_utf8(output.stdout)?.trim())?)
        }
        AutoUpdaterMode::Production => {
            // In production mode, we fetch tags from the remote repository
            // This ensures we get the latest version without needing a local git checkout
            let output = Command::new("git")
                .args(["ls-remote", "--tags", "--refs", REMOTE_REPO])
                .output()?;

            let tags = String::from_utf8(output.stdout)?;
            tags.lines()
                .last()
                .and_then(|line| line.split('/').last())
                .map(|v| v.to_string())
                .and_then(|v| Version::parse(&v).ok())
                .ok_or_else(|| "No tags found".into())
        }
    }
}

pub fn update_code_to_new_cli_version(
    version: &Version,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match config.mode {
        AutoUpdaterMode::Test => {
            // Test mode: use local repo
            Command::new("git")
                .args(["fetch", "--tags"])
                .current_dir(&config.repo_path)
                .output()?;

            Command::new("git")
                .args(["checkout", &version.to_string()])
                .current_dir(&config.repo_path)
                .output()?;
        }
        AutoUpdaterMode::Production => {
            // Production mode: pull from remote repo
            Command::new("git")
                .args(["fetch", "--tags", REMOTE_REPO])
                .output()?;

            Command::new("git")
                .args(["checkout", &version.to_string()])
                .output()?;
        }
    }

    Ok(())
}

pub fn restart_cli_process_with_new_version(
    new_version: &Version,
    current_version: &Arc<RwLock<Version>>,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update version tracking
    *current_version.write() = new_version.clone();
    write_version_to_file(new_version)?;

    let cli_path = std::path::Path::new(&config.repo_path);

    println!(
        "{}[auto-updater thread]{} Starting from directory: {}",
        BLUE,
        RESET,
        cli_path.display()
    );

    let mode_arg = match config.mode {
        AutoUpdaterMode::Test => "test",
        AutoUpdaterMode::Production => "production",
    };

    let child = Command::new("cargo")
        .args(["run", "--release", "--", &config.hostname, mode_arg])
        .current_dir(&cli_path)
        .process_group(0)
        .spawn()?;

    // Write the new PID to a file
    std::fs::write(".prover.pid", child.id().to_string())?;

    println!(
        "{}[auto-updater thread]{} Started new process with PID: {}",
        BLUE,
        RESET,
        child.id()
    );
    println!(
        "{}[auto-updater thread]{} Restarting with new version...",
        BLUE, RESET
    );

    std::process::exit(0);
}

pub fn get_latest_available_version(
    current_version: &Arc<RwLock<Version>>,
    updater_config: &UpdaterConfig,
) -> Result<VersionStatus, Box<dyn std::error::Error>> {
    let this_repo_version = current_version.read().clone();
    let latest_version = fetch_and_persist_cli_version(&updater_config)?;

    println!(
        "{}[auto-updater thread]{} Current: {} | Latest: {}",
        BLUE,
        RESET,
        this_repo_version.to_string(),
        latest_version.to_string()
    );

    if this_repo_version == latest_version {
        Ok(VersionStatus::UpToDate)
    } else {
        Ok(VersionStatus::UpdateAvailable(latest_version))
    }
}

// function to get the current git tag version from the file or git
pub fn fetch_and_persist_cli_version(
    updater_config: &UpdaterConfig,
) -> Result<Version, Box<dyn std::error::Error>> {
    //1. Get the current git tag version (which depends on the updater mode)
    let current_git_version = get_cli_version(updater_config)?;

    //2. Convert the semver to a number and write it to a file (so it can persist across updates)
    write_version_to_file(&current_git_version)?;

    println!(
        "{}[auto-updater thread]{} Wrote version to file: {}",
        BLUE,
        RESET,
        current_git_version.to_string()
    );

    Ok(current_git_version)
}

pub fn download_and_apply_update(
    new_version: &Version,
    current_version: &Arc<RwLock<Version>>,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}[auto-updater thread]{} Using repo path: {}",
        BLUE, RESET, config.repo_path
    );

    if config.mode == AutoUpdaterMode::Test {
        // 1. Verify repo exists
        if !std::path::Path::new(&config.repo_path).exists() {
            return Err(format!("Repository not found at: {}", config.repo_path).into());
        }

        // 2. Skip build (cargo run will handle it)
        println!(
            "{}[auto-updater thread]{} Starting new version...",
            BLUE, RESET
        );

        // 3. Restart with new version
        println!(
            "{}[auto-updater thread]{} Restarting with new version... in test mode",
            BLUE, RESET
        );
        restart_cli_process_with_new_version(new_version, current_version, config)?;
        Ok(())
    } else {
        // Production update logic
        println!(
            "{}[auto-updater thread]{} Updating production installation...",
            BLUE, RESET
        );

        // 1. Verify existing installation
        if !std::path::Path::new(&config.repo_path).exists() {
            return Err(format!(
                "Repository not found at {}. Please reinstall the CLI.",
                config.repo_path
            )
            .into());
        }

        // 2. Fetch updates from remote
        println!("{}[auto-updater thread]{} Fetching updates...", BLUE, RESET);
        Command::new("git")
            .args(["fetch", "--all", "--tags", "--prune"])
            .current_dir(&config.repo_path)
            .output()?;

        // 3. Checkout the new version
        println!(
            "{}[auto-updater thread]{} Checking out version {}...",
            BLUE,
            RESET,
            new_version.to_string()
        );
        let checkout_output = Command::new("git")
            .args(["checkout", &format!("tags/{}", new_version.to_string())])
            .current_dir(&config.repo_path)
            .output()?;

        if !checkout_output.status.success() {
            return Err(format!(
                "Failed to checkout version: {}",
                String::from_utf8_lossy(&checkout_output.stderr)
            )
            .into());
        }

        // 4. Build the new version
        let cli_path = std::path::Path::new(&config.repo_path);

        println!(
            "{}[auto-updater thread]{} Building new version...",
            BLUE, RESET
        );

        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&cli_path)
            .output()?;

        if !build_output.status.success() {
            return Err(format!(
                "Build failed: {}",
                String::from_utf8_lossy(&build_output.stderr)
            )
            .into());
        }

        // 5. Restart with new version
        println!(
            "{}[auto-updater thread]{} Restarting with new version... in production mode",
            BLUE, RESET
        );
        restart_cli_process_with_new_version(new_version, current_version, config)?;
        Ok(())
    }
}
