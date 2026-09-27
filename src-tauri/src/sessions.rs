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
    /// True from the moment a recording starts until its writer finishes. A
    /// session still in progress at startup was cut off by a quit or crash
    /// and is repaired by `recover` (C-02). Older meta files lack the field.
    #[serde(default)]
    pub in_progress: bool,
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

/// Session ids come from the webview over IPC, so they are checked before they
/// are joined into a path. Only the format `new_session_id` generates is
/// accepted (`YYYY-MM-DDTHH-MM-SSZ`, ASCII digits); that rules out empty ids,
/// `.`/`..`, path separators, drive letters and UNC prefixes.
pub fn validate_session_id(id: &str) -> Result<()> {
    let b = id.as_bytes();
    let ok = b.len() == 20
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 | 13 | 16 => c == b'-',
            10 => c == b'T',
            19 => c == b'Z',
            _ => c.is_ascii_digit(),
        });
    if ok {
        Ok(())
    } else {
        Err(anyhow!("invalid session id: {id:?}"))
    }
}

/// Every per-session path goes through here, so every accessor validates `id`.
pub fn session_dir(app_data: &Path, id: &str) -> Result<PathBuf> {
    validate_session_id(id)?;
    Ok(sessions_dir(app_data).join(id))
}

pub fn audio_path(app_data: &Path, id: &str) -> Result<PathBuf> {
    Ok(session_dir(app_data, id)?.join(AUDIO_FILENAME))
}

pub fn transcript_path(app_data: &Path, id: &str) -> Result<PathBuf> {
    Ok(session_dir(app_data, id)?.join(TRANSCRIPT_FILENAME))
}

pub fn meta_path(app_data: &Path, id: &str) -> Result<PathBuf> {
    Ok(session_dir(app_data, id)?.join(META_FILENAME))
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
    let dir = session_dir(app_data, &meta.id)?;
    std::fs::create_dir_all(&dir)?;
    let p = meta_path(app_data, &meta.id)?;
    std::fs::write(p, serde_json::to_vec_pretty(meta)?)?;
    Ok(())
}

pub fn read_meta(app_data: &Path, id: &str) -> Result<SessionMeta> {
    let p = meta_path(app_data, id)?;
    let s = std::fs::read_to_string(&p)?;
    Ok(serde_json::from_str(&s)?)
}

pub fn write_transcript(app_data: &Path, id: &str, t: &Transcript) -> Result<()> {
    let p = transcript_path(app_data, id)?;
    std::fs::write(p, serde_json::to_vec_pretty(t)?)?;
    Ok(())
}

pub fn read_transcript(app_data: &Path, id: &str) -> Result<Transcript> {
    let p = transcript_path(app_data, id)?;
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
            Ok(meta) if meta.in_progress => log::debug!("skipping in-progress session {id}"),
            Ok(meta) => out.push(meta),
            Err(e) => log::warn!("skipping malformed session {id}: {e}"),
        }
    }
    // Newest first.
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

