import { useCallback, useState } from "react";
import { AlertCircle, X } from "lucide-react";

export type ErrorEntry = { id: number; message: string };

const MAX_ERRORS = 5;
let nextId = 1;

/// State for the settings window's single error area. An error stays until
/// it is dismissed or `clear()` runs at the start of the next user action.
export function useErrorList() {
  const [errors, setErrors] = useState<ErrorEntry[]>([]);
  const report = useCallback((message: string) => {
    setErrors((prev) =>
      prev.some((p) => p.message === message)
        ? prev
        : [...prev, { id: nextId++, message }].slice(-MAX_ERRORS)
    );
  }, []);
  const dismiss = useCallback((id: number) => {
    setErrors((prev) => prev.filter((p) => p.id !== id));
  }, []);
  const clear = useCallback(() => setErrors([]), []);
  return { errors, report, dismiss, clear };
}

/// Sits directly under the header and sticks to the top while scrolling, so
/// a failure after clicking Save at the bottom is still on screen.
export function ErrorArea({ errors, onDismiss }: { errors: ErrorEntry[]; onDismiss: (id: number) => void }) {
  if (errors.length === 0) return null;
  return (
    <div className="sticky top-2 z-10 space-y-2">
      {errors.map((e) => (
        <div
          key={e.id}
          role="alert"
          className="flex items-start gap-2 rounded-[calc(var(--radius)-0.25rem)] border border-[var(--color-destructive)] bg-[var(--color-surface)] px-3 py-2 text-xs text-[var(--color-destructive)] shadow-sm"
        >
          <AlertCircle className="h-4 w-4 flex-shrink-0" aria-hidden="true" />
          <div className="flex-1 min-w-0 whitespace-pre-wrap break-words">{e.message}</div>
          <button
            type="button"
            onClick={() => onDismiss(e.id)}
            aria-label="Dismiss error"
            className="flex-shrink-0 rounded p-0.5 hover:bg-[var(--color-muted)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--color-accent)]/50"
          >
            <X className="h-3.5 w-3.5" aria-hidden="true" />
          </button>
        </div>
      ))}
    </div>
  );
}
