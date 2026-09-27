import { createRoot } from "react-dom/client";
import App from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { errorMessage, reportUnexpected } from "./lib/errors";
import "./index.css";

// Last-resort view when React itself can't mount; render errors after that
// are shown by ErrorBoundary.
function showFatal(msg: string) {
  const root = document.getElementById("root");
  if (!root) return;
  root.innerHTML = `<div style="padding:24px;font-family:system-ui;font-size:13px;color:#111">
    <div style="font-weight:600;margin-bottom:8px">Tiny Whisper crashed</div>
    <pre style="white-space:pre-wrap;color:#c00;font-size:12px">${msg.replace(/[<>&]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;" }[c]!))}</pre>
  </div>`;
}

// Errors outside rendering (event handlers, timers, rejected promises) are
// logged and shown in the error area; they no longer replace the window.
// Release builds have no visible console, so the error area is what users see.
window.addEventListener("error", (e) => {
  console.error("Uncaught error:", e.error ?? e.message);
  reportUnexpected(`Unexpected error: ${e.message || errorMessage(e.error)}`);
});
window.addEventListener("unhandledrejection", (e) => {
  console.error("Unhandled promise rejection:", e.reason);
  reportUnexpected(`Unexpected error: ${errorMessage(e.reason)}`);
});

try {
  createRoot(document.getElementById("root")!).render(
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  );
} catch (e) {
  showFatal(String((e as Error)?.stack ?? e));
}
