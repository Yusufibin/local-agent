//! List Pi sessions by scanning session-dir JSONL headers (not SessionManager RPC).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub path: PathBuf,
    pub id: Option<String>,
    pub timestamp: Option<String>,
    pub cwd: Option<String>,
    pub name: Option<String>,
}

pub fn list_sessions(dir: &Path) -> Vec<SessionInfo> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(info) = read_header(&path) {
            out.push(info);
        }
    }
    out.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    out
}

fn read_header(path: &Path) -> Option<SessionInfo> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let value: Value = serde_json::from_str(line.trim_end_matches(['\n', '\r'])).ok()?;
    Some(SessionInfo {
        path: path.to_path_buf(),
        id: value.get("id").and_then(|v| v.as_str()).map(str::to_string),
        timestamp: value
            .get("timestamp")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        cwd: value.get("cwd").and_then(|v| v.as_str()).map(str::to_string),
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}
