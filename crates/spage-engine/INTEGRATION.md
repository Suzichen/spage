# spage-engine — Rust 集成指南

面向在 Rust 项目（Swritor / Tauri 或任意 Rust 程序）中直接把 `spage-engine` 当 Cargo 依赖使用的集成方。Node 侧请看 `crates/spage-engine-napi/README.md`。

## 添加依赖

```toml
[dependencies]
spage-engine = { path = "../spage/crates/spage-engine" }
# 或 spage-engine = { git = "https://github.com/user/spage.git" }
```

> 不要启用 `napi` feature，它仅用于 Node.js 绑定层的错误转换。

## 三个高层入口

| 入口 | 作用 | 配置 / 结果 |
|---|---|---|
| `build::build_with_context(BuildOptions, Option<BuildContext>)` | 完整生产构建（清理 dist → 复制 shell → 文章 → 相册 → SEO/sitemap/RSS/robots） | `BuildResult` |
| `serve::serve_with_context(ServeConfig, Option<ServeContext>)` | 开发服务器，按需重新生成数据 | `ServeHandle` |
| `media_sync::sync_media_with_context(SyncConfig, Option<SyncContext>)` | 相册原图/缩略图同步到 S3 兼容存储 | `SyncResult` |

三者都有不带 `_with_context` 的简化版（`build`、`serve`、`sync_media`），行为等于传 `None`。配置结构体都实现了 `Default` + serde（camelCase），只需覆盖用到的字段：

```rust
use spage_engine::build::{build, BuildOptions};

let result = build(BuildOptions {
    work_dir: project_dir.into(),
    ..Default::default()          // output_dir = "dist", shell_dir/package_cache_dir = None
})?;
```

`BuildOptions.work_dir` 下需要有 `config.json`；`album.config.json`、`memo.config.json` 可选。

## Spage 资源解析

`build` 和 `serve` 在未提供 `shell_dir` 时读取 `package.json.spage.core`，把精确版本下载并缓存到 `<项目>/.cache/packages`，同时准备 `spage.plugins` 中已登记的资源，并把 core 的 JSON schema 镜像到 `<项目>/.cache/generated/schemas`（生成的配置文件用 `$schema` 指向这里，所以路径固定、不含版本号，也不跟随自定义 cache 目录）。该目录是镜像而非累积：每次重建，复制失败或 core 不带 schema 时整体删除，宁可让编辑器提示「无法解析」也不留下旧版本。优先级：显式 `shell_dir` > `spage.core` > 已安装的 `node_modules/@s-page/core/dist/shell`（旧项目兼容，会输出迁移 warning）。传显式 `shell_dir` 时不做 schema 镜像。

core 与 engine 必须处于同一发布线：`0.x` 要求 major/minor 相同，`1.x` 起要求 major 相同，否则报 `CoreVersionMismatch`。项目里没有版本区间字段，`spage update core` 会在当前 engine 的发布线内选最高的稳定 core（忽略 dist-tags，跳过 prerelease）。

```rust
use spage_engine::packages::{
    ensure_package, resolve_project_shell, update_resources,
    PackageResolverOptions, PackageSpec, UpdateOptions, UpdateTarget,
};

// serve/build 内部就是这一步，需要单独拿 shell 路径时可直接调用
let shell_dir = resolve_project_shell(project_dir, None, None)?;

// 单独准备任意一个包（例如插件）
let package_dir = ensure_package(
    &PackageSpec::parse("@s-page/core@0.6.10")?,
    &PackageResolverOptions {
        cache_dir: Some(project_dir.join(".cache/packages")),
        registry_url: None,      // 可指向 npm mirror
    },
)?;

// 写回 package.json.spage 并准备新缓存
let declaration = update_resources(UpdateOptions {
    work_dir: project_dir.into(),
    target: UpdateTarget::Core,  // All | Core | Plugins
    ..Default::default()
})?;
```

## 配置类型

`SiteConfig`（`config.json`）和 `AlbumConfig`（`album.config.json`）都是 `#[serde(rename_all = "camelCase")]` 的普通结构体，直接反序列化即可。字段以 `crates/spage-engine/src/lib.rs` 为准，这里不重复列举。

```rust
use spage_engine::{AlbumConfig, SiteConfig};

// 用户的配置文件允许写注释，engine 内部统一走 StripComments 解析
let raw = std::fs::read_to_string(project_dir.join("config.json"))?;
let site_config: SiteConfig =
    serde_json::from_reader(json_comments::StripComments::new(raw.as_bytes()))?;
```

