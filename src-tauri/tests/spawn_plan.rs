mod common;

use deskpi_lib::config::{
    build_pi_args, build_spawn_env, build_spawn_plan, resolve_pi_bin, AppPaths, Settings,
    PI_INSTALL_HINT,
};
use deskpi_lib::events::HostEvent;
use deskpi_lib::sidecar::{kill_group, process_alive, process_group_alive, Sidecar, SIGKILL_AFTER};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn runtime() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agent-runtime")
}

#[test]
fn spawn_args_env_and_pi_bin_override() {
    let args = build_pi_args(
        std::path::Path::new("/tmp/sessions"),
        runtime().as_path(),
        false,
    );
    assert!(args.windows(2).any(|w| w == ["--mode", "rpc"]));
    assert_eq!(args.iter().filter(|a| *a == "-e").count(), 3);
    assert_eq!(args.iter().filter(|a| *a == "--skill").count(), 2);
    assert!(args.iter().any(|a| *a == "--no-extensions"));
    assert!(args.iter().any(|a| *a == "--no-skills"));
    assert!(args.iter().any(|a| *a == "--no-prompt-templates"));
    assert!(args.iter().any(|a| a.contains("workspace-roots.ts")));
    assert!(args.iter().any(|a| a.contains("permission-gate.ts")));
    assert!(args.iter().any(|a| a.contains("protected-paths.ts")));
    assert!(args.iter().any(|a| a.contains("recap-dossier")));
    assert!(args.iter().any(|a| a.contains("trouver-dossier")));
    assert!(args.iter().any(|a| a.contains("recap.md")));
    assert!(!args.iter().any(|a| *a == "--approve"));

    let missing = resolve_pi_bin(None, Some("/no/such/bin"), &[]);
    assert_eq!(missing.unwrap_err(), PI_INSTALL_HINT);

    let tmp = tempfile::tempdir().unwrap();
    let app_data = tmp.path().join("data");
    let cwd = tmp.path().join("ws");
    fs::create_dir_all(&cwd).unwrap();
    let paths = AppPaths::from_app_data(app_data, runtime());
    paths.ensure_dirs().unwrap();
    let mut settings = Settings::default();
    settings.active_root = Some(cwd.clone());
    let mut host = BTreeMap::new();
    host.insert("HOME".into(), "/home/me".into());
    host.insert("USER".into(), "me".into());
    host.insert("PATH".into(), "/usr/bin".into());
    host.insert("LANG".into(), "C".into());
    host.insert("PI_BIN".into(), common::fake_pi().display().to_string());
    host.insert("IGNORED".into(), "nope".into());
    let secrets = BTreeMap::new();
    let plan = build_spawn_plan(&paths, &settings, &secrets, &host, None).unwrap();
    assert_eq!(plan.program, common::fake_pi());
    assert_eq!(plan.cwd, cwd);
    assert_eq!(plan.env.get("TERM").unwrap(), "dumb");
    assert_eq!(plan.env.get("NO_COLOR").unwrap(), "1");
    assert_eq!(plan.env.get("DESKPI_PERMISSION_MODE").unwrap(), "ask");
    assert!(plan.env.contains_key("DESKPI_WORKSPACE_FILE"));
    assert!(!plan.env.contains_key("IGNORED"));
    assert!(!plan.args.contains(&"--approve".into()));
    assert_eq!(
        plan.env.get(deskpi_lib::config::PI_AGENT_DIR_ENV).unwrap(),
        &paths.pi_agent_dir.display().to_string()
    );
    assert_eq!(plan.env.get("HOME").unwrap(), "/home/me");

    let env = build_spawn_env(
        &host,
        &secrets,
        &paths.workspace_file,
        "ask",
    );
    assert_eq!(env.get("HOME").unwrap(), "/home/me");
}

#[test]
fn resolve_pi_bin_prefers_bundled_then_path() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = tmp.path().join("agent-runtime");
    let bundled = runtime.join("vendor/pi/bin/pi");
    fs::create_dir_all(bundled.parent().unwrap()).unwrap();
    fs::write(&bundled, b"#!/bin/sh\n").unwrap();
    let path_dir = tmp.path().join("bin");
    fs::create_dir_all(&path_dir).unwrap();
    fs::write(path_dir.join("pi"), b"#!/bin/sh\n").unwrap();
    let found = resolve_pi_bin(None, Some(path_dir.to_str().unwrap()), &[runtime]).unwrap();
    assert_eq!(found, bundled);
}

