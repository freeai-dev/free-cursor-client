#![windows_subsystem = "windows"]

mod telemetry;

use std::{
    ffi::{OsStr, OsString},
    fs::OpenOptions,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use colored::Colorize as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use sysinfo::ProcessRefreshKind;
use telemetry::{report, TelemetryLogLevel};
use time::{
    format_description::well_known::Rfc3339, macros::format_description, OffsetDateTime, UtcOffset,
};
use tracing::{error, info};
use tracing::{level_filters::LevelFilter, warn};
use tracing_subscriber::{
    fmt::time::LocalTime, layer::SubscriberExt as _, EnvFilter, Layer as _, Registry,
};
use windows::{
    core::{w, HRESULT, HSTRING, PCWSTR},
    Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, HANDLE,
        },
        System::{
            Console::{AllocConsole, AttachConsole, ATTACH_PARENT_PROCESS},
            Threading::{CreateMutexW, DETACHED_PROCESS},
        },
    },
};
use windows_registry::CURRENT_USER;

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    #[command(
        about = "Install the program\nThe program will be installed to %APPDATA%/free-cursor-client, and will be started automatically"
    )]
    Install(InstallArgs),

    #[command(
        about = "Uninstall the program\nDefault only deletes the program executable, use --full to also delete configs and logs"
    )]
    Uninstall {
        #[arg(
            long,
            default_value_t = false,
            help = "Delete all program data, including configs and logs"
        )]
        full: bool,
    },

    #[command(about = "Run the service\nDO NOT USE THIS COMMAND MANUALLY")]
    Service,

    #[command(about = "Get the status of your token")]
    Status(StatusArgs),
}

#[derive(Debug, Args)]
struct InstallArgs {
    #[arg(long, help = "The token to use")]
    token: String,
}

#[derive(Debug, Args)]
struct StatusArgs {
    #[arg(
        long,
        help = "The token to use. The cached token will be used if not provided"
    )]
    token: Option<String>,
}

fn quote_path(path: &OsStr) -> OsString {
    let bytes = path.as_encoded_bytes();
    // check if bytes contains any whitespace and not starts with double quote and not ends with double quote
    if bytes.contains(&b' ') && !bytes.starts_with(&[b'"']) && !bytes.ends_with(&[b'"']) {
        let mut buf = Vec::with_capacity(bytes.len() + 2);
        buf.push(b'"');
        buf.extend_from_slice(bytes);
        buf.push(b'"');
        return unsafe { OsString::from_encoded_bytes_unchecked(buf) };
    }
    path.to_os_string()
}

fn get_program_path() -> anyhow::Result<PathBuf> {
    Ok(get_program_home()?.join("free-cursor-client.exe"))
}

fn install_program(target: &Path) -> anyhow::Result<()> {
    let program = std::env::current_exe()?;
    std::fs::copy(&program, target)?;
    info!("Installed program to {}", target.display());
    Ok(())
}

