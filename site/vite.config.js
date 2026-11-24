import { defineConfig } from "vite";
import { resolve } from "path";

// The product website, published to GitHub Pages by .github/workflows/pages.yml.
// SITE_BASE is the Pages path ("/<repo>/"); "/" for a custom domain.
export default defineConfig({
  root: __dirname,
  base: process.env.SITE_BASE ?? "/tiny-whisper/",
  build: {
    outDir: resolve(__dirname, "../dist-site"),
    emptyOutDir: true,
    chunkSizeWarningLimit: 900,
  },
  server: { port: 4173 },
});
