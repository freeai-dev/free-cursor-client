use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

pub(crate) fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("dev", "freeai", "free-cursor-client")
        .ok_or_else(|| anyhow::anyhow!("Failed to get project directories"))
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub token: Option<String>,
}

/// Migrate config from old location to new standard location
fn migrate_old_config() -> Result<()> {
    // Try to get old config path from APPDATA/HOME
    let old_path = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .map(|dir| Path::new(&dir).join("free-cursor-client"));

    if let Ok(old_path) = old_path {
        let old_config = old_path.join("config.json");
        if old_config.exists() {
            // Get new config path
            let new_config_dir = get_config_dir()?;
            let new_config = new_config_dir.join("config.json");

            // Only migrate if new config doesn't exist
            if !new_config.exists() {
                info!("Migrating config from old location to new location");
                std::fs::create_dir_all(new_config_dir)?;
                std::fs::copy(&old_config, &new_config)?;
            }
        }
    }
    Ok(())
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        // Try to migrate old config first
        let _ = migrate_old_config();

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
    let project_dirs = get_project_dirs()?;
    Ok(project_dirs.config_dir().to_path_buf())
}

pub fn get_program_path() -> Result<PathBuf> {
    let project_dirs = get_project_dirs()?;
    Ok(project_dirs
        .data_local_dir()
        .join(env!("CARGO_PKG_VERSION"))
        .join("free-cursor-client.exe"))
}
