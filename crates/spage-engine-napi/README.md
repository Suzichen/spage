# @s-page/engine

Rust-powered build engine for [spage](https://spage.me) — handles Markdown parsing, image processing, SEO/feed generation, and development server with native performance.

## Install

```bash
npm install @s-page/engine
```

The correct native binary for your platform is installed automatically.

### Supported Platforms

| OS      | Arch  | Package                          |
|---------|-------|----------------------------------|
| Windows | x64   | `@s-page/engine-win32-x64-msvc`  |
| macOS   | ARM64 | `@s-page/engine-darwin-arm64`    |
| Linux   | x64   | `@s-page/engine-linux-x64-gnu`   |

> macOS Intel users: the ARM64 binary runs seamlessly via Rosetta 2.

## CLI

The package registers an `spage` binary. In a user project, this is invoked via npm scripts:

```json
{
  "scripts": {
    "dev": "spage serve",
    "build": "spage build"
  }
}
```

### `spage build`

Runs the full production build pipeline:

1. Copies the pre-built App Shell from `@s-page/core`
2. Generates posts manifest and copies Markdown files
3. Processes album photos (WebP thumbnails + EXIF extraction)
4. Generates SEO pages, sitemap.xml, rss.xml, robots.txt
5. Copies static assets from `public/`

```
spage build [--output <dir>]

Options:
  --output <dir>  Output directory (default: dist)
```

### `spage serve`

Starts a development server with live content rebuilding:

```
spage serve [--port <number>]

Options:
  --port <number>  Port to listen on (default: 3000)
```

## Node.js API

For programmatic use (e.g., custom build scripts):

```js
const {
  generatePostsData,
  generatePostsManifestOnly,
  generateAlbumsData,
  generateSeoPages,
  generateSitemap,
  generateRss,
  generateRobots,
  buildCommand,
  serveCommand,
} = require('@s-page/engine')
```

### `generatePostsData(postsDir, outputDir, configJson) → string`

Scans `postsDir` for Markdown files, parses frontmatter, generates `manifest.json`, and copies `.md` files to `outputDir/posts/`. Returns the manifest as a JSON string.

### `generatePostsManifestOnly(postsDir, outputDir, configJson) → string`

Same as `generatePostsData` but skips copying Markdown files. Used in dev mode where files are served directly from source.

### `generateAlbumsData(albumsDir, outputDir, albumConfigJson, siteConfigJson) → string`

Generates album index and per-album detail JSON files, including WebP thumbnail generation and EXIF extraction. Returns the albums-index JSON string.

### `generateSeoPages(manifestJson, templatePath, outputDir, configJson) → number`

Generates an SEO-optimized HTML page for each post (Open Graph, Twitter Card, JSON-LD). Returns the number of pages generated.

### `generateSitemap(manifestJson, outputPath, configJson) → void`

Generates `sitemap.xml` conforming to the Sitemaps protocol.

### `generateRss(manifestJson, outputPath, configJson) → void`

Generates `rss.xml` conforming to RSS 2.0.

### `generateRobots(outputPath, configJson) → void`

Generates `robots.txt` with sitemap reference.

### `buildCommand(optionsJson) → string`

Executes the full production build pipeline. Accepts `{ "outputDir": "dist" }`. Returns a JSON string with build results:

```json
{
  "durationMs": 1234,
  "shellFilesCount": 15,
  "postsCount": 3,
  "albumsCount": 2,
  "seoPagesCount": 3,
  "staticFilesCount": 5
}
```

### `serveCommand(optionsJson) → void`

Starts the dev server. Accepts `{ "port": 3000 }`. Blocks until terminated.

## Config Format

All `configJson` parameters accept a JSON string matching `config.json`:

```json
{
  "title": "My Blog",
  "description": "A personal blog",
  "logo": "/logo.png",
  "favicon": "/favicon.ico",
  "siteUrl": "https://example.com",
  "author": "Author",
  "language": "en",
  "timezone": "Asia/Tokyo",
  "basePath": "/",
  "github": "https://github.com/username/repo"
}
```

## License

MIT
