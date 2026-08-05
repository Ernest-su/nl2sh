use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[derive(Clone)]
/// Append-only JSON Lines history used for diagnostics across process restarts.
pub struct HistoryLog {
    path: PathBuf,
    file: Arc<Mutex<File>>,
}

#[derive(Serialize)]
struct HistoryRecord<'a> {
    timestamp_ms: u128,
    event: &'a str,
    message: &'a str,
}

impl HistoryLog {
    /// Opens the configured log path, resolving relative paths beside config.toml.
    pub fn open(config_path: &Path, configured_path: &Path) -> Result<Self> {
        let path = if configured_path.is_absolute() {
            configured_path.to_path_buf()
        } else {
            config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(configured_path)
        };
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options
            .open(&path)
            .with_context(|| format!("cannot open history log {}", path.display()))?;
        Ok(Self {
            path,
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Appends and flushes one structured event so crash diagnostics remain available.
    pub fn record(&self, event: &str, message: &str) -> Result<()> {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let record = HistoryRecord {
            timestamp_ms,
            event,
            message,
        };
        let encoded = serde_json::to_string(&record).context("cannot encode history record")?;
        let mut file = self
            .file
            .lock()
            .map_err(|_| anyhow::anyhow!("history log lock is poisoned"))?;
        writeln!(file, "{encoded}")
            .with_context(|| format!("cannot write history log {}", self.path.display()))?;
        file.flush()
            .with_context(|| format!("cannot flush history log {}", self.path.display()))
    }

    /// Returns the resolved log path shown to users and diagnostics.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
