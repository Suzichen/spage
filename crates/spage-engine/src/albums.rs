//! Album data generation.
//!
//! Produces `albums-index.json` and per-album `album-{dirname}.json`
//! files from the albums directory and configuration.
//!
//! **Requirements**: 2.6.1, 2.6.2, 2.6.3, 2.6.4, 2.6.5

use std::fs;
use std::path::Path;
use std::time::Instant;

use log::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::exif::{read_exif, ExifData};
use crate::image_proc::is_photo_file;
use crate::path_util::normalize_base_path_option;
use crate::progress::BuildProgress;
use crate::AlbumConfig;

// ── Output types ───────────────────────────────────────────────────

/// Summary for a single album (written to `albums-index.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub dirname: String,
    pub name: String,
    pub cover: Option<String>,
    pub photo_count: usize,
}

/// Detail for a single album (written to `album-{dirname}.json`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlbumDetail {
    pub dirname: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    pub photos: Vec<PhotoItem>,
}

/// A single photo within an album detail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhotoItem {
    pub filename: String,
    pub thumbnail_url: String,
    pub original_url: String,
    pub exif: ExifData,
}

/// Combined output from album generation.
pub struct AlbumsOutput {
    pub summaries: Vec<AlbumSummary>,
    pub details: Vec<AlbumDetail>,
}

// ── Validation ─────────────────────────────────────────────────────

/// Returns `true` when `s` is a valid album directory name.
///
/// Valid names consist of Unicode letters, digits, underscores, or hyphens,
/// and must not be empty or start with a dot.
pub fn is_valid_dirname(s: &str) -> bool {
    if s.is_empty() || s.starts_with('.') {
        return false;
    }
    s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

// ── BasePath helper ─────────────────────────────────────────────────

// Uses the shared `normalize_base_path_option` from `path_util`.

// ── Thumbnail mode ──────────────────────────────────────────────────

/// Controls thumbnail behavior in album generation.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ThumbnailMode {
    /// Generate thumbnail files on disk + use thumbs URL. (build)
    Generate,
    /// Skip thumbnails entirely, URL points to original image. (serve)
    SkipFallbackOriginal,
    /// Don't generate thumbnail files, but URL still references thumbs path. (sync JSON export)
    MetadataOnly,
}

// ── Public API ─────────────────────────────────────────────────────

/// Generate album data from the given albums directory and configuration.
///
/// Produces:
/// - `{output_dir}/generated/albums-index.json` — array of [`AlbumSummary`]
/// - `{output_dir}/generated/album-{dirname}.json` — [`AlbumDetail`] per album
/// - Thumbnails in `{output_dir}/albums/{dirname}/thumbs/`
///
/// If `config.enabled` is `false`, writes an empty `albums-index.json` and
/// returns an empty [`AlbumsOutput`] (requirement 2.6.5).
///
/// The optional `base_path` is prepended to all generated URLs so that
/// the blog can be deployed under a subdirectory (requirement 1.5.5).
pub fn generate_albums_data(
    albums_dir: &Path,
    output_dir: &Path,
    config: &AlbumConfig,
) -> Result<AlbumsOutput, EngineError> {
    generate_albums_data_with_base(albums_dir, output_dir, config, None)
}

/// Like [`generate_albums_data`] but accepts an explicit `base_path`.
pub fn generate_albums_data_with_base(
    albums_dir: &Path,
    output_dir: &Path,
    config: &AlbumConfig,
    base_path: Option<&str>,
) -> Result<AlbumsOutput, EngineError> {
    generate_albums_impl(albums_dir, output_dir, config, base_path, ThumbnailMode::Generate, None)
}

/// Like [`generate_albums_data_with_base`] but skips thumbnail generation.
/// Used in serve mode for fast startup — thumbnailUrl points to the original image.
pub fn generate_albums_index_only(
    albums_dir: &Path,
    output_dir: &Path,
    config: &AlbumConfig,
    base_path: Option<&str>,
) -> Result<AlbumsOutput, EngineError> {
    generate_albums_impl(albums_dir, output_dir, config, base_path, ThumbnailMode::SkipFallbackOriginal, None)
}

