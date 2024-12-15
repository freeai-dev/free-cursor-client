pub mod order;

use anyhow::{Context, Result};
use colored::Colorize;
use std::os::windows::process::CommandExt;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use sysinfo::ProcessRefreshKind;
use tracing::{error, info, warn};
use windows::{
    core::{HRESULT, HSTRING},
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, System::Threading::DETACHED_PROCESS},
};
use windows_registry::CURRENT_USER;

use crate::config::{get_legacy_program_path, get_program_path, get_project_dirs};
use crate::{
    api::{call_login_api, call_status_api},
    cli::{InstallArgs, InviteArgs, StatusArgs},
    config::AppConfig,
    logger,
    models::{LoginResponse, Token},
    telemetry::{self, TelemetryLogLevel},
};

pub async fn handle_install(args: InstallArgs) -> Result<()> {
    tracing_subscriber::fmt().init();

    let token = match args.token.or_else(|| AppConfig::load_or_default().token) {
        Some(token) => token,
        None => {
            error!("未提供 Token 且配置中未找到 Token");
            return Err(anyhow::anyhow!("未提供 Token 且配置中未找到 Token"));
        }
    };

    do_install(token).await
}

pub async fn do_install(token: String) -> Result<()> {
    let mut config = AppConfig::load_or_default();
    config.token = Some(token.clone());
    config.save()?;

    check_cursor_installed()?;

    wait_cursor_processes()?;

    let program = get_program_path()?;
    stop_service()?;

    install_program(&program)?;
    install_auto_start(&program)?;

    info!("启动服务");
    Command::new(program)
        .arg("service")
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()?;

    telemetry::report(
        TelemetryLogLevel::Info,
        None,
        format!("程序已安装，Token: {}", token),
    )
    .await;

    Ok(())
}

pub async fn handle_uninstall(_full: bool) -> Result<()> {
    tracing_subscriber::fmt().init();
    stop_service()?;
    uninstall_auto_start()?;
    Ok(())
}

pub async fn run_service() -> Result<()> {
    logger::init_file_logs()?;

    let config = AppConfig::load_or_default();
    let Some(token) = config.token.as_ref() else {
        return Err(anyhow::anyhow!("未找到 Token"));
    };

    loop {
        let response = call_login_api(token).await;
        match response {
            Ok(LoginResponse::Token(token)) => {
                match save_configs(token).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("保存配置失败：{}", e);
                        telemetry::report(
                            TelemetryLogLevel::Error,
                            None,
                            format!("保存配置失败：{}", e),
                        )
                        .await;
                    }
                }
                std::thread::sleep(Duration::from_secs(30 * 60));
            }
            Ok(LoginResponse::Pending(_)) => {
                info!("登录等待中，30 秒后重试");
                std::thread::sleep(Duration::from_secs(30));
            }
            Ok(LoginResponse::Expired(_)) => {
                info!("订阅已过期");
                telemetry::report(
                    TelemetryLogLevel::Info,
                    None,
                    format!("订阅已过期，Token: {}", token),
                )
                .await;
                save_configs(Token::default()).await?;
                break;
            }
            Ok(LoginResponse::Error(e)) => {
                error!("登录错误：{}", e);
                std::thread::sleep(Duration::from_secs(30 * 60));
            }
            Err(e) => {
                error!("登录错误：{}", e);
                std::thread::sleep(Duration::from_secs(30));
            }
        }
    }

    Ok(())
}

pub async fn handle_status(args: StatusArgs) -> Result<()> {
    tracing_subscriber::fmt().init();
    let config = AppConfig::load_or_default();
    let token = args.token.or(config.token);
    match token {
        Some(token) => {
            let response = call_status_api(&token).await?;
            if response.subscriptions.is_empty() {
                info!("您目前{}订阅", "没有".red().bold());
            } else {
                info!("您的订阅：");
                for subscription in response.subscriptions {
                    let status = match subscription.status {
                        crate::models::UserSubscriptionStatus::Active => "有效".green().bold(),
                        crate::models::UserSubscriptionStatus::Expired => "已过期".red().bold(),
                        crate::models::UserSubscriptionStatus::Cancelled => {
                            "已取消".yellow().bold()
                        }
                    };
                    info!("  状态：{}", status);
                    info!("    开始日期：{}", subscription.start_date.0);
                    info!("    结束日期：{}", subscription.end_date.0);
                }
            }
        }
        None => {
            info!("未找到 Token");
        }
    }
    Ok(())
}

