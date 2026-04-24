# tiny-whisper — feature plan

Current state: working MVP with VAD-based live transcribe, floating indicator pill,
CPU-only whisper (small.en default), toggle hotkey (Ctrl+Shift+Space).

## Top priorities (pick first)

1. **Re-enable GPU (Vulkan)**
   - Unlocks near-instant small.en and makes medium.en / large-v3 practical
   - Blocked by earlier `bindgen` header issue — revisit with proper `BINDGEN_EXTRA_CLANG_ARGS` or switch to subprocess-based whisper.cpp
2. **Initial prompt / custom vocabulary**
   - whisper supports `initial_prompt` to bias toward specific words (names, jargon, acronyms)
   - Huge quality jump for technical dictation, ~50 lines of code
3. **Transcript history window**
   - Keep last N phrases in a scrollable panel, click to re-copy
   - Low effort, surprisingly useful

## Quick wins (~30 min each)

- **Paste-vs-type toggle** — clipboard paste fails in some apps (terminals, password fields). Fallback to synthesized keystrokes.
- **Auto-capitalize + punctuation cleanup** — whisper sometimes returns lowercase or missing trailing period
- **Mic device picker** — list input devices in settings, not just default
- **Mute toggle** — second hotkey to pause a session without stopping it
- **VAD sensitivity slider in UI** — expose `start_thresh` and `end_silence_frames` without editing source

## Medium effort (hours)

- **Autostart on login** — Windows registry entry, toggleable in settings
- **Spoken command substitution** — "new line", "period", "comma", "question mark" → literal characters
- **Undo last paste hotkey** — Ctrl+Z the last dictation if misspoken
- **Real tray icon** — replace placeholder "tw" PNG

## Bigger (half-day+)

- **True sliding-window live mode** — words appear as you talk, not after pauses. Dedup against previous window. Mostly interesting with GPU.
- **LLM post-processing pass** — pipe whisper output through a local llama.cpp for grammar/punctuation/code formatting. Slow on CPU, fine on GPU.
- **Per-app profiles** — detect focused app (Slack, VS Code, Terminal) and apply different capitalization/punctuation/prompt

## Known issues / tech debt

- whisper-rs 0.16 built without GPU features; need to revisit Vulkan bindgen setup
- Tray icon is placeholder (generated "tw" PNG)
- VAD thresholds are fixed in source (not yet user-tunable)
- No persistence of transcript history
- Settings JSON lives in app_data_dir but no migration/versioning story
