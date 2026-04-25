import { useEffect, useState, type ReactNode } from "react";
import { api, type Settings, type ModelStatus, type ModelId, type AppStatus } from "./api";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Check,
  Download,
  Globe,
  Keyboard,
  Loader2,
  Mic,
  Save,
  Speaker,
  Trash2,
} from "lucide-react";

type ModelMeta = { id: ModelId; label: string; sizeLabel: string };

const MODELS: ModelMeta[] = [
  { id: "tiny.en", label: "Tiny (English)", sizeLabel: "~75 MB" },
  { id: "base.en", label: "Base (English)", sizeLabel: "~142 MB" },
  { id: "small.en", label: "Small (English)", sizeLabel: "~466 MB" },
  { id: "medium.en", label: "Medium (English)", sizeLabel: "~1.5 GB" },
  { id: "large-v3", label: "Large v3", sizeLabel: "~2.9 GB" },
];

const STATUS_LABEL: Record<AppStatus["state"], string> = {
  idle: "Model Ready",
  listening: "Listening",
  speaking: "Hearing you",
  transcribing: "Transcribing…",
  error: "Error",
};

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [status, setStatus] = useState<AppStatus>({ state: "idle" });
  const [downloading, setDownloading] = useState<ModelId | null>(null);
  const [dlPct, setDlPct] = useState<number | null>(null);
  const [saved, setSaved] = useState(false);
  const [lastTranscription] = useState<string>("");
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [bootError, setBootError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      // In Tauri 2, the webview can load before setup() calls app.manage(),
      // so the first get_settings invocation races with state registration
      // and fails with "state not managed". Retry briefly before surfacing it.
      const started = Date.now();
      while (true) {
        try {
          setSettings(await api.getSettings());
          setModels(await api.listModels());
          try {
            setLaunchAtLogin(await api.getAutostart());
          } catch { /* plugin may not be registered yet; best-effort */ }
          break;
        } catch (e) {
          const msg = String(e);
          if (msg.includes("state not managed") && Date.now() - started < 5000) {
            await new Promise((r) => setTimeout(r, 100));
            continue;
          }
          setBootError(msg);
          break;
        }
      }
    })();
    const offStatus = api.onStatus(setStatus);
    const offProgress = api.onDownloadProgress((p) => {
      setDlPct(p.total_bytes > 0 ? (p.downloaded_bytes / p.total_bytes) * 100 : null);
    });
    return () => {
      offStatus.then((f) => f());
      offProgress.then((f) => f());
    };
  }, []);

  if (bootError) {
    return (
      <div className="p-6 text-sm">
        <div className="font-semibold mb-2">Tiny Whisper failed to load</div>
        <pre className="whitespace-pre-wrap text-xs text-[var(--color-destructive)]">{bootError}</pre>
      </div>
    );
  }
  if (!settings) {
    return (
      <div className="p-6 text-sm text-[var(--color-muted-foreground)]">Loading…</div>
    );
  }

  const byId = new Map(models.map((m) => [m.id, m]));
  const isActive = status.state === "listening" || status.state === "speaking" || status.state === "transcribing";
  const isTranscribing = status.state === "transcribing";
  const isSpeaking = status.state === "speaking";

  async function save() {
    await api.saveSettings(settings!);
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  async function download(id: ModelId) {
    setDownloading(id);
    setDlPct(0);
    try {
      await api.downloadModel(id);
      setModels(await api.listModels());
    } finally {
      setDownloading(null);
      setDlPct(null);
    }
  }

  async function remove(id: ModelId) {
    await api.deleteModel(id);
    setModels(await api.listModels());
  }

  const update = <K extends keyof Settings>(k: K, v: Settings[K]) =>
    setSettings({ ...settings!, [k]: v });

  const statusPillClass =
    status.state === "error"
      ? "bg-[var(--color-destructive)] text-white"
      : status.state === "transcribing"
      ? "bg-sky-600 text-white"
      : status.state === "speaking"
      ? "bg-amber-500 text-white"
      : status.state === "listening"
      ? "bg-emerald-600 text-white"
      : "bg-[var(--color-accent)] text-[var(--color-accent-fg)]";

  return (
    <div className="h-screen overflow-y-auto">
      <div className="max-w-[560px] mx-auto px-5 py-6 space-y-4">
        {/* Header */}
        <div className="flex items-center gap-3">
          <div className="h-10 w-10 rounded-full bg-[var(--color-accent)] text-[var(--color-accent-fg)] flex items-center justify-center">
            <Mic className="h-5 w-5" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-[15px] font-semibold leading-tight">Tiny Whisper</div>
            <div className="text-xs text-[var(--color-muted-foreground)]">Local AI transcription</div>
          </div>
          <span className={`text-[11px] font-medium px-3 py-1 rounded-full ${statusPillClass}`}>
            {STATUS_LABEL[status.state]}
          </span>
        </div>

        {/* Start/Stop — also reflects live status so the user sees transcription in progress */}
        <Button
          className={`w-full h-11 text-sm ${
            isTranscribing ? "bg-sky-600 hover:bg-sky-600/90" : isSpeaking ? "bg-amber-500 hover:bg-amber-500/90" : ""
          }`}
          onClick={() => { /* hotkey-driven for now */ }}
        >
          {isTranscribing ? (
            <><Loader2 className="h-4 w-4 animate-spin" /> Transcribing…</>
          ) : isSpeaking ? (
            <><Mic className="h-4 w-4" /> Hearing you…</>
          ) : isActive ? (
            <><Mic className="h-4 w-4" /> Listening — press hotkey to stop</>
          ) : (
            <><Mic className="h-4 w-4" /> Start Listening</>
          )}
        </Button>

        {/* Microphone */}
        <Card>
          <CardContent className="pt-5 space-y-2">
            <SectionHeader icon={<Speaker className="h-4 w-4" />} title="Microphone" subtitle="Select input device" />
            <Select value="default" onValueChange={() => {}}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="default">System default</SelectItem>
              </SelectContent>
            </Select>
          </CardContent>
        </Card>

        {/* Models */}
        <Card>
          <CardContent className="pt-5 space-y-3">
            <SectionHeader icon={<Download className="h-4 w-4" />} title="Whisper Models" subtitle="Download and select a model for local transcription" />
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <div className="text-xs text-[var(--color-muted-foreground)]">Active Model</div>
                <Select value={settings.model} onValueChange={(v) => update("model", v as ModelId)}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {MODELS.map((m) => (
                      <SelectItem key={m.id} value={m.id}>{m.label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-2">
                <div className="text-xs text-[var(--color-muted-foreground)]">Device</div>
                <Select value={settings.device} onValueChange={(v) => update("device", v as "cpu" | "gpu")}>
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="cpu">CPU</SelectItem>
                    <SelectItem value="gpu">GPU (Vulkan)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="space-y-2 pt-1">
              {MODELS.map((m) => {
                const dl = byId.get(m.id)?.downloaded ?? false;
                const isDownloading = downloading === m.id;
                return (
                  <div
                    key={m.id}
                    className="flex items-center justify-between gap-3 rounded-[calc(var(--radius)-0.25rem)] border border-[var(--color-border)] px-3 py-2"
                  >
                    <div className="flex items-baseline gap-2 min-w-0">
                      <span className="text-sm font-medium truncate">{m.label}</span>
                      <span className="text-xs text-[var(--color-muted-foreground)]">{m.sizeLabel}</span>
                    </div>
                    {dl ? (
                      <div className="flex items-center gap-1">
                        <span className="inline-flex items-center gap-1 text-xs text-[var(--color-success)] px-2 py-1">
                          <Check className="h-3.5 w-3.5" /> Ready
                        </span>
                        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={() => remove(m.id)} aria-label="Delete model">
                          <Trash2 className="h-3.5 w-3.5" />
                        </Button>
                      </div>
                    ) : (
                      <Button variant="outline" size="sm" disabled={isDownloading} onClick={() => download(m.id)}>
                        {isDownloading ? (
                          <><Loader2 className="h-3.5 w-3.5 animate-spin" />{dlPct != null ? ` ${dlPct.toFixed(0)}%` : "…"}</>
                        ) : (
                          <><Download className="h-3.5 w-3.5" /> Get</>
                        )}
                      </Button>
                    )}
                  </div>
                );
              })}
            </div>
          </CardContent>
        </Card>

        {/* Language */}
        <Card>
          <CardContent className="pt-5 space-y-2">
            <SectionHeader icon={<Globe className="h-4 w-4" />} title="Language" subtitle="Source language for transcription" />
            <Input
              value={settings.language}
              onChange={(e) => update("language", e.target.value)}
              placeholder="auto"
            />
          </CardContent>
        </Card>

        {/* Shortcut */}
        <Card>
          <CardContent className="pt-5 space-y-2">
            <SectionHeader icon={<Keyboard className="h-4 w-4" />} title="Shortcut" subtitle="Global keyboard shortcut" />
            <Input
              value={settings.hotkey}
              onChange={(e) => update("hotkey", e.target.value)}
              placeholder="CommandOrControl+Shift+Space"
            />
          </CardContent>
        </Card>

        {/* Launch at login */}
        <Card>
          <CardContent className="py-4 flex items-center gap-3">
            <div className="flex-1">
              <div className="text-sm font-medium">Launch at login</div>
              <div className="text-xs text-[var(--color-muted-foreground)]">Start Tiny Whisper when you log in</div>
            </div>
            <Toggle
              checked={launchAtLogin}
              onChange={async (v) => {
                setLaunchAtLogin(v);
                try {
                  await api.setAutostart(v);
                } catch (e) {
                  setLaunchAtLogin(!v);
                  console.error("setAutostart failed:", e);
                }
              }}
            />
          </CardContent>
        </Card>

        {/* Save */}
        <Button className="w-full h-11 text-sm" onClick={save}>
          {saved ? <><Check className="h-4 w-4" /> Saved</> : <><Save className="h-4 w-4" /> Save Settings</>}
        </Button>

        {/* Last transcription */}
        <Card>
          <CardContent className="py-4">
            <div className="text-[10px] font-medium tracking-wider text-[var(--color-muted-foreground)] uppercase">
              Last Transcription
            </div>
            <div className="text-sm mt-1 min-h-[1.25rem]">
              {lastTranscription || <span className="text-[var(--color-muted-foreground)]">—</span>}
            </div>
          </CardContent>
        </Card>

        {status.state === "error" && status.message && (
          <div className="text-xs text-[var(--color-destructive)]">{status.message}</div>
        )}
      </div>
    </div>
  );
}

function SectionHeader({ icon, title, subtitle }: { icon: ReactNode; title: string; subtitle: string }) {
  return (
    <div>
      <div className="flex items-center gap-2 text-sm font-medium">
        <span className="text-[var(--color-muted-foreground)]">{icon}</span>
        {title}
      </div>
      <div className="text-xs text-[var(--color-muted-foreground)] mt-0.5">{subtitle}</div>
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
        checked ? "bg-[var(--color-accent)]" : "bg-[var(--color-border)]"
      }`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${
          checked ? "translate-x-4" : "translate-x-0.5"
        }`}
      />
    </button>
  );
}
