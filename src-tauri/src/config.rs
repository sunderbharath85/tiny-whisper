use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Whisper,
    Parakeet,
    /// Speaker diarization (Sortformer). Not used as the active dictation model;
    /// downloaded separately for the session-recording feature.
    Diarizer,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ModelId {
    #[serde(rename = "tiny.en")]
    TinyEn,
    #[serde(rename = "base.en")]
    BaseEn,
    #[serde(rename = "small.en")]
    SmallEn,
    #[serde(rename = "medium.en")]
    MediumEn,
    #[serde(rename = "large-v3")]
    LargeV3,
    #[serde(rename = "parakeet-ctc-0.6b-en")]
    ParakeetCtc06BEn,
    #[serde(rename = "parakeet-tdt-0.6b-v3")]
    ParakeetTdt06BV3,
    #[serde(rename = "sortformer-4spk-v2")]
    Sortformer4SpkV2,
}

/// Description of a file that makes up a model.
pub struct ModelFile {
    pub filename: &'static str,
    pub url: &'static str,
}

impl ModelId {
    pub fn engine(self) -> Engine {
        match self {
            ModelId::TinyEn
            | ModelId::BaseEn
            | ModelId::SmallEn
            | ModelId::MediumEn
            | ModelId::LargeV3 => Engine::Whisper,
            ModelId::ParakeetCtc06BEn | ModelId::ParakeetTdt06BV3 => Engine::Parakeet,
            ModelId::Sortformer4SpkV2 => Engine::Diarizer,
        }
    }

    /// Subdirectory under `models/` that holds this model's files. `None` means
    /// files live directly in `models/` (current Whisper layout, kept stable so
    /// existing installs keep working).
    pub fn subdir(self) -> Option<&'static str> {
        match self {
            ModelId::ParakeetCtc06BEn => Some("parakeet-ctc-0.6b-en"),
            ModelId::ParakeetTdt06BV3 => Some("parakeet-tdt-0.6b-v3"),
            ModelId::Sortformer4SpkV2 => Some("sortformer-4spk-v2"),
            _ => None,
        }
    }

    pub fn files(self) -> &'static [ModelFile] {
        match self {
            ModelId::TinyEn => &[ModelFile {
                filename: "ggml-tiny.en.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin",
            }],
            ModelId::BaseEn => &[ModelFile {
                filename: "ggml-base.en.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin",
            }],
            ModelId::SmallEn => &[ModelFile {
                filename: "ggml-small.en.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en.bin",
            }],
            ModelId::MediumEn => &[ModelFile {
                filename: "ggml-medium.en.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.en.bin",
            }],
            ModelId::LargeV3 => &[ModelFile {
                filename: "ggml-large-v3.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3.bin",
            }],
            ModelId::ParakeetCtc06BEn => &[
                ModelFile {
                    filename: "model.onnx",
                    url: "https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/resolve/main/onnx/model.onnx",
                },
                ModelFile {
                    filename: "model.onnx_data",
                    url: "https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/resolve/main/onnx/model.onnx_data",
                },
                ModelFile {
                    filename: "tokenizer.json",
                    url: "https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX/resolve/main/tokenizer.json",
                },
            ],
            ModelId::ParakeetTdt06BV3 => &[
                ModelFile {
                    filename: "encoder-model.onnx",
                    url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/encoder-model.onnx",
                },
                ModelFile {
                    filename: "encoder-model.onnx.data",
                    url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/encoder-model.onnx.data",
                },
                ModelFile {
                    filename: "decoder_joint-model.onnx",
                    url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/decoder_joint-model.onnx",
                },
                ModelFile {
                    filename: "vocab.txt",
                    url: "https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/main/vocab.txt",
                },
            ],
            ModelId::Sortformer4SpkV2 => &[ModelFile {
                filename: "diar_streaming_sortformer_4spk-v2.onnx",
                url: "https://huggingface.co/altunenes/parakeet-rs/resolve/main/diar_streaming_sortformer_4spk-v2.onnx",
            }],
        }
    }

    pub fn size_mb(self) -> u32 {
        match self {
            ModelId::TinyEn => 39,
            ModelId::BaseEn => 142,
            ModelId::SmallEn => 466,
            ModelId::MediumEn => 1500,
            ModelId::LargeV3 => 3000,
            ModelId::ParakeetCtc06BEn => 2400,
            ModelId::ParakeetTdt06BV3 => 2500,
            ModelId::Sortformer4SpkV2 => 250,
        }
    }
    pub fn all() -> &'static [ModelId] {
        &[
            ModelId::TinyEn,
            ModelId::BaseEn,
            ModelId::SmallEn,
            ModelId::MediumEn,
            ModelId::LargeV3,
            ModelId::ParakeetCtc06BEn,
            ModelId::ParakeetTdt06BV3,
            ModelId::Sortformer4SpkV2,
        ]
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Device {
    Cpu,
    Gpu,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub hotkey: String,
    #[serde(default = "default_session_hotkey")]
    pub session_hotkey: String,
    pub model: ModelId,
    pub device: Device,
    pub language: String,
}

fn default_session_hotkey() -> String {
    "CommandOrControl+Shift+R".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "CommandOrControl+Shift+Space".into(),
            session_hotkey: default_session_hotkey(),
            model: ModelId::SmallEn,
            device: Device::Cpu,
            language: "auto".into(),
        }
    }
}

pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
}

/// Directory containing this model's files.
pub fn model_dir(app_data: &Path, model: ModelId) -> PathBuf {
    let base = models_dir(app_data);
    match model.subdir() {
        Some(sub) => base.join(sub),
        None => base,
    }
}

pub fn settings_path(app_data: &Path) -> PathBuf {
    app_data.join("settings.json")
}

pub fn load(app_data: &Path) -> Settings {
    let p = settings_path(app_data);
    match std::fs::read_to_string(&p) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(app_data: &Path, s: &Settings) -> Result<()> {
    std::fs::create_dir_all(app_data)?;
    let p = settings_path(app_data);
    std::fs::write(p, serde_json::to_vec_pretty(s)?)?;
    Ok(())
}
