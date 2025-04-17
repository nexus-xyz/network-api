use colored::Colorize;
use serde::{Deserialize, Serialize};
// use rand::RngCore;
// use random_word::Lang;
use std::{fs, path::Path, path::PathBuf};

#[derive(Serialize, Deserialize)]
struct NodeConfig {
    node_id: String,
}

pub fn get_home_directory() -> Result<PathBuf, &'static str> {
    match home::home_dir() {
        Some(path) if !path.as_os_str().is_empty() => Ok(path),
        _ => {
            println!("Could not determine home directory");
            Err("No home directory found")
        }
    }
}

pub fn create_nexus_directory(nexus_dir: &Path) -> std::io::Result<()> {
    println!("Attempting to create .nexus directory");
    if let Err(e) = fs::create_dir(nexus_dir) {
        eprintln!(
            "{}: {}",
            "Warning: Failed to create .nexus directory"
                .to_string()
                .yellow(),
            e
        );
        return Err(e);
    }

    Ok(())
}

pub fn read_existing_node_id(config_path: &Path) -> Result<String, std::io::Error> {
    let buf = fs::read(config_path)?;
    let config: NodeConfig = serde_json::from_slice(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    if config.node_id.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Node ID is empty",
        ));
    }

    Ok(config.node_id)
}

fn save_node_id(path: &Path, id: &str) {
    let config = NodeConfig {
        node_id: id.to_string(),
    };
    
    match serde_json::to_string_pretty(&config) {
        Ok(json) => {
            if let Err(e) = fs::write(path, json) {
                println!("Failed to save node-id to file: {}", e);
            } else {
                println!("Successfully saved new node-id to file: {}", id);
            }
        }
        Err(e) => {
            println!("Failed to serialize node-id: {}", e);
        }
    }
}

pub fn handle_read_error(e: std::io::Error, path: &Path, default_id: &str) {
    match e.kind() {
        std::io::ErrorKind::NotFound => {
            save_node_id(path, default_id);
        }
        std::io::ErrorKind::PermissionDenied => {
            eprintln!(
                "{}: {}",
                "Error: Permission denied when accessing node-id file"
                    .to_string()
                    .yellow(),
                e
            );
        }
        std::io::ErrorKind::InvalidData => {
            eprintln!(
                "{}: {}",
                "Error: node-id file is corrupted".to_string().yellow(),
                e
            );
        }
        _ => {
            eprintln!(
                "{}: {}",
                "Error: Unexpected IO error when reading node-id file"
                    .to_string()
                    .yellow(),
                e
            );
        }
    }
}
