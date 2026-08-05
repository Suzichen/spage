//! Production build pipeline for spage.
//!
//! The [`build`] function orchestrates the full production build:
//! clean dist → copy shell → generate posts → generate albums →
//! generate SEO/sitemap/rss/robots → copy static assets.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::progress::{BuildContext, BuildProgress};
use crate::{AlbumConfig, SiteConfig};

/// Build options — all fields have sensible defaults for zero-config usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildOptions {
    /// Working directory (project root). Defaults to `"."`.
    pub work_dir: PathBuf,
    /// Output directory for production artifacts. Defaults to `"dist"`.
    pub output_dir: PathBuf,
    /// Explicit app shell override. When omitted, resolve `package.json.spage.core`.
    pub shell_dir: Option<PathBuf>,
    /// Package cache root. Defaults to `<workDir>/.cache/packages`.
    pub package_cache_dir: Option<PathBuf>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            work_dir: PathBuf::from("."),
            output_dir: PathBuf::from("dist"),
            shell_dir: None,
            package_cache_dir: None,
        }
    }
}

/// Result returned by a successful production build.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildResult {
    /// Number of posts processed.
    pub posts_count: u32,
    /// Number of albums processed.
    pub albums_count: u32,
    /// Number of SEO pages generated.
    pub seo_pages_count: u32,
    /// Number of static files copied.
    pub static_files_count: u32,
    /// Number of shell files copied.
    pub shell_files_count: u32,
    /// Total build duration in milliseconds.
    pub duration_ms: u64,
}

const EXCLUDE: &[&str] = &[".DS_Store", "Thumbs.db", ".gitkeep", ".git"];

/// Execute the full production build pipeline.
///
/// # Errors
///
/// Returns [`EngineError::ConfigNotFound`] if `config.json` is missing.
/// Returns [`EngineError::BuildStepFailed`] if any step fails.
pub fn build(opts: BuildOptions) -> Result<BuildResult, EngineError> {
    build_with_context(opts, None)
}

