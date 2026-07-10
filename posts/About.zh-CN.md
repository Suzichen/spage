---
title: 关于本博客系统
date: 2025-12-19 12:00:00
tags: [blog-system, static-site, react, 中文]
categories: [Project]
preview: 介绍本网站背后的博客系统——它的工作原理、使用方法和部署方式。
---

## 关于本页

本页旨在介绍此博客系统的特性、使用方法和工作原理。

内容涵盖本站的构建流程、数据处理方式以及部署与维护指南。

**本站本身即是该系统的一个运行实例**。

---

## 快速开始

使用一条命令即可创建一个新博客：

```bash
npm create spage@latest my-blog
```

> 也可以使用 `bun create spage@latest my-blog` 或 `pnpm create spage my-blog`。

CLI 会引导你完成一些设置（项目名称、作者、用于 SEO 的网站 URL 等），并自动配置好一切。

然后：

```bash
cd my-blog
npm install
npm run dev
```

就是这样。你的博客已经跑起来了。

---

## 撰写文章

在 `posts/` 目录下创建一个 Markdown 文件：

```markdown
---
title: 我的第一篇文章
date: 2025-01-01
tags: [hello, blog]
categories: [General]
preview: 文章摘要，显示在列表中。
---

在这里写下你的内容...
```

构建系统会自动发现文章，无需手动注册。

### 多语言文章

使用文件名后缀发布同一篇文章的多个语言版本：

```
posts/
├── About.md          # 默认版本
├── About.zh-CN.md    # 中文版本
└── About.ja.md       # 日文版本
```

系统自动检测可用的翻译版本。当用户切换到没有对应翻译的语言时，会显示默认版本并附上回退提示。

### 语法指南

本系统支持使用标准 Markdown 语法撰写文章。若需查阅格式规范与排版示例，请参考 [Markdown 简明语法手册](../post/markdown)。

---

## 构建与部署

```bash
npm run build
```

这一条命令（由 Rust 构建引擎驱动）处理完整的构建流程：

1. **App Shell** — 从 `@s-page/core` 复制预构建的 React 前端
2. **文章处理** — 解析 frontmatter，生成清单，复制 Markdown 文件
3. **相册处理** — 生成 WebP 缩略图 + EXIF 元数据 JSON
4. **SEO 生成** — 生成每篇文章的 HTML 页面、sitemap.xml、rss.xml、robots.txt
5. **静态资源** — 复制 `public/` 到输出目录

输出结果是一个位于 `dist/` 目录下的纯静态网站。你可以将它部署在任何地方：

- 任何 CDN 或静态托管平台（Vercel、Netlify、Cloudflare Pages 等）
- 通过 CI/CD 部署到 GitHub Pages
- 使用 Nginx 或任何静态文件服务器进行私有化部署

构建完成后，不需要任何 Node.js 运行时或后端支持。

---

## 功能特性

### 📝 Markdown 博客
- Frontmatter 元数据（标题、日期、标签、分类、摘要）
- 基于 PrismJS 的语法高亮
- 自动生成目录
- 上一篇/下一篇导航

### 📸 相册功能
- 将照片放入 `albums/` 对应的目录即可
- 自动生成 WebP 缩略图（最大 1080px）
- 提取 EXIF 元数据（相机、镜头、光圈、快门速度、ISO）
- 全屏照片查看器
- 可通过 `album.config.json` 配置

### 🔍 SEO 优化
- 每篇文章生成独立的 HTML 页面，包含 Open Graph 和 Twitter Card 元标签
- 结构化数据 (JSON-LD)
- 自动生成 `sitemap.xml`、`rss.xml` 和 `robots.txt`
- 在 `config.json` 中配置 `siteUrl` 即可启用所有 SEO 功能

### 🌐 多语言 (i18n)
- 内置语言切换器（中文、英文、日文）
- 通过文件名后缀发布多语言文章
- 自动语言检测和回退机制

### ⚡ 性能优化
- 基于 React.lazy 的路由级代码分割
- 纯静态网站——没有运行时服务器开销
- 使用 Vite 进行优化的资源打包
- Rust 驱动的构建，实现近乎即时的内容处理

---

## 架构

spage 以三个 npm 包发布：

| 包名 | 用途 |
|------|------|
| `@s-page/core` | 预构建 App Shell — UI 组件、路由、样式、JSON Schema |
| `@s-page/engine` | Rust 驱动的构建引擎 — Markdown 解析、图片处理、SEO 生成、开发服务器 |
| `create-spage` | CLI 脚手架工具 — `npm create spage` |

你的项目只需要包含内容和配置：

```
my-blog/
├── posts/              # Markdown 文章
├── albums/             # 相册目录 (可选)
├── public/             # 静态资源 (logo, favicon)
├── config.json         # 站点配置
├── album.config.json   # 相册配置
└── package.json
```

更新框架：

```bash
npm update @s-page/core @s-page/engine
```

---

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
      { "platform": "email", "url": "mailto:you@example.com" },
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
| `siteUrl` | 否 | 生产环境 URL，启用 SEO 功能所必需 |
| `author` | 否 | 作者名，用于 SEO 元数据 |
| `language` | 否 | 默认语言代码 (`en`, `zh-CN`, `ja`) |
| `timezone` | 否 | IANA 时区，确保文章日期正确 |
| `basePath` | 否 | 子目录部署路径（如 `/blog`） |
| `links` | 否 | 友链插件 — 在右侧栏（桌面端）或页脚（移动端）显示文字链接 |
| `socialLinks` | 否 | 社交链接插件 — 内置 github、rss、x、weibo、zhihu、bilibili、email、facebook、instagram、tiktok 图标，支持自定义图标 |

### 相册配置 (`album.config.json`)

```json
{
  "enabled": true,
  "albums": [
    { "dir": "travel", "name": "旅行照片", "cover": "cover.jpg" },
    { "dir": "日常" }
  ]
}
```

---

## 适用人群

**非常适合：**
- 个人博客
- 技术写作
- 摄影作品集
- 文档类网站
- 重视极简性和可移植性的项目

**不适用于：**
- 多人发帖、互动的社区平台
- 需要频繁在页面上增删改查数据的动态应用
- 需要用户注册登录或依赖后端数据库的系统

---

## 结语

本网站既是为了发布内容，也是为了展示这套系统本身的运作效果。

静态单页应用（SPA）如何做到既极简，又不牺牲结构与 SEO？如果你对这个问题感兴趣，本项目提供了一种可能的思路。