> 集成方要按同样方式容忍注释，需自行加 `json_comments` 依赖；确定文件没有注释时用 `serde_json::from_str` 即可。

## 生成函数

需要比 `build` 更细的控制时，可以直接调用各阶段函数。全部返回 `Result<_, EngineError>`。

| 函数 | 参数 | 产物 |
|---|---|---|
| `posts::generate_posts_data` | `(posts_dir, output_dir, &SiteConfig)` | `Vec<PostMetadata>`（按日期降序）；写 `generated/manifest.json`，复制 `posts/*.md` |
| `posts::generate_posts_manifest_only` | 同上 | 同上但不复制 Markdown（serve 用） |
| `albums::generate_albums_data_with_base` | `(albums_dir, output_dir, &AlbumConfig, Option<base_path>)` | `AlbumsOutput`；写 `generated/albums-index.json`、`generated/album-*.json`，生成 WebP 缩略图 |
| `albums::generate_albums_index_only` | 同上 | 写索引和详情，不生成缩略图，缩略图 URL 指向原图（serve 用） |
| `seo::generate_seo_pages` | `(&[PostMetadata], template_path, output_dir, &SiteConfig)` | 页面数；写 `post/{slug}/index.html` |
| `seo::generate_album_seo_pages` | `(&AlbumConfig, template_path, output_dir, &SiteConfig)` | 页面数 |
| `sitemap::generate_sitemap` | `(&[PostMetadata], output_path, &SiteConfig)` | `sitemap.xml`（`site_url` 缺失时跳过并 warn） |
| `rss::generate_rss` | `(&[PostMetadata], output_path, &SiteConfig, Option<posts_dir>)` | `rss.xml`；传 `posts_dir` 时输出全文内容 |
| `robots::generate_robots` | `(output_path, &SiteConfig)` | `robots.txt` |

`template_path` 指向 App Shell 的 `index.html`（即 `resolve_project_shell` 返回目录下的那个，或已复制到 `dist/index.html` 的副本）。

## Tauri 集成

### 共享 runtime 启停 serve

Tauri 应用已有 tokio runtime，传入 `ServeContext { runtime }` 可避免 engine 自建 runtime。`serve_with_context` 用 `std::net::TcpListener` 绑定端口、不调用 `block_on`，可以直接在 async command 里调用；`ServeHandle` 是 `Send`，可存进 `Mutex<Option<ServeHandle>>`。

```rust
use spage_engine::serve::{serve_with_context, ServeConfig, ServeContext, ServeHandle};

#[tauri::command]
async fn start_server(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let handle = serve_with_context(
        ServeConfig { work_dir: "/path/to/project".into(), port: 3000, ..Default::default() },
        Some(ServeContext { runtime: Some(tokio::runtime::Handle::current()) }),
    )
    .map_err(|e| e.to_string())?;

    let addr = handle.address().to_string();
    *state.serve_handle.lock().unwrap() = Some(handle);
    Ok(addr)
}

#[tauri::command]
fn stop_server(state: tauri::State<'_, AppState>) {
    if let Some(mut h) = state.serve_handle.lock().unwrap().take() {
        h.shutdown();  // 不传 runtime 时，自建 runtime 也在这里 drop
    }
}
```

### build / sync 的进度与取消

`build_with_context` 和 `sync_media_with_context` 都是阻塞调用，在 Tauri async command 里必须用 `spawn_blocking`，否则会占住 worker thread。两个 context 共享同一套约定：`on_progress` 回调推送事件，`cancelled: Option<Arc<AtomicBool>>` 作为协作式取消令牌（置 `true` 后函数返回 `EngineError::Cancelled`，可据此区分「用户取消」和真错误）。

```rust
use std::sync::{atomic::AtomicBool, Arc};
use spage_engine::build::{build_with_context, BuildOptions};
use spage_engine::progress::{BuildContext, BuildProgressEvent};

let cancel = Arc::new(AtomicBool::new(false));   // 另一个线程 store(true) 即可取消

let result = tokio::task::spawn_blocking({
    let app = app.clone();
    let cancel = cancel.clone();
    move || {
        build_with_context(
            BuildOptions { work_dir: "/path/to/project".into(), ..Default::default() },
            Some(BuildContext {
                on_progress: Some(Box::new(move |evt: BuildProgressEvent| {
                    let _ = app.emit("build-progress", format!("{evt:?}"));
                })),
                cancelled: Some(cancel),
                credentials: None,   // 配了 S3 provider 且在 CI 拉缩略图时才需要
            }),
        )
    }
})
.await
.map_err(|e| e.to_string())?
.map_err(|e| e.to_string())?;
```

