//! Core auto-updater functionality and version management
//!
//! This module provides the underlying implementation for version checking and updates
//! using the self_update crate to handle version management and updates from GitHub releases.

use self_update::{cargo_crate_version, self_replace, ArchiveKind, Compression, Extract};
use semver::Version;
use std::path::Path;
use std::process::Command;

// ANSI escape codes for colors for pretty printing
pub const BLUE: &str = "\x1b[34m";
pub const RESET: &str = "\x1b[0m";

#[derive(Clone)]
pub struct UpdaterConfig {
    pub update_interval: u64,
    pub repo_path: String,
    pub remote_repo: String,
    pub hostname: String,
}

impl UpdaterConfig {
    pub fn new(hostname: String) -> Self {
        // Load .env file relative to project root
        let env_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
        println!(
            "Looking for .env at: {:?}",
            env_path.canonicalize().unwrap_or_default()
        );
        // match dotenv::from_path(&env_path) {
        //     Ok(_) => println!("Successfully loaded .env file"),
        //     Err(e) => println!("Failed to load .env file: {}", e),
        // }

        #[cfg(debug_assertions)]
        let config = Self {
            repo_path: std::env::current_dir()
                .expect("Failed to get current directory")
                .to_string_lossy()
                .into_owned(),
            remote_repo: String::from("."),
            update_interval: 30, // 30 seconds in debug mode
            hostname,
        };

        #[cfg(not(debug_assertions))]
        let config = Self {
            repo_path: format!(
                "{}/.nexus/network-api",
                std::env::var("HOME").unwrap_or_default()
            ),
            remote_repo: String::from("https://github.com/nexus-xyz/network-api.git"),
            update_interval: 3600, // 1 hour in release mode
            hostname,
        };

        println!(
            "{}[auto-updater]{} Checking for updates in {} seconds",
            BLUE, RESET, config.update_interval
        );

        config
    }
}

pub enum VersionStatus {
    UpdateAvailable(Version),
    UpToDate,
}

pub struct VersionManager {
    // config: UpdaterConfig,
    version_file: std::path::PathBuf,
}

impl VersionManager {
    pub fn new(config: UpdaterConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Get the full path to version file
        let version_file = get_binary_path().join("version");

        // Initialize version file if it doesn't exist
        if !version_file.exists() {
            if let Some(parent) = version_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&version_file, env!("CARGO_PKG_VERSION"))?;
        }

