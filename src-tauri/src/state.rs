use crate::config::Settings;
use crate::recorder::Recorder;
use crate::transcriber::Transcriber;
use parking_lot::{Condvar, Mutex};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Clone)]
#[allow(dead_code)] // populated for invariant tracking + future UI surfacing
pub struct ActiveSession {
    pub id: String,
    pub started_at: SystemTime,
}

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub settings: Arc<Mutex<Settings>>,
    pub recorder: Arc<Recorder>,
    pub transcriber: Arc<Transcriber>,
    pub is_recording: Arc<AtomicBool>,
    /// Set while a session recording is active.
    pub active_session: Arc<Mutex<Option<ActiveSession>>>,
    /// Notified when a session writer ends and releases `active_session`.
    /// Wait on it with `active_session` locked.
    pub session_ended: Arc<Condvar>,
    /// Error from boot (hotkey registration) for the UI to fetch once it loads.
    pub startup_error: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(
        app_data_dir: PathBuf,
        settings: Settings,
        recorder: Recorder,
        transcriber: Transcriber,
    ) -> Self {
        Self {
            app_data_dir,
            settings: Arc::new(Mutex::new(settings)),
            recorder: Arc::new(recorder),
            transcriber: Arc::new(transcriber),
            is_recording: Arc::new(AtomicBool::new(false)),
            active_session: Arc::new(Mutex::new(None)),
            session_ended: Arc::new(Condvar::new()),
            startup_error: Mutex::new(None),
        }
    }
}
