# spage-engine — Tauri Admin 集成指南

本文档说明如何在 Tauri Admin（或任何 Rust 项目）中将 `spage-engine` 作为 Cargo 依赖使用。

## 添加依赖

`spage-engine` 是一个独立的 Rust crate，不依赖 NAPI-RS 或 Node.js。可通过路径或 git 引用添加：

```toml
# Cargo.toml — 本地路径（monorepo 内开发时）
[dependencies]
spage-engine = { path = "../spage/crates/spage-engine" }

# 或 git 依赖
[dependencies]
spage-engine = { git = "https://github.com/user/spage.git" }
```

> 不要启用 `napi` feature，它仅用于 Node.js 绑定层的错误转换。

## Spage 资源解析

`build` 和 `serve` 在未提供 `shell_dir` 时读取项目的 `package.json.spage.core`，将精确版本下载并缓存到 `<项目>/.cache/packages`。显式 `shell_dir` 始终优先；旧项目只要已有 `node_modules/@s-page/core/dist/shell` 也可继续运行。

项目不再维护 `spage.requires`。core 与 engine 使用自动发布线兼容规则：`0.x` 要求 major/minor 相同，`1.x` 起要求 major 相同。`spage update core` 会从 registry 选择当前 engine 发布线内最高的稳定 core 版本；旧声明中的 `requires` 会在更新时自动移除。

```rust
use spage_engine::packages::{
    ensure_package, resolve_project_shell, update_resources,
    PackageResolverOptions, PackageSpec, UpdateOptions, UpdateTarget,
};

let shell_dir = resolve_project_shell(project_dir, None, None)?;

let cached_package = ensure_package(
    &PackageSpec::parse("@s-page/core@0.6.10")?,
    &PackageResolverOptions {
        cache_dir: Some(project_dir.join(".cache/packages")),
        registry_url: None,
    },
)?;

let declaration = update_resources(UpdateOptions {
    work_dir: project_dir.into(),
    target: UpdateTarget::Core,
    package_cache_dir: None,
    registry_url: None,
})?;
```

`update_resources` 写回 `package.json.spage` 后会准备对应缓存。`registry_url` 可用于 npm mirror。

## 配置类型

引擎通过两个配置结构体驱动，与用户项目中的 JSON 文件一一对应：

```rust
use spage_engine::{SiteConfig, AlbumConfig, AlbumEntry};

// 对应 config.json
let site_config: SiteConfig = serde_json::from_str(&std::fs::read_to_string("config.json")?)?;

// 对应 album.config.json
let album_config: AlbumConfig = serde_json::from_str(&std::fs::read_to_string("album.config.json")?)?;
```

也可以直接构造：

```rust
let site_config = SiteConfig {
    title: "My Blog".into(),
    description: "A personal blog".into(),
    logo: "/logo.png".into(),
    favicon: "/favicon.ico".into(),
    site_url: Some("https://example.com".into()),
    author: Some("Alice".into()),
    language: Some("en".into()),
    timezone: Some("Asia/Tokyo".into()),
    base_path: Some("/".into()),
    github: Some("https://github.com/user/repo".into()),
};

let album_config = AlbumConfig {
    enabled: true,
    albums: vec![
        AlbumEntry { dir: "travel".into(), name: Some("旅行".into()), cover: Some("cover.jpg".into()) },
        AlbumEntry { dir: "daily".into(), name: None, cover: None },
    ],
};
```

## API 概览

所有公开函数都返回 `Result<T, EngineError>`，使用 `?` 即可传播错误。

### 文章清单生成

扫描 Markdown 文件目录，解析 frontmatter，生成 `manifest.json` 并复制源文件。

```rust
use std::path::Path;
use spage_engine::posts::generate_posts_data;

let posts = generate_posts_data(
    Path::new("posts"),           // Markdown 文件目录
    Path::new("public"),          // 输出根目录
    &site_config,
)?;
// posts: Vec<PostMetadata>  — 按日期降序排列
// 写入: public/generated/manifest.json
// 复制: public/posts/*.md
```

### 相册数据生成

生成相册索引、每个相册的详情 JSON，以及 WebP 缩略图。

```rust
use spage_engine::albums::{generate_albums_data, generate_albums_data_with_base};

// 使用默认 basePath
let output = generate_albums_data(
    Path::new("albums"),          // 相册源目录
    Path::new("public"),          // 输出根目录
    &album_config,
)?;

// 或指定 basePath（子目录部署）
let output = generate_albums_data_with_base(
    Path::new("albums"),
    Path::new("public"),
    &album_config,
    Some("/blog"),
)?;

// output.summaries: Vec<AlbumSummary>
// output.details:   Vec<AlbumDetail>
// 写入: public/generated/albums-index.json
//       public/generated/album-{dirname}.json
// 生成: public/albums/{dirname}/thumbs/*.webp
```

