use std::{error::Error, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir: PathBuf = "./src/generated/".into();
    let proto_file: PathBuf = "../../proto/orchestrator.proto".into();
    let proto_dir = match proto_file.parent().ok_or("Failed to get parent directory of proto file") {
        Ok(dir) => dir,
        Err(e) => return Err(e.into()),
    };

    match Command::new("protoc --version").spawn() {
        Ok(_) => prost_build::Config::new()
            .out_dir(out_dir)
            .protoc_arg("--experimental_allow_proto3_optional")
            .compile_protos(&[&proto_file], &[proto_dir])?,
        Err(_) => {
            // Skipping protobuf compilation.
        }
    }

    Ok(())
}
