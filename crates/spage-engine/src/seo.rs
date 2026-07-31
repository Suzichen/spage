//! SEO static page generation.
//!
//! Generates `dist/post/{slug}/index.html` with meta tags,
//! Open Graph, Twitter Card, and JSON-LD Article schema.
//!
//! The implementation mirrors the TypeScript `generate-seo.ts` script
//! so that both produce byte-compatible output for the same inputs.

use std::fs;
use std::path::Path;

use log::warn;

use crate::error::EngineError;
use crate::path_util::{build_full_url, normalize_base_path_option};
use crate::{AlbumConfig, AlbumEntry, PostMetadata, SiteConfig};

// ── HTML escaping ──────────────────────────────────────────────────

/// Escape special characters for safe embedding in HTML attributes / text.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

/// Normalize line breaks for HTML meta attributes while leaving body and
/// structured-data content unchanged.
fn escape_meta_content(text: &str) -> String {
    escape_html(&text.replace("\r\n", " ").replace(['\n', '\r'], " "))
}

fn append_basic_meta(out: &mut String, title: &str, description: &str) {
    out.push_str(&format!("\n  <title>{}</title>", escape_html(title)));
    out.push_str(&format!(
        "\n  <meta name=\"title\" content=\"{}\">",
        escape_meta_content(title)
    ));
    out.push_str(&format!(
        "\n  <meta name=\"description\" content=\"{}\">",
        escape_meta_content(description)
    ));
}

fn append_indexing_meta(out: &mut String, author: Option<&str>, canonical_url: Option<&str>) {
    if let Some(author) = author {
        out.push_str(&format!(
            "\n  <meta name=\"author\" content=\"{}\">",
            escape_meta_content(author)
        ));
    }
    out.push_str("\n  <meta name=\"robots\" content=\"index, follow\">");
    if let Some(url) = canonical_url.filter(|url| !url.is_empty()) {
        out.push_str(&format!("\n  <link rel=\"canonical\" href=\"{}\">", url));
    }
}

fn append_open_graph_meta(
    out: &mut String,
    og_type: &str,
    url: &str,
    title: &str,
    description: &str,
    site_name: &str,
    image_url: Option<&str>,
) {
    out.push_str(&format!(
        "\n\n  <meta property=\"og:type\" content=\"{}\">",
        og_type
    ));
    out.push_str(&format!(
        "\n  <meta property=\"og:url\" content=\"{}\">",
        url
    ));
    out.push_str(&format!(
        "\n  <meta property=\"og:title\" content=\"{}\">",
        escape_meta_content(title)
    ));
    out.push_str(&format!(
        "\n  <meta property=\"og:description\" content=\"{}\">",
        escape_meta_content(description)
    ));
    out.push_str(&format!(
        "\n  <meta property=\"og:site_name\" content=\"{}\">",
        escape_meta_content(site_name)
    ));
    if let Some(image_url) = image_url.filter(|url| !url.is_empty()) {
        out.push_str(&format!(
            "\n  <meta property=\"og:image\" content=\"{}\">",
            image_url
        ));
    }
}

fn append_twitter_meta(
    out: &mut String,
    card: &str,
    url: &str,
    title: &str,
    description: &str,
    image_url: Option<&str>,
) {
    out.push_str(&format!(
        "\n\n  <meta name=\"twitter:card\" content=\"{}\">",
        card
    ));
    out.push_str(&format!(
        "\n  <meta name=\"twitter:url\" content=\"{}\">",
        url
    ));
    out.push_str(&format!(
        "\n  <meta name=\"twitter:title\" content=\"{}\">",
        escape_meta_content(title)
    ));
    out.push_str(&format!(
        "\n  <meta name=\"twitter:description\" content=\"{}\">",
        escape_meta_content(description)
    ));
    if let Some(image_url) = image_url.filter(|url| !url.is_empty()) {
        out.push_str(&format!(
            "\n  <meta name=\"twitter:image\" content=\"{}\">",
            image_url
        ));
    }
}

// ── URL helpers ────────────────────────────────────────────────────

/// Build the full URL for a post: `{siteUrl}{basePath}/post/{slug}/`
fn get_full_url(site_url: &str, base_path: &str, relative_path: &str) -> String {
    build_full_url(site_url, base_path, relative_path)
}

// ── JSON-LD serialization ──────────────────────────────────────────

/// Manually build the JSON-LD string to match the TS `JSON.stringify(obj, null, 2)` output exactly.
fn build_json_ld(
    title: &str,
    summary: &str,
    author: &str,
    publish_date: &str,
    post_url: &str,
    keywords: &str,
) -> String {
    // We build this manually to match the exact TS JSON.stringify(obj, null, 2) output.
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str("  \"@context\": \"https://schema.org\",\n");
    s.push_str("  \"@type\": \"Article\",\n");
    s.push_str(&format!(
        "  \"headline\": {},\n",
        serde_json::to_string(title).unwrap_or_else(|_| format!("\"{}\"", title))
    ));
    s.push_str(&format!(
        "  \"description\": {},\n",
        serde_json::to_string(summary).unwrap_or_else(|_| format!("\"{}\"", summary))
    ));
    s.push_str("  \"author\": {\n");
    s.push_str("    \"@type\": \"Person\",\n");
    s.push_str(&format!(
        "    \"name\": {}\n",
        serde_json::to_string(author).unwrap_or_else(|_| format!("\"{}\"", author))
    ));
    s.push_str("  },\n");
    s.push_str(&format!(
        "  \"datePublished\": {},\n",
        serde_json::to_string(publish_date).unwrap_or_else(|_| format!("\"{}\"", publish_date))
    ));
    s.push_str(&format!(
        "  \"url\": {},\n",
        serde_json::to_string(post_url).unwrap_or_else(|_| format!("\"{}\"", post_url))
    ));
    s.push_str(&format!(
        "  \"keywords\": {}\n",
        serde_json::to_string(keywords).unwrap_or_else(|_| format!("\"{}\"", keywords))
    ));
    s.push('}');
    s
}

