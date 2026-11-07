use std::path::Path;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_logging(log_level: &str, log_file: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let stdout_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    let registry = tracing_subscriber::registry().with(filter).with(stdout_layer);

    if let Some(file_path) = log_file {
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(file_path)?;
        let file_layer = fmt::layer().with_ansi(false).with_writer(file);
        registry.with(file_layer).try_init().ok();
    } else {
        registry.try_init().ok();
    }

    Ok(())
}
