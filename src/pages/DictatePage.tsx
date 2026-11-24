import { useStore } from "@/App";
import { api } from "@/api";
import { VoiceField } from "@/components/VoiceField";
import { Keys, ProgressRing, useToast } from "@/components/bits";
import { MODELS, modelMeta, moodOf, relativeTime } from "@/lib/catalog";
import { cn } from "@/lib/utils";
import { ArrowRight, Copy, Mic, Square } from "lucide-react";

const HEADLINE: Record<string, [string, string]> = {
  idle: ["Say", "anything."],
  listening: ["I'm", "listening."],
  speaking: ["Go", "on…"],
  thinking: ["Writing", "it down."],
  recording: ["Recording", "a session."],
  error: ["Something", "went wrong."],
};

export function DictatePage() {
  const store = useStore();
  const { settings, status, isDownloaded, runAction } = store;
  const hasAnyModel = MODELS.some((m) => m.engine !== "diarizer" && isDownloaded(m.id));
  if (!hasAnyModel) return <Welcome />;

  const mood = moodOf(status);
  const on = mood === "listening" || mood === "speaking" || mood === "thinking";
  const sessionBusy = status.state === "recording_session" || status.state === "transcribing_session";
  const activeMissing = !isDownloaded(settings.model);
  const [a, b] = HEADLINE[mood];

  return (
    <div>
      <section className="grid grid-cols-[1fr_auto] items-center gap-6">
        <div className="rise">
          <div className="eyebrow mb-5">01 — Dictate</div>
          <h1 key={mood} className="display text-[76px]">
            <span className="word">{a}</span>{" "}
            <span className={cn("word italic", on && "text-ember")} style={{ animationDelay: ".08s" }}>{b}</span>
          </h1>
          <p className="mt-6 max-w-[34ch] text-[14px] leading-relaxed text-ink-2">
            Press <Keys accel={settings.hotkey} size="sm" className="mx-0.5 align-[-3px]" /> in any app, or tap the ring.
            Your words are typed wherever the cursor is — transcribed on this computer, never uploaded.
          </p>
          {activeMissing && (
            <button
              onClick={() => store.go("models")}
              className="mt-5 inline-flex items-center gap-2 rounded-full bg-ember-soft px-3.5 py-1.5 text-xs font-medium text-ember"
            >
              {modelMeta(settings.model).name} isn't downloaded — choose a model <ArrowRight className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        <div className="relative rise rise-2">
          <VoiceField mood={mood} size={340} />
          <button
            type="button"
            disabled={sessionBusy}
            onClick={() => runAction("Could not toggle dictation", () => api.toggleDictation())}
            aria-label={on ? "Stop dictation" : "Start dictation"}
            aria-pressed={on}
            className={cn(
              "absolute left-1/2 top-1/2 flex h-[104px] w-[104px] -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full",
              "transition-all duration-500 ease-[cubic-bezier(.2,.8,.2,1)] hover:scale-[1.04] active:scale-95 disabled:opacity-40",
              on ? "bg-ember text-ember-ink shadow-[0_0_0_10px_var(--ember-soft)]" : "bg-fg text-bg shadow-[var(--shadow-lift)]"
            )}
          >
            {on ? <Square className="h-6 w-6 fill-current" /> : <Mic className="h-7 w-7" strokeWidth={1.6} />}
          </button>
          <div className="absolute inset-x-0 -bottom-1 text-center eyebrow">
            {sessionBusy ? "Paused while a session records" : on ? "Tap to stop" : "Tap to start"}
          </div>
        </div>
      </section>

      <History />
    </div>
  );
}

function History() {
  const { history, clearHistory } = useStore();
  const toast = useToast();

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast("Copied to clipboard");
    } catch {
      toast("Couldn't reach the clipboard");
    }
  }

  return (
    <section className="mt-14 rise rise-3">
      <div className="flex items-baseline justify-between border-b border-border pb-3">
        <h2 className="eyebrow">Recent phrases</h2>
        {history.length > 0 && (
          <button onClick={clearHistory} className="text-[11px] text-muted-foreground hover:text-fg">
            Clear
          </button>
        )}
      </div>

      {history.length === 0 ? (
        <div className="py-10 text-center">
          <div className="display text-[28px] italic text-muted-foreground">Nothing yet.</div>
          <p className="mt-2 text-xs text-muted-foreground">
            Everything you dictate collects here, so a phrase pasted into the wrong window is one click away.
          </p>
        </div>
      ) : (
        <ol>
          {history.map((p, i) => (
            <li key={p.id} className="group border-b border-border">
              <button
                onClick={() => copy(p.text)}
                className="flex w-full items-start gap-6 py-4 text-left transition-colors hover:bg-muted/50 -mx-3 px-3 rounded-[10px]"
                title="Copy"
              >
                <span className="tabular w-[92px] shrink-0 pt-[3px] text-[10.5px] text-muted-foreground">
                  {relativeTime(p.at)}
                </span>
                <span className={cn("flex-1 text-[15px] leading-relaxed", i === 0 ? "text-fg" : "text-ink-2")}>
                  {i === 0
                    ? p.text.split(" ").map((w, j) => (
                        <span key={j} className="word" style={{ animationDelay: `${j * 0.035}s` }}>
                          {w}&nbsp;
                        </span>
                      ))
                    : p.text}
                </span>
                <Copy className="mt-1 h-3.5 w-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
              </button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

/// First run: no transcription model on disk yet.
function Welcome() {
  const { download, downloadModel, saveSettings, settings } = useStore();
  const rec = MODELS.find((m) => m.recommended)!;
  const busy = download?.id === rec.id;

  async function start() {
    if (await downloadModel(rec.id)) await saveSettings({ model: rec.id });
  }

  return (
    <div className="pt-6">
      <div className="eyebrow mb-6 rise">Welcome</div>
      <h1 className="display text-[88px] rise rise-1">
        Speak, and it's <span className="italic text-ember">typed.</span>
      </h1>
      <p className="mt-6 max-w-[48ch] text-[15px] leading-relaxed text-ink-2 rise rise-2">
        Tiny Whisper turns your voice into text in any app. The speech model runs entirely on this computer — so the
        first step is to download one.
      </p>

      <ol className="mt-14 grid grid-cols-3 gap-px overflow-hidden rounded-[18px] border border-border bg-border rise rise-3">
        <Step n="01" title="Get a model" active>
          <p className="mb-5 text-xs leading-relaxed text-muted-foreground">
            {rec.name} · {rec.sizeLabel}. {rec.blurb}
          </p>
          <button
            onClick={start}
            disabled={busy}
            className="inline-flex h-10 items-center gap-2.5 rounded-full bg-fg pl-4 pr-5 text-[13px] font-medium text-bg transition-transform hover:scale-[1.02] disabled:opacity-90"
          >
            {busy ? (
              <>
                <ProgressRing pct={download?.pct ?? null} />
                <span className="tabular">{download?.pct != null ? `${download.pct.toFixed(0)}%` : "Starting…"}</span>
              </>
            ) : (
              <>Download {rec.name} <ArrowRight className="h-4 w-4" /></>
            )}
          </button>
        </Step>
        <Step n="02" title="Press the shortcut">
          <p className="mb-5 text-xs leading-relaxed text-muted-foreground">Works in any app, even when this window is closed.</p>
          <Keys accel={settings.hotkey} size="lg" />
        </Step>
        <Step n="03" title="Just talk">
          <p className="text-xs leading-relaxed text-muted-foreground">
            Pause between thoughts. Each phrase is pasted where your cursor is. Press the shortcut again to stop.
          </p>
        </Step>
      </ol>
    </div>
  );
}

function Step({ n, title, active, children }: { n: string; title: string; active?: boolean; children: React.ReactNode }) {
  return (
    <li className={cn("bg-surface p-6", !active && "bg-surface/70")}>
      <div className={cn("tabular text-[11px]", active ? "text-ember" : "text-muted-foreground")}>{n}</div>
      <div className="display mt-6 mb-3 text-[26px]">{title}</div>
      {children}
    </li>
  );
}