// ── SEO tag generation ─────────────────────────────────────────────

/// Generate the SEO `<head>` snippet for a single post.
///
/// The output matches the TS `generateSEOHtml` function exactly.
fn generate_seo_html(post: &PostMetadata, config: &SiteConfig, base_path: &str) -> String {
    let title = &post.title;
    let summary = &post.summary;
    let tags = &post.tags;
    let categories = &post.categories;
    let date = &post.date;
    let slug = &post.slug;

    let site_url = config.site_url.as_deref();
    let author = config.author.as_deref();

    let post_url = match site_url {
        Some(url) => get_full_url(url, base_path, &format!("/post/{}/", slug)),
        None => String::new(),
    };

    let keywords: String = tags
        .iter()
        .chain(categories.iter())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    let publish_date = if date.is_empty() {
        // TS uses `new Date().toISOString()` — but for reproducibility we
        // keep the empty string; the TS script only hits this branch when
        // date is falsy, and in practice all posts have dates.
        String::new()
    } else {
        date.clone()
    };

    let mut out = String::new();

    // --- basic meta ---
    append_basic_meta(&mut out, title, summary);
    if !keywords.is_empty() {
        out.push_str(&format!(
            "\n  <meta name=\"keywords\" content=\"{}\">",
            escape_html(&keywords)
        ));
    }
    append_indexing_meta(
        &mut out,
        author,
        (!post_url.is_empty()).then_some(post_url.as_str()),
    );

    // --- Open Graph + Twitter (only when siteUrl is set) ---
    if site_url.is_some() {
        append_open_graph_meta(
            &mut out,
            "article",
            &post_url,
            title,
            summary,
            &config.title,
            None,
        );
        out.push_str(&format!(
            "\n  <meta property=\"article:published_time\" content=\"{}\">",
            publish_date
        ));
        if let Some(a) = author {
            out.push_str(&format!(
                "\n  <meta property=\"article:author\" content=\"{}\">",
                escape_html(a)
            ));
        }
        for tag in tags {
            out.push_str(&format!(
                "\n  <meta property=\"article:tag\" content=\"{}\">",
                escape_html(tag)
            ));
        }

        append_twitter_meta(&mut out, "summary", &post_url, title, summary, None);
    }

    // --- JSON-LD (only when siteUrl is set) ---
    if site_url.is_some() {
        let author_name = author.unwrap_or("Anonymous");
        let json_ld = build_json_ld(
            title,
            summary,
            author_name,
            &publish_date,
            &post_url,
            &keywords,
        );
        out.push_str(&format!(
            "\n\n  <script type=\"application/ld+json\">\n{}\n  </script>",
            json_ld
        ));
    }

    out
}

// ── Album SEO ──────────────────────────────────────────────────────

fn generate_album_head(
    album: &AlbumEntry,
    album_config: &AlbumConfig,
    config: &SiteConfig,
    base_path: &str,
) -> String {
    let Some(site_url) = config.site_url.as_deref().filter(|url| !url.is_empty()) else {
        return String::new();
    };

    let name = album.name.as_deref().unwrap_or(&album.dir);
    let desc = album.desc.as_deref().unwrap_or_default();
    let title = format!("{} - {}", name, config.title);
    let album_path = format!("/albums/{}/", album.dir);
    let album_url = build_full_url(site_url, base_path, &album_path);
    let image_url = album
        .cover
        .as_deref()
        .and_then(|cover| {
            if let Some(provider) = album_config.provider.as_ref() {
                Some(format!(
                    "{}/albums/{}/{}",
                    provider.public_url.trim_end_matches('/'),
                    album.dir,
                    cover
                ))
            } else {
                Some(build_full_url(
                    site_url,
                    base_path,
                    &format!("/albums/{}/{}", album.dir, cover),
                ))
            }
        })
        .unwrap_or_default();

    let mut out = String::new();
    append_basic_meta(&mut out, &title, desc);
    append_indexing_meta(&mut out, config.author.as_deref(), Some(album_url.as_str()));
    append_open_graph_meta(
        &mut out,
        "website",
        &album_url,
        &title,
        desc,
        &config.title,
        Some(image_url.as_str()),
    );
    let card = if image_url.is_empty() {
        "summary"
    } else {
        "summary_large_image"
    };
    append_twitter_meta(
        &mut out,
        card,
        &album_url,
        &title,
        desc,
        Some(image_url.as_str()),
    );

    let mut json_ld = serde_json::json!({
            "@context": "https://schema.org",
            "@type": "CollectionPage",
            "name": name,
            "description": desc,
            "url": album_url,
            "isPartOf": {
                "@type": "WebSite",
                "name": config.title,
                "url": site_url
            }
    });
    if !image_url.is_empty() {
        json_ld["primaryImageOfPage"] = serde_json::json!({
            "@type": "ImageObject",
            "url": image_url
        });
    }
    let json_ld = serde_json::to_string_pretty(&json_ld)
        .unwrap_or_default()
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    out.push_str(&format!(
        "\n\n  <script type=\"application/ld+json\">\n{}\n  </script>",
        json_ld
    ));

    out
}

