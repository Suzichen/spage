/**
 * Shared utilities for Rust engine regression tests.
 */
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');
const FIXTURES_DIR = path.join(__dirname, 'fixtures');
const BASEPATH_FIXTURES_DIR = path.join(__dirname, 'fixtures-basepath');
const GOLDEN_DIR = path.join(__dirname, 'golden');
const BASEPATH_GOLDEN_DIR = path.join(__dirname, 'golden-basepath');
const ENGINE_CLI = path.join(PROJECT_ROOT, 'crates', 'spage-engine-napi', 'bin', 'spage.cjs');
const CORE_VERSION = JSON.parse(
  fs.readFileSync(path.join(PROJECT_ROOT, 'packages', 'core', 'package.json'), 'utf-8'),
).version as string;

export { GOLDEN_DIR, BASEPATH_GOLDEN_DIR };

/** Recursively copy a directory */
function copyDirSync(src: string, dest: string): void {
  fs.mkdirSync(dest, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      copyDirSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

/** Shell HTML template used by SEO generation */
const SHELL_TEMPLATE = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Test Blog</title>
  <link rel="icon" href="./favicon.ico">
  <link rel="stylesheet" href="./assets/index.css">
</head>
<body>
  <div id="root"></div>
  <script type="module" src="./assets/index.js"></script>
</body>
</html>`;

/**
 * Set up a temporary directory with fixtures in the structure expected by the Rust engine.
 */
export function setupTmpDir(tmpDir: string, variant: 'default' | 'basepath' = 'default'): string {
  cleanupTmpDir(tmpDir);
  fs.mkdirSync(tmpDir, { recursive: true });

  const configDir = variant === 'basepath' ? BASEPATH_FIXTURES_DIR : FIXTURES_DIR;

  fs.copyFileSync(path.join(configDir, 'config.json'), path.join(tmpDir, 'config.json'));
  fs.copyFileSync(path.join(FIXTURES_DIR, 'album.config.json'), path.join(tmpDir, 'album.config.json'));
  copyDirSync(path.join(FIXTURES_DIR, 'posts'), path.join(tmpDir, 'posts'));
  copyDirSync(path.join(FIXTURES_DIR, 'albums'), path.join(tmpDir, 'albums'));

  fs.writeFileSync(
    path.join(tmpDir, 'package.json'),
    JSON.stringify({
      name: 'spage-regression-fixture',
      private: true,
      spage: {
        core: `@s-page/core@${CORE_VERSION}`,
        plugins: [],
      },
    }),
    'utf-8',
  );

  // Exercise the default package declaration path without registry access.
  const coreDir = path.join(tmpDir, '.cache', 'packages', '@s-page__core', CORE_VERSION);
  const shellDir = path.join(coreDir, 'dist', 'shell');
  fs.mkdirSync(shellDir, { recursive: true });
  fs.writeFileSync(path.join(coreDir, 'package.json'), '{}', 'utf-8');
  fs.writeFileSync(path.join(coreDir, '.spage-complete'), '', 'utf-8');
  fs.writeFileSync(path.join(shellDir, 'index.html'), SHELL_TEMPLATE, 'utf-8');

  return tmpDir;
}

/** Remove a temp directory */
export function cleanupTmpDir(tmpDir: string): void {
  if (fs.existsSync(tmpDir)) {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

/**
 * Run the Rust engine build command with cwd set to the given temp directory.
 * Returns true if the build succeeded.
 */
export function runRustEngine(tmpDir: string): boolean {
  try {
    execSync(`node "${ENGINE_CLI}" build`, {
      cwd: tmpDir,
      stdio: 'pipe',
      env: { ...process.env, NODE_ENV: 'production' },
      timeout: 120_000,
    });
    return true;
  } catch (err: any) {
    const stderr = err.stderr?.toString() || '';
    const stdout = err.stdout?.toString() || '';
    console.warn('[WARN] Rust engine build failed:');
    if (stdout) console.warn('  stdout:', stdout);
    if (stderr) console.warn('  stderr:', stderr);
    return false;
  }
}

/** Read a golden file and return its contents as a string. */
export function readGoldenFile(relativePath: string, variant: 'default' | 'basepath' = 'default'): string {
  const dir = variant === 'basepath' ? BASEPATH_GOLDEN_DIR : GOLDEN_DIR;
  return fs.readFileSync(path.join(dir, relativePath), 'utf-8');
}

/** Read a file from a temp directory output. */
export function readTmpOutput(relativePath: string, tmpDir: string): string {
  return fs.readFileSync(path.join(tmpDir, relativePath), 'utf-8');
}

/** Check if a file exists in a temp directory output. */
export function tmpOutputExists(relativePath: string, tmpDir: string): boolean {
  return fs.existsSync(path.join(tmpDir, relativePath));
}

/** Normalize dynamic timestamps in sitemap.xml. */
export function normalizeSitemapTimestamps(xml: string): string {
  const knownPostDates = [
    '2024-08-10', '2024-07-01', '2024-06-15', '2024-05-22',
    '2024-04-10', '2024-03-20', '2024-02-28', '2024-01-15',
  ];
  return xml.replace(/\r\n/g, '\n').replace(
    /<lastmod>(\d{4}-\d{2}-\d{2})<\/lastmod>/g,
    (match, date) => {
      if (knownPostDates.includes(date)) return match;
      return '<lastmod>NORMALIZED-DATE</lastmod>';
    },
  );
}

/** Normalize dynamic timestamps in rss.xml. */
export function normalizeRssTimestamps(xml: string): string {
  let normalized = xml.replace(/\r\n/g, '\n').replace(
    /<lastBuildDate>.*?<\/lastBuildDate>/g,
    '<lastBuildDate>NORMALIZED-DATE</lastBuildDate>',
  );

  const knownPubDatePatterns = [
    'Sat, 10 Aug 2024',
    'Mon, 01 Jul 2024',
    'Sun, 30 Jun 2024',
    'Sat, 15 Jun 2024',
    'Wed, 22 May 2024',
    'Wed, 10 Apr 2024',
    'Wed, 20 Mar 2024',
    'Wed, 28 Feb 2024',
    'Mon, 15 Jan 2024',
  ];

  normalized = normalized.replace(
    /<pubDate>(.*?)<\/pubDate>/g,
    (match, dateStr) => {
      const isKnown = knownPubDatePatterns.some((p) => dateStr.startsWith(p));
      if (isKnown) return match;
      return '<pubDate>NORMALIZED-DATE</pubDate>';
    },
  );

  return normalized;
}

/** Normalize dynamic timestamps in SEO HTML files. */
export function normalizeSeoTimestamps(html: string): string {
  let normalized = html.replace(/\r\n/g, '\n').replace(
    /content="\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z"/g,
    'content="NORMALIZED-ISO-TIMESTAMP"',
  );
  normalized = normalized.replace(
    /"datePublished":\s*"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z"/g,
    '"datePublished": "NORMALIZED-ISO-TIMESTAMP"',
  );
  return normalized;
}

/** List all files recursively in a directory, returning paths relative to the directory. */
export function listFilesRecursive(dir: string): string[] {
  const results: string[] = [];
  if (!fs.existsSync(dir)) return results;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      for (const sub of listFilesRecursive(fullPath)) {
        results.push(path.join(entry.name, sub));
      }
    } else {
      results.push(entry.name);
    }
  }
  return results;
}
