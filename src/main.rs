use std::{path::PathBuf, time::Duration};

use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info};

#[derive(Debug, Parser)]
struct Cli {
    #[arg(short, long)]
    token: Option<String>,

    #[arg(short, long)]
    install: bool,

    #[arg(short, long)]
    uninstall: bool,
}

fn install_auto_start() -> anyhow::Result<()> {
    let path = std::env::current_exe()?;

    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            winreg::enums::KEY_ALL_ACCESS,
        )?
        .set_value("free-cursor-client", &path.as_os_str())?;

    info!("Installed auto start");

    Ok(())
}

fn uninstall_auto_start() -> anyhow::Result<()> {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            winreg::enums::KEY_ALL_ACCESS,
        )?
        .delete_value("free-cursor-client")?;

    info!("Uninstalled auto start");

    Ok(())
}

fn call_login_api(token: &str) -> anyhow::Result<LoginResponse> {
    let client = reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(60 * 3))
        .build()?;
    let response: LoginResponse = client
        .post("http://localhost:3000/api/v1/cursor/token")
        .json(&json!({
            "token": token
        }))
        .send()?
        .json()?;
    info!("Login response: {:?}", response);
    Ok(response)
}

fn save_configs(configs: Vec<Config>) -> anyhow::Result<()> {
    let user_config_dir = std::env::var("APPDATA").or_else(|_| std::env::var("HOME"))?;
    let db_path =
        std::path::Path::new(&user_config_dir).join("Cursor/User/globalStorage/state.vscdb");
    info!("Opening {}", db_path.display());

    let conn = rusqlite::Connection::open(&db_path)?;
    info!("Updating auth info in {}", db_path.display());

    let mut stmt = conn.prepare("UPDATE ItemTable SET value = ? WHERE key = ?")?;

    for config in configs {
        stmt.execute([&config.value, &config.key])?;
    }

    Ok(())
}

fn main_result() -> anyhow::Result<()> {
    let args = Cli::parse();

    info!("Starting free-cursor-client");

    let mut config = match AppConfig::load() {
        Ok(config) => config,
        Err(e) => {
            error!("Error loading config: {}", e);
            AppConfig::default()
        }
    };

    if args.install {
        install_auto_start()?;
    }

    if args.uninstall {
        uninstall_auto_start()?;
    }

    if let Some(token) = args.token.as_ref() {
        config.token = Some(token.to_string());
        config.save()?;
    }

    if let Some(token) = config.token.as_ref() {
        let response = call_login_api(token)?;
        save_configs(response.configs)?;
    }

    Ok(())
}

fn main() {
    tracing_subscriber::fmt().init();

    if let Err(e) = main_result() {
        error!("Error: {}", e);
    }
}

fn get_config_dir() -> anyhow::Result<PathBuf> {
    let app_data_dir = std::env::var("APPDATA").or_else(|_| std::env::var("HOME"))?;
    let config_dir = std::path::Path::new(&app_data_dir).join("free-cursor-client");
    Ok(config_dir)
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AppConfig {
    token: Option<String>,
}

impl AppConfig {
    fn load() -> anyhow::Result<Self> {
        let config_dir = get_config_dir()?;
        let config_path = std::path::Path::new(&config_dir).join("config.json");
        let config = std::fs::read_to_string(config_path)?;
        Ok(serde_json::from_str(&config)?)
    }

    fn save(&self) -> anyhow::Result<()> {
        let config_dir = get_config_dir()?;
        std::fs::create_dir_all(&config_dir)?;
        let config_path = std::path::Path::new(&config_dir).join("config.json");
        std::fs::write(config_path, serde_json::to_string(self)?)?;
        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub configs: Vec<Config>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub key: String,
    pub value: String,
}
