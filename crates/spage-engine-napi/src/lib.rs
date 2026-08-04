//! NAPI-RS bindings for spage-engine.
//!
//! Each exported function accepts JSON strings for configuration and
//! path strings for directories/files, then delegates to the core
//! `spage_engine` crate.  Results are returned as JSON strings (or
//! counts) so the Node.js layer stays thin.

use std::path::Path;

use napi_derive::napi;
use spage_engine::build::{BuildOptions, BuildResult};
use spage_engine::media_sync::SyncConfig;
use spage_engine::packages::UpdateOptions;
use spage_engine::serve::ServeConfig;
use spage_engine::{AlbumConfig, PostMetadata, SiteConfig};

// ── Posts ───────────────────────────────────────────────────────────

/// Generate the posts manifest and copy Markdown files.
///
/// Returns the manifest JSON string (array of `PostMetadata`).
#[napi]
pub fn generate_posts_data(
    posts_dir: String,
    output_dir: String,
    config_json: String,
) -> napi::Result<String> {
    let config: SiteConfig = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid config JSON: {e}")))?;

    let posts = spage_engine::posts::generate_posts_data(
        Path::new(&posts_dir),
        Path::new(&output_dir),
        &config,
    )?;

    let json = serde_json::to_string_pretty(&posts)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))?;

    Ok(json)
}

/// Generate only the posts manifest without copying Markdown files.
///
/// Use this for dev mode where files are served directly from source.
/// Returns the manifest JSON string (array of `PostMetadata`).
#[napi]
pub fn generate_posts_manifest_only(
    posts_dir: String,
    output_dir: String,
    config_json: String,
) -> napi::Result<String> {
    let config: SiteConfig = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid config JSON: {e}")))?;

    let posts = spage_engine::posts::generate_posts_manifest_only(
        Path::new(&posts_dir),
        Path::new(&output_dir),
        &config,
    )?;

    let json = serde_json::to_string_pretty(&posts)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))?;

    Ok(json)
}

// ── Albums ──────────────────────────────────────────────────────────

/// Generate album index and per-album detail JSON files, including
/// thumbnail generation and EXIF extraction.
///
/// Returns the albums-index JSON string.
#[napi]
pub fn generate_albums_data(
    albums_dir: String,
    output_dir: String,
    album_config_json: String,
    site_config_json: String,
) -> napi::Result<String> {
    let album_config: AlbumConfig = serde_json::from_str(&album_config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid album config JSON: {e}")))?;

    let site_config: SiteConfig = serde_json::from_str(&site_config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid site config JSON: {e}")))?;

    let base_path = site_config.base_path.as_deref();

    let output = spage_engine::albums::generate_albums_data_with_base(
        Path::new(&albums_dir),
        Path::new(&output_dir),
        &album_config,
        base_path,
    )?;

    let json = serde_json::to_string_pretty(&output.summaries)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))?;

    Ok(json)
}

// ── SEO ─────────────────────────────────────────────────────────────

