use crate::config::{Device, ModelId};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

struct Loaded {
    model: ModelId,
    device: Device,
    ctx: WhisperContext,
}

pub struct Transcriber {
    inner: Arc<Mutex<Option<Loaded>>>,
    models_dir: PathBuf,
}

impl Transcriber {
    pub fn new(models_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
            models_dir,
        }
    }

    pub fn model_path(&self, model: ModelId) -> PathBuf {
        self.models_dir.join(model.filename())
    }

    pub fn is_downloaded(&self, model: ModelId) -> bool {
        self.model_path(model).exists()
    }

    fn ensure_loaded(&self, model: ModelId, device: Device) -> Result<()> {
        let mut guard = self.inner.lock();
        if let Some(l) = guard.as_ref() {
            if l.model == model && l.device == device {
                return Ok(());
            }
        }
        let path = self.model_path(model);
        if !path.exists() {
            return Err(anyhow!("model not downloaded: {}", model.filename()));
        }
        let mut params = WhisperContextParameters::default();
        params.use_gpu(matches!(device, Device::Gpu));
        let ctx = WhisperContext::new_with_params(
            path.to_str().ok_or_else(|| anyhow!("bad path"))?,
            params,
        )?;
        *guard = Some(Loaded { model, device, ctx });
        Ok(())
    }

    /// Unload the current model (e.g. before deleting its file).
    pub fn unload(&self) {
        *self.inner.lock() = None;
    }

    pub fn transcribe(
        &self,
        samples: &[f32],
        model: ModelId,
        device: Device,
        language: &str,
    ) -> Result<String> {
        self.ensure_loaded(model, device)?;
        let guard = self.inner.lock();
        let loaded = guard.as_ref().ok_or_else(|| anyhow!("no model loaded"))?;
        let mut state = loaded.ctx.create_state()?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(num_threads() as i32);
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if language != "auto" && !language.is_empty() {
            params.set_language(Some(language));
        }

        state.full(params, samples)?;

        let num_segments = state.full_n_segments();
        let mut out = String::new();
        for i in 0..num_segments {
            if let Some(seg) = state.get_segment(i) {
                out.push_str(&seg.to_str_lossy()?);
            }
        }
        Ok(out.trim().to_string())
    }
}

fn num_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(2)
}

pub fn delete_model_file(models_dir: &Path, model: ModelId) -> Result<()> {
    let p = models_dir.join(model.filename());
    if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}
