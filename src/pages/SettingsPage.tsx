import type { ReactNode } from "react";
import { useStore } from "@/App";
import { HotkeyCapture } from "@/components/HotkeyCapture";
import { PageHead, Segmented, Switch } from "@/components/bits";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { LANGUAGES, modelMeta } from "@/lib/catalog";
import { ShieldCheck } from "lucide-react";

export function SettingsPage() {
  const { settings, saveSettings, gpu, autostart, setAutostart } = useStore();
  const englishOnly = modelMeta(settings.model).englishOnly;
  const known = LANGUAGES.some((l) => l.code === settings.language);

  return (
    <div>
      <PageHead
        index="04 — Settings"
        title={<>Make it <span className="italic">yours.</span></>}
        lede="Changes save as you make them."
      />

      <Group title="Shortcuts" className="rise rise-1">
        <Row
          label="Dictation"
          labelId="hotkey-dictation-label"
          hint="Press once to start listening, again to stop."
        >
          <div className="w-[260px]">
            <HotkeyCapture id="hotkey-dictation" value={settings.hotkey} onChange={(v) => saveSettings({ hotkey: v })} />
          </div>
        </Row>
        <Row label="Record a session" labelId="hotkey-session-label" hint="Optional. Starts and stops a full recording.">
          <div className="w-[260px]">
            <HotkeyCapture id="hotkey-session" value={settings.session_hotkey} onChange={(v) => saveSettings({ session_hotkey: v })} clearable />
          </div>
        </Row>
      </Group>

      <Group title="Transcription" className="rise rise-2">
        <Row
          label="Language"
          hint={englishOnly ? `${modelMeta(settings.model).name} only understands English. Pick a multilingual model to change this.` : "The language you'll speak. Detecting is a little slower."}
        >
          <Select value={englishOnly ? "en" : settings.language || "auto"} onValueChange={(v) => saveSettings({ language: v })} disabled={englishOnly}>
            <SelectTrigger className="w-[220px] rounded-full"><SelectValue /></SelectTrigger>
            <SelectContent>
              {!known && !englishOnly && <SelectItem value={settings.language}>{settings.language}</SelectItem>}
              {LANGUAGES.map((l) => <SelectItem key={l.code} value={l.code}>{l.name}</SelectItem>)}
            </SelectContent>
          </Select>
        </Row>
        <Row
          label="Processor"
          hint={gpu ? "The GPU is much faster for larger models." : "This build runs on the CPU. GPU acceleration needs a build with GPU support."}
        >
          <Segmented
            label="Processor"
            value={gpu ? settings.device : "cpu"}
            onChange={(v) => saveSettings({ device: v })}
            options={[
              { value: "cpu", label: "CPU" },
              { value: "gpu", label: "GPU", disabled: !gpu, hint: gpu ? undefined : "Not available in this build" },
            ]}
          />
        </Row>
        <Row label="Microphone" hint="Tiny Whisper listens to your system's default input. Change it in your sound settings.">
          <span className="text-[13px] text-ink-2">System default</span>
        </Row>
      </Group>

      <Group title="System" className="rise rise-3">
        <Row label="Open at login" hint="Start quietly in the menu bar / tray when you sign in.">
          <Switch checked={autostart} onChange={setAutostart} label="Open at login" />
        </Row>
      </Group>

      <div className="mt-12 flex items-start gap-4 rounded-[18px] border border-border p-6 rise rise-4">
        <ShieldCheck className="mt-0.5 h-5 w-5 shrink-0 text-moss" strokeWidth={1.6} />
        <div>
          <div className="display text-[22px]">Private by design.</div>
          <p className="mt-1 max-w-[60ch] text-[12.5px] leading-relaxed text-muted-foreground">
            Audio is processed by models on this computer. Nothing is sent to a server — the only network use is downloading
            models you choose.
          </p>
        </div>
      </div>
    </div>
  );
}

function Group({ title, children, className }: { title: string; children: ReactNode; className?: string }) {
  return (
    <section className={`mt-10 ${className ?? ""}`}>
      <h2 className="eyebrow border-b border-border pb-3">{title}</h2>
      <div>{children}</div>
    </section>
  );
}

function Row({ label, labelId, hint, children }: { label: string; labelId?: string; hint?: string; children: ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-10 border-b border-border py-5">
      <div className="min-w-0">
        <div id={labelId} className="text-[14px] font-medium">{label}</div>
        {hint && <p className="mt-1 max-w-[44ch] text-[12px] leading-relaxed text-muted-foreground">{hint}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
