#!/usr/bin/env node
/**
 * bump:repo — Update root project dependencies to match published versions.
 *
 * Usage:
 *   bun run bump:repo                # auto-detect from package.json files
 *   bun run bump:repo --engine=0.6.5 # override engine version
 *
 * This should be run AFTER engine has been published to npm.
 * It updates:
 *   1. package.json → dependencies["@s-page/engine"]
 *   2. bun.lock     → via `bun install`
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { execSync } from "node:child_process";

const ROOT = resolve(import.meta.dirname, "..");

// ── Parse args ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
const engineVersionArg = args.find((a) => a.startsWith("--engine="))?.split("=")[1];

// ── Helpers ─────────────────────────────────────────────────────────────────
function readJSON(relPath) {
  const abs = resolve(ROOT, relPath);
  return JSON.parse(readFileSync(abs, "utf-8"));
}

function writeJSON(relPath, data) {
  const abs = resolve(ROOT, relPath);
  writeFileSync(abs, JSON.stringify(data, null, 2) + "\n");
  console.log(`  ✔ ${relPath}`);
}

function run(cmd) {
  console.log(`  $ ${cmd}`);
  execSync(cmd, { cwd: ROOT, stdio: "inherit" });
}

// ── Detect versions ─────────────────────────────────────────────────────────
const engineVersion = engineVersionArg || readJSON("crates/spage-engine-napi/package.json").version;

if (!engineVersionArg) {
  console.log(`  ℹ Auto-detected @s-page/engine version: ${engineVersion}`);
}

// ── Update root package.json ────────────────────────────────────────────────
console.log(`\nUpdating root package.json...\n`);

const rootPkg = readJSON("package.json");
const oldVersion = rootPkg.dependencies?.["@s-page/engine"];
if (oldVersion === engineVersion) {
  console.log(`  ⏭ @s-page/engine already at ${engineVersion}, nothing to do.`);
  process.exit(0);
}

rootPkg.dependencies["@s-page/engine"] = engineVersion;
writeJSON("package.json", rootPkg);
console.log(`     ${oldVersion} → ${engineVersion}`);

// ── bun install ─────────────────────────────────────────────────────────────
console.log("\nUpdating bun.lock...");
run("bun install");

console.log(`\n✅ Done. Root project now uses @s-page/engine@${engineVersion}`);
