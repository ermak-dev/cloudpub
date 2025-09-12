use anyhow::{Context as _, Result};
use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use std::path::Path;
use tracing::debug;
pub use tracing_appender::non_blocking::WorkerGuard;
use tracing_log::LogTracer;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

pub fn init_log(
    level: &str,
    log_file: &Path,
    stderr: bool,
    max_size: usize,
    max_files: usize,
) -> Result<WorkerGuard> {
    let file_appender = BasicRollingFileAppender::new(
        log_file,
        RollingConditionBasic::new().max_size(max_size as u64), // 10 MB
        max_files,
    )
    .context("Failed to create rolling file appender")?;

    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let trace_cfg = format!(
        "{},hyper=error,tokio_postgres=info,pingora_core=info,pingora_proxy=off,pingora_pool=error,tungstenite=info,{}",
        level,
        std::env::var("RUST_LOG").unwrap_or_default()
    );

    let timer = time::format_description::parse(
        "[year]-[month padding:zero]-[day padding:zero] [hour]:[minute]:[second]",
    )
    .context("Failed to parse time format")?;

    let time_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let timer = fmt::time::OffsetTime::new(time_offset, timer);

    // Create the file layer with common settings
    let file_layer = fmt::Layer::default()
        .with_timer(timer.clone())
        .with_ansi(false)
        .with_writer(file_writer);

    // Build a registry with the file layer and conditional stderr layer
    let registry = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(trace_cfg))
        .with(file_layer);

    // Add stderr layer only if requested
    if stderr {
        let stderr_layer = fmt::Layer::default()
            .with_timer(timer)
            .with_ansi(cfg!(unix))
            .with_writer(std::io::stderr);

        tracing::subscriber::set_global_default(registry.with(stderr_layer))
            .context("Failed to set global default subscriber")?;
    } else {
        tracing::subscriber::set_global_default(registry)
            .context("Failed to set global default subscriber")?;
    }

    LogTracer::init().context("Failed to initialize log tracer")?;
    log_panics::init();

    debug!("Tracing initialized ({})", level);
    Ok(guard)
}
