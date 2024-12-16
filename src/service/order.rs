use anyhow::{bail, Result};
use tracing::error;

use crate::logger;
use crate::models::GeneralResponse;
use crate::{api::get_payment_url, models::Package};
use crate::{
    api::{self},
    config::AppConfig,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub async fn handle_order() -> Result<()> {
    logger::init_console_logs()?;

    let config = AppConfig::load_or_default();

    // Fetch available packages
    let packages = api::get_packages().await?;

    // Show package selection UI
    let Some(selected_package) = select_package(&packages.packages)? else {
        anyhow::bail!("User cancelled package selection");
    };

    // Ask user info
    let (name, contact, token) = match config.token {
        Some(token) => (None, None, Some(token)),
        None => {
            let name = loop {
                println!("请输入您的姓名：");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let input = input.trim().to_string();
                if !input.is_empty() {
                    break input;
                }
                println!("姓名不能为空，请重新输入。");
            };
            let contact = loop {
                println!("请输入您的联系方式（手机号或邮箱）:");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let input = input.trim().to_string();
                if !input.is_empty() {
                    break input;
                }
                println!("联系方式不能为空，请重新输入。");
            };
            (Some(name), Some(contact), None)
        }
    };

    // Get promotion code
    println!("请输入邀请码（如无邀请码请直接按回车）:");
    let mut promotion_code = String::new();
    std::io::stdin().read_line(&mut promotion_code)?;
    let promotion_code = promotion_code.trim();

    // Create order
    let order = api::create_order(
        selected_package.id.to_string(),
        promotion_code,
        name,
        contact,
        token,
    )
    .await?;

    let mut config = AppConfig::load_or_default();
    config.token = Some(order.token.clone());
    config.save()?;

    println!("订单号：{}", order.order.id);
    println!("Token: {} (Token 已自动保存到系统)", order.token);

    open::that_detached(&get_payment_url(&order.order.id).await?.url)?;

    // Add polling logic
    println!("\n等待支付完成...");
    let start_time = Instant::now();
    let timeout = Duration::from_secs(15 * 60); // 15 minutes

    while start_time.elapsed() < timeout {
        sleep(Duration::from_secs(10)).await; // Poll every 3 seconds

        let order_status = api::get_order_status(&order.order.id).await?;
        match order_status {
            GeneralResponse::Success(order_status) => {
                if order_status.status == "completed" {
                    println!("支付成功！");
                    crate::service::do_self_install(order.token).await?;
                    return Ok(());
                }
            }
            GeneralResponse::Error(err) => {
                error!("获取订单状态失败：{}", err.error);
                bail!("获取订单状态失败：{}", err.error);
            }
        }
    }

    println!("支付超时，请重新尝试下单。");
    anyhow::bail!("支付超时，请重新尝试下单。")
}

fn select_package(packages: &[Package]) -> Result<Option<&Package>> {
    loop {
        println!("\n可选套餐：");
        for (i, package) in packages.iter().enumerate() {
            println!(
                "{}. {} - ￥{} ({})",
                i + 1,
                package.name,
                package.price,
                package.duration
            );
        }
        println!(
            "\n请输入您选择的套餐编号 (1-{}) 或输入 'q' 退出：",
            packages.len()
        );

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("q") {
            return Ok(None);
        }

        if let Ok(choice) = input.parse::<usize>() {
            if choice >= 1 && choice <= packages.len() {
                return Ok(Some(&packages[choice - 1]));
            }
        }

        println!("选择无效，请重新输入。");
    }
}
