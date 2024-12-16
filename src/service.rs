pub mod order;

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::os::windows::process::CommandExt;
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use sysinfo::ProcessRefreshKind;
use tokio::task::spawn_blocking;
use tracing::{error, info, warn};
use windows::{
    core::{HRESULT, HSTRING},
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, System::Threading::DETACHED_PROCESS},
};
use windows_registry::CURRENT_USER;

use crate::config::{get_program_path, get_program_path_with_version};
use crate::models::GeneralResponse;
use crate::{
    api::{call_login_api, call_status_api, check_update},
    cli::{InstallArgs, InviteArgs, StatusArgs},
    config::AppConfig,
    logger,
    models::{LoginResponse, Token},
};

pub async fn handle_install(args: InstallArgs) -> Result<()> {
    logger::init_console_logs()?;

    let token = match args.token.or_else(|| AppConfig::load_or_default().token) {
        Some(token) => token,
        None => {
            error!("未提供 Token 且配置中未找到 Token");
            return Err(anyhow::anyhow!("未提供 Token 且配置中未找到 Token"));
        }
    };

    do_self_install(token).await
}

pub async fn do_self_install(token: String) -> Result<()> {
    let src_program = std::env::current_exe()?;
    let dst_program = get_program_path()?;
    do_install(token, &src_program, &dst_program).await
}

pub async fn do_install(token: String, src_program: &Path, dst_program: &Path) -> Result<()> {
    let mut config = AppConfig::load_or_default();
    config.token = Some(token.clone());
    config.save()?;

    info!("正在检查 Cursor 是否已安装");
    check_cursor_installed()?;

    info!("正在等待 Cursor 进程结束");
    wait_cursor_processes(true)?;

    info!("正在停止已安装的服务");
    stop_service()?;

    info!("正在安装程序");
    install_program(&src_program, &dst_program).await?;

    info!("正在安装自启动");
    install_auto_start(&dst_program)?;

    info!("正在启动服务");
    Command::new(dst_program)
        .arg("service")
        .creation_flags(DETACHED_PROCESS.0)
        .spawn()?;

    info!("安装完成，Token: {}", token);

    Ok(())
}

pub async fn handle_uninstall(_full: bool) -> Result<()> {
    logger::init_console_logs()?;

    info!("正在停止服务");
    stop_service()?;

    info!("正在卸载自启动");
    uninstall_auto_start()?;

    info!("卸载完成");
    Ok(())
}