/// Generate dedicated SEO pages for albums that define `desc`.
pub fn generate_album_seo_pages(
    album_config: &AlbumConfig,
    template_path: &Path,
    output_dir: &Path,
    config: &SiteConfig,
) -> Result<usize, EngineError> {
    if !album_config.enabled
        || !matches!(config.site_url.as_deref(), Some(site_url) if !site_url.is_empty())
    {
        return Ok(0);
    }

    let template = fs::read_to_string(template_path)?;
    let base_path = normalize_base_path_option(config.base_path.as_deref());
    let lang = config.language.as_deref().unwrap_or("en");
    let mut generated = 0;

    for album in album_config
        .albums
        .iter()
        .filter(|album| album.desc.is_some())
    {
        let name = album.name.as_deref().unwrap_or(&album.dir);
        let head_tags = generate_album_head(album, album_config, config, &base_path);
        let body_content = format!(
            "<main><h1>{}</h1><p style=\"white-space:pre-wrap\">{}</p></main>",
            escape_html(name),
            escape_html(album.desc.as_deref().unwrap_or_default())
        );

        let mut html = template.clone();
        html = inject_html_lang(&html, lang);
        html = rewrite_asset_paths(&html, &base_path);
        html = remove_title_tag(&html);
        html = html.replace("</head>", &format!("{}\n</head>", head_tags));
        html = html.replace(
            "<div id=\"root\"></div>",
            &format!("<div id=\"root\">{}</div>", body_content),
        );

        let album_dir = output_dir.join("albums").join(&album.dir);
        fs::create_dir_all(&album_dir)?;
        fs::write(album_dir.join("index.html"), html)?;
        generated += 1;
    }

    Ok(generated)
}

// ── Homepage SEO ───────────────────────────────────────────────────

const POSTS_PER_PAGE: usize = 10;

/// Generate the SEO `<head>` snippet for a homepage/pagination page.
fn generate_homepage_head(
    config: &SiteConfig,
    base_path: &str,
    page: usize,
    total_pages: usize,
) -> String {
    let site_url = config.site_url.as_deref();
    let author = config.author.as_deref();

    let page_path = if page == 1 {
        "/".to_string()
    } else {
        format!("/page/{}/", page)
    };
    let page_url = match site_url {
        Some(url) => build_full_url(url, base_path, &page_path),
        None => String::new(),
    };
    let image_url = match site_url {
        Some(url) => build_full_url(url, base_path, &config.logo),
        None => String::new(),
    };

    let title = if page == 1 {
        config.title.clone()
    } else {
        format!("{} - Page {}", config.title, page)
    };

    let mut out = String::new();

    append_basic_meta(&mut out, &title, &config.description);
    append_indexing_meta(
        &mut out,
        author,
        (!page_url.is_empty()).then_some(page_url.as_str()),
    );

    // Pagination rel links
    if let Some(url) = site_url {
        if page > 1 {
            let prev_path = if page == 2 {
                "/".to_string()
            } else {
                format!("/page/{}/", page - 1)
            };
            out.push_str(&format!(
                "\n  <link rel=\"prev\" href=\"{}\">",
                build_full_url(url, base_path, &prev_path)
            ));
        }
        if page < total_pages {
            out.push_str(&format!(
                "\n  <link rel=\"next\" href=\"{}\">",
                build_full_url(url, base_path, &format!("/page/{}/", page + 1))
            ));
        }
    }

    // OG + Twitter (only when siteUrl is set)
    if site_url.is_some() {
        append_open_graph_meta(
            &mut out,
            "website",
            &page_url,
            &title,
            &config.description,
            &config.title,
            Some(&image_url),
        );
        append_twitter_meta(
            &mut out,
            "summary_large_image",
            &page_url,
            &title,
            &config.description,
            Some(&image_url),
        );
    }

    // JSON-LD WebSite schema (only on page 1)
    if page == 1 {
        if let Some(_) = site_url {
            let author_name = author.unwrap_or("Anonymous");
            let json_ld = format!(
                "{{\n  \"@context\": \"https://schema.org\",\n  \"@type\": \"WebSite\",\n  \"name\": {},\n  \"url\": {},\n  \"description\": {},\n  \"author\": {{\n    \"@type\": \"Person\",\n    \"name\": {}\n  }}\n}}",
                serde_json::to_string(&config.title).unwrap_or_else(|_| format!("\"{}\"", &config.title)),
                serde_json::to_string(&page_url).unwrap_or_else(|_| format!("\"{}\"", &page_url)),
                serde_json::to_string(&config.description).unwrap_or_else(|_| format!("\"{}\"", &config.description)),
                serde_json::to_string(author_name).unwrap_or_else(|_| format!("\"{}\"", author_name)),
            );
            out.push_str(&format!(
                "\n\n  <script type=\"application/ld+json\">\n{}\n  </script>",
                json_ld
            ));
        }
    }

    out
}

