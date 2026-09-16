//! Size-based rotation for `{app_data}/logs/host.log` and `pi.stderr.log`.

use crate::events::{HostEvent, ProcessStatus};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
pub const LOG_BACKUPS: u32 = 3;

/// `path` → `path.1`, `path.1` → `path.2`, … then `path` is freed.
pub fn rotate_if_needed(path: &Path, max_bytes: u64, backups: u32) -> std::io::Result<bool> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if meta.len() <= max_bytes {
        return Ok(false);
    }
    if backups == 0 {
        fs::remove_file(path)?;
        return Ok(true);
    }
    let oldest = backup_path(path, backups);
    let _ = fs::remove_file(&oldest);
    for i in (1..backups).rev() {
        let from = backup_path(path, i);
        let to = backup_path(path, i + 1);
        if from.exists() {
            let _ = fs::rename(&from, &to);
        }
    }
    fs::rename(path, backup_path(path, 1))?;
    Ok(true)
}

pub fn backup_path(path: &Path, n: u32) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{n}"));
    PathBuf::from(s)
}

fn stamp() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms}")
}

#[derive(Clone)]
pub struct HostLogger {
    inner: Arc<Mutex<HostLoggerInner>>,
}

struct HostLoggerInner {
    path: PathBuf,
    max_bytes: u64,
    backups: u32,
}

impl HostLogger {
    pub fn new(path: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HostLoggerInner {
                path,
                max_bytes: MAX_LOG_BYTES,
                backups: LOG_BACKUPS,
            })),
        }
    }

    pub fn with_limits(path: PathBuf, max_bytes: u64, backups: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HostLoggerInner {
                path,
                max_bytes,
                backups,
            })),
        }
    }

    pub fn append(&self, level: &str, message: &str) {
        let Ok(inner) = self.inner.lock() else {
            return;
        };
        let _ = rotate_if_needed(&inner.path, inner.max_bytes, inner.backups);
        if let Some(parent) = inner.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&inner.path)
        else {
            return;
        };
        let line = format!("{} {level} {message}\n", stamp());
        let _ = f.write_all(line.as_bytes());
    }

    pub fn log_event(&self, event: &HostEvent) {
        match event {
            HostEvent::Log { level, message } => self.append(level, message),
            HostEvent::Process {
                status,
                code,
                message,
            } => {
                let extra = message.as_deref().unwrap_or("");
                match status {
                    ProcessStatus::Crashed => {
                        self.append("error", &format!("sidecar crashed code={code:?} {extra}"))
                    }
                    ProcessStatus::Spawned => self.append("info", "sidecar spawned"),
                    ProcessStatus::Exited => {
                        self.append("info", &format!("sidecar exited code={code:?}"))
                    }
                }
            }
            HostEvent::Watchdog { silence_ms } => {
                self.append("warn", &format!("sidecar silent for {silence_ms}ms"))
            }
            HostEvent::Rpc { .. } | HostEvent::UiRequest { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_when_over_max_and_keeps_backups() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("host.log");
        fs::write(&path, vec![b'x'; 64]).unwrap();
        assert!(rotate_if_needed(&path, 32, 3).unwrap());
        assert!(!path.exists());
        assert!(backup_path(&path, 1).exists());
        fs::write(&path, vec![b'y'; 64]).unwrap();
        assert!(rotate_if_needed(&path, 32, 3).unwrap());
        assert!(backup_path(&path, 1).exists());
        assert!(backup_path(&path, 2).exists());
        let first_gen = fs::read(backup_path(&path, 2)).unwrap();
        assert_eq!(first_gen.len(), 64);
        assert!(first_gen.iter().all(|b| *b == b'x'));
    }

    #[test]
    fn undersize_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("host.log");
        fs::write(&path, b"tiny").unwrap();
        assert!(!rotate_if_needed(&path, 32, 3).unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "tiny");
    }
}