pub async fn run_service() -> Result<()> {
    logger::init_file_logs()?;

    const MAGIC_STR: &str = concat!(
        "__FREE_CURSOR_CLIENT_VERSION_",
        env!("CARGO_PKG_VERSION"),
        "__"
    );
    info!("{}", MAGIC_STR);

    let config = AppConfig::load_or_default();
    let Some(token) = config.token.as_ref() else {
        return Err(anyhow::anyhow!("未找到 Token"));
    };

    info!("正在检查更新，当前版本：{}", env!("CARGO_PKG_VERSION"));
    match check_update().await {
        Ok(GeneralResponse::Success(update)) => {
            match (update.force_update, update.latest_version) {
                (Some(true), Some(version)) => {
                    info!("发现强制更新版本：{}", version);
                    if let Some(desc) = update.description.as_deref() {
                        info!("更新说明：{}", desc);
                    }
                    info!("正在执行更新...");

                    if let Some(url) = update.download_url {
                        match download_and_install_update(&url, &version, token).await {
                            Ok(_) => {
                                info!("更新完成，退出当前服务");
                                return Ok(());
                            }
                            Err(e) => error!("更新失败：{}", e),
                        }
                    } else {
                        error!("更新失败：未找到下载地址");
                    };
                }
                (Some(true), None) => {
                    warn!("未找到最新版本");
                }
                (_, _) => {
                    info!("无需强制更新");
                }
            }
        }
        Ok(GeneralResponse::Error(e)) => {
            error!("检查更新失败：{}", e.error);
        }
        Err(e) => {
            warn!("检查更新失败：{:?}", e);
        }
    }

    loop {
        let _ = wait_cursor_processes_async(false).await;

        let response = call_login_api(token).await;
        let count = scan_cursor_processes().map(|v| v.len()).unwrap_or_default();
        if count > 0 {
            break;
        }
        match response {
            Ok(LoginResponse::Token(token)) => {
                match save_configs(token).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("保存配置失败：{}", e);
                    }
                }
                std::thread::sleep(Duration::from_secs(30 * 60));
            }
            Ok(LoginResponse::Pending(_)) => {
                info!("登录等待中，30 秒后重试");
                std::thread::sleep(Duration::from_secs(30));
            }
            Ok(LoginResponse::Expired(_)) => {
                error!("订阅已过期：Token: {}", token);
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
    logger::init_console_logs()?;
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
    logger::init_console_logs()?;
    let client = reqwest::Client::new();

    let token = match args.token.or_else(|| AppConfig::load_or_default().token) {
        Some(token) => token,
        None => {
            error!("未找到 Token");
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
    let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
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
        error!("数据库文件未找到：{}", db_path.display());
        return Err(anyhow::anyhow!("数据库文件未找到：{}", db_path.display()));
    }

    info!("正在打开 {}", db_path.display());
    let conn = rusqlite::Connection::open(&db_path)?;

    info!("正在更新 {} 中的认证信息", db_path.display());
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
        info!("正在更新 {} 值为 {}", key, value);
        stmt.execute([key, &value])?;
    }

    info!(
        "配置已保存：access token: {}, email: {}",
        token.access_token, token.email
    );

    Ok(())
}

fn reset_machine_id(machine_id: &str) -> Result<()> {
    let cursor_dir = get_cursor_installed_dir()?;
    let storage_path = cursor_dir.join(r"User\globalStorage\storage.json");
    let storage = std::fs::read_to_string(&storage_path)
        .map_err(|e| anyhow::anyhow!("读取 storage.json 失败: {:?}", e))?;
    let mut storage: serde_json::Value = serde_json::from_str(&storage)
        .map_err(|e| anyhow::anyhow!("解析 storage.json 失败: {:?}", e))?;

    if let Some(obj) = storage.get_mut("telemetry.macMachineId") {
        *obj = serde_json::Value::from(machine_id);
    }
    std::fs::write(storage_path, serde_json::to_string(&storage)?)
        .map_err(|e| anyhow::anyhow!("写入 storage.json 失败: {:?}", e))?;

    info!("已重置机器 ID：{}", machine_id);

    Ok(())
}

async fn install_program(src_program: &Path, target: &Path) -> Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to get program parent"))?;
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }
    info!("正在复制程序到 {}", target.display());
    let mut content = tokio::fs::read(src_program).await?;
    let e_lfanew = content
        .get(0x3c..0x3c + 2)
        .ok_or_else(|| anyhow::anyhow!("Failed to get e_lfanew"))?;
    let e_lfanew = u16::from_le_bytes(e_lfanew.try_into()?);
    let subsystem_offset = e_lfanew + 0x18 + 68;
    let subsystem = content
        .get_mut(subsystem_offset as usize)
        .ok_or_else(|| anyhow::anyhow!("Failed to get subsystem"))?;
    *subsystem = 2;
    tokio::fs::write(target, content).await?;
    info!("复制完成");
    Ok(())
}

fn install_auto_start(program: &Path) -> Result<()> {
    let mut command = quote_path(program.as_os_str());
    command.push(" service");
    info!("正在安装自启动，命令：{}", command.to_string_lossy());

    let key = CURRENT_USER
        .create("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .context("CreateRegKey")?;
    let value = HSTRING::from(command.as_os_str());
    key.set_hstring("free-cursor-client", &value)
        .context("SetRegValue")?;

    info!("已安装自启动");

    Ok(())
}

fn uninstall_auto_start() -> Result<()> {
    let key = match CURRENT_USER.create("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        Ok(key) => key,
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            info!("注册表键未找到");
            return Ok(());
        }
        Err(e) => {
            error!("打开注册表键失败：{:?}", e);
            bail!("打开注册表键失败：{:?}", e);
        }
    };

    match key.remove_value("free-cursor-client") {
        Ok(_) => {}
        Err(e) if e.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            return Ok(());
        }
        Err(e) => {
            error!("删除注册表值失败：{:?}", e);
            bail!("删除注册表值失败：{:?}", e);
        }
    }

    info!("已卸载自启动");
    Ok(())
}