        Ok(Self {
            // config,
            version_file,
        })
    }

    pub fn update_version_status(&self) -> Result<VersionStatus, Box<dyn std::error::Error>> {
        println!("{}[auto-updater]{} Checking for updates...", BLUE, RESET);

        let current_version = self.get_current_version()?;
        let status = tokio::task::block_in_place(|| {
            let mut config = self_update::backends::github::Update::configure();

            let update_builder = config
                .repo_owner("nexus-xyz")
                .repo_name("network-api")
                .bin_name("prover")
                .current_version(cargo_crate_version!())
                .target(&self_update::get_target())
                .no_confirm(true);

            // Check if a GitHub token is available
            // if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            //     if !token.is_empty() {
            //         update_builder = update_builder.auth_token(token.as_str());
            //     }
            // }

            update_builder.build()?.get_latest_release()
        })?;

        // Compare versions
        if current_version.to_string() == status.version {
            println!(
                "{}[auto-updater]{} Versions match - no update needed",
                BLUE, RESET
            );
            Ok(VersionStatus::UpToDate)
        } else {
            println!(
                "{}[auto-updater]{} Update available: {} -> {}",
                BLUE, RESET, current_version, status.version
            );
            Ok(VersionStatus::UpdateAvailable(Version::parse(
                &status.version,
            )?))
        }
    }

    pub fn apply_update(
        &self,
        new_version: &Version,
    ) -> Result<(), Box<dyn std::error::Error + Send + 'static>> {
        println!(
            "{}[auto-updater]{} Inspecting update version {}...",
            BLUE, RESET, new_version
        );

        // Create a temporary directory for extraction
        let temp_dir = tempfile::Builder::new()
            .prefix("prover-update")
            .tempdir()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        // Download the release
        // let token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
        let mut config = self_update::backends::github::Update::configure();
        let mut update_builder = config.repo_owner("nexus-xyz");
        update_builder = update_builder
            .repo_name("network-api")
            .bin_name("prover")
            .current_version(cargo_crate_version!())
            .target(&self_update::get_target())
            .no_confirm(true);

        // Conditionally add the auth token if it is present
        // if !token.is_empty() {
        //     update_builder = update_builder.auth_token(&token);
        // }

        let release = update_builder
            .build()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
            .get_latest_release()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        // Get the asset
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == "aarch64-apple-darwin.tar.gz")
            .ok_or_else(|| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No matching asset found",
                )) as Box<dyn std::error::Error + Send>
            })?;

        println!(
            "{}[auto-updater]{} Downloading archive: {}",
            BLUE, RESET, asset.name
        );

        // Download to temp file
        let download_path = temp_dir.path().join(&asset.name);
        let mut request = reqwest::blocking::Client::new()
            .get(&asset.download_url)
            .header("Accept", "application/octet-stream")
            .header("User-Agent", "NexusUpdater/0.3.7");

        // Conditionally add the Authorization header if the token is present
        // if !token.is_empty() {
        //     request = request.header("Authorization", format!("token {}", token));
        // }

        let response = request.send().map_err(|e| {
            eprintln!("Failed to send request: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;

        // Check if the response is successful
        if !response.status().is_success() {
            eprintln!("Failed to download file: HTTP {}", response.status());
            let error_body = response
                .text()
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            eprintln!("Error body: {}", error_body);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to download file",
            )) as Box<dyn std::error::Error + Send>);
        }

        let mut file = std::fs::File::create(&download_path).map_err(|e| {
            eprintln!("Failed to create file: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;
        std::io::copy(
            &mut response
                .bytes()
                .map_err(|e| {
                    eprintln!("Failed to read response bytes: {:?}", e);
                    Box::new(e) as Box<dyn std::error::Error + Send>
                })?
                .as_ref(),
            &mut file,
        )
        .map_err(|e| {
            eprintln!("Failed to write to file: {:?}", e);
            Box::new(e) as Box<dyn std::error::Error + Send>
        })?;

        // Get the file size
        let file_size = std::fs::metadata(&download_path)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
            .len();

        println!(
            "{}[auto-updater]{} Archive downloaded to: {:?} (size: {} bytes)",
            BLUE, RESET, download_path, file_size
        );

        // Use the existing temporary directory for extraction
        let extract_path = temp_dir.path();

        // Extract and inspect the contents
        match extract_and_prepare_update(&download_path, &extract_path) {
            Ok(_) => {
                // Define the path to the new executable
                let new_exe_path = extract_path.join("prover"); // Adjust "prover" to the actual executable name

                // Apply the update
                if let Err(e) = self_replace::self_replace(&new_exe_path) {
                    eprintln!(
                        "{}[auto-updater]{} Failed to apply update: {}",
                        BLUE, RESET, e
                    );
                    return Err(Box::new(e) as Box<dyn std::error::Error + Send>);
                }

                // Verify the version of the updated binary
                let output = Command::new(&new_exe_path)
                    .arg("--version")
                    .output()
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

                if output.status.success() {
                    println!(
                        "{}[auto-updater]{} Restarting with new version...",
                        BLUE, RESET
                    );
                } else {
                    eprintln!(
                        "{}[auto-updater]{} Failed to verify updated binary version",
                        BLUE, RESET
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "{}[auto-updater]{} Failed to extract and inspect: {}",
                    BLUE, RESET, e
                );
                return Err(e);
            }
        }

        // Update version file only after successful binary replacement
        std::fs::write(&self.version_file, new_version.to_string())
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;

        println!(
            "{}[auto-updater]{} Version file updated to {}",
            BLUE, RESET, new_version
        );

        Ok(())
    }

    pub fn get_current_version(&self) -> Result<Version, Box<dyn std::error::Error>> {
        if self.version_file.exists() {
            let version = std::fs::read_to_string(&self.version_file)?
                .trim()
                .to_string();
            println!(
                "{}[auto-updater]{} Current version: {} (from version file)",
                BLUE, RESET, version
            );
            return Ok(Version::parse(&version)?);
        }

        // Fallback to compile-time version
        let version = env!("CARGO_PKG_VERSION");
        println!(
            "{}[auto-updater]{} Current version: {} (from CARGO_PKG_VERSION)",
            BLUE, RESET, version
        );

        // Initialize version file with compile-time version
        if let Some(parent) = self.version_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.version_file, version)?;

        Ok(Version::parse(version)?)
    }
}

pub fn get_binary_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{}/.nexus/bin", home))
}

fn extract_and_prepare_update(
    archive_path: &Path,
    extract_path: &Path,
) -> Result<(), Box<dyn std::error::Error + Send>> {
    // Attempt to extract the archive
    match Extract::from_source(archive_path)
        .archive(ArchiveKind::Tar(Some(Compression::Gz)))
        .extract_into(extract_path)
    {
        Ok(_) => {
            // Print the extracted files for inspection
            for entry in std::fs::read_dir(extract_path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?
            {
                let entry = entry.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send>)?;
                println!("Extracted file: {:?}", entry.path());
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to extract archive: {:?}", e);
            Err(Box::new(e) as Box<dyn std::error::Error + Send>)
        }
    }
}
