# tiny-whisper

`tiny-whisper` is a Tauri desktop app for local speech-to-text with a settings window and a floating indicator window.

## Prerequisites

- Node.js and npm
- Rust toolchain
- Tauri build prerequisites for your OS

## Development

```bash
npm install
npm run tauri dev
```

Frontend only:

```bash
npm run dev
```

## Build

Build frontend bundle:

```bash
npm run build
```

Build packaged desktop app:

```bash
npm run tauri build
```

## Test

Run Rust tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## Architecture (high level)

- React + Vite frontend in `src/`
- Tauri + Rust backend in `src-tauri/src/`
- Main settings window entry: `src/main.tsx`
- Floating indicator entry: `src/indicator-main.tsx`
- Rust backend entrypoint: `src-tauri/src/main.rs`

## Notes

- There is no dedicated lint script in `package.json`.
- There is no frontend test runner currently configured.
