// KeyboardEvent → global-shortcut accelerator mapping (contract K3).
//
// Output format: `[CommandOrControl+][Alt+][Shift+][Super+]<KeyboardEvent.code>`,
// modifiers always in that order, e.g. `CommandOrControl+Shift+Space`,
// `CommandOrControl+Shift+KeyR`, `Alt+F9`, `CommandOrControl+Alt+Digit1`.
// On macOS the backend reads `CommandOrControl` as Cmd, so there Cmd maps to
// `CommandOrControl` and Ctrl to `Control`:
// `[CommandOrControl+][Control+][Alt+][Shift+]<code>`.
// The backend parses this with global-hotkey's `HotKey::from_str`, which
// matches key tokens case-insensitively against the same names as
// `KeyboardEvent.code`, so the code is emitted unchanged.
//
// Pure apart from reading the platform once from `navigator`.

const IS_MAC = typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

/// The modifier keys, as this platform names them.
export const MODIFIER_NAMES = IS_MAC ? "Cmd, Ctrl, Option or Shift" : "Ctrl, Alt, Shift or Win";

/// The subset of `KeyboardEvent` the mapper reads.
export type KeyInput = {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
};

export type AcceleratorResult =
  | { kind: "ok"; accelerator: string }
  /// Only modifiers are held so far; `modifiers` is the partial accelerator.
  | { kind: "pending"; modifiers: string }
  | { kind: "cancel" }
  | { kind: "invalid"; reason: string };

const MODIFIER_CODES = new Set([
  "ControlLeft", "ControlRight",
  "AltLeft", "AltRight", "AltGraph",
  "ShiftLeft", "ShiftRight",
  "MetaLeft", "MetaRight", "OSLeft", "OSRight",
]);

const FUNCTION_KEY = /^F([1-9]|1[0-9]|2[0-4])$/;

/// Every `KeyboardEvent.code` that global-hotkey 0.7 `parse_key` accepts.
/// Escape is left out on purpose: it cancels recording.
const SUPPORTED_CODES = new Set<string>([
  "Backquote", "Backslash", "BracketLeft", "BracketRight", "Comma", "Equal",
  "Minus", "Period", "Quote", "Semicolon", "Slash",
  ..."0123456789".split("").map((d) => `Digit${d}`),
  ..."ABCDEFGHIJKLMNOPQRSTUVWXYZ".split("").map((c) => `Key${c}`),
  "Backspace", "CapsLock", "Enter", "Space", "Tab", "Delete", "End", "Home",
  "Insert", "PageDown", "PageUp", "PrintScreen", "ScrollLock", "Pause",
  "ArrowDown", "ArrowLeft", "ArrowRight", "ArrowUp",
  "NumLock",
  ..."0123456789".split("").map((d) => `Numpad${d}`),
  "NumpadAdd", "NumpadDecimal", "NumpadDivide", "NumpadEnter", "NumpadEqual",
  "NumpadMultiply", "NumpadSubtract",
  ...Array.from({ length: 24 }, (_, i) => `F${i + 1}`),
  "AudioVolumeDown", "AudioVolumeUp", "AudioVolumeMute",
  "MediaPlay", "MediaPause", "MediaPlayPause", "MediaStop", "MediaTrackNext",
  "MediaTrackPrevious",
]);

export function isModifierCode(code: string): boolean {
  return MODIFIER_CODES.has(code);
}

function modifierPrefix(e: Omit<KeyInput, "code">): string[] {
  const mods: string[] = [];
  if (IS_MAC) {
    if (e.metaKey) mods.push("CommandOrControl");
    if (e.ctrlKey) mods.push("Control");
  } else if (e.ctrlKey) {
    mods.push("CommandOrControl");
  }
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (!IS_MAC && e.metaKey) mods.push("Super");
  return mods;
}

/// The modifiers currently held, as a partial accelerator (`""` if none).
export function heldModifiers(e: Omit<KeyInput, "code">): string {
  return modifierPrefix(e).join("+");
}