/// Like [`generate_albums_data_with_base`] but with progress output support.
/// Used by the build pipeline to show per-photo progress.
pub fn generate_albums_data_with_progress(
    albums_dir: &Path,
    output_dir: &Path,
    config: &AlbumConfig,
    base_path: Option<&str>,
    progress: Option<&BuildProgress>,
) -> Result<AlbumsOutput, EngineError> {
    generate_albums_impl(albums_dir, output_dir, config, base_path, ThumbnailMode::Generate, progress)
}

/// Generate only the JSON metadata (albums-index + per-album detail) with correct
/// thumbnail URLs (`/albums/{dir}/thumbs/{stem}.webp`) but without writing any
/// thumbnail files to disk. Used by sync to produce index JSON for S3 upload.
pub fn generate_albums_metadata_only(
    albums_dir: &Path,
    output_dir: &Path,
    config: &AlbumConfig,
    base_path: Option<&str>,
) -> Result<AlbumsOutput, EngineError> {
    generate_albums_impl(albums_dir, output_dir, config, base_path, ThumbnailMode::MetadataOnly, None)
}

fn generate_albums_impl(
    albums_dir: &Path,
    output_dir: &Path,
    config: &AlbumConfig,
    base_path: Option<&str>,
    thumb_mode: ThumbnailMode,
    progress: Option<&BuildProgress>,
) -> Result<AlbumsOutput, EngineError> {
    let bp = normalize_base_path_option(base_path);
    // When provider is configured, original images are served from CDN
    let origin_prefix = config
        .provider
        .as_ref()
        .map(|p| p.public_url.trim_end_matches('/').to_string())
        .unwrap_or_else(|| bp.clone());
    let generated_dir = output_dir.join("generated");
    fs::create_dir_all(&generated_dir)?;

    // Disabled → empty index (requirement 2.6.5)
    if !config.enabled {
        info!("[albums] Album module is disabled. Generating empty index.");
        let empty: Vec<AlbumSummary> = Vec::new();
        let json = serde_json::to_string_pretty(&empty)?;
        fs::write(generated_dir.join("albums-index.json"), json)?;
        return Ok(AlbumsOutput {
            summaries: Vec::new(),
            details: Vec::new(),
        });
    }

    let mut summaries = Vec::new();
    let mut details = Vec::new();

    if let Some(p) = progress {
        p.albums_start(config.albums.len());
    }

    for entry in &config.albums {
        let dirname = &entry.dir;

        // Validate dirname (requirement 2.6.4)
        if !is_valid_dirname(dirname) {
            error!(
                "[ERROR] Invalid dirname \"{}\": contains invalid characters. Skipping.",
                dirname
            );
            continue;
        }

        // Locate album source directory
        let album_src = albums_dir.join(dirname);
        if !album_src.is_dir() {
            warn!(
                "[WARN] Album directory not found: {}. Skipping.",
                album_src.display()
            );
            continue;
        }

        // Collect photo files (sorted)
        let mut photo_files: Vec<String> = fs::read_dir(&album_src)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                e.path().is_file() && is_photo_file(&name)
            })
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        photo_files.sort();

        // Generate thumbnails (skipped in serve mode)
        let public_albums_dir = output_dir.join("albums").join(dirname);
        let photos = if thumb_mode == ThumbnailMode::SkipFallbackOriginal {
            photo_files.iter().map(|filename| {
                PhotoItem {
                    filename: filename.clone(),
                    thumbnail_url: format!("{bp}/albums/{dirname}/{filename}"),
                    original_url: format!("{origin_prefix}/albums/{dirname}/{filename}"),
                    exif: read_exif(&album_src.join(filename)),
                }
            }).collect()
        } else if thumb_mode == ThumbnailMode::MetadataOnly {
            // Produce correct thumbs URLs without generating files on disk
            photo_files.iter().map(|filename| {
                let stem = Path::new(filename).file_stem().unwrap_or_default().to_string_lossy();
                let thumb_filename = format!("{stem}.webp");
                PhotoItem {
                    filename: filename.clone(),
                    thumbnail_url: format!("{bp}/albums/{dirname}/thumbs/{thumb_filename}"),
                    original_url: format!("{origin_prefix}/albums/{dirname}/{filename}"),
                    exif: read_exif(&album_src.join(filename)),
                }
            }).collect()
        } else {
            let thumbs_dir = public_albums_dir.join("thumbs");
            fs::create_dir_all(&thumbs_dir)?;

            let album_start = Instant::now();
            let total = photo_files.len();
            let mut photos = Vec::new();
            for (i, filename) in photo_files.iter().enumerate() {
                let src_path = album_src.join(filename);
                let stem = Path::new(filename)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                let thumb_filename = format!("{stem}.webp");
                let dest_path = thumbs_dir.join(&thumb_filename);

                match crate::image_proc::generate_thumbnail(&src_path, &dest_path) {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("Skipping {}: {e}", src_path.display());
                        continue;
                    }
                }

                // Check cancellation after each (potentially slow) thumbnail generation
                if let Some(p) = progress {
                    if p.is_cancelled() {
                        return Err(EngineError::Cancelled);
                    }
                    p.photo_progress(dirname, i + 1, total);
                }

                let exif = read_exif(&src_path);

                photos.push(PhotoItem {
                    filename: filename.clone(),
                    thumbnail_url: format!("{bp}/albums/{dirname}/thumbs/{thumb_filename}"),
                    original_url: format!("{origin_prefix}/albums/{dirname}/{filename}"),
                    exif,
                });
            }

            if let Some(p) = progress {
                p.photo_album_done(dirname, photos.len(), &album_start);
            }
            photos
        };

        // Build summary (requirement 2.6.3)
        let name = entry.name.clone().unwrap_or_else(|| dirname.clone());
        let cover = build_cover_url(dirname, entry.cover.as_deref(), &photo_files, &bp, thumb_mode);

        let summary = AlbumSummary {
            dirname: dirname.clone(),
            name: name.clone(),
            cover,
            photo_count: photos.len(),
        };

        let detail = AlbumDetail {
            dirname: dirname.clone(),
            name,
            desc: entry.desc.clone(),
            photos,
        };

        // Write per-album detail JSON (requirement 2.6.2)
        let detail_json = serde_json::to_string_pretty(&detail)?;
        fs::write(
            generated_dir.join(format!("album-{dirname}.json")),
            detail_json,
        )?;
        info!(
            "[albums] Generated album-{}.json ({} photos)",
            dirname,
            detail.photos.len()
        );

        summaries.push(summary);
        details.push(detail);
    }

    // Write albums-index.json (requirement 2.6.1)
    let index_json = serde_json::to_string_pretty(&summaries)?;
    fs::write(generated_dir.join("albums-index.json"), index_json)?;
    info!(
        "[albums] Generated albums-index.json ({} albums)",
        summaries.len()
    );

    Ok(AlbumsOutput { summaries, details })
}

