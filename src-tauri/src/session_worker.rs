//! Background worker logic for the recorded-session feature.
//!
//! Two flavors:
//!   1. `spawn_writer` — owns a hound WavWriter for the duration of one
//!      recording. Listens for `WavMsg::Chunk` / `WavMsg::Stop`, writes a
//!      finished WAV + meta.json, emits `session://updated`.
//!   2. `spawn_transcriber` — runs Sortformer + per-segment transcription on
//!      a saved session. Emits `session://progress` while running and
//!      `session://updated` on completion.

use crate::config::{Device, ModelId, Settings};
use crate::hotkey::{emit_status, AppStatus};
use crate::recorder::WavMsg;
use crate::sessions::{
    self, audio_path, session_dir, SessionMeta, Transcript, TranscriptSegment,
};
use crate::state::AppState;
use crate::transcriber::SpeakerTurn;
use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager};

const SAMPLE_RATE: u32 = 16_000;

/// Spawn a writer thread for an in-progress recording.
pub fn spawn_writer(
    app: AppHandle,
    rx: Receiver<WavMsg>,
    session_id: String,
    started_at: SystemTime,
) {
    std::thread::spawn(move || {
        if let Err(e) = run_writer(&app, rx, &session_id, started_at) {
            log::error!("session writer ({session_id}) failed: {e}");
            emit_status(
                &app,
                AppStatus::Error {
                    message: format!("session write failed: {e}"),
                },
            );
        }
    });
}

fn run_writer(
    app: &AppHandle,
    rx: Receiver<WavMsg>,
    session_id: &str,
    started_at: SystemTime,
) -> Result<()> {
    let state = app.state::<AppState>();
    let app_data = state.app_data_dir.clone();
    let dir = session_dir(&app_data, session_id);
    std::fs::create_dir_all(&dir)?;
    let wav_path = audio_path(&app_data, session_id);

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&wav_path, spec)?;
    let mut total_samples: u64 = 0;

    while let Ok(msg) = rx.recv() {
        match msg {
            WavMsg::Chunk(chunk) => {
                for s in &chunk {
                    let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    writer.write_sample(v)?;
                }
                total_samples += chunk.len() as u64;
            }
            WavMsg::Stop => break,
        }
    }
    writer.finalize()?;

    let duration_secs = total_samples as f32 / SAMPLE_RATE as f32;
    let created_at = iso8601_utc(started_at);
    let meta = SessionMeta {
        id: session_id.to_string(),
        created_at,
        duration_secs,
        model_used: None,
        speaker_count: None,
        has_transcript: false,
    };
    sessions::write_meta(&app_data, &meta)?;

    // Clear active-session pointer.
    *state.active_session.lock() = None;

    let _ = app.emit("session://updated", &meta);
    Ok(())
}

/// Spawn a transcription job for an existing saved session.
pub fn spawn_transcriber(app: AppHandle, session_id: String, diarize: bool) {
    std::thread::spawn(move || {
        if let Err(e) = run_transcriber(&app, &session_id, diarize) {
            log::error!("transcribe session ({session_id}) failed: {e}");
            emit_status(
                &app,
                AppStatus::Error {
                    message: format!("transcribe failed: {e}"),
                },
            );
        }
    });
}

