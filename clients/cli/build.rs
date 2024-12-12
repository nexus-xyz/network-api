use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let generated_file: PathBuf = "./src/generated/nexus.orchestrator.rs".into();

    // Verify the generated file exists
    if !generated_file.exists() {
        println!(
            "cargo:warning=Generated protobuf file not found at {}",
            generated_file.display()
        );
        return Err("Missing required generated protobuf file".into());
    }

    Ok(())
}
