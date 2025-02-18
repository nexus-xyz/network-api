use colored::Colorize;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;

// Update the import path to use the proto module
// use crate::config;
// use crate::orchestrator_client::OrchestratorClient;

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

// fn save_user_config(user_id: &str, node_id: &str) -> std::io::Result<()> {
//     let proj_dirs =
//         ProjectDirs::from("xyz", "nexus", "cli").expect("Failed to determine config directory");

//     let config_dir = proj_dirs.config_dir();
//     fs::create_dir_all(config_dir)?;

//     let config_path = config_dir.join("user.json");
//     let config = UserConfig {
//         user_id: Some(user_id.to_string()),
//         node_id: node_id.to_string(),
//     };

//     fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

//     //print the user config was saved properly
//     println!("User ID: {}", user_id);
//     println!("Node ID: {}", node_id);
//     println!("User config saved to: {}", config_path.to_string_lossy());

//     Ok(())
// }

//function that takes a node_id and saves it to the user config
fn save_node_id(node_id: &str) -> std::io::Result<()> {
    let proj_dirs =
        ProjectDirs::from("xyz", "nexus", "cli").expect("Failed to determine config directory");
    let config_path = proj_dirs.config_dir().join("user.json");
    let config = UserConfig {
        node_id: node_id.to_string(),
        user_id: None,
    };

    fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    Ok(())
}

pub async fn run_initial_setup() -> SetupResult {
    //check if there is a user config file
    let proj_dirs =
        ProjectDirs::from("xyz", "nexus", "cli").expect("Failed to determine config directory");
    let config_path = proj_dirs.config_dir().join("user.json");
    if config_path.exists() {
        println!("\nThis node is already connected to an account");

        //ask the user if they want to use the existing config
        println!("Do you want to use the existing user account? (y/n)");
        let mut use_existing_config = String::new();
        std::io::stdin()
            .read_line(&mut use_existing_config)
            .unwrap();
        let use_existing_config = use_existing_config.trim();
        if use_existing_config == "y" {
            match fs::read_to_string(&config_path) {
                Ok(content) => match serde_json::from_str::<UserConfig>(&content) {
                    Ok(user_config) => {
                        println!("\nUsing existing node ID: {}", user_config.node_id);
                        return SetupResult::Connected(user_config.node_id);
                    }
                    Err(e) => {
                        println!("{}", format!("Failed to parse config file: {}", e).red());
                        return SetupResult::Invalid;
                    }
                },
                Err(e) => {
                    println!("{}", format!("Failed to read config file: {}", e).red());
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
    let proj_dirs =
        ProjectDirs::from("xyz", "nexus", "cli").expect("Failed to determine config directory");
    let config_path = proj_dirs.config_dir().join("user.json");
    if config_path.exists() {
        fs::remove_file(config_path)?;
    }
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