/// Determine the cover thumbnail URL for an album.
///
/// If the configured cover file exists in the photo list, use its thumbnail.
/// Otherwise fall back to the first photo's thumbnail. Returns `None` if
/// the album has no photos.
fn build_cover_url(
    dirname: &str,
    configured_cover: Option<&str>,
    photo_files: &[String],
    base_path: &str,
    thumb_mode: ThumbnailMode,
) -> Option<String> {
    let use_thumbs = thumb_mode != ThumbnailMode::SkipFallbackOriginal;

    // Check configured cover
    if let Some(cover_file) = configured_cover {
        if photo_files.iter().any(|f| f == cover_file) {
            if !use_thumbs {
                return Some(format!("{base_path}/albums/{dirname}/{cover_file}"));
            }
            let stem = Path::new(cover_file)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            return Some(format!("{base_path}/albums/{dirname}/thumbs/{stem}.webp"));
        }
    }

    // Fall back to first photo
    photo_files.first().map(|first| {
        if !use_thumbs {
            format!("{base_path}/albums/{dirname}/{first}")
        } else {
            let stem = Path::new(first)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            format!("{base_path}/albums/{dirname}/thumbs/{stem}.webp")
        }
    })
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_dirname_accepts_valid_names() {
        assert!(is_valid_dirname("travel"));
        assert!(is_valid_dirname("my-album"));
        assert!(is_valid_dirname("album_2024"));
        assert!(is_valid_dirname("test123"));
    }

    #[test]
    fn is_valid_dirname_rejects_invalid_names() {
        assert!(!is_valid_dirname(""));
        assert!(!is_valid_dirname(".hidden"));
        assert!(!is_valid_dirname("has space"));
        assert!(!is_valid_dirname("has/slash"));
        assert!(!is_valid_dirname("has.dot"));
    }

    #[test]
    fn build_cover_url_configured_cover_exists() {
        let photos = vec!["a.jpg".to_string(), "b.jpg".to_string()];
        let result = build_cover_url("travel", Some("b.jpg"), &photos, "", ThumbnailMode::Generate);
        assert_eq!(result, Some("/albums/travel/thumbs/b.webp".to_string()));
    }

    #[test]
    fn build_cover_url_configured_cover_missing_falls_back() {
        let photos = vec!["a.jpg".to_string(), "b.jpg".to_string()];
        let result = build_cover_url("travel", Some("nonexistent.jpg"), &photos, "", ThumbnailMode::Generate);
        assert_eq!(result, Some("/albums/travel/thumbs/a.webp".to_string()));
    }

    #[test]
    fn build_cover_url_no_config_uses_first() {
        let photos = vec!["first.jpg".to_string()];
        let result = build_cover_url("travel", None, &photos, "", ThumbnailMode::Generate);
        assert_eq!(result, Some("/albums/travel/thumbs/first.webp".to_string()));
    }

    #[test]
    fn build_cover_url_empty_album() {
        let photos: Vec<String> = Vec::new();
        let result = build_cover_url("travel", None, &photos, "", ThumbnailMode::Generate);
        assert_eq!(result, None);
    }

    #[test]
    fn album_summary_serializes_to_camel_case() {
        let summary = AlbumSummary {
            dirname: "test".into(),
            name: "Test".into(),
            cover: Some("/albums/test/thumbs/a.webp".into()),
            photo_count: 5,
        };
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["dirname"], "test");
        assert_eq!(json["name"], "Test");
        assert_eq!(json["cover"], "/albums/test/thumbs/a.webp");
        assert_eq!(json["photoCount"], 5);
    }

    #[test]
    fn album_summary_null_cover_serializes() {
        let summary = AlbumSummary {
            dirname: "empty".into(),
            name: "Empty".into(),
            cover: None,
            photo_count: 0,
        };
        let json = serde_json::to_value(&summary).unwrap();
        assert!(json["cover"].is_null());
    }

    #[test]
    fn photo_item_serializes_to_camel_case() {
        let item = PhotoItem {
            filename: "photo.jpg".into(),
            thumbnail_url: "/albums/test/thumbs/photo.webp".into(),
            original_url: "/albums/test/photo.jpg".into(),
            exif: ExifData::empty(),
        };
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["filename"], "photo.jpg");
        assert_eq!(json["thumbnailUrl"], "/albums/test/thumbs/photo.webp");
        assert_eq!(json["originalUrl"], "/albums/test/photo.jpg");
        assert!(json["exif"].is_object());
    }

    #[test]
    fn disabled_config_produces_empty_output() {
        let tmp = tempfile::TempDir::new().unwrap();
        let albums_dir = tmp.path().join("albums");
        fs::create_dir_all(&albums_dir).unwrap();

        let config = AlbumConfig {
            enabled: false,
            albums: vec![],
            provider: None,
        };

        let result = generate_albums_data(&albums_dir, tmp.path(), &config).unwrap();
        assert!(result.summaries.is_empty());
        assert!(result.details.is_empty());

        let index_path = tmp.path().join("generated/albums-index.json");
        assert!(index_path.exists());
        let content: Vec<serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(index_path).unwrap()).unwrap();
        assert!(content.is_empty());
    }

    #[test]
    fn invalid_dirname_is_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let albums_dir = tmp.path().join("albums");
        fs::create_dir_all(&albums_dir).unwrap();

        let config = AlbumConfig {
            enabled: true,
            albums: vec![crate::AlbumEntry {
                dir: ".hidden".into(),
                name: None,
                desc: None,
                cover: None,
            }],
            provider: None,
        };

        let result = generate_albums_data(&albums_dir, tmp.path(), &config).unwrap();
        assert!(result.summaries.is_empty());
    }

    #[test]
    fn missing_album_dir_is_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        let albums_dir = tmp.path().join("albums");
        fs::create_dir_all(&albums_dir).unwrap();

        let config = AlbumConfig {
            enabled: true,
            albums: vec![crate::AlbumEntry {
                dir: "nonexistent".into(),
                name: None,
                desc: None,
                cover: None,
            }],
            provider: None,
        };

        let result = generate_albums_data(&albums_dir, tmp.path(), &config).unwrap();
        assert!(result.summaries.is_empty());
    }

    #[test]
    fn preserves_multiline_album_description() {
        let tmp = tempfile::TempDir::new().unwrap();
        let albums_dir = tmp.path().join("albums");
        fs::create_dir_all(albums_dir.join("notes")).unwrap();
        let config = AlbumConfig {
            enabled: true,
            albums: vec![crate::AlbumEntry {
                dir: "notes".into(),
                name: None,
                desc: Some("First line\nSecond line".into()),
                cover: None,
            }],
            provider: None,
        };

        let result = generate_albums_data(&albums_dir, tmp.path(), &config).unwrap();
        assert_eq!(
            result.details[0].desc.as_deref(),
            Some("First line\nSecond line")
        );
        let json = fs::read_to_string(tmp.path().join("generated/album-notes.json")).unwrap();
        assert!(json.contains("\"desc\": \"First line\\nSecond line\""));
    }

    // ── basePath tests ─────────────────────────────────────────────

    #[test]
    fn normalize_base_path_for_albums_none_returns_empty() {
        use crate::path_util::normalize_base_path_option;
        assert_eq!(normalize_base_path_option(None), "");
    }

    #[test]
    fn normalize_base_path_for_albums_root_returns_empty() {
        use crate::path_util::normalize_base_path_option;
        assert_eq!(normalize_base_path_option(Some("/")), "");
    }

    #[test]
    fn normalize_base_path_for_albums_subdir() {
        use crate::path_util::normalize_base_path_option;
        assert_eq!(normalize_base_path_option(Some("/blog")), "/blog");
    }

    #[test]
    fn normalize_base_path_for_albums_trailing_slash() {
        use crate::path_util::normalize_base_path_option;
        assert_eq!(normalize_base_path_option(Some("/blog/")), "/blog");
    }

    #[test]
    fn build_cover_url_with_base_path() {
        let photos = vec!["a.jpg".to_string()];
        let result = build_cover_url("travel", None, &photos, "/blog", ThumbnailMode::Generate);
        assert_eq!(
            result,
            Some("/blog/albums/travel/thumbs/a.webp".to_string())
        );
    }

    #[test]
    fn photo_urls_include_base_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let albums_dir = tmp.path().join("albums");
        let album_dir = albums_dir.join("test");
        fs::create_dir_all(&album_dir).unwrap();

        // Create a minimal JPEG
        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(2, 2, |_, _| Rgb([128, 64, 32]));
        img.save(album_dir.join("photo.jpg")).unwrap();

        let config = AlbumConfig {
            enabled: true,
            albums: vec![crate::AlbumEntry {
                dir: "test".into(),
                name: Some("Test".into()),
                desc: None,
                cover: None,
            }],
            provider: None,
        };

        let result =
            generate_albums_data_with_base(&albums_dir, tmp.path(), &config, Some("/blog"))
                .unwrap();

        assert_eq!(result.summaries.len(), 1);
        let summary = &result.summaries[0];
        assert_eq!(
            summary.cover,
            Some("/blog/albums/test/thumbs/photo.webp".to_string())
        );

        let detail = &result.details[0];
        assert_eq!(
            detail.photos[0].thumbnail_url,
            "/blog/albums/test/thumbs/photo.webp"
        );
        assert_eq!(
            detail.photos[0].original_url,
            "/blog/albums/test/photo.jpg"
        );
    }

    #[test]
    fn no_base_path_keeps_root_urls() {
        let tmp = tempfile::TempDir::new().unwrap();
        let albums_dir = tmp.path().join("albums");
        let album_dir = albums_dir.join("test");
        fs::create_dir_all(&album_dir).unwrap();

        use image::{ImageBuffer, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(2, 2, |_, _| Rgb([128, 64, 32]));
        img.save(album_dir.join("photo.jpg")).unwrap();

        let config = AlbumConfig {
            enabled: true,
            albums: vec![crate::AlbumEntry {
                dir: "test".into(),
                name: Some("Test".into()),
                desc: None,
                cover: None,
            }],
            provider: None,
        };

        // Using the original function (no base_path)
        let result = generate_albums_data(&albums_dir, tmp.path(), &config).unwrap();

        let detail = &result.details[0];
        assert_eq!(
            detail.photos[0].thumbnail_url,
            "/albums/test/thumbs/photo.webp"
        );
        assert_eq!(detail.photos[0].original_url, "/albums/test/photo.jpg");
    }
}
