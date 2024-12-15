use serde::Serialize;

use crate::{config::AppConfig, logger::LogMessage};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryLog {
    token: String,
    log: String,
    os: String,
    version: String,
    machine_id: String,
    build: String,
    level: TelemetryLogLevel,
    pid: u32,
    seq: usize,
    timestamp: i64,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TelemetryLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl ToString for TelemetryLogLevel {
    fn to_string(&self) -> String {
        match self {
            TelemetryLogLevel::Debug => "debug",
            TelemetryLogLevel::Info => "info",
            TelemetryLogLevel::Warn => "warn",
            TelemetryLogLevel::Error => "error",
        }
        .to_string()
    }
}

pub(crate) async fn report(logs: Vec<LogMessage>) {
    let token = AppConfig::load_or_default().token.unwrap_or_default();

    let os_type = os_info::get().os_type().to_string();
    let os_version = os_info::get().version().to_string();
    let os = format!("{os_type} {os_version}");
    let version = env!("CARGO_PKG_VERSION");
    let machine_id = machine_uid::get().unwrap_or_else(|err| format!("GetMachineIdError: {err:?}"));
    let build = env!("BUILD_ID");
    let pid = std::process::id();

    let logs: Vec<_> = logs
        .into_iter()
        .map(|log| TelemetryLog {
            token: token.clone(),
            log: log.message,
            os: os.clone(),
            version: version.to_string(),
            machine_id: machine_id.to_string(),
            build: build.to_string(),
            level: log.level,
            pid,
            seq: log.seq,
            timestamp: log.timestamp,
        })
        .collect();

    let client = reqwest::Client::new();
    let _ = client
        .post("https://auth-server.freeai.dev/api/v1/cursor/telemetry")
        .json(&logs)
        .send()
        .await;
}
