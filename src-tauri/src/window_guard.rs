//! Keep the main window on screen after GTK dialogs, webview death, or unmap.

use crate::events::HostEvent;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

pub const MAIN_LABEL: &str = "main";
const MIN_INNER: u32 = 80;
const OFFSCREEN: i32 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestoreAction {
    None,
    Show,
    ResetGeometry,
    Recreate,
}

/// Pure classifier so we can test without a display.
pub fn classify_window(
    exists: bool,
    user_closing: bool,
    visible: Option<bool>,
    minimized: Option<bool>,
    width: Option<u32>,
    height: Option<u32>,
    x: Option<i32>,
    y: Option<i32>,
) -> RestoreAction {
    if user_closing {
        return RestoreAction::None;
    }
    if !exists {
        return RestoreAction::Recreate;
    }
    if let (Some(w), Some(h)) = (width, height) {
        if w < MIN_INNER || h < MIN_INNER {
            return RestoreAction::ResetGeometry;
        }
    }
    if let (Some(px), Some(py)) = (x, y) {
        if px.abs() > OFFSCREEN || py.abs() > OFFSCREEN {
            return RestoreAction::ResetGeometry;
        }
    }
    match (visible, minimized) {
        (Some(false), Some(true)) => RestoreAction::None,
        (Some(false), Some(false)) => RestoreAction::Show,
        (Some(false), None) => RestoreAction::None,
        _ => RestoreAction::None,
    }
}

pub fn restore_main_window(app: &AppHandle) {
    let Some(w) = app.get_webview_window(MAIN_LABEL) else {
        let _ = recreate_main_window(app);
        return;
    };
    let _ = w.show();
    let _ = w.unminimize();
    if let Ok(size) = w.inner_size() {
        if size.width < MIN_INNER || size.height < MIN_INNER {
            let _ = w.set_size(tauri::LogicalSize::new(1100.0, 720.0));
        }
    }
    if let Ok(pos) = w.outer_position() {
        if pos.x.abs() > OFFSCREEN || pos.y.abs() > OFFSCREEN {
            let _ = w.center();
        }
    }
    let _ = w.set_focus();
}

pub fn recreate_main_window(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(MAIN_LABEL).is_some() {
        restore_main_window(app);
        return Ok(());
    }
    let cfg = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_LABEL)
        .cloned()
        .ok_or_else(|| "no main window config".to_string())?;
    WebviewWindowBuilder::from_config(app, &cfg)
        .map_err(|e| e.to_string())?
        .visible(true)
        .build()
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "agent://event",
        &HostEvent::log("warn", "main window recreated after it disappeared"),
    );
    Ok(())
}

pub fn ensure_main_window(app: &AppHandle, user_closing: bool) {
    if user_closing {
        return;
    }
    let exists = app.get_webview_window(MAIN_LABEL);
    let action = match &exists {
        None => classify_window(false, user_closing, None, None, None, None, None, None),
        Some(w) => {
            let visible = w.is_visible().ok();
            let minimized = w.is_minimized().ok();
            let (width, height) = w
                .inner_size()
                .ok()
                .map(|s| (Some(s.width), Some(s.height)))
                .unwrap_or((None, None));
            let (x, y) = w
                .outer_position()
                .ok()
                .map(|p| (Some(p.x), Some(p.y)))
                .unwrap_or((None, None));
            classify_window(
                true,
                user_closing,
                visible,
                minimized,
                width,
                height,
                x,
                y,
            )
        }
    };
    match action {
        RestoreAction::None => {}
        RestoreAction::Show | RestoreAction::ResetGeometry => {
            let _ = app.emit(
                "agent://event",
                &HostEvent::log("warn", "main window was not visible; restored"),
            );
            restore_main_window(app);
        }
        RestoreAction::Recreate => {
            let _ = recreate_main_window(app);
        }
    }
}

pub fn schedule_on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || f(&app));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_closing_never_restores() {
        assert_eq!(
            classify_window(false, true, Some(false), Some(false), Some(10), Some(10), Some(0), Some(0)),
            RestoreAction::None
        );
    }

    #[test]
    fn missing_window_recreates() {
        assert_eq!(
            classify_window(false, false, None, None, None, None, None, None),
            RestoreAction::Recreate
        );
    }

    #[test]
    fn minimized_is_left_alone() {
        assert_eq!(
            classify_window(true, false, Some(false), Some(true), Some(1100), Some(720), Some(80), Some(80)),
            RestoreAction::None
        );
    }

    #[test]
    fn unmapped_not_minimized_is_shown() {
        assert_eq!(
            classify_window(true, false, Some(false), Some(false), Some(1100), Some(720), Some(80), Some(80)),
            RestoreAction::Show
        );
    }

    #[test]
    fn linux_unknown_minimized_does_not_steal_focus() {
        assert_eq!(
            classify_window(true, false, Some(false), None, Some(1100), Some(720), Some(80), Some(80)),
            RestoreAction::None
        );
    }

    #[test]
    fn tiny_or_offscreen_resets_geometry() {
        assert_eq!(
            classify_window(true, false, Some(true), Some(false), Some(0), Some(0), Some(10), Some(10)),
            RestoreAction::ResetGeometry
        );
        assert_eq!(
            classify_window(true, false, Some(true), Some(false), Some(1100), Some(720), Some(-50_000), Some(10)),
            RestoreAction::ResetGeometry
        );
    }

    #[test]
    fn healthy_visible_window_is_idle() {
        assert_eq!(
            classify_window(true, false, Some(true), Some(false), Some(1100), Some(720), Some(80), Some(80)),
            RestoreAction::None
        );
    }
}
