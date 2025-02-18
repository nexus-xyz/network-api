use colored::Colorize;
// use rand::RngCore;
// use random_word::Lang;
use std::{fs, path::Path, path::PathBuf};

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

pub fn read_existing_node_id(node_id_path: &Path) -> Result<String, std::io::Error> {
    let buf = fs::read(node_id_path)?;
    let id = String::from_utf8(buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        .trim()
        .to_string();

    if id.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Node ID file is empty",
        ));
    }

    Ok(id)
}

fn save_node_id(path: &Path, id: &str) {
    if let Err(e) = fs::write(path, id) {
        println!("Failed to save node-id to file: {}", e);
    } else {
        println!("Successfully saved new node-id to file: {}", id);
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
