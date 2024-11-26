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

pub struct VersionManager {
    current_version: Arc<RwLock<Version>>,
    config: UpdaterConfig,
}

impl VersionManager {
    pub fn new(config: UpdaterConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let current_version = Arc::new(RwLock::new(
            read_version_from_file().unwrap_or(FALLBACK_VERSION),
        ));
        Ok(Self {
            current_version,
            config,
        })
    }

    pub fn fetch_and_persist_cli_version(&self) -> Result<Version, Box<dyn std::error::Error>> {
        // 1. Get the current git tag version (which depends on the updater mode)
        let current_git_version = get_cli_version(&self.config)?;

        // 2. Convert the semver to a number and write it to a file (so it can persist across updates)
        write_version_to_file(&current_git_version)?;

        println!(
            "{}[auto-updater thread]{} Wrote version to file: {}",
            BLUE, RESET, current_git_version
        );

        Ok(current_git_version)
    }

    fn fetch_latest_version(&self) -> Result<Version, Box<dyn std::error::Error>> {
        let version = match self.config.mode {
            AutoUpdaterMode::Test => {
                let output = Command::new("git")
                    .args(["describe", "--tags", "--abbrev=0"])
                    .current_dir(&self.config.repo_path)
                    .output()?;
                Version::parse(String::from_utf8(output.stdout)?.trim())?
            }
            AutoUpdaterMode::Production => {
                let output = Command::new("git")
                    .args(["ls-remote", "--tags", "--refs", &self.config.remote_repo])
                    .output()?;
                let tags = String::from_utf8(output.stdout)?;
                tags.lines()
                    .last()
                    .and_then(|line| line.split('/').last())
                    .map(|v| v.to_string())
                    .and_then(|v| Version::parse(&v).ok())
                    .ok_or_else(|| Box::<dyn std::error::Error>::from("No tags found"))?
            }
        };
        write_version_to_file(&version)?;
        Ok(version)
    }

    pub fn apply_update(&self, new_version: &Version) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "{}[auto-updater thread]{} Using repo path: {}",
            BLUE, RESET, self.config.repo_path
        );

        match self.config.mode {
            AutoUpdaterMode::Test => self.apply_test_update(new_version),
            AutoUpdaterMode::Production => self.apply_production_update(new_version),
        }
    }

    fn apply_test_update(&self, new_version: &Version) -> Result<(), Box<dyn std::error::Error>> {
        if !std::path::Path::new(&self.config.repo_path).exists() {
            return Err(format!("Repository not found at: {}", self.config.repo_path).into());
        }

        println!(
            "{}[auto-updater thread]{} Starting new version...",
            BLUE, RESET
        );
        restart_cli_process_with_new_version(new_version, &self.current_version, &self.config)
    }

    fn apply_production_update(
        &self,
        new_version: &Version,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_path = std::path::Path::new(&self.config.repo_path);
        if !repo_path.exists() {
            return Err(format!("Repository not found at: {}", self.config.repo_path).into());
        }

        println!("{}[auto-updater thread]{} Fetching updates...", BLUE, RESET);
        Command::new("git")
            .args(["fetch", "--all", "--tags", "--prune"])
            .current_dir(repo_path)
            .output()?;

        println!(
            "{}[auto-updater thread]{} Checking out version {}...",
            BLUE, RESET, new_version
        );
        let checkout_output = Command::new("git")
            .args(["checkout", &format!("tags/{}", new_version)])
            .current_dir(repo_path)
            .output()?;

        if !checkout_output.status.success() {
            return Err(format!(
                "Failed to checkout version: {}",
                String::from_utf8_lossy(&checkout_output.stderr)
            )
            .into());
        }

        println!(
            "{}[auto-updater thread]{} Building new version...",
            BLUE, RESET
        );
        let build_output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(repo_path)
            .output()?;

        if !build_output.status.success() {
            return Err(format!(
                "Build failed: {}",
                String::from_utf8_lossy(&build_output.stderr)
            )
            .into());
        }

        restart_cli_process_with_new_version(new_version, &self.current_version, &self.config)
    }

    pub fn get_latest_available_version(
        &self,
    ) -> Result<VersionStatus, Box<dyn std::error::Error>> {
        let this_repo_version = self.current_version.read().clone();
        let latest_version = self.fetch_latest_version()?;

        println!(
            "{}[auto-updater thread]{} Current verrsion of CLI: {} | Latest version of CLI: {}",
            BLUE, RESET, this_repo_version, latest_version
        );

        if this_repo_version == latest_version {
            Ok(VersionStatus::UpToDate)
        } else {
            Ok(VersionStatus::UpdateAvailable(latest_version))
        }
    }
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

pub fn restart_cli_process_with_new_version(
    new_version: &Version,
    current_version: &Arc<RwLock<Version>>,
    config: &UpdaterConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update version tracking
    *current_version.write() = new_version.clone();
    write_version_to_file(new_version)?;

    let cli_path = std::path::Path::new(&config.repo_path);

    let mode_arg = match config.mode {
        AutoUpdaterMode::Test => "test",
        AutoUpdaterMode::Production => "production",
    };

    let child = Command::new("cargo")
        .args([
            "run",
            "--release",
            "--",
            &config.hostname,
            "--updater-mode",
            mode_arg,
        ])
        .current_dir(cli_path)
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
