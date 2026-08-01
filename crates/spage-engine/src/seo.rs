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
use crate::language::{
    default_language, language_entry_path, localized_languages, published_languages,
};
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

struct SeoLanguageContext<'config, 'path> {
    site_url: Option<&'config str>,
    base_path: &'path str,
    default_language: &'static str,
}

struct HomepagePage<'a> {
    number: usize,
    total: usize,
    language: &'static str,
    published_languages: &'a [&'static str],
}

impl<'config, 'path> SeoLanguageContext<'config, 'path> {
    fn new(config: &'config SiteConfig, base_path: &'path str) -> Self {
        Self {
            site_url: config.site_url.as_deref().filter(|url| !url.is_empty()),
            base_path,
            default_language: default_language(config),
        }
    }

    fn full_url(&self, path: &str) -> String {
        self.site_url
            .map(|site_url| build_full_url(site_url, self.base_path, path))
            .unwrap_or_default()
    }
}

fn localized_post(post: &PostMetadata, language: &str) -> PostMetadata {
    let Some(localized) = post.localized_meta.get(language) else {
        return post.clone();
    };

    let mut localized_post = post.clone();
    localized_post.title = localized.title.clone();
    localized_post.summary = localized.summary.clone();
    if !localized.tags.is_empty() {
        localized_post.tags = localized.tags.clone();
    }
    if !localized.categories.is_empty() {
        localized_post.categories = localized.categories.clone();
    }
    localized_post
}

fn page_path(page: usize, language: &str, default_language: &str) -> String {
    let path = if page == 1 {
        "/".to_string()
    } else {
        format!("/page/{page}/")
    };
    if language == default_language {
        path
    } else {
        language_entry_path(language, &path)
    }
}

