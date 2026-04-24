use crate::config::ModelId;
use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub model: ModelId,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

pub async fn download(app: AppHandle, models_dir: &Path, model: ModelId) -> Result<()> {
    tokio::fs::create_dir_all(models_dir).await?;
    let dest = models_dir.join(model.filename());
    let tmp = models_dir.join(format!("{}.part", model.filename()));

    let client = reqwest::Client::builder()
        .user_agent("tiny-whisper/0.1")
        .build()?;
    let resp = client.get(model.url()).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        if last_emit.elapsed() >= std::time::Duration::from_millis(150) {
            let _ = app.emit(
                "model://progress",
                DownloadProgress { model, downloaded_bytes: downloaded, total_bytes: total },
            );
            last_emit = std::time::Instant::now();
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&tmp, &dest).await?;

    let _ = app.emit(
        "model://progress",
        DownloadProgress { model, downloaded_bytes: downloaded, total_bytes: total.max(downloaded) },
    );
    Ok(())
}
