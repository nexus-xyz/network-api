use colored::Colorize;
use log::{info, error, warn};
use serde::{Deserialize, Serialize};
use std::{fs, io};

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
pub struct UserConfig {
    pub node_id: String,
    pub user_id: Option<String>,
}

//function that takes a node_id and saves it to the user config
fn save_node_id(node_id: &str) -> std::io::Result<()> {
    info!("Preparing is save node_id: {} ",node_id);
    //get the home directory
    let home_path = match get_home_directory() {
        Ok(path) => {
            info!("Home directory determined: {}",path.to_string_lossy());
            path
        },
        Err(_) => {
            error!("Failed to determine home directory");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No home dir"))
        }
    };

    let nexus_dir = home_path.join(".nexus");
    if nexus_dir.exists(){
        info!("Creating .nexus directory at {}",nexus_dir.to_string_lossy());
        create_nexus_directory(&nexus_dir)?;
    }

    let node_id_path = nexus_dir.join("node-id");
    //print how to find the node-id file
    info!("Will write node ID to file: {}", node_id_path.to_string_lossy());
    //write the node_id to the node-id file
    fs::write(&node_id_path, node_id).unwrap();

    // 2. If the .nexus directory exists, we need to read the node-id file
    match read_existing_node_id(&node_id_path) {
        // 2.1 Happy path - we successfully read the node-id file
        Ok(id) => {
            println!(
                "Successfully read existing node-id '{}' from file: {}",
                id,
                node_id_path.to_string_lossy()
            );
            Ok(())
        }
        // 2.2 We couldn't read the node-id file, so we may need to create a new one
        Err(e) => {
            eprintln!(
                "{}: {}",
                "Warning: Could not read node-id file".to_string().yellow(),
                e
            );
            handle_read_error(e, &node_id_path, node_id);
            Ok(())
        }
    }
}

pub async fn run_initial_setup() -> SetupResult {
    // Get home directory and check for prover-id file
    let home_path: std::path::PathBuf = match home::home_dir(){
        Some(path) =>{
            info!("Home directory: {}", path.to_string_lossy());
            path
        }
        None => {
            error!("Failed to dertermine home directory (None returned)");
            return SetupResult::Invalid;
        }
    };

    //If the .nexus directory doesn't exist, we need to create it
    let nexus_dir = home_path.join(".nexus");
    if !nexus_dir.exists() {
        info!("Creating .nexus directory at: {}", nexus_dir.to_string_lossy());
        if let Err(err) = create_nexus_directory(&nexus_dir){
            error!("Failed to create .nexus directory: {}", err);
            return SetupResult::Invalid;
        }
    }

    //Check if the node-id file exists, use it. If not, create a new one.
    let node_id_path = home_path.join("node-id");
    let node_id = fs::read_to_string(&node_id_path).unwrap_or_default();

    if node_id_path.exists() {
        println!(
            "\nThis node is already connected to an account using node id: {}",
            node_id
        );

        //ask the user if they want to use the existing config
        println!("Do you want to use the existing user account? (y/n)");
        let mut use_existing_config = String::new();
        if let Err(e) = std::io::stdin().read_line(&mut use_existing_config){
            error!("Failed to read user input: {}",e);
            return SetupResult::Invalid;
        }

        let use_existing_config = use_existing_config.trim();
        if use_existing_config == "y" {
            return match fs::read_to_string(&node_id_path) {
                Ok(content) => {
                    info!("Using existing node-id from file: {}", content.trim());
                    SetupResult::Connected(content.trim().to_string())
                }
                Err(e) => {
                    error!("Failed to read node-id file: {}", e);
                    return SetupResult::Invalid;
                }
            };
        } else {
            println!("Ignoring existing user account...");
            info!("User opted to ignore existing node-id. Will prompt for new setup.");
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
            info!("User selected anonymous mode (no node-id).");
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
            info!("User Provided node-id: {}",node_id);

            match save_node_id(&node_id) {
                Ok(_) => {
                    info!("Node ID saved successfully. Setup complete");
                    SetupResult::Connected(node_id)
                },
                Err(e) => {
                    error!("Failed to save node ID: {}", e);
                    SetupResult::Invalid
                }
            }
        }
        _ => {
            warn!("Invalid option selected: {}",option);
            SetupResult::Invalid
        }
    }
}

pub fn clear_node_id() -> std::io::Result<()> {
    let home_path: std::path::PathBuf = home::home_dir().ok_or_else(||{
        io::Error::new(io::ErrorKind::Other, "Failed to determined home directory")
    })?;

    //If the .nexus directory doesn't exist, nothing to clear
    let nexus_dir = home_path.join(".nexus");
    if !nexus_dir.exists() {
        info!("No .nexus directory found; nothing to clear.");
        return Ok(());
    }

    // if the nexus directory exists, check if the node-id file exists
    let node_id_path = home_path.join(".nexus").join("node-id");
    if !node_id_path.exists() {
        info!("No node-id file found; nothing to clear.");
        return Ok(());
    }

    //if the node-id file exists, clear it
    info!("Removing node-id file at {}", node_id_path.to_string_lossy());
    match fs::remove_file(&node_id_path) {
        Ok(_) => {
            println!(
                "Successfully cleared node ID configuration with file: {}",
                node_id_path.to_string_lossy()
            );
            Ok(())
        }
        Err(e) => {
            error!("Failed to remove node-id file: {}", e);
            Err(e)
        }
    }
}

fn get_node_id_from_user() -> String {
    println!("{}", "Please enter your node ID:".green());
    let mut node_id = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut node_id){
        error!("Failed to read node ID from stdin: {}", e);
        // fallback to empty
        return "".to_string();
    }
    node_id.trim().to_string()
}
