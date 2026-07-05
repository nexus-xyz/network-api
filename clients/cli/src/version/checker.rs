//! Version Checking Module
//!
//! Checks for new versions of the CLI by querying the GitHub API
//!
//! # Mock Testing
//!
//! This module provides several approaches for mock testing:
//!
//! ## 1. **Trait-Based Mocking (Recommended)**
//!
//! Uses the `VersionCheckable` trait with `mockall` for comprehensive mocking:
//!
//! ```rust
//! use mockall::predicate::*;
//! use version_checker::{MockVersionCheckable, version_checker_task_with_interval};
//!
//! #[tokio::test]
//! async fn test_version_update_detection() {
//!     let mut mock_checker = MockVersionCheckable::new();
//!     mock_checker
//!         .expect_current_version()
//!         .return_const("0.9.0".to_string());
//!
//!     mock_checker
//!         .expect_check_latest_version()
//!         .returning(|| Ok(create_mock_release("v0.9.1")));
//!
//!     let (event_sender, mut event_receiver) = mpsc::channel(10);
//!     let (shutdown_sender, shutdown_receiver) = broadcast::channel(1);
//!
//!     let task_handle = tokio::spawn(async move {
//!         version_checker_task_with_interval(
//!             Box::new(mock_checker),
//!             event_sender,
//!             shutdown_receiver,
//!             Duration::from_millis(100),
//!         ).await;
//!     });
//!
//!     // Test assertions here...
//! }
//! ```
//!
//! ## 2. **Dependency Injection**
//!
//! The `version_checker_task` function accepts `Box<dyn VersionCheckable>`, making it
//! easy to inject mocks or different implementations:
//!
//! ```rust
//! // Production code
//! let real_checker = Box::new(VersionChecker::new("0.9.0".to_string()));
//! version_checker_task(real_checker, event_sender, shutdown).await;
//!
//! // Test code
//! let mock_checker = Box::new(mock_version_checker);
//! version_checker_task(mock_checker, event_sender, shutdown).await;
//! ```
//!
//! ## 3. **Testing Different Scenarios**
//!
//! The mock system supports testing various scenarios:
//!
//! - **Update Available**: Mock returns newer version
//! - **No Update**: Mock returns same/older version
//! - **API Errors**: Mock returns errors to test error handling
//! - **Multiple Checks**: Test that duplicate notifications are prevented
//! - **Rate Limiting**: Test with different check intervals
//!
//! ## Mock Testing Best Practices
//!
//! 1. **Use `times()` wisely**: Specify expected call counts for better validation
//! 2. **Test edge cases**: Invalid versions, network errors, malformed responses
//! 3. **Verify event contents**: Check message format, log levels, event types
//! 4. **Test timing**: Use configurable intervals for faster tests
//! 5. **Clean shutdown**: Always test graceful shutdown scenarios

use reqwest::{Client, ClientBuilder};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[cfg(test)]
use mockall::{automock, predicate::*};

// GitHub API endpoint for the latest release
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/nexus-xyz/nexus-cli/releases/latest";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub published_at: String,
    pub html_url: String,
    pub prerelease: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub last_check: Option<Instant>,
}

impl VersionInfo {
    pub fn new(current_version: String) -> Self {
        Self {
            current_version,
            latest_version: None,
            update_available: false,
            release_url: None,
            last_check: None,
        }
    }

    pub fn update_from_release(&mut self, release: GitHubRelease) {
        self.latest_version = Some(release.tag_name.clone());
        self.release_url = Some(release.html_url);
        self.update_available = self.is_newer_version(&release.tag_name);
        self.last_check = Some(Instant::now());
    }

    /// Compare semantic versions to determine if the latest version is newer
    fn is_newer_version(&self, latest: &str) -> bool {
        match (parse_version(&self.current_version), parse_version(latest)) {
            (Ok(current), Ok(latest_ver)) => latest_ver > current,
            _ => false, // If parsing fails, assume no update needed
        }
    }
}

/// Parse a version string, handling optional 'v' prefix
fn parse_version(version: &str) -> Result<Version, semver::Error> {
    let clean_version = version.strip_prefix('v').unwrap_or(version);
    Version::parse(clean_version)
}

