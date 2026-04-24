import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Device = "cpu" | "gpu";

export type ModelId =
  | "tiny.en"
  | "base.en"
  | "small.en"
  | "medium.en"
  | "large-v3";

export interface Settings {
  hotkey: string;
  model: ModelId;
  device: Device;
  language: string;
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

export interface AppStatus {
  state: "idle" | "listening" | "speaking" | "transcribing" | "error";
  message?: string;
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (s: Settings) => invoke<void>("save_settings", { settings: s }),
  listModels: () => invoke<ModelStatus[]>("list_models"),
  downloadModel: (id: ModelId) => invoke<void>("download_model", { id }),
  deleteModel: (id: ModelId) => invoke<void>("delete_model", { id }),
  onDownloadProgress: (cb: (p: DownloadProgress) => void): Promise<UnlistenFn> =>
    listen<DownloadProgress>("model://progress", (e) => cb(e.payload)),
  onStatus: (cb: (s: AppStatus) => void): Promise<UnlistenFn> =>
    listen<AppStatus>("app://status", (e) => cb(e.payload)),
};
