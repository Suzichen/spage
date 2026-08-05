//! Cross-implementation verification test for `generate_seo_pages`.
//!
//! Runs the Rust engine against the golden manifest and compares the
//! generated SEO HTML pages against the golden files produced by the
//! TypeScript `generate-seo.ts` script.
//!
//! **Validates: Requirement 2.7.7** — The Rust output SHALL match the
//! TS `generate-seo.ts` output for the same input fixtures.
//!
//! ## Dynamic-date caveat
//!
//! The TS script uses `new Date().toISOString()` when a post has an
//! empty date, producing a dynamic timestamp. The Rust engine uses an
//! empty string instead. For posts with empty dates (`invalid-date`,
//! `no-frontmatter`), the comparison strips the dynamic
//! `article:published_time` and `datePublished` values before diffing.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use spage_engine::seo::generate_seo_pages;
use spage_engine::{PostMetadata, SiteConfig};

// ── Helpers ────────────────────────────────────────────────────────

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("failed to resolve workspace root")
}

fn fixtures_dir() -> PathBuf {
    workspace_root().join("tests/fixtures")
}

fn golden_dir() -> PathBuf {
    workspace_root().join("tests/golden")
}

fn fixtures_config() -> SiteConfig {
    let path = fixtures_dir().join("config.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

fn golden_manifest() -> Vec<PostMetadata> {
    let path = golden_dir().join("manifest.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

fn read_golden_seo(slug: &str) -> String {
    let path = golden_dir().join("seo").join(slug).join("index.html");
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read golden seo/{slug}/index.html: {e}"))
        .replace("\r\n", "\n")
}

/// The App Shell template that was used to generate the golden files.
/// Reconstructed from the golden output structure.
fn shell_template() -> String {
    r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>spage</title>
  <link rel="icon" href="./favicon.ico">
  <link rel="stylesheet" href="./assets/index.css">
</head>
<body>
  <div id="root"></div>
  <script type="module" src="./assets/index.js"></script>
</body>
</html>"#
        .to_string()
}

/// Posts whose date is empty in the manifest. The TS script fills these
/// with `new Date().toISOString()`, producing a dynamic timestamp that
/// cannot be compared byte-for-byte.
const DYNAMIC_DATE_SLUGS: &[&str] = &["invalid-date", "no-frontmatter"];

/// Returns true if this slug has a dynamic date in the golden file.
fn has_dynamic_date(slug: &str) -> bool {
    DYNAMIC_DATE_SLUGS.contains(&slug)
}

/// Normalize an HTML string for comparison when the post has a dynamic
/// date. Replaces the `article:published_time` content and the
/// `datePublished` JSON-LD value with a fixed placeholder so both the
/// golden (TS-generated) and Rust output can be compared.
fn normalize_dynamic_date(html: &str) -> String {
    let mut out = html.to_string();

    // Replace article:published_time content value
    // Pattern: content="2026-04-28T05:11:27.345Z" or content=""
    let re_published_time = "article:published_time\" content=\"";
    if let Some(start) = out.find(re_published_time) {
        let value_start = start + re_published_time.len();
        if let Some(end) = out[value_start..].find('"') {
            let before = &out[..value_start];
            let after = &out[value_start + end..];
            out = format!("{}__DYNAMIC_DATE__{}", before, after);
        }
    }

    // Replace datePublished in JSON-LD
    let re_date_published = "\"datePublished\": \"";
    if let Some(start) = out.find(re_date_published) {
        let value_start = start + re_date_published.len();
        if let Some(end) = out[value_start..].find('"') {
            let before = &out[..value_start];
            let after = &out[value_start + end..];
            out = format!("{}__DYNAMIC_DATE__{}", before, after);
        }
    }

    out
}

/// Run `generate_seo_pages` against the golden manifest and return
/// the output directory.
fn run_rust() -> tempfile::TempDir {
    let config = fixtures_config();
    let manifest = golden_manifest();
    let tmp = tempfile::TempDir::new().unwrap();

    // Write the template
    let template_path = tmp.path().join("index.html");
    fs::write(&template_path, shell_template()).unwrap();

    let output_dir = tmp.path().join("dist");
    let count = generate_seo_pages(&manifest, &template_path, &output_dir, &config)
        .expect("generate_seo_pages should succeed");

    assert_eq!(
        count,
        manifest.len(),
        "should generate one page per manifest entry"
    );

    tmp
}

/// Collect all generated SEO pages as slug → HTML content.
fn collect_rust_pages(tmp: &tempfile::TempDir) -> HashMap<String, String> {
    let post_dir = tmp.path().join("dist/post");
    let mut pages = HashMap::new();

    if !post_dir.exists() {
        return pages;
    }

    for entry in fs::read_dir(&post_dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            let slug = entry.file_name().to_string_lossy().to_string();
            let html_path = entry.path().join("index.html");
            if html_path.exists() {
                let html = fs::read_to_string(&html_path).unwrap();
                pages.insert(slug, html);
            }
        }
    }

    pages
}

// ── Tests ──────────────────────────────────────────────────────────

/// One SEO page is generated for every post in the manifest.
#[test]
fn rust_generates_page_for_every_post() {
    let tmp = run_rust();
    let manifest = golden_manifest();
    let pages = collect_rust_pages(&tmp);

    assert_eq!(
        pages.len(),
        manifest.len(),
        "page count mismatch: Rust={} manifest={}",
        pages.len(),
        manifest.len()
    );

    for post in &manifest {
        assert!(
            pages.contains_key(&post.slug),
            "missing SEO page for slug {:?}",
            post.slug
        );
    }
}

/// For posts with stable dates, the Rust output matches the golden
/// file byte-for-byte.
#[test]
fn rust_seo_matches_golden_stable_dates() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);
    let manifest = golden_manifest();

    for post in &manifest {
        if has_dynamic_date(&post.slug) {
            continue;
        }

        let golden = read_golden_seo(&post.slug);
        let actual = pages
            .get(&post.slug)
            .unwrap_or_else(|| panic!("missing Rust SEO page for {:?}", post.slug));

        assert_eq!(
            actual, &golden,
            "SEO page for {:?} differs from golden.\n--- Rust ---\n{}\n--- Golden ---\n{}",
            post.slug, actual, golden,
        );
    }
}

/// For posts with dynamic dates (empty date → TS uses `new Date()`),
/// the Rust output matches the golden file after normalizing the
/// dynamic timestamp fields.
#[test]
fn rust_seo_matches_golden_dynamic_dates() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);

    for slug in DYNAMIC_DATE_SLUGS {
        let golden = read_golden_seo(slug);
        let actual = pages
            .get(*slug)
            .unwrap_or_else(|| panic!("missing Rust SEO page for {:?}", slug));

        let golden_normalized = normalize_dynamic_date(&golden);
        let actual_normalized = normalize_dynamic_date(actual);

        assert_eq!(
            actual_normalized, golden_normalized,
            "SEO page for {:?} differs from golden (after date normalization).\n--- Rust ---\n{}\n--- Golden ---\n{}",
            slug, actual_normalized, golden_normalized,
        );
    }
}

