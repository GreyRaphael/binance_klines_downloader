use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Re-run if config.toml changes
    println!("cargo:rerun-if-changed=config.toml");

    let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap(); // "debug" or "release"
    let dest = PathBuf::from(&out_dir)
        .join("target")
        .join(&profile)
        .join("config.toml");

    fs::copy("config.toml", &dest).unwrap_or_else(|e| {
        panic!("Failed to copy config.toml to {}: {}", dest.display(), e);
    });
}
