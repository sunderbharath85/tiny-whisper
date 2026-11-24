#!/usr/bin/env node
// Keep the app version identical everywhere it is written down.
//
//   node scripts/version.mjs            print the version, fail if files disagree
//   node scripts/version.mjs 1.2.0      set every file to 1.2.0
//   node scripts/version.mjs --tag v1.2.0   fail unless the files say 1.2.0 (used by CI)
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const p = (f) => join(root, f);
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

// Each entry reads and (optionally) rewrites one version location.
const files = [
  {
    file: "package.json",
    get: (s) => JSON.parse(s).version,
    set: (s, v) => s.replace(/("version":\s*")[^"]+(")/, `$1${v}$2`),
  },
  {
    file: "package-lock.json",
    get: (s) => JSON.parse(s).version,
    // Top-level version and the root package entry ("packages": { "": { ... } }).
    set: (s, v) => {
      const j = JSON.parse(s);
      j.version = v;
      if (j.packages?.[""]) j.packages[""].version = v;
      return JSON.stringify(j, null, 2) + "\n";
    },
  },
  {
    file: "src-tauri/tauri.conf.json",
    get: (s) => JSON.parse(s).version,
    set: (s, v) => s.replace(/("version":\s*")[^"]+(")/, `$1${v}$2`),
  },
  {
    file: "src-tauri/Cargo.toml",
    get: (s) => s.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1],
    set: (s, v) => s.replace(/(^\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m, `$1${v}$2`),
  },
  {
    file: "src-tauri/Cargo.lock",
    get: (s) => s.match(/name = "tiny-whisper"\nversion = "([^"]+)"/)?.[1],
    set: (s, v) => s.replace(/(name = "tiny-whisper"\nversion = ")[^"]+(")/, `$1${v}$2`),
  },
];

const read = () => files.map((f) => ({ ...f, text: readFileSync(p(f.file), "utf8") }));

const arg = process.argv[2];

if (arg && arg !== "--tag") {
  if (!SEMVER.test(arg)) {
    console.error(`"${arg}" is not a version like 1.2.0`);
    process.exit(1);
  }
  for (const f of read()) {
    writeFileSync(p(f.file), f.set(f.text, arg));
    console.log(`${f.file} → ${arg}`);
  }
  process.exit(0);
}

const found = read().map((f) => ({ file: f.file, version: f.get(f.text) }));
const versions = new Set(found.map((f) => f.version));
if (versions.size !== 1) {
  console.error("Version mismatch:");
  for (const f of found) console.error(`  ${f.file}: ${f.version}`);
  console.error("Fix with: node scripts/version.mjs <version>");
  process.exit(1);
}
const [version] = versions;

if (arg === "--tag") {
  const tag = (process.argv[3] ?? "").replace(/^refs\/tags\//, "");
  if (tag !== `v${version}`) {
    console.error(`Tag "${tag}" does not match the app version ${version} (expected v${version}).`);
    process.exit(1);
  }
}
console.log(version);
