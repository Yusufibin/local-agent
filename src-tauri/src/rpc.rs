//! Thin JSONL RPC client: UUID ids, pending map, timeouts, event wrapping.

use crate::bridge::UiBridge;
use crate::events::{classify_stdout, wrap_stdout, HostEvent, StdoutClass};
use crate::jsonl::{Frame, JsonlFramer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(30);

pub fn command_uses_default_timeout(command: &str) -> bool {
    !matches!(command, "prompt" | "compact" | "bash")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub type_name: String,
    pub command: Option<String>,
    pub success: Option<bool>,
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Timeouts {
    pub default: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            default: DEFAULT_RPC_TIMEOUT,
        }
    }
}

pub struct PendingMap {
    inner: HashMap<String, Sender<RpcResponse>>,
}

impl PendingMap {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: String, tx: Sender<RpcResponse>) {
        self.inner.insert(id, tx);
    }

    /// Unknown ids are ignored (no panic, no send).
    pub fn complete(&mut self, id: &str, response: RpcResponse) -> bool {
        if let Some(tx) = self.inner.remove(id) {
            let _ = tx.send(response);
            true
        } else {
            false
        }
    }

    pub fn cancel(&mut self, id: &str) {
        self.inner.remove(id);
    }

    /// Drop all waiters so `recv` returns Disconnected (reader exit / sidecar death).
    pub fn fail_all(&mut self) {
        self.inner.clear();
    }
}

impl Default for PendingMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle one complete JSONL record from sidecar stdout.
/// The GUI is never waited on here: ui_request is forwarded and the caller continues.
pub fn handle_stdout_record(
    raw: &str,
    pending: &mut PendingMap,
    ui: &mut UiBridge,
    sink: &impl Fn(HostEvent),
) -> Result<(), String> {
    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    match classify_stdout(&value) {
        StdoutClass::Response => {
            let parsed: RpcResponse =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            if let Some(id) = parsed.id.clone() {
                pending.complete(&id, parsed);
            }
            Ok(())
        }
        StdoutClass::UiRequest => {
            ui.on_request(&value);
            sink(wrap_stdout(value));
            Ok(())
        }
        StdoutClass::RpcEvent => {
            sink(wrap_stdout(value));
            Ok(())
        }
    }
}

pub fn encode_command(mut body: Value, id: &str) -> Result<Vec<u8>, String> {
    if let Some(obj) = body.as_object_mut() {
        obj.insert("id".into(), json!(id));
    }
    let mut bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn new_request_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Clone)]
pub struct RpcSession {
    pub stdin: Arc<Mutex<Box<dyn Write + Send>>>,
    pub pending: Arc<Mutex<PendingMap>>,
    pub ui: Arc<Mutex<UiBridge>>,
    pub timeouts: Timeouts,
    pub oversize: Arc<Mutex<bool>>,
}

impl RpcSession {
    pub fn send_command(
        &self,
        command: &str,
        body: Value,
    ) -> Result<RpcResponse, String> {
        let timeout = if command_uses_default_timeout(command) {
            Some(self.timeouts.default)
        } else {
            None
        };
        self.send_command_timeout(command, body, timeout)
    }

