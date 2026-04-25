mod parakeet_onnx;
mod sortformer;
mod whisper_ggml;

use crate::config::{Device, Engine, ModelId};
use anyhow::Result;
use std::path::{Path, PathBuf};

use parakeet_onnx::ParakeetOnnxBackend;
pub use sortformer::{Diarizer, SpeakerTurn};
use whisper_ggml::WhisperGgmlBackend;

pub trait Backend: Send + Sync {
    fn transcribe(
        &self,
        samples: &[f32],
        model: ModelId,
        device: Device,
        language: &str,
    ) -> Result<String>;
    fn unload(&self);
}

pub struct Transcriber {
    models_dir: PathBuf,
    whisper: WhisperGgmlBackend,
    parakeet: ParakeetOnnxBackend,
    diarizer: Diarizer,
}

impl Transcriber {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            whisper: WhisperGgmlBackend::new(models_dir.clone()),
            parakeet: ParakeetOnnxBackend::new(models_dir.clone()),
            diarizer: Diarizer::new(models_dir.clone()),
            models_dir,
        }
    }

    pub fn diarizer(&self) -> &Diarizer {
        &self.diarizer
    }

    /// Directory containing this model's files.
    pub fn model_dir(&self, model: ModelId) -> PathBuf {
        match model.subdir() {
            Some(sub) => self.models_dir.join(sub),
            None => self.models_dir.clone(),
        }
    }

    pub fn is_downloaded(&self, model: ModelId) -> bool {
        let dir = self.model_dir(model);
        model.files().iter().all(|f| dir.join(f.filename).exists())
    }

    fn backend_for(&self, engine: Engine) -> &dyn Backend {
        match engine {
            Engine::Whisper => &self.whisper,
            Engine::Parakeet => &self.parakeet,
            // Diarizer is not a transcription backend; route to whisper as a
            // safe default. Callers that route by engine should not pass a
            // Diarizer model here.
            Engine::Diarizer => &self.whisper,
        }
    }

    pub fn unload(&self) {
        Backend::unload(&self.whisper);
        Backend::unload(&self.parakeet);
        self.diarizer.unload();
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        model: ModelId,
        device: Device,
        language: &str,
    ) -> Result<String> {
        self.backend_for(model.engine())
            .transcribe(samples, model, device, language)
    }
}

/// Delete every file (and the subdirectory, if any) belonging to `model`.
pub fn delete_model_files(models_dir: &Path, model: ModelId) -> Result<()> {
    let dir = match model.subdir() {
        Some(sub) => models_dir.join(sub),
        None => models_dir.to_path_buf(),
    };
    for f in model.files() {
        let p = dir.join(f.filename);
        if p.exists() {
            std::fs::remove_file(p)?;
        }
    }
    if model.subdir().is_some() && dir.exists() {
        // Best effort; non-empty dir or in-use file is fine to leave.
        let _ = std::fs::remove_dir(&dir);
    }
    Ok(())
}