/// Generate SEO HTML pages for every post in the manifest.
///
/// Returns the number of pages generated.
#[napi]
pub fn generate_seo_pages(
    manifest_json: String,
    template_path: String,
    output_dir: String,
    config_json: String,
) -> napi::Result<u32> {
    let manifest: Vec<PostMetadata> = serde_json::from_str(&manifest_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid manifest JSON: {e}")))?;

    let config: SiteConfig = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid config JSON: {e}")))?;

    let count = spage_engine::seo::generate_seo_pages(
        &manifest,
        Path::new(&template_path),
        Path::new(&output_dir),
        &config,
    )?;

    Ok(count as u32)
}

// ── Sitemap ─────────────────────────────────────────────────────────

/// Generate `sitemap.xml`.
#[napi]
pub fn generate_sitemap(
    manifest_json: String,
    output_path: String,
    config_json: String,
) -> napi::Result<()> {
    let manifest: Vec<PostMetadata> = serde_json::from_str(&manifest_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid manifest JSON: {e}")))?;

    let config: SiteConfig = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid config JSON: {e}")))?;

    spage_engine::sitemap::generate_sitemap(&manifest, Path::new(&output_path), &config)?;

    Ok(())
}

// ── RSS ─────────────────────────────────────────────────────────────

/// Generate `rss.xml`.
#[napi]
pub fn generate_rss(
    manifest_json: String,
    output_path: String,
    config_json: String,
    posts_dir: Option<String>,
) -> napi::Result<()> {
    let manifest: Vec<PostMetadata> = serde_json::from_str(&manifest_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid manifest JSON: {e}")))?;

    let config: SiteConfig = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid config JSON: {e}")))?;

    spage_engine::rss::generate_rss(
        &manifest,
        Path::new(&output_path),
        &config,
        posts_dir.as_deref().map(Path::new),
    )?;

    Ok(())
}

// ── Robots ──────────────────────────────────────────────────────────

/// Generate `robots.txt`.
#[napi]
pub fn generate_robots(output_path: String, config_json: String) -> napi::Result<()> {
    let config: SiteConfig = serde_json::from_str(&config_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid config JSON: {e}")))?;

    spage_engine::robots::generate_robots(Path::new(&output_path), &config)?;

    Ok(())
}

// ── CLI Commands ────────────────────────────────────────────────────

/// Execute the full production build pipeline.
///
/// Accepts a JSON string of `BuildOptions`, returns a JSON string of `BuildResult`.
#[napi]
pub fn build_command(options_json: String) -> napi::Result<String> {
    let _ = dotenvy::dotenv();
    let opts: BuildOptions = serde_json::from_str(&options_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid build options: {e}")))?;

    let result: BuildResult =
        spage_engine::build::build(opts).map_err(|e| napi::Error::from_reason(e.to_string()))?;

    serde_json::to_string(&result)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))
}

/// Start the development preview server and block until Ctrl+C.
///
/// Accepts a JSON string of `ServeOptions`. Prints the server address to stdout,
/// then blocks until the process receives a termination signal.
#[napi]
pub fn serve_command(options_json: String) -> napi::Result<()> {
    let opts: ServeConfig = serde_json::from_str(&options_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid serve options: {e}")))?;

    let mut handle =
        spage_engine::serve::serve(opts).map_err(|e| napi::Error::from_reason(e.to_string()))?;

    println!("Server running at http://{}", handle.address());
    for warning in handle.warnings() {
        println!("Warning: {warning}");
    }

    // Block until Ctrl+C
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| napi::Error::from_reason(format!("Failed to create runtime: {e}")))?;

    rt.block_on(tokio::signal::ctrl_c())
        .map_err(|e| napi::Error::from_reason(format!("Signal error: {e}")))?;

    handle.shutdown();
    Ok(())
}

/// Update Spage core/plugin declarations and warm the package cache.
#[napi]
pub fn update_resources_command(options_json: String) -> napi::Result<String> {
    let opts: UpdateOptions = serde_json::from_str(&options_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid update options: {e}")))?;
    let declaration = spage_engine::packages::update_resources(opts)
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    serde_json::to_string(&declaration)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))
}

// ── Sync Media ──────────────────────────────────────────────────────

/// Sync local album media to S3-compatible storage.
///
/// Accepts a JSON string of `SyncOptions`, returns a JSON string of `SyncResult`.
/// Prints progress to stdout for CLI usage.
#[napi]
pub fn sync_media_command(options_json: String) -> napi::Result<String> {
    use spage_engine::media_sync::{SyncContext, SyncProgress};

    let _ = dotenvy::dotenv();
    let opts: SyncConfig = serde_json::from_str(&options_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid sync options: {e}")))?;

    let ctx = SyncContext {
        on_progress: Some(Box::new(|evt| match evt {
            SyncProgress::GeneratingThumbnail {
                current,
                total,
                file,
            } => {
                println!("[sync] Generating thumbnail ({current}/{total}) {file}");
            }
            _ => {}
        })),
        credentials: None,
        cancelled: None,
    };

    let result = spage_engine::media_sync::sync_media_with_context(opts, Some(ctx))
        .map_err(|e| napi::Error::from_reason(e.to_string()))?;

    serde_json::to_string(&result)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))
}

/// Sync media with a JS progress callback. Designed for Tauri/GUI integration.
/// Runs on a background thread (libuv thread pool), does NOT block the JS event loop.
///
/// The callback receives a JSON string for each progress event:
/// `{"type":"generating_thumbnail","current":1,"total":44,"file":"album/photo.jpg"}`
/// `{"type":"uploading_thumbnail","current":1,"total":44}`
/// `{"type":"uploading","current":1,"total":11,"file":"album/photo.jpg"}`
/// `{"type":"scanning","total":11}`
/// `{"type":"done"}`
///
/// Returns a Promise that resolves to the result JSON string.
#[napi]
pub fn sync_media_with_progress(
    options_json: String,
    callback: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    >,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<SyncMediaTask>> {
    let opts: SyncConfig = serde_json::from_str(&options_json)
        .map_err(|e| napi::Error::from_reason(format!("Invalid sync options: {e}")))?;

    Ok(napi::bindgen_prelude::AsyncTask::new(SyncMediaTask {
        opts,
        callback,
    }))
}

pub struct SyncMediaTask {
    opts: SyncConfig,
    callback: napi::threadsafe_function::ThreadsafeFunction<
        String,
        napi::threadsafe_function::ErrorStrategy::Fatal,
    >,
}

impl napi::Task for SyncMediaTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        use spage_engine::media_sync::{SyncContext, SyncProgress};

        let callback = self.callback.clone();
        let ctx = SyncContext {
            on_progress: Some(Box::new(move |evt| {
                let json = match &evt {
                    SyncProgress::Scanning { total } => {
                        format!(r#"{{"type":"scanning","total":{total}}}"#)
                    }
                    SyncProgress::Uploading {
                        current,
                        total,
                        file,
                    } => {
                        format!(
                            r#"{{"type":"uploading","current":{current},"total":{total},"file":"{file}"}}"#
                        )
                    }
                    SyncProgress::GeneratingThumbnail {
                        current,
                        total,
                        file,
                    } => {
                        format!(
                            r#"{{"type":"generating_thumbnail","current":{current},"total":{total},"file":"{file}"}}"#
                        )
                    }
                    SyncProgress::UploadingThumbnail { current, total } => {
                        format!(
                            r#"{{"type":"uploading_thumbnail","current":{current},"total":{total}}}"#
                        )
                    }
                    SyncProgress::Done => r#"{"type":"done"}"#.to_string(),
                };
                callback.call(
                    json,
                    napi::threadsafe_function::ThreadsafeFunctionCallMode::NonBlocking,
                );
            })),
            credentials: None,
            cancelled: None,
        };

        let result =
            spage_engine::media_sync::sync_media_with_context(self.opts.clone(), Some(ctx))
                .map_err(|e| napi::Error::from_reason(e.to_string()))?;

        serde_json::to_string(&result)
            .map_err(|e| napi::Error::from_reason(format!("Failed to serialize result: {e}")))
    }

    fn resolve(&mut self, _env: napi::Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}
