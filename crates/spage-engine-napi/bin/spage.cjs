#!/usr/bin/env node
'use strict';

const path = require('path');
const pkg = require('../package.json');

const args = process.argv.slice(2);

function printHelp() {
  console.log(`spage v${pkg.version}

Usage: spage <command> [options]

Commands:
  build   Build the blog for production deployment
  serve   Start a development preview server
  update  Update Spage resources
  sync    Sync media files to S3-compatible storage

Options:
  --version  Show version number
  --help     Show this help message

Run "spage <command> --help" for command-specific options.`);
}

function printBuildHelp() {
  console.log(`Usage: spage build [options]

Build the blog for production deployment.

Options:
  --output <dir>  Output directory (default: dist)`);
}

function printServeHelp() {
  console.log(`Usage: spage serve [options]

Start a development preview server.

Options:
  --port <number>  Port to listen on (default: 3000)`);
}

function printSyncHelp() {
  console.log(`Usage: spage sync --media [options]

Sync local album media to S3-compatible storage.

Options:
  --media     Sync album media files (required)
  --dry-run   Preview files to upload without uploading`);
}

function printUpdateHelp() {
  console.log(`Usage: spage update [core|plugins]

Update resource versions recorded in package.json.spage.
Without a target, both core and registered plugins are updated.`);
}

function getFlag(flag) {
  const idx = args.indexOf(flag);
  if (idx === -1) return undefined;
  return args[idx + 1];
}

function hasFlag(flag) {
  return args.includes(flag);
}

function loadEngine() {
  try {
    return require(path.resolve(__dirname, '..', 'index.js'));
  } catch (e1) {
    try {
      return require('@s-page/engine');
    } catch (e2) {
      process.stderr.write(`Error: Failed to load @s-page/engine: ${e2.message}\n`);
      process.exit(1);
    }
  }
}

// Global flags
if (hasFlag('--version') || hasFlag('-v')) {
  console.log(pkg.version);
  process.exit(0);
}

if (args.length === 0 || (args.length === 1 && hasFlag('--help'))) {
  printHelp();
  process.exit(0);
}

const command = args[0];

if (command === 'build') {
  if (hasFlag('--help')) {
    printBuildHelp();
    process.exit(0);
  }

  const engine = loadEngine();
  const opts = {};
  const output = getFlag('--output');
  if (output) opts.outputDir = output;

  try {
    const resultJson = engine.buildCommand(JSON.stringify(opts));
    const result = JSON.parse(resultJson);
    console.log(`\nBuild completed in ${result.durationMs}ms`);
    console.log(`  Shell files: ${result.shellFilesCount}`);
    console.log(`  Posts: ${result.postsCount}`);
    console.log(`  Albums: ${result.albumsCount}`);
    console.log(`  SEO pages: ${result.seoPagesCount}`);
    console.log(`  Static files: ${result.staticFilesCount}`);
  } catch (e) {
    process.stderr.write(`Error: ${e.message}\n`);
    process.exit(1);
  }
} else if (command === 'serve') {
  if (hasFlag('--help')) {
    printServeHelp();
    process.exit(0);
  }

  const engine = loadEngine();
  const opts = {};
  const port = getFlag('--port');
  if (port !== undefined) {
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      process.stderr.write(`Error: Invalid port "${port}". Must be an integer between 1 and 65535.\n`);
      process.exit(1);
    }
    opts.port = p;
  }

  try {
    engine.serveCommand(JSON.stringify(opts));
  } catch (e) {
    process.stderr.write(`Error: ${e.message}\n`);
    process.exit(1);
  }
} else if (command === 'update') {
  if (hasFlag('--help')) {
    printUpdateHelp();
    process.exit(0);
  }

  const target = args[1] || 'all';
  if (!['all', 'core', 'plugins'].includes(target) || args.length > 2) {
    process.stderr.write(`Error: Invalid update target "${target}". Use core or plugins.\n`);
    process.exit(1);
  }

  const engine = loadEngine();
  try {
    const declaration = JSON.parse(engine.updateResourcesCommand(JSON.stringify({ target })));
    console.log('Spage resources updated:');
    console.log(`  Core: ${declaration.core}`);
    console.log(`  Plugins: ${declaration.plugins.length}`);
  } catch (e) {
    process.stderr.write(`Error: ${e.message}\n`);
    process.exit(1);
  }
} else if (command === 'sync') {
  if (hasFlag('--help')) {
    printSyncHelp();
    process.exit(0);
  }

  if (!hasFlag('--media')) {
    process.stderr.write(`Error: Missing --media flag. Run "spage sync --help" for usage.\n`);
    process.exit(1);
  }

  const engine = loadEngine();
  const opts = {};
  if (hasFlag('--dry-run')) opts.dryRun = true;

  try {
    const resultJson = engine.syncMediaCommand(JSON.stringify(opts));
    const result = JSON.parse(resultJson);
    console.log(`\nSync completed in ${result.durationMs}ms`);
    console.log(`  Uploaded: ${result.uploaded}`);
    console.log(`  Skipped: ${result.skipped}`);
    if (result.failed.length > 0) {
      console.log(`  Failed: ${result.failed.length}`);
      result.failed.forEach(f => console.log(`    - ${f}`));
    }
  } catch (e) {
    process.stderr.write(`Error: ${e.message}\n`);
    process.exit(1);
  }
} else {
  process.stderr.write(`Error: Unknown command "${command}". Run "spage --help" for usage.\n`);
  process.exit(1);
}
