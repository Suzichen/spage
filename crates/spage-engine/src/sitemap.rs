//! Sitemap XML generation.
//!
//! Produces a `sitemap.xml` conforming to the Sitemaps protocol.
//! The implementation mirrors the TypeScript `generate-sitemap.ts` script
//! so that both produce compatible output for the same inputs.

use std::fs;
use std::path::Path;

use log::warn;

use crate::error::EngineError;
use crate::language::{default_language, localized_languages};
use crate::path_util::{build_full_url, normalize_base_path_option};
use crate::{AlbumConfig, PostMetadata, SiteConfig};

// ── XML escaping ───────────────────────────────────────────────────

/// Escape special characters for safe embedding in XML.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── URL helpers ────────────────────────────────────────────────────

/// Build a full URL: `{siteUrl}{basePath}{relativePath}`
fn get_full_url(site_url: &str, base_path: &str, relative_path: &str) -> String {
    build_full_url(site_url, base_path, relative_path)
}

// ── Sitemap XML generation ─────────────────────────────────────────

/// Build the sitemap XML string.
///
/// `today` is the current date in `YYYY-MM-DD` format, used as the
/// `lastmod` for the homepage and as a fallback for posts without dates.
#[cfg(test)]
fn build_sitemap_xml(
    posts: &[PostMetadata],
    site_url: &str,
    base_path: &str,
    today: &str,
) -> String {
    build_sitemap_xml_with_albums(posts, None, site_url, base_path, today, "en")
}

fn build_sitemap_xml_with_albums(
    posts: &[PostMetadata],
    albums: Option<&AlbumConfig>,
    site_url: &str,
    base_path: &str,
    today: &str,
    default_language: &str,
) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");

    // Homepage
    let homepage_url = get_full_url(site_url, base_path, "/");
    xml.push_str("  <url>\n");
    xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&homepage_url)));
    xml.push_str(&format!("    <lastmod>{}</lastmod>\n", today));
    xml.push_str("    <changefreq>daily</changefreq>\n");
    xml.push_str("    <priority>1.0</priority>\n");
    xml.push_str("  </url>\n");

    // Albums with dedicated SEO pages
    if let Some(album_config) = albums.filter(|config| config.enabled) {
        for album in album_config
            .albums
            .iter()
            .filter(|album| album.desc.is_some())
        {
            let album_url = get_full_url(site_url, base_path, &format!("/albums/{}/", album.dir));
            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&album_url)));
            xml.push_str(&format!("    <lastmod>{}</lastmod>\n", today));
            xml.push_str("    <changefreq>monthly</changefreq>\n");
            xml.push_str("    <priority>0.7</priority>\n");
            xml.push_str("  </url>\n");
        }
    }

    // Posts
    for post in posts {
        let post_url = get_full_url(site_url, base_path, &format!("/post/{}/", post.slug));
        let lastmod = if post.date.is_empty() {
            today.to_string()
        } else {
            // Take the YYYY-MM-DD portion of the date
            post.date.split('T').next().unwrap_or(today).to_string()
        };

        xml.push_str("  <url>\n");
        xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&post_url)));
        xml.push_str(&format!("    <lastmod>{}</lastmod>\n", lastmod));
        xml.push_str("    <changefreq>monthly</changefreq>\n");
        xml.push_str("    <priority>0.8</priority>\n");
        xml.push_str("  </url>\n");

        for language in localized_languages(post, default_language) {
            let localized_url = get_full_url(
                site_url,
                base_path,
                &format!("/post/{}/lang/{}/", post.slug, language),
            );
            xml.push_str("  <url>\n");
            xml.push_str(&format!("    <loc>{}</loc>\n", escape_xml(&localized_url)));
            xml.push_str(&format!("    <lastmod>{}</lastmod>\n", lastmod));
            xml.push_str("    <changefreq>monthly</changefreq>\n");
            xml.push_str("    <priority>0.8</priority>\n");
            xml.push_str("  </url>\n");
        }
    }

    xml.push_str("</urlset>");
    xml
}

// ── Public API ─────────────────────────────────────────────────────

/// Generate a `sitemap.xml` file at `output_path`.
///
/// If `config.site_url` is `None`, logs a warning and returns `Ok(())`
/// without writing any file (matching the TS behaviour).
///
/// Creates parent directories if they don't exist.
pub fn generate_sitemap(
    manifest: &[PostMetadata],
    output_path: &Path,
    config: &SiteConfig,
) -> Result<(), EngineError> {
    generate_sitemap_with_albums(manifest, None, output_path, config)
}