pub fn delete(app_data: &Path, id: &str) -> Result<()> {
    let dir = session_dir(app_data, id)?;
    if !dir.exists() {
        return Err(anyhow!("session not found: {id}"));
    }
    ensure_in_sessions_dir(app_data, &dir)?;
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

/// Startup recovery (C-02). A session folder with `audio.wav` whose
/// `meta.json` is missing, unreadable or still in progress was cut off by a
/// quit or crash: repair its WAV header and write a finished meta, so it shows
/// up in `list()`. Folders whose names are not session ids are left alone.
/// Returns the recovered ids.
pub fn recover(app_data: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(sessions_dir(app_data)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if validate_session_id(&id).is_err() || !needs_recovery(app_data, &id) {
            continue;
        }
        match recover_session(app_data, &id) {
            Ok(meta) => {
                log::info!("recovered session {id} ({:.1}s)", meta.duration_secs);
                out.push(id);
            }
            Err(e) => log::warn!("could not recover session {id}: {e}"),
        }
    }
    out
}

fn needs_recovery(app_data: &Path, id: &str) -> bool {
    let has_audio = audio_path(app_data, id).is_ok_and(|p| p.is_file());
    has_audio && !matches!(read_meta(app_data, id), Ok(m) if !m.in_progress)
}

/// Rebuild a finished meta for `id` from its WAV, repairing the header first.
/// `created_at` comes from the provisional meta if there is one, else the id.
pub fn recover_session(app_data: &Path, id: &str) -> Result<SessionMeta> {
    let audio = audio_path(app_data, id)?;
    repair_wav_header(&audio)?;
    let reader = hound::WavReader::open(&audio)?;
    let duration_secs = reader.duration() as f32 / reader.spec().sample_rate as f32;
    let created_at = match read_meta(app_data, id) {
        Ok(m) => m.created_at,
        Err(_) => created_at_from_id(id),
    };
    let transcript = read_transcript(app_data, id).ok();
    let meta = SessionMeta {
        id: id.to_string(),
        created_at,
        duration_secs,
        model_used: transcript.as_ref().map(|t| t.model),
        speaker_count: None,
        has_transcript: transcript.is_some(),
        in_progress: false,
    };
    write_meta(app_data, &meta)?;
    Ok(meta)
}

/// `2026-04-24T15-12-09Z` -> `2026-04-24T15:12:09Z` (the writer's format).
/// `id` must already be valid.
fn created_at_from_id(id: &str) -> String {
    format!("{}{}", &id[..11], id[11..].replace('-', ":"))
}

/// Make the RIFF and `data` sizes in a WAV header match the bytes on disk. A
/// writer killed before `finalize` leaves them at its last `flush` (or 0), but
/// the samples written after that are still in the file. A trailing partial
/// frame is cut off.
fn repair_wav_header(path: &Path) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom, Write};

    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let file_len = f.metadata()?.len();
    let mut riff = [0u8; 12];
    f.read_exact(&mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err(anyhow!("not a WAV file"));
    }
    // Walk the chunks up to `data`, taking the frame size from `fmt `.
    let mut pos = 12u64;
    let mut block_align = 0u64;
    let data_start = loop {
        let mut chunk = [0u8; 8];
        f.seek(SeekFrom::Start(pos))?;
        f.read_exact(&mut chunk)
            .map_err(|_| anyhow!("WAV has no data chunk"))?;
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;
        match &chunk[0..4] {
            b"data" => break pos + 8,
            b"fmt " => {
                let mut fmt = [0u8; 14];
                f.read_exact(&mut fmt)?;
                block_align = u16::from_le_bytes([fmt[12], fmt[13]]) as u64;
            }
            _ => {}
        }
        pos += 8 + size + (size & 1);
    };
    if block_align == 0 {
        return Err(anyhow!("WAV has no usable fmt chunk"));
    }

    let data_len = file_len.saturating_sub(data_start) / block_align * block_align;
    let riff_len =
        u32::try_from(data_start - 8 + data_len).map_err(|_| anyhow!("WAV too large to repair"))?;
    if data_start + data_len < file_len {
        f.set_len(data_start + data_len)?;
    }
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&riff_len.to_le_bytes())?;
    f.seek(SeekFrom::Start(data_start - 4))?;
    f.write_all(&(data_len as u32).to_le_bytes())?;
    Ok(())
}