### SEO 页面生成

为每篇文章生成带有完整 SEO 元数据的静态 HTML 页面。

```rust
use spage_engine::seo::generate_seo_pages;

let count = generate_seo_pages(
    &posts,                                // 文章清单（来自 generate_posts_data）
    Path::new("dist/index.html"),          // App Shell 模板路径
    Path::new("dist"),                     // 输出目录
    &site_config,
)?;
// count: usize — 生成的页面数
// 写入: dist/post/{slug}/index.html
```

### Sitemap 生成

```rust
use spage_engine::sitemap::generate_sitemap;

generate_sitemap(
    &posts,
    Path::new("dist/sitemap.xml"),
    &site_config,
)?;
// 若 site_url 未配置，会跳过生成并输出警告
```

### RSS 生成

```rust
use spage_engine::rss::generate_rss;

generate_rss(
    &posts,
    Path::new("dist/rss.xml"),
    &site_config,
)?;
// 若 site_url 未配置，会跳过生成并输出警告
```

### robots.txt 生成

```rust
use spage_engine::robots::generate_robots;

generate_robots(
    Path::new("dist/robots.txt"),
    &site_config,
)?;
```

## 完整构建流程示例

以下展示 Tauri Admin 中执行完整博客构建的典型流程：

```rust
use std::path::Path;
use spage_engine::{SiteConfig, AlbumConfig};

fn build_blog(
    project_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 读取配置
    let site_config: SiteConfig =
        serde_json::from_str(&std::fs::read_to_string(project_dir.join("config.json"))?)?;
    let album_config: AlbumConfig =
        serde_json::from_str(&std::fs::read_to_string(project_dir.join("album.config.json"))?)?;

    let public_dir = output_dir.join("public");

    // 2. 生成文章清单
    let posts = spage_engine::posts::generate_posts_data(
        &project_dir.join("posts"),
        &public_dir,
        &site_config,
    )?;

    // 3. 生成相册数据 + 缩略图
    spage_engine::albums::generate_albums_data_with_base(
        &project_dir.join("albums"),
        &public_dir,
        &album_config,
        site_config.base_path.as_deref(),
    )?;

    // 4. 生成 SEO 页面
    let dist_dir = output_dir.join("dist");
    spage_engine::seo::generate_seo_pages(
        &posts,
        &dist_dir.join("index.html"),  // App Shell 模板
        &dist_dir,
        &site_config,
    )?;

    // 5. 生成 sitemap + RSS + robots.txt
    spage_engine::sitemap::generate_sitemap(
        &posts,
        &dist_dir.join("sitemap.xml"),
        &site_config,
    )?;
    spage_engine::rss::generate_rss(
        &posts,
        &dist_dir.join("rss.xml"),
        &site_config,
    )?;
    spage_engine::robots::generate_robots(
        &dist_dir.join("robots.txt"),
        &site_config,
    )?;

    Ok(())
}
```

## 错误处理

所有函数返回 `Result<T, spage_engine::EngineError>`。主要错误变体：

| 变体 | 触发场景 |
|------|----------|
| `DirectoryNotFound` | posts/albums 目录不存在 |
| `FrontmatterParse` | Markdown frontmatter 解析失败 |
| `InvalidDate` | 日期格式无效 |
| `InvalidTimezone` | 时区标识符无效 |
| `ImageDecode` | 图片解码失败 |
| `InvalidAlbumName` | 相册目录名包含非法字符 |
| `Config` | 配置错误 |
| `ConfigNotFound` | 配置文件不存在 |
| `BuildStepFailed` | 构建步骤执行失败 |
| `PortInUse` | 开发服务器端口被占用 |
| `ServeDirNotFound` | serve 目录不存在（需先 build） |
| `ProjectDeclarationNotFound` | 项目缺少 `package.json.spage` 且没有可用的旧 core 声明 |
| `InvalidPackageSpec` | 资源声明不是合法的包名加精确 semver |
| `PackageNotFound` / `PackageVersionNotFound` | registry 中不存在包或版本 |
| `PackageNetwork` | registry metadata 或 tarball 下载失败 |
| `UnsafePackageArchive` | tarball 包含越界路径、链接或其他不安全 entry |
| `InvalidPackageCache` | 下载内容不完整或 core 缺少 App Shell |
| `CoreVersionMismatch` | 声明的 core 与当前 engine 不在同一兼容发布线 |
| `Cancelled` | 操作被用户取消 |
| `Io` | 文件系统 I/O 错误 |
| `Json` | JSON 序列化/反序列化错误 |
| `Yaml` | YAML 解析错误 |

