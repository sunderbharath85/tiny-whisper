use crate::config::{Device, ModelId};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::PathBuf;

use parakeet_rs::sortformer::{DiarizationConfig, Sortformer};
use parakeet_rs::{ExecutionConfig, ExecutionProvider};

/// Result of diarizing an audio buffer: contiguous speaker turns in
/// chronological order.
#[derive(Clone, Debug)]
pub struct SpeakerTurn {
    pub start_secs: f32,
    pub end_secs: f32,
    pub speaker_id: u8,
}

struct Loaded {
    device: Device,
    sortformer: Sortformer,
}

pub struct Diarizer {
    inner: Mutex<Option<Loaded>>,
    models_dir: PathBuf,
}

impl Diarizer {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            inner: Mutex::new(None),
            models_dir,
        }
    }

    pub fn unload(&self) {
        *self.inner.lock() = None;
    }

    fn ensure_loaded(&self, device: Device) -> Result<()> {
        let mut guard = self.inner.lock();
        if let Some(l) = guard.as_ref() {
            if l.device == device {
                return Ok(());
            }
        }
        let model = ModelId::Sortformer4SpkV2;
        let dir = match model.subdir() {
            Some(sub) => self.models_dir.join(sub),
            None => self.models_dir.clone(),
        };
        let file = model
            .files()
            .first()
            .ok_or_else(|| anyhow!("sortformer has no files"))?;
        let path = dir.join(file.filename);
        if !path.exists() {
            return Err(anyhow!(
                "sortformer model not downloaded: {}",
                path.display()
            ));
        }
        let sortformer = Sortformer::with_config(
            path,
            exec_config(device),
            DiarizationConfig::default(),
        )
        .map_err(|e| anyhow!("sortformer load: {e}"))?;
        *guard = Some(Loaded { device, sortformer });
        Ok(())
    }

    /// Run full-file diarization on the given 16 kHz mono audio.
    pub fn diarize(&self, samples: &[f32], device: Device) -> Result<Vec<SpeakerTurn>> {
        self.ensure_loaded(device)?;
        let mut guard = self.inner.lock();
        let loaded = guard.as_mut().ok_or_else(|| anyhow!("no diarizer loaded"))?;
        let segs = loaded
            .sortformer
            .diarize(samples.to_vec(), 16_000, 1)
            .map_err(|e| anyhow!("sortformer diarize: {e}"))?;
        Ok(segs
            .into_iter()
            .map(|s| SpeakerTurn {
                start_secs: s.start as f32 / 16_000.0,
                end_secs: s.end as f32 / 16_000.0,
                speaker_id: s.speaker_id as u8,
            })
            .collect())
    }
}

fn exec_config(device: Device) -> Option<ExecutionConfig> {
    match device {
        #[cfg(all(feature = "gpu", target_os = "macos"))]
        Device::Gpu => Some(ExecutionConfig::new().with_execution_provider(ExecutionProvider::CoreML)),
        #[cfg(all(feature = "gpu", not(target_os = "macos")))]
        Device::Gpu => Some(ExecutionConfig::new().with_execution_provider(ExecutionProvider::DirectML)),
        #[cfg(not(feature = "gpu"))]
        Device::Gpu => Some(ExecutionConfig::new().with_execution_provider(ExecutionProvider::Cpu)),
        Device::Cpu => None,
    }
}
