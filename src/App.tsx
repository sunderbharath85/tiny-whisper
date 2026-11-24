import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, type AppStatus, type ModelId, type ModelStatus, type Settings } from "./api";
import { ErrorArea, useErrorList } from "@/components/ErrorArea";
import { ToastProvider } from "@/components/bits";
import { errorMessage, setUnexpectedSink, type RunAction } from "@/lib/errors";
import { MODELS, STATUS_WORD, modelName, moodOf } from "@/lib/catalog";
import { cn } from "@/lib/utils";
import { DictatePage } from "@/pages/DictatePage";
import { SessionsPage } from "@/pages/SessionsPage";
import { ModelsPage } from "@/pages/ModelsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { AudioLines, Cpu, Radio, SlidersHorizontal } from "lucide-react";

export type Page = "dictate" | "sessions" | "models" | "settings";
export type Phrase = { id: number; text: string; at: Date };
export type Download = { id: ModelId; pct: number | null };

type Store = {
  settings: Settings;
  models: ModelStatus[];
  status: AppStatus;
  history: Phrase[];
  download: Download | null;
  gpu: boolean;
  autostart: boolean;
  /// Persist a settings change immediately. Rolls back and reports on failure.
  saveSettings: (patch: Partial<Settings>) => Promise<boolean>;
  setAutostart: (v: boolean) => Promise<void>;
  downloadModel: (id: ModelId) => Promise<boolean>;
  deleteModel: (id: ModelId) => Promise<void>;
  isDownloaded: (id: ModelId) => boolean;
  runAction: RunAction;
  report: (msg: string) => void;
  go: (p: Page) => void;
  clearHistory: () => void;
};

const StoreCtx = createContext<Store | null>(null);
export const useStore = () => useContext(StoreCtx)!;

