use crate::{PostMetadata, SiteConfig};

pub(crate) const SUPPORTED_LANGUAGES: [&str; 3] = ["en", "zh-CN", "ja"];

pub(crate) fn language_entry_path(language: &str, path: &str) -> String {
    if path == "/" {
        format!("/lang/{language}/")
    } else {
        format!("{}/lang/{language}/", path.trim_end_matches('/'))
    }
}

pub(crate) fn default_language(config: &SiteConfig) -> &'static str {
    config
        .language
        .as_deref()
        .and_then(|language| {
            SUPPORTED_LANGUAGES
                .iter()
                .copied()
                .find(|supported| supported.eq_ignore_ascii_case(language))
        })
        .unwrap_or("en")
}

pub(crate) fn localized_languages<'a>(
    post: &'a PostMetadata,
    default_language: &str,
) -> impl Iterator<Item = &'static str> + 'a {
    let default_language = default_language.to_string();
    SUPPORTED_LANGUAGES.iter().copied().filter(move |language| {
        *language != default_language && post.localized_meta.contains_key(*language)
    })
}

pub(crate) fn published_languages<'a>(
    posts: &'a [PostMetadata],
    default_language: &str,
) -> impl Iterator<Item = &'static str> + 'a {
    let default_language = default_language.to_string();
    SUPPORTED_LANGUAGES.iter().copied().filter(move |language| {
        *language != default_language
            && posts
                .iter()
                .any(|post| post.localized_meta.contains_key(*language))
    })
}
