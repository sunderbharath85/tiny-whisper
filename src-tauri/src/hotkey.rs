use crate::config::Settings;
use crate::recorder::WavMsg;
use crate::session_worker;
use crate::sessions;
use crate::state::{ActiveSession, AppState};
use anyhow::Result;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AppStatus {
    Idle,
    Listening,
    Speaking,
    Transcribing,
    /// A session is being recorded to disk (raw capture mode).
    RecordingSession,
    /// A previously-recorded session is being transcribed offline.
    TranscribingSession { session_id: String, percent: f32 },
    Error { message: String },
}

pub fn emit_status(app: &AppHandle, status: AppStatus) {
    let _ = app.emit("app://status", status);
}

pub fn register(app: &AppHandle, settings: &Settings) -> Result<()> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let dictation: Shortcut = settings.hotkey.parse()?;
    let dict_handle = app.clone();
    gs.on_shortcut(dictation, move |app_handle, _sc, event| {
        handle_dictation(&dict_handle, app_handle, event);
    })?;

    // Session hotkey is best-effort: a parse failure shouldn't kill the
    // dictation hotkey we just registered.
    if !settings.session_hotkey.trim().is_empty() {
        match settings.session_hotkey.parse::<Shortcut>() {
            Ok(session_sc) => {
                let sess_handle = app.clone();
                gs.on_shortcut(session_sc, move |app_handle, _sc, event| {
                    handle_session(&sess_handle, app_handle, event);
                })?;
            }
            Err(e) => log::warn!(
                "skipping invalid session_hotkey {:?}: {e}",
                settings.session_hotkey
            ),
        }
    }
    Ok(())
}

fn handle_dictation(app: &AppHandle, _app_handle: &AppHandle, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    // Don't let the dictation hotkey fire while a session is being recorded —
    // the recorder would refuse the cpal stream and log a warning.
    if state.active_session.lock().is_some() {
        return;
    }
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

fn handle_session(app: &AppHandle, _app_handle: &AppHandle, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    let active = state.active_session.lock().clone();

    if active.is_some() {
        if let Err(e) = state.recorder.stop_raw_capture() {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        // Writer thread clears active_session on finalize.
        hide_indicator(app);
    } else {
        // Don't let a session start on top of an in-flight dictation.
        if state.is_recording.load(Ordering::SeqCst) {
            log::warn!("ignoring session hotkey while dictation is active");
            return;
        }
        let id = sessions::new_session_id();
        let started_at = SystemTime::now();
        let (wav_tx, wav_rx) = channel::<WavMsg>();
        if let Err(e) = state.recorder.start_raw_capture(wav_tx) {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        *state.active_session.lock() = Some(ActiveSession {
            id: id.clone(),
            started_at,
        });
        session_worker::spawn_writer(app.clone(), wav_rx, id, started_at);
        show_indicator(app);
        emit_status(app, AppStatus::RecordingSession);
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
