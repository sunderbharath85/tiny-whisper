use crate::vad::{Vad, FRAME_SAMPLES_16K};
use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

pub const TARGET_SR: u32 = 16_000;

pub enum Cmd {
    StartSession,
    StopSession,
    /// Begin a raw 16kHz mono capture, forwarding chunks to `tx`. Mutually
    /// exclusive with VAD-driven dictation; the recorder will refuse the
    /// command if a dictation session is already active.
    StartRawCapture(Sender<WavMsg>),
    StopRawCapture,
}

#[derive(Clone, Copy, Debug)]
pub enum RecorderEvent {
    SessionStarted,
    Listening,     // VAD idle, waiting for speech
    SpeechStarted, // VAD detected speech
    SessionStopped,
    /// Raw capture started (used by the session-recording feature).
    RawCaptureStarted,
    RawCaptureStopped,
    Error,
}

/// A completed phrase ready to transcribe, in 16kHz mono f32.
pub type Segment = Vec<f32>;

/// Messages sent to a session writer thread that owns a WavWriter.
pub enum WavMsg {
    Chunk(Vec<f32>),
    Stop,
}

pub struct Recorder {
    cmd_tx: Sender<Cmd>,
}

impl Recorder {
    pub fn spawn(seg_tx: Sender<Segment>, evt_tx: Sender<RecorderEvent>) -> Self {
        let (cmd_tx, cmd_rx) = channel::<Cmd>();
        std::thread::spawn(move || {
            run(cmd_rx, seg_tx, evt_tx);
        });
        Self { cmd_tx }
    }

