//! Spage package declarations, npm registry resolution, and package caching.

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;

const DEFAULT_REGISTRY_URL: &str = "https://registry.npmjs.org";
const CACHE_MARKER: &str = ".spage-complete";
const CORE_PACKAGE_NAME: &str = "@s-page/core";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSpec {
    pub name: String,
    pub version: String,
}

impl PackageSpec {
    pub fn parse(value: &str) -> Result<Self, EngineError> {
        let split = value.rfind('@').filter(|index| *index > 0).ok_or_else(|| {
            EngineError::InvalidPackageSpec {
                spec: value.to_string(),
                reason: "expected <package>@<exact-version>".into(),
            }
        })?;
        let (name, version_with_at) = value.split_at(split);
        let version = &version_with_at[1..];
        validate_package_name(name).map_err(|reason| EngineError::InvalidPackageSpec {
            spec: value.to_string(),
            reason,
        })?;
        Version::parse(version).map_err(|_| EngineError::InvalidPackageSpec {
            spec: value.to_string(),
            reason: "version must be an exact semantic version".into(),
        })?;
        Ok(Self {
            name: name.to_string(),
            version: version.to_string(),
        })
    }

    pub fn as_declaration(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackageResolverOptions {
    /// Directory containing package-name/version cache entries.
    pub cache_dir: Option<PathBuf>,
    pub registry_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpageDeclaration {
    pub requires: String,
    pub core: String,
    #[serde(default)]
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProjectPackageJson {
    #[serde(default)]
    spage: Option<SpageDeclaration>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateTarget {
    All,
    Core,
    Plugins,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateOptions {
    pub work_dir: PathBuf,
    pub target: UpdateTarget,
    pub package_cache_dir: Option<PathBuf>,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        Self {
            work_dir: PathBuf::from("."),
            target: UpdateTarget::All,
            package_cache_dir: None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegistryMetadata {
    #[serde(default)]
    versions: HashMap<String, RegistryVersion>,
    #[serde(rename = "dist-tags", default)]
    dist_tags: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct RegistryVersion {
    dist: RegistryDist,
}

#[derive(Debug, Deserialize)]
struct RegistryDist {
    tarball: String,
}

/// Download and unpack an exact npm package version, or reuse a complete cache entry.
pub fn ensure_package(
    spec: &PackageSpec,
    options: &PackageResolverOptions,
) -> Result<PathBuf, EngineError> {
    validate_package_name(&spec.name).map_err(|reason| EngineError::InvalidPackageSpec {
        spec: spec.as_declaration(),
        reason,
    })?;
    Version::parse(&spec.version).map_err(|_| EngineError::InvalidPackageSpec {
        spec: spec.as_declaration(),
        reason: "version must be an exact semantic version".into(),
    })?;

    let cache_root = options
        .cache_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(".cache/packages"));
    let package_dir = cache_root
        .join(cache_package_name(&spec.name))
        .join(&spec.version);
    if package_dir.join(CACHE_MARKER).is_file() && package_dir.join("package.json").is_file() {
        return Ok(package_dir);
    }

    let registry = registry_url(options);
    let metadata = fetch_metadata(&spec.name, &registry)?;
    let version = metadata.versions.get(&spec.version).ok_or_else(|| {
        EngineError::PackageVersionNotFound {
            name: spec.name.clone(),
            version: spec.version.clone(),
            registry: registry.clone(),
        }
    })?;
    let response =
        attohttpc::get(&version.dist.tarball)
            .send()
            .map_err(|e| EngineError::PackageNetwork {
                package: spec.as_declaration(),
                url: version.dist.tarball.clone(),
                reason: e.to_string(),
            })?;
    if !response.is_success() {
        return Err(EngineError::PackageNetwork {
            package: spec.as_declaration(),
            url: version.dist.tarball.clone(),
            reason: format!("HTTP {}", response.status()),
        });
    }
    let bytes = response.bytes().map_err(|e| EngineError::PackageNetwork {
        package: spec.as_declaration(),
        url: version.dist.tarball.clone(),
        reason: e.to_string(),
    })?;

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)?;
    }
    fs::create_dir_all(&package_dir)?;
    if let Err(error) = unpack_package(&bytes, &package_dir, spec) {
        let _ = fs::remove_dir_all(&package_dir);
        return Err(error);
    }
    if !package_dir.join("package.json").is_file() {
        let _ = fs::remove_dir_all(&package_dir);
        return Err(EngineError::InvalidPackageCache {
            package: spec.as_declaration(),
            reason: "archive does not contain package/package.json".into(),
        });
    }
    fs::write(package_dir.join(CACHE_MARKER), spec.as_declaration())?;
    Ok(package_dir)
}

/// Resolve the project shell and prepare every package declared in `spage.plugins`.
pub fn resolve_project_shell(
    work_dir: &Path,
    shell_dir: Option<&Path>,
    package_cache_dir: Option<&Path>,
) -> Result<PathBuf, EngineError> {
    if let Some(shell_dir) = shell_dir {
        return Ok(resolve_from_work_dir(work_dir, shell_dir));
    }

    let package_path = work_dir.join("package.json");
    let project = read_project_package_json(&package_path)?;
    if let Some(declaration) = project.spage {
        validate_engine_requirement(&declaration.requires)?;
        if project.dependencies.contains_key(CORE_PACKAGE_NAME) {
            eprintln!(
                "Warning: both package.json.spage.core and dependencies[\"@s-page/core\"] are present; using spage.core"
            );
        }
        let core = PackageSpec::parse(&declaration.core)?;
        if core.name != CORE_PACKAGE_NAME {
            return Err(EngineError::InvalidPackageSpec {
                spec: declaration.core,
                reason: format!("spage.core must reference {CORE_PACKAGE_NAME}"),
            });
        }
        let resolver = project_resolver_options(work_dir, package_cache_dir);
        let core_dir = ensure_package(&core, &resolver)?;
        sync_core_schemas(work_dir, &core_dir)?;
        for plugin in declaration.plugins {
            let plugin = PackageSpec::parse(&plugin)?;
            ensure_package(&plugin, &resolver)?;
        }
        return core_shell_dir(&core, &core_dir);
    }

    resolve_legacy_shell(
        work_dir,
        &package_path,
        &project.dependencies,
        package_cache_dir,
    )
}

/// Update declared package versions to the registry's latest dist-tag and warm their caches.
pub fn update_resources(options: UpdateOptions) -> Result<SpageDeclaration, EngineError> {
    update_resources_with_registry(options, None)
}

fn update_resources_with_registry(
    options: UpdateOptions,
    registry_url: Option<String>,
) -> Result<SpageDeclaration, EngineError> {
    let package_path = options.work_dir.join("package.json");
    let raw = fs::read_to_string(&package_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            EngineError::ProjectDeclarationNotFound(package_path.clone())
        } else {
            EngineError::Io(error)
        }
    })?;
    let mut package_json: serde_json::Value = serde_json::from_str(&raw)?;
    let mut declaration: SpageDeclaration = serde_json::from_value(
        package_json
            .get("spage")
            .cloned()
            .ok_or_else(|| EngineError::ProjectDeclarationNotFound(package_path.clone()))?,
    )?;
    validate_engine_requirement(&declaration.requires)?;

    let mut resolver =
        project_resolver_options(&options.work_dir, options.package_cache_dir.as_deref());
    resolver.registry_url = registry_url;
    if matches!(options.target, UpdateTarget::All | UpdateTarget::Core) {
        declaration.core = update_spec(&declaration.core, &resolver)?.as_declaration();
        let core = PackageSpec::parse(&declaration.core)?;
        let core_dir = ensure_package(&core, &resolver)?;
        sync_core_schemas(&options.work_dir, &core_dir)?;
    }
    if matches!(options.target, UpdateTarget::All | UpdateTarget::Plugins) {
        declaration.plugins = declaration
            .plugins
            .iter()
            .map(|plugin| update_spec(plugin, &resolver).map(|spec| spec.as_declaration()))
            .collect::<Result<Vec<_>, _>>()?;
        for plugin in &declaration.plugins {
            ensure_package(&PackageSpec::parse(plugin)?, &resolver)?;
        }
    }

    package_json["spage"] = serde_json::to_value(&declaration)?;
    let mut output = serde_json::to_string_pretty(&package_json)?;
    output.push('\n');
    fs::write(&package_path, output)?;
    Ok(declaration)
}

fn update_spec(
    declaration: &str,
    options: &PackageResolverOptions,
) -> Result<PackageSpec, EngineError> {
    let current = PackageSpec::parse(declaration)?;
    let registry = registry_url(options);
    let metadata = fetch_metadata(&current.name, &registry)?;
    let latest =
        metadata
            .dist_tags
            .get("latest")
            .ok_or_else(|| EngineError::PackageVersionNotFound {
                name: current.name.clone(),
                version: "latest".into(),
                registry,
            })?;
    let updated = PackageSpec {
        name: current.name,
        version: latest.clone(),
    };
    Ok(updated)
}

fn resolve_legacy_shell(
    work_dir: &Path,
    package_path: &Path,
    dependencies: &HashMap<String, String>,
    package_cache_dir: Option<&Path>,
) -> Result<PathBuf, EngineError> {
    let Some(version) = dependencies.get(CORE_PACKAGE_NAME) else {
        return Err(EngineError::ProjectDeclarationNotFound(
            package_path.to_path_buf(),
        ));
    };
    let installed = work_dir.join("node_modules/@s-page/core/dist/shell");
    if installed.is_dir() {
        log::warn!(
            "Using legacy dependencies[\"@s-page/core\"] and node_modules shell; migrate to package.json.spage.core"
        );
        return Ok(installed);
    }

    let exact = version.strip_prefix('=').unwrap_or(version);
    Version::parse(exact).map_err(|_| EngineError::InvalidPackageSpec {
        spec: format!("{CORE_PACKAGE_NAME}@{version}"),
        reason: "legacy version ranges require an installed node_modules shell; migrate to an exact spage.core declaration".into(),
    })?;
    let resolver = project_resolver_options(work_dir, package_cache_dir);
    let spec = PackageSpec {
        name: CORE_PACKAGE_NAME.into(),
        version: exact.into(),
    };
    let package_dir = ensure_package(&spec, &resolver)?;
    sync_core_schemas(work_dir, &package_dir)?;
    core_shell_dir(&spec, &package_dir)
}

fn core_shell_dir(spec: &PackageSpec, package_dir: &Path) -> Result<PathBuf, EngineError> {
    let shell = package_dir.join("dist/shell");
    if !shell.join("index.html").is_file() {
        return Err(EngineError::InvalidPackageCache {
            package: spec.as_declaration(),
            reason: "package does not contain dist/shell/index.html".into(),
        });
    }
    Ok(shell)
}

fn sync_core_schemas(work_dir: &Path, core_dir: &Path) -> Result<(), EngineError> {
    let source = core_dir.join("schemas");
    if !source.is_dir() {
        return Ok(());
    }
    let destination = work_dir.join(".cache/generated/schemas");
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    copy_directory(&source, &destination)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), EngineError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn read_project_package_json(path: &Path) -> Result<ProjectPackageJson, EngineError> {
    let raw = fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            EngineError::ProjectDeclarationNotFound(path.to_path_buf())
        } else {
            EngineError::Io(error)
        }
    })?;
    serde_json::from_str(&raw).map_err(EngineError::Json)
}

fn project_resolver_options(
    work_dir: &Path,
    package_cache_dir: Option<&Path>,
) -> PackageResolverOptions {
    let cache_dir = package_cache_dir
        .map(|path| resolve_from_work_dir(work_dir, path))
        .unwrap_or_else(|| work_dir.join(".cache/packages"));
    PackageResolverOptions {
        cache_dir: Some(cache_dir),
        registry_url: None,
    }
}

fn resolve_from_work_dir(work_dir: &Path, path: &Path) -> PathBuf {
    if path.is_relative() {
        work_dir.join(path)
    } else {
        path.to_path_buf()
    }
}

fn validate_engine_requirement(requirement: &str) -> Result<(), EngineError> {
    let npm_style = requirement
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(", ");
    let required = VersionReq::parse(requirement)
        .or_else(|_| VersionReq::parse(&npm_style))
        .map_err(|_| {
            EngineError::Config(format!(
                "Invalid package.json.spage.requires range: {requirement}"
            ))
        })?;
    let actual = Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version must be semver");
    if !required.matches(&actual) {
        return Err(EngineError::EngineVersionMismatch {
            required: requirement.into(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn fetch_metadata(name: &str, registry: &str) -> Result<RegistryMetadata, EngineError> {
    let url = format!(
        "{}/{}",
        registry.trim_end_matches('/'),
        encode_package_name(name)
    );
    let response = attohttpc::get(&url)
        .send()
        .map_err(|e| EngineError::PackageNetwork {
            package: name.into(),
            url: url.clone(),
            reason: e.to_string(),
        })?;
    if response.status().as_u16() == 404 {
        return Err(EngineError::PackageNotFound {
            name: name.into(),
            registry: registry.into(),
        });
    }
    if !response.is_success() {
        return Err(EngineError::PackageNetwork {
            package: name.into(),
            url,
            reason: format!("HTTP {}", response.status()),
        });
    }
    response.json().map_err(|e| EngineError::PackageNetwork {
        package: name.into(),
        url,
        reason: format!("invalid registry metadata: {e}"),
    })
}

fn unpack_package(bytes: &[u8], output: &Path, spec: &PackageSpec) -> Result<(), EngineError> {
    let decoder = GzDecoder::new(Cursor::new(bytes));
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| EngineError::InvalidPackageCache {
            package: spec.as_declaration(),
            reason: format!("cannot read tarball: {e}"),
        })?;
    for entry in entries {
        let mut entry = entry.map_err(|e| EngineError::InvalidPackageCache {
            package: spec.as_declaration(),
            reason: format!("cannot read tar entry: {e}"),
        })?;
        let archive_path = entry.path().map_err(|e| EngineError::InvalidPackageCache {
            package: spec.as_declaration(),
            reason: format!("cannot read tar path: {e}"),
        })?;
        let relative = safe_package_relative_path(&archive_path, spec)?;
        let Some(relative) = relative else { continue };
        let destination = output.join(relative);
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(&destination)?;
        } else if entry_type.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(&destination)?;
            std::io::copy(&mut entry, &mut file)?;
        } else {
            return Err(EngineError::UnsafePackageArchive {
                package: spec.as_declaration(),
                path: archive_path.display().to_string(),
            });
        }
    }
    Ok(())
}

fn safe_package_relative_path(
    archive_path: &Path,
    spec: &PackageSpec,
) -> Result<Option<PathBuf>, EngineError> {
    let mut components = archive_path.components();
    if !matches!(components.next(), Some(Component::Normal(first)) if first == "package") {
        return Err(EngineError::UnsafePackageArchive {
            package: spec.as_declaration(),
            path: archive_path.display().to_string(),
        });
    }
    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(part) => relative.push(part),
            _ => {
                return Err(EngineError::UnsafePackageArchive {
                    package: spec.as_declaration(),
                    path: archive_path.display().to_string(),
                })
            }
        }
    }
    Ok((!relative.as_os_str().is_empty()).then_some(relative))
}

fn validate_package_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('\\') {
        return Err("invalid npm package name".into());
    }
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    if let Some(scoped) = name.strip_prefix('@') {
        let mut parts = scoped.split('/');
        if !valid_segment(parts.next().unwrap_or_default())
            || !valid_segment(parts.next().unwrap_or_default())
            || parts.next().is_some()
        {
            return Err("scoped packages must use @scope/name".into());
        }
    } else if name.contains('/') || !valid_segment(name) {
        return Err("invalid npm package name".into());
    }
    Ok(())
}

