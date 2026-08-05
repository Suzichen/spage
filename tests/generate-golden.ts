/**
 * Generate golden files by running the Rust engine against test fixtures.
 *
 * Run: npx tsx tests/generate-golden.ts
 */
import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');
const FIXTURES_DIR = path.join(__dirname, 'fixtures');
const BASEPATH_FIXTURES_DIR = path.join(__dirname, 'fixtures-basepath');
const TMP_DIR = path.join(__dirname, '.tmp');
const ENGINE_CLI = path.join(PROJECT_ROOT, 'crates', 'spage-engine-napi', 'bin', 'spage.cjs');
const CORE_VERSION = JSON.parse(
  fs.readFileSync(path.join(PROJECT_ROOT, 'packages', 'core', 'package.json'), 'utf-8'),
).version as string;

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

function rmDirSync(dir: string): void {
  if (fs.existsSync(dir)) fs.rmSync(dir, { recursive: true, force: true });
}

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

function setupTmp(configDir: string): void {
  rmDirSync(TMP_DIR);
  fs.mkdirSync(TMP_DIR, { recursive: true });

  fs.copyFileSync(path.join(configDir, 'config.json'), path.join(TMP_DIR, 'config.json'));
  fs.copyFileSync(path.join(FIXTURES_DIR, 'album.config.json'), path.join(TMP_DIR, 'album.config.json'));
  copyDirSync(path.join(FIXTURES_DIR, 'posts'), path.join(TMP_DIR, 'posts'));
  copyDirSync(path.join(FIXTURES_DIR, 'albums'), path.join(TMP_DIR, 'albums'));

  fs.writeFileSync(
    path.join(TMP_DIR, 'package.json'),
    JSON.stringify({
      name: 'spage-golden-fixture',
      private: true,
      spage: {
        core: `@s-page/core@${CORE_VERSION}`,
        plugins: [],
      },
    }),
    'utf-8',
  );

  // Pre-seed the package cache so the build resolves the declared core without registry access.
  const coreDir = path.join(TMP_DIR, '.cache', 'packages', '@s-page__core', CORE_VERSION);
  const shellDir = path.join(coreDir, 'dist', 'shell');
  fs.mkdirSync(shellDir, { recursive: true });
  fs.writeFileSync(path.join(coreDir, 'package.json'), '{}', 'utf-8');
  fs.writeFileSync(path.join(coreDir, '.spage-complete'), '', 'utf-8');
  fs.writeFileSync(path.join(shellDir, 'index.html'), SHELL_TEMPLATE, 'utf-8');
}

function runBuild(): boolean {
  console.log('  Running spage build...');
  try {
    const output = execSync(`node "${ENGINE_CLI}" build`, {
      cwd: TMP_DIR,
      stdio: 'pipe',
      env: { ...process.env, NODE_ENV: 'production' },
      timeout: 120_000,
    });
    console.log(`    ${output.toString().trim().replace(/\n/g, '\n    ')}`);
    return true;
  } catch (err: any) {
    console.error('  Build failed:', err.stderr?.toString() || err.message);
    return false;
  }
}

function copyToGolden(goldenDir: string, tmpRelPath: string, goldenRelPath: string): boolean {
  const src = path.join(TMP_DIR, tmpRelPath);
  if (!fs.existsSync(src)) {
    console.warn(`  [WARN] Expected output not found: ${tmpRelPath}`);
    return false;
  }
  const dest = path.join(goldenDir, goldenRelPath);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
  return true;
}

function listFilesRecursive(dir: string): string[] {
  const results: string[] = [];
  if (!fs.existsSync(dir)) return results;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...listFilesRecursive(fullPath));
    } else {
      results.push(fullPath);
    }
  }
  return results;
}

function generateForVariant(label: string, configDir: string, goldenDir: string): void {
  console.log(`\n=== Generating golden files: ${label} ===\n`);

  rmDirSync(goldenDir);
  fs.mkdirSync(goldenDir, { recursive: true });
  setupTmp(configDir);

  if (!runBuild()) {
    console.error(`  FAILED to generate golden files for ${label}`);
    return;
  }

  // Copy outputs to golden dir
  copyToGolden(goldenDir, 'dist/generated/manifest.json', 'manifest.json');
  copyToGolden(goldenDir, 'dist/generated/albums-index.json', 'albums-index.json');

  const generatedDir = path.join(TMP_DIR, 'dist', 'generated');
  if (fs.existsSync(generatedDir)) {
    for (const file of fs.readdirSync(generatedDir)) {
      if (file.startsWith('album-') && file.endsWith('.json')) {
        copyToGolden(goldenDir, `dist/generated/${file}`, file);
      }
    }
  }

  // SEO pages
  const seoDir = path.join(TMP_DIR, 'dist', 'post');
  if (fs.existsSync(seoDir)) {
    copyDirSync(seoDir, path.join(goldenDir, 'seo'));
  }

  copyToGolden(goldenDir, 'dist/sitemap.xml', 'sitemap.xml');
  copyToGolden(goldenDir, 'dist/rss.xml', 'rss.xml');
  copyToGolden(goldenDir, 'dist/robots.txt', 'robots.txt');

  const files = listFilesRecursive(goldenDir);
  console.log(`\n  Generated ${files.length} golden files for ${label}`);
}

function main() {
  console.log('=== Golden File Generator (Rust Engine) ===');

  generateForVariant('default (basePath: "/")', FIXTURES_DIR, path.join(__dirname, 'golden'));
  generateForVariant('basepath (basePath: "/blog")', BASEPATH_FIXTURES_DIR, path.join(__dirname, 'golden-basepath'));

  rmDirSync(TMP_DIR);
  console.log('\n=== Done ===');
}

main();
