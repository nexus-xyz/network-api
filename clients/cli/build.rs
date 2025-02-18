use prost_build::Config;
use std::error::Error;
use std::fs;
use std::process::Command;
use std::{env, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    // Tell cargo to recompile if any of these files change
    println!("cargo:rerun-if-changed=proto/orchestrator.proto");
    println!("cargo:rerun-if-changed=build.rs");

    let mut config = Config::new();

    // Print current directory
    println!("Current dir: {:?}", env::current_dir()?);

    // Check if proto file exists
    let proto_path = Path::new("proto/orchestrator.proto");
    println!(
        "Looking for proto file at: {:?}",
        proto_path.canonicalize()?
    );

    if !proto_path.exists() {
        println!("Proto file not found at: {:?}", proto_path);
        return Err("Proto file not found".into());
    }

    let out_dir = "src/proto";
    config.out_dir(out_dir);
    // .file_descriptor_set_path("src/proto/orchestrator.rs");

    // Check if protoc is installed and accessible
    let output = Command::new("which")
        .arg("protoc")
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        println!("protoc is installed and accessible.");
    } else {
        println!("Error: protoc is not installed or not in PATH.");
        return Err("protoc not found".into());
    }

    // Check if the output directory exists and is writable
    if fs::metadata(out_dir).is_ok() {
        println!("Output directory {} exists.", out_dir);
    } else {
        println!("Error: Output directory {} does not exist.", out_dir);
        // Attempt to create the directory if it doesn't exist
        fs::create_dir_all(out_dir)?;
        println!("Created output directory {}.", out_dir);
    }

    // Attempt to compile the .proto file
    match config.compile_protos(&["proto/orchestrator.proto"], &["proto"]) {
        Ok(_) => {
            println!("Successfully compiled protobuf files.");
        }
        Err(e) => {
            println!("Error compiling protobuf files: {}", e);
            // Log more details about the error
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    println!("Error: Could not find a necessary file or directory.");
                }
                _ => println!("Error: {}", e),
            }
            return Err(Box::new(e));
        }
    }

    // Print where the generated file is saved
    let generated_file_path = format!("{}/nexus_orchestrator.rs", out_dir);
    println!("Generated file saved to: {}", generated_file_path);

    // Check if the generated file exists
    if fs::metadata(&generated_file_path).is_ok() {
        println!("Generated file exists.");
    } else {
        println!("Error: Generated file does not exist.");
    }

    Ok(())
}