```rust
use spage_engine::EngineError;

match result {
    Err(EngineError::DirectoryNotFound(path)) => {
        eprintln!("目录不存在: {}", path.display());
    }
    Err(e) => {
        eprintln!("构建失败: {e}");
    }
    Ok(_) => {}
}
```

## 底层模块

如需更细粒度的控制，可直接使用底层模块：

| 模块 | 用途 |
|------|------|
| `spage_engine::frontmatter` | Markdown frontmatter 解析 |
| `spage_engine::timezone` | 日期时区转换 |
| `spage_engine::image_proc` | 缩略图生成、尺寸计算 |
| `spage_engine::exif` | EXIF 元数据读取 |
| `spage_engine::path_util` | basePath 规范化、URL 构建 |
| `spage_engine::build` | 完整构建管线（buildCommand 使用） |
| `spage_engine::serve` | 开发服务器（serveCommand 使用） |
| `spage_engine::shell` | App Shell 复制逻辑 |
| `spage_engine::mime` | MIME 类型推断 |
| `spage_engine::packages` | package.json.spage 解析、registry 下载、缓存与资源更新 |

```rust
// 示例：单独解析 frontmatter
use spage_engine::frontmatter::parse_frontmatter;

let content = std::fs::read_to_string("posts/hello.md")?;
let (frontmatter, body) = parse_frontmatter(&content, "hello.md")?;
println!("标题: {:?}", frontmatter.title);
println!("标签: {:?}", frontmatter.tags);

// 示例：单独生成缩略图
use spage_engine::image_proc::generate_thumbnail;

generate_thumbnail(
    Path::new("photo.jpg"),
    Path::new("thumbs/photo.webp"),
)?;

// 示例：读取 EXIF
use spage_engine::exif::read_exif;

let exif = read_exif(Path::new("photo.jpg"));
println!("相机: {:?} {:?}", exif.camera_make, exif.camera_model);
```

## 注意事项

- 所有路径输出统一使用 `/` 作为分隔符，跨平台兼容
- 缩略图生成支持增量构建（已存在且较新的缩略图会跳过）
- 日志通过 `log` crate 输出，Tauri 项目中可用 `env_logger` 或 `tauri-plugin-log` 接收
- `SiteConfig` 使用 `#[serde(rename_all = "camelCase")]`，可直接从 camelCase JSON 反序列化

## Tauri 集成（Runtime 共享 & 进度回调）

在 Tauri 应用中，应用本身已有一个 tokio runtime。为避免 engine 内部再创建 runtime（导致资源泄漏），可通过 `ServeContext` / `SyncContext` 传入外部 Handle。

### 开发服务器（无泄漏启停）

```rust
use std::sync::Mutex;
use spage_engine::serve::{ServeConfig, ServeContext, ServeHandle, serve_with_context};

// Tauri state 中存储 handle
struct AppState {
    serve_handle: Mutex<Option<ServeHandle>>,
}

// ServeHandle 是 Send 的，可安全存入 Mutex 跨线程共享

#[tauri::command]
async fn start_server(state: tauri::State<'_, AppState>) -> Result<String, String> {
    // serve_with_context 本身不调用 block_on（使用 std::net::TcpListener 绑定端口），
    // 可安全在 async 上下文中调用
    let config = ServeConfig {
        work_dir: "/path/to/project".into(),
        port: 3000,
        ..Default::default()
    };
    let ctx = ServeContext {
        runtime: Some(tokio::runtime::Handle::current()),
    };

    let handle = serve_with_context(config, Some(ctx))
        .map_err(|e| e.to_string())?;

    let addr = handle.address().to_string();
    *state.serve_handle.lock().unwrap() = Some(handle);
    Ok(addr)
}

#[tauri::command]
fn stop_server(state: tauri::State<'_, AppState>) {
    if let Some(mut h) = state.serve_handle.lock().unwrap().take() {
        h.shutdown(); // 干净停止，runtime 不泄漏
    }
}
```

