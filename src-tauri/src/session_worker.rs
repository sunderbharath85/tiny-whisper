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
use crate::state::{ActiveSession, AppState};
use crate::transcriber::SpeakerTurn;
use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};

const SAMPLE_RATE: u32 = 16_000;

/// How often the writer rewrites the WAV header while recording, so a killed
/// process still leaves a playable file (C-02).
const FLUSH_EVERY: Duration = Duration::from_secs(1);

/// Spawn a writer thread for an in-progress recording.
pub fn spawn_writer(
    app: AppHandle,
    rx: Receiver<WavMsg>,
    session_id: String,
    started_at: SystemTime,
) {
    let state = app.state::<AppState>();
    let app_data = state.app_data_dir.clone();
    let release = ReleaseSession {
        id: session_id.clone(),
        active: state.active_session.clone(),
    };
    std::thread::spawn(move || {
        match write_session(&app_data, rx, &session_id, started_at, release) {
            Ok(meta) => {
                let _ = app.emit("session://updated", &meta);
            }
            Err(e) => {
                log::error!("session writer ({session_id}) failed: {e}");
                emit_status(
                    &app,
                    AppStatus::Error {
                        message: format!("session write failed: {e}"),
                    },
                );
                // Show whatever part of the recording was salvaged.
                if let Ok(meta) = sessions::read_meta(&app_data, &session_id) {
                    if !meta.in_progress {
                        let _ = app.emit("session://updated", &meta);
                    }
                }
            }
        }
    });
}

/// Run the writer, then always release the session: `release` clears
/// `active_session` on success, error or panic (C-09). On error, whatever
/// reached disk is salvaged as a finished session.
fn write_session(
    app_data: &Path,
    rx: Receiver<WavMsg>,
    session_id: &str,
    started_at: SystemTime,
    release: ReleaseSession,
) -> Result<SessionMeta> {
    let _release = release;
    run_writer(app_data, rx, session_id, started_at, FLUSH_EVERY).inspect_err(|_| {
        if let Err(e) = sessions::recover_session(app_data, session_id) {
            log::warn!("could not salvage session {session_id}: {e}");
        }
    })
}

