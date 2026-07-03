//! Blog post manifest generation.
//!
//! Scans a posts directory, parses frontmatter, and produces
//! `manifest.json` sorted by date descending. Optionally copies
//! Markdown source files to an output directory.

use std::fs;
use std::path::Path;

use log::{debug, warn};

use crate::error::EngineError;
use crate::frontmatter::parse_frontmatter;
use crate::timezone::{format_date_with_tz, resolve_timezone};
use crate::PostMetadata;
use crate::SiteConfig;

// ── Summary extraction ─────────────────────────────────────────────

/// Strip common Markdown syntax and return plain text.
///
/// Procedural implementation that avoids pulling in the `regex` crate.
/// Mirrors the TypeScript `getSummary` helper: images, links, code
/// blocks, headings, bold, italic are removed and newlines collapsed.
fn strip_markdown(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // --- Fenced code blocks: ```...``` ---
        if i + 2 < len && chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            // Skip to closing ```
            i += 3;
            loop {
                if i + 2 < len && chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
                    i += 3;
                    break;
                }
                if i >= len {
                    break;
                }
                i += 1;
            }
            continue;
        }

        // --- Inline code: `...` ---
        if chars[i] == '`' {
            i += 1;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            if i < len {
                i += 1; // skip closing `
            }
            continue;
        }

        // --- Images: ![alt](url) ---
        if chars[i] == '!' && i + 1 < len && chars[i + 1] == '[' {
            // Skip ![...](...) entirely
            i += 2;
            // skip to ]
            while i < len && chars[i] != ']' {
                i += 1;
            }
            if i < len {
                i += 1; // skip ]
            }
            // skip (...)
            if i < len && chars[i] == '(' {
                i += 1;
                while i < len && chars[i] != ')' {
                    i += 1;
                }
                if i < len {
                    i += 1; // skip )
                }
            }
            continue;
        }

        // --- Links: [text](url) → keep text ---
        if chars[i] == '[' {
            let start = i + 1;
            i += 1;
            while i < len && chars[i] != ']' {
                i += 1;
            }
            let text_end = i;
            if i < len {
                i += 1; // skip ]
            }
            if i < len && chars[i] == '(' {
                // It's a link — emit the text portion
                let link_text: String = chars[start..text_end].iter().collect();
                result.push_str(&link_text);
                i += 1;
                while i < len && chars[i] != ')' {
                    i += 1;
                }
                if i < len {
                    i += 1; // skip )
                }
                continue;
            } else {
                // Not a link, emit the [ and text
                result.push('[');
                let text: String = chars[start..text_end].iter().collect();
                result.push_str(&text);
                // i is already past ]
                continue;
            }
        }

        // --- Headings at start of line: # ... ---
        if chars[i] == '#' {
            // Check if at start of string or after newline
            let at_line_start = i == 0 || chars[i - 1] == '\n';
            if at_line_start {
                while i < len && chars[i] == '#' {
                    i += 1;
                }
                // skip space after #
                if i < len && chars[i] == ' ' {
                    i += 1;
                }
                continue;
            }
        }

        // --- Bold: **text** or __text__ ---
        // Only strip if a matching closing marker exists (mirrors TS
        // regex which simply doesn't match unclosed markers).
        if i + 1 < len
            && ((chars[i] == '*' && chars[i + 1] == '*')
                || (chars[i] == '_' && chars[i + 1] == '_'))
        {
            let marker = chars[i];
            // Look ahead for closing marker pair.
            let mut j = i + 2;
            let mut found_close = false;
            while j + 1 < len {
                if chars[j] == marker && chars[j + 1] == marker {
                    found_close = true;
                    break;
                }
                j += 1;
            }
            if found_close {
                // Emit inner text, skip both marker pairs.
                let inner: String = chars[i + 2..j].iter().collect();
                result.push_str(&inner);
                i = j + 2;
                continue;
            }
            // No closing marker — emit the opening markers literally
            // (TS regex would not match, leaving them in place).
            result.push(marker);
            result.push(marker);
            i += 2;
            continue;
        }

        // --- Italic: *text* or _text_ (single) ---
        // Same principle: only strip when a closing marker exists.
        if (chars[i] == '*' || chars[i] == '_')
            && i + 1 < len
            && chars[i + 1] != ' '
            && chars[i + 1] != chars[i]
        {
            let marker = chars[i];
            // Look ahead for closing marker.
            let mut j = i + 1;
            let mut found_close = false;
            while j < len {
                if chars[j] == marker {
                    found_close = true;
                    break;
                }
                j += 1;
            }
            if found_close {
                let inner: String = chars[i + 1..j].iter().collect();
                result.push_str(&inner);
                i = j + 1;
                continue;
            }
            // No closing marker — emit literally.
            result.push(marker);
            i += 1;
            continue;
        }

        // --- Newlines → space ---
        // Match TS behaviour: `.replace(/\n+/g, ' ')` only collapses
        // `\n` runs into a single space; `\r` is left as-is so that
        // Windows `\r\n` produces `\r ` (same as the TS golden output).
        if chars[i] == '\n' {
            while i < len && chars[i] == '\n' {
                i += 1;
            }
            result.push(' ');
            continue;
        }

        result.push(chars[i]);
        i += 1;
    }

    result
}