/// Every generated page contains the required SEO elements:
/// title, meta description, canonical link, OG tags, Twitter tags,
/// and JSON-LD.
#[test]
fn rust_seo_pages_contain_required_elements() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);
    let config = fixtures_config();

    for (slug, html) in &pages {
        // Basic meta
        assert!(
            html.contains("<title>"),
            "slug {slug:?}: missing <title>"
        );
        assert!(
            html.contains("name=\"description\""),
            "slug {slug:?}: missing meta description"
        );
        assert!(
            html.contains("name=\"robots\""),
            "slug {slug:?}: missing meta robots"
        );

        // When siteUrl is configured, OG/Twitter/JSON-LD should be present
        if config.site_url.is_some() {
            assert!(
                html.contains("rel=\"canonical\""),
                "slug {slug:?}: missing canonical link"
            );
            assert!(
                html.contains("property=\"og:type\""),
                "slug {slug:?}: missing og:type"
            );
            assert!(
                html.contains("property=\"og:title\""),
                "slug {slug:?}: missing og:title"
            );
            assert!(
                html.contains("property=\"og:description\""),
                "slug {slug:?}: missing og:description"
            );
            assert!(
                html.contains("property=\"og:url\""),
                "slug {slug:?}: missing og:url"
            );
            assert!(
                html.contains("name=\"twitter:card\""),
                "slug {slug:?}: missing twitter:card"
            );
            assert!(
                html.contains("name=\"twitter:title\""),
                "slug {slug:?}: missing twitter:title"
            );
            assert!(
                html.contains("application/ld+json"),
                "slug {slug:?}: missing JSON-LD"
            );
            assert!(
                html.contains("\"@type\": \"Article\""),
                "slug {slug:?}: missing Article schema type"
            );
        }
    }
}

