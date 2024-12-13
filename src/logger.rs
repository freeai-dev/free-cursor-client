use anyhow::Result;
use std::fs::OpenOptions;
use std::sync::Arc;
use time::macros::format_description;
use tracing::{level_filters::LevelFilter, subscriber::set_global_default};
use tracing_subscriber::{fmt::time::LocalTime, layer::SubscriberExt, EnvFilter, Layer, Registry};

use crate::config;

pub fn init_file_logs() -> Result<()> {
    let app_home = config::get_program_home()?;
    let logs_dir = app_home.join("logs");
    std::fs::create_dir_all(&logs_dir)?;

    let local = time::OffsetDateTime::now_local()?;
    let format = format_description!("[year][month][day]");
    let date = local.format(&format)?;
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

    set_global_default(Registry::default().with(file_log))?;
    Ok(())
}