/// Execute the full production build pipeline with optional progress/cancellation context.
pub fn build_with_context(opts: BuildOptions, ctx: Option<BuildContext>) -> Result<BuildResult, EngineError> {
    let start = Instant::now();

    let (progress, cancelled, credentials) = match ctx {
        Some(c) => {
            let p = match c.on_progress {
                Some(cb) => BuildProgress::with_callback(cb),
                None => BuildProgress::new(),
            };
            let cancelled = c.cancelled;
            let p = if let Some(ref token) = cancelled {
                p.with_cancelled(token.clone())
            } else {
                p
            };
            (p, cancelled, c.credentials)
        }
        None => (BuildProgress::new(), None, None),
    };

    let check_cancelled = || -> Result<(), EngineError> {
        if cancelled.as_ref().map_or(false, |c| c.load(std::sync::atomic::Ordering::SeqCst)) {
            return Err(EngineError::Cancelled);
        }
        Ok(())
    };

    let work_dir = &opts.work_dir;
    let output_dir = if opts.output_dir.is_relative() {
        work_dir.join(&opts.output_dir)
    } else {
        opts.output_dir.clone()
    };
    // Read configs
    let config_path = work_dir.join("config.json");
    if !config_path.exists() {
        return Err(EngineError::ConfigNotFound(config_path));
    }
    let config_raw = fs::read_to_string(&config_path).map_err(|e| EngineError::BuildStepFailed {
        step: "read config.json".into(),
        reason: e.to_string(),
    })?;
    let config: SiteConfig = serde_json::from_reader(
        json_comments::StripComments::new(config_raw.as_bytes()),
    )
    .map_err(|e| EngineError::BuildStepFailed {
        step: "parse config.json".into(),
        reason: e.to_string(),
    })?;

    let album_config_path = work_dir.join("album.config.json");
    let album_config: AlbumConfig = if album_config_path.exists() {
        let raw = fs::read_to_string(&album_config_path).map_err(|e| {
            EngineError::BuildStepFailed {
                step: "read album.config.json".into(),
                reason: e.to_string(),
            }
        })?;
        serde_json::from_reader(json_comments::StripComments::new(raw.as_bytes()))
            .map_err(|e| EngineError::BuildStepFailed {
            step: "parse album.config.json".into(),
            reason: e.to_string(),
        })?
    } else {
        AlbumConfig { enabled: false, albums: vec![], provider: None }
    };

    let shell_dir = crate::packages::resolve_project_shell(
        work_dir,
        opts.shell_dir.as_deref(),
        opts.package_cache_dir.as_deref(),
    )?;

    // Step 1: Clean dist
    check_cancelled()?;
    progress.step_start("Clean dist");
    if output_dir.exists() {
        fs::remove_dir_all(&output_dir).map_err(|e| EngineError::BuildStepFailed {
            step: "clean dist".into(),
            reason: e.to_string(),
        })?;
    }
    fs::create_dir_all(&output_dir).map_err(|e| EngineError::BuildStepFailed {
        step: "clean dist".into(),
        reason: e.to_string(),
    })?;

    // Step 2: Copy app shell (with basePath rewrite)
    progress.step_done("Clean dist", "");
    check_cancelled()?;
    progress.step_start("Copy shell");
    if !shell_dir.exists() {
        return Err(EngineError::BuildStepFailed {
            step: "copy shell".into(),
            reason: format!("App shell directory not found: {}", shell_dir.display()),
        });
    }
    let shell_files_count = copy_dir(&shell_dir, &output_dir)?;

    // Rewrite basePath in index.html
    let index_html_path = output_dir.join("index.html");
    if index_html_path.exists() {
        let html = fs::read_to_string(&index_html_path).map_err(|e| {
            EngineError::BuildStepFailed {
                step: "copy shell".into(),
                reason: e.to_string(),
            }
        })?;
        let base_path = config.base_path.as_deref().unwrap_or("/");
        let rewritten = crate::shell::rewrite_base_path(&html, base_path);
        fs::write(&index_html_path, rewritten).map_err(|e| EngineError::BuildStepFailed {
            step: "copy shell".into(),
            reason: e.to_string(),
        })?;
    }

    // Step 3: Generate posts data
    progress.step_done("Copy shell", &format!("{shell_files_count} files"));
    check_cancelled()?;
    progress.step_start("Generate posts");
    let posts_dir = work_dir.join("posts");
    let manifest = if posts_dir.exists() {
        crate::posts::generate_posts_data(&posts_dir, &output_dir, &config).map_err(|e| {
            EngineError::BuildStepFailed {
                step: "generate posts".into(),
                reason: e.to_string(),
            }
        })?
    } else {
        vec![]
    };
    let posts_count = manifest.len() as u32;

    // Step 4: Generate albums data
    progress.step_done("Generate posts", &format!("{posts_count} posts"));
    check_cancelled()?;
    progress.step_start("Generate albums");
    let albums_dir = work_dir.join("albums");

    // Warn if albums directory exceeds 500MB and no provider is configured
    if albums_dir.is_dir() && album_config.provider.is_none() {
        let size = crate::media_sync::calculate_dir_size(&albums_dir);
        let threshold = 500 * 1024 * 1024;
        if size > threshold {
            let size_mb = size as f64 / (1024.0 * 1024.0);
            log::warn!(
                "⚠ albums/ total size {size_mb:.1}MB (exceeds 500MB). Consider configuring a provider in album.config.json for external hosting"
            );
        }
    }

    // Provider mode without local albums: pull thumbs + JSON from S3
    let albums_count = if album_config.provider.is_some() && !albums_dir.is_dir() {
        crate::media_sync::pull_build_assets(
            album_config.provider.as_ref().unwrap(),
            &album_config,
            &output_dir,
            credentials.as_ref(),
        )
        .map_err(|e| EngineError::BuildStepFailed {
            step: "pull albums from S3".into(),
            reason: e.to_string(),
        })?
    } else {
        let albums_output = crate::albums::generate_albums_data_with_progress(
            &albums_dir,
            &output_dir,
            &album_config,
            config.base_path.as_deref(),
            Some(&progress),
        )
        .map_err(|e| EngineError::BuildStepFailed {
            step: "generate albums".into(),
            reason: e.to_string(),
        })?;
        albums_output.summaries.len() as u32
    };

    // Step 5: Generate SEO + sitemap + rss + robots
    progress.step_done("Generate albums", &format!("{albums_count} albums"));
    check_cancelled()?;
    progress.step_start("Generate SEO");
    let template_path = shell_dir.join("index.html");
    let seo_pages_count = if template_path.exists() {
        let post_pages =
            crate::seo::generate_seo_pages(&manifest, &template_path, &output_dir, &config)
                .map_err(|e| EngineError::BuildStepFailed {
                    step: "generate SEO".into(),
                    reason: e.to_string(),
                })?;
        let album_pages = crate::seo::generate_album_seo_pages(
            &album_config,
            &template_path,
            &output_dir,
            &config,
        )
        .map_err(|e| EngineError::BuildStepFailed {
            step: "generate album SEO".into(),
            reason: e.to_string(),
        })?;
        (post_pages + album_pages) as u32
    } else {
        0
    };

    crate::seo::generate_homepage_seo(&output_dir, &config, &manifest).map_err(|e| {
        EngineError::BuildStepFailed {
            step: "generate homepage SEO".into(),
            reason: e.to_string(),
        }
    })?;

    crate::sitemap::generate_sitemap_with_albums(
        &manifest,
        Some(&album_config),
        &output_dir.join("sitemap.xml"),
        &config,
    )
    .map_err(|e| EngineError::BuildStepFailed {
        step: "generate sitemap".into(),
        reason: e.to_string(),
    })?;

    crate::rss::generate_rss(&manifest, &output_dir.join("rss.xml"), &config, Some(&posts_dir)).map_err(|e| {
        EngineError::BuildStepFailed {
            step: "generate rss".into(),
            reason: e.to_string(),
        }
    })?;

    crate::robots::generate_robots(&output_dir.join("robots.txt"), &config).map_err(|e| {
        EngineError::BuildStepFailed {
            step: "generate robots".into(),
            reason: e.to_string(),
        }
    })?;

    // Step 6: Copy static assets
    progress.step_done("Generate SEO", &format!("{seo_pages_count} pages"));
    check_cancelled()?;
    progress.step_start("Copy static");
    let mut static_files_count: u32 = 0;

    // Copy albums/ originals (skip when provider is configured — originals served from CDN)
    if albums_dir.exists() && album_config.provider.is_none() {
        static_files_count += copy_dir(&albums_dir, &output_dir.join("albums"))?;
    }

    // Copy public/ root-level files only
    let public_dir = work_dir.join("public");
    if public_dir.exists() {
        for entry in fs::read_dir(&public_dir).map_err(|e| EngineError::BuildStepFailed {
            step: "copy static".into(),
            reason: e.to_string(),
        })? {
            let entry = entry.map_err(|e| EngineError::BuildStepFailed {
                step: "copy static".into(),
                reason: e.to_string(),
            })?;
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let name = entry.file_name();
                if !EXCLUDE.contains(&name.to_string_lossy().as_ref()) {
                    fs::copy(entry.path(), output_dir.join(&name)).map_err(|e| {
                        EngineError::BuildStepFailed {
                            step: "copy static".into(),
                            reason: e.to_string(),
                        }
                    })?;
                    static_files_count += 1;
                }
            }
        }
    }

    // Copy config files (strip JSONC comments for browser compatibility)
    for f in &["config.json", "album.config.json", "memo.config.json"] {
        let src = work_dir.join(f);
        if src.exists() {
            let raw = fs::read_to_string(&src).map_err(|e| EngineError::BuildStepFailed {
                step: "copy static".into(),
                reason: e.to_string(),
            })?;
            use std::io::Read;
            let mut clean = String::new();
            json_comments::StripComments::new(raw.as_bytes())
                .read_to_string(&mut clean)
                .map_err(|e| EngineError::BuildStepFailed {
                    step: "copy static".into(),
                    reason: e.to_string(),
                })?;
            fs::write(output_dir.join(f), &clean).map_err(|e| EngineError::BuildStepFailed {
                step: "copy static".into(),
                reason: e.to_string(),
            })?;
            static_files_count += 1;
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;
    progress.step_done("Copy static", &format!("{static_files_count} files"));

    Ok(BuildResult {
        posts_count,
        albums_count,
        seo_pages_count,
        static_files_count,
        shell_files_count: shell_files_count as u32,
        duration_ms,
    })
}

/// Recursively copy a directory, returning the number of files copied.
fn copy_dir(src: &Path, dest: &Path) -> Result<u32, EngineError> {
    let mut count = 0u32;
    if !dest.exists() {
        fs::create_dir_all(dest).map_err(|e| EngineError::BuildStepFailed {
            step: "copy dir".into(),
            reason: e.to_string(),
        })?;
    }
    for entry in fs::read_dir(src).map_err(|e| EngineError::BuildStepFailed {
        step: "copy dir".into(),
        reason: e.to_string(),
    })? {
        let entry = entry.map_err(|e| EngineError::BuildStepFailed {
            step: "copy dir".into(),
            reason: e.to_string(),
        })?;
        let name = entry.file_name();
        if EXCLUDE.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        let src_path = entry.path();
        let dest_path = dest.join(&name);
        if src_path.is_dir() {
            count += copy_dir(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path).map_err(|e| EngineError::BuildStepFailed {
                step: "copy dir".into(),
                reason: e.to_string(),
            })?;
            count += 1;
        }
    }
    Ok(count)
}


#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: engine-cli-commands, Property 2: Build error propagation
    proptest! {
        #[test]
        fn prop_build_step_failed_contains_step_and_reason(
            step in "[a-z ]{1,20}",
            reason in ".{1,50}",
        ) {
            let err = EngineError::BuildStepFailed {
                step: step.clone(),
                reason: reason.clone(),
            };
            let msg = err.to_string();
            prop_assert!(msg.contains(&step), "error message missing step name: {msg}");
            prop_assert!(msg.contains(&reason), "error message missing reason: {msg}");
        }
    }

    #[test]
    fn build_returns_config_not_found_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let opts = BuildOptions {
            work_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let err = build(opts).unwrap_err();
        assert!(matches!(err, EngineError::ConfigNotFound(_)));
    }

    // Feature: engine-cli-commands, Property 3: Non-legal JSON config error reporting
    proptest! {
        #[test]
        fn prop_invalid_json_config_reports_filename_and_reason(
            garbage in "[^{}\\[\\]\"]{1,30}",
        ) {
            let tmp = tempfile::tempdir().unwrap();
            // Write invalid JSON to config.json
            std::fs::write(tmp.path().join("config.json"), &garbage).unwrap();
            let opts = BuildOptions {
                work_dir: tmp.path().to_path_buf(),
                ..Default::default()
            };
            let err = build(opts).unwrap_err();
            let msg = err.to_string();
            prop_assert!(msg.contains("config.json"), "error missing filename: {msg}");
            // Should contain some parse error description
            prop_assert!(msg.len() > "config.json".len(), "error too short: {msg}");
        }
    }
}
