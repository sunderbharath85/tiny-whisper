import { defineConfig } from "vite";
import { resolve } from "path";
import { readFileSync } from "fs";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;
const pkg = JSON.parse(readFileSync(resolve(__dirname, "package.json"), "utf8")) as { version: string };

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Shown in the sidebar; package.json is kept in sync by scripts/version.mjs.
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  clearScreen: false,
  resolve: {
    alias: { "@": resolve(__dirname, "src") },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    rollupOptions: {
      input: {
        settings: resolve(__dirname, "index.html"),
        indicator: resolve(__dirname, "indicator.html"),
      },
    },
  },
});
