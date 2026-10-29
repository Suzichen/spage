#!/usr/bin/env node
/**
 * bump:core — Bump @s-page/core version.
 *
 * Usage:
 *   bun run bump:core <version>
 *
 * Crossing a release line (0.6 → 0.7) starts here: core is warned, not blocked, because it
 * has to be published before the matching engine. See scripts/version-utils.js.
 *
 * Files modified (1 place per RELEASE.md):
 *   1. packages/core/package.json → version
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { assertKnownFlags, assertSemver, warnIncompatible } from "./version-utils.js";

const ROOT = resolve(import.meta.dirname, "..");

// ── Parse args ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
const version = args.find((a) => !a.startsWith("-"));

if (!version) {
  console.error("Usage: bun run bump:core <version>");
  console.error("  e.g. bun run bump:core 0.6.1");
  process.exit(1);
}

assertKnownFlags(args, []);
assertSemver(version);

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

// core is allowed to move to a new release line first — it must be published first anyway.
warnIncompatible(
  version,
  readJSON("crates/spage-engine-napi/package.json").version,
  `Run \`bun run bump:engine ${version}\` in the same commit, and publish core before engine.`
);

const corePkgPath = "packages/core/package.json";
const corePkg = readJSON(corePkgPath);
corePkg.version = version;
writeJSON(corePkgPath, corePkg);

console.log(`\n✅ Done. Core publish is triggered via publish-core.yml (see RELEASE.md).`);
