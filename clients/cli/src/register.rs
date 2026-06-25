//! Registering a new user and node with the orchestrator.

use crate::cli_messages::{print_error, print_info, print_success};
use crate::config::Config;
use crate::keys;
use crate::orchestrator::Orchestrator;
use std::path::Path;

/// Registers a user with the orchestrator.
///
/// # Arguments
/// * `wallet_address` - The Ethereum wallet address of the user.
/// * `config_path` - The path to the configuration file where user details will be saved.
/// * `orchestrator` - The orchestrator client to communicate with the orchestrator.
pub async fn register_user(
    wallet_address: &str,
    config_path: &Path,
    orchestrator: Box<dyn Orchestrator>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if the wallet address is valid.
    if !keys::is_valid_eth_address(wallet_address) {
        print_error(
            "Invalid Ethereum wallet address",
            Some(&format!(
                "It should be a 42-character hex string starting with '0x', got: {}",
                wallet_address
            )),
        );
        let err_msg = format!(
            "Invalid Ethereum wallet address: {}. It should be a 42-character hex string starting with '0x'.",
            wallet_address
        );
        return Err(Box::from(err_msg));
    }

    // Check if the config file exists and contains this wallet address and a user ID.
    if config_path.exists() {
        if let Ok(config) = Config::load_from_file(config_path) {
            if config.wallet_address.to_lowercase() == wallet_address.to_lowercase()
                && !config.user_id.is_empty()
            {
                print_info(
                    "User already registered",
                    &format!(
                        "User ID: {}, Wallet Address: {}",
                        config.user_id, config.wallet_address
                    ),
                );

                // Guide user to next step
                print_success(
                    "User registration complete!",
                    "Next step - register a node: nexus-compute-cli register-node",
                );
                return Ok(());
            }
        }
    }

    // Check if the wallet address is already registered with the orchestrator.
    if let Ok(user_id) = orchestrator.get_user(wallet_address).await {
        print_info(
            "Wallet address is already registered",
            &format!("User ID: {}, Wallet Address: {}", user_id, wallet_address),
        );
        let config = Config::new(
            user_id,
            wallet_address.to_string(),
            String::new(), // node_id is empty for now
            orchestrator.environment().clone(),
        );
        // Save the configuration file with the user ID and wallet address.
        config.save(config_path).inspect_err(|e| {
            print_error("Failed to save config", Some(&e.to_string()));
        })?;

        // Guide user to next step
        print_success(
            "User registration complete!",
            "Next step - register a node: nexus-compute-cli register-node",
        );

        return Ok(());
    }

    // Otherwise, register the user with the orchestrator.
    let uuid = uuid::Uuid::new_v4().to_string();
    match orchestrator.register_user(&uuid, wallet_address).await {
        Ok(_) => {
            print_success(
                "User registered successfully",
                &format!("User ID: {}", uuid),
            );
        }
        Err(e) => {
            // Check if this looks like an orchestrator traffic issue
            if let Some(pretty_error) = e.to_pretty() {
                print_error("Failed to register user", Some(&pretty_error));
            } else {
                print_error("Failed to register user", Some(&e.to_string()));
            }

            return Err(e.into());
        }
    }

    // Save the configuration file with the user ID and wallet address.
    let config = Config::new(
        uuid,
        wallet_address.to_string(),
        String::new(), // node_id is empty for now
        orchestrator.environment().clone(),
    );
    config.save(config_path).inspect_err(|e| {
        print_error("Failed to save config", Some(&e.to_string()));
    })?;

    // Guide user to next step
    print_success(
        "User registration complete!",
        "Next step - register a node: nexus-compute-cli register-node",
    );

    Ok(())
}

