> *注：此项目原名为 s-blog，现已全面迁移并更名为 Spage。*

<div align="center">

<img alt="Ech0" src="./public/logo.svg" width="150">

# spage

<a title="en-US" href="./README.md"><img src="https://img.shields.io/badge/-English-545759?style=for-the-badge" alt="English"></a> <img src="https://img.shields.io/badge/-简体中文-F54A00?style=for-the-badge" alt="简体中文">  <a title="ja" href="./README.ja-JP.md"><img src="https://img.shields.io/badge/-日本語-545759?style=for-the-badge" alt="日本語"></a>

一个由 React 和 Rust 构建的现代化高性能静态博客系统。

</div>

**预览地址:**
- [spage 官方站点](https://spage.me)
- [Suzic's Blog](https://s-blog.suzichen.me/)

## 功能特性

- **技术栈**: React 19, Vite, TypeScript, Rust (构建引擎)
- **内容**: 基于 Markdown 的文章 (兼容 Hexo frontmatter)
- **功能**:
  - 即时搜索
  - 归档 (按年/月)
  - 标签和分类
  - 多语言支持 (中文、英文、日文)
  - 带 EXIF 元数据的相册
  - SEO (sitemap, RSS, Open Graph, JSON-LD)
- **样式**: 基于 Tailwind CSS 的简洁响应式设计
- **性能**: Rust 驱动的构建管线生成纯静态站点

## 快速开始

### 桌面客户端

请直接安装 [s-writor](https://github.com/Suzichen/s-writor/releases) 客户端。它包含了完整的创建/管理/预览/构建等功能，未来还将集成一键发布(您只需要申请一个网站前缀即可部署到 `spage.me`)

![s-writor](https://img.s-blog.me/s-writor/20260625/Snipaste_2026-06-25_15-48-39.png "s-writor")

### 使用CLI

#### 创建

创建新博客最快捷的方式：

```bash
npm create spage@latest
```

> **提示:** 也可以使用 `bun create spage@latest my-blog` 或 `pnpm create spage my-blog`。

CLI 会引导你完成项目设置。初始化后：

```bash
cd my-blog
npm install
npm run dev
```

#### 构建生产版本

```bash
npm run build
```

这一条命令处理完整的构建流程：
1. 复制预构建的 App Shell
2. 生成文章清单并复制 Markdown 文件
3. 处理相册照片（缩略图 + EXIF 提取）
4. 生成 SEO 页面、sitemap.xml、rss.xml、robots.txt

输出是位于 `dist/` 下的纯静态站点，可部署到任何静态托管服务。

#### 更新框架

```bash
npm run update
```

Core 版本记录在 `package.json.spage` 中，并由 Spage engine 负责更新。

## 架构

> **声明**: 本系统中的所有代码均由 AI 生成。

spage 以三个 npm 包发布：

| 包名 | 用途 |
|------|------|
| `@s-page/core` | 预构建 App Shell、UI 组件、路由、样式、JSON Schema |
| `@s-page/engine` | Rust 驱动的构建引擎 — Markdown 解析、图片处理、SEO 生成、开发服务器 |
| `create-spage` | CLI 脚手架工具 — `npm create spage` |

你的项目只包含内容和配置：

```
my-blog/
├── posts/              # Markdown 文章
├── albums/             # 相册照片 (可选)
├── public/             # 静态资源 (logo, favicon)
├── config.json         # 站点配置
├── album.config.json   # 相册配置
└── package.json
```

## 配置

### 站点配置 (`config.json`)

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
      "友人博客": "https://example.com"
    }
  },
  "socialLinks": {
    "enabled": true,
    "items": [
      { "platform": "rss" },
      { "platform": "github", "url": "https://github.com/username/repo" },
      { "platform": "x", "url": "https://x.com/username" },
      { "platform": "custom", "url": "https://example.com", "icon": "/icons/my-icon.png", "label": "我的站点" }
    ]
  }
}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `title` | 是 | 网站标题 |
| `description` | 是 | 网站描述 |
| `logo` | 是 | Logo 图片路径 |
| `favicon` | 是 | Favicon 路径 |
| `siteUrl` | 否 | 生产环境 URL。SEO 功能（sitemap、RSS、Open Graph）必须配置此字段 |
| `author` | 否 | 作者名，用于 SEO 元数据 |
| `language` | 否 | 默认语言代码 (`en`, `zh-CN`, `ja`)。影响 i18n 回退行为 |
| `timezone` | 否 | IANA 时区标识（如 `Asia/Shanghai`）。确保在 CI 环境构建时文章日期正确 |
| `basePath` | 否 | 子目录部署路径（如 `/blog`）。默认为 `/` |
| `links` | 否 | 友链插件（见下方） |
| `socialLinks` | 否 | 社交图标链接插件（见下方） |

#### 友链插件 (`links`)

在右侧栏（桌面端）或页脚（移动端）显示文字链接列表。

| 字段 | 必填 | 说明 |
|------|------|------|
| `links.enabled` | 是 | 开启/关闭友链插件 |
| `links.items` | 是 | 键值对：`{ "显示名称": "URL" }` |

#### 社交链接插件 (`socialLinks`)

显示一组图标链接。内置平台：`github`、`rss`、`x`、`twitter`、`weibo`、`zhihu`、`bilibili`、`email`、`facebook`、`instagram`、`tiktok`。

| 字段 | 必填 | 说明 |
|------|------|------|
| `socialLinks.enabled` | 是 | 开启/关闭社交链接插件 |
| `socialLinks.items` | 是 | 社交链接项数组 |
| `items[].platform` | 是 | 平台名（内置）或 `"custom"` 使用自定义图标 |
| `items[].url` | 视情况 | 链接 URL。`rss` 可省略（自动从 `siteUrl` 推导），其他平台必填 |
| `items[].icon` | 否 | 自定义图标图片路径。用于 `"custom"` 或未识别的平台 |
| `items[].label` | 否 | 鼠标悬停提示文字。默认为平台名 |

> **注意：** 当 `platform` 为 `"rss"` 且未配置 `url` 时，URL 自动设为 `{siteUrl}/rss.xml`。如果 `siteUrl` 未配置，该 RSS 项不会渲染。

### 相册配置 (`album.config.json`)

```json
{
  "enabled": true,
  "albums": [
    { "dir": "travel-2024", "name": "2024 Travel", "desc": "旅途记录\n难忘的瞬间", "cover": "cover.jpg" },
    { "dir": "日常", "cover": "best.jpg" }
  ]
}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `enabled` | 是 | 开启/关闭整个相册模块 |
| `albums[].dir` | 是 | `albums/` 下的目录名。支持字母、数字、连字符、下划线、CJK 字符 |
| `albums[].name` | 否 | 显示名称。默认使用 `dir` |
| `albums[].desc` | 否 | 相册描述，不限制字数并支持换行。配置后会展示在详情页并生成专属 SEO 页面 |
| `albums[].cover` | 否 | 封面照片文件名。默认使用第一张照片 |

## 撰写文章

在 `posts/` 目录下添加 Markdown 文件：

```yaml
---
title: 我的文章标题
date: 2024-01-01 12:00:00
tags: [技术, React]
categories: [编程]
preview: 文章摘要，用于预览列表。
---
```

### 多语言文章

使用文件名后缀发布同一篇文章的多个语言版本：

```
posts/
├── About.md          # 默认版本（匹配站点语言或英文）
├── About.zh-CN.md    # 中文版本
└── About.ja.md       # 日文版本
```

系统自动检测可用语言版本，当本地化版本不可用时显示回退提示。

## 相册

将照片放在 `albums/{dirname}/` 下。支持的格式：`.jpg`, `.jpeg`, `.png`, `.webp`

构建过程自动：
- 生成 WebP 缩略图（最大 1080px）
- 提取 EXIF 元数据（相机、镜头、光圈、快门速度、ISO）
- 生成 JSON 索引文件

缩略图增量生成 — 未更改的照片会被跳过。

## 媒体同步 (S3 兼容存储)

对于大型相册集合，可以将原图托管到 S3 兼容的存储服务（Cloudflare R2、AWS S3、Backblaze B2、MinIO），部署时仅保留轻量的缩略图。

### 配置

1. 在 `album.config.json` 中添加 `provider` 配置块：

```jsonc
{
  "enabled": true,
  "albums": [...],
  "provider": {
    "type": "s3",
    "endpoint": "https://<account_id>.r2.cloudflarestorage.com",
    "region": "auto",
    "bucket": "my-blog-media",
    "publicUrl": "https://media.yourdomain.com"
  }
}
```

2. 创建 `.env` 文件配置密钥：

```
S3_ACCESS_KEY=your-access-key-id
S3_SECRET_KEY=your-secret-access-key
```

### 命令

```bash
# 上传原图 + 缩略图 + 索引 JSON 到 S3
spage sync --media

# 预览待上传文件（不实际上传）
spage sync --media --dry-run
```

### 工作模式

| 模式 | 行为 |
|------|------|
| **无 provider**（默认） | 原图复制到 `dist/`，标准静态托管 |
| **有 provider + 本地有 `albums/`** | 缩略图本地生成，原图从 CDN 加载（`publicUrl`） |
| **有 provider + 本地无 `albums/`**（CI） | 缩略图和 JSON 从 S3 拉取，无需本地图片 |

### CI 工作流

使用 provider 后，无需将图片提交到 git：

```yaml
# .github/workflows/deploy.yml
env:
  S3_ACCESS_KEY: ${{ secrets.S3_ACCESS_KEY }}
  S3_SECRET_KEY: ${{ secrets.S3_SECRET_KEY }}
steps:
  - uses: actions/checkout@v4
  - run: npm install
  - run: npx spage build   # 自动从 S3 拉取缩略图
  - run: # 部署 dist/
```

同步锁文件（`.spage-sync.lock`）需要提交到 git —— 它记录了已上传文件的状态。

### 增量上传

`sync --media` 使用混合指纹策略避免重复上传：
- 文件 ≤ 5MB：SHA-256 内容哈希
- 文件 > 5MB：文件大小 + 修改时间

上传失败（3 次重试后）会记录日志并跳过，不阻塞后续文件。

## Memo 模块（Ech0 集成）

基于 [Ech0](https://github.com/lin-snow/ech0) 展示个人动态/微博时间线。数据在运行时从 Ech0 实例获取，无需重新构建。

### 前置条件

- 一个正在运行的 [Ech0](https://github.com/lin-snow/ech0) 实例，可从浏览器访问

### 配置 (`memo.config.json`)

在项目根目录创建 `memo.config.json`：

```json
{
  "enabled": true,
  "provider": "ech0",
  "serverUrl": "https://your-ech0-instance.com",
  "pageSize": 20,
  "title": "动态"
}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `enabled` | 是 | 开启/关闭 Memo 模块 |
| `provider` | 是 | 数据源，目前仅支持 `"ech0"` |
| `serverUrl` | 是 | Ech0 实例 URL |
| `pageSize` | 否 | 每次加载条数，默认 20 |
| `title` | 否 | 自定义页面标题，缺省使用 i18n 默认名称（"Memo" / "动态" / "メモ"） |

## SEO

配置 `siteUrl` 后，构建会自动生成：

- **SEO HTML 页面** (`dist/post/*/index.html`) — Open Graph, Twitter Card, JSON-LD
- **sitemap.xml** — XML 站点地图
- **rss.xml** — RSS 2.0 订阅源
- **robots.txt** — 爬虫指令

## 贡献

本项目严格禁止手动编码。所有代码必须由 AI 生成。

## AI 贡献者

- Gemini 3 Pro
- Gemini 3.1 Pro
- Claude Sonnet 4.5
- Claude Opus 4.5
- Claude Opus 4.6
- GPT 5.5
- GPT 5.6-sol

## Agentic 工具

- [Antigravity](https://antigravity.google/)
- [Kiro](https://kiro.dev/)
- [Gemini CLI](https://geminicli.com/)
- [Gemini CLI in Zed](https://zed.dev/acp/agent/gemini-cli)
- [Kiro CLI](https://kiro.dev/cli)

## 致谢

本项目的构建离不开众多优秀的开源项目：

- [React](https://react.dev/)
- [Vite](https://vite.dev/)
- [Tailwind CSS](https://tailwindcss.com/)
- [react-markdown](https://github.com/remarkjs/react-markdown) & [remark-gfm](https://github.com/remarkjs/remark-gfm)
- [i18next](https://www.i18next.com/)
- [NAPI-RS](https://napi.rs/)
- [Tokio](https://tokio.rs/)
- [Hyper](https://hyper.rs/)
- [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)
- [image](https://github.com/image-rs/image) & [webp](https://github.com/nickkross/libwebp-rs)
- [Ech0](https://github.com/lin-snow/ech0)
