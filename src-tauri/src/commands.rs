use crate::config::{self, ModelId, Settings};
use crate::hotkey;
use crate::models;
use crate::recorder::WavMsg;
use crate::session_worker;
use crate::sessions::{self, SessionMeta, Transcript};
use crate::state::{ActiveSession, AppState};
use crate::transcriber;
use serde::Serialize;
use std::sync::mpsc::channel;
use std::time::SystemTime;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[derive(Serialize)]
pub struct ModelStatus {
    pub id: ModelId,
    pub downloaded: bool,
    pub size_mb: u32,
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().clone()
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), String> {
    config::save(&state.app_data_dir, &settings).map_err(|e| e.to_string())?;
    // Re-register hotkey if changed.
    let prev_hotkey = state.settings.lock().hotkey.clone();
    *state.settings.lock() = settings.clone();
    if prev_hotkey != settings.hotkey {
        hotkey::register(&app, &settings).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_models(state: State<'_, AppState>) -> Vec<ModelStatus> {
    ModelId::all()
        .iter()
        .map(|&id| ModelStatus {
            id,
            downloaded: state.transcriber.is_downloaded(id),
            size_mb: id.size_mb(),
        })
        .collect()
}

#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    id: ModelId,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let dir = config::model_dir(&state.app_data_dir, id);
    models::download(app.clone(), &dir, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let m = app.autolaunch();
    if enabled {
        m.enable().map_err(|e| e.to_string())
    } else {
        m.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn start_session_recording(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    {
        let active = state.active_session.lock();
        if active.is_some() {
            return Err("a session recording is already active".into());
        }
    }
    let id = sessions::new_session_id();
    let started_at = SystemTime::now();
    let (wav_tx, wav_rx) = channel::<WavMsg>();
    state
        .recorder
        .start_raw_capture(wav_tx)
        .map_err(|e| e.to_string())?;
    *state.active_session.lock() = Some(ActiveSession {
        id: id.clone(),
        started_at,
    });
    session_worker::spawn_writer(app, wav_rx, id.clone(), started_at);
    Ok(id)
}

#[tauri::command]
pub fn stop_session_recording(state: State<'_, AppState>) -> Result<(), String> {
    if state.active_session.lock().is_none() {
        return Err("no active session".into());
    }
    state.recorder.stop_raw_capture().map_err(|e| e.to_string())?;
    // active_session is cleared by the writer thread when it finalizes.
    Ok(())
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    sessions::list(&state.app_data_dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_session(state: State<'_, AppState>, id: String) -> Result<(), String> {
    sessions::delete(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session_transcript(
    state: State<'_, AppState>,
    id: String,
) -> Result<Transcript, String> {
    sessions::read_transcript(&state.app_data_dir, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn transcribe_session(
    app: AppHandle,
    id: String,
    diarize: bool,
) -> Result<(), String> {
    session_worker::spawn_transcriber(app, id, diarize);
    Ok(())
}

#[tauri::command]
pub fn delete_model(state: State<'_, AppState>, id: ModelId) -> Result<(), String> {
    // If currently loaded, unload first.
    let current = state.settings.lock().model;
    if current == id {
        state.transcriber.unload();
    }
    let dir = config::models_dir(&state.app_data_dir);
    transcriber::delete_model_files(&dir, id).map_err(|e| e.to_string())
}