// ── HTML lang injection ─────────────────────────────────────────────

/// Inject `lang` attribute into the `<html>` tag.
///
/// Handles both `<html>` (no attributes) and `<html ...>` (existing attributes).
/// If the tag already contains a `lang` attribute, it is left unchanged.
fn inject_html_lang(html: &str, lang: &str) -> String {
    if let Some(pos) = html.find("<html") {
        let rest = &html[pos..];
        let close = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..close];
        // Already has lang attribute — skip
        if tag.contains("lang=") {
            return html.to_string();
        }
        if tag == "<html" {
            // bare <html>
            let mut result = String::with_capacity(html.len() + 12);
            result.push_str(&html[..pos]);
            result.push_str(&format!("<html lang=\"{}\">", lang));
            result.push_str(&html[pos + "<html>".len()..]);
            return result;
        }
        // <html with other attributes
        let mut result = String::with_capacity(html.len() + 12);
        result.push_str(&html[..pos]);
        result.push_str(&format!("<html lang=\"{}\" ", lang));
        result.push_str(&html[pos + "<html ".len()..]);
        return result;
    }
    html.to_string()
}

// ── Skeleton screen ────────────────────────────────────────────────

const SKELETON_HTML: &str = r#"
    <style>@keyframes sbp{0%,100%{opacity:.4}50%{opacity:1}}.sb-sk{animation:sbp 1.5s ease-in-out infinite;background:#e5e7eb;border-radius:4px}.sb-main{opacity:0;position:absolute;pointer-events:none}</style>
    <header style="padding:1rem 2rem;border-bottom:1px solid #eee;display:flex;align-items:center;gap:1rem">
      <div class="sb-sk" style="width:80px;height:80px;border-radius:50%;flex-shrink:0"></div>
      <div style="flex:1">
        <div class="sb-sk" style="width:160px;height:24px;margin-bottom:8px"></div>
        <div class="sb-sk" style="width:220px;height:14px"></div>
      </div>
      <nav style="display:flex;gap:12px">
        <div class="sb-sk" style="width:48px;height:16px"></div>
        <div class="sb-sk" style="width:48px;height:16px"></div>
        <div class="sb-sk" style="width:48px;height:16px"></div>
      </nav>
    </header>
    <div style="max-width:800px;margin:0 auto;padding:2rem 1rem">
      <div style="margin-bottom:3rem"><div class="sb-sk" style="width:70%;height:22px;margin-bottom:12px"></div><div class="sb-sk" style="width:30%;height:14px;margin-bottom:10px"></div><div class="sb-sk" style="width:100%;height:14px;margin-bottom:6px"></div><div class="sb-sk" style="width:85%;height:14px"></div></div>
      <div style="margin-bottom:3rem"><div class="sb-sk" style="width:55%;height:22px;margin-bottom:12px"></div><div class="sb-sk" style="width:25%;height:14px;margin-bottom:10px"></div><div class="sb-sk" style="width:100%;height:14px;margin-bottom:6px"></div><div class="sb-sk" style="width:90%;height:14px"></div></div>
      <div style="margin-bottom:3rem"><div class="sb-sk" style="width:60%;height:22px;margin-bottom:12px"></div><div class="sb-sk" style="width:28%;height:14px;margin-bottom:10px"></div><div class="sb-sk" style="width:95%;height:14px;margin-bottom:6px"></div><div class="sb-sk" style="width:80%;height:14px"></div></div>
    </div>"#;

/// Generate the article list HTML for the `<div id="root">` content.
fn generate_post_list_html(posts: &[PostMetadata], base_path: &str, title: &str) -> String {
    let mut out = String::new();
    out.push_str(SKELETON_HTML);
    out.push_str(&format!(
        "\n    <h1 style=\"position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);border:0\">{}</h1>",
        escape_html(title)
    ));
    out.push_str("\n    <main class=\"sb-main\">");
    for post in posts {
        let post_url = format!("{}/post/{}", base_path, &post.slug);
        out.push_str(&format!(
            "\n      <article>\n        <h2><a href=\"{}\">{}</a></h2>\n        <time datetime=\"{}\">{}</time>\n        <p>{}</p>\n      </article>",
            post_url,
            escape_html(&post.title),
            escape_html(&post.date),
            escape_html(&post.date),
            escape_html(&post.summary),
        ));
    }
    out.push_str("\n    </main>\n  ");
    out
}

/// Generate SEO-optimized homepage and pagination pages.
///
/// For each page, injects SEO head tags and a static article list into
/// `<div id="root">`. React will hydrate/replace the content on load.
///
/// - Page 1: `output_dir/index.html` (overwrites existing)
/// - Page N: `output_dir/page/N/index.html`
pub fn generate_homepage_seo(
    output_dir: &Path,
    config: &SiteConfig,
    manifest: &[PostMetadata],
) -> Result<(), EngineError> {
    let index_path = output_dir.join("index.html");
    if !index_path.exists() {
        return Ok(());
    }

    let template = fs::read_to_string(&index_path)?;
    let base_path = normalize_base_path_option(config.base_path.as_deref());
    let lang = config.language.as_deref().unwrap_or("en");
    let total_pages = if manifest.is_empty() {
        1
    } else {
        (manifest.len() + POSTS_PER_PAGE - 1) / POSTS_PER_PAGE
    };

    for page in 1..=total_pages {
        let start = (page - 1) * POSTS_PER_PAGE;
        let end = std::cmp::min(start + POSTS_PER_PAGE, manifest.len());
        let page_posts = &manifest[start..end];

        let head_tags = generate_homepage_head(config, &base_path, page, total_pages);
        let body_content = generate_post_list_html(page_posts, &base_path, &config.title);

        let mut html = template.clone();
        html = inject_html_lang(&html, lang);
        html = remove_title_tag(&html);
        html = html.replace("</head>", &format!("{}\n</head>", head_tags));
        html = html.replace(
            "<div id=\"root\"></div>",
            &format!("<div id=\"root\">{}</div>", body_content),
        );

        if page == 1 {
            fs::write(&index_path, &html)?;
        } else {
            let page_dir = output_dir.join(format!("page/{}", page));
            fs::create_dir_all(&page_dir)?;
            fs::write(page_dir.join("index.html"), &html)?;
        }
    }

    Ok(())
}

// ── Public API ─────────────────────────────────────────────────────

/// Generate one SEO HTML page per post.
///
/// For each entry in `manifest`, reads the App Shell template from
/// `template_path`, injects SEO tags into `<head>`, rewrites relative
/// asset paths to absolute, and writes the result to
/// `output_dir/post/{slug}/index.html`.
///
/// Returns the number of pages generated.
pub fn generate_seo_pages(
    manifest: &[PostMetadata],
    template_path: &Path,
    output_dir: &Path,
    config: &SiteConfig,
) -> Result<usize, EngineError> {
    let template = fs::read_to_string(template_path)?;
    let base_path = normalize_base_path_option(config.base_path.as_deref());
    let lang = config.language.as_deref().unwrap_or("en");

    let post_output_dir = output_dir.join("post");
    fs::create_dir_all(&post_output_dir)?;

    let mut generated: usize = 0;

    for post in manifest {
        let seo_tags = generate_seo_html(post, config, &base_path);

        let mut html = template.clone();
        html = inject_html_lang(&html, lang);

        // Rewrite relative asset paths to absolute (SEO pages live in
        // /post/{slug}/ so relative `./assets/` would break).
        html = rewrite_asset_paths(&html, &base_path);

        // Remove existing <title> tag (will be replaced by SEO title).
        html = remove_title_tag(&html);

        // Inject SEO tags right before </head>.
        html = html.replace("</head>", &format!("{}\n</head>", seo_tags));

        // Write to output_dir/post/{slug}/index.html
        let slug_dir = post_output_dir.join(&post.slug);
        fs::create_dir_all(&slug_dir)?;
        let output_file = slug_dir.join("index.html");
        fs::write(&output_file, &html)?;

        generated += 1;
    }

    if generated > 0 {
        if config.site_url.is_none() {
            warn!("siteUrl not configured. Some SEO features are limited.");
        }
        if !base_path.is_empty() {
            log::info!("SEO BasePath: {}", base_path);
        }
    }

    Ok(generated)
}

fn rewrite_asset_paths(html: &str, base_path: &str) -> String {
    html.replace("href=\"./assets/", &format!("href=\"{}/assets/", base_path))
        .replace("src=\"./assets/", &format!("src=\"{}/assets/", base_path))
        .replace("href=\"./favicon", &format!("href=\"{}/favicon", base_path))
        .replace("src=\"./favicon", &format!("src=\"{}/favicon", base_path))
}

/// Remove the first `<title>…</title>` from the HTML string.
///
/// Uses a simple scan rather than regex to avoid pulling in the `regex` crate.
fn remove_title_tag(html: &str) -> String {
    if let Some(start) = html.find("<title>") {
        if let Some(end_offset) = html[start..].find("</title>") {
            let end = start + end_offset + "</title>".len();
            let mut result = String::with_capacity(html.len());
            result.push_str(&html[..start]);
            result.push_str(&html[end..]);
            return result;
        }
    }
    html.to_string()
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
            albums: vec![AlbumEntry {
                dir: "spring".to_string(),
                name: Some("Spring & Flowers".to_string()),
                desc: Some("First line\nSecond <line>".to_string()),
                cover: Some("cover.jpg".to_string()),
            }],
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

    fn minimal_template() -> String {
        r#"<!DOCTYPE html>
<html>
<head>
  <title>App Shell</title>
  <link rel="icon" href="./favicon.ico">
  <script type="module" src="./assets/index.js"></script>
  <link rel="stylesheet" href="./assets/index.css">
</head>
<body>
  <div id="root"></div>
</body>
</html>"#
            .to_string()
    }

    #[test]
    fn generates_dedicated_album_seo_for_desc() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();
        let output_dir = tmp.path().join("dist");

        let count = generate_album_seo_pages(
            &sample_album_config(),
            &template_path,
            &output_dir,
            &sample_config(),
        )
        .unwrap();

        assert_eq!(count, 1);
        let html = fs::read_to_string(output_dir.join("albums/spring/index.html")).unwrap();
        assert!(html.contains("<title>Spring &amp; Flowers - My Blog</title>"));
        assert!(html.contains("name=\"description\" content=\"First line Second &lt;line&gt;\""));
        assert!(
            html.contains("property=\"og:description\" content=\"First line Second &lt;line&gt;\"")
        );
        assert!(html
            .contains("name=\"twitter:description\" content=\"First line Second &lt;line&gt;\""));
        assert!(html.contains("rel=\"canonical\" href=\"https://example.com/albums/spring/\""));
        assert!(html.contains(
            "property=\"og:image\" content=\"https://example.com/albums/spring/cover.jpg\""
        ));
        assert!(html.contains("\"@type\": \"CollectionPage\""));
        assert!(html.contains("style=\"white-space:pre-wrap\""));
        assert!(html.contains("First line\nSecond &lt;line&gt;"));
        assert!(html.contains("First line\\nSecond \\u003cline\\u003e"));
        assert!(!html.contains("Second <line>"));
    }

    #[test]
    fn album_head_is_empty_without_site_url() {
        let mut config = sample_config();
        config.site_url = None;
        let album_config = sample_album_config();
        let head = generate_album_head(
            &album_config.albums[0],
            &album_config,
            &config,
            "",
        );

        assert!(head.is_empty());
    }

    #[test]
    fn skips_album_seo_without_desc_or_when_disabled() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();
        let output_dir = tmp.path().join("dist");
        let mut album_config = sample_album_config();
        album_config.albums[0].desc = None;

        let count =
            generate_album_seo_pages(&album_config, &template_path, &output_dir, &sample_config())
                .unwrap();
        assert_eq!(count, 0);

        album_config.albums[0].desc = Some("Description".to_string());
        album_config.enabled = false;
        let count =
            generate_album_seo_pages(&album_config, &template_path, &output_dir, &sample_config())
                .unwrap();
        assert_eq!(count, 0);

        album_config.enabled = true;
        let mut site_config = sample_config();
        site_config.site_url = None;
        let count =
            generate_album_seo_pages(&album_config, &template_path, &output_dir, &site_config)
                .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn post_meta_descriptions_replace_line_breaks() {
        let mut post = sample_post();
        post.summary = "First line\nSecond line".to_string();
        let head = generate_seo_html(&post, &sample_config(), "");

        assert!(head.contains("name=\"description\" content=\"First line Second line\""));
        assert!(head.contains("property=\"og:description\" content=\"First line Second line\""));
        assert!(head.contains("name=\"twitter:description\" content=\"First line Second line\""));
        assert!(head.contains("\"description\": \"First line\\nSecond line\""));
    }

    #[test]
    fn generates_seo_page_for_each_post() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        let count = generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();
        assert_eq!(count, 1);

        let generated = output_dir.join("post/hello-world/index.html");
        assert!(generated.exists());
    }

    #[test]
    fn injects_title_and_meta_tags() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        assert!(html.contains("<title>Hello World</title>"));
        assert!(html.contains("name=\"description\" content=\"This is my first post\""));
        assert!(html.contains("name=\"keywords\" content=\"intro, blog, General\""));
        assert!(html.contains("rel=\"canonical\" href=\"https://example.com/post/hello-world/\""));
    }

    #[test]
    fn injects_open_graph_tags() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        assert!(html.contains("property=\"og:type\" content=\"article\""));
        assert!(
            html.contains("property=\"og:url\" content=\"https://example.com/post/hello-world/\"")
        );
        assert!(html.contains("property=\"og:title\" content=\"Hello World\""));
        assert!(html.contains("property=\"og:description\" content=\"This is my first post\""));
        assert!(html.contains("property=\"og:site_name\" content=\"My Blog\""));
        assert!(
            html.contains("property=\"article:published_time\" content=\"2024-01-15T10:30:00\"")
        );
        assert!(html.contains("property=\"article:author\" content=\"Alice\""));
        assert!(html.contains("property=\"article:tag\" content=\"intro\""));
        assert!(html.contains("property=\"article:tag\" content=\"blog\""));
    }

    #[test]
    fn injects_twitter_card_tags() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        assert!(html.contains("name=\"twitter:card\" content=\"summary\""));
        assert!(
            html.contains("name=\"twitter:url\" content=\"https://example.com/post/hello-world/\"")
        );
        assert!(html.contains("name=\"twitter:title\" content=\"Hello World\""));
        assert!(html.contains("name=\"twitter:description\" content=\"This is my first post\""));
    }

    #[test]
    fn injects_json_ld_article_schema() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        assert!(html.contains("application/ld+json"));
        assert!(html.contains("\"@context\": \"https://schema.org\""));
        assert!(html.contains("\"@type\": \"Article\""));
        assert!(html.contains("\"headline\": \"Hello World\""));
        assert!(html.contains("\"description\": \"This is my first post\""));
        assert!(html.contains("\"name\": \"Alice\""));
        assert!(html.contains("\"datePublished\": \"2024-01-15T10:30:00\""));
        assert!(html.contains("\"url\": \"https://example.com/post/hello-world/\""));
        assert!(html.contains("\"keywords\": \"intro, blog, General\""));
    }

    #[test]
    fn removes_original_title_tag() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        // Original "App Shell" title should be gone
        assert!(!html.contains("App Shell"));
        // New title should be present
        assert!(html.contains("<title>Hello World</title>"));
    }

    #[test]
    fn rewrites_relative_asset_paths() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        // Relative paths should be rewritten to absolute
        assert!(html.contains("src=\"/assets/index.js\""));
        assert!(html.contains("href=\"/assets/index.css\""));
        assert!(html.contains("href=\"/favicon.ico\""));
        // No relative paths should remain
        assert!(!html.contains("\"./assets/"));
        assert!(!html.contains("\"./favicon"));
    }

    #[test]
    fn handles_base_path() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let mut config = sample_config();
        config.base_path = Some("/blog".to_string());
        config.site_url = Some("https://example.com".to_string());
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        // Asset paths should include basePath
        assert!(html.contains("src=\"/blog/assets/index.js\""));
        assert!(html.contains("href=\"/blog/assets/index.css\""));
        assert!(html.contains("href=\"/blog/favicon.ico\""));
        // Canonical URL should include basePath
        assert!(html.contains("href=\"https://example.com/blog/post/hello-world/\""));
    }

    #[test]
    fn skips_og_twitter_jsonld_without_site_url() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let mut config = sample_config();
        config.site_url = None;
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();

        // Basic meta should still be present
        assert!(html.contains("<title>Hello World</title>"));
        assert!(html.contains("name=\"description\""));
        // OG, Twitter, JSON-LD should be absent
        assert!(!html.contains("og:type"));
        assert!(!html.contains("twitter:card"));
        assert!(!html.contains("application/ld+json"));
        // No canonical link
        assert!(!html.contains("rel=\"canonical\""));
    }

    #[test]
    fn generates_multiple_posts() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![
            sample_post(),
            PostMetadata {
                slug: "second-post".to_string(),
                title: "Second Post".to_string(),
                date: "2024-02-01T12:00:00".to_string(),
                tags: vec!["rust".to_string()],
                categories: vec![],
                summary: "Another post".to_string(),
                available_languages: vec![],
                localized_meta: std::collections::HashMap::new(),
            },
        ];

        let count = generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();
        assert_eq!(count, 2);

        assert!(output_dir.join("post/hello-world/index.html").exists());
        assert!(output_dir.join("post/second-post/index.html").exists());
    }

    #[test]
    fn empty_manifest_generates_nothing() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();

        let count = generate_seo_pages(&[], &template_path, &output_dir, &config).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn escapes_special_characters_in_html() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let config = sample_config();
        let posts = vec![PostMetadata {
            slug: "special-chars".to_string(),
            title: "A <b>bold</b> & \"quoted\" title".to_string(),
            date: "2024-01-01T00:00:00".to_string(),
            tags: vec![],
            categories: vec![],
            summary: "Summary with <script>alert('xss')</script>".to_string(),
            available_languages: vec![],
            localized_meta: std::collections::HashMap::new(),
        }];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/special-chars/index.html")).unwrap();

        // HTML meta attributes should be escaped
        assert!(html.contains("A &lt;b&gt;bold&lt;/b&gt; &amp; &quot;quoted&quot; title"));
        // The meta description should have escaped HTML
        assert!(html.contains(
            "content=\"Summary with &lt;script&gt;alert(&#039;xss&#039;)&lt;/script&gt;\""
        ));
    }

    #[test]
    fn remove_title_tag_works() {
        assert_eq!(
            remove_title_tag("<head><title>Old</title></head>"),
            "<head></head>"
        );
    }

    #[test]
    fn remove_title_tag_no_title() {
        let input = "<head><meta charset=\"utf-8\"></head>";
        assert_eq!(remove_title_tag(input), input);
    }

    // ── Homepage SEO tests ─────────────────────────────────────────

    #[test]
    fn homepage_seo_injects_basic_meta() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("<title>My Blog</title>"));
        assert!(html.contains("name=\"title\" content=\"My Blog\""));
        assert!(html.contains("name=\"description\" content=\"A personal blog\""));
        assert!(html.contains("name=\"author\" content=\"Alice\""));
        assert!(html.contains("name=\"robots\" content=\"index, follow\""));
        assert!(html.contains("rel=\"canonical\" href=\"https://example.com/\""));
        assert!(!html.contains("App Shell"));
    }

    #[test]
    fn homepage_seo_injects_og_tags() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("property=\"og:type\" content=\"website\""));
        assert!(html.contains("property=\"og:url\" content=\"https://example.com/\""));
        assert!(html.contains("property=\"og:title\" content=\"My Blog\""));
        assert!(html.contains("property=\"og:image\" content=\"https://example.com/logo.png\""));
    }

    #[test]
    fn homepage_seo_injects_twitter_card() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("name=\"twitter:card\" content=\"summary_large_image\""));
        assert!(html.contains("name=\"twitter:image\" content=\"https://example.com/logo.png\""));
    }

    #[test]
    fn homepage_seo_injects_json_ld() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("\"@type\": \"WebSite\""));
        assert!(html.contains("\"name\": \"My Blog\""));
    }

    #[test]
    fn homepage_seo_with_base_path() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let mut config = sample_config();
        config.base_path = Some("/blog".to_string());
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("rel=\"canonical\" href=\"https://example.com/blog/\""));
        assert!(
            html.contains("property=\"og:image\" content=\"https://example.com/blog/logo.png\"")
        );
    }

    #[test]
    fn homepage_seo_skips_og_without_site_url() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let mut config = sample_config();
        config.site_url = None;
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("<title>My Blog</title>"));
        assert!(!html.contains("og:type"));
        assert!(!html.contains("twitter:card"));
        assert!(!html.contains("application/ld+json"));
    }

    #[test]
    fn homepage_seo_no_op_when_index_missing() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        let config = sample_config();
        let result = generate_homepage_seo(&output_dir, &config, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn homepage_seo_includes_post_list() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        let posts = vec![sample_post()];
        generate_homepage_seo(&output_dir, &config, &posts).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("<article>"));
        assert!(html.contains("<a href=\"/post/hello-world\">Hello World</a>"));
        assert!(html.contains("This is my first post"));
    }

    #[test]
    fn homepage_seo_generates_pagination_pages() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        // Create 12 posts to trigger 2 pages
        let posts: Vec<PostMetadata> = (0..12)
            .map(|i| PostMetadata {
                slug: format!("post-{}", i),
                title: format!("Post {}", i),
                date: "2024-01-01T00:00:00".to_string(),
                tags: vec![],
                categories: vec![],
                summary: format!("Summary {}", i),
                available_languages: vec![],
                localized_meta: std::collections::HashMap::new(),
            })
            .collect();

        generate_homepage_seo(&output_dir, &config, &posts).unwrap();

        // Page 1 exists and has first 10 posts
        let page1 = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(page1.contains("post-0"));
        assert!(page1.contains("post-9"));
        assert!(!page1.contains("post-10"));
        assert!(page1.contains("rel=\"next\""));
        assert!(!page1.contains("rel=\"prev\""));

        // Page 2 exists
        let page2_path = output_dir.join("page/2/index.html");
        assert!(page2_path.exists());
        let page2 = fs::read_to_string(&page2_path).unwrap();
        assert!(page2.contains("post-10"));
        assert!(page2.contains("post-11"));
        assert!(!page2.contains("post-0"));
        assert!(page2.contains("rel=\"prev\""));
        assert!(!page2.contains("rel=\"next\""));
        assert!(page2.contains("<title>My Blog - Page 2</title>"));
    }

    // ── inject_html_lang tests ─────────────────────────────────────

    #[test]
    fn inject_html_lang_bare_tag() {
        let html = "<!DOCTYPE html>\n<html>\n<head>";
        let result = inject_html_lang(html, "zh");
        assert!(result.contains("<html lang=\"zh\">"));
    }

    #[test]
    fn inject_html_lang_with_existing_attrs() {
        let html = "<!DOCTYPE html>\n<html class=\"no-js\">\n<head>";
        let result = inject_html_lang(html, "ja");
        assert!(result.contains("<html lang=\"ja\" class=\"no-js\">"));
    }

    #[test]
    fn inject_html_lang_no_html_tag() {
        let html = "<head><title>Test</title></head>";
        let result = inject_html_lang(html, "en");
        assert_eq!(result, html);
    }

    #[test]
    fn inject_html_lang_already_present() {
        let html = "<!DOCTYPE html>\n<html lang=\"en\">\n<head>";
        let result = inject_html_lang(html, "ja");
        // Should not double-inject
        assert_eq!(result, html);
    }

    // ── hidden h1 tests ────────────────────────────────────────────

    #[test]
    fn homepage_seo_injects_hidden_h1() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        let posts = vec![sample_post()];
        generate_homepage_seo(&output_dir, &config, &posts).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("<h1 style=\"position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);border:0\">My Blog</h1>"));
    }

    // ── skeleton screen tests ──────────────────────────────────────

    #[test]
    fn homepage_seo_includes_skeleton() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        let posts = vec![sample_post()];
        generate_homepage_seo(&output_dir, &config, &posts).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        // Skeleton exists
        assert!(html.contains("sb-sk"));
        // SEO tags still present
        assert!(html.contains("<main"));
        assert!(html.contains("<article>"));
        assert!(html.contains("<h2>"));
    }

    // ── html lang injection in generated pages ─────────────────────

    #[test]
    fn homepage_seo_injects_html_lang() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        let config = sample_config();
        generate_homepage_seo(&output_dir, &config, &[]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(html.contains("<html lang=\"en\">"));
    }

    #[test]
    fn seo_pages_inject_html_lang() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let mut config = sample_config();
        config.language = Some("ja".to_string());
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();
        assert!(html.contains("<html lang=\"ja\">"));
    }

    #[test]
    fn seo_pages_default_lang_en() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let mut config = sample_config();
        config.language = None;
        let posts = vec![sample_post()];

        generate_seo_pages(&posts, &template_path, &output_dir, &config).unwrap();

        let html = fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();
        assert!(html.contains("<html lang=\"en\">"));
    }
}