**关键点：**
- `serve_with_context` 使用 `std::net::TcpListener` 绑定端口（纯同步），不调用 `block_on`
- 传入 `runtime: Some(Handle::current())` 时，server task 运行在 Tauri 的 runtime 上
- `ServeHandle` 内部不持有 owned runtime，shutdown 后所有资源正确释放
- 不传入 Handle（`ctx: None`）时保持 CLI 行为 — 自建 runtime 存储在 ServeHandle 中，shutdown 时 drop

### Media Sync（进度回调）

```rust
use spage_engine::media_sync::{SyncConfig, SyncContext, SyncProgress, S3Credentials, sync_media_with_context};

#[tauri::command]
async fn sync_media(app: tauri::AppHandle) -> Result<String, String> {
    let app_clone = app.clone();

    let result = tokio::task::spawn_blocking(move || {
        let config = SyncConfig {
            work_dir: "/path/to/project".into(),
            dry_run: false,
            ..Default::default()
        };
        let ctx = SyncContext {
            on_progress: Some(Box::new(move |progress: SyncProgress| {
                let _ = app_clone.emit("sync-progress", format!("{:?}", progress));
            })),
            // 显式传入凭证，避免多线程下的 env var UB
            credentials: Some(S3Credentials {
                access_key: "your-key".into(),
                secret_key: "your-secret".into(),
            }),
            // 外部取消令牌
            cancelled: None,
        };
        sync_media_with_context(config, Some(ctx))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    serde_json::to_string(&result).map_err(|e| e.to_string())
}
```

**设计说明：**
- `sync_media_with_context` 是阻塞的一次性操作（扫描文件→上传→生成缩略图→上传缩略图），内部自建 tokio runtime
- 在 Tauri async command 中必须通过 `spawn_blocking` 调用，否则会阻塞 tokio worker thread
- `SyncContext` 的核心价值：
  - `on_progress` — 让 Tauri 能实时向前端推送进度
  - `credentials` — 显式 S3 凭证，避免 `std::env::var` 在多线程下的 UB
  - `cancelled` — 外部取消令牌，GUI 可通过 `cancel_sync` 按钮触发

**`SyncProgress` 枚举变体：**

| 变体 | 含义 |
|------|------|
| `Scanning { total }` | 扫描完成，待上传文件数 |
| `Uploading { current, total, file }` | 正在上传原图 |
| `GeneratingThumbnail { current, total, file }` | 正在生成缩略图 |
| `UploadingThumbnail { current, total }` | 正在上传缩略图 |
| `Done` | 全部完成 |

### Build（进度回调 + 取消）

```rust
use std::sync::atomic::{Arc, AtomicBool};
use spage_engine::build::{BuildOptions, build_with_context};
use spage_engine::progress::{BuildContext, BuildProgressEvent};

#[tauri::command]
async fn build_blog(app: tauri::AppHandle, cancel_token: Arc<AtomicBool>) -> Result<String, String> {
    let app_clone = app.clone();

    let result = tokio::task::spawn_blocking(move || {
        let ctx = BuildContext {
            on_progress: Some(Box::new(move |evt: BuildProgressEvent| {
                let msg = format!("{:?}", evt);
                let _ = app_clone.emit("build-progress", msg);
            })),
            cancelled: Some(cancel_token),
        };
        let opts = BuildOptions {
            work_dir: "/path/to/project".into(),
            ..Default::default()
        };
        build_with_context(opts, Some(ctx))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    serde_json::to_string(&result).map_err(|e| e.to_string())
}
```

**`BuildProgressEvent` 枚举变体：**

| 变体 | 含义 |
|------|------|
| `StepStart { step }` | 构建步骤开始 |
| `StepDone { step, detail }` | 构建步骤完成（附带详情如文件数） |
| `AlbumsStart { count }` | 相册处理开始 |
| `PhotoProgress { album, current, total }` | 单张照片缩略图完成 |
| `PhotoAlbumDone { album, count, duration_ms }` | 单个相册处理完成 |

### 取消机制

Build 和 Sync 均支持通过 `Arc<AtomicBool>` 取消令牌实现协作式取消：

```rust
use std::sync::atomic::{Arc, AtomicBool, Ordering};

// 创建令牌
let cancel = Arc::new(AtomicBool::new(false));

// 传入 context
let ctx = BuildContext {
    on_progress: None,
    cancelled: Some(cancel.clone()),
};

// 在另一个线程（如 GUI 按钮回调）触发取消
cancel.store(true, Ordering::SeqCst);
```

当操作被取消时，函数返回 `Err(EngineError::Cancelled)`。调用方可据此区分「取消」和「真正的错误」来决定 UI 展示逻辑。
