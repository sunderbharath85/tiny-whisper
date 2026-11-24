import { useEffect, useMemo, useRef, useState } from "react";
import { useStore } from "@/App";
import { api, type SessionMeta, type Transcript } from "@/api";
import { ConfirmButton, Keys, PageHead, Switch, useToast } from "@/components/bits";
import { DIARIZER_ID, fmtDuration, modelName, relativeTime } from "@/lib/catalog";
import { errorMessage } from "@/lib/errors";
import { cn } from "@/lib/utils";
import { Check, ClipboardCopy, FileText, Loader2, Pencil, Trash2, Users } from "lucide-react";

const SPEAKER_HUES = ["var(--ember)", "var(--sky)", "var(--moss)", "oklch(0.66 0.14 300)"];

/// Speaker names are a UI nicety kept per session in localStorage.
function useSpeakerNames(id: string | null) {
  const key = id ? `tw.speakers.${id}` : null;
  const [names, setNames] = useState<Record<number, string>>({});
  useEffect(() => {
    if (!key) return;
    try { setNames(JSON.parse(localStorage.getItem(key) ?? "{}")); } catch { setNames({}); }
  }, [key]);
  const rename = (i: number, name: string) => {
    const next = { ...names, [i]: name };
    setNames(next);
    try { if (key) localStorage.setItem(key, JSON.stringify(next)); } catch { /* storage unavailable */ }
  };
  return { names, rename };
}

