use anyhow::Result;
use qrcode::{render::unicode, QrCode};

use crate::{api::get_payment_url, models::Package};
use crate::{
    api::{self},
    config::AppConfig,
};

pub async fn handle_order() -> Result<()> {
    tracing_subscriber::fmt().init();

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
                println!("请输入您的姓名:");
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

    println!("订单号: {}", order.order.id);
    println!(
        "Token: {} (Token 已自动保存到系统)",
        order.token
    );

    // Install the program before showing QR code
    crate::service::do_install(order.token).await?;

    // Generate and display QR code
    let qr = QrCode::new(&get_payment_url(&order.order.id).await?.url)?;
    let qr_string = qr
        .render::<unicode::Dense1x2>()
        .dark_color(unicode::Dense1x2::Light)
        .light_color(unicode::Dense1x2::Dark)
        .build();

    println!("\n请使用支付宝扫描下方二维码完成支付:");
    println!("{}", qr_string);
    println!("\n注意：支付完成后服务可能不会立即生效。");
    println!("如需加急处理，请联系 customer@freeai.dev");

    Ok(())
}

fn select_package(packages: &[Package]) -> Result<Option<&Package>> {
    loop {
        println!("\n可选套餐:");
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
            "\n请输入您选择的套餐编号 (1-{}) 或输入 'q' 退出:",
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
