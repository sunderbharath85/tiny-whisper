import { createRoot } from "react-dom/client";
import App from "./App";
import "./index.css";

// Surface any uncaught error visibly — otherwise a runtime crash shows as a blank
// white window with no console (release builds have devtools off by default).
function showFatal(msg: string) {
  const root = document.getElementById("root");
  if (!root) return;
  root.innerHTML = `<div style="padding:24px;font-family:system-ui;font-size:13px;color:#111">
    <div style="font-weight:600;margin-bottom:8px">Tiny Whisper crashed</div>
    <pre style="white-space:pre-wrap;color:#c00;font-size:12px">${msg.replace(/[<>&]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;" }[c]!))}</pre>
  </div>`;
}

window.addEventListener("error", (e) => {
  showFatal(`${e.message}\n${e.error?.stack ?? ""}\n${e.filename}:${e.lineno}:${e.colno}`);
});
window.addEventListener("unhandledrejection", (e) => {
  showFatal(`Unhandled promise rejection: ${String(e.reason?.stack ?? e.reason)}`);
});

try {
  createRoot(document.getElementById("root")!).render(<App />);
} catch (e) {
  showFatal(String((e as Error)?.stack ?? e));
}
