#!/usr/bin/env node
/**
 * bump:core — Bump @s-page/core version.
 *
 * Usage:
 *   bun run bump:core <version>
 *
 * Files modified (1 place per RELEASE.md):
 *   1. packages/core/package.json → version
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");

// ── Parse args ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
const version = args.find((a) => !a.startsWith("-"));

if (!version) {
  console.error("Usage: bun run bump:core <version>");
  console.error("  e.g. bun run bump:core 0.6.1");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+/.test(version)) {
  console.error(`Error: "${version}" doesn't look like a valid semver version`);
  process.exit(1);
}

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

// ── 1. Core package.json ────────────────────────────────────────────────────
console.log(`\nBumping core to ${version}...\n`);

const corePkgPath = "packages/core/package.json";
const corePkg = readJSON(corePkgPath);
corePkg.version = version;
writeJSON(corePkgPath, corePkg);

console.log(`\n✅ Done. Core publish is triggered via publish-core.yml (see RELEASE.md).`);
