import { type ReactElement, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Mic, Loader2, Radio } from "lucide-react";

type StatusState =
  | "idle"
  | "listening"
  | "speaking"
  | "transcribing"
  | "recording_session"
  | "transcribing_session"
  | "error";

type Status = {
  state: StatusState;
  message?: string;
  session_id?: string;
  percent?: number;
};

const META: Record<StatusState, { label: string; dot: string; icon: ReactElement }> = {
  idle:                 { label: "tiny-whisper",    dot: "bg-neutral-400",               icon: <Mic className="h-3.5 w-3.5" /> },
  listening:            { label: "listening",       dot: "bg-emerald-400 animate-pulse", icon: <Radio className="h-3.5 w-3.5" /> },
  speaking:             { label: "hearing you",     dot: "bg-amber-400 animate-pulse",   icon: <Mic className="h-3.5 w-3.5" /> },
  transcribing:         { label: "transcribing…",   dot: "bg-sky-400 animate-pulse",     icon: <Loader2 className="h-3.5 w-3.5 animate-spin" /> },
  recording_session:    { label: "recording",       dot: "bg-red-500 animate-pulse",     icon: <Radio className="h-3.5 w-3.5" /> },
  transcribing_session: { label: "transcribing…",   dot: "bg-sky-400 animate-pulse",     icon: <Loader2 className="h-3.5 w-3.5 animate-spin" /> },
  error:                { label: "error",           dot: "bg-red-500",                   icon: <Mic className="h-3.5 w-3.5" /> },
};

/// How long an error stays on the pill even if other statuses arrive (K6).
const ERROR_HOLD_MS = 5000;

export default function Indicator() {
  const [latest, setLatest] = useState<Status>({ state: "idle" });
  const [heldError, setHeldError] = useState<string | null>(null);
  const holdTimer = useRef<number | null>(null);

  useEffect(() => {
    const off = listen<Status>("app://status", (e) => {
      setLatest(e.payload);
      if (e.payload.state === "error") {
        // Show the error for ERROR_HOLD_MS, then resume the latest status.
        setHeldError(e.payload.message ?? "");
        if (holdTimer.current != null) window.clearTimeout(holdTimer.current);
        holdTimer.current = window.setTimeout(() => {
          holdTimer.current = null;
          setHeldError(null);
        }, ERROR_HOLD_MS);
      }
    });
    return () => {
      off.then((f) => f());
      if (holdTimer.current != null) window.clearTimeout(holdTimer.current);
    };
  }, []);

  const status: Status = heldError != null ? { state: "error", message: heldError } : latest;
  const meta = META[status.state];
  const label =
    status.state === "error" && status.message
      ? status.message
      : status.state === "transcribing_session" && status.percent != null
      ? `transcribing… ${status.percent.toFixed(0)}%`
      : meta.label;

  return (
    <div className="h-screen w-screen flex items-center justify-center p-2">
      <div className="flex items-center gap-2.5 rounded-full bg-neutral-900/90 backdrop-blur-md border border-white/10 px-4 py-2 shadow-2xl text-white text-xs w-full">
        <span className={`inline-block h-2 w-2 rounded-full ${meta.dot} flex-shrink-0`} />
        <span className="opacity-80 flex-shrink-0">{meta.icon}</span>
        <span className="truncate">{label}</span>
      </div>
    </div>
  );
}
