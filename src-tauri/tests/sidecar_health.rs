mod common;

use deskpi_lib::config::{build_spawn_plan, AppPaths};
use deskpi_lib::events::HostEvent;
use deskpi_lib::sidecar::{process_alive, Sidecar};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn runtime() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agent-runtime")
}

fn host_env() -> BTreeMap<String, String> {
    let mut host = BTreeMap::new();
    host.insert(
        "HOME".into(),
        std::env::var("HOME").unwrap_or_else(|_| "/root".into()),
    );
    host.insert("USER".into(), "root".into());
    host.insert("PATH".into(), std::env::var("PATH").unwrap());
    host.insert("LANG".into(), "C".into());
    host
}

fn collect_sink() -> (deskpi_lib::sidecar::EventSink, Arc<Mutex<Vec<HostEvent>>>) {
    let events = Arc::new(Mutex::new(Vec::<HostEvent>::new()));
    let sink_events = events.clone();
    let sink = Arc::new(move |e: HostEvent| {
        sink_events.lock().unwrap().push(e);
    });
    (sink, events)
}

#[test]
fn silence_watchdog_emits_once_while_streaming() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = AppPaths::from_app_data(tmp.path().join("data"), runtime());
    paths.ensure_dirs().unwrap();
    let (sink, events) = collect_sink();
    let mut sidecar = Sidecar::new(paths, sink).unwrap();
    sidecar.silence_timeout = Duration::from_millis(40);
    sidecar.running = true;
    {
        let mut last = sidecar.last_event.lock().unwrap();
        *last = Instant::now() - Duration::from_millis(80);
    }
    sidecar.maybe_emit_watchdog();
    sidecar.maybe_emit_watchdog();
    let n = events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, HostEvent::Watchdog { .. }))
        .count();
    assert_eq!(n, 1, "watchdog must fire once until a new event arrives");
}

#[test]
fn crash_poll_emits_crashed_without_get_state() {
    let tmp = tempfile::tempdir().unwrap();
    let app_data = tmp.path().join("data");
    let cwd = tmp.path().join("ws");
    fs::create_dir_all(&cwd).unwrap();
    let paths = AppPaths::from_app_data(app_data, runtime());
    paths.ensure_dirs().unwrap();
    let (sink, events) = collect_sink();
    let mut sidecar = Sidecar::new(paths.clone(), sink).unwrap();
    sidecar.settings.active_root = Some(cwd.clone());
    sidecar.settings.roots = vec![cwd.clone()];
    let mut plan = build_spawn_plan(
        &paths,
        &sidecar.settings,
        &BTreeMap::new(),
        &host_env(),
        Some(common::fake_pi().to_str().unwrap()),
    )
    .unwrap();
    plan.env.insert(
        "DESKPI_FAKE_STATE".into(),
        tmp.path().join("state.json").display().to_string(),
    );
    sidecar.spawn_plan(plan).expect("spawn");
    let pid = sidecar.pgid.unwrap() as u32;
    assert!(process_alive(pid));
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    assert!(
        common::wait_until(Duration::from_secs(3), || {
            sidecar.health_tick();
            events.lock().unwrap().iter().any(|e| {
                matches!(
                    e,
                    HostEvent::Process {
                        status: deskpi_lib::events::ProcessStatus::Crashed,
                        ..
                    }
                )
            })
        }),
        "health_tick must emit crashed after SIGKILL"
    );
}

#[test]
fn rpc_wait_does_not_hold_sidecar_mutex() {
    let tmp = tempfile::tempdir().unwrap();
    let app_data = tmp.path().join("data");
    let cwd = tmp.path().join("ws");
    fs::create_dir_all(&cwd).unwrap();
    let paths = AppPaths::from_app_data(app_data, runtime());
    paths.ensure_dirs().unwrap();
    let (sink, _events) = collect_sink();
    let mut sidecar = Sidecar::new(paths.clone(), sink).unwrap();
    sidecar.settings.active_root = Some(cwd.clone());
    sidecar.settings.roots = vec![cwd.clone()];
    let mut plan = build_spawn_plan(
        &paths,
        &sidecar.settings,
        &BTreeMap::new(),
        &host_env(),
        Some(common::fake_pi().to_str().unwrap()),
    )
    .unwrap();
    plan.env.insert("FAKE_PI_HANG".into(), "1".into());
    sidecar.spawn_plan(plan).expect("spawn");
    let rpc = sidecar.rpc_clone().expect("rpc");
    let waiter = thread::spawn(move || {
        rpc.send_command_timeout(
            "get_available_models",
            json!({}),
            Some(Duration::from_secs(2)),
        )
    });
    thread::sleep(Duration::from_millis(80));
    let started = Instant::now();
    sidecar.stop();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "stop must not wait on the in-flight RPC timeout (mutex was held across send)"
    );
    let _ = waiter.join();
}

#[test]
fn stop_and_respawn_does_not_replay_a_prompt() {
    let tmp = tempfile::tempdir().unwrap();
    let app_data = tmp.path().join("data");
    let cwd = tmp.path().join("ws");
    fs::create_dir_all(&cwd).unwrap();
    let paths = AppPaths::from_app_data(app_data, runtime());
    paths.ensure_dirs().unwrap();
    let (sink, events) = collect_sink();
    let mut sidecar = Sidecar::new(paths.clone(), sink).unwrap();
    sidecar.settings.active_root = Some(cwd.clone());
    sidecar.settings.roots = vec![cwd.clone()];
    let mut plan = build_spawn_plan(
        &paths,
        &sidecar.settings,
        &BTreeMap::new(),
        &host_env(),
        Some(common::fake_pi().to_str().unwrap()),
    )
    .unwrap();
    plan.env.insert(
        "DESKPI_FAKE_STATE".into(),
        tmp.path().join("state.json").display().to_string(),
    );
    sidecar.spawn_plan(plan.clone()).expect("spawn 1");
    sidecar.stop();
    sidecar.spawn_plan(plan).expect("spawn 2");
    let kinds: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            HostEvent::Rpc { event } => {
                event.get("type").and_then(|v| v.as_str()).map(str::to_string)
            }
            _ => None,
        })
        .collect();
    assert!(
        !kinds.iter().any(|t| t == "prompt"),
        "respawn must not replay a prompt: {kinds:?}"
    );
    sidecar.stop();
}
