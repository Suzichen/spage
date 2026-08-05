#!/usr/bin/env node
/**
 * bump:create — Bump create-spage version across all related files.
 *
 * Usage:
 *   bun run bump:create <version>        # bump only
 *   bun run bump:create <version> --tag  # bump + git tag + push (triggers CI)
 *
 * The scaffold's pinned core/engine versions always come from this source tree
 * (`packages/core/package.json`, `crates/spage-engine-napi/package.json`), because
 * spage-scaffold also embeds `packages/core/schemas` at compile time — a pinned version that
 * disagreed with the tree would ship schemas from a different core. Run bump:core / bump:engine
 * first, then bump:create.
 *
 * Files modified (6 places):
 *   1. crates/spage-scaffold/Cargo.toml                        → version
 *   2. packages/create-spage/package.json                      → version + optionalDependencies (×3)
 *   3. packages/create-spage/npm/darwin-arm64/package.json     → version
 *   4. packages/create-spage/npm/linux-x64-gnu/package.json   → version
 *   5. packages/create-spage/npm/win32-x64-msvc/package.json  → version
 *   6. crates/spage-scaffold/src/lib.rs                        → core resource + engine versions
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { execSync } from "node:child_process";
import { assertCompatible, assertKnownFlags, assertSemver, substitute, SEMVER } from "./version-utils.js";

const ROOT = resolve(import.meta.dirname, "..");

// ── Parse args ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
const version = args.find((a) => !a.startsWith("-"));
const shouldTag = args.includes("--tag");

if (!version) {
  console.error("Usage: bun run bump:create <version> [--tag]");
  console.error("  e.g. bun run bump:create 0.5.5");
  console.error("       bun run bump:create 0.5.5 --tag");
  process.exit(1);
}

assertKnownFlags(args, ["--tag"]);
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

function readText(relPath) {
  return readFileSync(resolve(ROOT, relPath), "utf-8");
}

function writeText(relPath, content) {
  writeFileSync(resolve(ROOT, relPath), content);
  console.log(`  ✔ ${relPath}`);
}

function run(cmd) {
  console.log(`  $ ${cmd}`);
  execSync(cmd, { cwd: ROOT, stdio: "inherit" });
}

// ── Scaffold pins, read from this source tree ───────────────────────────────
const coreVersion = readJSON("packages/core/package.json").version;
const engineVersion = readJSON("crates/spage-engine-napi/package.json").version;

assertSemver(coreVersion);
assertSemver(engineVersion);
// The scaffold must emit a pair that actually works together.
assertCompatible(coreVersion, engineVersion, "The scaffold cannot ship a mismatched pair.");

console.log(`  ℹ Scaffold will pin @s-page/core ${coreVersion} and @s-page/engine ${engineVersion}`);

// ── 1. Cargo.toml ───────────────────────────────────────────────────────────
console.log(`\nBumping create-spage to ${version}...\n`);

const cargoPath = "crates/spage-scaffold/Cargo.toml";
let cargo = readText(cargoPath);
cargo = substitute(
  cargo,
  new RegExp(String.raw`^(version\s*=\s*")${SEMVER}(")`, "m"),
  `$1${version}$2`,
  `${cargoPath} version`
);
writeText(cargoPath, cargo);

// ── 2. Main package.json ────────────────────────────────────────────────────
const mainPkgPath = "packages/create-spage/package.json";
const mainPkg = readJSON(mainPkgPath);
mainPkg.version = version;
for (const dep of Object.keys(mainPkg.optionalDependencies || {})) {
  mainPkg.optionalDependencies[dep] = version;
}
writeJSON(mainPkgPath, mainPkg);

// ── 3-5. Platform package.jsons ─────────────────────────────────────────────
const platforms = ["darwin-arm64", "linux-x64-gnu", "win32-x64-msvc"];
for (const plat of platforms) {
  const p = `packages/create-spage/npm/${plat}/package.json`;
  const pkg = readJSON(p);
  pkg.version = version;
  writeJSON(p, pkg);
}

// ── 6. lib.rs — update scaffold resource and engine versions ────────────────
const libPath = "crates/spage-scaffold/src/lib.rs";
let lib = readText(libPath);

lib = substitute(
  lib,
  new RegExp(String.raw`@s-page/core@${SEMVER}`),
  `@s-page/core@${coreVersion}`,
  `${libPath} spage.core`
);
console.log(`  ✔ ${libPath} — spage.core → @s-page/core@${coreVersion}`);

lib = substitute(
  lib,
  new RegExp(String.raw`(@s-page/engine\\": \\")${SEMVER}`),
  `$1${engineVersion}`,
  `${libPath} devDependencies["@s-page/engine"]`
);
console.log(`  ✔ ${libPath} — @s-page/engine → ${engineVersion}`);

writeText(libPath, lib);

// ── Sync Cargo.lock ─────────────────────────────────────────────────────────
console.log("\nSyncing Cargo.lock...");
run("cargo update -p spage-scaffold");

// ── Tag + Push ──────────────────────────────────────────────────────────────
if (shouldTag) {
  const tag = `create-v${version}`;
  console.log(`\nCommitting and tagging ${tag}...`);
  run("git add -A");
  run(`git commit -m "chore: bump create-spage to ${version}"`);
  run(`git tag ${tag}`);
  run(`git push origin HEAD ${tag}`);
  console.log(`\n🚀 Tag ${tag} pushed — CI will build and publish create-spage@${version}`);
} else {
  console.log(
    `\n✅ Done. Review changes with \`git diff\`, then either:\n   • Re-run with --tag to commit + tag + push automatically\n   • Or manually: git add -A && git commit && git tag create-v${version} && git push origin HEAD create-v${version}`
  );
}
