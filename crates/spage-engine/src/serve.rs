//! Serve options and handle types for the development preview server.

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::error::EngineError;
use crate::mime::{has_file_extension, resolve_mime_type};
use crate::{AlbumConfig, SiteConfig};

// ── Config (serializable) ──────────────────────────────────────────

/// Development server configuration — all fields have sensible defaults.
/// This struct is serializable and suitable for passing via JSON (e.g. from NAPI).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServeConfig {
    /// Working directory (project root). Defaults to `"."`.
    pub work_dir: PathBuf,
    /// Cache directory for generated data. Defaults to `".cache"`.
    pub cache_dir: PathBuf,
    /// Explicit app shell override. When omitted, resolve `package.json.spage.core`.
    pub shell_dir: Option<PathBuf>,
    /// Package cache root. Defaults to `<workDir>/.cache/packages`.
    pub package_cache_dir: Option<PathBuf>,
    /// Port to bind the HTTP server on. Defaults to `3000`.
    pub port: u16,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            work_dir: PathBuf::from("."),
            cache_dir: PathBuf::from(".cache"),
            shell_dir: None,
            package_cache_dir: None,
            port: 3000,
        }
    }
}

/// Backward-compatible alias.
pub type ServeOptions = ServeConfig;

// ── Context (runtime, non-serializable) ────────────────────────────

/// Runtime context for the serve command.
/// Pass this when you want to reuse an existing tokio runtime (e.g. in Tauri).
pub struct ServeContext {
    /// External tokio runtime handle. When provided, the server task is spawned
    /// on this runtime instead of creating (and leaking) a new one.
    pub runtime: Option<tokio::runtime::Handle>,
}

// ── Handle ─────────────────────────────────────────────────────────

/// Handle to a running development server.
///
/// Returned by `serve()` immediately after the server starts.
/// Use this handle to query the bound address and to shut down the server.
pub struct ServeHandle {
    /// The actual address the server is bound to (includes port).
    addr: SocketAddr,
    /// Oneshot sender to signal server shutdown; consumed on first `shutdown()` call.
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Owned runtime — kept alive when we created it ourselves. Dropped on shutdown.
    _owned_rt: Option<tokio::runtime::Runtime>,
}

// Compile-time assertion: ServeHandle must be Send for cross-thread sharing (e.g. Mutex<Option<ServeHandle>>)
const _: () = {
    fn _assert_send<T: Send>() {}
    fn _check() {
        _assert_send::<ServeHandle>();
    }
};

impl ServeHandle {
    /// Returns the socket address the server is bound to.
    pub fn address(&self) -> SocketAddr {
        self.addr
    }

    /// Gracefully shuts down the server.
    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Drop owned runtime if we have one — this cleanly stops all tasks
        self._owned_rt.take();
    }
}

// ── Internal state ─────────────────────────────────────────────────

/// Shared state for the HTTP request handler.
struct ServerState {
    cache_dir: PathBuf,
    public_dir: PathBuf,
    shell_dir: PathBuf,
    work_dir: PathBuf,
    config: SiteConfig,
    album_config: Option<AlbumConfig>,
    base_path: String,
}

// ── Public API ─────────────────────────────────────────────────────

/// Parse a port string, returning a valid port in [1, 65535] or an error message.
pub fn parse_port(s: &str) -> Result<u16, String> {
    let n: u32 = s
        .parse()
        .map_err(|_| "端口必须是 1-65535 之间的整数".to_string())?;
    if n < 1 || n > 65535 {
        return Err("端口必须是 1-65535 之间的整数".to_string());
    }
    Ok(n as u16)
}

/// Start the development preview server (backward-compatible wrapper).
///
/// Creates its own tokio runtime. For Tauri integration, use [`serve_with_context`] instead.
pub fn serve(opts: ServeOptions) -> Result<ServeHandle, EngineError> {
    serve_with_context(opts, None)
}

