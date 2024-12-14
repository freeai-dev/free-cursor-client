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
                println!("Please enter your name:");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let input = input.trim().to_string();
                if !input.is_empty() {
                    break input;
                }
                println!("Name cannot be empty. Please try again.");
            };
            let contact = loop {
                println!("Please enter your contact info (phone or email):");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let input = input.trim().to_string();
                if !input.is_empty() {
                    break input;
                }
                println!("Contact info cannot be empty. Please try again.");
            };
            (Some(name), Some(contact), None)
        }
    };

    // Get promotion code
    println!("Please enter your promotion code (press Enter to skip):");
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

    println!("Order ID: {}", order.order.id);
    println!(
        "Token: {} (Token has been automatically saved to system)",
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

    println!("\nPlease scan the QR code below with Alipay to complete payment:");
    println!("{}", qr_string);
    println!("\nNote: The service activation may not be immediate after payment.");
    println!("For urgent processing, please contact customer@freeai.dev");

    Ok(())
}

fn select_package(packages: &[Package]) -> Result<Option<&Package>> {
    loop {
        println!("\nAvailable packages:");
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
            "\nEnter the number of your choice (1-{}) or 'q' to quit:",
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

        println!("Invalid selection. Please try again.");
    }
}
