#!/usr/bin/env node
// Build the Windows GPU (Vulkan) installer locally:
//   npm run tauri:build:vulkan [-- extra tauri build args]
//
// - Bundles the Vulkan loader (vulkan-1.dll) next to the exe, so the app
//   starts on PCs without a GPU driver and falls back to the CPU there.
//   Taken from src-tauri/vendor/ if present, else from System32 (installed by
//   GPU drivers and the Vulkan SDK/runtime).
// - Puts cargo's intermediate files in a short folder. whisper.cpp's Vulkan
//   shader generator nests deep enough under src-tauri\target that MSBuild
//   hits Windows' 248-character folder limit (error FTK1011).
// Needs the Vulkan SDK (VULKAN_SDK, glslc) to compile whisper.cpp's shaders.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, parse } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const fail = (msg) => {
  console.error(`build-vulkan: ${msg}`);
  process.exit(1);
};

if (process.platform !== "win32") fail("Vulkan builds are for Windows; on macOS use --features metal.");
if (!process.env.VULKAN_SDK) fail("VULKAN_SDK is not set. Install the Vulkan SDK from https://vulkan.lunarg.com/.");

const vendored = join(root, "src-tauri", "vendor", "vulkan-1.dll");
if (!existsSync(vendored)) {
  const system = join(process.env.SystemRoot ?? "C:\\Windows", "System32", "vulkan-1.dll");
  if (!existsSync(system)) fail(`no Vulkan loader at ${system}; put vulkan-1.dll in src-tauri/vendor/.`);
  mkdirSync(dirname(vendored), { recursive: true });
  copyFileSync(system, vendored);
  console.log(`build-vulkan: bundling ${system}`);
}

const env = { ...process.env };
env.CARGO_BUILD_BUILD_DIR ??= join(parse(root).root, "tw-build");
console.log(`build-vulkan: CARGO_BUILD_BUILD_DIR=${env.CARGO_BUILD_BUILD_DIR}`);

// Tauri checks whether bundled DLLs are already signed with a signtool it
// finds through the registry's Windows Kits root, which can point at a folder
// without one ("SignTool not found"). Hand it the signtool on PATH instead.
if (!env.TAURI_WINDOWS_SIGNTOOL_PATH) {
  const where = spawnSync("where", ["signtool"], { encoding: "utf8" });
  const found = where.status === 0 ? where.stdout.split(/\r?\n/)[0].trim() : "";
  if (found) env.TAURI_WINDOWS_SIGNTOOL_PATH = found;
}

const args = ["tauri", "build", "--features", "vulkan", "--config", "src-tauri/tauri.vulkan.conf.json", ...process.argv.slice(2)];
const run = spawnSync("npx", args, { cwd: root, env, stdio: "inherit", shell: true });
process.exit(run.status ?? 1);