/// Build a plain-text summary from the Markdown body.
///
/// Truncation uses **character count** (not byte count) to match the
/// TypeScript `plainText.substring(0, length)` behaviour. JS
/// `substring` counts UTF-16 code units, which for all BMP characters
/// (including CJK) equals the Unicode scalar count that Rust's
/// `chars()` iterator produces.
fn build_summary(body: &str, max_chars: usize) -> String {
    let plain = strip_markdown(body);
    let trimmed = plain.trim();
    let char_count = trimmed.chars().count();
    if char_count > max_chars {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{}...", truncated)
    } else {
        trimmed.to_string()
    }
}


// ── Filename parsing (i18n) ─────────────────────────────────────────

/// 解析 Markdown 文件名，提取 slug 和可选的语言代码。
///
/// 命名规范: `{slug}.md` (默认) 或 `{slug}.{lang}.md` (本地化)
/// 语言代码遵循 BCP 47 格式，如 `zh-CN`、`en`、`ja`。
///
/// 返回 (slug, Option<language_code>)
pub fn parse_post_filename(filename: &str) -> (String, Option<String>) {
    // 去掉 .md 后缀
    let stem = filename.strip_suffix(".md").unwrap_or(filename);

    // 尝试匹配 BCP 47 语言代码模式: {slug}.{lang}
    // 策略: 从最后一个 '.' 分割，检查后半部分是否为有效的 BCP 47 语言标签
    if let Some(dot_pos) = stem.rfind('.') {
        let potential_lang = &stem[dot_pos + 1..];
        if is_bcp47_language_tag(potential_lang) {
            let slug = stem[..dot_pos].to_string();
            return (slug, Some(potential_lang.to_string()));
        }
    }

    (stem.to_string(), None)
}

/// 简单的 BCP 47 语言标签验证。
/// 支持: "en", "zh", "ja", "zh-CN", "pt-BR", "en-US" 等。
fn is_bcp47_language_tag(s: &str) -> bool {
    // 2-3 字母的主语言标签，可选 '-' + 2-8 字母/数字的子标签
    let parts: Vec<&str> = s.split('-').collect();
    if parts.is_empty() || parts.len() > 3 {
        return false;
    }
    // 主标签: 2-3 个 ASCII 字母
    let primary = parts[0];
    if primary.len() < 2 || primary.len() > 3 || !primary.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    // 子标签: 2-8 个 ASCII 字母或数字
    for subtag in &parts[1..] {
        if subtag.len() < 2
            || subtag.len() > 8
            || !subtag.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return false;
        }
    }
    true
}

// ── Internal grouping structs (i18n) ───────────────────────────────

use std::collections::HashMap;

/// Information about a single post file (used during grouping).
struct PostFileInfo {
    filename: String,
    frontmatter: crate::frontmatter::FrontmatterData,
    body: String,
}

/// A group of files for the same slug (default + localized versions).
struct PostGroup {
    default_file: Option<PostFileInfo>,
    localized_files: Vec<(String, PostFileInfo)>, // (lang_code, file_info)
}

// ── Public API ─────────────────────────────────────────────────────

/// Generate the posts manifest and copy Markdown files to the output directory.
///
/// Use this for production builds where `output_dir` is `dist/`.
/// Generates `{output_dir}/generated/manifest.json` and copies all
/// relevant `.md` files to `{output_dir}/posts/`.
///
/// # Arguments
///
/// * `posts_dir`  – Directory containing `*.md` files.
/// * `output_dir` – Root output directory (e.g. `dist/`).
/// * `config`     – Site configuration (used for timezone).
///
/// # Errors
///
/// Returns [`EngineError::DirectoryNotFound`] if `posts_dir` does not exist.
pub fn generate_posts_data(
    posts_dir: &Path,
    output_dir: &Path,
    config: &SiteConfig,
) -> Result<Vec<PostMetadata>, EngineError> {
    generate_posts_internal(posts_dir, output_dir, config, true)
}

