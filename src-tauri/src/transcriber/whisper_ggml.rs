use super::Backend;
use crate::config::{Device, ModelId};
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

struct Loaded {
    model: ModelId,
    device: Device,
    ctx: WhisperContext,
}

pub struct WhisperGgmlBackend {
    inner: Mutex<Option<Loaded>>,
    models_dir: PathBuf,
}

impl WhisperGgmlBackend {
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
        let file = model
            .files()
            .first()
            .ok_or_else(|| anyhow!("whisper model has no files"))?;
        let path = self.models_dir.join(file.filename);
        if !path.exists() {
            return Err(anyhow!("model not downloaded: {}", file.filename));
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
}

impl Backend for WhisperGgmlBackend {
    fn transcribe(
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

    fn unload(&self) {
        WhisperGgmlBackend::unload(self);
    }
}

fn num_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(2)
}