fn append_language_alternates(
    out: &mut String,
    context: &SeoLanguageContext<'_, '_>,
    path: &str,
    alternate_languages: &[&str],
) {
    let Some(site_url) = context.site_url else {
        return;
    };

    let default_url = build_full_url(site_url, context.base_path, path);
    out.push_str(&format!(
        "\n  <link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
        context.default_language,
        escape_html(&default_url)
    ));
    for language in alternate_languages {
        let url = build_full_url(
            site_url,
            context.base_path,
            &language_entry_path(language, path),
        );
        out.push_str(&format!(
            "\n  <link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
            language,
            escape_html(&url)
        ));
    }
    out.push_str(&format!(
        "\n  <link rel=\"alternate\" hreflang=\"x-default\" href=\"{}\">",
        escape_html(&default_url)
    ));
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
fn generate_seo_html_for_language(
    post: &PostMetadata,
    config: &SiteConfig,
    context: &SeoLanguageContext<'_, '_>,
    language: &str,
) -> String {
    let title = &post.title;
    let summary = &post.summary;
    let tags = &post.tags;
    let categories = &post.categories;
    let date = &post.date;
    let slug = &post.slug;

    let site_url = context.site_url;
    let author = config.author.as_deref();

    let post_path = format!("/post/{}/", slug);
    let page_path = if language != context.default_language {
        language_entry_path(language, &post_path)
    } else {
        post_path.clone()
    };
    let post_url = context.full_url(&page_path);

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
    let alternate_languages: Vec<_> = localized_languages(post, context.default_language).collect();
    if !alternate_languages.is_empty() {
        append_language_alternates(&mut out, context, &post_path, &alternate_languages);
    }

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
    let default_language = default_language(config);
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
        html = inject_html_lang(&html, default_language);
        html = crate::shell::rewrite_base_path(&html, &base_path);
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
    context: &SeoLanguageContext<'_, '_>,
    page: &HomepagePage<'_>,
) -> String {
    let site_url = context.site_url;
    let author = config.author.as_deref();

    let default_page_path = page_path(
        page.number,
        context.default_language,
        context.default_language,
    );
    let seo_page_path = page_path(page.number, page.language, context.default_language);
    let page_url = context.full_url(&seo_page_path);
    let image_url = match site_url {
        Some(url) => build_full_url(url, context.base_path, &config.logo),
        None => String::new(),
    };

    let title = if page.number == 1 {
        config.title.clone()
    } else {
        format!("{} - Page {}", config.title, page.number)
    };

    let mut out = String::new();

    append_basic_meta(&mut out, &title, &config.description);
    append_indexing_meta(
        &mut out,
        author,
        (!page_url.is_empty()).then_some(page_url.as_str()),
    );
    if page.published_languages.len() > 1 {
        let alternate_languages: Vec<&str> = page
            .published_languages
            .iter()
            .copied()
            .filter(|candidate| *candidate != context.default_language)
            .collect();
        append_language_alternates(&mut out, context, &default_page_path, &alternate_languages);
    }

    // Pagination rel links
    if let Some(url) = site_url {
        if page.number > 1 {
            out.push_str(&format!(
                "\n  <link rel=\"prev\" href=\"{}\">",
                build_full_url(
                    url,
                    context.base_path,
                    &page_path(page.number - 1, page.language, context.default_language)
                )
            ));
        }
        if page.number < page.total {
            out.push_str(&format!(
                "\n  <link rel=\"next\" href=\"{}\">",
                build_full_url(
                    url,
                    context.base_path,
                    &page_path(page.number + 1, page.language, context.default_language)
                )
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
    if page.number == 1 {
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
/// Handles bare tags, existing attributes, and an existing double-quoted `lang`.
fn inject_html_lang(html: &str, lang: &str) -> String {
    let Some(tag_start) = html.find("<html") else {
        return html.to_string();
    };
    let tag = &html[tag_start
        ..html[tag_start..]
            .find('>')
            .map_or(html.len(), |end| tag_start + end)];
    if let Some(attr_start) = tag.find("lang=\"") {
        let value_start = tag_start + attr_start + "lang=\"".len();
        if let Some(value_end) = html[value_start..].find('"') {
            let mut result = html.to_string();
            result.replace_range(value_start..value_start + value_end, lang);
            return result;
        }
    }
    html.replacen("<html", &format!("<html lang=\"{lang}\""), 1)
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
fn generate_post_list_html(
    posts: &[PostMetadata],
    base_path: &str,
    title: &str,
    language: &str,
    default_language: &str,
) -> String {
    let mut out = String::new();
    out.push_str(SKELETON_HTML);
    out.push_str(&format!(
        "\n    <h1 style=\"position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);border:0\">{}</h1>",
        escape_html(title)
    ));
    out.push_str("\n    <main class=\"sb-main\">");
    for post in posts {
        let has_localized_version =
            language != default_language && post.localized_meta.contains_key(language);
        let post_url = if has_localized_version {
            format!("{}/post/{}/lang/{}", base_path, &post.slug, language)
        } else {
            format!("{}/post/{}", base_path, &post.slug)
        };
        let post = localized_post(post, language);
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

fn homepage_output_dir(
    output_dir: &Path,
    page: usize,
    language: &str,
    default_language: &str,
) -> std::path::PathBuf {
    match (language == default_language, page) {
        (true, 1) => output_dir.to_path_buf(),
        (true, _) => output_dir.join("page").join(page.to_string()),
        (false, 1) => output_dir.join("lang").join(language),
        (false, _) => output_dir
            .join("page")
            .join(page.to_string())
            .join("lang")
            .join(language),
    }
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
    let context = SeoLanguageContext::new(config, &base_path);
    let published_languages: Vec<_> = std::iter::once(context.default_language)
        .chain(published_languages(manifest, context.default_language))
        .collect();
    let total_pages = if manifest.is_empty() {
        1
    } else {
        (manifest.len() + POSTS_PER_PAGE - 1) / POSTS_PER_PAGE
    };

    for language in &published_languages {
        for page_number in 1..=total_pages {
            let start = (page_number - 1) * POSTS_PER_PAGE;
            let end = std::cmp::min(start + POSTS_PER_PAGE, manifest.len());
            let page_posts = &manifest[start..end];
            let page = HomepagePage {
                number: page_number,
                total: total_pages,
                language,
                published_languages: &published_languages,
            };
            let head_tags = generate_homepage_head(config, &context, &page);
            let body_content = generate_post_list_html(
                page_posts,
                &base_path,
                &config.title,
                language,
                context.default_language,
            );

            let mut html = template.clone();
            html = inject_html_lang(&html, language);
            html = crate::shell::rewrite_base_path(&html, &base_path);
            html = remove_title_tag(&html);
            html = html.replace("</head>", &format!("{}\n</head>", head_tags));
            html = html.replace(
                "<div id=\"root\"></div>",
                &format!("<div id=\"root\">{}</div>", body_content),
            );

            let page_dir =
                homepage_output_dir(output_dir, page_number, language, context.default_language);
            fs::create_dir_all(&page_dir)?;
            fs::write(page_dir.join("index.html"), html)?;
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
    let context = SeoLanguageContext::new(config, &base_path);

    let post_output_dir = output_dir.join("post");
    fs::create_dir_all(&post_output_dir)?;

    let mut generated: usize = 0;

    for post in manifest {
        for language in std::iter::once(context.default_language)
            .chain(localized_languages(post, context.default_language))
        {
            let localized_post = localized_post(post, language);
            let seo_tags =
                generate_seo_html_for_language(&localized_post, config, &context, language);

            let mut html = template.clone();
            html = inject_html_lang(&html, language);
            html = crate::shell::rewrite_base_path(&html, &base_path);
            html = remove_title_tag(&html);
            html = html.replace("</head>", &format!("{}\n</head>", seo_tags));

            let mut slug_dir = post_output_dir.join(&post.slug);
            if language != context.default_language {
                slug_dir = slug_dir.join("lang").join(language);
            }
            fs::create_dir_all(&slug_dir)?;
            fs::write(slug_dir.join("index.html"), html)?;
            generated += 1;
        }
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

    fn add_translation(post: &mut PostMetadata, language: &str, title: &str) {
        post.available_languages.push(language.to_string());
        post.localized_meta.insert(
            language.to_string(),
            crate::LocalizedPostMeta {
                title: title.to_string(),
                summary: format!("{title} summary"),
                tags: vec![],
                categories: vec![],
            },
        );
    }

    fn minimal_template() -> String {
        r#"<!DOCTYPE html>
<html>
<head>
  <title>App Shell</title>
  <link rel="icon" href="./favicon.ico">
  <link rel="apple-touch-icon" href="./apple-touch-icon.png">
  <link rel="manifest" href="./site.webmanifest">
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
        let head = generate_album_head(&album_config.albums[0], &album_config, &config, "");

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
        let config = sample_config();
        let context = SeoLanguageContext::new(&config, "");
        let head = generate_seo_html_for_language(&post, &config, &context, "en");

        assert!(head.contains("name=\"description\" content=\"First line Second line\""));
        assert!(head.contains("property=\"og:description\" content=\"First line Second line\""));
        assert!(head.contains("name=\"twitter:description\" content=\"First line Second line\""));
        assert!(head.contains("\"description\": \"First line\\nSecond line\""));
    }

    #[test]
    fn generates_default_and_translated_post_seo_pages() {
        let tmp = TempDir::new().unwrap();
        let template_path = tmp.path().join("index.html");
        fs::write(&template_path, minimal_template()).unwrap();

        let output_dir = tmp.path().join("dist");
        let mut config = sample_config();
        config.language = Some("zh-CN".to_string());
        let mut post = sample_post();
        post.title = "你好世界".to_string();
        add_translation(&mut post, "en", "Hello World");
        add_translation(&mut post, "fr", "Bonjour");

        let count = generate_seo_pages(&[post], &template_path, &output_dir, &config).unwrap();
        assert_eq!(count, 2);

        let default_html =
            fs::read_to_string(output_dir.join("post/hello-world/index.html")).unwrap();
        let en_html =
            fs::read_to_string(output_dir.join("post/hello-world/lang/en/index.html")).unwrap();
        assert!(default_html.contains("<html lang=\"zh-CN\">"));
        assert!(default_html.contains("<title>你好世界</title>"));
        assert!(default_html
            .contains("hreflang=\"zh-CN\" href=\"https://example.com/post/hello-world/\""));
        assert!(default_html
            .contains("hreflang=\"x-default\" href=\"https://example.com/post/hello-world/\""));
        assert!(default_html
            .contains("hreflang=\"en\" href=\"https://example.com/post/hello-world/lang/en/\""));
        assert!(en_html.contains("<html lang=\"en\">"));
        assert!(en_html
            .contains("rel=\"canonical\" href=\"https://example.com/post/hello-world/lang/en/\""));
        assert!(!output_dir
            .join("post/hello-world/lang/fr/index.html")
            .exists());
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
        assert!(html.contains("href=\"/apple-touch-icon.png\""));
        assert!(html.contains("href=\"/site.webmanifest\""));
        // No relative paths should remain
        assert!(!html.contains("\"./"));
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
        assert!(html.contains("href=\"/blog/apple-touch-icon.png\""));
        assert!(html.contains("href=\"/blog/site.webmanifest\""));
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

        let mut config = sample_config();
        config.base_path = Some("/blog".to_string());
        config.language = Some("zh-CN".to_string());
        // Create 12 posts to trigger 2 pages
        let mut posts: Vec<PostMetadata> = (0..12)
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
        add_translation(&mut posts[0], "ja", "記事 0");

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

        let ja_page1 = fs::read_to_string(output_dir.join("lang/ja/index.html")).unwrap();
        let ja_page2 = fs::read_to_string(output_dir.join("page/2/lang/ja/index.html")).unwrap();
        assert!(!output_dir.join("lang/en/index.html").exists());
        assert!(!output_dir.join("lang/zh-CN/index.html").exists());
        assert!(page1.contains("hreflang=\"zh-CN\" href=\"https://example.com/blog/\""));
        assert!(page1.contains("hreflang=\"x-default\" href=\"https://example.com/blog/\""));
        assert!(page1.contains("hreflang=\"ja\" href=\"https://example.com/blog/lang/ja/\""));
        assert!(!page1.contains("hreflang=\"en\""));
        assert!(ja_page1.contains("<html lang=\"ja\">"));
        assert!(ja_page1.contains("href=\"/blog/post/post-0/lang/ja\">記事 0</a>"));
        assert!(ja_page1.contains("href=\"https://example.com/blog/lang/ja/\""));
        assert!(ja_page2.contains("rel=\"prev\" href=\"https://example.com/blog/lang/ja/\""));
    }

    #[test]
    fn homepage_seo_skips_languages_without_translated_posts() {
        let tmp = TempDir::new().unwrap();
        let output_dir = tmp.path().join("dist");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("index.html"), minimal_template()).unwrap();

        generate_homepage_seo(&output_dir, &sample_config(), &[sample_post()]).unwrap();

        let html = fs::read_to_string(output_dir.join("index.html")).unwrap();
        assert!(!output_dir.join("lang/ja/index.html").exists());
        assert!(!output_dir.join("lang/zh-CN/index.html").exists());
        assert!(!html.contains("hreflang=\"ja\""));
        assert!(!html.contains("hreflang=\"zh-CN\""));
        assert!(!html.contains("hreflang=\"x-default\""));
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
        assert!(result.contains("<html lang=\"ja\">"));
        assert!(!result.contains("<html lang=\"en\">"));
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
