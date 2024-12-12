use serde::Serialize;
use tracing::warn;

use crate::AppConfig;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryLog {
    token: String,
    log: String,
    os: String,
    version: String,
    machine_id: String,
    level: TelemetryLogLevel,
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

pub(crate) async fn report(level: TelemetryLogLevel, token: Option<String>, message: String) {
    let token = token
        .or_else(|| AppConfig::load_or_default().token)
        .unwrap_or_default();

    let os = std::env::consts::OS;
    let version = env!("CARGO_PKG_VERSION");
    let machine_id = machine_uid::get().unwrap_or_else(|err| format!("GetMachineIdError: {err:?}"));
    let log = TelemetryLog {
        token,
        log: message,
        os: os.to_string(),
        version: version.to_string(),
        machine_id: machine_id.to_string(),
        level,
    };

    let client = reqwest::Client::new();
    match client
        .post("https://auth-server.freeai.dev/api/v1/cursor/telemetry")
        .json(&log)
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
                match response.text().await {
                    Ok(text) => warn!("Failed to send telemetry log: {text}"),
                    Err(e) => warn!("Failed to send telemetry log: {e:?}"),
                }
            }
        }
        Err(e) => warn!("Failed to send telemetry log: {e:?}"),
    }
}
