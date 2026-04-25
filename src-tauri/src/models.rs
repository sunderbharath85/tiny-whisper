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

/// `target_dir` is the directory the model's files belong in (already includes
/// any per-model subdirectory).
pub async fn download(app: AppHandle, target_dir: &Path, model: ModelId) -> Result<()> {
    tokio::fs::create_dir_all(target_dir).await?;

    let client = reqwest::Client::builder()
        .user_agent("tiny-whisper/0.1")
        .build()?;

    let files = model.files();
    let mut cumulative_downloaded: u64 = 0;
    // Track totals per-file. We sum already-downloaded files plus the current
    // request's content_length so the bar advances smoothly without HEAD calls.
    let mut known_total: u64 = 0;
    let mut last_emit = std::time::Instant::now();

    for (idx, f) in files.iter().enumerate() {
        let dest = target_dir.join(f.filename);
        let tmp = target_dir.join(format!("{}.part", f.filename));
        if dest.exists() {
            cumulative_downloaded += tokio::fs::metadata(&dest).await?.len();
            known_total = known_total.max(cumulative_downloaded);
            continue;
        }

        let resp = client.get(f.url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "download failed for {}: HTTP {}",
                f.filename,
                resp.status()
            ));
        }
        let this_total = resp.content_length().unwrap_or(0);
        let base = cumulative_downloaded;
        known_total = known_total.max(base + this_total);

        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            cumulative_downloaded += chunk.len() as u64;
            if last_emit.elapsed() >= std::time::Duration::from_millis(150) {
                let _ = app.emit(
                    "model://progress",
                    DownloadProgress {
                        model,
                        downloaded_bytes: cumulative_downloaded,
                        total_bytes: known_total.max(cumulative_downloaded),
                    },
                );
                last_emit = std::time::Instant::now();
            }
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp, &dest).await?;

        // Emit a sync after each file so the UI doesn't sit at the same number.
        let is_last = idx + 1 == files.len();
        let _ = app.emit(
            "model://progress",
            DownloadProgress {
                model,
                downloaded_bytes: cumulative_downloaded,
                total_bytes: if is_last {
                    cumulative_downloaded
                } else {
                    known_total.max(cumulative_downloaded)
                },
            },
        );
    }

    Ok(())
}