fn run_transcriber(app: &AppHandle, session_id: &str, diarize: bool) -> Result<()> {
    let state = app.state::<AppState>();
    let app_data = state.app_data_dir.clone();
    let settings: Settings = state.settings.lock().clone();

    let wav_path = audio_path(&app_data, session_id);
    if !wav_path.exists() {
        return Err(anyhow!("audio missing: {}", wav_path.display()));
    }
    let samples = read_wav_16k_mono(&wav_path)?;

    let active_model: ModelId = settings.model;
    let device: Device = settings.device;

    emit_progress(app, session_id, 0.0);

    // 1. Speaker turns.
    let turns: Vec<SpeakerTurn> = if diarize {
        let d = state.transcriber.diarizer();
        d.diarize(&samples, device)?
    } else {
        let total = samples.len() as f32 / SAMPLE_RATE as f32;
        vec![SpeakerTurn {
            start_secs: 0.0,
            end_secs: total,
            speaker_id: 0,
        }]
    };
    let merged = merge_turns(&turns, 0.3);

    // 2. Transcribe each turn with the active model.
    let mut segments: Vec<TranscriptSegment> = Vec::with_capacity(merged.len());
    for (i, t) in merged.iter().enumerate() {
        let start_idx = (t.start_secs * SAMPLE_RATE as f32) as usize;
        let end_idx = ((t.end_secs * SAMPLE_RATE as f32) as usize).min(samples.len());
        if end_idx <= start_idx {
            continue;
        }
        let slice = &samples[start_idx..end_idx];
        // Whisper rejects sub-100ms inputs; let it skip those silently.
        let text = if slice.len() < 1600 {
            String::new()
        } else {
            state
                .transcriber
                .transcribe(slice, active_model, device, &settings.language)
                .unwrap_or_default()
        };
        segments.push(TranscriptSegment {
            start_secs: t.start_secs,
            end_secs: t.end_secs,
            speaker: if diarize { Some(t.speaker_id) } else { None },
            text: text.trim().to_string(),
        });
        let pct = (i + 1) as f32 / merged.len().max(1) as f32 * 100.0;
        emit_progress(app, session_id, pct);
    }

    // 3. Persist transcript and updated meta.
    let transcript = Transcript {
        model: active_model,
        diarized: diarize,
        segments: segments.clone(),
    };
    sessions::write_transcript(&app_data, session_id, &transcript)?;

    let mut meta = sessions::read_meta(&app_data, session_id)?;
    meta.has_transcript = true;
    meta.model_used = Some(active_model);
    meta.speaker_count = if diarize {
        Some(
            segments
                .iter()
                .filter_map(|s| s.speaker)
                .collect::<std::collections::BTreeSet<_>>()
                .len() as u8,
        )
    } else {
        None
    };
    sessions::write_meta(&app_data, &meta)?;

    let _ = app.emit("session://updated", &meta);
    emit_status(app, AppStatus::Idle);
    Ok(())
}

fn emit_progress(app: &AppHandle, session_id: &str, percent: f32) {
    emit_status(
        app,
        AppStatus::TranscribingSession {
            session_id: session_id.to_string(),
            percent,
        },
    );
}

/// Merge contiguous same-speaker turns separated by less than `gap_secs`.
fn merge_turns(turns: &[SpeakerTurn], gap_secs: f32) -> Vec<SpeakerTurn> {
    let mut sorted: Vec<SpeakerTurn> = turns.to_vec();
    sorted.sort_by(|a, b| a.start_secs.partial_cmp(&b.start_secs).unwrap());
    let mut out: Vec<SpeakerTurn> = Vec::new();
    for t in sorted {
        match out.last_mut() {
            Some(prev)
                if prev.speaker_id == t.speaker_id
                    && (t.start_secs - prev.end_secs) <= gap_secs =>
            {
                prev.end_secs = prev.end_secs.max(t.end_secs);
            }
            _ => out.push(t),
        }
    }
    out
}

fn read_wav_16k_mono(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE {
        return Err(anyhow!(
            "session WAV must be 16kHz mono, got {}ch {}Hz",
            spec.channels,
            spec.sample_rate
        ));
    }
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
            .collect::<std::result::Result<_, _>>()?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<_, _>>()?,
    };
    Ok(samples)
}

/// Format a SystemTime as ISO-8601 UTC (`2026-04-25T13-22-09Z`).
/// Hyphens used in time portion to keep filenames sane (matches new_session_id format).
fn iso8601_utc(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = unix_to_ymdhms(secs as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn unix_to_ymdhms(t: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = t.div_euclid(86_400);
    let secs_of_day = t.rem_euclid(86_400) as u32;
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d, h, mi, s)
}