`SyncContext` 的字段与 `BuildContext` 一致（`on_progress` / `cancelled` / `credentials`）。

S3 凭证只有两个来源：显式传 `credentials: Some(S3Credentials { .. })`，或进程环境变量 `S3_ACCESS_KEY` / `S3_SECRET_KEY`（engine 只读 `std::env::var`）。**engine 不会加载 `.env`** —— 那是 Node CLI 入口做的事，所以直接以 Rust crate 集成时，要么自己在启动时调用 `dotenvy::dotenv()`，要么显式传凭证，否则会得到 `S3_ACCESS_KEY not set`。GUI 场景推荐显式传：凭证跟着调用走，不依赖进程全局环境。

事件变体（用于 UI 展示）：

| `BuildProgressEvent` | 含义 |
|---|---|
| `StepStart { step }` / `StepDone { step, detail }` | 构建步骤开始 / 完成 |
| `AlbumsStart { count }` | 相册处理开始 |
| `PhotoProgress { album, current, total }` | 单张照片缩略图完成 |
| `PhotoAlbumDone { album, count, duration_ms }` | 单个相册完成 |

| `SyncProgress` | 含义 |
|---|---|
| `Scanning { total }` | 扫描完成，待上传文件数 |
| `Uploading { current, total, file }` | 上传原图 |
| `GeneratingThumbnail { current, total, file }` | 生成缩略图 |
| `UploadingThumbnail { current, total }` | 上传缩略图 |
| `Done` | 全部完成 |

## 错误处理

所有函数返回 `Result<T, spage_engine::EngineError>`。

| 变体 | 触发场景 |
|------|----------|
| `DirectoryNotFound` | posts/albums 目录不存在 |
| `FrontmatterParse` / `InvalidDate` / `InvalidTimezone` | Markdown frontmatter、日期、时区解析失败 |
| `ImageDecode` | 图片解码失败 |
| `InvalidAlbumName` | 相册目录名包含非法字符 |
| `Config` / `ConfigNotFound` | 配置错误 / 配置文件不存在 |
| `BuildStepFailed` | 构建步骤执行失败 |
| `PortInUse` | 开发服务器端口被占用 |
| `ServeDirNotFound` | serve 目录不存在（需先 build） |
| `ProjectDeclarationNotFound` | 既没有 `package.json.spage`，也没有可用的 `node_modules` shell |
| `InvalidPackageSpec` | 通过 `PackageSpec::parse` 传入的声明不是「包名 + 精确 semver」，或 `spage.core` 不是 `@s-page/core` |
| `PackageNotFound` / `PackageVersionNotFound` | registry 中不存在该包 / 该版本（`update` 找不到同发布线的版本时也是后者） |
| `PackageNetwork` | registry metadata 或 tarball 下载失败 |
| `UnsafePackageArchive` | tarball 含越界路径、符号链接或其他不安全 entry |
| `InvalidPackageCache` | 下载内容不完整，或 core 缺少 `dist/shell/index.html` |
| `CoreVersionMismatch` | 声明的 core 与当前 engine 不在同一发布线 |
| `Cancelled` | 操作被取消 |
| `Io` / `Json` / `Yaml` | 文件系统、JSON、YAML 错误 |

> `package.json.spage` 里的声明格式错误会在反序列化阶段报出，因此拿到的是 `Json`（消息中带具体原因），而不是 `InvalidPackageSpec`。

## 日志

engine 通过 `log` crate 输出，**宿主不安装 sink 就什么都看不到**——包括「正在使用 legacy node_modules shell」这类迁移提示。Tauri 项目用 `tauri-plugin-log`，普通程序用 `env_logger` 即可。

## 注意事项

- 所有路径输出统一使用 `/` 分隔符，跨平台一致
- 缩略图增量生成：已存在且较新的会跳过
- `.cache/packages` 是按项目的包缓存，删掉会在下次 serve/build 重新下载；`PackageResolverOptions.cache_dir` 可换成多项目共享目录
- 底层模块（需要更细粒度时直接用）：`frontmatter`、`timezone`、`image_proc`、`exif`、`path_util`、`shell`、`mime`、`packages`
