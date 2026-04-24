use crate::config::Settings;
use crate::recorder::Recorder;
use crate::transcriber::Transcriber;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub settings: Arc<Mutex<Settings>>,
    pub recorder: Arc<Recorder>,
    pub transcriber: Arc<Transcriber>,
    pub is_recording: Arc<AtomicBool>,
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
        }
    }
}
