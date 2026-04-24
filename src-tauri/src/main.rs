#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod hotkey;
mod models;
mod paster;
mod recorder;
mod state;
mod transcriber;
mod vad;

use crate::config::Settings;
use crate::hotkey::{emit_status, AppStatus};
use crate::recorder::{Recorder, RecorderEvent, Segment};
use crate::state::AppState;
use crate::transcriber::Transcriber;
use std::sync::mpsc::{channel, Receiver};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

fn main() {
    init_logging();
    install_panic_logger();
    // Route whisper.cpp + ggml's internal logs through the `log` crate so they
    // hit the file logger above instead of the (detached) stderr in release.
    whisper_rs::install_logging_hooks();

    let (seg_tx, seg_rx) = channel::<Segment>();
    let (evt_tx, evt_rx) = channel::<RecorderEvent>();
    let recorder = Recorder::spawn(seg_tx, evt_tx);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide to tray instead of quitting the app.
                if window.label() == "settings" || window.label() == "indicator" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::list_models,
            commands::download_model,
            commands::delete_model,
        ])
        .setup(move |app| {
            let app_data_dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();
            std::fs::create_dir_all(config::models_dir(&app_data_dir)).ok();

            let settings: Settings = config::load(&app_data_dir);
            let transcriber = Transcriber::new(config::models_dir(&app_data_dir));
            let app_state = AppState::new(app_data_dir, settings.clone(), recorder, transcriber);
            app.manage(app_state);

            hotkey::register(app.handle(), &settings)
                .unwrap_or_else(|e| log::error!("hotkey register failed: {e}"));

            // Worker: consume recorder events, translate to UI status.
            let evt_app = app.handle().clone();
            std::thread::spawn(move || event_worker(evt_app, evt_rx));

            // Worker: consume segments, transcribe, paste.
            let seg_app = app.handle().clone();
            std::thread::spawn(move || segment_worker(seg_app, seg_rx));

            build_tray(app.handle())?;
            position_indicator(app.handle());
            // Show settings window on first launch so the user isn't staring at a
            // hidden window after install. Close-to-tray keeps subsequent launches quiet.
            show_settings(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // Tray app: don't quit when the last window hides. Without this,
            // tao panics with "cannot move state from Destroyed" on Windows
            // when the settings window is hidden (which looks like "closing"
            // the last window to the event loop).
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

fn log_file_path() -> std::path::PathBuf {
    let dir = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev.tinywhisper.app");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("tiny-whisper.log")
}

fn init_logging() {
    // Release builds have no stderr (windows_subsystem="windows"). Write logs
    // to %APPDATA%\dev.tinywhisper.app\tiny-whisper.log. Level defaults to
    // `debug` so whisper.cpp's decoder traces show up; override with RUST_LOG.
    let path = log_file_path();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path);
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"));
    if let Ok(f) = file {
        // LineWriter flushes on each newline so records actually land on disk
        // even if the process is killed or panics mid-session.
        let writer = std::io::LineWriter::new(f);
        builder.target(env_logger::Target::Pipe(Box::new(writer)));
    }
    let _ = builder.try_init();
    log::info!("tiny-whisper starting; log level = {}", std::env::var("RUST_LOG").unwrap_or_else(|_| "debug".into()));
}

fn install_panic_logger() {
    // Release builds use windows_subsystem="windows" — no console. Without this
    // hook a panic in setup disappears silently and the user just sees a blank
    // window. Write to %APPDATA%\dev.tinywhisper.app\panic.log (or XDG equivalent).
    let dir = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("dev.tinywhisper.app");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("panic.log");
    std::panic::set_hook(Box::new(move |info| {
        let msg = format!(
            "panic at {}: {}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            info
        );
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            use std::io::Write;
            let _ = f.write_all(msg.as_bytes());
        }
        log::error!("{msg}");
    }));
}

fn event_worker(app: AppHandle, rx: Receiver<RecorderEvent>) {
    while let Ok(e) = rx.recv() {
        match e {
            RecorderEvent::SessionStarted | RecorderEvent::Listening => {
                emit_status(&app, AppStatus::Listening);
            }
            RecorderEvent::SpeechStarted => {
                emit_status(&app, AppStatus::Speaking);
            }
            RecorderEvent::SessionStopped => {
                emit_status(&app, AppStatus::Idle);
            }
            RecorderEvent::Error => {
                emit_status(
                    &app,
                    AppStatus::Error {
                        message: "recorder error".into(),
                    },
                );
            }
        }
    }
}

fn segment_worker(app: AppHandle, rx: Receiver<Segment>) {
    while let Ok(samples) = rx.recv() {
        if samples.len() < 1600 {
            continue; // <100ms, ignore
        }
        let state = app.state::<AppState>();
        emit_status(&app, AppStatus::Transcribing);
        let settings = state.settings.lock().clone();
        let result = state.transcriber.transcribe(
            &samples,
            settings.model,
            settings.device,
            &settings.language,
        );
        match result {
            Ok(text) if !text.trim().is_empty() && text.trim() != "[BLANK_AUDIO]" => {
                if let Err(e) = paster::paste(&(text.trim().to_string() + " ")) {
                    emit_status(
                        &app,
                        AppStatus::Error {
                            message: e.to_string(),
                        },
                    );
                }
            }
            Err(e) => {
                emit_status(
                    &app,
                    AppStatus::Error {
                        message: e.to_string(),
                    },
                );
            }
            _ => {}
        }
        // After transcription, if session still active, go back to Listening.
        if state
            .is_recording
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            emit_status(&app, AppStatus::Listening);
        } else {
            emit_status(&app, AppStatus::Idle);
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_i = MenuItem::with_id(app, "open", "Open settings", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &quit_i])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon should exist from bundled icons");
    let _tray = TrayIconBuilder::with_id("main")
        .tooltip("tiny-whisper")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn position_indicator(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        if let Ok(monitor) = w.current_monitor() {
            if let Some(m) = monitor {
                let size = m.size();
                let scale = m.scale_factor();
                let w_px = 240.0;
                let h_px = 64.0;
                let margin = 20.0;
                let x = (size.width as f64 / scale) - w_px - margin;
                let y = (size.height as f64 / scale) - h_px - margin - 40.0; // above taskbar
                let _ = w.set_position(tauri::LogicalPosition::new(x, y));
            }
        }
    }
}