export function SessionsPage() {
  const { status, runAction, report, settings, isDownloaded } = useStore();
  const [sessions, setSessions] = useState<SessionMeta[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [transcripts, setTranscripts] = useState<Record<string, Transcript>>({});
  const [diarize, setDiarize] = useState(true);
  const recording = status.state === "recording_session";

  useEffect(() => {
    let alive = true;
    api.listSessions()
      .then((s) => {
        if (!alive) return;
        setSessions(s);
        setSelected((cur) => cur ?? s[0]?.id ?? null);
      })
      .catch((e) => alive && report(`Could not load sessions: ${errorMessage(e)}`));
    const off = api
      .onSessionUpdated((m) => {
        setSessions((prev) => {
          const exists = prev.some((p) => p.id === m.id);
          return exists ? prev.map((p) => (p.id === m.id ? m : p)) : [m, ...prev];
        });
        // A finished transcript replaces the cached one.
        setTranscripts(({ [m.id]: _, ...rest }) => rest);
        setSelected((cur) => cur ?? m.id);
      })
      .catch((e): undefined => { report(`Could not listen for session updates: ${errorMessage(e)}`); return undefined; });
    return () => { alive = false; off.then((f) => f?.()); };
  }, [report]);

  const current = sessions.find((s) => s.id === selected) ?? null;

  useEffect(() => {
    if (!current?.has_transcript || transcripts[current.id]) return;
    runAction("Could not load the transcript", async () => {
      const t = await api.getSessionTranscript(current.id);
      setTranscripts((prev) => ({ ...prev, [current.id]: t }));
    });
  }, [current?.id, current?.has_transcript]);

  async function remove(id: string) {
    await runAction("Could not delete the session", async () => {
      await api.deleteSession(id);
      setSessions((prev) => {
        const next = prev.filter((s) => s.id !== id);
        if (selected === id) setSelected(next[0]?.id ?? null);
        return next;
      });
    });
  }

  const canDiarize = isDownloaded(DIARIZER_ID);

  return (
    <div>
      <PageHead
        index="02 — Sessions"
        title={<>Meetings, <span className="italic">kept.</span></>}
        lede={<>Record a whole conversation, then transcribe it with speakers labelled. {settings.session_hotkey && <>Start from anywhere with <Keys accel={settings.session_hotkey} size="sm" className="align-[-3px]" />.</>}</>}
        right={<RecordButton recording={recording} />}
      />

      <div className="mt-10 grid grid-cols-[260px_1fr] gap-8 rise rise-2">
        <ul className="space-y-1" aria-label="Recorded sessions">
          {recording && (
            <li className="flex items-center gap-3 rounded-[12px] border border-ember/40 bg-ember-soft px-3.5 py-3">
              <span className="h-2 w-2 rounded-full bg-ember live-dot" />
              <span className="text-[13px] font-medium">Recording now…</span>
            </li>
          )}
          {sessions.length === 0 && !recording && (
            <li className="rounded-[12px] border border-dashed border-border-strong px-4 py-8 text-center text-xs text-muted-foreground">
              No sessions yet.
            </li>
          )}
          {sessions.map((s) => {
            const d = new Date(s.created_at);
            const active = s.id === selected;
            const busy = status.state === "transcribing_session" && status.session_id === s.id;
            return (
              <li key={s.id}>
                <button
                  onClick={() => setSelected(s.id)}
                  aria-current={active}
                  className={cn(
                    "w-full rounded-[12px] px-3.5 py-3 text-left transition-all",
                    active ? "bg-surface shadow-[var(--shadow-lift)]" : "hover:bg-surface/60"
                  )}
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-[13px] font-medium">{relativeTime(d)}</span>
                    <span className="tabular text-[10.5px] text-muted-foreground">{fmtDuration(s.duration_secs)}</span>
                  </div>
                  <div className="mt-1 flex items-center gap-2 text-[11px] text-muted-foreground">
                    {busy ? (
                      <span className="text-ember">Transcribing {status.percent.toFixed(0)}%</span>
                    ) : s.has_transcript ? (
                      <>
                        <span>{s.model_used ? modelName(s.model_used) : "Transcribed"}</span>
                        {s.speaker_count != null && (
                          <span className="inline-flex items-center gap-1">
                            {Array.from({ length: s.speaker_count }, (_, i) => (
                              <span key={i} className="h-1.5 w-1.5 rounded-full" style={{ background: SPEAKER_HUES[i % 4] }} />
                            ))}
                          </span>
                        )}
                      </>
                    ) : (
                      <span className="italic">Not transcribed</span>
                    )}
                  </div>
                </button>
              </li>
            );
          })}
        </ul>

        {current ? (
          <Reader
            key={current.id}
            meta={current}
            transcript={transcripts[current.id] ?? null}
            diarize={diarize && canDiarize}
            canDiarize={canDiarize}
            setDiarize={setDiarize}
            onDelete={() => remove(current.id)}
          />
        ) : (
          <div className="flex min-h-[320px] flex-col items-center justify-center rounded-[18px] border border-border text-center">
            <div className="display text-[34px] italic text-muted-foreground">An empty archive.</div>
            <p className="mt-2 max-w-[36ch] text-xs text-muted-foreground">
              Record a meeting, interview or lecture. Audio stays on this computer until you delete it.
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function RecordButton({ recording }: { recording: boolean }) {
  const { runAction, status } = useStore();
  const [secs, setSecs] = useState(0);
  const since = useRef<number | null>(null);

  useEffect(() => {
    if (!recording) { since.current = null; setSecs(0); return; }
    since.current ??= Date.now();
    const t = window.setInterval(() => setSecs(Math.floor((Date.now() - since.current!) / 1000)), 250);
    return () => clearInterval(t);
  }, [recording]);

  const dictating = ["listening", "speaking", "transcribing"].includes(status.state);

  return recording ? (
    <button
      onClick={() => runAction("Could not stop the session", () => api.stopSessionRecording())}
      className="group flex h-14 items-center gap-4 rounded-full bg-ember pl-5 pr-6 text-ember-ink shadow-[0_0_0_8px_var(--ember-soft)] transition-transform hover:scale-[1.02]"
    >
      <span className="flex h-5 items-end gap-[3px]" aria-hidden="true">
        {[0, 1, 2, 3, 4].map((i) => (
          <span key={i} className="w-[3px] h-full origin-bottom rounded-full bg-current" style={{ animation: `bar ${0.7 + i * 0.13}s ease-in-out ${i * 0.09}s infinite` }} />
        ))}
      </span>
      <span className="tabular text-[20px] font-medium">{fmtDuration(secs)}</span>
      <span className="ml-1 text-xs font-medium opacity-80">Stop</span>
    </button>
  ) : (
    <button
      disabled={dictating}
      title={dictating ? "Stop dictation first" : undefined}
      onClick={() => runAction("Could not start the session", () => api.startSessionRecording())}
      className="flex h-14 items-center gap-3 rounded-full bg-fg pl-5 pr-6 text-bg shadow-[var(--shadow-lift)] transition-transform hover:scale-[1.02] disabled:opacity-40"
    >
      <span className="h-3 w-3 rounded-full bg-ember" />
      <span className="text-[13px] font-medium">Record session</span>
    </button>
  );
}

function Reader({
  meta,
  transcript,
  diarize,
  canDiarize,
  setDiarize,
  onDelete,
}: {
  meta: SessionMeta;
  transcript: Transcript | null;
  diarize: boolean;
  canDiarize: boolean;
  setDiarize: (v: boolean) => void;
  onDelete: () => void;
}) {
  const { status, runAction, go } = useStore();
  const toast = useToast();
  const { names, rename } = useSpeakerNames(meta.id);
  const [editing, setEditing] = useState<number | null>(null);
  const d = new Date(meta.created_at);
  const busy = status.state === "transcribing_session" && status.session_id === meta.id;
  const pct = busy ? status.percent : 0;
  const speakerName = (i: number | null) => (i == null ? "" : names[i] || `Speaker ${i + 1}`);

  // Merge consecutive segments from the same speaker into one paragraph.
  const blocks = useMemo(() => {
    if (!transcript) return [];
    const out: { speaker: number | null; start: number; text: string[] }[] = [];
    for (const s of transcript.segments) {
      const last = out[out.length - 1];
      if (last && last.speaker === s.speaker) last.text.push(s.text);
      else out.push({ speaker: s.speaker, start: s.start_secs, text: [s.text] });
    }
    return out;
  }, [transcript]);

  const share = useMemo(() => {
    if (!transcript?.diarized) return [];
    const t: Record<number, number> = {};
    for (const s of transcript.segments) if (s.speaker != null) t[s.speaker] = (t[s.speaker] ?? 0) + (s.end_secs - s.start_secs);
    const total = Object.values(t).reduce((a, b) => a + b, 0) || 1;
    return Object.entries(t).map(([k, v]) => ({ speaker: Number(k), frac: v / total }));
  }, [transcript]);

  function asText(markdown: boolean) {
    const head = markdown ? `# Session — ${d.toLocaleString()}\n\n` : "";
    return head + blocks
      .map((b) => {
        const who = b.speaker != null ? speakerName(b.speaker) : "";
        const ts = fmtDuration(b.start);
        const body = b.text.join(" ").trim();
        return markdown ? `**${who || "Transcript"}** · \`${ts}\`\n\n${body}` : `${who ? `${who} [${ts}]: ` : ""}${body}`;
      })
      .join("\n\n");
  }
  async function copy(markdown: boolean) {
    try {
      await navigator.clipboard.writeText(asText(markdown));
      toast(markdown ? "Markdown copied" : "Transcript copied");
    } catch {
      toast("Couldn't reach the clipboard");
    }
  }

  return (
    <article className="min-w-0">
      <div className="flex items-center justify-between">
        <div className="eyebrow">{d.toLocaleDateString([], { weekday: "long" })}</div>
        <ConfirmButton ariaLabel="Delete session" confirmLabel="Delete for good?" onConfirm={onDelete}>
          <Trash2 className="h-3.5 w-3.5" /> Delete
        </ConfirmButton>
      </div>
      <h2 className="display mt-2 text-[40px]">
        {d.toLocaleDateString([], { day: "numeric", month: "long" })}
        <span className="italic text-muted-foreground"> at {d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</span>
      </h2>

      <div className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 text-[11.5px] text-muted-foreground">
        <span className="tabular">{fmtDuration(meta.duration_secs)}</span>
        {meta.model_used && <span>{modelName(meta.model_used)}</span>}
        {meta.speaker_count != null && (
          <span className="inline-flex items-center gap-1.5"><Users className="h-3.5 w-3.5" /> {meta.speaker_count} speakers</span>
        )}
      </div>

      {share.length > 0 && (
        <div className="mt-5">
          <div className="flex h-1.5 overflow-hidden rounded-full">
            {share.map((s) => (
              <span key={s.speaker} style={{ width: `${s.frac * 100}%`, background: SPEAKER_HUES[s.speaker % 4] }} />
            ))}
          </div>
          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
            {share.map((s) => (
              <span key={s.speaker} className="text-[11px] text-muted-foreground">
                <span className="mr-1.5 inline-block h-1.5 w-1.5 rounded-full align-middle" style={{ background: SPEAKER_HUES[s.speaker % 4] }} />
                {speakerName(s.speaker)} <span className="tabular">{Math.round(s.frac * 100)}%</span>
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Toolbar */}
      <div className="mt-6 flex flex-wrap items-center gap-1.5 border-y border-border py-3">
        <button
          disabled={busy || status.state === "recording_session"}
          onClick={() => runAction("Could not transcribe the session", () => api.transcribeSession(meta.id, diarize))}
          className="inline-flex h-8 items-center gap-2 rounded-full bg-fg px-3.5 text-xs font-medium text-bg disabled:opacity-50"
        >
          {busy ? <><Loader2 className="h-3.5 w-3.5 animate-spin" /> <span className="tabular">{pct.toFixed(0)}%</span></> : meta.has_transcript ? "Re-transcribe" : "Transcribe"}
        </button>
        <label className={cn("ml-1 inline-flex items-center gap-2 text-xs", canDiarize ? "text-ink-2" : "text-muted-foreground")}>
          <Switch checked={diarize} onChange={setDiarize} disabled={!canDiarize} label="Label speakers" />
          Label speakers
        </label>
        {!canDiarize && (
          <button onClick={() => go("models")} className="text-[11px] text-ember underline-offset-2 hover:underline">
            Get Sortformer
          </button>
        )}
        <span className="flex-1" />
        {transcript && (
          <>
            <button onClick={() => copy(false)} className="inline-flex h-8 items-center gap-1.5 rounded-full px-2.5 text-xs text-muted-foreground hover:bg-muted hover:text-fg">
              <ClipboardCopy className="h-3.5 w-3.5" /> Copy
            </button>
            <button onClick={() => copy(true)} className="inline-flex h-8 items-center gap-1.5 rounded-full px-2.5 text-xs text-muted-foreground hover:bg-muted hover:text-fg">
              <FileText className="h-3.5 w-3.5" /> Markdown
            </button>
          </>
        )}
      </div>
      {busy && (
        <div className="h-px w-full bg-border">
          <div className="h-px bg-ember transition-[width] duration-300" style={{ width: `${pct}%` }} />
        </div>
      )}

      {/* Body */}
      <div className="mt-8 space-y-7">
        {!meta.has_transcript ? (
          <p className="display text-[26px] italic text-muted-foreground">
            {busy ? "Listening back…" : "Not transcribed yet."}
          </p>
        ) : !transcript ? (
          <p className="shimmer-text text-sm">Loading transcript…</p>
        ) : (
          blocks.map((b, i) => (
            <div key={i} className="grid grid-cols-[64px_1fr] gap-4">
              <span className="tabular pt-1 text-[10.5px] text-muted-foreground">{fmtDuration(b.start)}</span>
              <div>
                {b.speaker != null && (
                  editing === b.speaker ? (
                    <SpeakerInput
                      initial={speakerName(b.speaker)}
                      onDone={(v) => { if (v.trim()) rename(b.speaker!, v.trim()); setEditing(null); }}
                    />
                  ) : (
                    <button
                      onClick={() => setEditing(b.speaker)}
                      className="group mb-1.5 inline-flex items-center gap-1.5 text-[11.5px] font-semibold"
                      style={{ color: SPEAKER_HUES[b.speaker % 4] }}
                      title="Rename speaker"
                    >
                      {speakerName(b.speaker)}
                      <Pencil className="h-3 w-3 opacity-0 transition-opacity group-hover:opacity-60" />
                    </button>
                  )
                )}
                <p className="text-[15px] leading-[1.7] text-fg/90">
                  {b.text.join(" ").trim() || <span className="italic text-muted-foreground">(silence)</span>}
                </p>
              </div>
            </div>
          ))
        )}
      </div>
    </article>
  );
}

function SpeakerInput({ initial, onDone }: { initial: string; onDone: (v: string) => void }) {
  const [v, setV] = useState(initial);
  return (
    <span className="mb-1.5 inline-flex items-center gap-1">
      <input
        autoFocus
        value={v}
        onChange={(e) => setV(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") onDone(v); if (e.key === "Escape") onDone(""); }}
        onBlur={() => onDone(v)}
        aria-label="Speaker name"
        className="h-6 w-40 rounded-md border border-border-strong bg-surface px-2 text-[12px] font-semibold outline-none focus:border-ember"
      />
      <Check className="h-3.5 w-3.5 text-muted-foreground" />
    </span>
  );
}