/// Trait for version checking - allows for easy mocking in tests
#[cfg_attr(test, automock)]
#[async_trait::async_trait]
pub trait VersionCheckable: Send + Sync {
    /// Check for the latest version from the remote source
    async fn check_latest_version(
        &self,
    ) -> Result<GitHubRelease, Box<dyn std::error::Error + Send + Sync>>;
}

/// Version checker client for making GitHub API requests
pub struct VersionChecker {
    client: Client,
}

impl VersionChecker {
    /// Creates a new `VersionChecker`.
    ///
    /// Returns an error instead of panicking if the underlying HTTP client
    /// cannot be initialized (e.g. TLS stack unavailable).  Callers should
    /// treat a construction failure as non-fatal and skip the version check
    /// rather than aborting the process.
    pub fn new(current_version: String) -> Result<Self, reqwest::Error> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .user_agent(format!("nexus-cli/{}", current_version))
            .build()?;

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl VersionCheckable for VersionChecker {
    /// Check for latest version from GitHub API
    async fn check_latest_version(
        &self,
    ) -> Result<GitHubRelease, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.client.get(GITHUB_RELEASES_URL).send().await?;

        if !response.status().is_success() {
            return Err(format!("GitHub API returned status: {}", response.status()).into());
        }

        let release: GitHubRelease = response.json().await?;
        Ok(release)
    }
}

/// Check if a new version is available and return notification message
pub async fn check_for_new_version(current_version: &str) -> Option<String> {
    let version_checker = match VersionChecker::new(current_version.to_string()) {
        Ok(v) => v,
        Err(_) => return None, // Non-fatal: skip version check if client init fails
    };

    if let Ok(release) = version_checker.check_latest_version().await {
        let mut version_info = VersionInfo::new(current_version.to_string());
        version_info.update_from_release(release.clone());

        if version_info.update_available {
            return Some(format!(
                "New version {} is available (current: {}). Download: {}",
                release.tag_name, current_version, release.html_url
            ));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        // Test version comparison logic
        let info_090 = VersionInfo::new("0.9.0".to_string());
        let info_091 = VersionInfo::new("0.9.1".to_string());
        let info_100 = VersionInfo::new("1.0.0".to_string());

        // Test newer version detection
        assert!(info_090.is_newer_version("0.9.1"));
        assert!(info_090.is_newer_version("v0.9.1"));
        assert!(info_091.is_newer_version("1.0.0"));
        assert!(info_091.is_newer_version("v1.0.0"));

        // Test same version
        assert!(!info_091.is_newer_version("0.9.1"));
        assert!(!info_091.is_newer_version("v0.9.1"));

        // Test older version
        assert!(!info_091.is_newer_version("0.9.0"));
        assert!(!info_100.is_newer_version("0.9.1"));
    }

    #[test]
    fn test_version_info_update() {
        let mut info = VersionInfo::new("0.9.0".to_string());

        let release = GitHubRelease {
            tag_name: "v0.9.1".to_string(),
            name: "Release v0.9.1".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            html_url: "https://github.com/nexus-xyz/nexus-cli/releases/tag/v0.9.1".to_string(),
            prerelease: false,
        };

        info.update_from_release(release);

        assert!(info.update_available);
        assert_eq!(info.latest_version, Some("v0.9.1".to_string()));
    }

    #[test]
    fn test_edge_case_version_comparisons() {
        // Test various edge cases with semver
        let info_100 = VersionInfo::new("1.0.0".to_string());
        let info_1100 = VersionInfo::new("1.10.0".to_string());
        let info_1010 = VersionInfo::new("1.0.10".to_string());
        let info_20 = VersionInfo::new("2.0.0".to_string());

        // Test major version differences
        assert!(info_100.is_newer_version("2.0.0"));
        assert!(!info_20.is_newer_version("1.9.9"));

        // Test minor version differences
        assert!(info_100.is_newer_version("1.10.0"));
        assert!(!info_1100.is_newer_version("1.9.0"));

        // Test patch version differences
        assert!(info_100.is_newer_version("1.0.10"));
        assert!(!info_1010.is_newer_version("1.0.9"));

        // Test that semver handles malformed versions gracefully
        assert!(!info_100.is_newer_version("not.a.version"));
        assert!(!info_100.is_newer_version(""));
    }
}
