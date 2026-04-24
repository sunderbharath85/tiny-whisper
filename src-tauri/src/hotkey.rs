use crate::config::Settings;
use crate::state::AppState;
use anyhow::Result;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum AppStatus {
    Idle,
    Listening,
    Speaking,
    Transcribing,
    Error { message: String },
}

pub fn emit_status(app: &AppHandle, status: AppStatus) {
    let _ = app.emit("app://status", status);
}

pub fn register(app: &AppHandle, settings: &Settings) -> Result<()> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let shortcut: Shortcut = settings.hotkey.parse()?;
    let handle = app.clone();
    gs.on_shortcut(shortcut, move |app_handle, _sc, event| {
        handle_event(&handle, app_handle, event);
    })?;
    Ok(())
}

fn handle_event(app: &AppHandle, _app_handle: &AppHandle, event: ShortcutEvent) {
    // Toggle on each press only; ignore release.
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    let was_recording = state.is_recording.load(Ordering::SeqCst);

    if was_recording {
        if let Err(e) = state.recorder.stop_session() {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        state.is_recording.store(false, Ordering::SeqCst);
        emit_status(app, AppStatus::Idle);
        hide_indicator(app);
    } else {
        if let Err(e) = state.recorder.start_session() {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        state.is_recording.store(true, Ordering::SeqCst);
        show_indicator(app);
        emit_status(app, AppStatus::Listening);
    }
}

pub fn show_indicator(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        let _ = w.show();
    }
}

pub fn hide_indicator(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        let _ = w.hide();
    }
}
