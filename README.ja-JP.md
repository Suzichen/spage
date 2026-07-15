> *注：このプロジェクトは元々 s-blog という名前でしたが、現在は Spage に移行し、名前が変更されました。*

<div align="center">

<img alt="Ech0" src="./public/logo.svg" width="150">

# spage

<a title="en-US" href="./README.md"><img src="https://img.shields.io/badge/-English-545759?style=for-the-badge" alt="English"></a> <a title="zh-CN" href="./README.zh-CN.md"><img src="https://img.shields.io/badge/-简体中文-545759?style=for-the-badge" alt="简体中文"></a>  <img src="https://img.shields.io/badge/-日本語-F54A00?style=for-the-badge" alt="日本語">

React と Rust で構築されたモダンな高性能静的ブログシステム。

</div>

**プレビュー:**
- [spage 公式ブログ](https://spage.me)
- [Suzic's Blog](https://s-blog.suzichen.me/)

## 機能

- **技術スタック**: React 19, Vite, TypeScript, Rust (ビルドエンジン)
- **コンテンツ**: Markdown ベースの記事 (Hexo 互換 frontmatter)
- **機能**:
  - インスタント検索
  - アーカイブ (年/月)
  - タグとカテゴリー
  - 多言語サポート (英語、中国語、日本語)
  - EXIF メタデータ付きフォトアルバム
  - SEO (sitemap, RSS, Open Graph, JSON-LD)
- **スタイル**: Tailwind CSS によるクリーンなレスポンシブデザイン
- **パフォーマンス**: Rust 駆動のビルドパイプラインによる静的サイト生成

## クイックスタート

### デスクトップクライアント

[s-writor](https://github.com/Suzichen/s-writor/releases) クライアントを直接インストールしてください。作成/管理/プレビュー/ビルドの完全な機能を備えており、将来的にはワンクリック公開も統合予定です（サイトプレフィックスを申請するだけで `spage.me` にデプロイ可能）。

![s-writor](https://img.s-blog.me/s-writor/20260625/Snipaste_2026-06-25_15-48-39.png "s-writor")

### CLI を使用

#### 作成

新しいブログを作成する最速の方法：

```bash
npm create spage@latest
```

> **ヒント:** `bun create spage@latest my-blog` や `pnpm create spage my-blog` も使用可能です。

CLI がプロジェクトのセットアップをガイドします。初期化後：

```bash
cd my-blog
npm install
npm run dev
```

#### 本番ビルド

```bash
npm run build
```

このコマンド一つで完全なパイプラインを処理します：
1. プリビルト App Shell のコピー
2. 記事マニフェスト生成と Markdown ファイルのコピー
3. アルバム写真の処理（サムネイル + EXIF 抽出）
4. SEO ページ、sitemap.xml、rss.xml、robots.txt の生成

出力は `dist/` 内の完全な静的サイトです。任意の静的ホスティングにデプロイできます。

#### フレームワークの更新

```bash
npm update @s-page/core @s-page/engine
```

メンテナンスが必要なのはコンテンツファイル（`posts/`、`config.json`、`album.config.json`、`albums/`、`public/`）のみです。フレームワークの更新はパッケージマネージャーを通じて配信されます。

## アーキテクチャ

> **免責事項**: 本システムのすべてのコードは AI によって生成されています。

spage は 3 つの npm パッケージとして公開されています：

| パッケージ | 用途 |
|-----------|------|
| `@s-page/core` | プリビルト App Shell、UI コンポーネント、ルーティング、スタイル、JSON Schema |
| `@s-page/engine` | Rust 駆動のビルドエンジン — Markdown パース、画像処理、SEO 生成、開発サーバー |
| `create-spage` | CLI スキャフォールディングツール — `npm create spage` |

プロジェクトにはコンテンツと設定のみを含みます：

```
my-blog/
├── posts/              # Markdown 記事
├── albums/             # フォトアルバム (オプション)
├── public/             # 静的アセット (logo, favicon)
├── config.json         # サイト設定
├── album.config.json   # アルバム設定
└── package.json
```

## 設定

### サイト設定 (`config.json`)

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
      "友人ブログ": "https://example.com"
    }
  },
  "socialLinks": {
    "enabled": true,
    "items": [
      { "platform": "rss" },
      { "platform": "github", "url": "https://github.com/username/repo" },
      { "platform": "x", "url": "https://x.com/username" },
      { "platform": "custom", "url": "https://example.com", "icon": "/icons/my-icon.png", "label": "マイサイト" }
    ]
  }
}
```

| フィールド | 必須 | 説明 |
|-----------|------|------|
| `title` | はい | サイトタイトル |
| `description` | はい | サイト説明 |
| `logo` | はい | ロゴ画像パス |
| `favicon` | はい | Favicon パス |
| `siteUrl` | いいえ | 本番 URL。SEO 機能（sitemap、RSS、Open Graph）に必要 |
| `author` | いいえ | 著者名。SEO メタデータに使用 |
| `language` | いいえ | デフォルト言語コード (`en`, `zh-CN`, `ja`)。i18n フォールバック動作に影響 |
| `timezone` | いいえ | IANA タイムゾーン（例：`Asia/Tokyo`）。CI でのビルド時に記事の日付を正確にする |
| `basePath` | いいえ | サブディレクトリデプロイパス（例：`/blog`）。デフォルトは `/` |
| `links` | いいえ | リンクウィジェット（下記参照） |
| `socialLinks` | いいえ | ソーシャルアイコンリンクウィジェット（下記参照） |

#### リンクウィジェット (`links`)

右サイドバー（デスクトップ）またはフッター（モバイル）にテキストリンクのリストを表示します。

| フィールド | 必須 | 説明 |
|-----------|------|------|
| `links.enabled` | はい | リンクウィジェットのオン/オフ |
| `links.items` | はい | キーバリューペア：`{ "表示名": "URL" }` |

#### ソーシャルリンクウィジェット (`socialLinks`)

アイコンリンクの行を表示します。内蔵プラットフォーム：`github`、`rss`、`x`、`twitter`、`weibo`、`zhihu`、`bilibili`、`email`、`facebook`、`instagram`、`tiktok`。

| フィールド | 必須 | 説明 |
|-----------|------|------|
| `socialLinks.enabled` | はい | ソーシャルリンクウィジェットのオン/オフ |
| `socialLinks.items` | はい | ソーシャルリンク項目の配列 |
| `items[].platform` | はい | プラットフォーム名（内蔵）またはカスタムアイコン用の `"custom"` |
| `items[].url` | 場合による | リンク先 URL。`rss` は省略可（`siteUrl` から自動生成）、それ以外は必須 |
| `items[].icon` | いいえ | カスタムアイコン画像パス。`"custom"` または未認識プラットフォーム用 |
| `items[].label` | いいえ | ツールチップテキスト。デフォルトはプラットフォーム名 |

> **注意：** `platform` が `"rss"` で `url` が省略された場合、URL は自動的に `{siteUrl}/rss.xml` に設定されます。`siteUrl` が未設定の場合、その RSS 項目はレンダリングされません。

### アルバム設定 (`album.config.json`)

```json
{
  "enabled": true,
  "albums": [
    { "dir": "travel-2024", "name": "2024 Travel", "cover": "cover.jpg" },
    { "dir": "日常", "cover": "best.jpg" }
  ]
}
```

| フィールド | 必須 | 説明 |
|-----------|------|------|
| `enabled` | はい | アルバムモジュール全体のオン/オフ |
| `albums[].dir` | はい | `albums/` 下のディレクトリ名。英数字、ハイフン、アンダースコア、CJK 文字に対応 |
| `albums[].name` | いいえ | 表示名。デフォルトは `dir` |
| `albums[].cover` | いいえ | カバー写真のファイル名。デフォルトは最初の写真 |

## 記事の作成

`posts/` ディレクトリに Markdown ファイルを追加：

```yaml
---
title: 記事タイトル
date: 2024-01-01 12:00:00
tags: [Tech, React]
categories: [Programming]
preview: プレビュー用の短い説明。
---
```

### 多言語記事

ファイル名サフィックスを使って同じ記事の多言語版を公開：

```
posts/
├── About.md          # デフォルト（サイト言語または英語に対応）
├── About.zh-CN.md    # 中国語版
└── About.ja.md       # 日本語版
```

システムが利用可能な言語を自動検出し、ローカライズ版が利用できない場合はフォールバック通知を表示します。

## フォトアルバム

写真を `albums/{dirname}/` に配置。対応フォーマット：`.jpg`, `.jpeg`, `.png`, `.webp`

ビルドプロセスが自動的に：
- WebP サムネイル生成（最大 1080px）
- EXIF メタデータ抽出（カメラ、レンズ、絞り、シャッタースピード、ISO）
- JSON インデックスファイル生成

サムネイルはインクリメンタルに生成され、変更のない写真はスキップされます。

## メディア同期（S3互換ストレージ）

大規模なアルバムコレクションの場合、オリジナル写真をS3互換ストレージ（Cloudflare R2、AWS S3、Backblaze B2、MinIO）にオフロードし、デプロイには軽量なサムネイルのみを保持できます。

### セットアップ

1. `album.config.json` に `provider` ブロックを追加：

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

2. `.env` ファイルに認証情報を設定：

```
S3_ACCESS_KEY=your-access-key-id
S3_SECRET_KEY=your-secret-access-key
```

### コマンド

```bash
# オリジナル + サムネイル + インデックスJSONをS3にアップロード
spage sync --media

# アップロード予定ファイルをプレビュー
spage sync --media --dry-run
```

### 動作モード

| モード | 動作 |
|--------|------|
| **providerなし**（デフォルト） | オリジナル写真を `dist/` にコピー。標準的な静的ホスティング |
| **provider + ローカル `albums/` あり** | サムネイルをローカル生成。オリジナルはCDN（`publicUrl`）から配信 |
| **provider + ローカル `albums/` なし**（CI） | サムネイルとJSONをS3からプル。ローカルに写真不要 |

### CIワークフロー

provider使用時、写真をgitにコミットする必要はありません：

```yaml
# .github/workflows/deploy.yml
env:
  S3_ACCESS_KEY: ${{ secrets.S3_ACCESS_KEY }}
  S3_SECRET_KEY: ${{ secrets.S3_SECRET_KEY }}
steps:
  - uses: actions/checkout@v4
  - run: npm install
  - run: npx spage build   # S3からサムネイルを自動プル
  - run: # dist/ をデプロイ
```

同期ロックファイル（`.spage-sync.lock`）はgitにコミットしてください — アップロード済みファイルの状態を追跡します。

### 増分アップロード

`sync --media` はハイブリッドフィンガープリント戦略で重複アップロードを回避：
- ファイル ≤ 5MB：SHA-256コンテンツハッシュ
- ファイル > 5MB：ファイルサイズ + 更新日時

アップロード失敗（3回リトライ後）はログに記録しスキップ、後続ファイルをブロックしません。

## メモモジュール（Ech0 連携）

[Ech0](https://github.com/lin-snow/ech0) を利用した個人メモ/マイクロブログのタイムライン表示。データはランタイムで Ech0 インスタンスから取得され、ビルドは不要です。

### 前提条件

- ブラウザからアクセス可能な [Ech0](https://github.com/lin-snow/ech0) インスタンス

### 設定 (`memo.config.json`)

プロジェクトルートに `memo.config.json` を作成：

```json
{
  "enabled": true,
  "provider": "ech0",
  "serverUrl": "https://your-ech0-instance.com",
  "pageSize": 20,
  "title": "メモ"
}
```

| フィールド | 必須 | 説明 |
|-----------|------|------|
| `enabled` | はい | メモモジュールのオン/オフ |
| `provider` | はい | データソース。現在 `"ech0"` のみ対応 |
| `serverUrl` | はい | Ech0 インスタンスの URL |
| `pageSize` | いいえ | 1回あたりの読み込み件数。デフォルト: 20 |
| `title` | いいえ | カスタムページタイトル。省略時は i18n デフォルト名を使用 |

## SEO

`siteUrl` を設定すると、ビルド時に自動生成：

- **SEO HTML ページ** (`dist/post/*/index.html`) — Open Graph, Twitter Card, JSON-LD
- **sitemap.xml** — XML サイトマップ
- **rss.xml** — RSS 2.0 フィード
- **robots.txt** — クローラー指示

## コントリビューション

本プロジェクトは手動コーディングを厳格に禁止しています。すべてのコードは AI によって生成される必要があります。

## AI コントリビューター

- Gemini 3 Pro
- Gemini 3.1 Pro
- Claude Sonnet 4.5
- Claude Opus 4.5
- Claude Opus 4.6
- GPT 5.5
- GPT 5.6-sol

## エージェントツール

- [Antigravity](https://antigravity.google/)
- [Kiro](https://kiro.dev/)
- [Gemini CLI](https://geminicli.com/)
- [Gemini CLI in Zed](https://zed.dev/acp/agent/gemini-cli)
- [Kiro CLI](https://kiro.dev/cli)

## 謝辞

本プロジェクトは多くの優れたオープンソースプロジェクトの上に構築されています：

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
