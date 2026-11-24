import { useStore } from "@/App";
import type { Engine } from "@/api";
import { ConfirmButton, Meter, PageHead, ProgressRing } from "@/components/bits";
import { ENGINE_LABEL, MODELS, modelMeta, type ModelMeta } from "@/lib/catalog";
import { cn } from "@/lib/utils";
import { ArrowDown, Check, Trash2 } from "lucide-react";

const ENGINES: Engine[] = ["whisper", "parakeet", "diarizer"];

export function ModelsPage() {
  const { settings, isDownloaded } = useStore();
  const active = modelMeta(settings.model);
  const ready = isDownloaded(active.id);
  const diskUsed = MODELS.filter((m) => isDownloaded(m.id)).length;

  return (
    <div>
      <PageHead
        index="03 — Models"
        title={<>The <span className="italic">ear.</span></>}
        lede="Bigger models hear more, but take longer. All of them run on this computer; downloads happen once."
        right={
          <div className="text-right">
            <div className="eyebrow">Installed</div>
            <div className="display text-[40px] tabular">{diskUsed}<span className="text-muted-foreground">/{MODELS.length}</span></div>
          </div>
        }
      />

      <section className="mt-10 overflow-hidden rounded-[20px] border border-border bg-surface p-7 shadow-[var(--shadow-lift)] rise rise-1">
        <div className="flex items-start justify-between gap-6">
          <div>
            <div className="eyebrow flex items-center gap-2">
              <span className={cn("h-1.5 w-1.5 rounded-full", ready ? "bg-moss" : "bg-ember live-dot")} />
              {ready ? "Dictating with" : "Selected, not downloaded"}
            </div>
            <div className="display mt-3 text-[56px]">{active.name}</div>
            <div className="mt-2 text-xs text-muted-foreground">{ENGINE_LABEL[active.engine]} · {active.sizeLabel} · {active.englishOnly ? "English" : "Multilingual"}</div>
          </div>
          <div className="space-y-2 pt-8">
            <Meter value={active.speed} label="Speed" />
            <Meter value={active.accuracy} label="Accuracy" />
          </div>
        </div>
        <p className="mt-5 max-w-[56ch] text-[13.5px] leading-relaxed text-ink-2">{active.blurb}</p>
      </section>

      {ENGINES.map((eng, gi) => (
        <section key={eng} className={cn("mt-12 rise", `rise-${gi + 2}`)}>
          <div className="mb-3 flex items-baseline justify-between border-b border-border pb-3">
            <h2 className="eyebrow">{ENGINE_LABEL[eng]}</h2>
            {eng === "diarizer" && <span className="text-[11px] text-muted-foreground">Used by Sessions, alongside your dictation model</span>}
          </div>
          <div>
            {MODELS.filter((m) => m.engine === eng).map((m) => <Row key={m.id} m={m} />)}
          </div>
        </section>
      ))}
    </div>
  );
}

function Row({ m }: { m: ModelMeta }) {
  const { settings, isDownloaded, download, downloadModel, deleteModel, saveSettings } = useStore();
  const have = isDownloaded(m.id);
  const inUse = settings.model === m.id;
  const mine = download?.id === m.id;
  const transcriber = m.engine !== "diarizer";

  return (
    <div className="group grid grid-cols-[1fr_auto_auto] items-center gap-8 border-b border-border py-5">
      <div className="min-w-0">
        <div className="flex items-center gap-3">
          <span className="display text-[26px]">{m.name}</span>
          {m.recommended && <span className="rounded-full bg-ember-soft px-2 py-0.5 text-[10px] font-semibold text-ember">Recommended</span>}
          {inUse && transcriber && <span className="rounded-full bg-fg px-2 py-0.5 text-[10px] font-semibold text-bg">In use</span>}
        </div>
        <p className="mt-1 max-w-[52ch] text-[12.5px] leading-relaxed text-muted-foreground">{m.blurb}</p>
      </div>

      <div className="space-y-1.5">
        <Meter value={m.speed} label="Speed" />
        <Meter value={m.accuracy} label="Accuracy" />
      </div>

      <div className="flex w-[176px] items-center justify-end gap-1">
        <span className="tabular mr-2 text-[11px] text-muted-foreground">{m.sizeLabel}</span>
        {have ? (
          <>
            {transcriber && !inUse && (
              <button
                onClick={() => saveSettings({ model: m.id })}
                className="h-8 rounded-full border border-border-strong px-3.5 text-xs font-medium hover:bg-fg hover:text-bg transition-colors"
              >
                Use
              </button>
            )}
            {(inUse || !transcriber) && (
              <span className="inline-flex h-8 items-center gap-1 px-2 text-xs text-moss"><Check className="h-3.5 w-3.5" /> Ready</span>
            )}
            {!inUse && (
              <ConfirmButton ariaLabel={`Delete ${m.name}`} confirmLabel="Delete?" onConfirm={() => deleteModel(m.id)}>
                <Trash2 className="h-3.5 w-3.5" />
              </ConfirmButton>
            )}
          </>
        ) : (
          <button
            disabled={!!download && !mine}
            onClick={() => downloadModel(m.id)}
            title={download && !mine ? "Another download is running" : undefined}
            className={cn(
              "inline-flex h-8 items-center gap-2 rounded-full px-3.5 text-xs font-medium transition-colors disabled:opacity-35",
              mine ? "bg-muted" : "bg-fg text-bg hover:opacity-90"
            )}
          >
            {mine ? (
              <><ProgressRing pct={download!.pct} size={14} /><span className="tabular">{download!.pct != null ? `${download!.pct.toFixed(0)}%` : "…"}</span></>
            ) : (
              <><ArrowDown className="h-3.5 w-3.5" /> Get</>
            )}
          </button>
        )}
      </div>
    </div>
  );
}