/// Generate a sitemap including albums that have dedicated SEO pages.
pub fn generate_sitemap_with_albums(
    manifest: &[PostMetadata],
    albums: Option<&AlbumConfig>,
    output_path: &Path,
    config: &SiteConfig,
) -> Result<(), EngineError> {
    let site_url = match config.site_url.as_deref() {
        Some(url) if !url.is_empty() => url,
        _ => {
            warn!("Skipping sitemap.xml generation (siteUrl not configured)");
            return Ok(());
        }
    };

    let base_path = normalize_base_path_option(config.base_path.as_deref());
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let default_language = default_language(config);
    let xml = build_sitemap_xml_with_albums(
        manifest,
        albums,
        site_url,
        &base_path,
        &today,
        default_language,
    );

    // Ensure parent directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output_path, &xml)?;

    log::info!(
        "Generated sitemap.xml with {} URLs",
        xml.matches("<url>").count()
    );
    if !base_path.is_empty() {
        log::info!("  BasePath: {}", base_path);
    }

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_config() -> SiteConfig {
        SiteConfig {
            title: "My Blog".to_string(),
            description: "A personal blog".to_string(),
            logo: "/logo.png".to_string(),
            favicon: "/favicon.ico".to_string(),
            site_url: Some("https://example.com".to_string()),
            author: Some("Alice".to_string()),
            language: Some("en".to_string()),
            timezone: None,
            base_path: Some("/".to_string()),
        }
    }

    fn sample_album_config() -> AlbumConfig {
        AlbumConfig {
            enabled: true,
            albums: vec![
                crate::AlbumEntry {
                    dir: "spring".to_string(),
                    name: Some("Spring".to_string()),
                    desc: Some("Spring photos".to_string()),
                    cover: None,
                },
                crate::AlbumEntry {
                    dir: "private".to_string(),
                    name: None,
                    desc: None,
                    cover: None,
                },
            ],
            provider: None,
        }
    }

    fn sample_post() -> PostMetadata {
        PostMetadata {
            slug: "hello-world".to_string(),
            title: "Hello World".to_string(),
            date: "2024-01-15T10:30:00".to_string(),
            tags: vec!["intro".to_string(), "blog".to_string()],
            categories: vec!["General".to_string()],
            summary: "This is my first post".to_string(),
            available_languages: vec![],
            localized_meta: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn generates_valid_sitemap_xml() {
        let xml = build_sitemap_xml(&[sample_post()], "https://example.com", "", "2024-06-01");

        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">"));
        assert!(xml.ends_with("</urlset>"));
    }

    #[test]
    fn homepage_has_priority_1_0() {
        let xml = build_sitemap_xml(&[], "https://example.com", "", "2024-06-01");

        assert!(xml.contains("<loc>https://example.com/</loc>"));
        assert!(xml.contains("<priority>1.0</priority>"));
        assert!(xml.contains("<changefreq>daily</changefreq>"));
    }

    #[test]
    fn includes_only_albums_with_dedicated_seo_pages() {
        let albums = sample_album_config();
        let xml = build_sitemap_xml_with_albums(
            &[],
            Some(&albums),
            "https://example.com",
            "/blog",
            "2024-06-01",
            "en",
        );

        assert!(xml.contains("<loc>https://example.com/blog/albums/spring/</loc>"));
        assert!(xml.contains("<priority>0.7</priority>"));
        assert!(!xml.contains("/albums/private/"));
        assert_eq!(xml.matches("<url>").count(), 2);
    }

    #[test]
    fn disabled_album_config_is_excluded() {
        let mut albums = sample_album_config();
        albums.enabled = false;
        let xml = build_sitemap_xml_with_albums(
            &[],
            Some(&albums),
            "https://example.com",
            "",
            "2024-06-01",
            "en",
        );

        assert!(!xml.contains("/albums/spring/"));
        assert_eq!(xml.matches("<url>").count(), 1);
    }

    #[test]
    fn includes_supported_post_translations() {
        let mut post = sample_post();
        post.available_languages = vec!["en".to_string(), "fr".to_string()];
        let translation = crate::LocalizedPostMeta {
            title: "Translated".to_string(),
            summary: "Summary".to_string(),
            tags: vec![],
            categories: vec![],
        };
        post.localized_meta
            .insert("en".to_string(), translation.clone());
        post.localized_meta.insert("fr".to_string(), translation);
        let xml = build_sitemap_xml_with_albums(
            &[post],
            None,
            "https://example.com",
            "/blog",
            "2024-06-01",
            "zh-CN",
        );

        assert!(xml.contains("<loc>https://example.com/blog/post/hello-world/</loc>"));
        assert!(xml.contains("<loc>https://example.com/blog/post/hello-world/lang/en/</loc>"));
        assert!(!xml.contains("/lang/fr/"));
        assert!(xml.contains("<priority>0.8</priority>"));
        assert!(xml.contains("<changefreq>monthly</changefreq>"));
        assert_eq!(xml.matches("<url>").count(), 3);
    }

    #[test]
    fn post_lastmod_uses_date_portion() {
        let xml = build_sitemap_xml(&[sample_post()], "https://example.com", "", "2024-06-01");

        // Post date is "2024-01-15T10:30:00", lastmod should be "2024-01-15"
        assert!(xml.contains("<lastmod>2024-01-15</lastmod>"));
    }

    #[test]
    fn post_without_date_uses_today() {
        let post = PostMetadata {
            slug: "no-date".to_string(),
            title: "No Date".to_string(),
            date: String::new(),
            tags: vec![],
            categories: vec![],
            summary: "No date post".to_string(),
            available_languages: vec![],
            localized_meta: std::collections::HashMap::new(),
        };

        let xml = build_sitemap_xml(&[post], "https://example.com", "", "2024-06-01");

        // Both homepage and post should use today's date
        let lastmod_count = xml.matches("<lastmod>2024-06-01</lastmod>").count();
        assert_eq!(lastmod_count, 2);
    }

    #[test]
    fn handles_base_path() {
        let xml = build_sitemap_xml(
            &[sample_post()],
            "https://example.com",
            "/blog",
            "2024-06-01",
        );

        assert!(xml.contains("<loc>https://example.com/blog/</loc>"));
        assert!(xml.contains("<loc>https://example.com/blog/post/hello-world/</loc>"));
    }

    #[test]
    fn escapes_special_characters_in_urls() {
        let post = PostMetadata {
            slug: "a&b<c".to_string(),
            title: "Special".to_string(),
            date: "2024-01-01T00:00:00".to_string(),
            tags: vec![],
            categories: vec![],
            summary: "Special chars".to_string(),
            available_languages: vec![],
            localized_meta: std::collections::HashMap::new(),
        };

        let xml = build_sitemap_xml(&[post], "https://example.com", "", "2024-06-01");

        assert!(xml.contains("<loc>https://example.com/post/a&amp;b&lt;c/</loc>"));
    }

    #[test]
    fn skips_generation_when_site_url_not_configured() {
        let tmp = TempDir::new().unwrap();
        let output = tmp.path().join("sitemap.xml");
        let mut config = sample_config();
        config.site_url = None;

        let result = generate_sitemap(&[sample_post()], &output, &config);
        assert!(result.is_ok());
        assert!(!output.exists());
    }

    #[test]
    fn skips_generation_when_site_url_empty() {
        let tmp = TempDir::new().unwrap();
        let output = tmp.path().join("sitemap.xml");
        let mut config = sample_config();
        config.site_url = Some(String::new());

        let result = generate_sitemap(&[sample_post()], &output, &config);
        assert!(result.is_ok());
        assert!(!output.exists());
    }

    #[test]
    fn writes_sitemap_file() {
        let tmp = TempDir::new().unwrap();
        let output = tmp.path().join("dist/sitemap.xml");
        let config = sample_config();

        let result = generate_sitemap(&[sample_post()], &output, &config);
        assert!(result.is_ok());
        assert!(output.exists());

        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(content.contains("https://example.com/"));
        assert!(content.contains("https://example.com/post/hello-world/"));
    }

    #[test]
    fn creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let output = tmp.path().join("deep/nested/dir/sitemap.xml");
        let config = sample_config();

        let result = generate_sitemap(&[sample_post()], &output, &config);
        assert!(result.is_ok());
        assert!(output.exists());
    }

    #[test]
    fn empty_manifest_generates_homepage_only() {
        let xml = build_sitemap_xml(&[], "https://example.com", "", "2024-06-01");

        assert!(xml.contains("<loc>https://example.com/</loc>"));
        // Only one <url> block
        assert_eq!(xml.matches("<url>").count(), 1);
    }

    #[test]
    fn multiple_posts() {
        let posts = vec![
            sample_post(),
            PostMetadata {
                slug: "second-post".to_string(),
                title: "Second Post".to_string(),
                date: "2024-02-01T12:00:00".to_string(),
                tags: vec![],
                categories: vec![],
                summary: "Another post".to_string(),
                available_languages: vec![],
                localized_meta: std::collections::HashMap::new(),
            },
        ];

        let xml = build_sitemap_xml(&posts, "https://example.com", "", "2024-06-01");

        // Homepage + 2 posts = 3 URL blocks
        assert_eq!(xml.matches("<url>").count(), 3);
        assert!(xml.contains("<loc>https://example.com/post/hello-world/</loc>"));
        assert!(xml.contains("<loc>https://example.com/post/second-post/</loc>"));
    }

    #[test]
    fn base_path_normalization() {
        use crate::path_util::normalize_base_path_option;
        // "/" should become ""
        assert_eq!(normalize_base_path_option(Some("/")), "");
        assert_eq!(normalize_base_path_option(None), "");
        assert_eq!(normalize_base_path_option(Some("")), "");
        // "/blog/" should become "/blog"
        assert_eq!(normalize_base_path_option(Some("/blog/")), "/blog");
        assert_eq!(normalize_base_path_option(Some("/blog")), "/blog");
    }
}
