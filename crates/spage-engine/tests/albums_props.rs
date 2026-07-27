//! Property-based tests for album data generation (Property 7).
//!
//! **Property 7 – Album Data Completeness**
//!
//! *For any* set of valid album directories, the generated
//! `albums-index.json` SHALL contain exactly one summary entry per
//! album with all required fields (`dirname`, `name`, `cover`,
//! `photoCount`), and each album SHALL have a corresponding
//! `album-{dirname}.json` detail file.
//!
//! **Validates: Requirements 2.6.1, 2.6.2, 2.6.3**

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use proptest::prelude::*;
use tempfile::TempDir;

use spage_engine::albums::{generate_albums_data, AlbumDetail, AlbumSummary};
use spage_engine::{AlbumConfig, AlbumEntry};

// ── Strategies ─────────────────────────────────────────────────────

/// Generate a valid album dirname (alphanumeric + hyphens, non-empty,
/// must not start with a dot).
fn dirname_strategy() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9\\-]{0,12}[a-z0-9]"
}

/// Generate an optional album display name.
fn album_name_strategy() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        "[A-Za-z ]{2,20}".prop_map(Some),
    ]
}

/// A single generated album descriptor.
#[derive(Debug, Clone)]
struct GenAlbum {
    dirname: String,
    name: Option<String>,
    /// Number of photo files to create in this album directory.
    photo_count: usize,
}

/// Strategy for a single album with 0-5 photos.
fn album_strategy() -> impl Strategy<Value = GenAlbum> {
    (dirname_strategy(), album_name_strategy(), 0..=5usize).prop_map(
        |(dirname, name, photo_count)| GenAlbum {
            dirname,
            name,
            photo_count,
        },
    )
}

/// Strategy for a set of 1-5 albums with unique dirnames.
fn albums_strategy() -> impl Strategy<Value = Vec<GenAlbum>> {
    prop::collection::vec(album_strategy(), 1..=5).prop_map(|mut albums| {
        let mut seen = HashSet::new();
        for (i, a) in albums.iter_mut().enumerate() {
            if !seen.insert(a.dirname.clone()) {
                a.dirname = format!("{}-{}", a.dirname, i);
            }
        }
        albums
    })
}

// ── Helpers ────────────────────────────────────────────────────────

/// Create a minimal valid JPEG file in memory.
///
/// This produces a tiny 2×2 JPEG that the `image` crate can decode,
/// which is enough for `generate_albums_data` to process thumbnails.
fn create_test_jpeg(path: &Path) {
    use image::{ImageBuffer, Rgb};
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(2, 2, |_, _| Rgb([128, 64, 32]));
    img.save(path).expect("failed to write test JPEG");
}

/// Write album directories with photo files to a temp directory and
/// build the corresponding `AlbumConfig`.
fn write_albums(albums: &[GenAlbum]) -> (TempDir, AlbumConfig) {
    let dir = TempDir::new().unwrap();
    let albums_root = dir.path().join("albums");
    fs::create_dir_all(&albums_root).unwrap();

    let mut entries = Vec::new();

    for album in albums {
        let album_dir = albums_root.join(&album.dirname);
        fs::create_dir_all(&album_dir).unwrap();

        for i in 0..album.photo_count {
            let filename = format!("photo{:03}.jpg", i);
            create_test_jpeg(&album_dir.join(&filename));
        }

        entries.push(AlbumEntry {
            dir: album.dirname.clone(),
            name: album.name.clone(),
            desc: None,
            cover: None,
        });
    }

    let config = AlbumConfig {
        enabled: true,
        albums: entries,
        provider: None,
    };

    (dir, config)
}

