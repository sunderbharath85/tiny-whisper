/// Readable text for whatever a rejected `invoke()` (or anything else) carries.
/// Tauri commands reject with the command's `Err(String)`.
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object" && "message" in e && typeof e.message === "string") {
    return e.message;
  }
  try {
    return JSON.stringify(e) ?? String(e);
  } catch {
    return String(e);
  }
}

/// Runs a user-triggered backend call. Starting it clears the previous errors
/// (they stay until dismissed or the next user action); a failure is logged
/// and shown in the error area as `${failure}: ${reason}`. Resolves to
/// whether `fn` succeeded.
export type RunAction = (failure: string, fn: () => Promise<unknown>) => Promise<boolean>;

// Failures outside any handler (window "error", "unhandledrejection") are
// reported here by main.tsx and shown by App once it has mounted.
type Sink = (message: string) => void;
let sink: Sink | null = null;
const pending: string[] = [];

export function reportUnexpected(message: string) {
  if (sink) sink(message);
  else pending.push(message);
}

export function setUnexpectedSink(next: Sink): () => void {
  sink = next;
  for (const m of pending.splice(0)) next(m);
  return () => {
    if (sink === next) sink = null;
  };
}