const HISTORY_MAX = 50;

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [status, setStatus] = useState<AppStatus>({ state: "idle" });
  const [download, setDownload] = useState<Download | null>(null);
  const [history, setHistory] = useState<Phrase[]>([]);
  const [gpu, setGpu] = useState(false);
  const [autostart, setAutostartState] = useState(false);
  const [bootError, setBootError] = useState<string | null>(null);
  const [page, setPage] = useState<Page>("dictate");
  const { errors, report, dismiss, clear: clearErrors } = useErrorList();
  const settingsRef = useRef<Settings | null>(null);
  settingsRef.current = settings;
  const phraseId = useRef(1);

  useEffect(() => {
    async function showStartupError() {
      try {
        const err = await api.takeStartupError();
        if (err) report(err);
      } catch (e) {
        const msg = errorMessage(e);
        if (/not found/i.test(msg)) console.warn("take_startup_error unavailable:", msg);
        else report(`Could not read startup errors: ${msg}`);
      }
    }

    (async () => {
      // In Tauri 2, the webview can load before setup() calls app.manage(),
      // so the first get_settings invocation races with state registration
      // and fails with "state not managed". Retry briefly before surfacing it.
      const started = Date.now();
      while (true) {
        try {
          setSettings(await api.getSettings());
          setModels(await api.listModels());
          try { setAutostartState(await api.getAutostart()); } catch { /* best-effort */ }
          try { setGpu(await api.gpuAvailable()); } catch { /* older backend */ }
          await showStartupError();
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
    const offUnexpected = setUnexpectedSink(report);
    const listenFailed = (what: string) => (e: unknown): undefined => {
      report(`Could not listen for ${what}: ${errorMessage(e)}`);
      return undefined;
    };
    const offStatus = api
      .onStatus((s) => {
        setStatus(s);
        if (s.state === "error") report(s.message || "Something went wrong (no details given).");
      })
      .catch(listenFailed("status updates"));
    const offProgress = api
      .onDownloadProgress((p) => {
        setDownload((d) =>
          d && d.id === p.model ? { id: p.model, pct: p.total_bytes > 0 ? (p.downloaded_bytes / p.total_bytes) * 100 : null } : d
        );
      })
      .catch(listenFailed("download progress"));
    const offText = api
      .onDictationText((text) => {
        setHistory((h) => [{ id: phraseId.current++, text, at: new Date() }, ...h].slice(0, HISTORY_MAX));
      })
      .catch(listenFailed("transcribed text"));
    return () => {
      offUnexpected();
      offStatus.then((f) => f?.());
      offProgress.then((f) => f?.());
      offText.then((f) => f?.());
    };
  }, []);

  const runAction: RunAction = useCallback(
    async (failure, fn) => {
      clearErrors();
      try {
        await fn();
        return true;
      } catch (e) {
        console.error(`${failure}:`, e);
        report(`${failure}: ${errorMessage(e)}`);
        return false;
      }
    },
    [clearErrors, report]
  );

  const refreshModels = useCallback(async () => {
    try {
      setModels(await api.listModels());
    } catch (e) {
      report(`Could not refresh the model list: ${errorMessage(e)}`);
    }
  }, [report]);

  const store = useMemo<Store | null>(() => {
    if (!settings) return null;
    const isDownloaded = (id: ModelId) => models.find((m) => m.id === id)?.downloaded ?? false;
    return {
      settings,
      models,
      status,
      history,
      download,
      gpu,
      autostart,
      isDownloaded,
      runAction,
      report,
      go: setPage,
      clearHistory: () => setHistory([]),
      async saveSettings(patch) {
        const prev = settingsRef.current!;
        const next = { ...prev, ...patch };
        setSettings(next);
        // The backend rejects with a message naming the bad field (K2).
        const ok = await runAction("Could not save settings", () => api.saveSettings(next));
        if (!ok) setSettings(prev);
        return ok;
      },
      async setAutostart(v) {
        setAutostartState(v);
        const ok = await runAction(`Could not turn ${v ? "on" : "off"} launch at login`, () => api.setAutostart(v));
        if (!ok) setAutostartState(!v);
      },
      async downloadModel(id) {
        setDownload({ id, pct: 0 });
        const ok = await runAction(`Could not download ${modelName(id)}`, () => api.downloadModel(id));
        setDownload(null);
        await refreshModels();
        return ok;
      },
      async deleteModel(id) {
        await runAction(`Could not delete ${modelName(id)}`, () => api.deleteModel(id));
        await refreshModels();
      },
    };
  }, [settings, models, status, history, download, gpu, autostart, runAction, report, refreshModels]);

  if (bootError) {
    return (
      <div className="grain flex h-screen flex-col justify-center px-12">
        <div className="eyebrow mb-4">Startup failed</div>
        <h1 className="display text-5xl">Tiny Whisper couldn't start.</h1>
        <pre className="mt-6 max-w-2xl whitespace-pre-wrap font-mono text-xs text-destructive">{bootError}</pre>
      </div>
    );
  }
  if (!store) {
    return (
      <div className="grain flex h-screen items-center justify-center">
        <span className="display text-3xl italic shimmer-text">Tiny Whisper</span>
      </div>
    );
  }

  return (
    <StoreCtx.Provider value={store}>
      <ToastProvider>
        <div className="grain flex h-screen overflow-hidden">
          <Sidebar page={page} onPage={setPage} />
          <main className="relative flex-1 overflow-y-auto">
            <div className="sticky top-0 z-20 px-10 pt-4 empty:hidden">
              <ErrorArea errors={errors} onDismiss={dismiss} />
            </div>
            <div key={page} className="mx-auto max-w-[880px] px-10 pb-16 pt-10">
              {page === "dictate" && <DictatePage />}
              {page === "sessions" && <SessionsPage />}
              {page === "models" && <ModelsPage />}
              {page === "settings" && <SettingsPage />}
            </div>
          </main>
        </div>
      </ToastProvider>
    </StoreCtx.Provider>
  );
}

const NAV: { id: Page; label: string; icon: ReactNode }[] = [
  { id: "dictate", label: "Dictate", icon: <AudioLines className="h-[15px] w-[15px]" /> },
  { id: "sessions", label: "Sessions", icon: <Radio className="h-[15px] w-[15px]" /> },
  { id: "models", label: "Models", icon: <Cpu className="h-[15px] w-[15px]" /> },
  { id: "settings", label: "Settings", icon: <SlidersHorizontal className="h-[15px] w-[15px]" /> },
];

function Sidebar({ page, onPage }: { page: Page; onPage: (p: Page) => void }) {
  const { status, settings, isDownloaded } = useStore();
  const mood = moodOf(status);
  const live = mood !== "idle" && mood !== "error";
  const missing = MODELS.filter((m) => m.engine !== "diarizer").every((m) => !isDownloaded(m.id));

  return (
    <aside className="flex w-[216px] shrink-0 flex-col border-r border-border bg-muted/50 px-4 pb-4 pt-6">
      <div className="px-2">
        <div className="display text-[26px] italic leading-none">Tiny Whisper</div>
        <div className="eyebrow mt-2 !text-[9.5px]">On-device · v0.2</div>
      </div>

      <nav className="mt-10 space-y-0.5" aria-label="Sections">
        {NAV.map((n, i) => {
          const active = page === n.id;
          const badge =
            (n.id === "models" && missing) ||
            (n.id === "sessions" && status.state === "recording_session");
          return (
            <button
              key={n.id}
              type="button"
              aria-current={active ? "page" : undefined}
              onClick={() => onPage(n.id)}
              className={cn(
                "group flex w-full items-center gap-3 rounded-[10px] px-2.5 py-2 text-left text-[13px] transition-all",
                active ? "bg-surface text-fg shadow-[var(--shadow-lift)]" : "text-ink-2 hover:bg-surface/60 hover:text-fg"
              )}
            >
              <span className={cn("transition-colors", active ? "text-fg" : "text-muted-foreground group-hover:text-fg")}>{n.icon}</span>
              <span className="flex-1 font-medium">{n.label}</span>
              {badge ? (
                <span className="h-1.5 w-1.5 rounded-full bg-ember live-dot" />
              ) : (
                <span className="tabular text-[10px] text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100">0{i + 1}</span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="mt-auto rounded-[12px] border border-border bg-surface/70 p-3" role="status" aria-live="polite">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "h-2 w-2 rounded-full",
              live ? "bg-ember live-dot" : mood === "error" ? "bg-destructive" : missing ? "bg-border-strong" : "bg-moss"
            )}
          />
          <span className="text-[12px] font-medium">{missing ? "No model yet" : STATUS_WORD[status.state]}</span>
        </div>
        <div className="mt-1.5 truncate text-[11px] text-muted-foreground">
          {modelName(settings.model)} · {settings.device === "gpu" ? "GPU" : "CPU"}
        </div>
      </div>
    </aside>
  );
}
