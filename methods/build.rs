use std::collections::HashMap;
use std::path::PathBuf;

use risc0_build::{DockerOptions, GuestOptions};

fn main() {
    let mut options = HashMap::new();
    options.insert(
        "guest",
        GuestOptions {
            features: vec![],
            use_docker: Some(DockerOptions {
                root_dir: std::env::var_os("OUT_DIR/docker/").map(PathBuf::from),
            }),
        },
    );
    risc0_build::embed_methods_with_options(options);
}
