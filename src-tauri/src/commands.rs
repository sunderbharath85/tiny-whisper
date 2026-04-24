use crate::config::{self, ModelId, Settings};
use crate::hotkey;
use crate::models;
use crate::state::AppState;
use crate::transcriber;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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
    let dir = config::models_dir(&state.app_data_dir);
    models::download(app.clone(), &dir, id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_model(state: State<'_, AppState>, id: ModelId) -> Result<(), String> {
    // If currently loaded, unload first.
    let current = state.settings.lock().model;
    if current == id {
        state.transcriber.unload();
    }
    let dir = config::models_dir(&state.app_data_dir);
    transcriber::delete_model_file(&dir, id).map_err(|e| e.to_string())
}
