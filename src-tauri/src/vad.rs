/// Simple energy-based VAD with hysteresis.
/// Works on 30ms frames at 16kHz mono f32 (= 480 samples/frame).
pub struct Vad {
    state: State,
    speech_frames: u32,
    silence_frames: u32,
    start_thresh: f32,
    end_thresh: f32,
    min_speech_frames: u32,   // must exceed before we commit to speech
    end_silence_frames: u32,  // consecutive silence frames to end phrase
    max_phrase_frames: u32,   // hard cap to avoid runaway
    current_phrase_frames: u32,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum State {
    Idle,
    Speech,
}

#[derive(Debug)]
pub enum Event {
    SpeechStarted,
    SpeechEnded,
    MaxLenReached,
}

impl Default for Vad {
    fn default() -> Self {
        Self {
            state: State::Idle,
            speech_frames: 0,
            silence_frames: 0,
            start_thresh: 0.010,
            end_thresh: 0.005,
            min_speech_frames: 3,   // ~90ms
            end_silence_frames: 25, // ~750ms — long enough for natural pauses
            max_phrase_frames: 1000, // ~30s hard cap
            current_phrase_frames: 0,
        }
    }
}

impl Vad {
    pub fn process_frame(&mut self, frame: &[f32]) -> Option<Event> {
        let rms = rms(frame);
        match self.state {
            State::Idle => {
                if rms >= self.start_thresh {
                    self.speech_frames += 1;
                    if self.speech_frames >= self.min_speech_frames {
                        self.state = State::Speech;
                        self.silence_frames = 0;
                        self.current_phrase_frames = self.speech_frames;
                        return Some(Event::SpeechStarted);
                    }
                } else {
                    self.speech_frames = 0;
                }
                None
            }
            State::Speech => {
                self.current_phrase_frames += 1;
                if rms < self.end_thresh {
                    self.silence_frames += 1;
                    if self.silence_frames >= self.end_silence_frames {
                        self.state = State::Idle;
                        self.speech_frames = 0;
                        self.silence_frames = 0;
                        self.current_phrase_frames = 0;
                        return Some(Event::SpeechEnded);
                    }
                } else {
                    self.silence_frames = 0;
                }
                if self.current_phrase_frames >= self.max_phrase_frames {
                    self.state = State::Idle;
                    self.speech_frames = 0;
                    self.silence_frames = 0;
                    self.current_phrase_frames = 0;
                    return Some(Event::MaxLenReached);
                }
                None
            }
        }
    }
}

fn rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f32 = frame.iter().map(|s| s * s).sum();
    (sum / frame.len() as f32).sqrt()
}

pub const FRAME_SAMPLES_16K: usize = 480; // 30ms at 16kHz
