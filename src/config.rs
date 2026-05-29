use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub proxy: String,
    pub output_dir: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = std::path::Path::new("config.toml");
        let content = fs::read_to_string(path).context("Failed to read config.toml")?;
        let config: AppConfig = toml::from_str(&content).context("Failed to parse config.toml")?;
        Ok(config)
    }
}
