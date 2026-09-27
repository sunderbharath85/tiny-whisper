import { useState, type KeyboardEvent } from "react";
import { acceleratorFromEvent, acceleratorLabels, heldModifiers } from "@/lib/accelerator";
import { cn } from "@/lib/utils";

type Props = {
  /// id of the control; the visible caption is expected at `${id}-label`.
  id: string;
  value: string;
  onChange: (accelerator: string) => void;
  /// Backspace/Delete while recording clears the value (blank = no shortcut).
  clearable?: boolean;
};

/// "Press keys to record" shortcut field. Click it (or focus it and press
/// Enter/Space), then press the new combination. Escape or leaving the field
/// cancels; plain Tab / Shift+Tab still move focus.
export function HotkeyCapture({ id, value, onChange, clearable = false }: Props) {
  const [recording, setRecording] = useState(false);
  const [held, setHeld] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  function stop() {
    setRecording(false);
    setHeld("");
  }

  function onKeyDown(e: KeyboardEvent<HTMLButtonElement>) {
    if (!recording) return;
    // Let plain Tab / Shift+Tab move focus out; onBlur cancels recording.
    if (e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey) return;
    e.preventDefault();
    e.stopPropagation();
    const bare = !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey;
    if (clearable && bare && (e.code === "Backspace" || e.code === "Delete")) {
      setProblem(null);
      stop();
      onChange("");
      return;
    }
    const r = acceleratorFromEvent(e);
    switch (r.kind) {
      case "cancel":
        setProblem(null);
        stop();
        break;
      case "pending":
        setHeld(r.modifiers);
        break;
      case "invalid":
        setHeld("");
        setProblem(r.reason);
        break;
      case "ok":
        setProblem(null);
        stop();
        onChange(r.accelerator);
        break;
    }
  }

  function onKeyUp(e: KeyboardEvent<HTMLButtonElement>) {
    if (!recording) return;
    e.preventDefault();
    setHeld(heldModifiers(e));
  }

  const shown = recording ? (held ? `${held}+…` : "") : value;
  const labels = acceleratorLabels(shown);

  return (
    <div className="space-y-1">
      <button
        type="button"
        id={id}
        aria-labelledby={`${id}-label ${id}-value`}
        aria-describedby={`${id}-hint`}
        onClick={() => {
          if (recording) return;
          setProblem(null);
          setRecording(true);
        }}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        onBlur={() => {
          setProblem(null);
          stop();
        }}
        className={cn(
          "flex h-9 w-full items-center gap-1 rounded-md border px-3 text-left text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-accent)]/50",
          recording ? "border-[var(--color-accent)]" : "border-[var(--color-border)]",
          problem && "border-[var(--color-destructive)]"
        )}
      >
        <span id={`${id}-value`} className="flex items-center gap-1 min-w-0">
          {labels.length > 0 ? (
            labels.map((l, i) => (
              <kbd
                key={i}
                className="rounded border border-[var(--color-border)] bg-[var(--color-muted)] px-1.5 py-0.5 font-sans text-xs"
              >
                {l}
              </kbd>
            ))
          ) : (
            <span className="text-[var(--color-muted-foreground)]">
              {recording ? "Press keys…" : "Not set"}
            </span>
          )}
        </span>
      </button>
      <div
        id={`${id}-hint`}
        aria-live="polite"
        className={cn(
          "text-[11px]",
          problem ? "text-[var(--color-destructive)]" : "text-[var(--color-muted-foreground)]"
        )}
      >
        {problem
          ? problem
          : recording
          ? `Hold Ctrl, Alt, Shift or Win and press a key. Esc cancels.${clearable ? " Backspace clears." : ""}`
          : "Click or press Enter to record a new shortcut."}
      </div>
    </div>
  );
}