/// Generate only the posts manifest, without copying Markdown files.
///
/// Use this for dev mode where a Vite plugin serves `.md` files directly
/// from the source directory. Only writes `{output_dir}/generated/manifest.json`.
///
/// # Arguments
///
/// * `posts_dir`  – Directory containing `*.md` files.
/// * `output_dir` – Root output directory (e.g. `public/`).
/// * `config`     – Site configuration (used for timezone).
///
/// # Errors
///
/// Returns [`EngineError::DirectoryNotFound`] if `posts_dir` does not exist.
pub fn generate_posts_manifest_only(
    posts_dir: &Path,
    output_dir: &Path,
    config: &SiteConfig,
) -> Result<Vec<PostMetadata>, EngineError> {
    generate_posts_internal(posts_dir, output_dir, config, false)
}

/// Internal implementation: generates manifest and optionally copies files.
fn generate_posts_internal(
    posts_dir: &Path,
    output_dir: &Path,
    config: &SiteConfig,
    copy_files: bool,
) -> Result<Vec<PostMetadata>, EngineError> {
    if !posts_dir.exists() {
        return Err(EngineError::DirectoryNotFound(posts_dir.to_path_buf()));
    }

    // Pre-resolve timezone once for the whole loop.
    let tz = config
        .timezone
        .as_deref()
        .and_then(resolve_timezone);

    // Collect .md files
    let mut md_files: Vec<String> = Vec::new();
    for entry in fs::read_dir(posts_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".md") {
            md_files.push(name);
        }
    }
    md_files.sort(); // deterministic iteration order

    // ── Phase 1: Parse files and group by slug ─────────────────────

    let mut groups: HashMap<String, PostGroup> = HashMap::new();

    for file_name in &md_files {
        let file_path = posts_dir.join(file_name);
        let content = fs::read_to_string(&file_path).map_err(|e| {
            warn!("Failed to read {}: {}", file_name, e);
            e
        })?;

        let (slug, lang) = parse_post_filename(file_name);

        let (fm, body) = match parse_frontmatter(&content, file_name) {
            Ok(pair) => pair,
            Err(e) => {
                warn!("Skipping {} due to frontmatter error: {}", file_name, e);
                continue;
            }
        };

        let file_info = PostFileInfo {
            filename: file_name.clone(),
            frontmatter: fm,
            body: body.to_string(),
        };

        let group = groups.entry(slug).or_insert_with(|| PostGroup {
            default_file: None,
            localized_files: Vec::new(),
        });

        match lang {
            None => {
                group.default_file = Some(file_info);
            }
            Some(lang_code) => {
                group.localized_files.push((lang_code, file_info));
            }
        }
    }

    // ── Phase 2: Build manifest and determine which files to copy ──

    let mut posts: Vec<PostMetadata> = Vec::new();
    let mut files_to_copy: Vec<String> = Vec::new();

    // Sort slugs for deterministic output
    let mut slugs: Vec<String> = groups.keys().cloned().collect();
    slugs.sort();

    for slug in &slugs {
        let group = groups.remove(slug).unwrap();

        match group.default_file {
            None => {
                // Only localized files, no default file → warn and skip
                warn!(
                    "No default file for slug '{}', localized files will be skipped",
                    slug
                );
                // Do NOT copy any files for this slug, do NOT add to manifest
            }
            Some(default_info) => {
                // Log coexistence of default + localized files (this is normal usage)
                for (lang_code, _) in &group.localized_files {
                    debug!(
                        "Slug '{}': default file coexists with .{}.md localized variant",
                        slug, lang_code
                    );
                }

                // Build available_languages (sorted)
                let mut available_languages: Vec<String> = group
                    .localized_files
                    .iter()
                    .map(|(lang, _)| lang.clone())
                    .collect();
                available_languages.sort();

                // Format date using timezone module
                let date_str = match &default_info.frontmatter.date {
                    Some(d) => format_date_with_tz(d, tz),
                    None => String::new(),
                };

                // Summary: prefer frontmatter preview/description/excerpt, fall back to body
                let summary = default_info
                    .frontmatter
                    .preview
                    .clone()
                    .or(default_info.frontmatter.description.clone())
                    .or(default_info.frontmatter.excerpt.clone())
                    .unwrap_or_else(|| build_summary(&default_info.body, 140));

                let title = default_info
                    .frontmatter
                    .title
                    .clone()
                    .unwrap_or_else(|| slug.clone());

                // Build localized_meta from each localized file's frontmatter
                let mut localized_meta: HashMap<String, crate::LocalizedPostMeta> = HashMap::new();
                for (lang_code, localized_info) in &group.localized_files {
                    // Title: use localized frontmatter title, fall back to default file's title
                    let loc_title = localized_info
                        .frontmatter
                        .title
                        .clone()
                        .unwrap_or_else(|| title.clone());

                    // Summary: use localized frontmatter preview/description/excerpt,
                    // fall back to localized body's first 140 chars,
                    // fall back to default file's summary
                    let loc_summary = localized_info
                        .frontmatter
                        .preview
                        .clone()
                        .or(localized_info.frontmatter.description.clone())
                        .or(localized_info.frontmatter.excerpt.clone())
                        .unwrap_or_else(|| {
                            let body_summary = build_summary(&localized_info.body, 140);
                            if body_summary.is_empty() {
                                summary.clone()
                            } else {
                                body_summary
                            }
                        });

                    // Tags: use localized frontmatter tags if non-empty, otherwise omit (will fall back to default)
                    let loc_tags = if localized_info.frontmatter.tags.is_empty() {
                        Vec::new()
                    } else {
                        localized_info.frontmatter.tags.clone()
                    };

                    // Categories: use localized frontmatter categories if non-empty, otherwise omit
                    let loc_categories = if localized_info.frontmatter.categories.is_empty() {
                        Vec::new()
                    } else {
                        localized_info.frontmatter.categories.clone()
                    };

                    localized_meta.insert(
                        lang_code.clone(),
                        crate::LocalizedPostMeta {
                            title: loc_title,
                            summary: loc_summary,
                            tags: loc_tags,
                            categories: loc_categories,
                        },
                    );
                }

                posts.push(PostMetadata {
                    slug: slug.clone(),
                    title,
                    date: date_str,
                    tags: default_info.frontmatter.tags,
                    categories: default_info.frontmatter.categories,
                    summary,
                    available_languages,
                    localized_meta,
                });

                // Mark all files for copying (default + localized)
                files_to_copy.push(default_info.filename);
                for (_, localized_info) in group.localized_files {
                    files_to_copy.push(localized_info.filename);
                }
            }
        }
    }

    // Sort by date descending (lexicographic on ISO strings works fine).
    posts.sort_by(|a, b| b.date.cmp(&a.date));

    // Write manifest.json
    let manifest_dir = output_dir.join("generated");
    fs::create_dir_all(&manifest_dir)?;
    let manifest_path = manifest_dir.join("manifest.json");
    let json = serde_json::to_string_pretty(&posts)?;
    fs::write(&manifest_path, &json)?;

    // Copy markdown files to output_dir/posts/ (only in full build mode)
    if copy_files {
        let posts_output = output_dir.join("posts");
        fs::create_dir_all(&posts_output)?;
        for file_name in &files_to_copy {
            let src = posts_dir.join(file_name);
            let dst = posts_output.join(file_name);
            fs::copy(&src, &dst)?;
        }
    }

    Ok(posts)
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a temp posts dir with given files.
    fn setup_posts(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, content) in files {
            fs::write(dir.path().join(name), content).unwrap();
        }
        dir
    }

    fn default_config() -> SiteConfig {
        SiteConfig {
            title: "Test".into(),
            description: "Test blog".into(),
            logo: "/logo.png".into(),
            favicon: "/favicon.ico".into(),
            site_url: None,
            author: None,
            language: None,
            timezone: None,
            base_path: Some("/".into()),
        }
    }

    #[test]
    fn generates_manifest_for_single_post() {
        let posts = setup_posts(&[(
            "hello.md",
            "---\ntitle: Hello World\ndate: 2025-01-15 10:30:00\ntags: [intro]\ncategories: [General]\npreview: My first post\n---\nBody here.",
        )]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].slug, "hello");
        assert_eq!(result[0].title, "Hello World");
        assert_eq!(result[0].date, "2025-01-15T10:30:00");
        assert_eq!(result[0].tags, vec!["intro"]);
        assert_eq!(result[0].categories, vec!["General"]);
        assert_eq!(result[0].summary, "My first post");

        // manifest.json should exist
        let manifest = out.path().join("generated/manifest.json");
        assert!(manifest.exists());

        // Markdown file should be copied
        let copied = out.path().join("posts/hello.md");
        assert!(copied.exists());
    }

    #[test]
    fn sorts_posts_by_date_descending() {
        let posts = setup_posts(&[
            (
                "old.md",
                "---\ntitle: Old\ndate: 2024-01-01 00:00:00\n---\n",
            ),
            (
                "new.md",
                "---\ntitle: New\ndate: 2025-06-01 00:00:00\n---\n",
            ),
            (
                "mid.md",
                "---\ntitle: Mid\ndate: 2025-01-01 00:00:00\n---\n",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result[0].slug, "new");
        assert_eq!(result[1].slug, "mid");
        assert_eq!(result[2].slug, "old");
    }

    #[test]
    fn uses_slug_as_title_when_missing() {
        let posts = setup_posts(&[("no-title.md", "---\ndate: 2025-01-01\n---\nBody")]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result[0].title, "no-title");
    }

    #[test]
    fn returns_error_for_missing_directory() {
        let out = TempDir::new().unwrap();
        let missing = Path::new("/tmp/nonexistent_posts_dir_12345");

        let result = generate_posts_data(missing, out.path(), &default_config());

        assert!(result.is_err());
        match result.unwrap_err() {
            EngineError::DirectoryNotFound(_) => {}
            other => panic!("Expected DirectoryNotFound, got: {:?}", other),
        }
    }

    #[test]
    fn handles_empty_posts_directory() {
        let posts = setup_posts(&[]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert!(result.is_empty());
        // manifest.json should still be created (empty array)
        let manifest = out.path().join("generated/manifest.json");
        assert!(manifest.exists());
        let content = fs::read_to_string(manifest).unwrap();
        assert_eq!(content.trim(), "[]");
    }

    #[test]
    fn ignores_non_md_files() {
        let posts = setup_posts(&[
            ("post.md", "---\ntitle: Post\ndate: 2025-01-01\n---\n"),
            ("readme.txt", "not a post"),
            ("image.png", "binary data"),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].slug, "post");
    }

    #[test]
    fn applies_timezone_conversion() {
        let posts = setup_posts(&[(
            "tz.md",
            "---\ntitle: TZ Post\ndate: 2025-01-01T09:00:00+00:00\n---\n",
        )]);
        let out = TempDir::new().unwrap();

        let mut config = default_config();
        config.timezone = Some("Asia/Tokyo".into());

        let result = generate_posts_data(posts.path(), out.path(), &config).unwrap();

        assert_eq!(result[0].date, "2025-01-01T18:00:00");
    }

    #[test]
    fn summary_falls_back_to_body() {
        let posts = setup_posts(&[(
            "no-preview.md",
            "---\ntitle: No Preview\ndate: 2025-01-01\n---\nThis is the body content of the post.",
        )]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert!(result[0].summary.contains("This is the body content"));
    }

    #[test]
    fn prefers_preview_over_description() {
        let posts = setup_posts(&[(
            "both.md",
            "---\ntitle: Both\ndate: 2025-01-01\npreview: The preview\ndescription: The description\n---\nBody",
        )]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result[0].summary, "The preview");
    }

    #[test]
    fn uses_description_when_no_preview() {
        let posts = setup_posts(&[(
            "desc.md",
            "---\ntitle: Desc\ndate: 2025-01-01\ndescription: The description\n---\nBody",
        )]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result[0].summary, "The description");
    }

    // ── strip_markdown tests ───────────────────────────────────────

    #[test]
    fn strip_markdown_removes_headings() {
        let input = "## Hello\nWorld";
        let result = strip_markdown(input);
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
        assert!(!result.contains("##"));
    }

    #[test]
    fn strip_markdown_removes_images() {
        let input = "Before ![alt](url) After";
        let result = strip_markdown(input);
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
        assert!(!result.contains("alt"));
        assert!(!result.contains("url"));
    }

    #[test]
    fn strip_markdown_keeps_link_text() {
        let input = "Click [here](https://example.com) now";
        let result = strip_markdown(input);
        assert!(result.contains("here"));
        assert!(!result.contains("https://example.com"));
    }

    #[test]
    fn strip_markdown_removes_bold() {
        let input = "This is **bold** text";
        let result = strip_markdown(input);
        assert!(result.contains("bold"));
        assert!(!result.contains("**"));
    }

    #[test]
    fn strip_markdown_removes_code_blocks() {
        let input = "Before\n```\ncode here\n```\nAfter";
        let result = strip_markdown(input);
        assert!(result.contains("Before"));
        assert!(result.contains("After"));
        assert!(!result.contains("code here"));
    }

    #[test]
    fn build_summary_truncates_long_text() {
        let long_body = "A".repeat(200);
        let summary = build_summary(&long_body, 140);
        assert!(summary.ends_with("..."));
        // 140 chars + "..."
        assert_eq!(summary.chars().count(), 143);
    }

    #[test]
    fn build_summary_truncates_by_char_not_byte() {
        // Each CJK character is 3 bytes in UTF-8 but 1 char.
        // TS substring(0, 140) would keep 140 CJK characters.
        let cjk_body = "你".repeat(200);
        let summary = build_summary(&cjk_body, 140);
        assert!(summary.ends_with("..."));
        // Should be exactly 140 CJK chars + "..."
        assert_eq!(summary.chars().count(), 143);
        // And NOT truncated at 140 bytes (which would be ~46 chars).
        let without_dots = summary.trim_end_matches("...");
        assert_eq!(without_dots.chars().count(), 140);
    }

    #[test]
    fn build_summary_no_truncation_for_short_text() {
        let summary = build_summary("Short body", 140);
        assert_eq!(summary, "Short body");
        assert!(!summary.contains("..."));
    }

    // ── unclosed marker tests ──────────────────────────────────────

    #[test]
    fn strip_markdown_unclosed_bold_preserved() {
        // TS regex /(\*\*|__)(.*?)\1/g won't match unclosed **
        let input = "This is **unclosed bold";
        let result = strip_markdown(input);
        assert!(result.contains("**"), "unclosed ** should be preserved, got: {}", result);
        assert!(result.contains("unclosed bold"));
    }

    #[test]
    fn strip_markdown_unclosed_italic_preserved() {
        let input = "This is *unclosed italic";
        let result = strip_markdown(input);
        assert!(result.contains("*"), "unclosed * should be preserved, got: {}", result);
        assert!(result.contains("unclosed italic"));
    }

    // ── parse_post_filename tests ──────────────────────────────────

    #[test]
    fn parse_filename_default() {
        assert_eq!(parse_post_filename("about.md"), ("about".into(), None));
    }

    #[test]
    fn parse_filename_with_lang() {
        assert_eq!(
            parse_post_filename("about.zh-CN.md"),
            ("about".into(), Some("zh-CN".into()))
        );
    }

    #[test]
    fn parse_filename_numeric_suffix_not_lang() {
        // "about.123.md" — 123 is not a valid BCP 47 tag
        assert_eq!(parse_post_filename("about.123.md"), ("about.123".into(), None));
    }

    #[test]
    fn parse_filename_slug_with_dots() {
        // "my.post.en.md" — "en" is a valid lang, slug is "my.post"
        assert_eq!(
            parse_post_filename("my.post.en.md"),
            ("my.post".into(), Some("en".into()))
        );
    }

    #[test]
    fn parse_filename_no_md_suffix() {
        // Edge case: filename without .md suffix
        assert_eq!(parse_post_filename("about"), ("about".into(), None));
    }

    #[test]
    fn parse_filename_lang_ja() {
        assert_eq!(
            parse_post_filename("hello-world.ja.md"),
            ("hello-world".into(), Some("ja".into()))
        );
    }

    #[test]
    fn parse_filename_lang_pt_br() {
        assert_eq!(
            parse_post_filename("intro.pt-BR.md"),
            ("intro".into(), Some("pt-BR".into()))
        );
    }

    // ── is_bcp47_language_tag tests ────────────────────────────────

    #[test]
    fn bcp47_valid_two_letter() {
        assert!(is_bcp47_language_tag("en"));
        assert!(is_bcp47_language_tag("zh"));
        assert!(is_bcp47_language_tag("ja"));
    }

    #[test]
    fn bcp47_valid_three_letter() {
        assert!(is_bcp47_language_tag("yue"));
    }

    #[test]
    fn bcp47_valid_with_region() {
        assert!(is_bcp47_language_tag("zh-CN"));
        assert!(is_bcp47_language_tag("pt-BR"));
        assert!(is_bcp47_language_tag("en-US"));
    }

    #[test]
    fn bcp47_invalid_single_letter() {
        assert!(!is_bcp47_language_tag("a"));
    }

    #[test]
    fn bcp47_invalid_numeric() {
        assert!(!is_bcp47_language_tag("123"));
    }

    #[test]
    fn bcp47_invalid_too_many_parts() {
        assert!(!is_bcp47_language_tag("en-US-extra-long"));
    }

    #[test]
    fn bcp47_invalid_empty() {
        assert!(!is_bcp47_language_tag(""));
    }

    // ── i18n generate_posts_data tests ─────────────────────────────

    #[test]
    fn i18n_no_localized_files() {
        // Only default files, no localized files → available_languages should be empty
        let posts = setup_posts(&[
            (
                "about.md",
                "---\ntitle: About\ndate: 2025-01-01 00:00:00\n---\nDefault content",
            ),
            (
                "hello.md",
                "---\ntitle: Hello\ndate: 2025-01-02 00:00:00\n---\nHello content",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result.len(), 2);
        for post in &result {
            assert!(
                post.available_languages.is_empty(),
                "slug '{}' should have empty available_languages",
                post.slug
            );
        }

        // All files should be copied
        let posts_output = out.path().join("posts");
        assert!(posts_output.join("about.md").exists());
        assert!(posts_output.join("hello.md").exists());
    }

    #[test]
    fn i18n_with_localized_files() {
        // Default file + localized files for same slug
        let posts = setup_posts(&[
            (
                "about.md",
                "---\ntitle: About\ndate: 2025-01-01 00:00:00\n---\nDefault content",
            ),
            (
                "about.zh-CN.md",
                "---\ntitle: 关于\ndate: 2025-01-01 00:00:00\n---\n中文内容",
            ),
            (
                "about.ja.md",
                "---\ntitle: アバウト\ndate: 2025-01-01 00:00:00\n---\n日本語コンテンツ",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        // Should have exactly one entry in manifest
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].slug, "about");
        assert_eq!(result[0].title, "About"); // from default file
        // available_languages should be sorted alphabetically
        assert_eq!(result[0].available_languages, vec!["ja", "zh-CN"]);

        // All files should be copied
        let posts_output = out.path().join("posts");
        assert!(posts_output.join("about.md").exists());
        assert!(posts_output.join("about.zh-CN.md").exists());
        assert!(posts_output.join("about.ja.md").exists());
    }

    #[test]
    fn i18n_only_localized_no_default() {
        // Only localized files, no default file → should warn and skip
        let posts = setup_posts(&[
            (
                "orphan.zh-CN.md",
                "---\ntitle: 孤儿\ndate: 2025-01-01 00:00:00\n---\n中文内容",
            ),
            (
                "orphan.ja.md",
                "---\ntitle: 孤児\ndate: 2025-01-01 00:00:00\n---\n日本語",
            ),
            (
                "valid.md",
                "---\ntitle: Valid\ndate: 2025-01-02 00:00:00\n---\nValid content",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        // Only "valid" should appear in manifest
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].slug, "valid");

        // Orphan files should NOT be copied
        let posts_output = out.path().join("posts");
        assert!(!posts_output.join("orphan.zh-CN.md").exists());
        assert!(!posts_output.join("orphan.ja.md").exists());
        // Valid file should be copied
        assert!(posts_output.join("valid.md").exists());
    }

    #[test]
    fn i18n_naming_conflict_warning() {
        // Default file + localized files exist → should warn about naming conflict but continue
        let posts = setup_posts(&[
            (
                "about.md",
                "---\ntitle: About\ndate: 2025-01-01 00:00:00\n---\nDefault content",
            ),
            (
                "about.en.md",
                "---\ntitle: About EN\ndate: 2025-01-01 00:00:00\n---\nEnglish content",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        // Should still produce a valid manifest entry
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].slug, "about");
        assert_eq!(result[0].title, "About"); // from default file
        assert_eq!(result[0].available_languages, vec!["en"]);

        // Both files should be copied
        let posts_output = out.path().join("posts");
        assert!(posts_output.join("about.md").exists());
        assert!(posts_output.join("about.en.md").exists());
    }

    #[test]
    fn i18n_mixed_slugs() {
        // Mix of slugs: some with only default, some with default+localized, some with only localized
        let posts = setup_posts(&[
            // "simple" - only default
            (
                "simple.md",
                "---\ntitle: Simple\ndate: 2025-01-03 00:00:00\n---\nSimple content",
            ),
            // "multi" - default + localized
            (
                "multi.md",
                "---\ntitle: Multi\ndate: 2025-01-02 00:00:00\n---\nMulti default",
            ),
            (
                "multi.zh-CN.md",
                "---\ntitle: 多语言\ndate: 2025-01-02 00:00:00\n---\n多语言内容",
            ),
            // "orphan" - only localized (no default)
            (
                "orphan.fr.md",
                "---\ntitle: Orphelin\ndate: 2025-01-01 00:00:00\n---\nContenu français",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        // Only "simple" and "multi" should appear in manifest (orphan skipped)
        assert_eq!(result.len(), 2);

        let simple = result.iter().find(|p| p.slug == "simple").unwrap();
        assert!(simple.available_languages.is_empty());

        let multi = result.iter().find(|p| p.slug == "multi").unwrap();
        assert_eq!(multi.available_languages, vec!["zh-CN"]);

        // Verify file copying
        let posts_output = out.path().join("posts");
        assert!(posts_output.join("simple.md").exists());
        assert!(posts_output.join("multi.md").exists());
        assert!(posts_output.join("multi.zh-CN.md").exists());
        // Orphan should NOT be copied
        assert!(!posts_output.join("orphan.fr.md").exists());
    }

    // ── localized_meta tests ───────────────────────────────────────

    #[test]
    fn localized_meta_with_title_and_summary() {
        // Localized files have their own title and summary (via preview)
        let posts = setup_posts(&[
            (
                "hello.md",
                "---\ntitle: Hello World\ndate: 2025-01-01 00:00:00\npreview: English summary\n---\nEnglish body",
            ),
            (
                "hello.zh-CN.md",
                "---\ntitle: 你好世界\npreview: 中文摘要\n---\n中文正文",
            ),
            (
                "hello.ja.md",
                "---\ntitle: ハローワールド\ndescription: 日本語の要約\n---\n日本語本文",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result.len(), 1);
        let post = &result[0];
        assert_eq!(post.slug, "hello");
        assert_eq!(post.title, "Hello World");
        assert_eq!(post.summary, "English summary");

        // localized_meta should have entries for zh-CN and ja
        assert_eq!(post.localized_meta.len(), 2);

        let zh = post.localized_meta.get("zh-CN").unwrap();
        assert_eq!(zh.title, "你好世界");
        assert_eq!(zh.summary, "中文摘要");

        let ja = post.localized_meta.get("ja").unwrap();
        assert_eq!(ja.title, "ハローワールド");
        assert_eq!(ja.summary, "日本語の要約");
    }

    #[test]
    fn localized_meta_title_fallback_to_default() {
        // Localized file has no title → should fall back to default file's title
        let posts = setup_posts(&[
            (
                "about.md",
                "---\ntitle: About Me\ndate: 2025-01-01 00:00:00\npreview: Default summary\n---\nDefault body",
            ),
            (
                "about.zh-CN.md",
                "---\npreview: 中文摘要\n---\n中文正文",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result.len(), 1);
        let post = &result[0];

        let zh = post.localized_meta.get("zh-CN").unwrap();
        // Title should fall back to default file's title
        assert_eq!(zh.title, "About Me");
        // Summary should use the localized file's preview
        assert_eq!(zh.summary, "中文摘要");
    }

    #[test]
    fn localized_meta_summary_fallback_to_body() {
        // Localized file has no preview/description/excerpt → should use body's first 140 chars
        let posts = setup_posts(&[
            (
                "post.md",
                "---\ntitle: Post\ndate: 2025-01-01 00:00:00\npreview: Default summary\n---\nDefault body",
            ),
            (
                "post.zh-CN.md",
                "---\ntitle: 文章\n---\n这是中文正文内容，用于测试摘要回退逻辑。",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        let post = &result[0];
        let zh = post.localized_meta.get("zh-CN").unwrap();
        assert_eq!(zh.title, "文章");
        // Summary should be extracted from the localized body
        assert!(zh.summary.contains("这是中文正文内容"));
    }

    #[test]
    fn localized_meta_summary_fallback_to_default_when_body_empty() {
        // Localized file has no preview/description/excerpt AND empty body → fall back to default summary
        let posts = setup_posts(&[
            (
                "post.md",
                "---\ntitle: Post\ndate: 2025-01-01 00:00:00\npreview: Default summary\n---\nDefault body",
            ),
            (
                "post.ja.md",
                "---\ntitle: 記事\n---\n",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        let post = &result[0];
        let ja = post.localized_meta.get("ja").unwrap();
        assert_eq!(ja.title, "記事");
        // Summary should fall back to default file's summary since body is empty
        assert_eq!(ja.summary, "Default summary");
    }

    #[test]
    fn localized_meta_empty_when_no_localized_files() {
        // No localized files → localized_meta should be empty
        let posts = setup_posts(&[
            (
                "solo.md",
                "---\ntitle: Solo Post\ndate: 2025-01-01 00:00:00\npreview: Solo summary\n---\nSolo body",
            ),
        ]);
        let out = TempDir::new().unwrap();

        let result = generate_posts_data(posts.path(), out.path(), &default_config()).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result[0].localized_meta.is_empty());
    }
}
