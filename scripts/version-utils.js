/**
 * Shared version helpers for the bump:* scripts.
 *
 * Two concerns:
 *   1. semver-safe text substitution — every version string in the repo may carry a
 *      prerelease suffix (0.6.9-beta.1), so patterns must never be `[\d.]+`.
 *   2. core ↔ engine release-line compatibility, mirroring
 *      `spage_engine::packages::same_release_line`.
 */

/** Full semver, including prerelease and build metadata. */
export const SEMVER = String.raw`\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?`;

/** Exit unless `version` is a complete semver string. */
export function assertSemver(version) {
  if (new RegExp(`^${SEMVER}$`).test(version)) return;
  console.error(`Error: "${version}" is not a valid semver version (e.g. 0.7.0 or 0.7.0-beta.1)`);
  process.exit(1);
}

/**
 * Replace the first match of `pattern` in `text`.
 *
 * Exits when the pattern matches nothing at all — a silently skipped substitution leaves the
 * repo half-bumped (this is exactly what `[\d.]+` patterns did once a version carried a
 * prerelease suffix). A match that already holds the target value is fine, so re-running a
 * bump is still idempotent.
 */
export function substitute(text, pattern, replacement, label) {
  if (!pattern.test(text)) {
    console.error(`Error: ${label} — pattern ${pattern} matched nothing`);
    console.error("  Check the file by hand; the version strings may have drifted from what the script expects.");
    process.exit(1);
  }
  return text.replace(pattern, replacement);
}

/**
 * Exit on unrecognised `-`-prefixed arguments, so a removed flag fails loudly instead of
 * being silently ignored. Entries ending in `=` are matched as prefixes.
 */
export function assertKnownFlags(args, known) {
  const unknown = args.filter(
    (arg) =>
      arg.startsWith("-") &&
      !known.some((flag) => (flag.endsWith("=") ? arg.startsWith(flag) : arg === flag))
  );
  if (unknown.length === 0) return;
  console.error(`Error: unknown option(s): ${unknown.join(" ")}`);
  if (known.length > 0) console.error(`  Accepted: ${known.join(" ")}`);
  process.exit(1);
}

/**
 * core and engine must share a release line:
 *   pre-1.0 → major and minor must match (0.6.x engine ⇄ 0.6.x core)
 *   1.0+    → major must match
 */
export function isCompatible(coreVersion, engineVersion) {
  const line = (version) => {
    const [major, minor] = version.split(".").map((part) => parseInt(part, 10));
    return { major, minor };
  };
  const core = line(coreVersion);
  const engine = line(engineVersion);
  return core.major === engine.major && (engine.major > 0 || core.minor === engine.minor);
}

const RULE = "  Pre-1.0 releases must share major/minor; 1.0+ must share major.";

/**
 * Hard gate — use where shipping a mismatch would break users:
 *   • bump:engine, because an engine ahead of core leaves `spage update core` with nothing to pick
 *   • bump:create, because the scaffold must emit a working pair
 */
export function assertCompatible(coreVersion, engineVersion, hint) {
  if (isCompatible(coreVersion, engineVersion)) return;

  console.error(`Error: core ${coreVersion} is not compatible with engine ${engineVersion}`);
  console.error(RULE);
  if (hint) console.error(`  ${hint}`);
  process.exit(1);
}

/**
 * Soft gate — core moving to a new line first is the supported way to cross lines,
 * so warn instead of blocking, and say what has to follow.
 */
export function warnIncompatible(coreVersion, engineVersion, hint) {
  if (isCompatible(coreVersion, engineVersion)) return;
  console.warn(`  ⚠ core ${coreVersion} is on a different release line than engine ${engineVersion}`);
  if (hint) console.warn(`    ${hint}`);
}
