import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Check } from "lucide-react";
import { acceleratorLabels } from "@/lib/accelerator";
import { cn } from "@/lib/utils";

/// Keycaps for an accelerator string like `CommandOrControl+Shift+Space`.
export function Keys({ accel, size = "md", className }: { accel: string; size?: "sm" | "md" | "lg"; className?: string }) {
  const labels = acceleratorLabels(accel);
  if (labels.length === 0) return <span className="text-muted-foreground text-xs">Not set</span>;
  return (
    <span className={cn("inline-flex items-center gap-1", className)}>
      {labels.map((l, i) => (
        <kbd
          key={i}
          className={cn(
            "font-mono inline-flex items-center justify-center rounded-[7px] border border-border-strong bg-surface text-fg",
            "shadow-[0_1px_0_var(--rule-strong)]",
            size === "sm" && "h-5 min-w-5 px-1.5 text-[10.5px]",
            size === "md" && "h-6 min-w-6 px-2 text-[11.5px]",
            size === "lg" && "h-8 min-w-8 px-2.5 text-[13px]"
          )}
        >
          {l}
        </kbd>
      ))}
    </span>
  );
}

/// iOS-style switch.
export function Switch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full transition-colors duration-300 disabled:opacity-40",
        checked ? "bg-fg" : "bg-border-strong"
      )}
    >
      <span
        className={cn(
          "inline-block h-[18px] w-[18px] rounded-full bg-bg shadow-sm transition-transform duration-300 ease-[cubic-bezier(.3,1.4,.5,1)]",
          checked ? "translate-x-[18px]" : "translate-x-[2px]"
        )}
      />
    </button>
  );
}

/// A row of mutually exclusive options.
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string; disabled?: boolean; hint?: string }[];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex rounded-full border border-border bg-muted p-0.5">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          disabled={o.disabled}
          title={o.hint}
          onClick={() => onChange(o.value)}
          className={cn(
            "h-7 rounded-full px-3.5 text-xs font-medium transition-all disabled:opacity-35 disabled:cursor-not-allowed",
            value === o.value ? "bg-surface text-fg shadow-[var(--shadow-lift)]" : "text-muted-foreground hover:text-fg"
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/// 1–5 dot meter for speed/accuracy.
export function Meter({ value, label }: { value: number; label: string }) {
  return (
    <span className="flex items-center gap-2" aria-label={`${label} ${value} of 5`}>
      <span className="eyebrow !text-[9.5px] w-[66px]">{label}</span>
      <span className="inline-flex gap-[3px]">
        {[1, 2, 3, 4, 5].map((i) => (
          <span key={i} className={cn("h-[5px] w-3 rounded-full", i <= value ? "bg-fg" : "bg-border-strong")} />
        ))}
      </span>
    </span>
  );
}

/// Ring that fills with download progress.
export function ProgressRing({ pct, size = 18 }: { pct: number | null; size?: number }) {
  const r = size / 2 - 2;
  const c = 2 * Math.PI * r;
  return (
    <svg width={size} height={size} className={pct == null ? "animate-spin" : ""} aria-hidden="true">
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--rule-strong)" strokeWidth="2" />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="var(--ember)"
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - (pct ?? 25) / 100)}
        transform={`rotate(-90 ${size / 2} ${size / 2})`}
        style={{ transition: "stroke-dashoffset .2s" }}
      />
    </svg>
  );
}

/// A button that needs a second click to confirm (no modal dialogs).
export function ConfirmButton({
  onConfirm,
  children,
  confirmLabel = "Confirm?",
  className,
  ariaLabel,
}: {
  onConfirm: () => void;
  children: ReactNode;
  confirmLabel?: string;
  className?: string;
  ariaLabel: string;
}) {
  const [armed, setArmed] = useState(false);
  const timer = useRef<number | null>(null);
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);
  return (
    <button
      type="button"
      aria-label={armed ? `${ariaLabel} — click again to confirm` : ariaLabel}
      onClick={() => {
        if (armed) {
          setArmed(false);
          onConfirm();
          return;
        }
        setArmed(true);
        timer.current = window.setTimeout(() => setArmed(false), 3000);
      }}
      onBlur={() => setArmed(false)}
      className={cn(
        "inline-flex h-8 items-center gap-1.5 rounded-full px-2.5 text-xs font-medium transition-all",
        armed ? "bg-destructive text-white px-3" : "text-muted-foreground hover:text-fg hover:bg-muted",
        className
      )}
    >
      {armed ? confirmLabel : children}
    </button>
  );
}

/* ---------- toasts ---------- */

type Toast = { id: number; text: string };
const ToastCtx = createContext<(text: string) => void>(() => {});
export const useToast = () => useContext(ToastCtx);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toast, setToast] = useState<Toast | null>(null);
  const timer = useRef<number | null>(null);
  const show = useCallback((text: string) => {
    if (timer.current) clearTimeout(timer.current);
    setToast({ id: Date.now(), text });
    timer.current = window.setTimeout(() => setToast(null), 1800);
  }, []);
  return (
    <ToastCtx.Provider value={show}>
      {children}
      <div aria-live="polite" className="pointer-events-none fixed bottom-6 left-1/2 z-50">
        {toast && (
          <div
            key={toast.id}
            className="flex items-center gap-2 rounded-full bg-fg px-4 py-2 text-xs font-medium text-bg shadow-xl"
            style={{ animation: "toast-in .35s cubic-bezier(.2,.8,.2,1) both", transform: "translateX(-50%)" }}
          >
            <Check className="h-3.5 w-3.5" /> {toast.text}
          </div>
        )}
      </div>
    </ToastCtx.Provider>
  );
}

/// Page header: serif title with a mono index and a short lede.
export function PageHead({ index, title, lede, right }: { index: string; title: ReactNode; lede?: ReactNode; right?: ReactNode }) {
  return (
    <header className="flex items-end justify-between gap-6 rise">
      <div>
        <div className="eyebrow mb-3">{index}</div>
        <h1 className="display text-[52px]">{title}</h1>
        {lede && <p className="mt-3 max-w-[46ch] text-[13.5px] leading-relaxed text-ink-2">{lede}</p>}
      </div>
      {right}
    </header>
  );
}
