use std::{env, sync::Arc};

use crate::BackendOutputStream;
use once_cell::sync::Lazy;
use tokio::{
    io::{BufReader, Lines},
    process::ChildStderr,
    sync::{Mutex, oneshot},
};

/// Stderr stream that captures the stderr output of the backend and stores it
/// in a buffer for later use.
pub(crate) async fn stderr_buffer(
    buffer: Arc<Mutex<Lines<BufReader<ChildStderr>>>>,
    cancel: oneshot::Receiver<()>,
) -> Result<String, std::io::Error> {
    // Create a future that continuously read from the buffer and stores the lines
    // until all data is received.
    let mut lines = Vec::new();

    let read_and_buffer = async {
        let mut buffer = buffer.lock().await;
        while let Some(line) = buffer.next_line().await? {
            log_backend_stderr(&line);
            lines.push(line);
        }
        Ok(lines.join("\n"))
    };

    // Either wait until the cancel signal is received or the `read_and_buffer`
    // finishes which means there is no more data to read.
    tokio::select! {
        _ = cancel => {
            Ok(lines.join("\n"))
        }
        result = read_and_buffer => {
            result
        }
    }
}

/// Environment variable that forces backend stderr to be forwarded directly to
/// stderr, regardless of the tracing level.
const PRINT_STDERR_ENV: &str = "PIXI_PRINT_BACKEND_STDERR";

/// Controls whether backend stderr should also be echoed directly to stderr.
static FORCE_STDERR_PRINT: Lazy<bool> = Lazy::new(|| {
    env::var(PRINT_STDERR_ENV)
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "" | "0" | "false" | "off")
        })
        .unwrap_or(false)
});

fn log_backend_stderr(line: &str) {
    if tracing::event_enabled!(tracing::Level::DEBUG) {
        tracing::debug!(target: "pixi::backend::stderr", "{line}");
    }

    if *FORCE_STDERR_PRINT {
        eprintln!("[backend] {line}");
    }
}

/// Stderr stream that captures the stderr output of the backend and stores it
/// in a buffer for later use.
pub(crate) async fn stream_stderr<W: BackendOutputStream>(
    buffer: Arc<Mutex<Lines<BufReader<ChildStderr>>>>,
    cancel: oneshot::Receiver<()>,
    mut on_log: W,
) -> Result<String, std::io::Error> {
    // Create a future that continuously read from the buffer and stores the lines
    // until all data is received.
    let mut lines = Vec::new();
    let read_and_buffer = async {
        let mut buffer = buffer.lock().await;
        while let Some(line) = buffer.next_line().await? {
            log_backend_stderr(&line);
            on_log.on_line(line.clone());
            lines.push(line);
        }
        Ok(lines.join("\n"))
    };

    // Either wait until the cancel signal is received or the `read_and_buffer`
    // finishes which means there is no more data to read.
    tokio::select! {
        _ = cancel => {
            Ok(lines.join("\n"))
        }
        result = read_and_buffer => {
            result
        }
    }
}
