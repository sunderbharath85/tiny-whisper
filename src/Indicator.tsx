import { type ReactElement, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Mic, Loader2, Radio } from "lucide-react";

type Status = {
  state: "idle" | "listening" | "speaking" | "transcribing" | "error";
  message?: string;
};

const META: Record<Status["state"], { label: string; dot: string; icon: ReactElement }> = {
  idle:         { label: "tiny-whisper",    dot: "bg-neutral-400",                   icon: <Mic className="h-3.5 w-3.5" /> },
  listening:    { label: "listening",       dot: "bg-emerald-400 animate-pulse",     icon: <Radio className="h-3.5 w-3.5" /> },
  speaking:     { label: "hearing you",     dot: "bg-amber-400 animate-pulse",       icon: <Mic className="h-3.5 w-3.5" /> },
  transcribing: { label: "transcribing…",   dot: "bg-sky-400 animate-pulse",         icon: <Loader2 className="h-3.5 w-3.5 animate-spin" /> },
  error:        { label: "error",           dot: "bg-red-500",                        icon: <Mic className="h-3.5 w-3.5" /> },
};

export default function Indicator() {
  const [status, setStatus] = useState<Status>({ state: "listening" });

  useEffect(() => {
    const off = listen<Status>("app://status", (e) => setStatus(e.payload));
    return () => { off.then((f) => f()); };
  }, []);

  const meta = META[status.state];
  const label = status.state === "error" && status.message ? status.message : meta.label;

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
