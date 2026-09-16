pub mod bridge;
pub mod commands;
pub mod config;
pub mod error;
pub mod events;
pub mod jsonl;
pub mod logs;
pub mod rpc;
pub mod sessions;
pub mod sidecar;
pub mod window_guard;

use commands::{persist_window_close, AppState};
use config::{current_host_env, resolve_agent_runtime, AppPaths};
use logs::HostLogger;
use sidecar::Sidecar;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{Manager, RunEvent, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let host_env = current_host_env();
            let runtime = resolve_agent_runtime(
                app.path().resource_dir().ok().as_deref(),
                &host_env,
                std::env::current_exe().ok().as_deref(),
                std::env::current_dir().ok().as_deref(),
            )
            .unwrap_or_else(|_| PathBuf::from("agent-runtime"));
            let paths = AppPaths::from_app_data(app_data, runtime);
            paths.ensure_dirs().map_err(|e| e.to_string())?;
            let logger = HostLogger::new(paths.host_log.clone());
            logger.append("info", "host starting");
            let sink = commands::make_sink(app.handle().clone(), Some(logger));
            let sidecar = Sidecar::new(paths, sink).map_err(|e| e.to_string())?;
            let sidecar = Arc::new(Mutex::new(sidecar));
            sidecar::spawn_health_monitor(sidecar.clone());
            app.manage(AppState {
                sidecar,
                user_closing: AtomicBool::new(false),
            });
            spawn_window_watch(app.handle().clone());
            window_guard::schedule_on_main(app.handle(), |a| {
                window_guard::restore_main_window(a);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agent_start,
            commands::agent_stop,
            commands::agent_restart,
            commands::prompt,
            commands::steer,
            commands::follow_up,
            commands::abort,
            commands::new_session,
            commands::switch_session,
            commands::list_sessions,
            commands::get_state,
            commands::get_messages,
            commands::set_model,
            commands::set_thinking_level,
            commands::get_available_models,
            commands::get_session_stats,
            commands::compact,
            commands::get_commands,
            commands::ui_respond,
            commands::pick_workspace,
            commands::save_secret,
            commands::get_settings,
            commands::set_permission_mode,
            commands::add_root,
            commands::mark_running,
            commands::install_hint,
        ])
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { .. } => {
                if let Some(state) = window.try_state::<AppState>() {
                    persist_window_close(&state);
                }
            }
            WindowEvent::Destroyed => {
                let app = window.app_handle().clone();
                let user_closing = app
                    .try_state::<AppState>()
                    .map(|s| s.user_closing.load(Ordering::SeqCst))
                    .unwrap_or(false);
                if user_closing {
                    app.exit(0);
                    return;
                }
                if window.label() != window_guard::MAIN_LABEL {
                    return;
                }
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(150));
                    if app
                        .try_state::<AppState>()
                        .map(|s| s.user_closing.load(Ordering::SeqCst))
                        .unwrap_or(false)
                    {
                        return;
                    }
                    window_guard::schedule_on_main(&app, |a| {
                        let _ = window_guard::recreate_main_window(a);
                    });
                });
            }
            _ => {}
        });

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| {
        if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
            if let Some(state) = app.try_state::<AppState>() {
                persist_window_close(&state);
            }
        }
    });
}

fn spawn_window_watch(app: tauri::AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));
        let closing = app
            .try_state::<AppState>()
            .map(|s| s.user_closing.load(Ordering::SeqCst))
            .unwrap_or(false);
        if closing {
            break;
        }
        window_guard::schedule_on_main(&app, move |a| {
            let closing = a
                .try_state::<AppState>()
                .map(|s| s.user_closing.load(Ordering::SeqCst))
                .unwrap_or(false);
            window_guard::ensure_main_window(a, closing);
        });
    });
}
