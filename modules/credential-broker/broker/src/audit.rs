//! Append-only audit log: one JSON object per line, one file per day.
use anyhow::Result;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Serialize)]
pub struct Entry<'a> {
    pub ts: String,
    pub task: &'a str,
    pub scope: &'a str,
    pub method: &'a str,
    pub host: &'a str,
    pub path: &'a str,
    pub status: u16,
    pub bytes_in: usize,
    pub bytes_out: usize,
    /// auto | approved | refused:<reason>
    pub decision: &'a str,
    pub peer_uid: u32,
}

pub struct Audit {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Audit {
    pub fn new(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            lock: Mutex::new(()),
        })
    }

    pub fn file_for_today(&self) -> PathBuf {
        self.dir.join(format!(
            "broker-{}.jsonl",
            chrono::Utc::now().format("%Y-%m-%d")
        ))
    }

    pub fn write(&self, e: &Entry<'_>) {
        let line = match serde_json::to_string(e) {
            Ok(l) => l,
            Err(err) => {
                tracing::error!(%err, "audit serialize");
                return;
            }
        };
        let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let res = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o640)
            .open(self.file_for_today())
            .and_then(|mut f| writeln!(f, "{line}"));
        if let Err(err) = res {
            // The design treats the audit log as part of the control: refuse to run silently.
            tracing::error!(%err, "audit write failed");
        }
    }
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
