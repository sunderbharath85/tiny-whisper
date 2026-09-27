use crate::config::Settings;
use crate::recorder::WavMsg;
use crate::session_worker;
use crate::sessions;
use crate::state::{ActiveSession, AppState};
use serde::Serialize;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AppStatus {
    Idle,
    Listening,
    Speaking,
    Transcribing,
    /// A session is being recorded to disk (raw capture mode).
    RecordingSession,
    /// A previously-recorded session is being transcribed offline.
    TranscribingSession { session_id: String, percent: f32 },
    Error { message: String },
}

pub fn emit_status(app: &AppHandle, status: AppStatus) {
    let _ = app.emit("app://status", status);
}

/// The parsed dictation and session shortcuts from `Settings`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkeys {
    pub dictation: Shortcut,
    /// `None` when `session_hotkey` is blank, which means no session shortcut.
    pub session: Option<Shortcut>,
}

type ParseError = <Shortcut as FromStr>::Err;

/// Parse an accelerator such as `CommandOrControl+Shift+Space`. The error is a
/// short reason to show in the UI (the parser's own messages link to GitHub).
pub fn parse_accelerator(accel: &str) -> Result<Shortcut, String> {
    accel.parse::<Shortcut>().map_err(|e| match e {
        ParseError::UnsupportedKey(key) if key.trim().is_empty() => "no key given".to_string(),
        ParseError::UnsupportedKey(key) => format!("unknown key \"{key}\""),
        ParseError::EmptyToken(_) => "a key is missing next to a \"+\"".to_string(),
        ParseError::InvalidFormat(_) => "put the modifiers first, then exactly one key".to_string(),
    })
}

/// Keys that type text: letters, digits, Space, Enter, Tab, Backspace,
/// punctuation, and their numpad twins. Bound with no modifier, a global
/// shortcut would swallow that key in every app.
fn is_typing_key(key: Code) -> bool {
    use Code::*;
    matches!(
        key,
        KeyA | KeyB | KeyC | KeyD | KeyE | KeyF | KeyG | KeyH | KeyI | KeyJ | KeyK | KeyL | KeyM
            | KeyN | KeyO | KeyP | KeyQ | KeyR | KeyS | KeyT | KeyU | KeyV | KeyW | KeyX | KeyY
            | KeyZ
            | Digit0 | Digit1 | Digit2 | Digit3 | Digit4 | Digit5 | Digit6 | Digit7 | Digit8
            | Digit9
            | Space | Enter | Tab | Backspace
            | Backquote | Backslash | BracketLeft | BracketRight | Comma | Equal | Minus | Period
            | Quote | Semicolon | Slash | IntlBackslash | IntlRo | IntlYen
            | Numpad0 | Numpad1 | Numpad2 | Numpad3 | Numpad4 | Numpad5 | Numpad6 | Numpad7
            | Numpad8 | Numpad9 | NumpadAdd | NumpadComma | NumpadDecimal | NumpadDivide
            | NumpadEnter | NumpadEqual | NumpadMultiply | NumpadSubtract
    )
}

/// Whether `sc` is a typing key with no modifier, such as `Space` or `A`.
/// F-keys, Pause, media keys and other non-typing keys may be used bare.
pub fn needs_modifier(sc: Shortcut) -> bool {
    let mods = Modifiers::ALT | Modifiers::CONTROL | Modifiers::SHIFT | Modifiers::SUPER;
    !sc.mods.intersects(mods) && is_typing_key(sc.key)
}

fn needs_modifier_error(field: &str, accel: &str) -> String {
    let mut label = field.to_string();
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    format!("{label} hotkey \"{accel}\" needs a modifier (Ctrl, Alt, Shift or Win)")
}

fn parse_field(field: &str, accel: &str) -> Result<Shortcut, String> {
    let sc = parse_accelerator(accel).map_err(|e| format!("Invalid {field} hotkey \"{accel}\": {e}"))?;
    if needs_modifier(sc) {
        return Err(needs_modifier_error(field, accel));
    }
    Ok(sc)
}

fn same_hotkey_error(settings: &Settings) -> String {
    format!(
        "Session hotkey \"{}\" is the same as the dictation hotkey \"{}\"",
        settings.session_hotkey, settings.hotkey
    )
}