    pub fn send_command_timeout(
        &self,
        command: &str,
        mut body: Value,
        timeout: Option<Duration>,
    ) -> Result<RpcResponse, String> {
        let id = new_request_id();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("id".into(), json!(id));
            obj.insert("type".into(), json!(command));
        }
        let (tx, rx) = mpsc::channel();
        {
            let mut pending = self.pending.lock().map_err(|e| e.to_string())?;
            pending.insert(id.clone(), tx);
        }
        {
            let mut stdin = self.stdin.lock().map_err(|e| e.to_string())?;
            let mut bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            stdin.write_all(&bytes).map_err(|e| e.to_string())?;
            stdin.flush().map_err(|e| e.to_string())?;
        }
        match timeout {
            Some(d) => match rx.recv_timeout(d) {
                Ok(resp) => Ok(resp),
                Err(RecvTimeoutError::Timeout) => {
                    if let Ok(mut pending) = self.pending.lock() {
                        pending.cancel(&id);
                    }
                    Err(format!("rpc timeout after {d:?} for {command}"))
                }
                Err(RecvTimeoutError::Disconnected) => Err("rpc channel closed".into()),
            },
            None => rx.recv().map_err(|_| "rpc channel closed".to_string()),
        }
    }

    pub fn write_raw(&self, value: &Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().map_err(|e| e.to_string())?;
        let mut bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        stdin.write_all(&bytes).map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn spawn_stdout_reader<R, S>(
    mut reader: R,
    pending: Arc<Mutex<PendingMap>>,
    ui: Arc<Mutex<UiBridge>>,
    oversize: Arc<Mutex<bool>>,
    sink: S,
) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
    S: Fn(HostEvent) + Send + 'static,
{
    thread::spawn(move || {
        let mut framer = JsonlFramer::new();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let frames = framer.push(&buf[..n]);
                    for frame in frames {
                        match frame {
                            Frame::Oversize => {
                                if let Ok(mut flag) = oversize.lock() {
                                    *flag = true;
                                }
                                sink(HostEvent::log(
                                    "error",
                                    "JSONL line exceeded 16 MiB; dropping and restarting sidecar",
                                ));
                            }
                            Frame::Record(bytes) => {
                                let Ok(line) = String::from_utf8(bytes) else {
                                    sink(HostEvent::log("warn", "non-utf8 JSONL record dropped"));
                                    continue;
                                };
                                let mut pending = match pending.lock() {
                                    Ok(p) => p,
                                    Err(_) => break,
                                };
                                let mut ui = match ui.lock() {
                                    Ok(u) => u,
                                    Err(_) => break,
                                };
                                if let Err(e) =
                                    handle_stdout_record(&line, &mut pending, &mut ui, &sink)
                                {
                                    sink(HostEvent::log("warn", format!("stdout parse: {e}")));
                                }
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        if let Ok(mut pending) = pending.lock() {
            pending.fail_all();
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::UiBridge;

    fn sink_collect(buf: Arc<Mutex<Vec<HostEvent>>>) -> impl Fn(HostEvent) {
        move |ev| buf.lock().unwrap().push(ev)
    }

    #[test]
    fn unknown_response_id_is_ignored() {
        let mut pending = PendingMap::new();
        let mut ui = UiBridge::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let s = sink_collect(events.clone());
        let raw = r#"{"id":"ghost","type":"response","command":"get_state","success":true}"#;
        handle_stdout_record(raw, &mut pending, &mut ui, &s).unwrap();
        assert!(events.lock().unwrap().is_empty());
        assert!(pending.inner.is_empty());
    }

    #[test]
    fn prompt_ack_does_not_wait_for_settled() {
        let mut pending = PendingMap::new();
        let (tx, rx) = mpsc::channel();
        pending.insert("req-1".into(), tx);
        let mut ui = UiBridge::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let s = sink_collect(events.clone());
        handle_stdout_record(
            r#"{"id":"req-1","type":"response","command":"prompt","success":true}"#,
            &mut pending,
            &mut ui,
            &s,
        )
        .unwrap();
        let ack = rx.recv_timeout(Duration::from_millis(50)).unwrap();
        assert_eq!(ack.success, Some(true));
        handle_stdout_record(r#"{"type":"agent_start"}"#, &mut pending, &mut ui, &s).unwrap();
        handle_stdout_record(r#"{"type":"agent_settled"}"#, &mut pending, &mut ui, &s).unwrap();
        let kinds: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .map(|e| match e {
                HostEvent::Rpc { event } => event.get("type").and_then(|v| v.as_str()).unwrap().to_string(),
                HostEvent::Watchdog { .. } => "watchdog".into(),
                _ => "other".into(),
            })
            .collect();
        assert_eq!(kinds, vec!["agent_start", "agent_settled"]);
    }
}
