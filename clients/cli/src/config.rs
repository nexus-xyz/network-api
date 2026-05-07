//! Application configuration.

use crate::cli_messages::{print_error, print_info, print_success};
use crate::environment::Environment;
use crate::orchestrator::Orchestrator;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Get the path to the Nexus config file, typically located at ~/.nexus/config.json.
pub fn get_config_path() -> Result<PathBuf, std::io::Error> {
    let home_path = home::home_dir().ok_or(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Home directory not found",
    ))?;
    let config_path = home_path.join(".nexus").join("config.json");
    Ok(config_path)
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    /// Environment from config file
    #[serde(default)]
    pub environment: String,

    /// User ID from config file
    #[serde(default)]
    pub user_id: String,

    /// Wallet address, resolved during `Config::resolve`
    #[serde(default)]
    pub wallet_address: String,

    /// Node ID, resolved to a valid u64 during `Config::resolve`
    #[serde(default)]
    pub node_id: String,
}

impl Config {
    /// Create Config with the given node_id.
    pub fn new(
        user_id: String,
        wallet_address: String,
        node_id: String,
        environment: Environment,
    ) -> Self {
        Config {
            user_id,
            wallet_address,
            node_id,
            environment: environment.to_string(),
        }
    }

    /// Loads configuration from a JSON file at the given path.
    pub fn load_from_file(path: &Path) -> Result<Self, std::io::Error> {
        let buf = fs::read(path)?;
        let config: Config = serde_json::from_slice(&buf)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(config)
    }

