use crate::config::ModelId;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Filesystem layout under `app_data/sessions/<id>/`:
///   audio.wav            16 kHz mono PCM16 (written by session_writer)
///   transcript.json      optional, present once `transcribe_session` finishes
///   meta.json            session metadata (duration, created_at, etc.)
const AUDIO_FILENAME: &str = "audio.wav";
const TRANSCRIPT_FILENAME: &str = "transcript.json";
const META_FILENAME: &str = "meta.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    /// ISO-8601 UTC timestamp.
    pub created_at: String,
    pub duration_secs: f32,
    pub model_used: Option<ModelId>,
    pub speaker_count: Option<u8>,
    pub has_transcript: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_secs: f32,
    pub end_secs: f32,
    pub speaker: Option<u8>,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transcript {
    pub model: ModelId,
    pub diarized: bool,
    pub segments: Vec<TranscriptSegment>,
}

pub fn sessions_dir(app_data: &Path) -> PathBuf {
    app_data.join("sessions")
}

pub fn session_dir(app_data: &Path, id: &str) -> PathBuf {
    sessions_dir(app_data).join(id)
}

pub fn audio_path(app_data: &Path, id: &str) -> PathBuf {
    session_dir(app_data, id).join(AUDIO_FILENAME)
}

pub fn transcript_path(app_data: &Path, id: &str) -> PathBuf {
    session_dir(app_data, id).join(TRANSCRIPT_FILENAME)
}

pub fn meta_path(app_data: &Path, id: &str) -> PathBuf {
    session_dir(app_data, id).join(META_FILENAME)
}

/// Generate a new session id from the current UTC time, e.g. "2026-04-24T15-12-09Z".
pub fn new_session_id() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s) = unix_to_ymdhms(secs as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}Z")
}

/// Convert a unix timestamp to (year, month, day, hour, min, sec) UTC.
/// Sufficient for filenames; not a calendar library.
fn unix_to_ymdhms(t: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = t.div_euclid(86_400);
    let secs_of_day = t.rem_euclid(86_400) as u32;
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;

    // Civil-from-days algorithm (Howard Hinnant).
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

pub fn write_meta(app_data: &Path, meta: &SessionMeta) -> Result<()> {
    let dir = session_dir(app_data, &meta.id);
    std::fs::create_dir_all(&dir)?;
    let p = meta_path(app_data, &meta.id);
    std::fs::write(p, serde_json::to_vec_pretty(meta)?)?;
    Ok(())
}

pub fn read_meta(app_data: &Path, id: &str) -> Result<SessionMeta> {
    let p = meta_path(app_data, id);
    let s = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&s)?)
}

pub fn write_transcript(app_data: &Path, id: &str, t: &Transcript) -> Result<()> {
    let p = transcript_path(app_data, id);
    std::fs::write(p, serde_json::to_vec_pretty(t)?)?;
    Ok(())
}

pub fn read_transcript(app_data: &Path, id: &str) -> Result<Transcript> {
    let p = transcript_path(app_data, id);
    let s = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&s)?)
}

pub fn list(app_data: &Path) -> Result<Vec<SessionMeta>> {
    let dir = sessions_dir(app_data);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        match read_meta(app_data, &id) {
            Ok(meta) => out.push(meta),
            Err(e) => log::warn!("skipping malformed session {id}: {e}"),
        }
    }
    // Newest first.
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub fn delete(app_data: &Path, id: &str) -> Result<()> {
    let dir = session_dir(app_data, id);
    if !dir.exists() {
        return Err(anyhow!("session not found: {id}"));
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
