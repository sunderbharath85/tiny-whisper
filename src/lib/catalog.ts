import type { AppStatus, Engine, ModelId } from "../api";

/// UI metadata for each model. Keep ids in sync with `config.rs`.
export type ModelMeta = {
  id: ModelId;
  name: string;
  engine: Engine;
  sizeLabel: string;
  /// 1–5, relative to the other models.
  speed: number;
  accuracy: number;
  englishOnly: boolean;
  blurb: string;
  recommended?: boolean;
};

export const MODELS: ModelMeta[] = [
  { id: "tiny.en", name: "Tiny", engine: "whisper", sizeLabel: "75 MB", speed: 5, accuracy: 2, englishOnly: true, blurb: "Instant, forgiving of slow machines. Good for quick notes." },
  { id: "base.en", name: "Base", engine: "whisper", sizeLabel: "142 MB", speed: 4, accuracy: 3, englishOnly: true, blurb: "The everyday choice — fast on any laptop, accurate enough for messages.", recommended: true },
  { id: "small.en", name: "Small", engine: "whisper", sizeLabel: "466 MB", speed: 3, accuracy: 4, englishOnly: true, blurb: "Noticeably better with names and jargon. Still comfortable on CPU." },
  { id: "medium.en", name: "Medium", engine: "whisper", sizeLabel: "1.5 GB", speed: 2, accuracy: 4, englishOnly: true, blurb: "High accuracy. Best with a GPU build." },
  { id: "large-v3", name: "Large v3", engine: "whisper", sizeLabel: "2.9 GB", speed: 1, accuracy: 5, englishOnly: false, blurb: "The most accurate Whisper, in 99 languages. Slow on CPU." },
  { id: "parakeet-ctc-0.6b-en", name: "Parakeet CTC", engine: "parakeet", sizeLabel: "2.4 GB", speed: 4, accuracy: 4, englishOnly: true, blurb: "NVIDIA's English model. Fast and sharp on long dictation." },
  { id: "parakeet-tdt-0.6b-v3", name: "Parakeet TDT v3", engine: "parakeet", sizeLabel: "2.5 GB", speed: 4, accuracy: 5, englishOnly: false, blurb: "25 European languages with punctuation built in." },
  { id: "sortformer-4spk-v2", name: "Sortformer", engine: "diarizer", sizeLabel: "250 MB", speed: 4, accuracy: 4, englishOnly: false, blurb: "Tells up to four speakers apart in recorded sessions." },
];

export const DIARIZER_ID: ModelId = "sortformer-4spk-v2";

export const ENGINE_LABEL: Record<Engine, string> = {
  whisper: "OpenAI Whisper",
  parakeet: "NVIDIA Parakeet",
  diarizer: "Speaker diarization",
};

export const modelMeta = (id: ModelId) => MODELS.find((m) => m.id === id)!;
export const modelName = (id: ModelId) => MODELS.find((m) => m.id === id)?.name ?? id;

export const LANGUAGES: { code: string; name: string }[] = [
  { code: "auto", name: "Detect automatically" },
  { code: "en", name: "English" },
  { code: "es", name: "Spanish" },
  { code: "fr", name: "French" },
  { code: "de", name: "German" },
  { code: "it", name: "Italian" },
  { code: "pt", name: "Portuguese" },
  { code: "nl", name: "Dutch" },
  { code: "pl", name: "Polish" },
  { code: "sv", name: "Swedish" },
  { code: "ru", name: "Russian" },
  { code: "uk", name: "Ukrainian" },
  { code: "tr", name: "Turkish" },
  { code: "ar", name: "Arabic" },
  { code: "hi", name: "Hindi" },
  { code: "zh", name: "Chinese" },
  { code: "ja", name: "Japanese" },
  { code: "ko", name: "Korean" },
];

export type Mood = "idle" | "listening" | "speaking" | "thinking" | "recording" | "error";

export function moodOf(s: AppStatus): Mood {
  switch (s.state) {
    case "listening": return "listening";
    case "speaking": return "speaking";
    case "transcribing":
    case "transcribing_session": return "thinking";
    case "recording_session": return "recording";
    case "error": return "error";
    default: return "idle";
  }
}

export const STATUS_WORD: Record<AppStatus["state"], string> = {
  idle: "Ready",
  listening: "Listening",
  speaking: "Hearing you",
  transcribing: "Writing",
  recording_session: "Recording",
  transcribing_session: "Transcribing session",
  error: "Error",
};

export function fmtDuration(sec: number): string {
  const s = Math.max(0, Math.round(sec));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}` : `${m}:${String(r).padStart(2, "0")}`;
}

export function relativeTime(d: Date, now = new Date()): string {
  const diff = (now.getTime() - d.getTime()) / 1000;
  if (diff < 45) return "just now";
  if (diff < 3600) return `${Math.round(diff / 60)} min ago`;
  const sameDay = d.toDateString() === now.toDateString();
  const time = d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  if (sameDay) return `Today, ${time}`;
  const y = new Date(now);
  y.setDate(now.getDate() - 1);
  if (d.toDateString() === y.toDateString()) return `Yesterday, ${time}`;
  return d.toLocaleDateString([], { weekday: "short", month: "short", day: "numeric" }) + `, ${time}`;
}