    /// Saves the configuration to a JSON file at the given path.
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Serialization failed: {}", e),
            )
        })?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Clear the node ID configuration file.
    pub fn clear_node_config(path: &Path) -> std::io::Result<()> {
        if !path.exists() {
            println!("No config file found at {}", path.display());
            return Ok(());
        }
        if !path.ends_with("config.json") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path must end with config.json",
            ));
        }
        fs::remove_file(path)
    }

    /// Resolves configuration and ensures node_id is available
    pub async fn resolve(
        node_id_arg: Option<u64>,
        config_path: &Path,
        orchestrator: &impl Orchestrator,
    ) -> Result<Self, Box<dyn Error>> {
        // Special case: if --node-id is provided, allow running without config file
        if let Some(node_id) = node_id_arg {
            print_success("Using provided Node ID", &format!("Node ID: {}", node_id));

            // Get the wallet address for analytics
            let wallet_address = orchestrator.get_node(&node_id.to_string()).await?;

            // Resolve the user_id for this wallet so that analytics and reward
            // tracking use the real identity instead of a placeholder string.
            // A lookup failure is non-fatal: fall back to an empty string so the
            // node can still prove and submit tasks.
            let user_id = orchestrator
                .get_user(&wallet_address)
                .await
                .unwrap_or_else(|_| String::new());

            let config = Config {
                user_id,
                wallet_address,
                node_id: node_id.to_string(),
                environment: "".to_string(),
            };

            return Ok(config);
        }

        // For normal operation without --node-id, config file is required
        if !config_path.exists() {
            print_info(
                "Welcome to Nexus CLI!",
                "Please register your wallet address to get started: nexus-cli register-user --wallet-address <your-wallet-address>",
            );
            return Err("Configuration file not found. Please register first.".into());
        }

        // Load the config file
        let mut config = Config::load_from_file(config_path)?;

        // Resolve node_id from config file
        let resolved_node_id = match config.resolve_node_id_from_config() {
            Ok(id_from_config) => {
                print_success(
                    "Found Node ID from config file",
                    &format!("Node ID: {}", id_from_config),
                );
                id_from_config
            }
            Err(e) => {
                // The config is present but incomplete or invalid
                print_error(
                    "Your configuration is incomplete or invalid.",
                    Some("Please register your node. Start with: nexus-cli register-node"),
                );
                return Err(e);
            }
        };

        // Get the wallet address for analytics
        let wallet_address = orchestrator.get_node(&resolved_node_id.to_string()).await?;

        // Populate the config struct with the resolved values
        config.node_id = resolved_node_id.to_string();
        config.wallet_address = wallet_address;

        Ok(config)
    }

    /// Resolves node ID from the configuration file content
    fn resolve_node_id_from_config(&self) -> Result<u64, Box<dyn Error>> {
        if self.user_id.is_empty() {
            return Err("User not registered in config file.".into());
        }

        if self.node_id.is_empty() {
            print_error(
                "User registered, but no node found",
                Some("Please register a node to continue: nexus-cli register-node"),
            );
            return Err(
                "Node registration required. Please run 'nexus-cli register-node' first.".into(),
            );
        }

        match self.node_id.parse::<u64>() {
            Ok(id) => Ok(id),
            Err(_) => {
                print_error(
                    "Invalid node ID in config file",
                    Some("Please register a new node: nexus-cli register-node"),
                );
                Err(
                    "Invalid node ID in config. Please run 'nexus-cli register-node' to fix this."
                        .into(),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    /// Helper function to create a test configuration.
    fn get_config() -> Config {
        Config {
            environment: "test".to_string(),
            user_id: "test_user_id".to_string(),
            wallet_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            node_id: "test_node_id".to_string(),
        }
    }

    #[test]
    // Loading a saved configuration file should return the same configuration.
    fn test_load_recovers_saved_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = get_config();
        config.save(&path).unwrap();

        let loaded_config = Config::load_from_file(&path).unwrap();
        assert_eq!(config, loaded_config);
    }

    #[test]
    // Saving a configuration should create directories if they don't exist.
    fn test_save_creates_directories() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent_dir").join("config.json");
        let config = get_config();
        let result = config.save(&path);

        // Check if the directories were created
        assert!(result.is_ok(), "Failed to save config");
        assert!(
            path.parent().unwrap().exists(),
            "Parent directory does not exist"
        );
    }

    #[test]
    // Saving a configuration should overwrite an existing file.
    fn test_save_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Create an initial config and save it
        let mut config1 = get_config();
        config1.user_id = "test_user_id".to_string();
        config1.save(&path).unwrap();

        // Create a new config and save it to the same path
        let mut config2 = get_config();
        config2.user_id = "new_test_user_id".to_string();
        config2.save(&path).unwrap();

        // Load the saved config and check if it matches the second one
        let loaded_config = Config::load_from_file(&path).unwrap();
        assert_eq!(config2, loaded_config);
    }

    #[test]
    // Loading an invalid JSON file should return an error.
    fn test_load_rejects_invalid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("invalid_config.json");

        let mut file = File::create(&path).unwrap();
        writeln!(file, "invalid json").unwrap();

        let result = Config::load_from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    // Clearing the node configuration file should remove it if it exists.
    fn test_clear_node_config_removes_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = get_config();
        config.save(&path).unwrap();

        Config::clear_node_config(&path).unwrap();
        assert!(!path.exists(), "Config file was not removed");
    }

    #[test]
    // Should load JSON containing a user_id and empty strings for other fields.
    fn test_load_config_with_user_id_and_empty_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Write a JSON with user_id and empty strings for other fields
        let mut file = File::create(&path).unwrap();
        writeln!(file, r#"{{ "user_id": "test_user", "wallet_address": "", "environment": "", "node_id": "" }}"#).unwrap();

        match Config::load_from_file(&path) {
            Ok(config) => {
                // The user_id must be set correctly.
                assert_eq!(config.user_id, "test_user");
                // Other fields should be empty or default
                assert!(config.wallet_address.is_empty());
                assert!(config.environment.is_empty());
                assert!(config.node_id.is_empty());
            }
            Err(e) => {
                panic!("Failed to load config with user_id and empty fields: {}", e);
            }
        }
    }

    #[test]
    // (Backwards compatibility) Should load JSON containing only node_id.
    fn test_load_config_with_only_node_id() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Write a minimal JSON with only node_id
        let mut file = File::create(&path).unwrap();
        writeln!(file, r#"{{ "node_id": "12345" }}"#).unwrap();

        match Config::load_from_file(&path) {
            Ok(config) => {
                // The node_id must be set correctly.
                assert_eq!(config.node_id, "12345");
                // Other fields should be empty or default
                assert!(config.user_id.is_empty());
                assert!(config.wallet_address.is_empty());
                assert!(config.environment.is_empty());
            }
            Err(e) => {
                panic!("Failed to load config with only node_id: {}", e);
            }
        }
    }

    #[test]
    // (Backwards compatibility) Should load JSON with node_id and empty strings for other fields.
    fn test_load_config_with_node_id_and_empty_strings() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = Config {
            environment: "".to_string(),
            user_id: "".to_string(),
            wallet_address: "".to_string(),
            node_id: "12345".to_string(),
        };
        config.save(&path).unwrap();

        match Config::load_from_file(&path) {
            Ok(config) => {
                // The node_id must be set correctly.
                assert_eq!(config.node_id, "12345");
                // Other fields should be empty or default
                assert!(config.user_id.is_empty());
                assert!(config.wallet_address.is_empty());
                assert!(config.environment.is_empty());
            }
            Err(e) => {
                panic!("Failed to load config with only node_id: {}", e);
            }
        }
    }

    #[test]
    // Should ignore unexpected fields in the JSON.
    fn test_load_config_with_additional_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Write a JSON with additional fields
        let mut file = File::create(&path).unwrap();
        writeln!(file, r#"{{ "node_id": "12345", "extra_field": "value" }}"#).unwrap();

        match Config::load_from_file(&path) {
            Ok(config) => {
                // The node_id must be set correctly.
                assert_eq!(config.node_id, "12345");
                // Other fields should be empty or default
                assert!(config.user_id.is_empty());
                assert!(config.wallet_address.is_empty());
                assert!(config.environment.is_empty());
            }
            Err(e) => {
                panic!("Failed to load config with additional fields: {}", e);
            }
        }
    }
}