/// Start the development preview server with optional runtime context.
///
/// When `ctx` provides a runtime handle, the server task is spawned on that runtime
/// and no runtime is leaked. When `ctx` is None, a new runtime is created and kept
/// alive inside the returned [`ServeHandle`] (dropped on shutdown).
pub fn serve_with_context(
    config: ServeConfig,
    ctx: Option<ServeContext>,
) -> Result<ServeHandle, EngineError> {
    let work_dir = &config.work_dir;
    let cache_dir = if config.cache_dir.is_relative() {
        work_dir.join(&config.cache_dir)
    } else {
        config.cache_dir.clone()
    };
    let shell_dir = crate::packages::resolve_project_shell(
        work_dir,
        config.shell_dir.as_deref(),
        config.package_cache_dir.as_deref(),
    )?;

    if !shell_dir.exists() {
        return Err(EngineError::ServeDirNotFound(shell_dir));
    }

    // Ensure cache dir exists
    fs::create_dir_all(&cache_dir).map_err(|_| EngineError::BuildStepFailed {
        step: "create cache dir".into(),
        reason: format!("cannot create {}", cache_dir.display()),
    })?;

    // Parse site config
    let posts_dir = work_dir.join("posts");
    let config_path = work_dir.join("config.json");
    let site_config: SiteConfig = if config_path.exists() {
        let config_raw = fs::read_to_string(&config_path).unwrap_or_default();
        serde_json::from_reader(json_comments::StripComments::new(config_raw.as_bytes())).map_err(
            |e| EngineError::BuildStepFailed {
                step: "parse config.json".into(),
                reason: e.to_string(),
            },
        )?
    } else {
        return Err(EngineError::ConfigNotFound(config_path));
    };

    // Parse album config
    let album_config_path = work_dir.join("album.config.json");
    let album_config: Option<AlbumConfig> = if album_config_path.exists() {
        fs::read_to_string(&album_config_path).ok().and_then(|raw| {
            serde_json::from_reader(json_comments::StripComments::new(raw.as_bytes())).ok()
        })
    } else {
        None
    };

    // Initial generation (warm-up)
    if posts_dir.exists() {
        let _ = crate::posts::generate_posts_manifest_only(&posts_dir, &cache_dir, &site_config);
    }
    if let Some(ref ac) = album_config {
        let albums_dir = work_dir.join("albums");
        if albums_dir.exists() {
            let _ = crate::albums::generate_albums_index_only(
                &albums_dir,
                &cache_dir,
                ac,
                site_config.base_path.as_deref(),
            );
        }
    }

    // Determine if we have an external runtime
    let external_handle = ctx.and_then(|c| c.runtime);

    // Bind using std::net::TcpListener (sync, works in any context including async).
    // Then convert to tokio TcpListener when spawning the server task.
    let addr: SocketAddr = ([127, 0, 0, 1], config.port).into();
    let std_listener = std::net::TcpListener::bind(addr)
        .map_err(|_| EngineError::PortInUse { port: config.port })?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| EngineError::BuildStepFailed {
            step: "set listener nonblocking".into(),
            reason: e.to_string(),
        })?;
    let bound_addr = std_listener
        .local_addr()
        .map_err(|_| EngineError::PortInUse { port: config.port })?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Either use the external runtime or create our own.
    let (owned_rt, handle) = match external_handle {
        Some(h) => (None, h),
        None => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| EngineError::BuildStepFailed {
                    step: "start runtime".into(),
                    reason: e.to_string(),
                })?;
            let h = rt.handle().clone();
            (Some(rt), h)
        }
    };

    let base_path = site_config
        .base_path
        .as_deref()
        .map(|bp| crate::shell::normalize_base_path(bp))
        .unwrap_or_default();

    let state = Arc::new(ServerState {
        cache_dir,
        public_dir: work_dir.join("public"),
        shell_dir,
        work_dir: work_dir.clone(),
        config: site_config,
        album_config,
        base_path,
    });

    // Spawn the server loop
    handle.spawn(async move {
        // Convert std listener to tokio listener inside the async context
        let listener = TcpListener::from_std(std_listener).expect("failed to convert TcpListener");
        let mut shutdown_rx = shutdown_rx;
        let mut connections = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                result = listener.accept() => {
                    if let Ok((stream, _)) = result {
                        let state = state.clone();
                        connections.spawn(async move {
                            let io = TokioIo::new(stream);
                            let svc = service_fn(move |req| {
                                let state = state.clone();
                                async move { handle_request(req, &state) }
                            });
                            let _ = hyper::server::conn::http1::Builder::new()
                                .serve_connection(io, svc)
                                .await;
                        });
                    }
                }
                _ = &mut shutdown_rx => {
                    // Abort all in-flight connections
                    connections.abort_all();
                    break;
                }
            }
        }
    });

    Ok(ServeHandle {
        addr: bound_addr,
        shutdown_tx: Some(shutdown_tx),
        _owned_rt: owned_rt,
    })
}