/// Registers a node with the orchestrator.
///
/// # Arguments
/// * `node_id` - Optional node ID. If provided, it will be used to register the node.
/// * `config_path` - The path to the configuration file where node details will be saved.
/// * `orchestrator` - The orchestrator client to communicate with the orchestrator.
pub async fn register_node(
    node_id: Option<u64>,
    config_path: &Path,
    orchestrator: Box<dyn Orchestrator>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Register a new node, or link an existing node to a user.
    // Requires: a config file with a registered user.
    // If a node_id is provided, update the config with it and use it.
    // If no node_id is provided, generate a new one.
    let mut config = Config::load_from_file(config_path).inspect_err(|e| {
        print_error(
            "Failed to load config, please register a user first",
            Some(&e.to_string()),
        );
    })?;
    if config.user_id.is_empty() {
        print_error("No user registered", Some("Please register a user first."));
        return Err(Box::from(
            "No user registered. Please register a user first.",
        ));
    }
    if let Some(node_id) = node_id {
        // If a node_id is provided, update the config with it.
        println!("Registering node ID: {}", node_id);
        config.node_id = node_id.to_string();
        config.save(config_path).inspect_err(|e| {
            print_error("Failed to save updated config", Some(&e.to_string()));
        })?;

        // Guide user to next step
        print_success(
            "Node registration complete!",
            &format!(
                "Successfully registered node with ID: {}. Next step - start proving: nexus-compute-cli start",
                node_id
            ),
        );

        Ok(())
    } else {
        println!(
            "No node ID provided. Registering a new node in environment: {:?}",
            orchestrator.environment()
        );
        match orchestrator.register_node(&config.user_id).await {
            Ok(node_id) => {
                // Update the config with the new node ID
                let mut updated_config = config;
                updated_config.node_id = node_id.clone();
                updated_config.save(config_path).inspect_err(|e| {
                    print_error("Failed to save updated config", Some(&e.to_string()));
                })?;

                // Guide user to next step
                print_success(
                    "Node registration complete!",
                    &format!(
                        "Successfully registered node with ID: {}. Next step - start proving: nexus-compute-cli start",
                        node_id
                    ),
                );

                Ok(())
            }
            Err(e) => {
                print_error("Failed to register node", Some(&e.to_string()));
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::Environment;
    use crate::orchestrator::MockOrchestrator;
    use crate::orchestrator::error::OrchestratorError;
    use predicates::ord::eq;
    use tempfile::tempdir;

    /// Happy-path: wallet *not* yet known to orchestrator.
    #[tokio::test]
    async fn registers_new_wallet_and_writes_config() {
        const WALLET: &str = "0x1234567890123456789012345678901234567890";
        // ---- temp file that lives only for the test ----
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        // ---- mock orchestrator behaviour ----
        let mut orchestrator = MockOrchestrator::new();
        orchestrator
            .expect_environment()
            .return_const(Environment::Production); // whatever you need here

        orchestrator
            .expect_get_user()
            .with(eq(WALLET))
            .returning(|_| {
                Err(OrchestratorError::Http {
                    status: 404,
                    message: "User not found".to_string(),
                    headers: std::collections::HashMap::new(),
                })
            });

        orchestrator
            .expect_register_user()
            .withf(|uid, addr| addr == WALLET && uuid::Uuid::parse_str(uid).is_ok())
            .returning(|_, _| Ok(()));

        // ---- call the function under test ----
        register_user(WALLET, &path, Box::new(orchestrator))
            .await
            .expect("registration should succeed");

        // ---- verify side-effects *inside* the sandbox ----
        let cfg = Config::load_from_file(&path).unwrap();
        assert_eq!(cfg.wallet_address.to_lowercase(), WALLET.to_lowercase());
        assert!(!cfg.user_id.is_empty());
    }

    #[tokio::test]
    /// Config file already exists with a registered user.
    async fn skips_registration_if_config_matches_wallet_and_user_id() {
        use std::fs::File;
        use std::io::Write;
        use tempfile::tempdir;

        // Setup: create a temp directory and a config file in it
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        let wallet_address = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
        let user_id = "existing-user-id";

        // Write a pre-existing config with matching wallet and user_id
        let config_json = format!(
            r#"{{
            "user_id": "{}",
            "wallet_address": "{}",
            "node_id": "",
            "environment": "Staging"
        }}"#,
            user_id, wallet_address
        );
        let mut file = File::create(&config_path).unwrap();
        file.write_all(config_json.as_bytes()).unwrap();
        drop(file); // ensure the file is flushed and closed

        // MockOrchestrator that must not be called
        let mut orchestrator = MockOrchestrator::new();
        orchestrator.expect_get_user().never();
        orchestrator.expect_register_user().never();

        // Call the function
        let result = register_user(wallet_address, &config_path, Box::new(orchestrator)).await;

        assert!(result.is_ok(), "should succeed without making any requests");

        // Confirm the file was not modified (still contains same user ID)
        let config = Config::load_from_file(&config_path).unwrap();
        assert_eq!(config.user_id, user_id);
        assert_eq!(
            config.wallet_address.to_lowercase(),
            wallet_address.to_lowercase()
        );
    }
}