/// Parse and check both hotkeys in `settings`. Errors name the field and the
/// value, so the UI can show them as they are.
pub fn parse_hotkeys(settings: &Settings) -> Result<Hotkeys, String> {
    let dictation = parse_field("dictation", &settings.hotkey)?;
    let session = if settings.session_hotkey.trim().is_empty() {
        None
    } else {
        let session = parse_field("session", &settings.session_hotkey)?;
        if session == dictation {
            return Err(same_hotkey_error(settings));
        }
        Some(session)
    };
    Ok(Hotkeys { dictation, session })
}

/// Decide what saving `next` over `prev` must register. `Some` when either
/// hotkey changed (C-12), or when `prev`'s shortcuts are not all registered
/// (boot registration failed), so saving retries. `None` leaves the shortcuts
/// alone. Errors when `next` has an invalid hotkey, changed or not (C-04).
pub fn plan_hotkey_change(
    prev: &Settings,
    next: &Settings,
    prev_registered: bool,
) -> Result<Option<Hotkeys>, String> {
    let keys = parse_hotkeys(next)?;
    let changed = prev.hotkey != next.hotkey || prev.session_hotkey != next.session_hotkey;
    Ok((changed || !prev_registered).then_some(keys))
}

/// Whether every shortcut in `settings` is currently registered by this app.
pub fn is_registered(app: &AppHandle, settings: &Settings) -> bool {
    let gs = app.global_shortcut();
    match parse_hotkeys(settings) {
        Ok(keys) => {
            gs.is_registered(keys.dictation) && keys.session.map_or(true, |s| gs.is_registered(s))
        }
        Err(_) => false,
    }
}

fn register_error(field: &str, accel: &str, e: &tauri_plugin_global_shortcut::Error) -> String {
    let msg = e.to_string();
    let reason = if msg.starts_with("HotKey already registered") {
        "it is already in use by another app".to_string()
    } else {
        msg
    };
    format!("Could not register {field} hotkey \"{accel}\": {reason}")
}

fn register_dictation(app: &AppHandle, accel: &str, shortcut: Shortcut) -> Result<(), String> {
    let dict_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app_handle, _sc, event| {
            handle_dictation(&dict_handle, app_handle, event);
        })
        .map_err(|e| register_error("dictation", accel, &e))
}

fn register_session(app: &AppHandle, accel: &str, shortcut: Shortcut) -> Result<(), String> {
    let sess_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |app_handle, _sc, event| {
            handle_session(&sess_handle, app_handle, event);
        })
        .map_err(|e| register_error("session", accel, &e))
}

/// What `register` does with each hotkey in `settings`, decided on its own so
/// one bad hotkey doesn't take the other down. `Err` means that hotkey is
/// skipped and the message reported (unparsable, a bare typing key, or a
/// session hotkey equal to the dictation one).
#[derive(Debug, PartialEq, Eq)]
pub struct RegisterPlan {
    pub dictation: Result<Shortcut, String>,
    /// `Ok(None)` when `session_hotkey` is blank.
    pub session: Result<Option<Shortcut>, String>,
}

pub fn plan_register(settings: &Settings) -> RegisterPlan {
    let dictation = parse_field("dictation", &settings.hotkey);
    let session = if settings.session_hotkey.trim().is_empty() {
        Ok(None)
    } else {
        match parse_field("session", &settings.session_hotkey) {
            Ok(sc) if dictation.as_ref().ok() == Some(&sc) => Err(same_hotkey_error(settings)),
            other => other.map(Some),
        }
    };
    RegisterPlan { dictation, session }
}

