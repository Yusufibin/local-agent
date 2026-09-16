mod common;

use common::{event_types, spawn_fake, wait_until};
use deskpi_lib::bridge::UiBridge;
use deskpi_lib::events::{wrap_stdout, HostEvent};
use deskpi_lib::rpc::{handle_stdout_record, PendingMap};
use serde_json::{json, Value};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn golden(name: &str) -> String {
    fs::read_to_string(common::golden_dir().join(name)).unwrap()
}

#[test]
fn command_id_round_trip_unknown_id_ignored_timeout_and_prompt_ack() {
    let live = spawn_fake("slow-settled", &[]);
    let resp = live
        .rpc
        .send_command("get_state", json!({}))
        .expect("get_state");
    assert_eq!(resp.success, Some(true));
    assert!(resp.id.is_some());
    assert_eq!(
        resp.data.as_ref().unwrap()["model"]["id"],
        "fake-model"
    );

    let mut pending = PendingMap::new();
    let mut ui = UiBridge::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = {
        let events = events.clone();
        move |e: HostEvent| events.lock().unwrap().push(e)
    };
    handle_stdout_record(
        r#"{"id":"unknown-id","type":"response","command":"get_state","success":true}"#,
        &mut pending,
        &mut ui,
        &sink,
    )
    .unwrap();
    assert!(events.lock().unwrap().is_empty());

    let hang = spawn_fake("default", &[("FAKE_PI_HANG", "1")]);
    let err = hang
        .rpc
        .send_command_timeout(
            "get_available_models",
            json!({}),
            Some(Duration::from_millis(200)),
        )
        .expect_err("timeout");
    assert!(err.contains("timeout"), "{err}");

    let started = Instant::now();
    let ack = live.rpc.send_command("prompt", json!({"message": "hi"})).unwrap();
    let ack_elapsed = started.elapsed();
    assert_eq!(ack.success, Some(true));
    assert!(
        ack_elapsed < Duration::from_millis(350),
        "prompt must return ack without waiting for agent_settled, took {ack_elapsed:?}"
    );
}

#[test]
fn goldens_produce_host_events() {
    let mut pending = PendingMap::new();
    let mut ui = UiBridge::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = {
        let events = events.clone();
        move |e: HostEvent| events.lock().unwrap().push(e)
    };

    for name in ["prompt.jsonl", "tool_call.jsonl", "extension_ui_request.jsonl"] {
        for line in golden(name).lines().filter(|l| !l.is_empty()) {
            handle_stdout_record(line, &mut pending, &mut ui, &sink).unwrap();
        }
    }
    let kinds: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| match e {
            HostEvent::Rpc { event } => {
                format!("rpc:{}", event.get("type").and_then(|v| v.as_str()).unwrap())
            }
            HostEvent::UiRequest { request } => format!(
                "ui_request:{}",
                request.get("method").and_then(|v| v.as_str()).unwrap()
            ),
            HostEvent::Process { .. } => "process".into(),
            HostEvent::Log { .. } => "log".into(),
            HostEvent::Watchdog { .. } => "watchdog".into(),
        })
        .collect();
    assert!(kinds.contains(&"rpc:agent_start".into()));
    assert!(kinds.contains(&"rpc:agent_settled".into()));
    assert!(kinds.contains(&"rpc:tool_execution_start".into()));
    assert!(kinds.contains(&"ui_request:select".into()));
    assert!(kinds.contains(&"ui_request:confirm".into()));

    let wrapped = wrap_stdout(serde_json::from_str::<Value>(r#"{"type":"agent_start"}"#).unwrap());
    assert!(matches!(wrapped, HostEvent::Rpc { .. }));
}

#[test]
fn fake_sidecar_emits_prompt_golden_as_host_events() {
    let live = spawn_fake("prompt", &[]);
    live.rpc
        .send_command("prompt", json!({"message": "Hello"}))
        .unwrap();
    assert!(wait_until(Duration::from_secs(2), || {
        event_types(&live.events)
            .iter()
            .any(|t| t == "rpc:agent_settled")
    }));
    let kinds = event_types(&live.events);
    assert!(kinds.iter().any(|t| t == "rpc:agent_start"));
    assert!(kinds.iter().any(|t| t == "rpc:message_update"));
}
