use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir: PathBuf = "./src/generated/".into();
    let proto_file: PathBuf = "../../proto/orchestrator.proto".into();
    let proto_dir = proto_file.parent().unwrap();

    prost_build::Config::new()
        .out_dir(out_dir)
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(&[&proto_file], &[proto_dir])?;

    Ok(())
}
