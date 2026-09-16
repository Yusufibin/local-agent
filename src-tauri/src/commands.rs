use crate::config::{
    load_secrets, provider_to_env_key, save_secrets, save_settings, Settings,
};
use crate::error::HostResult;
use crate::events::HostEvent;
use crate::logs::HostLogger;
use crate::rpc::RpcSession;
use crate::sessions::list_sessions as scan_session_dir;
use crate::sidecar::Sidecar;
use crate::window_guard;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

pub const MAX_UI_MESSAGES: usize = 200;

pub struct AppState {
    pub sidecar: Arc<Mutex<Sidecar>>,
    pub user_closing: AtomicBool,
}

fn emit_event(app: &AppHandle, event: HostEvent) {
    let _ = app.emit("agent://event", &event);
    if let HostEvent::UiRequest { request } = &event {
        let _ = app.emit("agent://ui_request", request);
        if let Some("setTitle") = request.get("method").and_then(|v| v.as_str()) {
            if let Some(title) = request.get("title").and_then(|v| v.as_str()) {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_title(title);
                }
            }
        }
        if request.get("method").and_then(|v| v.as_str()) == Some("notify") {
            let msg = request
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let _ = app
                .notification()
                .builder()
                .title("Local Agent")
                .body(msg)
                .show();
        }
    }
}

pub fn make_sink(app: AppHandle, logger: Option<HostLogger>) -> crate::sidecar::EventSink {
    std::sync::Arc::new(move |event: HostEvent| {
        if let Some(log) = &logger {
            log.log_event(&event);
        }
        emit_event(&app, event);
    })
}

fn lock_sidecar(state: &AppState) -> HostResult<std::sync::MutexGuard<'_, Sidecar>> {
    state.sidecar.lock().map_err(|e| e.to_string().into())
}

/// Clone the RPC handle and drop the sidecar mutex before waiting on Pi.
fn rpc_session(state: &AppState) -> HostResult<RpcSession> {
    lock_sidecar(state)?.rpc_clone()
}

fn cap_messages(mut data: Value) -> Value {
    if let Some(arr) = data.get_mut("messages").and_then(|v| v.as_array_mut()) {
        if arr.len() > MAX_UI_MESSAGES {
            let skip = arr.len() - MAX_UI_MESSAGES;
            arr.drain(0..skip);
            data["truncatedFrom"] = json!(skip);
        }
    }
    data
}

fn pick_folder_blocking(app: &AppHandle) -> Option<PathBuf> {
    let mut dlg = app.dialog().file();
    if let Some(w) = app.get_webview_window("main") {
        dlg = dlg.set_parent(&w);
    }
    dlg.blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
}

fn send_rpc(state: &AppState, command: &str, body: Value) -> HostResult<crate::rpc::RpcResponse> {
    rpc_session(state)?
        .send_command(command, body)
        .map_err(|e| e.into())
}

#[tauri::command]
pub fn agent_start(state: State<AppState>) -> HostResult<Value> {
    lock_sidecar(&state)?.start()
}

#[tauri::command]
pub fn agent_stop(state: State<AppState>) -> HostResult<()> {
    lock_sidecar(&state)?.stop();
    Ok(())
}

#[tauri::command]
pub fn agent_restart(state: State<AppState>) -> HostResult<Value> {
    // Exclusive: kill the group, then spawn. Do not replay the last prompt.
    lock_sidecar(&state)?.restart()
}

#[tauri::command]
pub fn prompt(
    state: State<AppState>,
    text: String,
    images: Option<Value>,
    behavior: Option<String>,
) -> HostResult<Value> {
    let rpc = {
        let sc = lock_sidecar(&state)?;
        if sc.is_streaming() && behavior.is_none() {
            return Err("agent is streaming; use steer or follow_up".into());
        }
        sc.rpc_clone()?
    };
    let mut body = json!({ "message": text });
    if let Some(images) = images {
        body["images"] = images;
    }
    if let Some(behavior) = behavior {
        body["streamingBehavior"] = json!(behavior);
    }
    let resp = rpc.send_command("prompt", body).map_err(crate::error::HostError::from)?;
    Ok(json!({
        "success": resp.success,
        "error": resp.error,
        "data": resp.data,
    }))
}

