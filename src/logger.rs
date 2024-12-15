use anyhow::Result;
use parking_lot::Mutex;
use std::fs::OpenOptions;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::OnceLock;
use time::macros::format_description;
use tokio::sync::mpsc;
use tracing::{level_filters::LevelFilter, subscriber::set_global_default};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{fmt::time::LocalTime, layer::SubscriberExt, EnvFilter, Layer, Registry};

use crate::{
    config,
    telemetry::{self, TelemetryLogLevel},
};

// 定义日志消息结构
pub(crate) struct LogMessage {
    pub(crate) level: TelemetryLogLevel,
    pub(crate) message: String,
    pub(crate) timestamp: i64,
    pub(crate) seq: usize,
}

// 添加新的 TelemetryLayer 结构体
struct TelemetryLayer {
    sender: mpsc::UnboundedSender<LogMessage>,
}

static SEQ: AtomicUsize = AtomicUsize::new(0);

impl TelemetryLayer {
    fn new() -> (Self, mpsc::UnboundedReceiver<LogMessage>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }
}

impl<S: Subscriber> Layer<S> for TelemetryLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = metadata.level();

        let telemetry_level = match *level {
            Level::ERROR => TelemetryLogLevel::Error,
            Level::WARN => TelemetryLogLevel::Warn,
            Level::INFO => TelemetryLogLevel::Info,
            Level::DEBUG | Level::TRACE => TelemetryLogLevel::Debug,
        };

        let mut message = String::new();
        let mut visitor = MessageVisitor(&mut message);
        event.record(&mut visitor);

        // 通过 channel 发送日志消息
        let _ = self.sender.send(LogMessage {
            level: telemetry_level,
            message,
            timestamp: (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1000 / 1000)
                as i64,
            seq: SEQ.fetch_add(1, Ordering::Relaxed),
        });
    }
}

// 修改 OnceLock 类型为 Sender 的克隆
static SHUTDOWN_TX: OnceLock<mpsc::UnboundedSender<()>> = OnceLock::new();
static TELEMETRY_TASK_JOIN_HANDLE: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);

// 修改 spawn_telemetry_task 函数，返回 shutdown sender
fn spawn_telemetry_task(
    mut receiver: mpsc::UnboundedReceiver<LogMessage>,
) -> mpsc::UnboundedSender<()> {
    let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();

    let handle = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let mut wants_exit = false;
        while !wants_exit {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    wants_exit = true;
                }
                log = receiver.recv() => {
                    if let Some(log) = log {
                        buffer.push(log);
                    } else {
                        wants_exit = true;
                    }
                }
            };

            loop {
                match receiver.try_recv() {
                    Ok(log) => buffer.push(log),
                    Err(_) => break,
                }
            }

            telemetry::report(buffer).await;
            buffer = Vec::new();
        }
    });
    *TELEMETRY_TASK_JOIN_HANDLE.lock() = Some(handle);

    shutdown_tx
}

// 添加一个等待日志发送完成的函数
pub async fn wait_for_logger() {
    if let Some(shutdown_tx) = SHUTDOWN_TX.get() {
        let _ = shutdown_tx.send(());
        if let Some(handle) = TELEMETRY_TASK_JOIN_HANDLE.lock().take() {
            let _ = handle.await;
        }
    }
}

// 用于提取日志消息的访问器
struct MessageVisitor<'a>(&'a mut String);

impl<'a> tracing::field::Visit for MessageVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push_str(&format!("{:?}", value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0.push_str(value);
        }
    }
}

pub fn init_file_logs() -> Result<()> {
    let project_dirs = config::get_project_dirs()?;
    let logs_dir = project_dirs
        .data_local_dir()
        .join(env!("CARGO_PKG_VERSION"))
        .join("logs");
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
        ));

    // 创建 telemetry layer 和 receiver
    let (telemetry_layer, receiver) = TelemetryLayer::new();

    // 启动日志处理任务并保存 shutdown sender
    let shutdown_tx = spawn_telemetry_task(receiver);
    let _ = SHUTDOWN_TX.set(shutdown_tx);

    // 设置全局 subscriber
    set_global_default(
        Registry::default()
            .with(env_filter)
            .with(file_log)
            .with(telemetry_layer),
    )?;
    Ok(())
}

pub fn init_console_logs() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .from_env()?
        .add_directive(concat!(env!("CARGO_CRATE_NAME"), "=debug").parse()?);

    let console_log =
        tracing_subscriber::fmt::layer().with_timer(LocalTime::new(format_description!(
            "[year]-[month]-[day] [hour repr:24]:[minute]:[second]::[subsecond digits:4]"
        )));

    // 创建 telemetry layer 和 receiver
    let (telemetry_layer, receiver) = TelemetryLayer::new();

    // 启动日志处理任务并保存 shutdown sender
    let shutdown_tx = spawn_telemetry_task(receiver);
    let _ = SHUTDOWN_TX.set(shutdown_tx);

    // 设置全局 subscriber
    set_global_default(
        Registry::default()
            .with(env_filter)
            .with(console_log)
            .with(telemetry_layer),
    )?;
    Ok(())
}
