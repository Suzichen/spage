#!/usr/bin/env node
/**
 * bump:engine — Bump @s-page/engine version across all related files.
 *
 * Usage:
 *   bun run bump:engine <version>            # bump only
 *   bun run bump:engine <version> --tag      # bump + git tag + push (triggers CI)
 *
 * Files modified (7 places per RELEASE.md):
 *   1. crates/spage-engine/Cargo.toml                          → version
 *   2. crates/spage-engine-napi/package.json                   → version + optionalDependencies (×3)
 *   3. crates/spage-engine-napi/npm/darwin-arm64/package.json  → version
 *   4. crates/spage-engine-napi/npm/linux-x64-gnu/package.json → version
 *   5. crates/spage-engine-napi/npm/win32-x64-msvc/package.json→ version
 *   6. crates/spage-engine-napi/package-lock.json              → version + optionalDependencies
 *   7. package.json (root)                                     → dependencies["@s-page/engine"]
 */

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { execSync } from "node:child_process";

const ROOT = resolve(import.meta.dirname, "..");

// ── Parse args ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
const version = args.find((a) => !a.startsWith("-"));
const shouldTag = args.includes("--tag");

if (!version) {
  console.error("Usage: bun run bump:engine <version> [--tag]");
  console.error("  e.g. bun run bump:engine 0.6.6");
  console.error("       bun run bump:engine 0.6.6 --tag   # also git tag + push");
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

// ── 1. Cargo.toml ───────────────────────────────────────────────────────────
console.log(`\nBumping engine to ${version}...\n`);

const cargoPath = "crates/spage-engine/Cargo.toml";
let cargo = readText(cargoPath);
cargo = cargo.replace(
  /^(version\s*=\s*")[\d.]+(")/m,
  `$1${version}$2`
);
writeText(cargoPath, cargo);

// ── 2. Main package.json ────────────────────────────────────────────────────
const mainPkgPath = "crates/spage-engine-napi/package.json";
const mainPkg = readJSON(mainPkgPath);
mainPkg.version = version;
for (const dep of Object.keys(mainPkg.optionalDependencies || {})) {
  mainPkg.optionalDependencies[dep] = version;
}
writeJSON(mainPkgPath, mainPkg);

// ── 3-5. Platform package.jsons ─────────────────────────────────────────────
const platforms = ["darwin-arm64", "linux-x64-gnu", "win32-x64-msvc"];
for (const plat of platforms) {
  const p = `crates/spage-engine-napi/npm/${plat}/package.json`;
  const pkg = readJSON(p);
  pkg.version = version;
  writeJSON(p, pkg);
}

// ── 6. package-lock.json ────────────────────────────────────────────────────
const lockPath = "crates/spage-engine-napi/package-lock.json";
const lock = readJSON(lockPath);
lock.version = version;
if (lock.packages?.[""]) {
  lock.packages[""].version = version;
  if (lock.packages[""].optionalDependencies) {
    for (const dep of Object.keys(lock.packages[""].optionalDependencies)) {
      lock.packages[""].optionalDependencies[dep] = version;
    }
  }
}
// Also update platform entries inside packages
for (const plat of platforms) {
  const key = `node_modules/@s-page/engine-${plat}`;
  if (lock.packages?.[key]) {
    lock.packages[key].version = version;
  }
}
writeJSON(lockPath, lock);

// ── 7. Root package.json ────────────────────────────────────────────────────
const rootPkgPath = "package.json";
const rootPkg = readJSON(rootPkgPath);
if (rootPkg.dependencies?.["@s-page/engine"]) {
  rootPkg.dependencies["@s-page/engine"] = version;
}
writeJSON(rootPkgPath, rootPkg);

// ── Sync Cargo.lock ─────────────────────────────────────────────────────────
console.log("\nSyncing Cargo.lock...");
run("cargo update -p spage-engine");

// ── Tag + Push ──────────────────────────────────────────────────────────────
if (shouldTag) {
  const tag = `engine-v${version}`;
  console.log(`\nCommitting and tagging ${tag}...`);
  run("git add -A");
  run(`git commit -m "chore: bump @s-page/engine to ${version}"`);
  run(`git tag ${tag}`);
  run(`git push origin HEAD ${tag}`);
  console.log(`\n🚀 Tag ${tag} pushed — CI will build and publish @s-page/engine@${version}`);
} else {
  console.log(
    `\n✅ Done. Review changes with \`git diff\`, then either:\n   • Re-run with --tag to commit + tag + push automatically\n   • Or manually: git add -A && git commit && git tag engine-v${version} && git push origin HEAD engine-v${version}`
  );
}
