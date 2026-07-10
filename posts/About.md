---
title: About This Blog System
date: 2025-12-19 12:00:00
tags: [blog-system, static-site, react]
categories: [Project]
preview: An introduction to the blog system behind this site — how it works, how to use it, and how to deploy.
---

## What This Page Is

This page documents **the blog system itself**.

It explains how this site is built, how content is handled, and how the system can be deployed and maintained.

If you are reading this on the live site, you are already looking at a real instance of this system in action.

---

## Quick Start

Create a new blog in one command:

```bash
npm create spage@latest my-blog
```

> You can also use `bun create spage@latest my-blog` or `pnpm create spage my-blog`.

The CLI will walk you through a few prompts (project name, author, site URL for SEO, etc.) and set up everything automatically.

Then:

```bash
cd my-blog
npm install
npm run dev
```

That's it. Your blog is running.

---

## Writing Posts

Create a Markdown file under `posts/`:

```markdown
---
title: My First Post
date: 2025-01-01
tags: [hello, blog]
categories: [General]
preview: A short summary for the post list.
---

Your content here...
```

Posts are automatically picked up by the build system — no manual registration needed.

### Multi-language Posts

To publish a post in multiple languages, use filename suffixes:

```
posts/
├── About.md          # Default version
├── About.zh-CN.md    # Chinese version
└── About.ja.md       # Japanese version
```

The system automatically detects available translations. When a user switches to a language without a localized version, the default version is shown with a fallback notice.

### Syntax Guide

This system supports standard Markdown for writing posts. For formatting guidelines and layout examples, please refer to the [Markdown Quick Reference](../post/markdown).

---

## Building & Deploying

```bash
npm run build
```

This single command (powered by the Rust build engine) handles the full pipeline:

1. **App Shell** — copies the pre-built React frontend from `@s-page/core`
2. **Posts** — parses frontmatter, generates manifest, copies Markdown files
3. **Albums** — generates WebP thumbnails + EXIF metadata JSON
4. **SEO** — generates per-post HTML pages, sitemap.xml, rss.xml, robots.txt
5. **Static assets** — copies `public/` to output

The output is a fully static site in `dist/`. Deploy it anywhere:

- Any CDN or static hosting (Vercel, Netlify, Cloudflare Pages, etc.)
- GitHub Pages via CI/CD
- Self-hosted with Nginx or any static file server

No Node.js runtime or backend required after build.

---

## Features

### 📝 Markdown Blogging
- Frontmatter metadata (title, date, tags, categories, preview)
- Syntax highlighting with PrismJS
- Table of contents generation
- Previous/next post navigation

### 📸 Photo Albums
- Drop photos into `albums/` directories
- Auto-generated WebP thumbnails (max 1080px)
- EXIF metadata extraction (camera, lens, aperture, shutter speed, ISO)
- Full-screen photo viewer
- Configurable via `album.config.json`

### 🔍 SEO
- Per-post HTML pages with Open Graph and Twitter Card meta tags
- Structured data (JSON-LD)
- Auto-generated `sitemap.xml`, `rss.xml`, and `robots.txt`
- Configure `siteUrl` in `config.json` to enable all SEO features

### 🌐 i18n
- Built-in language switcher (English, Chinese, Japanese)
- Multi-language posts via filename suffixes
- Automatic language detection and fallback

### ⚡ Performance
- Route-level code splitting via React.lazy
- Static site — no runtime server overhead
- Optimized asset bundling with Vite
- Rust-powered build for near-instant content processing

---

## Architecture

spage is published as three npm packages:

| Package | Purpose |
|---------|---------|
| `@s-page/core` | Pre-built App Shell — UI components, routing, styles, JSON schemas |
| `@s-page/engine` | Rust-powered build engine — Markdown parsing, image processing, SEO generation, dev server |
| `create-spage` | CLI scaffolding tool — `npm create spage` |

Your project only contains content and configuration:

```
my-blog/
├── posts/              # Markdown posts
├── albums/             # Photo album directories (optional)
├── public/             # Static assets (logo, favicon)
├── config.json         # Site configuration
├── album.config.json   # Album configuration
└── package.json
```

To update the framework:

```bash
npm update @s-page/core @s-page/engine
```

---

## Configuration

### Site Config (`config.json`)

```json
{
  "title": "My Blog",
  "description": "A personal blog",
  "logo": "/logo.png",
  "favicon": "/favicon.ico",
  "siteUrl": "https://example.com",
  "author": "Your Name",
  "language": "en",
  "timezone": "Asia/Tokyo",
  "links": {
    "enabled": true,
    "items": {
      "Friend Blog": "https://example.com"
    }
  },
  "socialLinks": {
    "enabled": true,
    "items": [
      { "platform": "rss" },
      { "platform": "github", "url": "https://github.com/username/repo" },
      { "platform": "email", "url": "mailto:you@example.com" },
      { "platform": "custom", "url": "https://example.com", "icon": "/icons/my-icon.png", "label": "My Site" }
    ]
  }
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `title` | Yes | Site title |
| `description` | Yes | Site description |
| `logo` | Yes | Logo image path |
| `favicon` | Yes | Favicon path |
| `siteUrl` | No | Production URL. Enables SEO features |
| `author` | No | Author name for SEO metadata |
| `language` | No | Default language (`en`, `zh-CN`, `ja`) |
| `timezone` | No | IANA timezone for correct post dates |
| `basePath` | No | Sub-directory deployment path (e.g., `/blog`) |
| `links` | No | Friend links widget — text links in right sidebar (desktop) or footer (mobile) |
| `socialLinks` | No | Social icon links widget — built-in icons for github, rss, x, weibo, zhihu, bilibili, email, facebook, instagram, tiktok; supports custom icons |

### Album Config (`album.config.json`)

```json
{
  "enabled": true,
  "albums": [
    { "dir": "travel", "name": "Travel Photos", "cover": "cover.jpg" },
    { "dir": "日常" }
  ]
}
```

---

## Who This System Is For

**Good fit for:**
- Personal blogs
- Technical writing
- Photo portfolios
- Documentation-style sites
- Projects that value simplicity and portability

**Not intended for:**
- User-generated content platforms
- Applications requiring runtime data mutation
- Systems that depend on authentication or server-side state

---

## Closing Note

This blog exists not only to publish content, but also to document the system that publishes it.

If you are interested in how static sites can remain simple without sacrificing structure or SEO, this project is one possible answer.
