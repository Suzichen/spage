
# create-spage

The official scaffolding CLI for creating an [spage](https://spage.me) project.

## Usage

```bash
# npm
npm create spage@latest my-blog

# bun
bun create spage@latest my-blog

# pnpm
pnpm create spage my-blog

# yarn
yarn create spage my-blog
```

To initialize in the current directory:

```bash
npm create spage@latest .
```

The CLI prompts for project name, description, author, site URL, and timezone — then generates everything you need.

## What Gets Generated

```
my-blog/
├── posts/
│   ├── hello-world.md    # Sample welcome post
│   └── about.md          # Sample about page
├── albums/
│   └── blog/             # Sample album directory
├── public/
│   ├── logo.png
│   ├── logomax.png
│   ├── favicon.ico
│   └── _redirects
├── config.json           # Site configuration (with your answers filled in)
├── album.config.json     # Album configuration
├── memo.config.json      # Memo module configuration (Ech0 integration, disabled by default)
├── package.json          # { "dev": "spage serve", "build": "spage build" }
└── .gitignore
```

### Generated `package.json`

```json
{
  "name": "my-blog",
  "private": true,
  "type": "module",
  "spage": {
    "core": "@s-page/core@0.6.10",
    "plugins": []
  },
  "scripts": {
    "dev": "spage serve",
    "build": "spage build",
    "update": "spage update"
  },
  "devDependencies": {
    "@s-page/engine": "0.6.8"
  }
}
```

The engine resolves the exact core version from `spage.core` into `.cache/packages`; core is not installed into `node_modules`. `spage update` automatically selects the newest core compatible with the installed engine.

The config files reference `./.cache/generated/schemas/*.json`. `create spage` seeds them so the project validates in your editor right away, and every `dev`/`build` re-mirrors them from the resolved core — so editor validation always matches your pinned core version.

## After Scaffolding

```bash
cd my-blog
npm install
npm run dev      # Start development server (localhost:3000)
npm run build    # Build for production → dist/
```

## Configuration

### `config.json`

| Field | Required | Description |
|-------|----------|-------------|
| `title` | Yes | Site title |
| `description` | Yes | Site description |
| `logo` | Yes | Logo image path |
| `favicon` | Yes | Favicon path |
| `siteUrl` | No | Production URL (enables SEO features) |
| `author` | No | Author name |
| `language` | No | Default language (`en`, `zh-CN`, `ja`) |
| `timezone` | No | IANA timezone for correct post dates |
| `basePath` | No | Sub-directory path (e.g., `/blog`) |
| `github` | No | GitHub URL (shows icon in top-right) |

### `album.config.json`

```json
{
  "enabled": true,
  "albums": [
    { "dir": "blog", "name": "Blog Photos" }
  ]
}
```

### `memo.config.json`

Enables the Memo module powered by [Ech0](https://github.com/lin-snow/ech0). Disabled by default.

```json
{
  "enabled": true,
  "provider": "ech0",
  "serverUrl": "https://your-ech0-instance.com",
  "pageSize": 20,
  "title": "Memo"
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `enabled` | Yes | Toggle the memo module on/off |
| `provider` | Yes | Data provider (currently only `"ech0"`) |
| `serverUrl` | Yes | Ech0 instance URL |
| `pageSize` | No | Memos per load (default: 20) |
| `title` | No | Custom page title (falls back to i18n default) |

## License

MIT © Suzichen
