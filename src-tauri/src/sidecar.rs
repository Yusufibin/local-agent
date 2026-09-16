//! Spawn / kill / restart the Pi RPC sidecar in its own process group.

use crate::bridge::UiBridge;
use crate::config::{
    build_spawn_plan, current_host_env, load_secrets, load_settings, write_workspace_file,
    AppPaths, Settings, SpawnPlan, PI_INSTALL_HINT,
};
use crate::error::{HostError, HostResult};
use crate::events::{HostEvent, ProcessStatus};
use crate::logs::{rotate_if_needed, LOG_BACKUPS, MAX_LOG_BYTES};
use crate::rpc::{RpcResponse, RpcSession, Timeouts};
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
pub const SIGKILL_AFTER: Duration = Duration::from_secs(2);
/// Plan §9.2: streaming with no event for this long → silence banner, no auto-kill.
pub const SILENCE_WATCHDOG: Duration = Duration::from_secs(10 * 60);
const READER_JOIN: Duration = Duration::from_secs(2);

pub type EventSink = Arc<dyn Fn(HostEvent) + Send + Sync>;

pub struct Sidecar {
    pub paths: AppPaths,
    pub settings: Settings,
    pub child: Option<Child>,
    pub pgid: Option<i32>,
    pub rpc: Option<RpcSession>,
    pub running: bool,
    pub running_flag: Arc<Mutex<bool>>,
    pub last_missing_pi: Option<String>,
    pub last_crash: Option<String>,
    pub sink: EventSink,
    pub timeouts: Timeouts,
    pub oversize: Arc<Mutex<bool>>,
    pub reader: Option<thread::JoinHandle<()>>,
    pub last_event: Arc<Mutex<Instant>>,
    pub watchdog_emitted: Arc<Mutex<bool>>,
    pub silence_timeout: Duration,
    stopping: bool,
}

impl Sidecar {
    pub fn new(paths: AppPaths, sink: EventSink) -> HostResult<Self> {
        paths.ensure_dirs()?;
        let _ = crate::config::seed_pi_home(&paths, &current_host_env());
        let settings = load_settings(&paths.settings_file, &paths.agent_runtime)?;
        if settings.active_root.is_some() && !paths.settings_file.exists() {
            let _ = crate::config::save_settings(&paths.settings_file, &settings);
        }
        Ok(Self {
            paths,
            settings,
            child: None,
            pgid: None,
            rpc: None,
            running: false,
            running_flag: Arc::new(Mutex::new(false)),
            last_missing_pi: None,
            last_crash: None,
            sink,
            timeouts: Timeouts::default(),
            oversize: Arc::new(Mutex::new(false)),
            reader: None,
            last_event: Arc::new(Mutex::new(Instant::now())),
            watchdog_emitted: Arc::new(Mutex::new(false)),
            silence_timeout: SILENCE_WATCHDOG,
            stopping: false,
        })
    }

    pub fn reload_settings(&mut self) -> HostResult<()> {
        self.settings = load_settings(&self.paths.settings_file, &self.paths.agent_runtime)?;
        Ok(())
    }

    pub fn persist_settings(&self) -> HostResult<()> {
        crate::config::save_settings(&self.paths.settings_file, &self.settings)
    }