/// Asset paths are rewritten from relative to absolute.
#[test]
fn rust_seo_rewrites_asset_paths() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);

    for (slug, html) in &pages {
        assert!(
            !html.contains("\"./assets/"),
            "slug {slug:?}: still has relative ./assets/ path"
        );
        assert!(
            !html.contains("\"./favicon"),
            "slug {slug:?}: still has relative ./favicon path"
        );
        assert!(
            html.contains("href=\"/assets/") || html.contains("src=\"/assets/"),
            "slug {slug:?}: missing absolute /assets/ path"
        );
    }
}

/// The original template <title> is replaced by the post title.
#[test]
fn rust_seo_replaces_template_title() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);

    for (slug, html) in &pages {
        assert!(
            !html.contains("<title>spage</title>"),
            "slug {slug:?}: original template title not removed"
        );
    }
}

/// Post URLs in canonical/OG/Twitter include the correct slug.
#[test]
fn rust_seo_urls_contain_slug() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);
    let config = fixtures_config();

    if config.site_url.is_none() {
        return;
    }

    let site_url = config.site_url.as_ref().unwrap();

    for (slug, html) in &pages {
        let expected_url = format!("{}/post/{}/", site_url.trim_end_matches('/'), slug);
        assert!(
            html.contains(&expected_url),
            "slug {slug:?}: expected URL {expected_url:?} not found in page"
        );
    }
}

/// Posts with tags have article:tag meta elements.
#[test]
fn rust_seo_includes_article_tags() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);
    let manifest = golden_manifest();

    for post in &manifest {
        if post.tags.is_empty() {
            continue;
        }
        let html = pages.get(&post.slug).unwrap();
        for tag in &post.tags {
            let expected = format!("article:tag\" content=\"{}\"", tag);
            assert!(
                html.contains(&expected),
                "slug {:?}: missing article:tag for {:?}",
                post.slug,
                tag
            );
        }
    }
}

/// Posts with tags/categories have a keywords meta element.
#[test]
fn rust_seo_includes_keywords() {
    let tmp = run_rust();
    let pages = collect_rust_pages(&tmp);
    let manifest = golden_manifest();

    for post in &manifest {
        if post.tags.is_empty() && post.categories.is_empty() {
            continue;
        }
        let html = pages.get(&post.slug).unwrap();
        let keywords: Vec<&str> = post
            .tags
            .iter()
            .chain(post.categories.iter())
            .map(|s| s.as_str())
            .collect();
        let expected = keywords.join(", ");
        assert!(
            html.contains(&format!("keywords\" content=\"{}\"", expected)),
            "slug {:?}: keywords mismatch, expected {:?}",
            post.slug,
            expected
        );
    }
}