// ── Request handling ───────────────────────────────────────────────

/// Handle a single HTTP request by resolving the file path.
fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: &ServerState,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let raw_path = req.uri().path();

    // Strip basePath prefix if configured
    let path = if !state.base_path.is_empty() && raw_path.starts_with(&state.base_path) {
        let stripped = &raw_path[state.base_path.len()..];
        if stripped.is_empty() || stripped.starts_with('/') {
            stripped
        } else {
            raw_path
        }
    } else {
        raw_path
    };

    let rel_raw = path.trim_start_matches('/');
    let rel_decoded = percent_decode(rel_raw);
    let rel = rel_decoded.as_str();

    // Dynamic regeneration for manifest and album index on every request
    if rel == "generated/manifest.json" {
        let posts_dir = state.work_dir.join("posts");
        if posts_dir.exists() {
            let _ = crate::posts::generate_posts_manifest_only(
                &posts_dir,
                &state.cache_dir,
                &state.config,
            );
        }
    } else if rel == "generated/albums-index.json" || rel.starts_with("generated/album-") {
        if let Some(ref ac) = state.album_config {
            let albums_dir = state.work_dir.join("albums");
            if albums_dir.exists() {
                let _ = crate::albums::generate_albums_index_only(
                    &albums_dir,
                    &state.cache_dir,
                    ac,
                    state.config.base_path.as_deref(),
                );
            }
        }
    }

    // Priority 1: .cache/ generated data
    let candidate = state.cache_dir.join(rel);
    if candidate.is_file() {
        return Ok(serve_file(&candidate));
    }

    // Priority 2: work_dir files
    let candidate = state.work_dir.join(rel);
    if candidate.is_file() {
        if rel == "config.json" || rel == "album.config.json" || rel == "memo.config.json" {
            if let Ok(raw) = fs::read_to_string(&candidate) {
                use std::io::Read;
                let mut stripped = String::new();
                if json_comments::StripComments::new(raw.as_bytes())
                    .read_to_string(&mut stripped)
                    .is_ok()
                {
                    return Ok(Response::builder()
                        .header("content-type", "application/json")
                        .body(Full::new(Bytes::from(stripped)))
                        .unwrap());
                }
            }
        }
        return Ok(serve_file(&candidate));
    }

    // Priority 3: public/ static assets
    let candidate = state.public_dir.join(rel);
    if candidate.is_file() {
        return Ok(serve_file(&candidate));
    }

    // Priority 4: shell files
    let candidate = state.shell_dir.join(rel);
    if candidate.is_file() {
        if rel == "index.html" || rel.is_empty() {
            return Ok(serve_index_html(&candidate, &state.base_path));
        }
        return Ok(serve_file(&candidate));
    }

    // Priority 5: SPA fallback or 404
    if has_file_extension(path) {
        Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from("404 Not Found")))
            .unwrap())
    } else {
        let index = state.shell_dir.join("index.html");
        if index.is_file() {
            Ok(serve_index_html(&index, &state.base_path))
        } else {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Full::new(Bytes::from("404 Not Found")))
                .unwrap())
        }
    }
}

fn serve_index_html(path: &Path, base_path: &str) -> Response<Full<Bytes>> {
    match fs::read_to_string(path) {
        Ok(html) => {
            let rewritten = crate::shell::rewrite_base_path(&html, base_path);
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "text/html")
                .body(Full::new(Bytes::from(rewritten)))
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Full::new(Bytes::from("500 Internal Server Error")))
            .unwrap(),
    }
}

fn serve_file(path: &Path) -> Response<Full<Bytes>> {
    match fs::read(path) {
        Ok(content) => {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let mime = resolve_mime_type(ext);
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", mime)
                .body(Full::new(Bytes::from(content)))
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Full::new(Bytes::from("500 Internal Server Error")))
            .unwrap(),
    }
}

fn percent_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(|c| hex_val(c));
            let lo = chars.next().and_then(|c| hex_val(c));
            if let (Some(h), Some(l)) = (hi, lo) {
                bytes.push(h << 4 | l);
            } else {
                bytes.push(b'%');
            }
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| input.to_string())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