pub async fn handle_invite(args: InviteArgs) -> Result<()> {
    tracing_subscriber::fmt().init();
    let client = reqwest::Client::new();

    let token = match args.token.or_else(|| AppConfig::load_or_default().token) {
        Some(token) => token,
        None => {
            error!("No token found");
            return Ok(());
        }
    };

    let response = client
        .post("https://auth-server.freeai.dev/api/v1/promotions")
        .json(&serde_json::json!({
            "token": token
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        error!("生成邀请码失败");
        return Ok(());
    }

    let promotion: serde_json::Value = response.json().await?;
    if let Some(code) = promotion.get("promotion").and_then(|p| p.get("code")) {
        info!(
            "您的邀请码是：{}",
            code.as_str().unwrap_or_default().green().bold()
        );
    }

    Ok(())
}

// Helper functions
fn check_cursor_installed() -> Result<()> {
    let cursor_dir = get_cursor_installed_dir()?;
    if !cursor_dir.exists() {
        error!("未安装 Cursor");
        return Err(anyhow::anyhow!("未安装 Cursor"));
    }
    Ok(())
}

fn get_cursor_installed_dir() -> Result<PathBuf> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Failed to get config dir"))?;
    let cursor_dir = config_dir.join("Cursor");
    Ok(cursor_dir)
}

async fn save_configs(token: Token) -> Result<()> {
    if let Some(machine_id) = token.machine_id {
        reset_machine_id(&machine_id)?;
    }

    let cursor_dir = get_cursor_installed_dir()?;
    let db_path = cursor_dir.join("User/globalStorage/state.vscdb");
    if !db_path.exists() {
        error!("Database file not found: {}", db_path.display());
        return Err(anyhow::anyhow!(
            "Database file not found: {}",
            db_path.display()
        ));
    }

    info!("Opening {}", db_path.display());
    let conn = rusqlite::Connection::open(&db_path)?;

    info!("Updating auth info in {}", db_path.display());
    let mut stmt = conn.prepare(
        "INSERT INTO ItemTable (key, value) VALUES (?, ?) 
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )?;

    let configs = [
        ("cursorAuth/accessToken", token.access_token.clone()),
        ("cursorAuth/refreshToken", token.refresh_token),
        ("cursorAuth/cachedEmail", token.email.clone()),
        ("cursorAuth/cachedSignUpType", "Auth_0".to_string()),
        ("cursorAuth/stripeMembershipType", "free_trial".to_string()),
    ];

    for (key, value) in configs {
        info!("Upserting {} with {}", key, value);
        stmt.execute([key, &value])?;
    }

    info!("Saved configs");
    telemetry::report(
        TelemetryLogLevel::Info,
        None,
        format!(
            "Saved access token: {}, email: {}",
            token.access_token, token.email
        ),
    )
    .await;

    Ok(())
}

fn reset_machine_id(machine_id: &str) -> Result<()> {
    let cursor_dir = get_cursor_installed_dir()?;
    let storage_path = cursor_dir.join(r"User\globalStorage\storage.json");
    let storage = std::fs::read_to_string(&storage_path)?;
    let mut storage: serde_json::Value = serde_json::from_str(&storage)?;

    if let Some(obj) = storage.get_mut("telemetry.macMachineId") {
        *obj = serde_json::Value::from(machine_id);
    }
    std::fs::write(storage_path, serde_json::to_string(&storage)?)?;

    info!("Reset machine ID: {}", machine_id);

    Ok(())
}

fn install_program(target: &Path) -> Result<()> {
    let program = std::env::current_exe()?;
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to get program parent"))?;
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&program, target)?;
    info!("Installed program to {}", target.display());
    Ok(())
}

fn install_auto_start(program: &Path) -> Result<()> {
    let mut command = quote_path(program.as_os_str());
    command.push(" service");
    info!(
        "Installing auto start with command: {}",
        command.to_string_lossy()
    );

    let key = CURRENT_USER
        .create("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .context("CreateRegKey")?;
    let value = HSTRING::from(command.as_os_str());
    key.set_hstring("free-cursor-client", &value)
        .context("SetRegValue")?;

    info!("Installed auto start");

    Ok(())
}

fn uninstall_auto_start() -> Result<()> {
    let key = match CURRENT_USER.create("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        Ok(key) => key,
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            info!("Registry key not found");
            return Ok(());
        }
        Err(e) => {
            return Err(anyhow::Error::from(e).context("RegOpenKey"));
        }
    };

    match key.remove_value("free-cursor-client") {
        Ok(_) => {}
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            return Ok(());
        }
        Err(e) => {
            return Err(anyhow::Error::from(e).context("RegDeleteValue"));
        }
    }

    info!("Uninstalled auto start");

    Ok(())
}

fn stop_service() -> Result<()> {
    info!("Stopping service");

    let self_pid = std::process::id();
    let project_dirs = get_project_dirs()?;
    let base = project_dirs.data_local_dir();
    let legacy_base = get_legacy_program_path();

    let mut sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let processes = sys.processes();
    for (pid, process) in processes {
        if pid.as_u32() != self_pid {
            if let Some(exe) = process.exe() {
                let match_legacy = if let Ok(legacy_base) = &legacy_base {
                    exe.starts_with(legacy_base)
                } else {
                    false
                };
                if exe.starts_with(base) || match_legacy {
                    info!("Stopping process: {}", pid.as_u32());
                    process.kill();
                }
            }
        }
    }

    info!("Stopped service");

    Ok(())
}

fn wait_cursor_processes() -> Result<()> {
    let mut tips = false;
    loop {
        let processes = match scan_cursor_processes() {
            Ok(processes) => processes,
            Err(e) => {
                warn!("Failed to scan cursor processes: {}", e);
                return Ok(());
            }
        };
        if processes.is_empty() {
            return Ok(());
        }
        if !tips {
            info!("Found running Cursor processes:");
            for p in processes {
                info!("  PID: {}", p);
            }
            info!("Please close all Cursor processes before continuing...");
            tips = true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn scan_cursor_processes() -> Result<Vec<u32>> {
    let mut sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let processes = sys.processes();
    let cursor_processes = processes
        .iter()
        .filter(|(_, process)| process.name().eq_ignore_ascii_case("Cursor.exe"))
        .map(|(pid, _)| pid.as_u32())
        .collect();
    Ok(cursor_processes)
}

fn quote_path(path: &OsStr) -> OsString {
    let bytes = path.as_encoded_bytes();
    if bytes.contains(&b' ') && !bytes.starts_with(&[b'"']) && !bytes.ends_with(&[b'"']) {
        let mut buf = Vec::with_capacity(bytes.len() + 2);
        buf.push(b'"');
        buf.extend_from_slice(bytes);
        buf.push(b'"');
        return unsafe { OsString::from_encoded_bytes_unchecked(buf) };
    }
    path.to_os_string()
}