/// Second guard before a recursive delete: the resolved target must be a
/// direct child of the resolved sessions dir, whatever the path looks like.
fn ensure_in_sessions_dir(app_data: &Path, dir: &Path) -> Result<()> {
    let root = std::fs::canonicalize(sessions_dir(app_data))?;
    let target = std::fs::canonicalize(dir)?;
    if target.parent() != Some(root.as_path()) {
        return Err(anyhow!(
            "refusing to delete {}: not inside {}",
            target.display(),
            root.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const VALID_ID: &str = "2026-04-24T15-12-09Z";

    /// Ids that must never reach the filesystem (criterion 2.2).
    const BAD_IDS: &[&str] = &[
        "",
        "..",
        ".",
        "../x",
        "a/b",
        "a\\b",
        "C:\\Windows",
        "\\\\server\\share",
        "2026-04-24T15-12-09Z/..",
        "2026-04-24T15-12-09Z\\..",
    ];

    /// Near misses of the generated format.
    const MALFORMED_IDS: &[&str] = &[
        "2026-04-24T15-12-9Z",
        "2026-04-24T15-12-099",
        "2026-04-24 15-12-09Z",
        "2026-04-24T15-12-09z",
        "2026-04-24T15:12:09Z",
        "2026-04-24T15-12-09Z ",
        "x2026-04-24T15-12-09Z",
        "\u{ff12}026-04-24T15-12-09Z",
    ];

    /// A fresh `app_data` dir (with an empty `sessions/`) under the system
    /// temp dir, removed on drop.
    pub(crate) struct TempAppData(pub(crate) PathBuf);

    impl TempAppData {
        pub(crate) fn new(name: &str) -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "tiny-whisper-test-{name}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::SeqCst)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(sessions_dir(&dir)).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempAppData {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn meta(id: &str) -> SessionMeta {
        SessionMeta {
            id: id.to_string(),
            created_at: "2026-04-24T15:12:09Z".into(),
            duration_secs: 1.0,
            model_used: None,
            speaker_count: None,
            has_transcript: false,
            in_progress: false,
        }
    }

    fn provisional(id: &str) -> SessionMeta {
        SessionMeta {
            duration_secs: 0.0,
            in_progress: true,
            ..meta(id)
        }
    }

    fn sample(i: usize) -> i16 {
        (i % 1000) as i16
    }

    /// A 16kHz mono PCM16 WAV as a writer killed after its last flush leaves
    /// it: the header counts `in_header` samples, the file holds `on_disk`.
    fn write_cut_off_wav(path: &Path, in_header: usize, on_disk: usize) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..in_header {
            w.write_sample(sample(i)).unwrap();
        }
        w.finalize().unwrap();
        let tail: Vec<u8> = (in_header..on_disk)
            .flat_map(|i| sample(i).to_le_bytes())
            .collect();
        append_bytes(path, &tail);
    }

    fn append_bytes(path: &Path, bytes: &[u8]) {
        let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    /// The RIFF and data sizes as `WavWriter::create` leaves them: 0.
    fn zero_header_sizes(path: &Path) {
        let mut bytes = std::fs::read(path).unwrap();
        bytes[4..8].fill(0);
        bytes[40..44].fill(0);
        std::fs::write(path, bytes).unwrap();
    }

    fn samples_in(path: &Path) -> Vec<i16> {
        hound::WavReader::open(path)
            .unwrap()
            .samples::<i16>()
            .map(|s| s.unwrap())
            .collect()
    }

    fn transcript() -> Transcript {
        Transcript {
            model: ModelId::TinyEn,
            diarized: false,
            segments: Vec::new(),
        }
    }

    /// The error must come from the validator, not from a failed file op.
    fn assert_invalid_id<T: std::fmt::Debug>(id: &str, r: Result<T>) {
        let e = r.expect_err(id);
        assert!(e.to_string().contains("invalid session id"), "{id:?}: {e}");
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn generated_id_is_accepted() {
        let id = new_session_id();
        assert!(validate_session_id(&id).is_ok(), "{id}");
        assert!(validate_session_id(VALID_ID).is_ok());
        assert_eq!(
            session_dir(Path::new("app"), &id).unwrap(),
            Path::new("app").join("sessions").join(&id)
        );
    }

    #[test]
    fn every_path_accessor_rejects_bad_ids() {
        let app = Path::new("app");
        for id in BAD_IDS.iter().chain(MALFORMED_IDS) {
            assert_invalid_id(id, validate_session_id(id));
            assert_invalid_id(id, session_dir(app, id));
            assert_invalid_id(id, audio_path(app, id));
            assert_invalid_id(id, transcript_path(app, id));
            assert_invalid_id(id, meta_path(app, id));
        }
    }

    #[test]
    fn readers_and_writers_reject_bad_ids_without_touching_disk() {
        let tmp = TempAppData::new("rw");
        let app = &tmp.0;
        for id in BAD_IDS {
            // Never hand an id to a file op unless the validator refuses it.
            assert!(validate_session_id(id).is_err(), "{id:?}");
            assert_invalid_id(id, write_meta(app, &meta(id)));
            assert_invalid_id(id, read_meta(app, id));
            assert_invalid_id(id, write_transcript(app, id, &transcript()));
            assert_invalid_id(id, read_transcript(app, id));
        }
        assert_eq!(names_in(app), vec!["sessions"]);
        assert!(names_in(&sessions_dir(app)).is_empty());
    }

    #[test]
    fn delete_rejects_bad_ids_and_deletes_nothing() {
        let tmp = TempAppData::new("delete-bad");
        let app = &tmp.0;
        // What a traversal could reach: a sibling of sessions/ (models), a
        // real session, and an absolute path outside app_data.
        let models = app.join("models");
        std::fs::create_dir_all(&models).unwrap();
        std::fs::write(models.join("model.bin"), b"x").unwrap();
        write_meta(app, &meta(VALID_ID)).unwrap();
        let outside = TempAppData::new("delete-outside");
        let outside_abs = outside.0.to_string_lossy().into_owned();

        let ids = BAD_IDS
            .iter()
            .map(|s| s.to_string())
            .chain([outside_abs, format!("{VALID_ID}/../../models")]);
        for id in ids {
            assert!(validate_session_id(&id).is_err(), "{id:?}");
            assert_invalid_id(&id, delete(app, &id));
        }

        assert!(models.join("model.bin").exists());
        assert!(meta_path(app, VALID_ID).unwrap().exists());
        assert!(sessions_dir(&outside.0).exists());
        assert_eq!(names_in(app), vec!["models", "sessions"]);
    }

    #[test]
    fn delete_removes_only_the_given_session() {
        let tmp = TempAppData::new("delete-ok");
        let app = &tmp.0;
        let id = new_session_id();
        write_meta(app, &meta(&id)).unwrap();
        write_transcript(app, &id, &transcript()).unwrap();
        write_meta(app, &meta(VALID_ID)).unwrap();

        delete(app, &id).unwrap();

        assert!(!session_dir(app, &id).unwrap().exists());
        assert!(session_dir(app, VALID_ID).unwrap().exists());
        let e = delete(app, &id).unwrap_err();
        assert!(e.to_string().contains("session not found"), "{e}");
    }

    #[test]
    fn delete_guard_rejects_targets_not_directly_in_sessions_dir() {
        let tmp = TempAppData::new("guard");
        let app = &tmp.0;
        let session = session_dir(app, VALID_ID).unwrap();
        std::fs::create_dir_all(session.join("nested")).unwrap();
        let models = app.join("models");
        std::fs::create_dir_all(&models).unwrap();
        let outside = TempAppData::new("guard-outside");

        assert!(ensure_in_sessions_dir(app, &session).is_ok());
        for target in [
            models.clone(),
            session.join("nested"),
            sessions_dir(app),
            app.clone(),
            outside.0.clone(),
            // `..` is resolved before the check.
            session.join("..").join("..").join("models"),
        ] {
            let e = ensure_in_sessions_dir(app, &target).unwrap_err();
            assert!(e.to_string().contains("refusing to delete"), "{target:?}: {e}");
        }
        assert!(models.exists() && session.join("nested").exists() && outside.0.exists());
    }

    #[test]
    fn list_skips_folders_that_are_not_session_ids() {
        let tmp = TempAppData::new("list");
        let app = &tmp.0;
        write_meta(app, &meta(VALID_ID)).unwrap();
        let stray = sessions_dir(app).join("not-a-session");
        std::fs::create_dir_all(&stray).unwrap();
        std::fs::write(
            stray.join(META_FILENAME),
            serde_json::to_vec(&meta("not-a-session")).unwrap(),
        )
        .unwrap();

        let ids: Vec<String> = list(app).unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![VALID_ID.to_string()]);
    }

    #[test]
    fn recovers_provisional_meta_with_flushed_wav() {
        let tmp = TempAppData::new("recover-provisional");
        let app = &tmp.0;
        write_meta(app, &provisional(VALID_ID)).unwrap();
        let audio = audio_path(app, VALID_ID).unwrap();
        // Last flush at 1 s, killed at 1.5 s.
        write_cut_off_wav(&audio, 16_000, 24_000);
        assert!(
            list(app).unwrap().is_empty(),
            "in-progress sessions are hidden"
        );

        assert_eq!(recover(app), vec![VALID_ID.to_string()]);

        let listed = list(app).unwrap();
        assert_eq!(listed.len(), 1);
        let m = &listed[0];
        assert_eq!(m.id, VALID_ID);
        assert!(!m.in_progress);
        assert_eq!(m.duration_secs, 1.5);
        assert_eq!(m.created_at, provisional(VALID_ID).created_at);
        let expected: Vec<i16> = (0..24_000).map(sample).collect();
        assert_eq!(samples_in(&audio), expected);
        // Nothing left to do on the next start.
        assert!(recover(app).is_empty());
    }

    #[test]
    fn recovers_missing_meta_with_unfinalized_header() {
        let tmp = TempAppData::new("recover-no-meta");
        let app = &tmp.0;
        let id = "2026-04-24T15-12-10Z";
        std::fs::create_dir_all(session_dir(app, id).unwrap()).unwrap();
        let audio = audio_path(app, id).unwrap();
        // Header never flushed, 2 s of samples plus half a sample.
        write_cut_off_wav(&audio, 0, 32_000);
        zero_header_sizes(&audio);
        append_bytes(&audio, &[0x7f]);
        assert!(list(app).unwrap().is_empty());

        assert_eq!(recover(app), vec![id.to_string()]);

        let m = read_meta(app, id).unwrap();
        assert_eq!(m.duration_secs, 2.0);
        assert_eq!(m.created_at, "2026-04-24T15:12:10Z");
        assert!(!m.in_progress && !m.has_transcript);
        assert_eq!(samples_in(&audio).len(), 32_000);
        assert_eq!(list(app).unwrap().len(), 1);
    }

    #[test]
    fn recover_leaves_finished_and_foreign_folders_alone() {
        let tmp = TempAppData::new("recover-skip");
        let app = &tmp.0;
        // Finished session: its meta is not rewritten.
        write_meta(app, &meta(VALID_ID)).unwrap();
        write_cut_off_wav(&audio_path(app, VALID_ID).unwrap(), 16_000, 16_000);
        let before = std::fs::read(meta_path(app, VALID_ID).unwrap()).unwrap();
        // Not a session id: never touched.
        let stray = sessions_dir(app).join("not-a-session");
        std::fs::create_dir_all(&stray).unwrap();
        write_cut_off_wav(&stray.join(AUDIO_FILENAME), 0, 1_600);
        // Session folder without audio: nothing to recover.
        let no_audio = "2026-04-24T15-12-11Z";
        std::fs::create_dir_all(session_dir(app, no_audio).unwrap()).unwrap();
        // Unreadable audio: logged and skipped, meta stays provisional.
        let broken = "2026-04-24T15-12-12Z";
        write_meta(app, &provisional(broken)).unwrap();
        std::fs::write(audio_path(app, broken).unwrap(), b"not a wav").unwrap();

        assert!(recover(app).is_empty());

        let after = std::fs::read(meta_path(app, VALID_ID).unwrap()).unwrap();
        assert_eq!(after, before);
        assert!(!stray.join(META_FILENAME).exists());
        assert!(!meta_path(app, no_audio).unwrap().exists());
        assert!(read_meta(app, broken).unwrap().in_progress);
    }

    #[test]
    fn meta_without_in_progress_field_is_finished() {
        let json = r#"{"id":"2026-04-24T15-12-09Z","created_at":"2026-04-24T15:12:09Z",
            "duration_secs":1.0,"model_used":null,"speaker_count":null,"has_transcript":false}"#;
        let m: SessionMeta = serde_json::from_str(json).unwrap();
        assert!(!m.in_progress);
    }
}
