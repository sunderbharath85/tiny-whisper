use super::Backend;
use crate::config::{Device, ModelId};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::PathBuf;

use parakeet_rs::{
    ExecutionConfig, ExecutionProvider, Parakeet, ParakeetTDT, Transcriber as PrTranscriber,
};

enum LoadedKind {
    Ctc(Parakeet),
    Tdt(ParakeetTDT),
}

struct Loaded {
    model: ModelId,
    device: Device,
    inner: LoadedKind,
}

pub struct ParakeetOnnxBackend {
    inner: Mutex<Option<Loaded>>,
    models_dir: PathBuf,
}

impl ParakeetOnnxBackend {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            inner: Mutex::new(None),
            models_dir,
        }
    }

    pub fn unload(&self) {
        *self.inner.lock() = None;
    }

    fn ensure_loaded(&self, model: ModelId, device: Device) -> Result<()> {
        let mut guard = self.inner.lock();
        if let Some(l) = guard.as_ref() {
            if l.model == model && l.device == device {
                return Ok(());
            }
        }

        let dir = match model.subdir() {
            Some(sub) => self.models_dir.join(sub),
            None => self.models_dir.clone(),
        };
        if !dir.exists() {
            return Err(anyhow!("parakeet model dir missing: {}", dir.display()));
        }

        let dir_str = dir
            .to_str()
            .ok_or_else(|| anyhow!("non-utf8 model path"))?;

        let inner = match model {
            ModelId::ParakeetCtc06BEn => {
                let p = Parakeet::from_pretrained(dir_str, exec_config(device))
                    .map_err(|e| anyhow!("parakeet ctc load: {e}"))?;
                LoadedKind::Ctc(p)
            }
            ModelId::ParakeetTdt06BV3 => {
                let p = ParakeetTDT::from_pretrained(dir_str, exec_config(device))
                    .map_err(|e| anyhow!("parakeet tdt load: {e}"))?;
                LoadedKind::Tdt(p)
            }
            _ => return Err(anyhow!("not a parakeet model: {:?}", model)),
        };

        *guard = Some(Loaded {
            model,
            device,
            inner,
        });
        Ok(())
    }
}

fn exec_config(device: Device) -> Option<ExecutionConfig> {
    match device {
        #[cfg(feature = "gpu")]
        Device::Gpu => {
            Some(ExecutionConfig::new().with_execution_provider(ExecutionProvider::DirectML))
        }
        #[cfg(not(feature = "gpu"))]
        Device::Gpu => Some(ExecutionConfig::new().with_execution_provider(ExecutionProvider::Cpu)),
        Device::Cpu => None,
    }
}

impl Backend for ParakeetOnnxBackend {
    fn transcribe(
        &self,
        samples: &[f32],
        model: ModelId,
        device: Device,
        _language: &str,
    ) -> Result<String> {
        self.ensure_loaded(model, device)?;
        let mut guard = self.inner.lock();
        let loaded = guard.as_mut().ok_or_else(|| anyhow!("no model loaded"))?;
        let owned: Vec<f32> = samples.to_vec();
        let result = match &mut loaded.inner {
            LoadedKind::Ctc(p) => p
                .transcribe_samples(owned, 16000, 1, None)
                .map_err(|e| anyhow!("parakeet ctc transcribe: {e}"))?,
            LoadedKind::Tdt(p) => p
                .transcribe_samples(owned, 16000, 1, None)
                .map_err(|e| anyhow!("parakeet tdt transcribe: {e}"))?,
        };
        Ok(result.text.trim().to_string())
    }

    fn unload(&self) {
        ParakeetOnnxBackend::unload(self);
    }
}