fn cache_package_name(name: &str) -> String {
    name.replace('/', "__")
}

fn encode_package_name(name: &str) -> String {
    name.replace('/', "%2F")
}

fn registry_url(options: &PackageResolverOptions) -> String {
    options
        .registry_url
        .clone()
        .unwrap_or_else(|| DEFAULT_REGISTRY_URL.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn parses_scoped_exact_package_spec() {
        let spec = PackageSpec::parse("@s-page/core@0.7.0").unwrap();
        assert_eq!(spec.name, "@s-page/core");
        assert_eq!(spec.version, "0.7.0");
    }

    #[test]
    fn rejects_version_ranges() {
        assert!(matches!(
            PackageSpec::parse("@s-page/core@^0.7.0"),
            Err(EngineError::InvalidPackageSpec { .. })
        ));
    }

    #[test]
    fn explicit_shell_does_not_require_package_json() {
        let temp = tempfile::tempdir().unwrap();
        let shell = resolve_project_shell(temp.path(), Some(Path::new("shell")), None).unwrap();
        assert_eq!(shell, temp.path().join("shell"));
    }

    #[test]
    fn legacy_installed_shell_is_supported() {
        let temp = tempfile::tempdir().unwrap();
        let shell = temp.path().join("node_modules/@s-page/core/dist/shell");
        fs::create_dir_all(&shell).unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"dependencies":{"@s-page/core":"^0.6.0"}}"#,
        )
        .unwrap();
        assert_eq!(
            resolve_project_shell(temp.path(), None, None).unwrap(),
            shell
        );
    }

    #[test]
    fn project_declaration_resolves_cached_core_without_node_modules() {
        let temp = tempfile::tempdir().unwrap();
        let cached = temp.path().join(".cache/packages/@s-page__core/0.6.10");
        fs::create_dir_all(cached.join("dist/shell")).unwrap();
        fs::write(cached.join("package.json"), "{}").unwrap();
        fs::write(cached.join("dist/shell/index.html"), "<html></html>").unwrap();
        fs::write(cached.join(CACHE_MARKER), "@s-page/core@0.6.10").unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{"spage":{"requires":">=0.6.8 <0.7.0","core":"@s-page/core@0.6.10","plugins":[]}}"#,
        )
        .unwrap();

        let shell = resolve_project_shell(temp.path(), None, None).unwrap();
        assert_eq!(shell, cached.join("dist/shell"));
        assert!(!temp.path().join("node_modules").exists());
    }

    #[test]
    fn complete_cache_is_reused_without_registry_access() {
        let temp = tempfile::tempdir().unwrap();
        let spec = PackageSpec::parse("@s-page/core@0.7.0").unwrap();
        let cached = temp.path().join("@s-page__core/0.7.0");
        fs::create_dir_all(&cached).unwrap();
        fs::write(cached.join("package.json"), "{}").unwrap();
        fs::write(cached.join(CACHE_MARKER), spec.as_declaration()).unwrap();
        let resolved = ensure_package(
            &spec,
            &PackageResolverOptions {
                cache_dir: Some(temp.path().to_path_buf()),
                registry_url: Some("http://127.0.0.1:1".into()),
            },
        )
        .unwrap();
        assert_eq!(resolved, cached);
    }

    #[test]
    fn downloads_unpacks_and_then_reuses_cache() {
        let tarball = package_tarball(&[
            (
                "package/package.json",
                br#"{"name":"@s-page/core","version":"0.7.0"}"#,
            ),
            (
                "package/dist/shell/index.html",
                b"<html>cached shell</html>",
            ),
        ]);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let metadata = format!(
            r#"{{"versions":{{"0.7.0":{{"dist":{{"tarball":"http://{address}/core.tgz"}}}}}},"dist-tags":{{"latest":"0.7.0"}}}}"#
        );
        let server = thread::spawn(move || {
            serve_response(&listener, "application/json", metadata.as_bytes());
            serve_response(&listener, "application/octet-stream", &tarball);
        });

        let temp = tempfile::tempdir().unwrap();
        let spec = PackageSpec::parse("@s-page/core@0.7.0").unwrap();
        let options = PackageResolverOptions {
            cache_dir: Some(temp.path().to_path_buf()),
            registry_url: Some(format!("http://{address}")),
        };
        let package_dir = ensure_package(&spec, &options).unwrap();
        server.join().unwrap();
        assert_eq!(
            fs::read_to_string(package_dir.join("dist/shell/index.html")).unwrap(),
            "<html>cached shell</html>"
        );
        assert!(package_dir.join(CACHE_MARKER).is_file());

        let offline_options = PackageResolverOptions {
            cache_dir: options.cache_dir,
            registry_url: Some("http://127.0.0.1:1".into()),
        };
        assert_eq!(
            ensure_package(&spec, &offline_options).unwrap(),
            package_dir
        );
    }

    #[test]
    fn update_core_writes_exact_version_and_syncs_schemas() {
        let tarball = package_tarball(&[
            (
                "package/package.json",
                br#"{"name":"@s-page/core","version":"0.6.10"}"#,
            ),
            ("package/dist/shell/index.html", b"<html></html>"),
            ("package/schemas/config.schema.json", b"{}"),
        ]);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let metadata = format!(
            r#"{{"versions":{{"0.6.10":{{"dist":{{"tarball":"http://{address}/core.tgz"}}}}}},"dist-tags":{{"latest":"0.6.10"}}}}"#
        );
        let server = thread::spawn(move || {
            serve_response(&listener, "application/json", metadata.as_bytes());
            serve_response(&listener, "application/json", metadata.as_bytes());
            serve_response(&listener, "application/octet-stream", &tarball);
        });
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            r#"{
  "name": "test-project",
  "spage": {
    "requires": ">=0.6.8 <0.7.0",
    "core": "@s-page/core@0.6.9",
    "plugins": ["@s-page/plugin-example@1.0.0"]
  }
}"#,
        )
        .unwrap();

        let declaration = update_resources_with_registry(
            UpdateOptions {
                work_dir: temp.path().to_path_buf(),
                target: UpdateTarget::Core,
                package_cache_dir: None,
            },
            Some(format!("http://{address}")),
        )
        .unwrap();
        server.join().unwrap();

        assert_eq!(declaration.core, "@s-page/core@0.6.10");
        assert_eq!(declaration.plugins, ["@s-page/plugin-example@1.0.0"]);
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(temp.path().join("package.json")).unwrap())
                .unwrap();
        assert_eq!(written["spage"]["core"], "@s-page/core@0.6.10");
        assert!(temp
            .path()
            .join(".cache/generated/schemas/config.schema.json")
            .is_file());
    }

    #[test]
    fn archive_path_must_stay_under_package_prefix() {
        let spec = PackageSpec::parse("@s-page/core@0.7.0").unwrap();
        assert!(
            safe_package_relative_path(Path::new("package/dist/shell/index.html"), &spec)
                .unwrap()
                .is_some()
        );
        assert!(matches!(
            safe_package_relative_path(Path::new("other/file"), &spec),
            Err(EngineError::UnsafePackageArchive { .. })
        ));
        assert!(matches!(
            safe_package_relative_path(Path::new("package/../outside"), &spec),
            Err(EngineError::UnsafePackageArchive { .. })
        ));
    }

    fn package_tarball(files: &[(&str, &[u8])]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(encoder);
        for (path, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, path, *contents).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    fn serve_response(listener: &TcpListener, content_type: &str, body: &[u8]) {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request).unwrap();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    }
}
