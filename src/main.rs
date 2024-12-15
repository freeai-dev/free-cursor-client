#![windows_subsystem = "windows"]

mod api;
mod cli;
mod config;
mod logger;
mod models;
mod service;
mod telemetry;
mod utils;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, CliCommand};
use tracing::error;

#[tokio::main]
async fn main() {
    if let Err(e) = main_result().await {
        error!("程序退出时发生错误：{e:?}");
        telemetry::report(
            telemetry::TelemetryLogLevel::Error,
            None,
            format!("程序退出时发生错误：{e:?}"),
        )
        .await;
    }
}

async fn main_result() -> Result<()> {
    let args = match Cli::try_parse() {
        Ok(args) => args,
        Err(e) => {
            utils::attach_console()?;
            e.exit();
        }
    };

    if !matches!(args.command, CliCommand::Service) {
        utils::attach_console()?;
    }

    match args.command {
        CliCommand::Install(args) => service::handle_install(args).await?,
        CliCommand::Uninstall { full } => service::handle_uninstall(full).await?,
        CliCommand::Service => service::run_service().await?,
        CliCommand::Status(args) => service::handle_status(args).await?,
        CliCommand::Invite(args) => service::handle_invite(args).await?,
        CliCommand::Order => service::order::handle_order().await?,
    }

    Ok(())
}