fn install_auto_start(program: &Path) -> anyhow::Result<()> {
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

fn uninstall_auto_start() -> anyhow::Result<()> {
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

fn stop_service(program: &Path) -> anyhow::Result<()> {
    info!("Stopping service");

    let self_pid = std::process::id();

    let mut sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let processes = sys.processes();
    for (pid, process) in processes {
        if process.exe() == Some(program) && pid.as_u32() != self_pid {
            info!("Stopping process: {}", pid.as_u32());
            process.kill();
        }
    }

    info!("Stopped service");

    Ok(())
}

async fn call_status_api(token: &str) -> anyhow::Result<StatusResponse> {
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(60 * 3))
        .build()?;
    let response: StatusResponse = client
        .get(format!(
            "https://auth-server.freeai.dev/api/v1/cursor/token/{token}"
        ))
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn call_login_api(token: &str) -> anyhow::Result<LoginResponse> {
    let machine_id = machine_uid::get().map_err(|_| anyhow::anyhow!("Failed to get machine id"))?;
    let client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(60 * 3))
        .build()?;
    let response: LoginResponse = client
        .post("https://auth-server.freeai.dev/api/v1/cursor/token")
        .json(&json!({
            "token": token,
            "machineId": machine_id
        }))
        .send()
        .await?
        .json()
        .await?;
    info!("Login response: {:?}", response);
    Ok(response)
}

async fn save_configs(token: Token) -> anyhow::Result<()> {
    reset_machine_id(&token.machine_id)?;

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
    report(
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

fn get_cursor_installed_dir() -> anyhow::Result<PathBuf> {
    let user_config_dir = std::env::var("APPDATA").or_else(|_| std::env::var("HOME"))?;
    let cursor_dir = std::path::Path::new(&user_config_dir).join("Cursor");
    Ok(cursor_dir)
}

fn check_cursor_installed() -> anyhow::Result<()> {
    let cursor_dir = get_cursor_installed_dir()?;
    if !cursor_dir.exists() {
        error!("Cursor is not installed");
        return Err(anyhow::anyhow!("Cursor is not installed"));
    }
    Ok(())
}

// ref: https://github.com/bestK/cursor-fake-machine/blob/4df22912b8e6774faa1828d8d530c04e3fe0a79a/extension.js#L36
fn reset_machine_id(machine_id: &str) -> anyhow::Result<()> {
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

static CONSOLE_ATTACHED: std::sync::Once = std::sync::Once::new();
fn attach_console() -> anyhow::Result<()> {
    unsafe {
        CONSOLE_ATTACHED.call_once(|| {
            if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
                let _ = AllocConsole();
            }
        });
    }
    Ok(())
}

fn scan_cursor_processes() -> anyhow::Result<Vec<u32>> {
    let mut sys = sysinfo::System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let processes = sys.processes();
    let cursor_processes = processes
        .iter()
        .filter(|(_, process)| {
            process
                .exe()
                .map(|exe| exe.to_string_lossy().eq_ignore_ascii_case("Cursor.exe"))
                .unwrap_or(false)
        })
        .map(|(pid, _)| pid.as_u32())
        .collect();
    Ok(cursor_processes)
}

fn wait_cursor_processes() -> anyhow::Result<()> {
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
            info!("Waiting for Cursor to stop...");
            for p in processes {
                info!("  PID: {}", p);
            }
            tips = true;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

async fn main_result() -> anyhow::Result<()> {
    let args = match Cli::try_parse() {
        Ok(args) => args,
        Err(e) => {
            attach_console()?;
            e.exit();
        }
    };

    if !matches!(args.command, CliCommand::Service) {
        attach_console()?;
    }

    match args.command {
        CliCommand::Install(args) => {
            tracing_subscriber::fmt().init();

            let mut config = AppConfig::load_or_default();
            config.token = Some(args.token.clone());
            config.save()?;

            check_cursor_installed()?;

            let response = call_login_api(&args.token).await?;
            match response {
                LoginResponse::Token(token) => {
                    wait_cursor_processes()?;
                    save_configs(token).await?;
                }
                LoginResponse::Expired(_) => {
                    info!("Subscription expired, please renew your subscription");
                    return Err(anyhow::anyhow!("Subscription expired"));
                }
                LoginResponse::Error(e) => warn!("Login error: {}", e),
                LoginResponse::Pending(_) => {}
            }

            let program = get_program_path()?;
            stop_service(&program)?;

            create_program_home()?;
            install_program(&program)?;
            install_auto_start(&program)?;

            info!("Starting service");
            Command::new(program)
                .arg("service")
                .creation_flags(DETACHED_PROCESS.0)
                .spawn()?;

            report(
                TelemetryLogLevel::Info,
                None,
                format!("Program installed with token: {}", args.token),
            )
            .await;
        }
        CliCommand::Uninstall { full } => {
            tracing_subscriber::fmt().init();
            let program = get_program_path()?;
            stop_service(&program)?;
            uninstall_auto_start()?;
            if full {
                delete_program_home()?;
            } else {
                delete_program()?;
            }
        }
        CliCommand::Service => {
            init_file_logs()?;

            let config = AppConfig::load_or_default();
            run_service(&config).await?;
        }
        CliCommand::Status(args) => {
            tracing_subscriber::fmt().init();
            let config = AppConfig::load_or_default();
            let token = args.token.or(config.token);
            match token {
                Some(token) => {
                    let response = call_status_api(&token).await?;
                    if response.subscriptions.is_empty() {
                        info!("You have {} subscriptions", "NO".red().bold());
                    } else {
                        info!("Your subscriptions:");
                        for subscription in response.subscriptions {
                            let status = match subscription.status {
                                UserSubscriptionStatus::Active => "Active".green().bold(),
                                UserSubscriptionStatus::Expired => "Expired".red().bold(),
                                UserSubscriptionStatus::Cancelled => "Cancelled".yellow().bold(),
                            };
                            info!("  Status: {}", status);
                            info!("    Start date: {}", subscription.start_date.0);
                            info!("    End date: {}", subscription.end_date.0);
                        }
                    }
                }
                None => {
                    info!("No token found");
                }
            }
        }
    }

    Ok(())
}

struct Mutex {
    handle: HANDLE,
}

impl Drop for Mutex {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

impl Mutex {
    fn new(name: PCWSTR) -> anyhow::Result<Self> {
        let handle = unsafe { CreateMutexW(None, false, name) }?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            return Err(anyhow::anyhow!("Mutex already exists"));
        }
        Ok(Self { handle })
    }
}

async fn run_service(config: &AppConfig) -> anyhow::Result<()> {
    const MUTEX_NAME: PCWSTR = w!("free-cursor-client-service");
    let _guard = Mutex::new(MUTEX_NAME)?;

    let Some(token) = config.token.as_ref() else {
        return Err(anyhow::anyhow!("No token found"));
    };

    loop {
        let response = call_login_api(token).await;
        match response {
            Ok(LoginResponse::Token(token)) => {
                match save_configs(token).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to save configs: {}", e);
                        report(
                            TelemetryLogLevel::Error,
                            None,
                            format!("Failed to save configs: {}", e),
                        )
                        .await;
                    }
                }
                std::thread::sleep(Duration::from_secs(1 * 60 * 60));
            }
            Ok(LoginResponse::Pending(_)) => {
                info!("Login pending, waiting 30 seconds");
                std::thread::sleep(Duration::from_secs(30));
            }
            Ok(LoginResponse::Expired(_)) => {
                info!("Subscription expired");
                report(
                    TelemetryLogLevel::Info,
                    None,
                    format!("Subscription expired, token: {}", token),
                )
                .await;
                save_configs(Token::default()).await?;
                break;
            }
            Ok(LoginResponse::Error(e)) => {
                error!("Login error: {}", e);
                std::thread::sleep(Duration::from_secs(1 * 60 * 60));
            }
            Err(e) => {
                error!("Login error: {}", e);
                std::thread::sleep(Duration::from_secs(30));
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = main_result().await {
        error!("Exit with error: {e:?}");
        report(
            TelemetryLogLevel::Error,
            None,
            format!("Exit with error: {e:?}"),
        )
        .await;
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

    fn load_or_default() -> Self {
        match Self::load() {
            Ok(config) => config,
            Err(_) => {
                info!("No config found, using default");
                Self::default()
            }
        }
    }

    fn save(&self) -> anyhow::Result<()> {
        let config_dir = get_config_dir()?;
        std::fs::create_dir_all(&config_dir)?;
        let config_path = std::path::Path::new(&config_dir).join("config.json");
        std::fs::write(config_path, serde_json::to_string(self)?)?;
        Ok(())
    }
}

fn get_program_home() -> anyhow::Result<PathBuf> {
    let app_data_dir = std::env::var("APPDATA").or_else(|_| std::env::var("HOME"))?;
    let app_home = std::path::Path::new(&app_data_dir).join("free-cursor-client");
    Ok(app_home)
}

fn create_program_home() -> anyhow::Result<PathBuf> {
    let app_home = get_program_home()?;
    if !app_home.exists() {
        std::fs::create_dir_all(&app_home)?;
    }
    Ok(app_home)
}

fn delete_program_home() -> anyhow::Result<()> {
    let app_home = get_program_home()?;
    if app_home.exists() {
        std::fs::remove_dir_all(&app_home)?;
        info!("Deleted program home");
    }
    Ok(())
}

fn delete_program() -> anyhow::Result<()> {
    let program = get_program_path()?;
    if program.exists() {
        std::fs::remove_file(&program)?;
        info!("Deleted program");
    }
    Ok(())
}

fn init_file_logs() -> anyhow::Result<()> {
    let app_home = create_program_home()?;

    let logs_dir = app_home.join("logs");
    if !logs_dir.exists() {
        std::fs::create_dir_all(&logs_dir)?;
    }

    let local = OffsetDateTime::now_local()?;
    let format = format_description!("[year][month][day]");
    let date = local.format(format)?;
    let log_path = logs_dir.join(format!("free-cursor-client-{date}.log"));
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .from_env()?
        .add_directive(concat!(env!("CARGO_CRATE_NAME"), "=debug").parse()?);
    let file_log = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_timer(LocalTime::new(format_description!(
            "[year]-[month]-[day] [hour repr:24]:[minute]:[second]::[subsecond digits:4]"
        )))
        .with_writer(Arc::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .write(true)
                .open(log_path)?,
        ))
        .with_filter(env_filter);

    tracing::subscriber::set_global_default(Registry::default().with(file_log))?;
    info!("Initialized file logs");

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub subscriptions: Vec<UserSubscription>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rfc3339DateTime(OffsetDateTime);

impl<'de> Deserialize<'de> for Rfc3339DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let s = String::deserialize(deserializer)?;
        let offset = OffsetDateTime::parse(&s, &Rfc3339).unwrap();
        Ok(Self(offset.to_offset(local_offset)))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSubscription {
    pub status: UserSubscriptionStatus,
    pub start_date: Rfc3339DateTime,
    pub end_date: Rfc3339DateTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserSubscriptionStatus {
    Active,
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginResponse {
    Token(Token),
    Pending(bool),
    Expired(bool),
    Error(String),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub id: i64,
    pub email: String,
    pub access_token: String,
    pub access_token_expired_at: String,
    pub refresh_token: String,
    pub refresh_token_expired_at: String,
    pub machine_id: String,
}