#[tauri::command]
pub fn steer(state: State<AppState>, text: String) -> HostResult<Value> {
    let resp = send_rpc(&state, "steer", json!({ "message": text }))?;
    Ok(resp.data.unwrap_or(json!({ "success": resp.success })))
}

#[tauri::command]
pub fn follow_up(state: State<AppState>, text: String) -> HostResult<Value> {
    let resp = send_rpc(&state, "follow_up", json!({ "message": text }))?;
    Ok(resp.data.unwrap_or(json!({ "success": resp.success })))
}

#[tauri::command]
pub fn abort(state: State<AppState>) -> HostResult<Value> {
    let rpc = rpc_session(&state)?;
    let clear = rpc
        .send_command("clear_queue", json!({}))
        .map_err(crate::error::HostError::from)?;
    let data = clear.data.clone().unwrap_or(json!({}));
    let _ = rpc.send_command("abort", json!({}));
    Ok(data)
}

#[tauri::command]
pub fn new_session(state: State<AppState>) -> HostResult<Value> {
    let resp = send_rpc(&state, "new_session", json!({}))?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn switch_session(state: State<AppState>, path: String) -> HostResult<Value> {
    let resp = send_rpc(&state, "switch_session", json!({ "sessionPath": path }))?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn list_sessions(state: State<AppState>) -> HostResult<Value> {
    let sessions_dir = lock_sidecar(&state)?.paths.sessions_dir.clone();
    let list = scan_session_dir(&sessions_dir);
    Ok(serde_json::to_value(list)?)
}

#[tauri::command]
pub fn get_state(state: State<AppState>) -> HostResult<Value> {
    let rpc = {
        let mut sc = lock_sidecar(&state)?;
        if let Some(msg) = sc.last_missing_pi.clone() {
            return Ok(json!({ "missingPi": msg, "install": crate::config::PI_INSTALL_HINT }));
        }
        if !sc.is_alive() {
            if let Some(msg) = sc.last_crash.clone() {
                return Ok(json!({ "crashed": true, "stderr": msg }));
            }
            return Ok(json!({ "idle": true, "running": false }));
        }
        sc.rpc_clone()?
    };
    let resp = rpc
        .send_command("get_state", json!({}))
        .map_err(crate::error::HostError::from)?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn get_messages(state: State<AppState>) -> HostResult<Value> {
    let resp = send_rpc(&state, "get_messages", json!({}))?;
    Ok(cap_messages(resp.data.unwrap_or(json!({ "messages": [] }))))
}

#[tauri::command]
pub fn set_model(state: State<AppState>, provider: String, model_id: String) -> HostResult<Value> {
    let resp = send_rpc(
        &state,
        "set_model",
        json!({ "provider": provider, "modelId": model_id }),
    )?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn set_thinking_level(state: State<AppState>, level: String) -> HostResult<Value> {
    let resp = send_rpc(&state, "set_thinking_level", json!({ "level": level }))?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn get_available_models(state: State<AppState>) -> HostResult<Value> {
    let resp = send_rpc(&state, "get_available_models", json!({}))?;
    Ok(resp.data.unwrap_or(json!({ "models": [] })))
}

#[tauri::command]
pub fn get_session_stats(state: State<AppState>) -> HostResult<Value> {
    let resp = send_rpc(&state, "get_session_stats", json!({}))?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn compact(state: State<AppState>) -> HostResult<Value> {
    let resp = send_rpc(&state, "compact", json!({}))?;
    Ok(resp.data.unwrap_or(json!({})))
}

#[tauri::command]
pub fn get_commands(state: State<AppState>) -> HostResult<Value> {
    let resp = send_rpc(&state, "get_commands", json!({}))?;
    Ok(resp.data.unwrap_or(json!({ "commands": [] })))
}

#[tauri::command]
pub fn ui_respond(state: State<AppState>, id: String, payload: Value) -> HostResult<()> {
    let rpc = rpc_session(&state)?;
    let mut ui = rpc.ui.lock().map_err(|e| e.to_string())?;
    if !ui.accept_response(&id) {
        return Ok(());
    }
    drop(ui);
    let body = crate::bridge::UiBridge::build_response(&id, &payload);
    rpc.write_raw(&body).map_err(crate::error::HostError::from)
}

#[tauri::command]
pub async fn pick_workspace(app: AppHandle, state: State<'_, AppState>) -> HostResult<Value> {
    let app_dlg = app.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || pick_folder_blocking(&app_dlg))
        .await
        .map_err(|e| format!("dialog failed: {e}"))?;
    window_guard::schedule_on_main(&app, |a| window_guard::restore_main_window(a));
    let Some(path) = folder else {
        return Ok(json!({ "cancelled": true }));
    };
    let mut sc = lock_sidecar(&state)?;
    sc.settings.active_root = Some(path.clone());
    if !sc.settings.roots.iter().any(|r| r == &path) {
        sc.settings.roots.push(path.clone());
    }
    sc.persist_settings()?;
    let st = sc.restart()?;
    Ok(json!({ "activeRoot": path, "state": st }))
}

#[tauri::command]
pub fn save_secret(state: State<AppState>, provider: String, key: String) -> HostResult<Value> {
    let mut sc = lock_sidecar(&state)?;
    let mut secrets = load_secrets(&sc.paths.secrets_file)?;
    secrets.insert(provider_to_env_key(&provider), key);
    save_secrets(&sc.paths.secrets_file, &secrets)?;
    if sc.settings.active_root.is_some() {
        let st = sc.restart()?;
        return Ok(st);
    }
    Ok(json!({ "saved": true }))
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> HostResult<Settings> {
    Ok(lock_sidecar(&state)?.settings.clone())
}

#[tauri::command]
pub fn set_permission_mode(state: State<AppState>, mode: String) -> HostResult<Value> {
    if !matches!(mode.as_str(), "ask" | "full" | "readonly") {
        return Err("mode must be ask|full|readonly".into());
    }
    let mut sc = lock_sidecar(&state)?;
    sc.settings.permission_mode = mode;
    sc.persist_settings()?;
    if sc.settings.active_root.is_some() {
        return sc.restart();
    }
    Ok(json!({ "ok": true }))
}

#[tauri::command]
pub async fn add_root(app: AppHandle, state: State<'_, AppState>) -> HostResult<Value> {
    let app_dlg = app.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || pick_folder_blocking(&app_dlg))
        .await
        .map_err(|e| format!("dialog failed: {e}"))?;
    window_guard::schedule_on_main(&app, |a| window_guard::restore_main_window(a));
    let Some(path) = folder else {
        return Ok(json!({ "cancelled": true }));
    };
    let mut sc = lock_sidecar(&state)?;
    if !sc.settings.roots.iter().any(|r| r == &path) {
        sc.settings.roots.push(path.clone());
    }
    sc.persist_settings()?;
    if sc.settings.active_root.is_some() {
        let st = sc.restart()?;
        return Ok(json!({ "roots": sc.settings.roots, "state": st }));
    }
    Ok(json!({ "roots": sc.settings.roots }))
}

#[tauri::command]
pub fn mark_running(state: State<AppState>, running: bool) -> HostResult<()> {
    let mut sc = lock_sidecar(&state)?;
    sc.running = running;
    Ok(())
}

#[tauri::command]
pub fn install_hint() -> String {
    crate::config::PI_INSTALL_HINT.to_string()
}

pub fn on_rpc_event(state: &AppState, event: &HostEvent) {
    if let HostEvent::Rpc { event } = event {
        if let Some(t) = event.get("type").and_then(|v| v.as_str()) {
            if let Ok(mut sc) = state.sidecar.lock() {
                match t {
                    "agent_start" => sc.running = true,
                    "agent_settled" => sc.running = false,
                    _ => {}
                }
            }
        }
    }
}

pub fn persist_window_close(state: &AppState) {
    state
        .user_closing
        .store(true, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut sc) = state.sidecar.lock() {
        sc.stop();
    }
}

pub fn save_settings_direct(path: &std::path::Path, settings: &Settings) -> HostResult<()> {
    save_settings(path, settings)
}