/// Register the shortcuts in `settings`, each on its own (`plan_register`).
/// Used at boot and to restore after a failed change. Returns every failure
/// as one message.
pub fn register(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let _ = app.global_shortcut().unregister_all();
    let plan = plan_register(settings);
    let mut errors = Vec::new();

    match plan.dictation {
        Ok(sc) => {
            if let Err(e) = register_dictation(app, &settings.hotkey, sc) {
                errors.push(e);
            }
        }
        Err(e) => errors.push(e),
    }
    match plan.session {
        Ok(Some(sc)) => {
            if let Err(e) = register_session(app, &settings.session_hotkey, sc) {
                errors.push(e);
            }
        }
        Ok(None) => {}
        Err(e) => errors.push(e),
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Swap the registered shortcuts for `keys` (parsed from `next`). If any of
/// them fails to register, put back `prev`'s shortcuts and return the error.
pub fn apply(app: &AppHandle, prev: &Settings, next: &Settings, keys: Hotkeys) -> Result<(), String> {
    let _ = app.global_shortcut().unregister_all();
    let result = register_dictation(app, &next.hotkey, keys.dictation).and_then(|()| match keys.session {
        Some(sc) => register_session(app, &next.session_hotkey, sc),
        None => Ok(()),
    });
    if result.is_err() {
        restore(app, prev);
    }
    result
}

/// Put back `prev`'s shortcuts after a failed change. Failures are only logged,
/// since the caller is already returning an error.
pub fn restore(app: &AppHandle, prev: &Settings) {
    if let Err(e) = register(app, prev) {
        log::error!("could not restore previous hotkeys: {e}");
    }
}

fn handle_dictation(app: &AppHandle, _app_handle: &AppHandle, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    // Don't let the dictation hotkey fire while a session is being recorded —
    // the recorder would refuse the cpal stream and log a warning.
    if state.active_session.lock().is_some() {
        return;
    }
    let was_recording = state.is_recording.load(Ordering::SeqCst);

    if was_recording {
        if let Err(e) = state.recorder.stop_session() {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        state.is_recording.store(false, Ordering::SeqCst);
        emit_status(app, AppStatus::Idle);
        hide_indicator(app);
    } else {
        if let Err(e) = state.recorder.start_session() {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        state.is_recording.store(true, Ordering::SeqCst);
        show_indicator(app);
        emit_status(app, AppStatus::Listening);
    }
}

fn handle_session(app: &AppHandle, _app_handle: &AppHandle, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    let active = state.active_session.lock().clone();

    if active.is_some() {
        if let Err(e) = state.recorder.stop_raw_capture() {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        // Writer thread clears active_session on finalize.
        hide_indicator(app);
    } else {
        // Don't let a session start on top of an in-flight dictation.
        if state.is_recording.load(Ordering::SeqCst) {
            log::warn!("ignoring session hotkey while dictation is active");
            return;
        }
        let id = sessions::new_session_id();
        let started_at = SystemTime::now();
        let (wav_tx, wav_rx) = channel::<WavMsg>();
        if let Err(e) = state.recorder.start_raw_capture(wav_tx) {
            emit_status(app, AppStatus::Error { message: e.to_string() });
            return;
        }
        *state.active_session.lock() = Some(ActiveSession {
            id: id.clone(),
            started_at,
        });
        session_worker::spawn_writer(app.clone(), wav_rx, id, started_at);
        show_indicator(app);
        emit_status(app, AppStatus::RecordingSession);
    }
}

pub fn show_indicator(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        let _ = w.show();
    }
}

pub fn hide_indicator(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        let _ = w.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(hotkey: &str, session_hotkey: &str) -> Settings {
        Settings {
            hotkey: hotkey.into(),
            session_hotkey: session_hotkey.into(),
            ..Settings::default()
        }
    }

    #[test]
    fn accepts_k3_accelerators_and_defaults() {
        for accel in [
            "CommandOrControl+Shift+Space",
            "CommandOrControl+Shift+KeyR",
            "CommandOrControl+Shift+R",
            "Alt+F9",
            "CommandOrControl+Alt+Digit1",
            "CommandOrControl+Alt+Shift+Super+KeyA",
            "F9",
        ] {
            assert!(parse_accelerator(accel).is_ok(), "{accel} should parse");
        }
        let d = Settings::default();
        assert!(parse_hotkeys(&d).is_ok(), "default settings should parse");
    }

    #[test]
    fn k3_spellings_map_to_expected_keys() {
        let sc = parse_accelerator("CommandOrControl+Shift+KeyR").unwrap();
        assert_eq!(sc, parse_accelerator("CommandOrControl+Shift+R").unwrap());
        assert_eq!(sc.key, Code::KeyR);
        assert!(sc.mods.contains(Modifiers::SHIFT));
        let sc = parse_accelerator("CommandOrControl+Alt+Digit1").unwrap();
        assert_eq!(sc.key, Code::Digit1);
        assert!(sc.mods.contains(Modifiers::ALT));
        assert_eq!(parse_accelerator("Alt+F9").unwrap().key, Code::F9);
    }

    #[test]
    fn rejects_invalid_accelerators() {
        for accel in ["Ctrl+Shft+Space", "", "Ctrl+", "+Space", "Ctrl+Shift", "Ctrl+Space+Shift"] {
            assert!(parse_accelerator(accel).is_err(), "{accel:?} should be rejected");
        }
        assert_eq!(parse_accelerator("Ctrl+Shft+Space").unwrap_err(), "unknown key \"Shft\"");
        assert_eq!(parse_accelerator("").unwrap_err(), "no key given");
    }

    #[test]
    fn parse_errors_name_the_field_and_value() {
        let err = parse_hotkeys(&settings("Ctrl+Shft+Space", "CommandOrControl+Shift+R")).unwrap_err();
        assert_eq!(err, "Invalid dictation hotkey \"Ctrl+Shft+Space\": unknown key \"Shft\"");
        let err = parse_hotkeys(&settings("CommandOrControl+Shift+Space", "Ctrl+")).unwrap_err();
        assert!(err.starts_with("Invalid session hotkey \"Ctrl+\": "), "{err}");
    }

    #[test]
    fn blank_session_hotkey_means_none() {
        let keys = parse_hotkeys(&settings("CommandOrControl+Shift+Space", "  ")).unwrap();
        assert_eq!(keys.session, None);
    }

    #[test]
    fn rejects_same_dictation_and_session_hotkey() {
        let err = parse_hotkeys(&settings("Ctrl+Shift+R", "CommandOrControl+Shift+KeyR")).unwrap_err();
        assert!(err.starts_with("Session hotkey \"CommandOrControl+Shift+KeyR\" is the same"), "{err}");
    }

    #[test]
    fn plan_unchanged_and_registered_does_nothing() {
        let prev = Settings::default();
        let next = Settings { language: "en".into(), ..prev.clone() };
        assert_eq!(plan_hotkey_change(&prev, &next, true), Ok(None));
    }

    #[test]
    fn plan_only_session_hotkey_changed_reregisters() {
        let prev = Settings::default();
        let next = Settings { session_hotkey: "Alt+F9".into(), ..prev.clone() };
        let keys = plan_hotkey_change(&prev, &next, true).unwrap().expect("must re-register");
        assert_eq!(keys.dictation, parse_accelerator(&prev.hotkey).unwrap());
        assert_eq!(keys.session, Some(parse_accelerator("Alt+F9").unwrap()));
    }

    #[test]
    fn plan_only_dictation_hotkey_changed_reregisters() {
        let prev = Settings::default();
        let next = Settings { hotkey: "CommandOrControl+Alt+Digit1".into(), ..prev.clone() };
        assert!(plan_hotkey_change(&prev, &next, true).unwrap().is_some());
    }

    #[test]
    fn plan_clearing_session_hotkey_reregisters_without_it() {
        let prev = Settings::default();
        let next = Settings { session_hotkey: String::new(), ..prev.clone() };
        let keys = plan_hotkey_change(&prev, &next, true).unwrap().expect("must re-register");
        assert_eq!(keys.session, None);
    }

    #[test]
    fn plan_unchanged_but_not_registered_retries() {
        let prev = Settings::default();
        assert!(plan_hotkey_change(&prev, &prev.clone(), false).unwrap().is_some());
    }

    #[test]
    fn plan_rejects_invalid_hotkeys_before_anything_changes() {
        let prev = Settings::default();
        let next = Settings { hotkey: "Ctrl+Shft+Space".into(), ..prev.clone() };
        assert!(plan_hotkey_change(&prev, &next, true).is_err());
        let next = Settings { session_hotkey: "+Space".into(), ..prev.clone() };
        assert!(plan_hotkey_change(&prev, &next, true).is_err());
        // Invalid even when unchanged, e.g. a bad value loaded from settings.json.
        let bad = settings("Ctrl+Shft+Space", "CommandOrControl+Shift+R");
        assert!(plan_hotkey_change(&bad, &bad.clone(), false).is_err());
    }

    const NEEDS_MOD: &str = "needs a modifier (Ctrl, Alt, Shift or Win)";

    #[test]
    fn needs_modifier_only_for_bare_typing_keys() {
        for accel in [
            "Space", "KeyA", "A", "z", "Digit1", "0", "Enter", "Tab", "Backspace", "Comma", "/",
            "Backquote", "BracketLeft", "Numpad5", "NumpadEnter",
        ] {
            assert!(needs_modifier(parse_accelerator(accel).unwrap()), "{accel} needs a modifier");
        }
        for accel in [
            "F1", "F9", "F24", "Pause", "ScrollLock", "MediaPlayPause", "AudioVolumeMute",
            "PrintScreen", "Insert", "Escape", "ArrowUp", "CommandOrControl+Space", "Shift+KeyA",
            "Alt+Digit1", "Super+Enter", "Ctrl+Shift+Space",
        ] {
            assert!(!needs_modifier(parse_accelerator(accel).unwrap()), "{accel} is allowed");
        }
    }

    #[test]
    fn plan_rejects_bare_typing_keys() {
        let prev = Settings::default();
        for accel in ["Space", "KeyA", "A", "Digit1", "Enter"] {
            let next = Settings { hotkey: accel.into(), ..prev.clone() };
            assert_eq!(
                plan_hotkey_change(&prev, &next, true),
                Err(format!("Dictation hotkey \"{accel}\" {NEEDS_MOD}"))
            );
            let next = Settings { session_hotkey: accel.into(), ..prev.clone() };
            assert_eq!(
                plan_hotkey_change(&prev, &next, true),
                Err(format!("Session hotkey \"{accel}\" {NEEDS_MOD}"))
            );
        }
    }

    #[test]
    fn plan_allows_f_keys_non_typing_keys_and_modified_keys() {
        let prev = Settings::default();
        for accel in ["F9", "Pause", "CommandOrControl+Space", "Shift+KeyA"] {
            let next = settings(accel, "Alt+F10");
            assert!(plan_hotkey_change(&prev, &next, true).unwrap().is_some(), "dictation {accel}");
            let next = settings("Alt+F10", accel);
            assert!(plan_hotkey_change(&prev, &next, true).unwrap().is_some(), "session {accel}");
        }
        // A blank session hotkey still means none, not a bare key.
        let next = settings("F9", "");
        let keys = plan_hotkey_change(&prev, &next, true).unwrap().unwrap();
        assert_eq!(keys.session, None);
    }

    #[test]
    fn plan_register_skips_a_bare_typing_key_from_settings_json() {
        // Dictation bare: skipped with the error, session still registered.
        let plan = plan_register(&settings("Space", "CommandOrControl+Shift+R"));
        assert_eq!(plan.dictation, Err(format!("Dictation hotkey \"Space\" {NEEDS_MOD}")));
        assert_eq!(plan.session, Ok(Some(parse_accelerator("CommandOrControl+Shift+R").unwrap())));
        // Session bare: skipped with the error, dictation still registered.
        let plan = plan_register(&settings("CommandOrControl+Shift+Space", "KeyA"));
        assert_eq!(plan.dictation, Ok(parse_accelerator("CommandOrControl+Shift+Space").unwrap()));
        assert_eq!(plan.session, Err(format!("Session hotkey \"KeyA\" {NEEDS_MOD}")));
    }

    #[test]
    fn plan_register_keeps_the_existing_boot_rules() {
        let d = Settings::default();
        let plan = plan_register(&d);
        assert!(plan.dictation.is_ok() && matches!(plan.session, Ok(Some(_))), "{plan:?}");
        let plan = plan_register(&settings("F9", "  "));
        assert_eq!(plan, RegisterPlan { dictation: Ok(parse_accelerator("F9").unwrap()), session: Ok(None) });
        let plan = plan_register(&settings("Ctrl+Shft+Space", "Alt+F9"));
        assert!(plan.dictation.unwrap_err().starts_with("Invalid dictation hotkey"));
        assert!(plan.session.is_ok());
        let plan = plan_register(&settings("Ctrl+Shift+R", "CommandOrControl+Shift+KeyR"));
        assert!(plan.dictation.is_ok());
        assert!(plan.session.unwrap_err().starts_with("Session hotkey \"CommandOrControl+Shift+KeyR\" is the same"));
    }

    #[test]
    fn register_error_explains_conflicts() {
        let e = tauri_plugin_global_shortcut::Error::GlobalHotkey(
            "HotKey already registered: HotKey { mods: Modifiers(CONTROL), key: Space, id: 1 }".into(),
        );
        assert_eq!(
            register_error("dictation", "Ctrl+Space", &e),
            "Could not register dictation hotkey \"Ctrl+Space\": it is already in use by another app"
        );
        let e = tauri_plugin_global_shortcut::Error::GlobalHotkey("boom".into());
        assert_eq!(
            register_error("session", "Alt+F9", &e),
            "Could not register session hotkey \"Alt+F9\": boom"
        );
    }
}
