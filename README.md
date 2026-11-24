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

## Release

Releases are built by GitHub Actions (`.github/workflows/release.yml`) for
macOS (Apple Silicon + Intel), Windows and Linux.

1. Add the release notes under a new heading in `CHANGELOG.md`, e.g. `## [1.0.1] - 2026-12-01`.
2. Set the version everywhere at once: `npm run version:set -- 1.0.1`
   (updates `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock`).
3. Commit, merge to `main`, then tag and push: `git tag v1.0.1 && git push origin v1.0.1`.
4. The workflow checks the tag matches the version, builds every platform and
   attaches the installers to a **draft** GitHub release. Review it and click *Publish*.

To rebuild an existing tag, run the *Release* workflow manually from the Actions tab.

**Code signing (optional, via repository secrets):**

| Platform | Secrets | Without them |
| --- | --- | --- |
| Windows | `WINDOWS_CERTIFICATE` (base64 `.pfx`), `WINDOWS_CERTIFICATE_PASSWORD` | Unsigned installer (SmartScreen warning) |
| macOS | `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`; for notarization also `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID` | Ad-hoc signed; users right-click → Open the first time |

`ci.yml` type-checks and builds the frontend and runs `cargo check` / `cargo test`
on every pull request and push to `main`.

## Website

The product site lives in `site/` (Vite, three.js, GSAP, Lenis) and is published to
GitHub Pages by `.github/workflows/pages.yml` on every push to `main` that touches it.
One-time setup: **Settings → Pages → Source: GitHub Actions**.

```bash
npm run site:dev     # http://localhost:4173/tiny-whisper/
npm run site:build   # → dist-site/
npm run site:shots   # refresh site/public/shots/ from the app (needs Playwright)
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
