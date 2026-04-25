import { useEffect, useMemo, useRef, useState } from "react";
import { api, type AppStatus, type SessionMeta, type Transcript } from "../api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  ChevronDown,
  ChevronRight,
  Loader2,
  Mic,
  Radio,
  Square,
  Trash2,
  Users,
} from "lucide-react";

type Props = {
  status: AppStatus;
};

export function SessionsCard({ status }: Props) {
  const [sessions, setSessions] = useState<SessionMeta[]>([]);
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [recordSeconds, setRecordSeconds] = useState(0);
  const [diarize, setDiarize] = useState(true);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [transcripts, setTranscripts] = useState<Record<string, Transcript>>({});
  const [busyId, setBusyId] = useState<string | null>(null);
  const tickRef = useRef<number | null>(null);

  // Reflect backend recording state coming in via app status.
  useEffect(() => {
    if (status.state === "recording_session" && !recordingId) {
      // Hotkey-triggered start: we don't know the id, but the writer will emit
      // an `updated` event when finalized; until then leave id undefined and
      // just show a recording chip.
      setRecordingId("__pending__");
    }
    if (status.state === "idle" && recordingId) {
      setRecordingId(null);
    }
  }, [status.state, recordingId]);

  // Refresh list on boot and on every "updated" event.
  useEffect(() => {
    let alive = true;
    api.listSessions().then((s) => alive && setSessions(s));
    const off = api.onSessionUpdated((m) => {
      setSessions((prev) => {
        const next = prev.filter((p) => p.id !== m.id);
        next.unshift(m);
        return next;
      });
    });
    return () => {
      alive = false;
      off.then((f) => f());
    };
  }, []);

  // Recording timer.
  useEffect(() => {
    if (recordingId) {
      setRecordSeconds(0);
      const start = Date.now();
      tickRef.current = window.setInterval(() => {
        setRecordSeconds(Math.floor((Date.now() - start) / 1000));
      }, 250);
    } else if (tickRef.current != null) {
      window.clearInterval(tickRef.current);
      tickRef.current = null;
    }
    return () => {
      if (tickRef.current != null) window.clearInterval(tickRef.current);
    };
  }, [recordingId]);

  const transcribingId = useMemo(() => {
    if (status.state === "transcribing_session") return status.session_id;
    return null;
  }, [status]);
  const transcribingPct = useMemo(() => {
    if (status.state === "transcribing_session") return status.percent;
    return 0;
  }, [status]);

  async function start() {
    try {
      const id = await api.startSessionRecording();
      setRecordingId(id);
    } catch (e) {
      console.error("start session:", e);
    }
  }

  async function stop() {
    try {
      await api.stopSessionRecording();
    } catch (e) {
      console.error("stop session:", e);
    }
  }

  async function transcribe(id: string) {
    setBusyId(id);
    try {
      await api.transcribeSession(id, diarize);
    } catch (e) {
      console.error("transcribe:", e);
    } finally {
      setBusyId(null);
    }
  }

  async function remove(id: string) {
    await api.deleteSession(id);
    setSessions((prev) => prev.filter((s) => s.id !== id));
    setTranscripts((prev) => {
      const { [id]: _, ...rest } = prev;
      return rest;
    });
    if (expanded === id) setExpanded(null);
  }

  async function toggleExpand(s: SessionMeta) {
    if (expanded === s.id) {
      setExpanded(null);
      return;
    }
    setExpanded(s.id);
    if (s.has_transcript && !transcripts[s.id]) {
      try {
        const t = await api.getSessionTranscript(s.id);
        setTranscripts((prev) => ({ ...prev, [s.id]: t }));
      } catch (e) {
        console.error("get transcript:", e);
      }
    }
  }

  return (
    <Card>
      <CardContent className="pt-5 space-y-3">
        <div>
          <div className="flex items-center gap-2 text-sm font-medium">
            <Radio className="h-4 w-4 text-[var(--color-muted-foreground)]" />
            Sessions
          </div>
          <div className="text-xs text-[var(--color-muted-foreground)] mt-0.5">
            Record a session, transcribe later, label speakers.
          </div>
        </div>

        {/* Record controls */}
        <div className="flex items-center gap-2">
          {recordingId ? (
            <Button
              variant="destructive"
              className="h-9"
              onClick={stop}
            >
              <Square className="h-4 w-4" /> Stop ({fmtTime(recordSeconds)})
            </Button>
          ) : (
            <Button className="h-9" onClick={start}>
              <Mic className="h-4 w-4" /> Start session
            </Button>
          )}
          <label className="ml-auto flex items-center gap-2 text-xs text-[var(--color-muted-foreground)]">
            <input
              type="checkbox"
              checked={diarize}
              onChange={(e) => setDiarize(e.target.checked)}
            />
            Diarize speakers
          </label>
        </div>

        {/* List */}
        <div className="space-y-2 pt-1">
          {sessions.length === 0 && (
            <div className="text-xs text-[var(--color-muted-foreground)] py-2">
              No recordings yet.
            </div>
          )}
          {sessions.map((s) => {
            const isOpen = expanded === s.id;
            const isTranscribing = transcribingId === s.id;
            const isBusy = busyId === s.id || isTranscribing;
            return (
              <div
                key={s.id}
                className="rounded-[calc(var(--radius)-0.25rem)] border border-[var(--color-border)] px-3 py-2"
              >
                <div className="flex items-center gap-2">
                  <button
                    className="text-[var(--color-muted-foreground)]"
                    onClick={() => toggleExpand(s)}
                    aria-label="Toggle transcript"
                  >
                    {isOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
                  </button>
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium truncate">{prettyDate(s.created_at)}</div>
                    <div className="text-[11px] text-[var(--color-muted-foreground)] flex items-center gap-2 flex-wrap">
                      <span>{fmtTime(Math.round(s.duration_secs))}</span>
                      {s.model_used && <span>· {s.model_used}</span>}
                      {s.speaker_count != null && (
                        <span className="inline-flex items-center gap-1">
                          <Users className="h-3 w-3" /> {s.speaker_count}
                        </span>
                      )}
                    </div>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={isBusy}
                    onClick={() => transcribe(s.id)}
                  >
                    {isTranscribing ? (
                      <><Loader2 className="h-3.5 w-3.5 animate-spin" /> {transcribingPct.toFixed(0)}%</>
                    ) : s.has_transcript ? (
                      "Re-transcribe"
                    ) : (
                      "Transcribe"
                    )}
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7"
                    onClick={() => remove(s.id)}
                    aria-label="Delete session"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </div>
                {isOpen && (
                  <TranscriptView
                    meta={s}
                    transcript={transcripts[s.id] ?? null}
                  />
                )}
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}

function TranscriptView({ meta, transcript }: { meta: SessionMeta; transcript: Transcript | null }) {
  if (!meta.has_transcript) {
    return (
      <div className="text-xs text-[var(--color-muted-foreground)] mt-2 pl-6">
        Not yet transcribed.
      </div>
    );
  }
  if (!transcript) {
    return (
      <div className="text-xs text-[var(--color-muted-foreground)] mt-2 pl-6">
        Loading…
      </div>
    );
  }
  return (
    <div className="mt-2 pl-6 space-y-2 text-sm">
      {transcript.segments.map((seg, i) => (
        <div key={i}>
          <div className="text-[10px] uppercase tracking-wider text-[var(--color-muted-foreground)]">
            {seg.speaker != null ? `Speaker ${seg.speaker + 1}` : "Transcript"}
            <span className="ml-2 normal-case tracking-normal">{fmtTime(Math.round(seg.start_secs))}</span>
          </div>
          <div>{seg.text || <span className="text-[var(--color-muted-foreground)]">(silence)</span>}</div>
        </div>
      ))}
    </div>
  );
}

function fmtTime(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function prettyDate(iso: string): string {
  // The backend writes 2026-04-25T13:22:09Z; show local-friendly.
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString();
}
