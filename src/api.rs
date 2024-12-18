use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::models::{
    GeneralResponse, LoginResponse, OrderResponse, PackageResponse, PaymentUrlResponse,
    StatusResponse, UpdateCheckResponse,
};

#[derive(Deserialize)]
pub struct OrderStatus {
    pub status: String,
}

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

pub async fn get_packages() -> Result<PackageResponse> {
    let client = create_client()?;
    let response = client
        .get("https://auth-server.freeai.dev/api/v1/packages")
        .send()
        .await?;

    let packages = response.json().await?;
    Ok(packages)
}

pub async fn create_order(
    package_id: String,
    promotion_code: &str,
    name: Option<String>,
    contact: Option<String>,
    token: Option<String>,
) -> Result<OrderResponse> {
    let client = create_client()?;
    let response = client
        .post("https://auth-server.freeai.dev/api/v1/orders")
        .json(&serde_json::json!({
            "packageId": package_id,
            "promotionCode": promotion_code,
            "name": name,
            "contact": contact,
            "token": token
        }))
        .send()
        .await?;

    let order = response.json().await?;
    Ok(order)
}

pub async fn get_payment_url(order_id: &str) -> Result<PaymentUrlResponse> {
    let client = create_client()?;
    let response = client
        .get(format!(
            "https://auth-server.freeai.dev/api/v1/payment/url/{order_id}"
        ))
        .send()
        .await?;

    let payment_url = response.json().await?;
    Ok(payment_url)
}

pub async fn check_update() -> Result<GeneralResponse<UpdateCheckResponse>> {
    let client = create_client()?;
    let response = client
        .get(format!(
            "https://auth-server.freeai.dev/api/v1/versions/check-update?currentVersion={}&platform={}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS
        ))
        .send()
        .await?;

    let update_info = response.json().await?;
    Ok(update_info)
}

pub async fn get_order_status(order_id: &str) -> Result<GeneralResponse<OrderStatus>> {
    let client = create_client()?;
    let url = format!("https://auth-server.freeai.dev/api/v1/orders/{}", order_id);

    let response = client.get(&url).send().await?;
    let status = response.json().await?;

    Ok(status)
}

fn create_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(60 * 3))
        .build()?)
}
