use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub token: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_dir = get_config_dir()?;
        let config_path = Path::new(&config_dir).join("config.json");
        let config = std::fs::read_to_string(config_path)?;
        Ok(serde_json::from_str(&config)?)
    }

    pub fn load_or_default() -> Self {
        match Self::load() {
            Ok(config) => config,
            Err(_) => {
                info!("No config found, using default");
                Self::default()
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = get_config_dir()?;
        std::fs::create_dir_all(&config_dir)?;
        let config_path = Path::new(&config_dir).join("config.json");
        std::fs::write(config_path, serde_json::to_string(self)?)?;
        Ok(())
    }
}

pub fn get_config_dir() -> Result<PathBuf> {
    let app_data_dir = std::env::var("APPDATA").or_else(|_| std::env::var("HOME"))?;
    let config_dir = Path::new(&app_data_dir).join("free-cursor-client");
    Ok(config_dir)
}

pub fn get_program_home() -> Result<PathBuf> {
    let app_data_dir = std::env::var("APPDATA").or_else(|_| std::env::var("HOME"))?;
    let app_home = Path::new(&app_data_dir).join("free-cursor-client");
    Ok(app_home)
}
