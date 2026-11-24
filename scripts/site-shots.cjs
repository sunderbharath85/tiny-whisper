// Capture the app's screens (dark theme, mock backend) for the website.
// Needs Playwright with Chromium (`npm i -D playwright && npx playwright install chromium`).
// Usage: npm run site:shots   → writes site/public/shots/*.jpg
const { spawn } = require("node:child_process");
const path = require("node:path");
const { chromium } = require("playwright");

const OUT = path.join(__dirname, "..", "site", "public", "shots");
const URL = "http://localhost:1420/";

(async () => {
  const vite = spawn("npx", ["vite", "--port", "1420", "--strictPort"], { cwd: path.join(__dirname, ".."), stdio: "ignore" });
  try {
    await new Promise((r) => setTimeout(r, 2500));
    const browser = await chromium.launch();
    const page = await browser.newPage({ viewport: { width: 1040, height: 720 }, deviceScaleFactor: 2, colorScheme: "dark" });
    const shot = (name) => page.screenshot({ path: path.join(OUT, `${name}.jpg`), type: "jpeg", quality: 86 });

    await page.goto(`${URL}?fresh`);
    await page.waitForTimeout(1500);
    await shot("welcome");

    await page.goto(URL);
    await page.waitForTimeout(900);
    await page.getByRole("button", { name: "Start dictation" }).click();
    await page.waitForTimeout(13_800); // two phrases have arrived; settled in "speaking" again
    await shot("dictate");
    await page.getByRole("button", { name: "Stop dictation" }).click();

    for (const name of ["Sessions", "Models", "Settings"]) {
      await page.getByRole("navigation").getByRole("button", { name: new RegExp(name) }).click();
      await page.waitForTimeout(1400);
      await shot(name.toLowerCase());
    }
    await browser.close();
  } finally {
    vite.kill();
  }
})();
