use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

// Update the import path to use the proto module
use crate::node_id_manager::{
    create_nexus_directory, get_home_directory, handle_read_error, read_existing_node_id,
};

pub enum SetupResult {
    Anonymous,
    Connected(String),
    Invalid,
}

#[derive(Serialize, Deserialize)]
pub struct NodeConfig {
    pub node_id: String,
}

//function that takes a node_id and saves it to the user config
fn save_node_id(node_id: &str) -> std::io::Result<()> {
    //get the home directory
    let home_path = match get_home_directory() {
        Ok(path) => path,
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to determine home directory",
            ))
        }
    };

    let nexus_dir = home_path.join(".nexus");
    let config_path = nexus_dir.join("config.json");

    // Print how to find the config file
    println!("Loading configuration: {}", config_path.to_string_lossy());
    
    // Create the config object
    let config = NodeConfig {
        node_id: node_id.to_string(),
    };
    
    // Write the config to file
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(&config_path, json)?;

    // 2. If the .nexus directory exists, we need to read the config file
    match read_existing_node_id(&config_path) {
        // 2.1 Happy path - we successfully read the config file
        Ok(id) => {
            println!(
                "Successfully read existing node-id '{}' from file: {}",
                id,
                config_path.to_string_lossy()
            );
            Ok(())
        }
        // 2.2 We couldn't read the config file, so we may need to create a new one
        Err(e) => {
            eprintln!(
                "{}: {}",
                "Warning: Could not read node-id file".to_string().yellow(),
                e
            );
            handle_read_error(e, &config_path, node_id);
            Ok(())
        }
    }
}

pub async fn run_initial_setup() -> SetupResult {
    // Get home directory and check for prover-id file
    let home_path: std::path::PathBuf =
        home::home_dir().expect("Failed to determine home directory");

    //If the .nexus directory doesn't exist, we need to create it
    let nexus_dir = home_path.join(".nexus");
    if !nexus_dir.exists() {
        create_nexus_directory(&nexus_dir).expect("Failed to create .nexus directory");
    }

    //Check if the node-id file exists, use it. If not, create a new one.
    let node_config_path = home_path.join(".nexus").join("config.json");
    let node_id = match fs::read_to_string(&node_config_path) {
        Ok(content) => {
            match serde_json::from_str::<NodeConfig>(&content) {
                Ok(config) => config.node_id,
                Err(_) => String::new(),
            }
        }
        Err(_) => String::new(),
    };

    if node_config_path.exists() && !node_id.is_empty() {
        println!(
            "\nThis node is already connected to an account using node id: {}",
            node_id
        );

        //ask the user if they want to use the existing config
        println!("Do you want to use the existing user account? [Y/n]");
        let mut use_existing_config = String::new();
        std::io::stdin()
            .read_line(&mut use_existing_config)
            .unwrap();
        let use_existing_config = use_existing_config.trim();
        if use_existing_config != "n" {
            return SetupResult::Connected(node_id);
        } else {
            println!("Ignoring existing node id...");
        }
    }

    println!("\nThis node is not connected to any account.\n");
    println!("[1] Enter '1' Anonymous mode: start proving without earning Devnet points");
    println!("[2] Enter '2' Authenticated mode: start proving and earning Devnet points");

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
            println!("You chose to start earning Devnet points by connecting your node ID\n");
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

pub fn clear_node_id() -> std::io::Result<()> {
    let home_path: std::path::PathBuf =
        home::home_dir().expect("Failed to determine home directory");

    //If the .nexus directory doesn't exist, nothing to clear
    let nexus_dir = home_path.join(".nexus");
    if !nexus_dir.exists() {
        // nothing to clear
        return Ok(());
    }

    // if the nexus directory exists, check if the node-id file exists
    let node_config_path = home_path.join(".nexus").join("config.json");
    if !node_config_path.exists() {
        // nothing to clear
        return Ok(());
    }

    //if the node-id file exists, clear it
    match fs::remove_file(&node_config_path) {
        Ok(_) => {
            println!(
                "Successfully cleared node ID configuration with file: {}",
                node_config_path.to_string_lossy()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "{}",
                format!("Failed to clear node ID configuration: {}", e).red()
            );
            Err(e)
        }
    }
}

fn get_node_id_from_user() -> String {
    println!("{}", "Please enter your node ID:".green());
    let mut node_id = String::new();
    std::io::stdin()
        .read_line(&mut node_id)
        .expect("Failed to read node ID");
    node_id.trim().to_string()
}
