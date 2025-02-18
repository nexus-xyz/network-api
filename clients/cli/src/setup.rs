use colored::Colorize;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;

// Update the import path to use the proto module
// use crate::config;
// use crate::orchestrator_client::OrchestratorClient;
use crate::prover_id_manager::get_or_generate_prover_id;

pub enum SetupResult {
    Anonymous,
    Connected(String), // String could be the public key or other connection info
    Invalid,
}

#[derive(Serialize, Deserialize)]
pub struct UserConfig {
    pub node_id: String,
    pub user_id: Option<String>,
}

fn save_user_config(user_id: &str, node_id: &str) -> std::io::Result<()> {
    let proj_dirs =
        ProjectDirs::from("xyz", "nexus", "cli").expect("Failed to determine config directory");

    let config_dir = proj_dirs.config_dir();
    fs::create_dir_all(config_dir)?;

    let config_path = config_dir.join("user.json");
    let config = UserConfig {
        user_id: Some(user_id.to_string()),
        node_id: node_id.to_string(),
    };

    fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    //print the user config was saved properly
    println!("User ID: {}", user_id);
    println!("Node ID: {}", node_id);
    println!("User config saved to: {}", config_path.to_string_lossy());

    Ok(())
}

//function that takes a node_id and saves it to the user config
fn save_node_id(node_id: &str) -> std::io::Result<()> {
    get_or_generate_prover_id(node_id);

    Ok(())
}

pub async fn run_initial_setup() -> SetupResult {
    // Get home directory and check for prover-id file
    let home_path = home::home_dir().expect("Failed to determine home directory");
    let prover_id_path = home_path.join(".nexus").join("prover-id");

    let node_id = match fs::read_to_string(&prover_id_path) {
        Ok(content) => content,
        Err(_) => String::new(), // Return empty string if file doesn't exist or can't be read
    };

    if prover_id_path.exists() {
        println!(
            "\nThis node is already connected to an account using node id: {}",
            node_id
        );

        //ask the user if they want to use the existing config
        println!("Do you want to use the existing user account? (y/n)");
        let mut use_existing_config = String::new();
        std::io::stdin()
            .read_line(&mut use_existing_config)
            .unwrap();
        let use_existing_config = use_existing_config.trim();
        if use_existing_config == "y" {
            match fs::read_to_string(&prover_id_path) {
                Ok(content) => {
                    println!("\nUsing existing node ID: {}", content.trim());
                    return SetupResult::Connected(content.trim().to_string());
                }
                Err(e) => {
                    println!("{}", format!("Failed to read prover-id file: {}", e).red());
                    return SetupResult::Invalid;
                }
            }
        } else {
            println!("Ignoring existing user account...");
        }
    }

    println!("\nThis node is not connected to any account.\n");
    println!("[1] Enter '1' to start proving without earning NEX");
    println!("[2] Enter '2' to start earning NEX by connecting adding your node ID");

    let mut option = String::new();
    std::io::stdin().read_line(&mut option).unwrap();
    let option = option.trim();

    //if no config file exists, ask the user to enter their email
    match option {
        "1" => {
            println!("You chose option 1\n");
            SetupResult::Anonymous
        }
        "2" => {
            println!(
                "\n===== {} =====\n",
                "Adding your node ID to the CLI"
                    .bold()
                    .underline()
                    .bright_cyan()
            );
            println!("You chose to start earning NEX by connecting your node ID\n");
            println!("If you don't have a node ID, you can get it by following these steps:\n");
            println!("1. Go to https://app.nexus.xyz/nodes");
            println!("2. Sign in");
            println!("3. Click on the '+ Add Node' button");
            println!("4. Select 'Add CLI node'");
            println!("5. You will be given a node ID to add to this CLI");
            println!("6. Enter the node ID into the terminal below:\n");

            let node_id = get_node_id_from_user();
            match save_node_id(&node_id) {
                Ok(_) => SetupResult::Connected(node_id),
                Err(e) => {
                    println!("{}", format!("Failed to save node ID: {}", e).red());
                    SetupResult::Invalid
                }
            }
        }
        _ => {
            println!("Invalid option");
            SetupResult::Invalid
        }
    }
}

pub fn clear_user_config() -> std::io::Result<()> {
    // Clear prover-id file
    let home_path = home::home_dir().expect("Failed to determine home directory");
    let prover_id_path = home_path.join(".nexus").join("prover-id");
    if prover_id_path.exists() {
        fs::remove_file(prover_id_path)?;
        println!("Cleared prover ID configuration");
    }

    println!("User configuration cleared");
    Ok(())
}

fn get_node_id_from_user() -> String {
    println!("{}", "Please enter your node ID:".green());
    let mut node_id = String::new();
    std::io::stdin()
        .read_line(&mut node_id)
        .expect("Failed to read node ID");
    node_id.trim().to_string()
}