    pub fn is_alive(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code();
                    self.child = None;
                    self.rpc = None;
                    let was_running = self.running;
                    self.running = false;
                    if let Ok(mut g) = self.running_flag.lock() {
                        *g = false;
                    }
                    let expected_stop = self.stopping;
                    if !expected_stop && (code != Some(0) || was_running) {
                        let tail = stderr_tail(&self.paths.stderr_log, 4000);
                        self.last_crash = Some(tail.clone());
                        (self.sink)(HostEvent::process_msg(
                            ProcessStatus::Crashed,
                            code,
                            tail.clone(),
                        ));
                        (self.sink)(HostEvent::log(
                            "error",
                            format!("sidecar exited {:?}: {tail}", code),
                        ));
                    } else if !expected_stop {
                        (self.sink)(HostEvent::process(ProcessStatus::Exited, code));
                    }
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub fn start(&mut self) -> HostResult<Value> {
        if self.is_alive() {
            return self.get_state();
        }
        self.reload_settings()?;
        if !crate::config::is_agent_runtime_dir(&self.paths.agent_runtime) {
            return Err(HostError::from(format!(
                "agent-runtime not found at {} (set DESKPI_RUNTIME)",
                self.paths.agent_runtime.display()
            )));
        }
        if self.settings.active_root.is_none() {
            return Err(HostError::from(
                "pick a workspace folder before starting the agent",
            ));
        }
        write_workspace_file(&self.paths, &self.settings)?;
        let secrets = load_secrets(&self.paths.secrets_file)?;
        let host_env = current_host_env();
        let _ = crate::config::seed_pi_home(&self.paths, &host_env);
        let plan = match build_spawn_plan(
            &self.paths,
            &self.settings,
            &secrets,
            &host_env,
            None,
        ) {
            Ok(p) => p,
            Err(msg) => {
                self.last_missing_pi = Some(msg.clone());
                (self.sink)(HostEvent::log("error", msg.clone()));
                return Err(HostError::from(msg));
            }
        };
        self.spawn_plan(plan)
    }

    pub fn spawn_plan(&mut self, plan: SpawnPlan) -> HostResult<Value> {
        self.paths.ensure_dirs()?;
        let _ = rotate_if_needed(&plan.stderr_log, MAX_LOG_BYTES, LOG_BACKUPS);
        let stderr_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&plan.stderr_log)?;

        let mut cmd = Command::new(&plan.program);
        cmd.args(&plan.args)
            .current_dir(&plan.cwd)
            .env_clear()
            .envs(&plan.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr_file));
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let msg = if e.kind() == std::io::ErrorKind::NotFound {
                    PI_INSTALL_HINT.to_string()
                } else {
                    format!("failed to spawn pi: {e}")
                };
                self.last_missing_pi = Some(msg.clone());
                (self.sink)(HostEvent::log("error", msg.clone()));
                return Err(HostError::from(msg));
            }
        };

        let pid = child.id() as i32;
        let stdin = child.stdin.take().ok_or_else(|| HostError::from("no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HostError::from("no stdout"))?;

        let pending = Arc::new(Mutex::new(crate::rpc::PendingMap::new()));
        let ui = Arc::new(Mutex::new(UiBridge::new()));
        let oversize = Arc::new(Mutex::new(false));
        self.oversize = oversize.clone();
        let sink = self.sink.clone();
        let running_flag = Arc::new(Mutex::new(false));
        let running_flag_reader = running_flag.clone();
        let last_event = self.last_event.clone();
        let watchdog_emitted = self.watchdog_emitted.clone();
        if let Ok(mut g) = last_event.lock() {
            *g = Instant::now();
        }
        if let Ok(mut g) = watchdog_emitted.lock() {
            *g = false;
        }
        let reader = crate::rpc::spawn_stdout_reader(
            stdout,
            pending.clone(),
            ui.clone(),
            oversize.clone(),
            move |ev| {
                match &ev {
                    HostEvent::Rpc { event } => {
                        match event.get("type").and_then(|v| v.as_str()) {
                            Some("agent_start") => {
                                if let Ok(mut g) = running_flag_reader.lock() {
                                    *g = true;
                                }
                            }
                            Some("agent_settled") => {
                                if let Ok(mut g) = running_flag_reader.lock() {
                                    *g = false;
                                }
                            }
                            _ => {}
                        }
                        if let Ok(mut g) = last_event.lock() {
                            *g = Instant::now();
                        }
                        if let Ok(mut g) = watchdog_emitted.lock() {
                            *g = false;
                        }
                    }
                    HostEvent::UiRequest { .. } => {
                        if let Ok(mut g) = last_event.lock() {
                            *g = Instant::now();
                        }
                        if let Ok(mut g) = watchdog_emitted.lock() {
                            *g = false;
                        }
                    }
                    _ => {}
                }
                sink(ev);
            },
        );
        self.running_flag = running_flag;

        self.rpc = Some(RpcSession {
            stdin: Arc::new(Mutex::new(Box::new(stdin))),
            pending,
            ui,
            timeouts: self.timeouts.clone(),
            oversize: self.oversize.clone(),
        });
        self.child = Some(child);
        self.pgid = Some(pid);
        self.last_missing_pi = None;
        self.last_crash = None;
        (self.sink)(HostEvent::process(ProcessStatus::Spawned, None));

        match self.get_state_with_timeout(HEALTH_TIMEOUT) {
            Ok(state) => {
                self.running = false;
                self.reader = Some(reader);
                Ok(state)
            }
            Err(e) => {
                let tail = stderr_tail(&plan.stderr_log, 4000);
                self.stop_process();
                let msg = format!("sidecar health check failed: {e}; stderr: {tail}");
                self.last_crash = Some(msg.clone());
                (self.sink)(HostEvent::process_msg(ProcessStatus::Crashed, None, msg.clone()));
                (self.sink)(HostEvent::log("error", msg.clone()));
                Err(HostError::from(msg))
            }
        }
    }

    pub fn stop(&mut self) {
        self.stop_process();
        self.running = false;
        if let Ok(mut g) = self.running_flag.lock() {
            *g = false;
        }
        self.rpc = None;
    }

    fn stop_process(&mut self) {
        self.stopping = true;
        if let Some(pid) = self.pgid.take() {
            kill_group(pid, SIGKILL_AFTER);
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
        self.rpc = None;
        if let Some(handle) = self.reader.take() {
            join_reader(handle, READER_JOIN);
        }
        self.stopping = false;
    }

    pub fn restart(&mut self) -> HostResult<Value> {
        self.stop();
        self.last_crash = None;
        self.start()
    }

    pub fn rpc(&self) -> HostResult<&RpcSession> {
        self.rpc
            .as_ref()
            .ok_or_else(|| HostError::from("sidecar is not running"))
    }

    pub fn rpc_clone(&self) -> HostResult<RpcSession> {
        self.rpc
            .clone()
            .ok_or_else(|| HostError::from("sidecar is not running"))
    }

    pub fn send(&self, command: &str, body: Value) -> HostResult<RpcResponse> {
        let rpc = self.rpc()?;
        rpc.send_command(command, body).map_err(HostError::from)
    }

    pub fn get_state(&self) -> HostResult<Value> {
        let resp = self.send("get_state", json!({}))?;
        Ok(resp.data.unwrap_or(json!({})))
    }

    fn get_state_with_timeout(&self, timeout: Duration) -> HostResult<Value> {
        let rpc = self.rpc()?;
        match rpc.send_command_timeout("get_state", json!({}), Some(timeout)) {
            Ok(resp) if resp.success.unwrap_or(false) => Ok(resp.data.unwrap_or(json!({}))),
            Ok(resp) => Err(HostError::from(
                resp.error.unwrap_or_else(|| "get_state failed".into()),
            )),
            Err(e) => Err(HostError::from(e)),
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.running
            || self
                .running_flag
                .lock()
                .map(|g| *g)
                .unwrap_or(false)
    }

    pub fn take_oversize(&mut self) -> bool {
        let flag = *self.oversize.lock().unwrap_or_else(|e| e.into_inner());
        if flag {
            if let Ok(mut g) = self.oversize.lock() {
                *g = false;
            }
            true
        } else {
            false
        }
    }

    pub fn maybe_restart_on_oversize(&mut self) -> HostResult<()> {
        if self.take_oversize() {
            self.restart()?;
        }
        Ok(())
    }

    pub fn maybe_emit_watchdog(&mut self) {
        if !self.is_streaming() {
            if let Ok(mut g) = self.watchdog_emitted.lock() {
                *g = false;
            }
            return;
        }
        let elapsed = self
            .last_event
            .lock()
            .map(|g| g.elapsed())
            .unwrap_or_else(|_| Duration::ZERO);
        let already = self
            .watchdog_emitted
            .lock()
            .map(|g| *g)
            .unwrap_or(true);
        if elapsed >= self.silence_timeout && !already {
            if let Ok(mut g) = self.watchdog_emitted.lock() {
                *g = true;
            }
            let ms = elapsed.as_millis() as u64;
            (self.sink)(HostEvent::watchdog(ms));
            (self.sink)(HostEvent::log(
                "warn",
                format!("sidecar silent for {ms}ms (no auto-kill; use Restart)"),
            ));
        }
    }

    /// Poll child exit, oversize JSONL, and silence. Safe to call from a timer thread.
    pub fn health_tick(&mut self) {
        let _ = self.is_alive();
        if self.take_oversize() {
            let _ = self.restart();
        }
        self.maybe_emit_watchdog();
    }

    pub fn ui_respond(&self, id: &str, payload: Value) -> HostResult<()> {
        let rpc = self.rpc()?;
        let mut ui = rpc.ui.lock().map_err(|e| HostError::from(e.to_string()))?;
        if !ui.accept_response(id) {
            return Ok(());
        }
        drop(ui);
        let body = UiBridge::build_response(id, &payload);
        rpc.write_raw(&body).map_err(HostError::from)
    }

    pub fn abort_with_queue_restore(&self) -> HostResult<Value> {
        let clear = self.send("clear_queue", json!({}))?;
        let data = clear.data.clone().unwrap_or(json!({}));
        let _ = self.send("abort", json!({}))?;
        Ok(data)
    }
}

/// True if any process in the group still exists (`kill(-pgid, 0)`).
pub fn process_group_alive(pgid: i32) -> bool {
    #[cfg(unix)]
    unsafe {
        libc::kill(-pgid, 0) == 0
    }
    #[cfg(not(unix))]
    {
        let _ = pgid;
        false
    }
}

/// SIGTERM the process group, wait, then always SIGKILL the group.
/// Do not return early when only the leader is reaped — bash children must die too.
pub fn kill_group(pid: i32, wait: Duration) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    let _ = pid;
    let start = Instant::now();
    while start.elapsed() < wait {
        if !process_group_alive(pid) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
        let mut status = 0;
        libc::waitpid(pid, &mut status, libc::WNOHANG);
    }
}

pub fn stderr_tail(path: &std::path::Path, max: usize) -> String {
    let Ok(mut f) = File::open(path) else {
        return String::new();
    };
    let mut buf = String::new();
    let _ = f.read_to_string(&mut buf);
    if buf.len() <= max {
        buf
    } else {
        buf[buf.len() - max..].to_string()
    }
}

fn join_reader(handle: thread::JoinHandle<()>, wait: Duration) {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = handle.join();
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(wait);
}

pub fn spawn_health_monitor(sidecar: Arc<Mutex<Sidecar>>) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(500));
        match sidecar.lock() {
            Ok(mut sc) => sc.health_tick(),
            Err(_) => break,
        }
    })
}

pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, 0) == 0
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}
