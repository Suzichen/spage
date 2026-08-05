#!/usr/bin/env node
/**
 * bump:create — Bump create-spage version across all related files.
 *
 * Usage:
 *   bun run bump:create <version>                            # bump only
 *   bun run bump:create <version> --tag                      # bump + git tag + push (triggers CI)
 *   bun run bump:create <version> --core=0.7.0 --engine=0.7.0  # also update scaffold deps
 *
 * Files modified (7 places per RELEASE.md):
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

const ROOT = resolve(import.meta.dirname, "..");

// ── Parse args ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
const version = args.find((a) => !a.startsWith("-"));
const shouldTag = args.includes("--tag");

// Parse --core=x.y.z and --engine=x.y.z for updating scaffold template deps
// If not specified, auto-detect from current package.json files
const coreVersionArg = args.find((a) => a.startsWith("--core="))?.split("=")[1];
const engineVersionArg = args.find((a) => a.startsWith("--engine="))?.split("=")[1];

if (!version) {
  console.error("Usage: bun run bump:create <version> [--tag] [--core=X.Y.Z] [--engine=X.Y.Z]");
  console.error("  e.g. bun run bump:create 0.5.5");
  console.error("       bun run bump:create 0.5.5 --tag");
  console.error("       bun run bump:create 0.5.5 --core=0.7.0 --engine=0.7.0 --tag");
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

// ── Auto-detect core/engine versions if not specified ────────────────────────
const coreVersion = coreVersionArg || readJSON("packages/core/package.json").version;
const engineVersion = engineVersionArg || readJSON("crates/spage-engine-napi/package.json").version;

const coreParts = coreVersion.split(".").map(Number);
const engineParts = engineVersion.split(".").map(Number);
const compatible = coreParts[0] === engineParts[0]
  && (engineParts[0] > 0 || coreParts[1] === engineParts[1]);
if (!compatible) {
  console.error(`Error: core ${coreVersion} is not compatible with engine ${engineVersion}`);
  console.error("Pre-1.0 versions must share major/minor; stable versions must share major.");
  process.exit(1);
}

if (!coreVersionArg) console.log(`  ℹ Auto-detected @s-page/core version: ${coreVersion}`);
if (!engineVersionArg) console.log(`  ℹ Auto-detected @s-page/engine version: ${engineVersion}`);

// ── 1. Cargo.toml ───────────────────────────────────────────────────────────
console.log(`\nBumping create-spage to ${version}...\n`);

const cargoPath = "crates/spage-scaffold/Cargo.toml";
let cargo = readText(cargoPath);
cargo = cargo.replace(
  /^(version\s*=\s*")[\d.]+(")/m,
  `$1${version}$2`
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
let libLines = readText(libPath).split("\n");

for (let i = 0; i < libLines.length; i++) {
  const line = libLines[i];
  if (line.includes("@s-page/core@")) {
    libLines[i] = line.replace(/@s-page\/core@[\d.]+/, `@s-page/core@${coreVersion}`);
    console.log(`  ✔ ${libPath}:${i + 1} — spage.core → @s-page/core@${coreVersion}`);
  }
  if (line.includes("@s-page/engine")) {
    libLines[i] = line.replace(/(engine\\\": \\")[\d.]+/, `$1${engineVersion}`);
    console.log(`  ✔ ${libPath}:${i + 1} — @s-page/engine → ${engineVersion}`);
  }
}

writeText(libPath, libLines.join("\n"));

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
