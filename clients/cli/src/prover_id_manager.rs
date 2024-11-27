use colored::Colorize;
use rand::RngCore;
use random_word::Lang;
use std::{fs, path::Path};

/// Gets an existing prover ID from the filesystem or generates a new one
pub fn get_or_generate_prover_id() -> String {
    // If the prover_id file is found, use the contents, otherwise generate a new random id
    // and store it. e.g., "happy-cloud-42"
    let default_prover_id: String = format!(
        "{}-{}-{}",
        random_word::gen(Lang::En),
        random_word::gen(Lang::En),
        rand::thread_rng().next_u32() % 100,
    );

    // setting the prover-id we will use (either from the file or generated)
    let prover_id: String = match home::home_dir() {
        Some(path) if !path.as_os_str().is_empty() => {
            let nexus_dir = Path::new(&path).join(".nexus");

            // Try to read the prover-id file
            match fs::read(nexus_dir.join("prover-id")) {
                // 1. If file exists and can be read:
                Ok(buf) => match String::from_utf8(buf) {
                    Ok(id) => id.trim().to_string(), // Trim whitespace
                    Err(e) => {
                        eprintln!("Failed to read prover-id file. Using default: {}", e);
                        default_prover_id // Fall back to generated ID, if file has invalid UTF-8
                    }
                },
                // 2. If file doesn't exist or can't be read:
                Err(e) => {
                    eprintln!(
                        "{}: {}",
                        "Warning: Could not read prover-id file"
                            .to_string()
                            .yellow(),
                        e
                    );

                    // if the error is because the file doesn't exist
                    // Try to save the generated prover-id to the file
                    if e.kind() == std::io::ErrorKind::NotFound {
                        // Try to create the .nexus directory
                        match fs::create_dir(nexus_dir.clone()) {
                            Ok(_) => {
                                // Only try to write file if directory was created successfully
                                if let Err(e) =
                                    fs::write(nexus_dir.join("prover-id"), &default_prover_id)
                                {
                                    eprintln!("Warning: Could not save prover-id: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "{}: {}",
                                    "Warning: Failed to create .nexus directory"
                                        .to_string()
                                        .yellow(),
                                    e
                                );
                            }
                        }
                    }

                    // Use the previously generated prover-id
                    default_prover_id
                }
            }
        }
        _ => {
            println!("Unable to determine home directory. Using temporary prover-id.");
            default_prover_id
        }
    };

    prover_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir; // This is needed to run tests serially, to remove flakiness

    /// Tests the behavior for a first-time user with no existing configuration.
    /// This simulates a new user scenario where:
    /// 1. No .nexus directory exists yet
    /// 2. No prover-id file exists yet
    /// 3. The program needs to create both directory and file
    #[test]
    #[serial]
    fn test_new_user() {
        let temp_path = {
            let temp_dir = TempDir::new().unwrap();
            let path = temp_dir.path().to_path_buf();
            println!("Directory at start: {:?}", path);
            assert!(path.exists(), "Directory should exist during test");

            // Setup - create temporary home directory
            let original_home = env::var("HOME").ok();
            env::set_var("HOME", temp_dir.path());
            std::thread::sleep(std::time::Duration::from_millis(10)); // Give env time to update

            // Verify .nexus directory doesn't exist yet
            let nexus_dir = temp_dir.path().join(".nexus");
            assert!(!nexus_dir.exists(), "Nexus directory should not exist yet");

            // Get prover ID - should create directory and file
            let id1 = get_or_generate_prover_id();
            println!("Generated ID: {}", id1);

            // Verify ID format (word-word-number)
            let parts: Vec<&str> = id1.split('-').collect();
            assert_eq!(parts.len(), 3, "ID should be in format word-word-number");
            assert!(
                parts[2].parse::<u32>().is_ok(),
                "Last part should be a number"
            );

            // Verify directory and file were created
            assert!(
                nexus_dir.exists(),
                "Nexus directory should have been created"
            );
            let id_path = nexus_dir.join("prover-id");
            assert!(id_path.exists(), "Prover ID file should have been created");

            // Verify saved ID matches what we got
            let saved_id =
                fs::read_to_string(&id_path).expect("Should be able to read prover-id file");
            assert_eq!(saved_id, id1, "Saved ID should match generated ID");

            // Get ID again - should return same ID
            let id2 = get_or_generate_prover_id();
            assert_eq!(id2, id1, "Second call should return same ID");

            // Cleanup
            match original_home {
                None => env::remove_var("HOME"),
                Some(home) => env::set_var("HOME", home),
            }
            std::thread::sleep(std::time::Duration::from_millis(10)); // Give env time to update

            path // Return the path for checking later
        }; // temp_dir is dropped here, cleaning up

        // Verify cleanup happened
        println!("Directory after test: {:?}", temp_path);
        assert!(!temp_path.exists(), "Directory should be cleaned up");
    }

    /// Tests that the function can properly read an existing prover ID configuration.
    /// This simulates a scenario where:
    /// 1. User already has a .nexus directory
    /// 2. User already has a prover-id file with valid content
    /// 3. Function should read and use the existing ID without modification
    #[test]
    #[serial]
    fn test_read_existing_prover_id() {
        // Setup - create temporary home directory
        let original_home = env::var("HOME").ok();
        let temp_dir = TempDir::new().unwrap();
        println!("Created temp dir: {:?}", temp_dir.path());
        env::set_var("HOME", temp_dir.path());
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create pre-existing configuration
        let nexus_dir = temp_dir.path().join(".nexus");
        fs::create_dir(&nexus_dir).expect("Failed to create .nexus directory");

        let pre_existing_id = "happy-prover-42";
        let id_path = nexus_dir.join("prover-id");
        fs::write(&id_path, pre_existing_id).expect("Failed to create prover-id file");

        // Verify our setup worked
        assert!(nexus_dir.exists(), "Setup: .nexus directory should exist");
        assert!(id_path.exists(), "Setup: prover-id file should exist");
        assert_eq!(
            fs::read_to_string(&id_path).expect("Setup: should be able to read file"),
            pre_existing_id,
            "Setup: file should contain our ID"
        );

        // Test that the function reads the existing ID
        let read_id = get_or_generate_prover_id();
        assert_eq!(
            read_id, pre_existing_id,
            "Should return the pre-existing ID"
        );

        // Verify nothing was modified
        assert!(nexus_dir.exists(), "Directory should still exist");
        assert!(id_path.exists(), "File should still exist");
        assert_eq!(
            fs::read_to_string(&id_path).expect("Should still be able to read file"),
            pre_existing_id,
            "File content should be unchanged"
        );

        // Cleanup
        match original_home {
            None => env::remove_var("HOME"),
            Some(home) => env::set_var("HOME", home),
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
