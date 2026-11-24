/// A stand-in backend used when the UI runs in a plain browser (`npm run dev`)
/// instead of inside Tauri. It mimics the command and event shapes of the Rust
/// side closely enough to design against. `?fresh` starts with no models.
import type { AppStatus, ModelId, ModelStatus, SessionMeta, Settings, Transcript } from "../api";

type Handler = (e: { payload: unknown }) => void;
const handlers = new Map<string, Set<Handler>>();

function emit(event: string, payload: unknown) {
  handlers.get(event)?.forEach((h) => h({ payload }));
}

// Lets screenshot scripts push events (e.g. indicator states) into the page.
if (typeof window !== "undefined") (window as unknown as { __twMock: unknown }).__twMock = { emit };

export async function mockListen(event: string, cb: Handler): Promise<() => void> {
  if (!handlers.has(event)) handlers.set(event, new Set());
  handlers.get(event)!.add(cb);
  return () => handlers.get(event)?.delete(cb);
}

const params = new URLSearchParams(typeof location !== "undefined" ? location.search : "");
const fresh = params.has("fresh");

const settings: Settings = {
  hotkey: "CommandOrControl+Shift+Space",
  session_hotkey: "CommandOrControl+Shift+R",
  model: "base.en",
  device: "cpu",
  language: "en",
};

const SIZES: Record<ModelId, number> = {
  "tiny.en": 75, "base.en": 142, "small.en": 466, "medium.en": 1500, "large-v3": 2900,
  "parakeet-ctc-0.6b-en": 2400, "parakeet-tdt-0.6b-v3": 2500, "sortformer-4spk-v2": 250,
};
const downloaded = new Set<ModelId>(fresh ? [] : ["base.en", "small.en", "sortformer-4spk-v2"]);

const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();
let sessions: SessionMeta[] = fresh
  ? []
  : [
      { id: "s3", created_at: ago(42), duration_secs: 1834, model_used: "small.en", speaker_count: 3, has_transcript: true },
      { id: "s2", created_at: ago(60 * 26), duration_secs: 612, model_used: null, speaker_count: null, has_transcript: false },
      { id: "s1", created_at: ago(60 * 24 * 4), duration_secs: 2710, model_used: "base.en", speaker_count: 2, has_transcript: true },
    ];

const LINES = [
  [0, "Okay, let's get started. The goal today is to agree on what ships in the next release."],
  [1, "I think the onboarding flow has to be in. Right now people install it and then nothing happens until they find the models page."],
  [0, "Agreed. And the dictate button actually needs to do something."],
  [2, "From the support side, the most common question is where the transcript went. They don't realise it pasted into whatever window had focus."],
  [1, "So a history of recent phrases solves that. Click to copy it again."],
  [0, "Good. Let's also export sessions as markdown, people keep asking for meeting notes."],
  [2, "And speaker names. 'Speaker three' isn't great in a document you send around."],
  [0, "Noted. Let's wrap there and follow up on Thursday."],
] as const;
const transcriptFor = (id: string): Transcript => ({
  model: id === "s1" ? "base.en" : "small.en",
  diarized: true,
  segments: LINES.map(([speaker, text], i) => ({
    start_secs: i * 47 + 3,
    end_secs: i * 47 + 40,
    speaker: id === "s1" ? speaker % 2 : speaker,
    text,
  })),
});

let dictating = false;
const PHRASES = [
  "Let's move the standup to ten thirty so the Berlin team can join.",
  "Reminder to send the revised estimate to Priya before Friday.",
  "The build is green again, it was a stale cache on the runner.",
];
let phrase = 0;
const timers: number[] = [];
const status = (s: AppStatus) => emit("app://status", s);

function dictationLoop() {
  const step = (ms: number, fn: () => void) => timers.push(window.setTimeout(fn, ms));
  step(1400, () => dictating && status({ state: "speaking" }));
  step(4200, () => dictating && status({ state: "transcribing" }));
  step(5400, () => {
    if (!dictating) return;
    emit("dictation://text", PHRASES[phrase++ % PHRASES.length]);
    status({ state: "listening" });
    dictationLoop();
  });
}

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function mockInvoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
  await wait(40);
  switch (cmd) {
    case "get_settings": return { ...settings };
    case "save_settings": Object.assign(settings, args.settings); return;
    case "list_models":
      return (Object.keys(SIZES) as ModelId[]).map<ModelStatus>((id) => ({ id, downloaded: downloaded.has(id), size_mb: SIZES[id] }));
    case "download_model": {
      const id = args.id as ModelId;
      const total = SIZES[id] * 1_000_000;
      for (let i = 1; i <= 40; i++) {
        await wait(70);
        emit("model://progress", { model: id, downloaded_bytes: (total * i) / 40, total_bytes: total });
      }
      downloaded.add(id);
      return;
    }
    case "delete_model": downloaded.delete(args.id as ModelId); return;
    case "get_autostart": return true;
    case "set_autostart": return;
    case "take_startup_error": return null;
    case "gpu_available": return false;
    case "toggle_dictation":
      if (!downloaded.has(settings.model)) {
        status({ state: "error", message: `Model "${settings.model}" is not downloaded. Open Settings → Models to download it.` });
        return;
      }
      dictating = !dictating;
      timers.splice(0).forEach(clearTimeout);
      status({ state: dictating ? "listening" : "idle" });
      if (dictating) dictationLoop();
      return;
    case "start_session_recording":
      status({ state: "recording_session" });
      return "s-new";
    case "stop_session_recording": {
      status({ state: "idle" });
      const m: SessionMeta = { id: `s${Date.now()}`, created_at: new Date().toISOString(), duration_secs: 74, model_used: null, speaker_count: null, has_transcript: false };
      sessions = [m, ...sessions];
      emit("session://updated", m);
      return;
    }
    case "list_sessions": return sessions;
    case "delete_session": sessions = sessions.filter((s) => s.id !== args.id); return;
    case "get_session_transcript": return transcriptFor(args.id as string);
    case "transcribe_session": {
      const id = args.id as string;
      for (let p = 0; p <= 100; p += 5) {
        status({ state: "transcribing_session", session_id: id, percent: p });
        await wait(90);
      }
      const m = sessions.find((s) => s.id === id)!;
      Object.assign(m, { has_transcript: true, model_used: settings.model, speaker_count: args.diarize ? 3 : null });
      emit("session://updated", { ...m });
      status({ state: "idle" });
      return;
    }
  }
  throw new Error(`mock: unknown command ${cmd}`);
}
