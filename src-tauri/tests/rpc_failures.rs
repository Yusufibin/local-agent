use deskpi_lib::rpc::{RpcResponse, RpcSession, PendingMap, Timeouts};
use deskpi_lib::bridge::UiBridge;
use serde_json::json;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

#[test]
fn application_failure_is_not_a_successful_host_result() {
    let response: RpcResponse = serde_json::from_value(json!({
        "type": "response", "command": "set_model", "success": false, "error": "Unknown model"
    })).unwrap();
    assert_eq!(response.checked().unwrap_err(), "Unknown model");
}

struct BrokenPipe;
impl Write for BrokenPipe {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn failed_write_cancels_its_pending_request() {
    let rpc = RpcSession {
        stdin: Arc::new(Mutex::new(Box::new(BrokenPipe))),
        pending: Arc::new(Mutex::new(PendingMap::new())),
        ui: Arc::new(Mutex::new(UiBridge::new())),
        timeouts: Timeouts::default(),
        oversize: Arc::new(Mutex::new(false)),
    };
    assert!(rpc.send_command("get_state", json!({})).is_err());
    assert!(rpc.pending.lock().unwrap().is_empty());
}
