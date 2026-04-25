# Copilot Instructions

## Build, test, and run commands

| Task | Command | Notes |
| --- | --- | --- |
| Install JS dependencies | `npm install` | Frontend and Tauri CLI live in the repo root `package.json`. |
| Run the desktop app in dev | `npm run tauri dev` | Tauri starts Vite for you via `src-tauri/tauri.conf.json`; the frontend dev server is `http://localhost:1420`. |
| Run the frontend only | `npm run dev` | Useful for iterating on the settings UI without rebuilding the Rust side. |
| Build the frontend bundle | `npm run build` | Runs `tsc && vite build` and emits both `index.html` and `indicator.html` into `dist\`. |
| Build the packaged desktop app | `npm run tauri build` | Uses `npm run build` as the prebuild step. |
| Run Rust tests | `cargo test --manifest-path src-tauri\Cargo.toml` | There are currently no checked-in tests, but this is the active Rust test entrypoint. |
| Run a single Rust test | `cargo test <test_name> --manifest-path src-tauri\Cargo.toml` | Use a test name filter when Rust tests are added. |

There is currently no dedicated lint script and no frontend test runner configured in `package.json`.

## High-level architecture

- This is a **Tauri desktop app** with two React entrypoints: the main settings window (`src\main.tsx` -> `src\App.tsx`) and a separate floating indicator window (`src\indicator-main.tsx` -> `src\Indicator.tsx`). Tauri declares both windows in `src-tauri\tauri.conf.json`.
- The **Rust backend is the source of truth for app behavior**. `src-tauri\src\main.rs` wires plugins, registers Tauri commands, creates the tray icon, manages app state, and starts background worker threads.
- **Live dictation** flows through the recorder pipeline in `src-tauri\src\recorder.rs`: microphone input is captured with `cpal`, converted to mono 16 kHz, segmented with VAD, then sent to `segment_worker` in `main.rs`, which transcribes with the active model and pastes text through `paster.rs`.
- **Recorded sessions** use a separate raw-capture path. `commands.rs` and `hotkey.rs` start/stop raw recording, `session_worker.rs` writes `audio.wav` plus `meta.json`, then later transcribes that saved WAV and optionally runs diarization before writing `transcript.json`.
- **Model management** is centralized in `src-tauri\src\config.rs`, `models.rs`, and `transcriber\`. `config.rs` defines model IDs, download URLs, sizes, engine mapping, and on-disk layout; `models.rs` downloads files and emits `model://progress`; `transcriber\mod.rs` routes requests to the Whisper, Parakeet, or Sortformer backends.
- The React side talks to Rust only through `src\api.ts`, which wraps `invoke(...)` commands and Tauri events like `app://status`, `model://progress`, and `session://updated`.

## Key conventions

- **Keep Rust and TypeScript model/status definitions in sync.** `ModelId`, `Engine`, settings fields, and app status variants are mirrored across `src-tauri\src\config.rs`, `src-tauri\src\hotkey.rs`, `src\api.ts`, and UI metadata in `src\App.tsx` / `src\Indicator.tsx`.
- **Dictation and session recording are intentionally separate modes.** Dictation uses VAD-segmented phrases and `is_recording`; session capture uses raw WAV recording and `active_session`. The code prevents both from running at the same time, so changes in one path usually need a quick check of the other.
- **Session persistence has a fixed filesystem contract.** Each session lives under `app_data\sessions\<id>\` with `audio.wav`, `meta.json`, and optional `transcript.json`; the UI depends on that layout through `sessions.rs` and `SessionsCard.tsx`.
- **App state changes are event-driven, not polled.** Backend workers emit status and session/model events, and both React windows subscribe to those events. New backend workflows should usually surface progress through Tauri events rather than frontend timers.
- **The UI uses Tailwind v4 theme tokens and lightweight shared primitives.** Styling is mostly done with custom CSS variables from `src\index.css` plus local Radix-based components in `src\components\ui`; prefer extending those patterns over introducing a new styling system.
- **Release diagnostics are file-based.** Because the packaged app runs without a console, important failures are surfaced through `main.tsx`/`App.tsx` fallback UI and Rust log files written under `%APPDATA%\dev.tinywhisper.app\`.
