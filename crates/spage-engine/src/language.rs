use crate::{PostMetadata, SiteConfig};

pub(crate) const SUPPORTED_LANGUAGES: [&str; 3] = ["en", "zh-CN", "ja"];

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
    default_language: &'a str,
) -> impl Iterator<Item = &'static str> + 'a {
    SUPPORTED_LANGUAGES.iter().copied().filter(move |language| {
        *language != default_language && post.localized_meta.contains_key(*language)
    })
}
