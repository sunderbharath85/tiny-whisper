import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";
import { mockInvoke, mockListen } from "./lib/mock";

export type Device = "cpu" | "gpu";

export type ModelId =
  | "tiny.en"
  | "base.en"
  | "small.en"
  | "medium.en"
  | "large-v3"
  | "parakeet-ctc-0.6b-en"
  | "parakeet-tdt-0.6b-v3"
  | "sortformer-4spk-v2";

export type Engine = "whisper" | "parakeet" | "diarizer";

export interface Settings {
  hotkey: string;
  session_hotkey: string;
  model: ModelId;
  device: Device;
  language: string;
}

export interface SessionMeta {
  id: string;
  created_at: string;
  duration_secs: number;
  model_used: ModelId | null;
  speaker_count: number | null;
  has_transcript: boolean;
}

export interface TranscriptSegment {
  start_secs: number;
  end_secs: number;
  speaker: number | null;
  text: string;
}

export interface Transcript {
  model: ModelId;
  diarized: boolean;
  segments: TranscriptSegment[];
}

export interface ModelStatus {
  id: ModelId;
  downloaded: boolean;
  size_mb: number;
}

export interface DownloadProgress {
  model: ModelId;
  downloaded_bytes: number;
  total_bytes: number;
}

export type AppStatus =
  | { state: "idle" }
  | { state: "listening" }
  | { state: "speaking" }
  | { state: "transcribing" }
  | { state: "recording_session" }
  | { state: "transcribing_session"; session_id: string; percent: number }
  | { state: "error"; message?: string };

/// Outside Tauri (plain `npm run dev` in a browser) a mock backend stands in,
/// so the UI can be designed and screenshotted without the Rust side.
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const invoke: typeof tauriInvoke = inTauri ? tauriInvoke : (mockInvoke as typeof tauriInvoke);
const listen: typeof tauriListen = inTauri ? tauriListen : (mockListen as typeof tauriListen);

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (s: Settings) => invoke<void>("save_settings", { settings: s }),
  listModels: () => invoke<ModelStatus[]>("list_models"),
  downloadModel: (id: ModelId) => invoke<void>("download_model", { id }),
  deleteModel: (id: ModelId) => invoke<void>("delete_model", { id }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  /// Error from app setup (e.g. hotkey registration), returned once then cleared.
  takeStartupError: () => invoke<string | null>("take_startup_error"),
  /// Start or stop dictation, the same as pressing the dictation hotkey.
  toggleDictation: () => invoke<void>("toggle_dictation"),
  /// Whether this build has a GPU backend compiled in.
  gpuAvailable: () => invoke<boolean>("gpu_available"),

  startSessionRecording: () => invoke<string>("start_session_recording"),
  stopSessionRecording: () => invoke<void>("stop_session_recording"),
  listSessions: () => invoke<SessionMeta[]>("list_sessions"),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),
  getSessionTranscript: (id: string) => invoke<Transcript>("get_session_transcript", { id }),
  transcribeSession: (id: string, diarize: boolean) =>
    invoke<void>("transcribe_session", { id, diarize }),

  onDownloadProgress: (cb: (p: DownloadProgress) => void): Promise<UnlistenFn> =>
    listen<DownloadProgress>("model://progress", (e) => cb(e.payload)),
  onStatus: (cb: (s: AppStatus) => void): Promise<UnlistenFn> =>
    listen<AppStatus>("app://status", (e) => cb(e.payload)),
  onSessionUpdated: (cb: (m: SessionMeta) => void): Promise<UnlistenFn> =>
    listen<SessionMeta>("session://updated", (e) => cb(e.payload)),
  /// Each dictated phrase, after it has been pasted.
  onDictationText: (cb: (text: string) => void): Promise<UnlistenFn> =>
    listen<string>("dictation://text", (e) => cb(e.payload)),
};