fn stop_service() -> Result<()> {
    let self_pid = std::process::id();

    info!("正在扫描进程");
    let mut sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let processes = sys.processes();

    let mut terminated_processes = vec![];
    for (pid, process) in processes {
        if pid.as_u32() != self_pid {
            let name = process
                .name()
                .to_ascii_lowercase()
                .to_string_lossy()
                .into_owned();
            if name.starts_with("free-cursor-client") {
                info!("正在停止进程：{}", pid.as_u32());
                process.kill();
                terminated_processes.push(process);
            }
        }
    }

    if !terminated_processes.is_empty() {
        info!("正在等待已终止的进程");
        for process in terminated_processes {
            process.wait();
        }
    }

    info!("服务已停止");

    Ok(())
}

async fn wait_cursor_processes_async(interactive: bool) -> Result<()> {
    spawn_blocking(move || wait_cursor_processes(interactive)).await??;
    Ok(())
}

#[cfg(windows)]
fn wait_cursor_processes(interactive: bool) -> Result<()> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Threading::{WaitForMultipleObjects, INFINITE};

    let mut tips = false;
    loop {
        let processes = match scan_cursor_processes() {
            Ok(processes) => processes,
            Err(e) => {
                warn!("扫描 Cursor 进程失败: {:?}", e);
                return Ok(());
            }
        };

        if processes.is_empty() {
            return Ok(());
        }

        if interactive && !tips {
            info!("发现正在运行的 Cursor 进程：");
            for pid in &processes {
                info!("  进程 ID：{}", pid);
            }
            info!("请在继续之前关闭所有 Cursor 进程...");
            tips = true;
        }

        // Convert process IDs to handles
        let handles: Vec<HANDLE> = processes
            .iter()
            .filter_map(|&pid| unsafe {
                let handle = windows::Win32::System::Threading::OpenProcess(
                    windows::Win32::System::Threading::PROCESS_SYNCHRONIZE,
                    false,
                    pid,
                )
                .ok()?;
                Some(handle)
            })
            .collect();

        if handles.is_empty() {
            return Ok(());
        }

        // Wait for all processes to exit
        unsafe {
            WaitForMultipleObjects(
                &handles, true, // Wait for all processes
                INFINITE,
            )
        };

        // Clean up handles
        for handle in handles {
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
    }
}

#[cfg(not(windows))]
fn wait_cursor_processes(interactive: bool) -> Result<()> {
    let mut tips = false;
    loop {
        let processes = match scan_cursor_processes() {
            Ok(processes) => processes,
            Err(e) => {
                warn!("扫描 Cursor 进程失败: {:?}", e);
                return Ok(());
            }
        };
        if processes.is_empty() {
            return Ok(());
        }
        if !tips {
            info!("发现正在运行的 Cursor 进程：");
            for p in processes {
                info!("  进程 ID：{}", p);
            }
            info!("请在继续之前关闭所有 Cursor 进程...");
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

async fn download_and_install_update(url: &str, version: &str, token: &str) -> Result<()> {
    info!("正在下载新版本...");

    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("下载更新失败：HTTP {}", response.status()));
    }

    let program_path = tempfile::NamedTempFile::new()?;

    // Download to temporary file
    let content = response.bytes().await?;
    std::fs::write(&program_path.path(), content)?;

    info!("下载完成，正在安装...");
    do_install(
        token.to_string(),
        program_path.path(),
        &get_program_path_with_version(version)?,
    )
    .await
}