/// Write `rx` to the session's `audio.wav` until `Stop` (or the recorder
/// hangs up). A provisional meta.json goes first and the WAV header is
/// flushed every `flush_every`, so a quit or crash leaves a recoverable
/// session (C-02); the final meta replaces it at the end.
fn run_writer(
    app_data: &Path,
    rx: Receiver<WavMsg>,
    session_id: &str,
    started_at: SystemTime,
    flush_every: Duration,
) -> Result<SessionMeta> {
    let dir = session_dir(app_data, session_id)?;
    std::fs::create_dir_all(&dir)?;
    let wav_path = audio_path(app_data, session_id)?;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&wav_path, spec)?;
    let mut meta = SessionMeta {
        id: session_id.to_string(),
        created_at: iso8601_utc(started_at),
        duration_secs: 0.0,
        model_used: None,
        speaker_count: None,
        has_transcript: false,
        in_progress: true,
    };
    sessions::write_meta(app_data, &meta)?;
    let mut total_samples: u64 = 0;
    let mut last_flush = Instant::now();

    loop {
        match rx.recv_timeout(flush_every) {
            Ok(WavMsg::Chunk(chunk)) => {
                for s in &chunk {
                    let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                    writer.write_sample(v)?;
                }
                total_samples += chunk.len() as u64;
            }
            Ok(WavMsg::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if last_flush.elapsed() >= flush_every {
            writer.flush()?;
            last_flush = Instant::now();
        }
    }
    writer.finalize()?;

    meta.duration_secs = total_samples as f32 / SAMPLE_RATE as f32;
    meta.in_progress = false;
    sessions::write_meta(app_data, &meta)?;
    Ok(meta)
}

/// Dropped when a writer ends, however it ends: clears `active_session` if it
/// still names this session.
struct ReleaseSession {
    id: String,
    active: Arc<Mutex<Option<ActiveSession>>>,
}

impl Drop for ReleaseSession {
    fn drop(&mut self) {
        let mut active = self.active.lock();
        if active.as_ref().is_some_and(|a| a.id == self.id) {
            *active = None;
        }
    }
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

    let wav_path = audio_path(&app_data, session_id)?;
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

#[cfg(test)]
mod writer_tests {
    use super::*;
    use crate::sessions::tests::TempAppData;
    use std::sync::mpsc::channel;

    const ID: &str = "2026-04-24T15-12-09Z";

    type Active = Arc<Mutex<Option<ActiveSession>>>;

    fn active(id: &str) -> Active {
        Arc::new(Mutex::new(Some(ActiveSession {
            id: id.into(),
            started_at: SystemTime::now(),
        })))
    }

    fn release(id: &str, active: &Active) -> ReleaseSession {
        ReleaseSession {
            id: id.into(),
            active: active.clone(),
        }
    }

    /// Poll until `done` holds; the writer runs on another thread.
    fn eventually(what: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wav_len(path: &Path) -> Option<u32> {
        hound::WavReader::open(path).ok().map(|r| r.len())
    }

    #[test]
    fn writer_keeps_wav_and_provisional_meta_valid_while_recording() {
        assert!(FLUSH_EVERY <= Duration::from_secs(2));
        let tmp = TempAppData::new("writer-flush");
        let app_data = tmp.0.clone();
        let (tx, rx) = channel();
        let writer = std::thread::spawn(move || {
            let flush_every = Duration::from_millis(20);
            run_writer(&app_data, rx, ID, SystemTime::now(), flush_every)
        });
        let app_data = &tmp.0;
        let audio = audio_path(app_data, ID).unwrap();

        // What a crash mid-recording would leave: the header already counts
        // the audio, and the meta says "in progress".
        tx.send(WavMsg::Chunk(vec![0.25; 16_000])).unwrap();
        eventually("a flushed header", || wav_len(&audio) == Some(16_000));
        assert!(sessions::read_meta(app_data, ID).unwrap().in_progress);
        assert!(sessions::list(app_data).unwrap().is_empty());

        tx.send(WavMsg::Chunk(vec![0.25; 8_000])).unwrap();
        tx.send(WavMsg::Stop).unwrap();
        let meta = writer.join().unwrap().unwrap();
        assert!(!meta.in_progress);
        assert_eq!(meta.duration_secs, 1.5);
        assert_eq!(wav_len(&audio), Some(24_000));
        let listed = sessions::list(app_data).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].in_progress);
    }

    #[test]
    fn writer_error_still_clears_active_session() {
        let tmp = TempAppData::new("writer-error");
        // A file where the session folder should go: create_dir_all fails.
        std::fs::write(sessions::sessions_dir(&tmp.0).join(ID), b"").unwrap();
        let active = active(ID);
        let (_tx, rx) = channel();

        let guard = release(ID, &active);
        let res = write_session(&tmp.0, rx, ID, SystemTime::now(), guard);

        assert!(res.is_err());
        assert!(active.lock().is_none(), "not cleared after an error");
    }

    #[test]
    fn finished_writer_clears_active_session() {
        let tmp = TempAppData::new("writer-done");
        let active = active(ID);
        let (tx, rx) = channel();
        tx.send(WavMsg::Chunk(vec![0.1; 1_600])).unwrap();
        tx.send(WavMsg::Stop).unwrap();

        let guard = release(ID, &active);
        let meta = write_session(&tmp.0, rx, ID, SystemTime::now(), guard).unwrap();

        assert_eq!(meta.duration_secs, 0.1);
        assert!(active.lock().is_none());
    }

    #[test]
    fn release_clears_on_panic_and_spares_a_newer_session() {
        let active = active(ID);
        let guard = release(ID, &active);
        let crashed = std::thread::spawn(move || {
            let _guard = guard;
            panic!("writer panicked");
        });
        assert!(crashed.join().is_err());
        assert!(active.lock().is_none());

        // A late release from an old writer must not end the current session.
        let current = "2026-04-24T15-12-10Z";
        *active.lock() = Some(ActiveSession {
            id: current.into(),
            started_at: SystemTime::now(),
        });
        drop(release(ID, &active));
        assert_eq!(active.lock().as_ref().unwrap().id, current);
    }
}
