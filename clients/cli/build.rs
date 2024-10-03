use std::{error::Error, path::PathBuf, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir: PathBuf = "./src/generated/".into();
    let proto_file: PathBuf = "../../proto/orchestrator.proto".into();
    let proto_dir = proto_file.parent().unwrap();

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
