---
title: "New Feature: Album Module"
date: 2026-04-09 12:00:00
tags: [blog-system, album, photo-gallery]
categories: [Project]
preview: The blog system now includes a built-in album module for organizing and displaying photos — fully static, with auto-generated thumbnails, EXIF metadata, and a full-screen photo viewer.
---

## What's New

The blog system now has a built-in **album module** — a photo gallery feature that lets you organize photos into albums, with auto-generated thumbnails, EXIF metadata extraction, and a full-screen photo viewer.

Like everything else in this system, the album module is **purely static**. All processing happens at build time, and the frontend consumes static JSON and image files. No backend required.

---

## How It Works

The album module consists of three layers:

1. **Configuration** (`album.config.json`) — Define your albums
2. **Build** (`npm run build`) — Photos are processed as part of the standard build pipeline
3. **Frontend Pages** — Browse albums and view photos

### 1. Configure Albums

Albums are defined in a single config file:

```json
{
  "enabled": true,
  "albums": [
    { "dir": "travel-2024", "name": "2024 Travel", "cover": "cover.jpg" },
    { "dir": "日常写真" }
  ]
}
```

- **`dir`** — Directory name under `albums/`. Supports letters, numbers, hyphens, underscores, and CJK characters. No spaces or path separators.
- **`name`** (optional) — Display name. Defaults to `dir` if not set.
- **`cover`** (optional) — Cover photo filename. Falls back to the first photo.
- **`enabled`** — Toggle the entire module on/off. When disabled, album routes redirect to home and the nav link is hidden.

### 2. Prepare Your Photos

Place original photos in `albums/{dirname}/`:

```
albums/travel-2024/
  photo-01.jpg
  photo-02.jpg
  photo-03.png
```

Supported formats: `.jpg`, `.jpeg`, `.png`, `.webp`

### 3. Build

Album processing is part of the standard build command:

```bash
npm run build
```

The Rust build engine handles album processing automatically:

- **Scans** each album directory for photo files (direct children only, no recursion)
- **Generates WebP thumbnails** (max 1080px long side, aspect ratio preserved) in a `thumbs/` subdirectory
- **Extracts EXIF metadata** — camera make/model, focal length, aperture, shutter speed, ISO
- **Outputs JSON** index files to the output directory

The process is **incremental** — thumbnails are only regenerated when the source photo has been modified. This makes repeated builds fast.

### Sync Photos to Remote Storage

With an S3-compatible storage provider configured, run `spage sync --media`.

Sync follows `album.config.json`:

- Listed albums with a local folder are updated from that folder.
- Listed albums without a local folder are kept on remote storage. You do not need to download their photos first.
- Removing an album from the config and syncing removes it from the remote album list. Its photo files are kept.

If an upload fails, sync reports an error and keeps the previous album list. Run the command again to retry.

See the README's Media Sync section for storage and credential setup.

---

## Photo Directory Structure

After building, the output structure looks like this:

```
albums/                     ← Original photos (placed by you, version-controlled)
  travel-2024/
    photo-01.jpg
    photo-02.jpg
  日常写真/
    img-001.jpg

dist/                       ← Build output
  albums/
    travel-2024/
      thumbs/               ← Thumbnails (auto-generated)
        photo-01.webp
        photo-02.webp
    日常写真/
      thumbs/
        img-001.webp
  generated/                ← JSON index files (auto-generated)
    albums-index.json
    album-travel-2024.json
    album-日常写真.json
```

Original photos in `albums/` remain clean and version-controlled. All generated outputs go to `dist/`.

---

## Frontend Features

### Album List Page (`/albums`)

Displays all albums in a responsive grid with cover thumbnails, album names, and photo counts. Supports i18n — the entire UI adapts to Chinese, English, and Japanese.

### Album Detail Page (`/albums/:dirname`)

Shows all photos in a grid. Click any thumbnail to open the full-screen viewer.

### Photo Viewer

The photo viewer provides an immersive browsing experience:

- **Full-screen overlay** with semi-transparent dark backdrop
- **Progressive loading** — shows the thumbnail as a blurred placeholder while the original loads
- **EXIF data display** — camera info, focal length, aperture, shutter speed, ISO (only non-null fields shown)
- **Keyboard navigation** — `←` previous, `→` next, `Esc` close
- **Click backdrop to close**
- **Scroll lock** — page background doesn't scroll while the viewer is open

---

## Design Decisions

### Why build-time processing?

Consistent with the blog's philosophy: all heavy work at build time, zero runtime dependencies. The frontend just fetches static JSON and images.

### Why WebP thumbnails?

WebP offers significantly smaller file sizes than JPEG at comparable quality, resulting in faster page loads. The long side is capped at 1080px — large enough for previews, small enough for fast loading.

### Why Rust for image processing?

The build engine uses native Rust image processing for thumbnail generation and EXIF extraction, providing significantly faster performance compared to the previous TypeScript-based approach — especially for albums with many photos.

---

## Summary

The album module adds a fully static photo gallery to the blog system with minimal complexity. No new runtime dependencies, no backend changes, no database — just configuration, photos, and the standard build command.
