# Changelog

All notable changes to Tiny Whisper. The release workflow copies the section
for the tagged version into the GitHub release notes, so write it for users.

## [Unreleased]

## [1.0.0] - 2026-11-24

The first stable release: a completely redesigned app around the same private,
on-device transcription.

### New design
- **Four clear places** — Dictate, Sessions, Models and Settings — in a sidebar,
  with light and dark themes that follow your system.
- **Dictate** shows a live voice field that reacts as you speak. The big button
  now really starts and stops dictation, and every phrase you dictate is kept in
  a *Recent phrases* list — click one to copy it again.
- **First-run setup** downloads the recommended model and shows your shortcut,
  so dictation works within a minute of installing.
- **Sessions** opens recordings in a reader with each speaker's share of the
  conversation, speaker names you can edit, and *Copy* / *Copy as Markdown*.
- **Models** compares speed and accuracy at a glance, marks the model in use,
  and shows download progress per model.
- **Settings save as you change them.** Language is a proper list (and is locked
  for English-only models); GPU is only offered when a usable GPU is found.
- A redrawn floating indicator with live level bars; long errors show in full on hover.
- Deleting a model or a session asks for a second click; the model in use
  can't be deleted.

### Faster transcription
- **Windows uses your GPU again.** The installer is a GPU (Vulkan) build:
  on an RTX 3060 a phrase transcribes in about 0.2–0.5 s instead of ~3 s on
  the CPU. PCs without a GPU get the CPU automatically, from the same installer.
- **Much faster on the CPU, too.** Whisper is now built with compiler
  optimizations on Windows, which earlier builds were missing.
- The model loads and warms up when the app starts, so the first phrase is as
  quick as the rest.
- Fixed a crash on transcription on CPUs without AVX-512 (e.g. AMD Ryzen 5000).

### Also in 1.0
- Live dictation with voice-activity detection: speak, pause, and the phrase is
  typed where your cursor is.
- Recorded sessions with optional speaker labelling (Sortformer), safe against
  crashes and quitting mid-recording.
- Whisper (Tiny → Large v3) and NVIDIA Parakeet models, all running locally.
- Global shortcuts you set by pressing keys, launch at login, and a strict
  content security policy.
- macOS: Cmd/Ctrl mapping, microphone and Accessibility permission handling.

## [0.2.0]

- Recorded sessions, Parakeet and Sortformer support, reliability fixes.
