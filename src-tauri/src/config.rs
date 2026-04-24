use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
}

impl ModelId {
    pub fn filename(self) -> &'static str {
        match self {
            ModelId::TinyEn => "ggml-tiny.en.bin",
            ModelId::BaseEn => "ggml-base.en.bin",
            ModelId::SmallEn => "ggml-small.en.bin",
            ModelId::MediumEn => "ggml-medium.en.bin",
            ModelId::LargeV3 => "ggml-large-v3.bin",
        }
    }
    pub fn url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }
    pub fn size_mb(self) -> u32 {
        match self {
            ModelId::TinyEn => 39,
            ModelId::BaseEn => 142,
            ModelId::SmallEn => 466,
            ModelId::MediumEn => 1500,
            ModelId::LargeV3 => 3000,
        }
    }
    pub fn all() -> &'static [ModelId] {
        &[
            ModelId::TinyEn,
            ModelId::BaseEn,
            ModelId::SmallEn,
            ModelId::MediumEn,
            ModelId::LargeV3,
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
    pub model: ModelId,
    pub device: Device,
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "CommandOrControl+Shift+Space".into(),
            model: ModelId::SmallEn,
            device: Device::Cpu,
            language: "auto".into(),
        }
    }
}

pub fn models_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
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
