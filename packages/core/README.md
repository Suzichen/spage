# @s-page/core

The pre-built App Shell and UI framework for [spage](https://spage.me).

## What This Package Provides

- **Pre-built App Shell** — Production-ready `index.html` + hashed JS/CSS assets (React SPA)
- **UI Components** — Post list, post detail with Markdown rendering, photo albums, memo timeline (Ech0 integration), search overlay, language switcher, etc.
- **Routing** — Pre-configured React Router with code-splitting (lazy-loaded routes)
- **i18n** — Built-in internationalization (English, Chinese, Japanese)
- **JSON Schemas** — Validation schemas for `config.json` and `album.config.json`
- **Type Definitions** — TypeScript types for blog metadata, album data, and site config

## How It Works

`@s-page/core` is the **frontend** half of spage. It pairs with `@s-page/engine` (the **build engine**) to form the complete system:

1. `@s-page/engine` builds your content (Markdown → manifest, photos → thumbnails, SEO pages)
2. `@s-page/core` provides the App Shell that loads and renders that content at runtime

Users interact with both through simple npm scripts:

```bash
npm run dev    # Start dev server (powered by @s-page/engine)
npm run build  # Full production build (engine copies shell from core, then processes content)
```

## Installation

> **Recommended:** Use `npm create spage@latest` to scaffold a new project. It pins `spage.core` and installs `@s-page/engine` for you.

## Project Structure (User's Project)

After scaffolding, a user's project contains only content:

```
my-blog/
├── posts/              # Markdown posts
├── albums/             # Photo albums (optional)
├── public/             # Static assets (logo, favicon)
├── config.json         # Site configuration
├── album.config.json   # Album configuration
└── package.json        # scripts: { dev, build, update }
```

All framework code lives outside your project: the engine is a `devDependency`, and core is resolved from `package.json`'s `spage.core` into `.cache/packages`.

## Updating

```bash
npm install -D @s-page/engine@latest
npm run update
```

Upgrade the engine first; `spage update` then selects the newest compatible core and plugins.

## License

MIT © Suzichen
