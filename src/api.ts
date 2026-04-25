import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (s: Settings) => invoke<void>("save_settings", { settings: s }),
  listModels: () => invoke<ModelStatus[]>("list_models"),
  downloadModel: (id: ModelId) => invoke<void>("download_model", { id }),
  deleteModel: (id: ModelId) => invoke<void>("delete_model", { id }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),

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
};