// ── Property Tests ─────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    // ── P7.1: One summary entry per album ──────────────────────────
    //
    // The albums-index must contain exactly one summary per configured
    // album directory.
    #[test]
    fn one_summary_per_album(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        prop_assert_eq!(
            result.summaries.len(),
            albums.len(),
            "summary count ({}) != album count ({})",
            result.summaries.len(),
            albums.len()
        );

        let summary_dirnames: HashSet<&str> =
            result.summaries.iter().map(|s| s.dirname.as_str()).collect();
        for a in &albums {
            prop_assert!(
                summary_dirnames.contains(a.dirname.as_str()),
                "dirname {:?} missing from summaries",
                a.dirname
            );
        }
    }

    // ── P7.2: All required summary fields present ──────────────────
    //
    // Every summary entry must have dirname, name, cover, and
    // photoCount fields in the serialized JSON.
    #[test]
    fn summary_has_all_required_fields(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        for summary in &result.summaries {
            let json = serde_json::to_value(summary).unwrap();

            prop_assert!(json.get("dirname").is_some(), "dirname field missing");
            prop_assert!(json.get("name").is_some(), "name field missing");
            // cover may be null but the key must exist
            prop_assert!(json.get("cover").is_some(), "cover field missing");
            prop_assert!(json.get("photoCount").is_some(), "photoCount field missing");

            // dirname must be non-empty
            prop_assert!(
                !summary.dirname.is_empty(),
                "dirname must not be empty"
            );
            // name must be non-empty (defaults to dirname if not configured)
            prop_assert!(
                !summary.name.is_empty(),
                "name must not be empty for dirname={}",
                summary.dirname
            );
        }
    }

    // ── P7.3: Each album has a corresponding detail file ───────────
    //
    // For every album in the summaries, a matching album-{dirname}.json
    // detail file must be written to disk.
    #[test]
    fn each_album_has_detail_file(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        for summary in &result.summaries {
            let detail_path = out
                .path()
                .join(format!("generated/album-{}.json", summary.dirname));
            prop_assert!(
                detail_path.exists(),
                "detail file for album {:?} must exist at {}",
                summary.dirname,
                detail_path.display()
            );

            // Parse and verify structure
            let raw = fs::read_to_string(&detail_path).unwrap();
            let detail: AlbumDetail = serde_json::from_str(&raw).unwrap();
            prop_assert_eq!(
                &detail.dirname,
                &summary.dirname,
                "detail dirname must match summary"
            );
            prop_assert_eq!(
                &detail.name,
                &summary.name,
                "detail name must match summary"
            );
        }
    }

    // ── P7.4: photoCount matches actual photo count in detail ──────
    //
    // The summary's photoCount must equal the number of photos in the
    // corresponding detail file.
    #[test]
    fn photo_count_matches_detail(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        for summary in &result.summaries {
            let detail = result
                .details
                .iter()
                .find(|d| d.dirname == summary.dirname)
                .expect("detail must exist for summary");

            prop_assert_eq!(
                summary.photo_count,
                detail.photos.len(),
                "photoCount ({}) != detail photos len ({}) for album {}",
                summary.photo_count,
                detail.photos.len(),
                summary.dirname
            );
        }
    }

    // ── P7.5: albums-index.json written to disk ────────────────────
    //
    // The on-disk albums-index.json must be valid JSON containing the
    // same number of entries as the returned summaries.
    #[test]
    fn albums_index_written_to_disk(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        let index_path = out.path().join("generated/albums-index.json");
        prop_assert!(index_path.exists(), "albums-index.json must be written");

        let raw = fs::read_to_string(&index_path).unwrap();
        let parsed: Vec<AlbumSummary> = serde_json::from_str(&raw).unwrap();

        prop_assert_eq!(
            parsed.len(),
            result.summaries.len(),
            "on-disk index count must match returned summaries"
        );
    }

    // ── P7.6: Name defaults to dirname when not configured ─────────
    //
    // When no display name is configured, the summary name must equal
    // the dirname.
    #[test]
    fn name_defaults_to_dirname(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        for (gen_album, summary) in albums.iter().zip(result.summaries.iter()) {
            if gen_album.name.is_none() {
                prop_assert_eq!(
                    &summary.name,
                    &gen_album.dirname,
                    "name should default to dirname for album {}",
                    gen_album.dirname
                );
            } else {
                prop_assert_eq!(
                    &summary.name,
                    gen_album.name.as_ref().unwrap(),
                    "name should match configured name for album {}",
                    gen_album.dirname
                );
            }
        }
    }

    // ── P7.7: Cover is present when album has photos ───────────────
    //
    // When an album has at least one photo, the cover field must be
    // Some (non-null). When empty, it must be None.
    #[test]
    fn cover_present_when_photos_exist(albums in albums_strategy()) {
        let (dir, config) = write_albums(&albums);
        let out = TempDir::new().unwrap();

        let result = generate_albums_data(
            &dir.path().join("albums"),
            out.path(),
            &config,
        ).unwrap();

        for (gen_album, summary) in albums.iter().zip(result.summaries.iter()) {
            if gen_album.photo_count > 0 {
                prop_assert!(
                    summary.cover.is_some(),
                    "cover must be Some when album {} has {} photos",
                    gen_album.dirname,
                    gen_album.photo_count
                );
            } else {
                prop_assert!(
                    summary.cover.is_none(),
                    "cover must be None when album {} has no photos",
                    gen_album.dirname
                );
            }
        }
    }
}

// ── Edge-case: disabled config ─────────────────────────────────────

#[test]
fn disabled_config_produces_empty_output() {
    let dir = TempDir::new().unwrap();
    let albums_root = dir.path().join("albums");
    fs::create_dir_all(&albums_root).unwrap();

    let config = AlbumConfig {
        enabled: false,
        albums: vec![],
        provider: None,
    };

    let result = generate_albums_data(&albums_root, dir.path(), &config).unwrap();
    assert!(result.summaries.is_empty());
    assert!(result.details.is_empty());

    let index_path = dir.path().join("generated/albums-index.json");
    assert!(index_path.exists());
    let content: Vec<serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(index_path).unwrap()).unwrap();
    assert!(content.is_empty());
}

// ── Edge-case: invalid dirnames are skipped ────────────────────────

#[test]
fn invalid_dirnames_excluded_from_output() {
    let dir = TempDir::new().unwrap();
    let albums_root = dir.path().join("albums");
    fs::create_dir_all(&albums_root).unwrap();

    // Create a valid album
    let valid_dir = albums_root.join("valid-album");
    fs::create_dir_all(&valid_dir).unwrap();

    let config = AlbumConfig {
        enabled: true,
        albums: vec![
            AlbumEntry {
                dir: ".hidden".into(),
                name: None,
                desc: None,
                cover: None,
            },
            AlbumEntry {
                dir: "has space".into(),
                name: None,
                desc: None,
                cover: None,
            },
            AlbumEntry {
                dir: "valid-album".into(),
                name: Some("Valid".into()),
                desc: None,
                cover: None,
            },
        ],
        provider: None,
    };

    let result = generate_albums_data(&albums_root, dir.path(), &config).unwrap();
    assert_eq!(result.summaries.len(), 1);
    assert_eq!(result.summaries[0].dirname, "valid-album");
}
