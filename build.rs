use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Re-run if config files change
    println!("cargo:rerun-if-changed=config.toml");
    println!("cargo:rerun-if-changed=config.toml.example");

    let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap(); // "debug" or "release"
    let dest = PathBuf::from(&out_dir)
        .join("target")
        .join(&profile)
        .join("config.toml");

    // Copy the first available config file to the output directory
    let src = if std::path::Path::new("config.toml").exists() {
        "config.toml"
    } else if std::path::Path::new("config.toml.example").exists() {
        println!("cargo:warning=config.toml not found, using config.toml.example (remember to create config.toml for your own settings)");
        "config.toml.example"
    } else {
        println!("cargo:warning=neither config.toml nor config.toml.example found");
        return;
    };

    if let Err(e) = fs::copy(src, &dest) {
        println!("cargo:warning=failed to copy {} to {}: {}", src, dest.display(), e);
    }
}
