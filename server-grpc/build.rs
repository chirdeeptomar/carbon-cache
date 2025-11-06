use std::error::Error;
use std::{env, path::PathBuf};
use tonic_prost_build::{compile_protos, configure};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    configure()
        .protoc_arg("--experimental_allow_proto3_optional")
        .file_descriptor_set_path(out_dir.join("service_descriptor.bin"))
        .compile_protos(&["proto/service.proto"], &["proto"])?;

    compile_protos("proto/service.proto")?;
    Ok(())
}
