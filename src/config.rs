use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub proxy: String,
    pub output_dir: String,
    pub symbols: Vec<String>,
    pub intervals: Vec<String>,
    pub gotify_url: String,
    pub gotify_token: String,
    pub ntfy_url: String,
    pub ntfy_token: String,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: AppConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }

    fn locate(name: &str) -> Option<PathBuf> {
        if let Ok(exe) = std::env::current_exe() {
            let p = exe.parent().unwrap().join(name);
            if p.exists() {
                return Some(p);
            }
        }
        let cwd = std::env::current_dir().ok()?;
        let p = cwd.join(name);
        if p.exists() {
            return Some(p);
        }
        None
    }

    fn config_path() -> Result<PathBuf> {
        if let Some(p) = Self::locate("config.toml") {
            return Ok(p);
        }
        if let Some(p) = Self::locate("config.toml.example") {
            tracing::warn!("config.toml not found, falling back to config.toml.example — copy it to config.toml and fill in your settings");
            return Ok(p);
        }
        anyhow::bail!("config.toml (or config.toml.example) not found next to executable or in current directory")
    }
}