    pub fn start_session(&self) -> Result<()> {
        self.cmd_tx
            .send(Cmd::StartSession)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub fn stop_session(&self) -> Result<()> {
        self.cmd_tx
            .send(Cmd::StopSession)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub fn start_raw_capture(&self, tx: Sender<WavMsg>) -> Result<()> {
        self.cmd_tx
            .send(Cmd::StartRawCapture(tx))
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    pub fn stop_raw_capture(&self) -> Result<()> {
        self.cmd_tx
            .send(Cmd::StopRawCapture)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }
}

enum SessionMode {
    /// VAD-driven dictation — splits audio into phrases on silence.
    Vad {
        vad: Vad,
        phrase: Vec<f32>,
        pre_roll: Vec<f32>,
    },
    /// Raw capture — stream every 16kHz mono frame to `tx`.
    Raw { tx: Sender<WavMsg> },
}

struct Session {
    /// `None` once stopped (and in tests, which have no audio device).
    stream: Option<cpal::Stream>,
    /// Filled by the cpal callback, emptied by every `drain_tick`.
    buf: Arc<Mutex<Vec<f32>>>,
    /// Takes the captured samples each tick; its allocation is swapped back
    /// into `buf`, so the callback reuses it instead of reallocating.
    raw: Vec<f32>,
    channels: u16,
    /// One resampler for the whole session; `None` when the device is 16kHz.
    resampler: Option<StreamResampler>,
    pending_16k: Vec<f32>,
    mode: SessionMode,
}

const PRE_ROLL_FRAMES: usize = 5; // 150ms pre-roll prepended to phrase start
const RESAMPLER_CHUNK_MS: usize = 10; // fixed resampler input chunk

fn run(cmd_rx: Receiver<Cmd>, seg_tx: Sender<Segment>, evt_tx: Sender<RecorderEvent>) {
    let mut session: Option<Session> = None;
    loop {
        match cmd_rx.try_recv() {
            Ok(Cmd::StartSession) => {
                if session.is_none() {
                    match start_session(SessionMode::Vad {
                        vad: Vad::default(),
                        phrase: Vec::new(),
                        pre_roll: Vec::with_capacity(PRE_ROLL_FRAMES * FRAME_SAMPLES_16K),
                    }) {
                        Ok(s) => {
                            session = Some(s);
                            let _ = evt_tx.send(RecorderEvent::SessionStarted);
                            let _ = evt_tx.send(RecorderEvent::Listening);
                        }
                        Err(e) => {
                            log::error!("start_session: {e}");
                            let _ = evt_tx.send(RecorderEvent::Error);
                        }
                    }
                }
            }
            Ok(Cmd::StopSession) => {
                if let Some(s) = session.take() {
                    end_session(s, RecorderEvent::SessionStopped, &seg_tx, &evt_tx);
                }
            }
            Ok(Cmd::StartRawCapture(tx)) => {
                if session.is_some() {
                    log::warn!("ignoring StartRawCapture: a session is already active");
                    let _ = tx.send(WavMsg::Stop);
                } else {
                    match start_session(SessionMode::Raw { tx: tx.clone() }) {
                        Ok(s) => {
                            session = Some(s);
                            let _ = evt_tx.send(RecorderEvent::RawCaptureStarted);
                        }
                        Err(e) => {
                            log::error!("start_raw_capture: {e}");
                            let _ = tx.send(WavMsg::Stop);
                            let _ = evt_tx.send(RecorderEvent::Error);
                        }
                    }
                }
            }
            Ok(Cmd::StopRawCapture) => {
                if let Some(s) = session.take() {
                    end_session(s, RecorderEvent::RawCaptureStopped, &seg_tx, &evt_tx);
                }
            }
            Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {}
        }

        if let Some(s) = &mut session {
            drain_tick(s, &seg_tx, &evt_tx);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Stop a session on request: flush everything still buffered, tell the
/// writer (Raw mode) and report `stopped`.
fn end_session(
    mut s: Session,
    stopped: RecorderEvent,
    seg_tx: &Sender<Segment>,
    evt_tx: &Sender<RecorderEvent>,
) {
    finish(&mut s, seg_tx, evt_tx);
    let _ = evt_tx.send(stopped);
}

fn start_session(mode: SessionMode) -> Result<Session> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device"))?;
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let resampler = new_resampler(sample_rate)?;
    let buf = Arc::new(Mutex::new(capture_buffer(sample_rate, channels)));
    let buf_cb = buf.clone();
    let err_fn = |e| log::error!("cpal stream error: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| buf_cb.lock().extend_from_slice(data),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _| {
                let mut b = buf_cb.lock();
                b.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _| {
                let mut b = buf_cb.lock();
                b.extend(data.iter().map(|&s| (s as f32 - 32768.0) / 32768.0));
            },
            err_fn,
            None,
        )?,
        fmt => return Err(anyhow!("unsupported sample format: {fmt:?}")),
    };
    stream.play()?;

    Ok(Session::new(Some(stream), buf, channels, resampler, mode))
}

impl Session {
    fn new(
        stream: Option<cpal::Stream>,
        buf: Arc<Mutex<Vec<f32>>>,
        channels: u16,
        resampler: Option<StreamResampler>,
        mode: SessionMode,
    ) -> Self {
        let raw = Vec::with_capacity(buf.lock().capacity());
        Self {
            stream,
            buf,
            raw,
            channels,
            resampler,
            pending_16k: Vec::new(),
            mode,
        }
    }
}

/// Room for ~1s of interleaved input. It is emptied every ~20ms tick, so the
/// callback normally never has to grow it.
fn capture_buffer(sample_rate: u32, channels: u16) -> Vec<f32> {
    Vec::with_capacity(sample_rate as usize * channels.max(1) as usize)
}

/// Move everything captured since the last tick into `out`, leaving the
/// shared buffer empty (P-01). `out`'s old allocation goes back to the
/// callback, so in steady state neither side allocates.
fn take_captured(buf: &Mutex<Vec<f32>>, out: &mut Vec<f32>) {
    out.clear();
    std::mem::swap(&mut *buf.lock(), out);
}

fn drain_tick(s: &mut Session, seg_tx: &Sender<Segment>, evt_tx: &Sender<RecorderEvent>) {
    take_captured(&s.buf, &mut s.raw);
    if s.raw.is_empty() {
        return;
    }

    let mono = to_mono(&s.raw, s.channels);
    let mono_16k = match &mut s.resampler {
        None => mono,
        Some(r) => match r.process(&mono) {
            Ok(v) => v,
            Err(e) => {
                log::error!("resample failed: {e}");
                return;
            }
        },
    };
    deliver(s, mono_16k, seg_tx, evt_tx)
}

/// Stop the device, then push everything still buffered through the session:
/// the last callback's samples, the resampler tail (C-01) and, if a phrase is
/// open, the last partial frame with the phrase itself.
fn finish(s: &mut Session, seg_tx: &Sender<Segment>, evt_tx: &Sender<RecorderEvent>) {
    s.stream = None;
    drain_tick(s, seg_tx, evt_tx);
    let tail = match &mut s.resampler {
        Some(r) => r.flush().unwrap_or_else(|e| {
            log::error!("resampler flush failed: {e}");
            Vec::new()
        }),
        None => Vec::new(),
    };
    deliver(s, tail, seg_tx, evt_tx);

    match &mut s.mode {
        SessionMode::Vad { phrase, .. } => {
            if !phrase.is_empty() {
                phrase.extend_from_slice(&s.pending_16k);
                let _ = seg_tx.send(std::mem::take(phrase));
            }
        }
        SessionMode::Raw { tx } => {
            let _ = tx.send(WavMsg::Stop);
        }
    }
    s.pending_16k.clear();
}

/// Route a batch of 16kHz mono audio to the session's consumer.
fn deliver(
    s: &mut Session,
    mono_16k: Vec<f32>,
    seg_tx: &Sender<Segment>,
    evt_tx: &Sender<RecorderEvent>,
) {
    if mono_16k.is_empty() {
        return;
    }
    match &mut s.mode {
        SessionMode::Raw { tx } => {
            // Forward as-is. No frame alignment requirement.
            let _ = tx.send(WavMsg::Chunk(mono_16k));
        }
        SessionMode::Vad { vad, phrase, pre_roll } => {
            s.pending_16k.extend_from_slice(&mono_16k);
            while s.pending_16k.len() >= FRAME_SAMPLES_16K {
                let frame: Vec<f32> = s.pending_16k.drain(..FRAME_SAMPLES_16K).collect();
                let speaking = !phrase.is_empty();
                if speaking {
                    phrase.extend_from_slice(&frame);
                } else {
                    pre_roll.extend_from_slice(&frame);
                    let cap = PRE_ROLL_FRAMES * FRAME_SAMPLES_16K;
                    if pre_roll.len() > cap {
                        let drop = pre_roll.len() - cap;
                        pre_roll.drain(..drop);
                    }
                }
                match vad.process_frame(&frame) {
                    Some(crate::vad::Event::SpeechStarted) => {
                        let mut seeded = pre_roll.clone();
                        seeded.extend_from_slice(&frame);
                        *phrase = seeded;
                        pre_roll.clear();
                        let _ = evt_tx.send(RecorderEvent::SpeechStarted);
                    }
                    Some(crate::vad::Event::SpeechEnded)
                    | Some(crate::vad::Event::MaxLenReached) => {
                        let seg = std::mem::take(phrase);
                        if !seg.is_empty() {
                            let _ = seg_tx.send(seg);
                        }
                        let _ = evt_tx.send(RecorderEvent::Listening);
                    }
                    None => {}
                }
            }
        }
    }
}

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn sinc_params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    }
}

/// The session's resampler, or `None` when the device already runs at 16kHz.
fn new_resampler(sample_rate: u32) -> Result<Option<StreamResampler>> {
    if sample_rate == TARGET_SR {
        return Ok(None);
    }
    Ok(Some(StreamResampler::new(sample_rate, TARGET_SR)?))
}

/// One rubato resampler kept for a whole capture session (C-01). Input of any
/// size is buffered and fed in fixed chunks, so filter state carries across
/// ticks, and `flush` emits the tail so the output totals
/// `round(input_len * ratio)`. `SincFixedIn` starts its first output at input
/// time ~0 (within a sample), so there is no start-up delay to drop.
struct StreamResampler {
    inner: SincFixedIn<f32>,
    ratio: f64,
    /// Input not yet fed to `inner` (less than one chunk).
    pending: Vec<f32>,
    /// Output scratch, reused on every call.
    out_buf: Vec<Vec<f32>>,
    total_in: u64,
    total_out: u64,
}

impl StreamResampler {
    fn new(from: u32, to: u32) -> Result<Self> {
        let ratio = to as f64 / from as f64;
        let chunk = (from as usize * RESAMPLER_CHUNK_MS / 1000).max(64);
        let inner = SincFixedIn::<f32>::new(ratio, 1.0, sinc_params(), chunk, 1)?;
        let out_buf = inner.output_buffer_allocate(true);
        Ok(Self {
            inner,
            ratio,
            pending: Vec::with_capacity(chunk * 2),
            out_buf,
            total_in: 0,
            total_out: 0,
        })
    }

    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        self.total_in += input.len() as u64;
        self.pending.extend_from_slice(input);
        let chunk = self.inner.input_frames_next();
        let mut out = Vec::with_capacity((self.pending.len() as f64 * self.ratio) as usize + 16);
        let mut fed = 0;
        while self.pending.len() - fed >= chunk {
            let (_, n) = self.inner.process_into_buffer(
                &[&self.pending[fed..fed + chunk]],
                &mut self.out_buf,
                None,
            )?;
            self.emit(n, &mut out);
            fed += chunk;
        }
        self.pending.drain(..fed);
        Ok(out)
    }

    /// End of stream: feed the zero-padded remainder, then zero chunks until
    /// the delayed tail is out, trimmed to exactly `round(total_in * ratio)`.
    fn flush(&mut self) -> Result<Vec<f32>> {
        let expected = (self.total_in as f64 * self.ratio).round() as u64;
        let mut out = Vec::new();
        if !self.pending.is_empty() {
            let (_, n) = self.inner.process_partial_into_buffer(
                Some(&[&self.pending[..]][..]),
                &mut self.out_buf,
                None,
            )?;
            self.pending.clear();
            self.emit(n, &mut out);
        }
        // rubato holds back less than one chunk, so this ends in a call or two.
        for _ in 0..8 {
            if self.total_out >= expected {
                break;
            }
            let (_, n) = self.inner.process_partial_into_buffer(
                None::<&[&[f32]]>,
                &mut self.out_buf,
                None,
            )?;
            self.emit(n, &mut out);
        }
        let excess = self.total_out.saturating_sub(expected) as usize;
        out.truncate(out.len().saturating_sub(excess));
        self.total_out = self.total_out.min(expected);
        Ok(out)
    }

    fn emit(&mut self, n: usize, out: &mut Vec<f32>) {
        out.extend_from_slice(&self.out_buf[0][..n]);
        self.total_out += n as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// 0.5-amplitude sine, computed in f64 so phase error stays negligible.
    fn sine(freq: f64, sr: u32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (0.5 * (2.0 * PI * freq * i as f64 / sr as f64).sin()) as f32)
            .collect()
    }

    /// `sine` at 16kHz, shifted by `offset` samples.
    fn sine_at(freq: f64, n: usize, offset: f64) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let t = (i as f64 + offset) / TARGET_SR as f64;
                (0.5 * (2.0 * PI * freq * t).sin()) as f32
            })
            .collect()
    }

    /// A session with no audio device: tests act as the cpal callback by
    /// pushing into `buf` themselves.
    fn test_session(sample_rate: u32, channels: u16, mode: SessionMode) -> Session {
        let buf = Arc::new(Mutex::new(capture_buffer(sample_rate, channels)));
        let resampler = new_resampler(sample_rate).unwrap();
        Session::new(None, buf, channels, resampler, mode)
    }

    fn raw_session(sample_rate: u32, channels: u16) -> (Session, Receiver<WavMsg>) {
        let (tx, rx) = channel();
        let s = test_session(sample_rate, channels, SessionMode::Raw { tx });
        (s, rx)
    }

    /// Stands in for the cpal callback.
    fn callback(s: &Session, data: &[f32]) {
        s.buf.lock().extend_from_slice(data);
    }

    /// All chunk audio the writer got, and whether it got `Stop`.
    fn written(rx: &Receiver<WavMsg>) -> (Vec<f32>, bool) {
        let mut out = Vec::new();
        let mut stopped = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                WavMsg::Chunk(c) => out.extend(c),
                WavMsg::Stop => stopped = true,
            }
        }
        (out, stopped)
    }

    /// Run mono `input` through a Raw session one 20ms callback per tick,
    /// then stop it, and return what the writer received.
    fn capture_in_20ms_ticks(input: &[f32], sample_rate: u32) -> Vec<f32> {
        let (mut s, rx) = raw_session(sample_rate, 1);
        let (seg_tx, _seg_rx) = channel();
        let (evt_tx, _evt_rx) = channel();
        for data in input.chunks(sample_rate as usize / 50) {
            callback(&s, data);
            drain_tick(&mut s, &seg_tx, &evt_tx);
        }
        finish(&mut s, &seg_tx, &evt_tx);
        let (out, stopped) = written(&rx);
        assert!(stopped, "writer did not get Stop");
        out
    }

    /// Reference: the whole signal through a fresh resampler in one call.
    fn one_shot(input: &[f32], from: u32) -> Vec<f32> {
        let ratio = TARGET_SR as f64 / from as f64;
        let mut r = SincFixedIn::<f32>::new(ratio, 1.0, sinc_params(), input.len(), 1).unwrap();
        let mut out = r.process(&[input], None).unwrap().remove(0);
        let tail = r.process_partial(None::<&[&[f32]]>, None).unwrap();
        out.extend_from_slice(&tail[0]);
        out.truncate((input.len() as f64 * ratio).round() as usize);
        out
    }

    fn snr_db(signal: &[f32], reference: &[f32]) -> f64 {
        assert_eq!(signal.len(), reference.len());
        let (mut power, mut noise) = (0.0f64, 0.0f64);
        for (&x, &r) in signal.iter().zip(reference) {
            power += (r as f64).powi(2);
            noise += (x as f64 - r as f64).powi(2);
        }
        10.0 * (power / noise.max(1e-30)).log10()
    }

    fn longest_near_zero_run(x: &[f32]) -> usize {
        let (mut longest, mut run) = (0, 0);
        for s in x {
            run = if s.abs() < 1e-4 { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        longest
    }

    fn check_sine_through_session(from: u32) {
        let input = sine(440.0, from, from as usize); // 1s
        let out = capture_in_20ms_ticks(&input, from);
        let edge = TARGET_SR as usize / 100; // 10ms

        // (a) 16000 samples +-1%
        let n = out.len();
        assert!((15_840..=16_160).contains(&n), "{from} Hz: {n} samples out");

        // (b) no dropouts after the first 10ms
        let run = longest_near_zero_run(&out[edge..]);
        assert!(run <= 8, "{from} Hz: {run} consecutive near-zero samples");

        // (c) matches a one-shot resample, first/last 10ms excluded
        let reference = one_shot(&input, from);
        let len = n.min(reference.len());
        let snr = snr_db(&out[edge..len - edge], &reference[edge..len - edge]);
        assert!(snr >= 30.0, "{from} Hz: SNR vs one-shot {snr:.1} dB");

        // And it is the ideal 16kHz sine, aligned to within one sample.
        let best = (-20..=20)
            .map(|step| {
                let ideal = sine_at(440.0, n, step as f64 / 20.0);
                snr_db(&out[edge..n - edge], &ideal[edge..n - edge])
            })
            .fold(f64::MIN, f64::max);
        assert!(best >= 30.0, "{from} Hz: SNR vs ideal {best:.1} dB");
    }

    #[test]
    fn resample_48k_sine_in_20ms_ticks_is_continuous() {
        check_sine_through_session(48_000);
    }

    #[test]
    fn resample_44k1_sine_in_20ms_ticks_is_continuous() {
        check_sine_through_session(44_100);
    }

    #[test]
    fn device_at_16k_skips_resampling() {
        assert!(new_resampler(16_000).unwrap().is_none());
        assert!(new_resampler(48_000).unwrap().is_some());
        assert!(new_resampler(44_100).unwrap().is_some());

        // Samples pass through bit for bit, with no delay and no tail.
        let input = sine(440.0, TARGET_SR, 16_000);
        let out = capture_in_20ms_ticks(&input, TARGET_SR);
        assert_eq!(out, input);
    }

    #[test]
    fn drain_empties_capture_buffer_every_tick() {
        let (mut s, rx) = raw_session(48_000, 2);
        let (seg_tx, _seg_rx) = channel();
        let (evt_tx, _evt_rx) = channel();
        let cap = s.buf.lock().capacity();
        let data = vec![0.25f32; 480 * 2]; // 10ms of 48kHz stereo

        let ticks = 500; // 10s
        for _ in 0..ticks {
            callback(&s, &data);
            callback(&s, &data);
            drain_tick(&mut s, &seg_tx, &evt_tx);
            let buf = s.buf.lock();
            assert_eq!(buf.len(), 0, "capture buffer not drained");
            assert_eq!(buf.capacity(), cap, "callback buffer reallocated");
        }
        finish(&mut s, &seg_tx, &evt_tx);
        let (out, _) = written(&rx);
        assert_eq!(out.len(), ticks * 320); // 20ms at 16kHz per tick
    }
}
