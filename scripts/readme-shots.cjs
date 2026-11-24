// Screenshots for README.md: every app screen in light and dark, the floating
// indicator in each state, and the website hero. Uses the mock backend.
// Needs Playwright with Chromium. Usage: npm run readme:shots
const { spawn } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");
const { chromium } = require("playwright");

// Run Vite with node directly (not via npx) so kill() stops the server itself.
const VITE = path.join(__dirname, "..", "node_modules", "vite", "bin", "vite.js");

const ROOT = path.join(__dirname, "..");
const OUT = path.join(ROOT, "docs", "screenshots");
const APP = "http://localhost:1420/";
const SITE = "http://localhost:4173/tiny-whisper/";
fs.mkdirSync(OUT, { recursive: true });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  const app = spawn(process.execPath, [VITE, "--port", "1420", "--strictPort"], { cwd: ROOT, stdio: "ignore" });
  // The site is shot from a production build; the dev server loads too slowly.
  require("node:child_process").execSync("npx vite build --config site/vite.config.js", { cwd: ROOT, stdio: "ignore" });
  const site = spawn(process.execPath, [VITE, "preview", "--config", "site/vite.config.js", "--port", "4173", "--strictPort"], { cwd: ROOT, stdio: "ignore" });
  const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
  try {
    await wait(3000);
    for (const scheme of ["dark", "light"]) {
      const page = await browser.newPage({ viewport: { width: 1040, height: 720 }, deviceScaleFactor: 2, colorScheme: scheme });
      const shot = (name) => page.screenshot({ path: path.join(OUT, `${name}-${scheme}.jpg`), type: "jpeg", quality: 84 });
      await page.goto(`${APP}?fresh`);
      await wait(1500);
      await shot("welcome");
      await page.goto(APP);
      await wait(900);
      await page.getByRole("button", { name: "Start dictation" }).click();
      await wait(13_800); // two phrases in, settled on "Hearing you"
      await shot("dictate");
      await page.getByRole("button", { name: "Stop dictation" }).click();
      for (const name of ["Sessions", "Models", "Settings"]) {
        await page.getByRole("navigation").getByRole("button", { name: new RegExp(name) }).click();
        await wait(1400);
        await shot(name.toLowerCase());
      }
      await page.close();
    }

    const ind = await browser.newPage({ viewport: { width: 240, height: 64 }, deviceScaleFactor: 2, colorScheme: "dark" });
    await ind.goto(`${APP}indicator.html`);
    await wait(600);
    const states = {
      listening: { state: "listening" },
      speaking: { state: "speaking" },
      writing: { state: "transcribing" },
      recording: { state: "recording_session" },
      error: { state: "error", message: "Model \"small.en\" is not downloaded" },
    };
    for (const [name, status] of Object.entries(states)) {
      await ind.evaluate((s) => window.__twMock.emit("app://status", s), status);
      await wait(700);
      await ind.screenshot({ path: path.join(OUT, `indicator-${name}.png`), omitBackground: true });
    }

    const web = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 });
    await web.goto(SITE);
    await wait(4500);
    await web.addStyleTag({ content: ".hero__scroll{display:none!important}" });
    await web.screenshot({ path: path.join(OUT, "website.jpg"), type: "jpeg", quality: 86 });
  } finally {
    await browser.close();
    app.kill();
    site.kill();
  }
})();
