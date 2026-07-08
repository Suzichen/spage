//! MIME type resolution and SPA fallback path helpers.
//!
//! Provides utilities for determining Content-Type headers from file extensions
//! and deciding whether a request path should trigger SPA fallback behavior.

/// Resolves a file extension (without the leading dot) to its MIME type.
///
/// Returns `"application/octet-stream"` for unrecognized extensions.
///
/// # Examples
///
/// ```
/// use spage_engine::mime::resolve_mime_type;
///
/// assert_eq!(resolve_mime_type("html"), "text/html");
/// assert_eq!(resolve_mime_type("css"), "text/css");
/// assert_eq!(resolve_mime_type("unknown"), "application/octet-stream");
/// ```
pub fn resolve_mime_type(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "eot" => "application/vnd.ms-fontobject",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Checks whether a URL path has a file extension.
///
/// Used for SPA fallback logic: paths without a file extension should serve
/// `index.html`, while paths with an extension that don't match a file return 404.
///
/// The check looks at the last path segment (after the last `/`) and returns
/// `true` if it contains a dot.
///
/// # Examples
///
/// ```
/// use spage_engine::mime::has_file_extension;
///
/// assert!(has_file_extension("/assets/style.css"));
/// assert!(has_file_extension("/image.png"));
/// assert!(!has_file_extension("/about"));
/// assert!(!has_file_extension("/blog/my-post"));
/// assert!(!has_file_extension("/"));
/// ```
pub fn has_file_extension(path: &str) -> bool {
    let last_segment = path.rsplit('/').next().unwrap_or(path);
    last_segment.contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── resolve_mime_type tests ─────────────────────────────────────

    #[test]
    fn known_extensions_return_correct_mime() {
        assert_eq!(resolve_mime_type("html"), "text/html");
        assert_eq!(resolve_mime_type("htm"), "text/html");
        assert_eq!(resolve_mime_type("css"), "text/css");
        assert_eq!(resolve_mime_type("js"), "application/javascript");
        assert_eq!(resolve_mime_type("mjs"), "application/javascript");
        assert_eq!(resolve_mime_type("json"), "application/json");
        assert_eq!(resolve_mime_type("png"), "image/png");
        assert_eq!(resolve_mime_type("jpg"), "image/jpeg");
        assert_eq!(resolve_mime_type("jpeg"), "image/jpeg");
        assert_eq!(resolve_mime_type("gif"), "image/gif");
        assert_eq!(resolve_mime_type("svg"), "image/svg+xml");
        assert_eq!(resolve_mime_type("ico"), "image/x-icon");
        assert_eq!(resolve_mime_type("webp"), "image/webp");
        assert_eq!(resolve_mime_type("avif"), "image/avif");
        assert_eq!(resolve_mime_type("woff"), "font/woff");
        assert_eq!(resolve_mime_type("woff2"), "font/woff2");
        assert_eq!(resolve_mime_type("ttf"), "font/ttf");
        assert_eq!(resolve_mime_type("eot"), "application/vnd.ms-fontobject");
        assert_eq!(resolve_mime_type("xml"), "application/xml");
        assert_eq!(resolve_mime_type("txt"), "text/plain");
        assert_eq!(resolve_mime_type("mp4"), "video/mp4");
        assert_eq!(resolve_mime_type("webm"), "video/webm");
        assert_eq!(resolve_mime_type("pdf"), "application/pdf");
    }

    #[test]
    fn unknown_extension_returns_octet_stream() {
        assert_eq!(resolve_mime_type("xyz"), "application/octet-stream");
        assert_eq!(resolve_mime_type("foo"), "application/octet-stream");
        assert_eq!(resolve_mime_type(""), "application/octet-stream");
    }

    #[test]
    fn case_insensitive_extension() {
        assert_eq!(resolve_mime_type("HTML"), "text/html");
        assert_eq!(resolve_mime_type("Css"), "text/css");
        assert_eq!(resolve_mime_type("JS"), "application/javascript");
        assert_eq!(resolve_mime_type("PNG"), "image/png");
        assert_eq!(resolve_mime_type("AVIF"), "image/avif");
        assert_eq!(resolve_mime_type("WOFF2"), "font/woff2");
    }

    // ── has_file_extension tests ───────────────────────────────────

    #[test]
    fn paths_with_extension() {
        assert!(has_file_extension("/assets/style.css"));
        assert!(has_file_extension("/image.png"));
        assert!(has_file_extension("/path/to/file.js"));
        assert!(has_file_extension("file.txt"));
        assert!(has_file_extension("/a/b/c.woff2"));
    }

    #[test]
    fn paths_without_extension() {
        assert!(!has_file_extension("/about"));
        assert!(!has_file_extension("/blog/my-post"));
        assert!(!has_file_extension("/"));
        assert!(!has_file_extension(""));
        assert!(!has_file_extension("/path/to/page"));
    }

    #[test]
    fn dot_in_directory_not_in_filename() {
        // A dot in a directory name should not count — only the last segment matters
        assert!(!has_file_extension("/path.with.dots/page"));
        assert!(!has_file_extension("/.hidden/route"));
    }

    #[test]
    fn hidden_files_have_extension() {
        // Files starting with a dot are considered to have an extension
        assert!(has_file_extension("/.htaccess"));
        assert!(has_file_extension("/path/.env"));
    }
}