/// Map one keydown to an accelerator, a partial (modifiers only), a cancel
/// (Escape), or a reason the combination can't be used.
export function acceleratorFromEvent(e: KeyInput): AcceleratorResult {
  const mods = modifierPrefix(e);

  if (e.code === "Escape") return { kind: "cancel" };
  if (isModifierCode(e.code)) return { kind: "pending", modifiers: mods.join("+") };
  if (!SUPPORTED_CODES.has(e.code)) {
    return {
      kind: "invalid",
      reason: e.code
        ? `${e.code} can't be used in a shortcut. Try a letter, digit, F-key or Space.`
        : "That key can't be used in a shortcut. Try a letter, digit, F-key or Space.",
    };
  }
  if (mods.length === 0 && !FUNCTION_KEY.test(e.code)) {
    return {
      kind: "invalid",
      reason: `Add ${MODIFIER_NAMES} to ${keyLabel(e.code)}. Only F-keys can be used alone.`,
    };
  }
  return { kind: "ok", accelerator: [...mods, e.code].join("+") };
}

// `CommandOrControl` is Cmd on macOS and Ctrl elsewhere, like the backend.
const CMD_OR_CTRL = IS_MAC ? "Cmd" : "Ctrl";
const SUPER_LABEL = IS_MAC ? "Cmd" : "Win";
const ALT_LABEL = IS_MAC ? "Option" : "Alt";

const TOKEN_LABEL: Record<string, string> = {
  COMMANDORCONTROL: CMD_OR_CTRL,
  COMMANDORCTRL: CMD_OR_CTRL,
  CMDORCTRL: CMD_OR_CTRL,
  CMDORCONTROL: CMD_OR_CTRL,
  CONTROL: "Ctrl",
  CTRL: "Ctrl",
  ALT: ALT_LABEL,
  OPTION: ALT_LABEL,
  SHIFT: "Shift",
  SUPER: SUPER_LABEL,
  COMMAND: SUPER_LABEL,
  CMD: SUPER_LABEL,
  META: SUPER_LABEL,
  BACKQUOTE: "`",
  BACKSLASH: "\\",
  BRACKETLEFT: "[",
  BRACKETRIGHT: "]",
  COMMA: ",",
  EQUAL: "=",
  MINUS: "-",
  PERIOD: ".",
  QUOTE: "'",
  SEMICOLON: ";",
  SLASH: "/",
  ARROWUP: "Up",
  ARROWDOWN: "Down",
  ARROWLEFT: "Left",
  ARROWRIGHT: "Right",
  PAGEUP: "Page Up",
  PAGEDOWN: "Page Down",
  PRINTSCREEN: "Print Screen",
  CAPSLOCK: "Caps Lock",
  NUMLOCK: "Num Lock",
  SCROLLLOCK: "Scroll Lock",
  ESCAPE: "Esc",
};

/// Human label for one accelerator token (`KeyR` → `R`, `Digit1` → `1`,
/// `CommandOrControl` → `Ctrl`, or `Cmd` on macOS). Unknown tokens are shown as-is.
export function keyLabel(token: string): string {
  const t = token.trim();
  const upper = t.toUpperCase();
  if (TOKEN_LABEL[upper]) return TOKEN_LABEL[upper];
  let m = /^KEY([A-Z])$/.exec(upper);
  if (m) return m[1];
  m = /^DIGIT([0-9])$/.exec(upper);
  if (m) return m[1];
  m = /^NUMPAD([0-9])$/.exec(upper);
  if (m) return `Num ${m[1]}`;
  if (/^[a-z]$/.test(t)) return upper;
  return t;
}

/// Split a saved accelerator into display labels. Accepts any saved value,
/// including older spellings like `CommandOrControl+Shift+R`.
export function acceleratorLabels(accelerator: string): string[] {
  return accelerator
    .split("+")
    .map((t) => t.trim())
    .filter((t) => t.length > 0)
    .map(keyLabel);
}
