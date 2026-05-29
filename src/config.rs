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
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        // Try exe directory first, then current directory
        let path = Self::config_path()?;
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: AppConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(config)
    }

    fn config_path() -> Result<PathBuf> {
        // 1) Try next to the executable
        if let Ok(exe) = std::env::current_exe() {
            let p = exe.parent().unwrap().join("config.toml");
            if p.exists() {
                return Ok(p);
            }
        }
        // 2) Fallback to current working directory
        let cwd = std::env::current_dir().context("Cannot determine current directory")?;
        let p = cwd.join("config.toml");
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!("config.toml not found next to executable or in current directory")
    }
}
