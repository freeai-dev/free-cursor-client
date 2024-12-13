use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

use crate::models::{LoginResponse, StatusResponse};

pub async fn call_status_api(token: &str) -> Result<StatusResponse> {
    let client = Client::builder()
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

pub async fn call_login_api(token: &str) -> Result<LoginResponse> {
    let machine_id = machine_uid::get().map_err(|_| anyhow::anyhow!("Failed to get machine id"))?;
    let client = Client::builder()
        .timeout(Duration::from_secs(60 * 3))
        .build()?;
    let response: LoginResponse = client
        .post("https://auth-server.freeai.dev/api/v1/cursor/token")
        .json(&serde_json::json!({
            "token": token,
            "machineId": machine_id
        }))
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}
