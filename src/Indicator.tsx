import { useEffect, useRef, useState } from "react";
import { api, type AppStatus } from "./api";
import { moodOf, type Mood } from "./lib/catalog";

const LABEL: Record<AppStatus["state"], string> = {
  idle: "Tiny Whisper",
  listening: "Listening",
  speaking: "Hearing you",
  transcribing: "Writing",
  recording_session: "Recording",
  transcribing_session: "Transcribing",
  error: "Error",
};

/// Bar heights (0–1) and speed per mood.
const BARS: Record<Mood, { h: number[]; speed: number; color: string }> = {
  idle: { h: [0.2, 0.3, 0.2, 0.3, 0.2], speed: 0, color: "rgba(255,255,255,.45)" },
  listening: { h: [0.3, 0.5, 0.35, 0.5, 0.3], speed: 1.6, color: "var(--ember)" },
  speaking: { h: [0.7, 1, 0.8, 1, 0.65], speed: 0.6, color: "var(--ember)" },
  thinking: { h: [0.35, 0.35, 0.35, 0.35, 0.35], speed: 1, color: "#9cc7ff" },
  recording: { h: [0.6, 0.9, 0.7, 0.9, 0.6], speed: 0.9, color: "var(--ember)" },
  error: { h: [0.25, 0.25, 0.25, 0.25, 0.25], speed: 0, color: "var(--danger)" },
};

/// How long an error stays on the pill even if other statuses arrive (K6).
const ERROR_HOLD_MS = 5000;

export default function Indicator() {
  const [latest, setLatest] = useState<AppStatus>({ state: "idle" });
  const [heldError, setHeldError] = useState<string | null>(null);
  const holdTimer = useRef<number | null>(null);

  useEffect(() => {
    document.body.style.background = "transparent";
    const off = api.onStatus((s) => {
      setLatest(s);
      if (s.state === "error") {
        setHeldError(s.message ?? "");
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

  const status: AppStatus = heldError != null ? { state: "error", message: heldError } : latest;
  const mood = moodOf(status);
  const bars = BARS[mood];
  const label =
    status.state === "error" && status.message
      ? status.message
      : status.state === "transcribing_session"
      ? `Transcribing ${status.percent.toFixed(0)}%`
      : LABEL[status.state];

  return (
    <div className="h-screen w-screen flex items-center justify-center p-2">
      <div
        title={label}
        className="flex w-full items-center gap-3 rounded-full border border-white/10 bg-[oklch(0.17_0.006_70/0.88)] py-2 pl-3.5 pr-4 text-white shadow-2xl backdrop-blur-xl"
      >
        <span className="flex h-4 items-center gap-[2.5px]" aria-hidden="true">
          {bars.h.map((h, i) => (
            <span
              key={i}
              className="w-[2.5px] rounded-full transition-all duration-500"
              style={{
                height: `${h * 100}%`,
                background: bars.color,
                animation:
                  mood === "thinking"
                    ? `pulse-op 1s ease-in-out ${i * 0.12}s infinite`
                    : bars.speed
                    ? `bar ${bars.speed + i * 0.11}s ease-in-out ${i * 0.07}s infinite`
                    : undefined,
              }}
            />
          ))}
        </span>
        <span className={`truncate text-[12px] font-medium tracking-tight ${mood === "idle" ? "font-display italic text-[14px] text-white/80" : ""}`}>
          {label}
        </span>
      </div>
      <style>{`@keyframes pulse-op { 0%,100% { opacity:.25 } 50% { opacity:1 } }`}</style>
    </div>
  );
}
