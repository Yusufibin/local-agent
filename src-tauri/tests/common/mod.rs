#![allow(dead_code)]

use deskpi_lib::bridge::UiBridge;
use deskpi_lib::events::HostEvent;
use deskpi_lib::rpc::{spawn_stdout_reader, PendingMap, RpcSession, Timeouts};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub fn fake_pi() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/helpers/fake-pi.mjs")
}

pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/rpc_golden")
}

pub fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if pred() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

pub struct LiveSidecar {
    pub child: Child,
    pub rpc: RpcSession,
    pub events: Arc<Mutex<Vec<HostEvent>>>,
}

pub fn spawn_fake(scenario: &str, extra_env: &[(&str, &str)]) -> LiveSidecar {
    let mut cmd = Command::new(&fake_pi());
    cmd.arg("--mode")
        .arg("rpc")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("FAKE_PI_SCENARIO", scenario)
        .env("FAKE_PI_GOLDEN_DIR", golden_dir());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().expect("spawn fake-pi");
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let ev2 = events.clone();
    let pending = Arc::new(Mutex::new(PendingMap::new()));
    let ui = Arc::new(Mutex::new(UiBridge::new()));
    let oversize = Arc::new(Mutex::new(false));
    spawn_stdout_reader(stdout, pending.clone(), ui.clone(), oversize.clone(), move |e| {
        ev2.lock().unwrap().push(e);
    });
    let rpc = RpcSession {
        stdin: Arc::new(Mutex::new(Box::new(stdin))),
        pending,
        ui,
        timeouts: Timeouts {
            default: Duration::from_secs(3),
        },
        oversize,
    };
    LiveSidecar { child, rpc, events }
}

pub fn event_types(events: &Arc<Mutex<Vec<HostEvent>>>) -> Vec<String> {
    events
        .lock()
        .unwrap()
        .iter()
        .map(|e| match e {
            HostEvent::Rpc { event } => format!(
                "rpc:{}",
                event.get("type").and_then(|v| v.as_str()).unwrap_or("?")
            ),
            HostEvent::UiRequest { request } => format!(
                "ui_request:{}",
                request.get("method").and_then(|v| v.as_str()).unwrap_or("?")
            ),
            HostEvent::Process { status, .. } => format!("process:{status:?}"),
            HostEvent::Log { level, .. } => format!("log:{level}"),
            HostEvent::Watchdog { silence_ms } => format!("watchdog:{silence_ms}"),
        })
        .collect()
}
