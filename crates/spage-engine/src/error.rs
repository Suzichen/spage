//! Unified error types for the engine.
//!
//! Includes a `From<EngineError>` impl for `napi::Error` so that the
//! NAPI binding layer can use `?` directly without boilerplate conversions.

use std::path::PathBuf;

/// All errors that the engine can produce.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),

    #[error("Failed to parse frontmatter in {file}: {reason}")]
    FrontmatterParse { file: String, reason: String },

    #[error("Invalid date format in {file}: {date}")]
    InvalidDate { file: String, date: String },

    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),

    #[error("Failed to decode image {file}: {reason}")]
    ImageDecode { file: String, reason: String },

    #[error("Invalid album directory name: {0}")]
    InvalidAlbumName(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Configuration file not found: {0}")]
    ConfigNotFound(PathBuf),

    #[error("Build step '{step}' failed: {reason}")]
    BuildStepFailed { step: String, reason: String },

    #[error("Port {port} is already in use")]
    PortInUse { port: u16 },

    #[error("Serve directory not found: {0}\nHint: run `spage build` first")]
    ServeDirNotFound(PathBuf),

    #[error("Spage project declaration not found: {0}\nHint: add a `spage` object to package.json or install the legacy @s-page/core dependency")]
    ProjectDeclarationNotFound(PathBuf),

    #[error("Invalid Spage package spec `{spec}`: {reason}")]
    InvalidPackageSpec { spec: String, reason: String },

    #[error("Package `{name}` was not found in registry {registry}")]
    PackageNotFound { name: String, registry: String },

    #[error("Version `{version}` of package `{name}` was not found in registry {registry}")]
    PackageVersionNotFound {
        name: String,
        version: String,
        registry: String,
    },

    #[error("Failed to fetch package `{package}` from {url}: {reason}")]
    PackageNetwork {
        package: String,
        url: String,
        reason: String,
    },

    #[error("Package `{package}` contains an unsafe archive entry: {path}")]
    UnsafePackageArchive { package: String, path: String },

    #[error("Package cache for `{package}` is incomplete: {reason}")]
    InvalidPackageCache { package: String, reason: String },

    #[error(
        "@s-page/core `{core}` is not compatible with spage-engine `{engine}`\nHint: run `spage update core`"
    )]
    CoreVersionMismatch { core: String, engine: String },

    #[error("Operation cancelled")]
    Cancelled,
}

// ── NAPI conversion ────────────────────────────────────────────────

/// When the `napi` feature is enabled, `EngineError` can be converted
/// directly into `napi::Error` via `?` in binding functions.
#[cfg(feature = "napi")]
impl From<EngineError> for napi::Error {
    fn from(e: EngineError) -> Self {
        napi::Error::from_reason(e.to_string())
    }
}
