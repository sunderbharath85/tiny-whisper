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
}

#[derive(Clone, Copy, Debug)]
pub enum RecorderEvent {
    SessionStarted,
    Listening,     // VAD idle, waiting for speech
    SpeechStarted, // VAD detected speech
    SessionStopped,
    Error,
}

/// A completed phrase ready to transcribe, in 16kHz mono f32.
pub type Segment = Vec<f32>;

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
}

struct Session {
    _stream: cpal::Stream,
    buf: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
    cursor: usize, // next unread sample index in raw buffer
    // Resampled-but-unsegmented samples (16kHz mono).
    pending_16k: Vec<f32>,
    vad: Vad,
    // Samples accumulated for the current phrase (16kHz mono).
    phrase: Vec<f32>,
    pre_roll: Vec<f32>, // last few frames kept to prepend at speech start
}

const PRE_ROLL_FRAMES: usize = 5; // 150ms pre-roll prepended to phrase start

fn run(cmd_rx: Receiver<Cmd>, seg_tx: Sender<Segment>, evt_tx: Sender<RecorderEvent>) {
    let mut session: Option<Session> = None;
    loop {
        match cmd_rx.try_recv() {
            Ok(Cmd::StartSession) => {
                if session.is_none() {
                    match start_session() {
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
                if let Some(mut s) = session.take() {
                    // Drain any remaining phrase as a final segment.
                    drain_tick(&mut s, &seg_tx, &evt_tx);
                    if !s.phrase.is_empty() {
                        let final_seg = std::mem::take(&mut s.phrase);
                        let _ = seg_tx.send(final_seg);
                    }
                    let _ = evt_tx.send(RecorderEvent::SessionStopped);
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

fn start_session() -> Result<Session> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("no default input device"))?;
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let buf: Arc<Mutex<Vec<f32>>> =
        Arc::new(Mutex::new(Vec::with_capacity(sample_rate as usize * 10)));
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

    Ok(Session {
        _stream: stream,
        buf,
        sample_rate,
        channels,
        cursor: 0,
        pending_16k: Vec::new(),
        vad: Vad::default(),
        phrase: Vec::new(),
        pre_roll: Vec::with_capacity(PRE_ROLL_FRAMES * FRAME_SAMPLES_16K),
    })
}

fn drain_tick(s: &mut Session, seg_tx: &Sender<Segment>, evt_tx: &Sender<RecorderEvent>) {
    // Pull any new raw samples since last tick.
    let new_raw: Vec<f32> = {
        let guard = s.buf.lock();
        if guard.len() <= s.cursor {
            return;
        }
        let slice = guard[s.cursor..].to_vec();
        s.cursor = guard.len();
        slice
    };
    if new_raw.is_empty() {
        return;
    }

    // Downmix to mono, resample to 16kHz, append to pending.
    let mono = to_mono(&new_raw, s.channels);
    let mono_16k = if s.sample_rate == TARGET_SR {
        mono
    } else {
        match resample(&mono, s.sample_rate, TARGET_SR) {
            Ok(v) => v,
            Err(e) => {
                log::error!("resample failed: {e}");
                return;
            }
        }
    };
    s.pending_16k.extend_from_slice(&mono_16k);

    // Consume complete 480-sample frames through VAD.
    while s.pending_16k.len() >= FRAME_SAMPLES_16K {
        let frame: Vec<f32> = s.pending_16k.drain(..FRAME_SAMPLES_16K).collect();
        if s.vad_is_speaking() {
            s.phrase.extend_from_slice(&frame);
        } else {
            // Maintain pre-roll ring.
            s.pre_roll.extend_from_slice(&frame);
            let cap = PRE_ROLL_FRAMES * FRAME_SAMPLES_16K;
            if s.pre_roll.len() > cap {
                let drop = s.pre_roll.len() - cap;
                s.pre_roll.drain(..drop);
            }
        }
        match s.vad.process_frame(&frame) {
            Some(crate::vad::Event::SpeechStarted) => {
                // Prepend pre-roll to start of phrase (speech frames weren't added yet
                // during Idle; the transition frame itself needs to be in phrase too).
                let mut seeded = s.pre_roll.clone();
                seeded.extend_from_slice(&frame);
                s.phrase = seeded;
                s.pre_roll.clear();
                let _ = evt_tx.send(RecorderEvent::SpeechStarted);
            }
            Some(crate::vad::Event::SpeechEnded)
            | Some(crate::vad::Event::MaxLenReached) => {
                let seg = std::mem::take(&mut s.phrase);
                if !seg.is_empty() {
                    let _ = seg_tx.send(seg);
                }
                let _ = evt_tx.send(RecorderEvent::Listening);
            }
            None => {}
        }
    }
}

impl Session {
    fn vad_is_speaking(&self) -> bool {
        !self.phrase.is_empty()
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

fn resample(input: &[f32], from: u32, to: u32) -> Result<Vec<f32>> {
    let params = SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 128,
        window: WindowFunction::BlackmanHarris2,
    };
    let ratio = to as f64 / from as f64;
    let mut resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, input.len(), 1)?;
    let out = resampler.process(&[input.to_vec()], None)?;
    Ok(out.into_iter().next().unwrap_or_default())
}
