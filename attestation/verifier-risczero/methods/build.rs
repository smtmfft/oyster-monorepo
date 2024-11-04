use std::collections::HashMap;
use std::path::Path;

use risc0_build::{DockerOptions, GuestOptions};

fn main() {
    let mut options = HashMap::new();
    options.insert(
        "guest",
        GuestOptions {
            features: vec![],
            use_docker: Some(DockerOptions {
                root_dir: std::env::current_dir()
                    .unwrap()
                    .parent()
                    .map(Path::to_path_buf),
            }),
        },
    );
    risc0_build::embed_methods_with_options(options);
}
