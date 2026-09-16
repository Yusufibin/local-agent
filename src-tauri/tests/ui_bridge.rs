mod common;

use common::{event_types, spawn_fake, wait_until};
use serde_json::json;
use std::fs;
use std::time::Duration;

#[test]
fn extension_ui_bridge_forwards_without_blocking_and_ignores_late_response() {
    let ui_log = tempfile::NamedTempFile::new().unwrap();
    let live = spawn_fake(
        "ui_select",
        &[("DESKPI_FAKE_UI_LOG", ui_log.path().to_str().unwrap())],
    );

    assert!(
        wait_until(Duration::from_secs(2), || {
            event_types(&live.events)
                .iter()
                .any(|t| t == "ui_request:select")
        }),
        "ui_request must be forwarded"
    );
    // Reader is not blocked: get_state still works after the dialog request.
    let state = live
        .rpc
        .send_command("get_state", json!({}))
        .expect("get_state while dialog open");
    assert_eq!(state.success, Some(true));
    assert!(event_types(&live.events)
        .iter()
        .any(|t| t == "rpc:agent_start"));

    live.rpc
        .ui
        .lock()
        .unwrap()
        .on_request(&json!({
            "type": "extension_ui_request",
            "id": "ui-select-1",
            "method": "select"
        }));
    assert!(live.rpc.ui.lock().unwrap().accept_response("ui-select-1"));
    live.rpc
        .write_raw(&json!({
            "type": "extension_ui_response",
            "id": "ui-select-1",
            "value": "Allow"
        }))
        .unwrap();
    assert!(wait_until(Duration::from_secs(1), || {
        fs::read_to_string(ui_log.path())
            .unwrap_or_default()
            .contains("ui-select-1")
    }));

    let live2 = spawn_fake("ui_confirm", &[]);
    assert!(wait_until(Duration::from_secs(2), || {
        event_types(&live2.events)
            .iter()
            .any(|t| t == "ui_request:confirm")
    }));
    std::thread::sleep(Duration::from_millis(120));
    assert!(
        !live2.rpc.ui.lock().unwrap().accept_response("ui-confirm-1"),
        "late response after timeout must be ignored"
    );
}