#[test]
fn start_stop_restart_kills_process_group_and_changes_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let app_data = tmp.path().join("data");
    let cwd1 = tmp.path().join("ws1");
    let cwd2 = tmp.path().join("ws2");
    fs::create_dir_all(&cwd1).unwrap();
    fs::create_dir_all(&cwd2).unwrap();
    let paths = AppPaths::from_app_data(app_data, runtime());
    paths.ensure_dirs().unwrap();
    deskpi_lib::config::write_workspace_file(&paths, &{
        let mut s = Settings::default();
        s.active_root = Some(cwd1.clone());
        s.roots = vec![cwd1.clone()];
        s
    })
    .unwrap();

    let events = Arc::new(Mutex::new(Vec::<HostEvent>::new()));
    let sink_events = events.clone();
    let sink = Arc::new(move |e: HostEvent| {
        sink_events.lock().unwrap().push(e);
    });
    let mut sidecar = Sidecar::new(paths.clone(), sink).unwrap();
    sidecar.timeouts.default = Duration::from_secs(2);
    sidecar.settings.active_root = Some(cwd1.clone());
    sidecar.settings.roots = vec![cwd1.clone()];

    let state_file = tmp.path().join("fake-state.json");
    let mut host = BTreeMap::new();
    host.insert("HOME".into(), std::env::var("HOME").unwrap_or_else(|_| "/root".into()));
    host.insert("USER".into(), "root".into());
    host.insert("PATH".into(), std::env::var("PATH").unwrap());
    host.insert("LANG".into(), "C".into());
    let secrets = BTreeMap::new();
    let mut plan = build_spawn_plan(
        &paths,
        &sidecar.settings,
        &secrets,
        &host,
        Some(common::fake_pi().to_str().unwrap()),
    )
    .unwrap();
    plan.env.insert(
        "DESKPI_FAKE_STATE".into(),
        state_file.display().to_string(),
    );

    sidecar.spawn_plan(plan.clone()).expect("spawn 1");
    let pid1 = sidecar.pgid.unwrap() as u32;
    assert!(process_alive(pid1));
    let st = sidecar.send("get_state", json!({})).unwrap();
    assert_eq!(st.success, Some(true));

    sidecar.stop();
    assert!(
        common::wait_until(Duration::from_secs(3), || !process_alive(pid1)),
        "child must be gone after stop"
    );

    sidecar.settings.active_root = Some(cwd2.clone());
    plan.cwd = cwd2.clone();
    sidecar.spawn_plan(plan).expect("spawn 2");
    let pid2 = sidecar.pgid.unwrap() as u32;
    assert_ne!(pid1, pid2);
    assert!(process_alive(pid2));
    let raw = fs::read_to_string(&state_file).unwrap();
    assert!(raw.contains(&cwd2.display().to_string()), "{raw}");
    sidecar.stop();
    assert!(
        common::wait_until(Duration::from_secs(3), || !process_alive(pid2)),
        "restarted child must be gone after stop"
    );

    let _ = SIGKILL_AFTER;
    let _ = kill_group;
    let _ = process_group_alive;
}

/// Phase 4: killing the app during a bash must not leave a grandchild `sleep`.
#[test]
fn kill_group_reaps_bash_grandchild() {
    let mut cmd = Command::new("bash");
    cmd.args(["-c", "sleep 120 & echo $!; wait"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn().expect("spawn bash sleep group");
    let pgid = child.id() as i32;
    let mut buf = [0u8; 64];
    let n = child
        .stdout
        .as_mut()
        .map(|o| o.read(&mut buf).unwrap_or(0))
        .unwrap_or(0);
    let grandchild: Option<u32> = String::from_utf8_lossy(&buf[..n])
        .trim()
        .lines()
        .next()
        .and_then(|s| s.parse().ok());
    assert!(process_alive(pgid as u32), "bash leader must be alive");
    let g = grandchild.expect("bash must print grandchild pid");
    assert!(process_alive(g), "grandchild sleep must be alive before kill");
    assert!(process_group_alive(pgid), "process group must be alive");

    kill_group(pgid, Duration::from_secs(2));
    let _ = child.wait();
    assert!(
        common::wait_until(Duration::from_secs(2), || !process_alive(pgid as u32)),
        "leader must die"
    );
    assert!(
        common::wait_until(Duration::from_secs(2), || !process_alive(g)),
        "grandchild sleep must die with the group (no orphan)"
    );
    assert!(
        !process_group_alive(pgid),
        "process group must be gone after kill_group"
    );
}
