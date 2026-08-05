//! Spage resource declarations, registry resolution, and package caching.

use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize};

use crate::error::EngineError;

const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";
const CACHE_MARKER: &str = ".spage-complete";
const CORE_PACKAGE: &str = "@s-page/core";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct PackageSpec(String);

impl PackageSpec {
    pub fn parse(value: &str) -> Result<Self, EngineError> {
        let split = value
            .rfind('@')
            .filter(|i| *i > 0)
            .ok_or_else(|| invalid_spec(value, "expected <package>@<exact-version>"))?;
        let (name, version) = value.split_at(split);
        if name.is_empty()
            || name.contains('\\')
            || name
                .split('/')
                .any(|part| part.is_empty() || matches!(part, "." | ".."))
        {
            return Err(invalid_spec(value, "invalid npm package name"));
        }
        Version::parse(&version[1..])
            .map_err(|_| invalid_spec(value, "version must be an exact semantic version"))?;
        Ok(Self(value.into()))
    }

    pub fn name(&self) -> &str {
        self.0.rsplit_once('@').unwrap().0
    }
    pub fn version(&self) -> &str {
        self.0.rsplit_once('@').unwrap().1
    }
}

impl<'de> Deserialize<'de> for PackageSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackageResolverOptions {
    pub cache_dir: Option<PathBuf>,
    pub registry_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpageDeclaration {
    pub core: PackageSpec,
    pub plugins: Vec<PackageSpec>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateTarget {
    #[default]
    All,
    Core,
    Plugins,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateOptions {
    pub work_dir: PathBuf,
    pub target: UpdateTarget,
    pub package_cache_dir: Option<PathBuf>,
    pub registry_url: Option<String>,
}

#[derive(Deserialize)]
struct RegistryMetadata {
    versions: std::collections::HashMap<String, RegistryVersion>,
}

#[derive(Deserialize)]
struct RegistryVersion {
    dist: RegistryDist,
}

#[derive(Deserialize)]
struct RegistryDist {
    tarball: String,
}

/// Download and unpack an exact package version, or reuse its complete cache entry.
pub fn ensure_package(
    spec: &PackageSpec,
    options: &PackageResolverOptions,
) -> Result<PathBuf, EngineError> {
    prepare_package(spec, options, None)
}

/// Resolve an explicit shell, a declared core, or an installed legacy shell, in that order.
pub fn resolve_project_shell(
    work_dir: &Path,
    shell_dir: Option<&Path>,
    package_cache_dir: Option<&Path>,
) -> Result<PathBuf, EngineError> {
    if let Some(shell) = shell_dir {
        return Ok(work_dir.join(shell));
    }

    let package_path = work_dir.join("package.json");
    let package: serde_json::Value = read_json(&package_path)?;
    if let Some(value) = package.get("spage") {
        let declaration: SpageDeclaration = serde_json::from_value(value.clone())?;
        validate_core(&declaration.core)?;
        let resolver = project_resolver(work_dir, package_cache_dir, None);
        let core_dir = ensure_package(&declaration.core, &resolver)?;
        return core_shell(&declaration.core, &core_dir);
    }

    let legacy = work_dir.join("node_modules/@s-page/core/dist/shell");
    if legacy.is_dir() {
        log::warn!("Using legacy node_modules/@s-page/core shell; add package.json.spage and run `spage update`");
        Ok(legacy)
    } else {
        Err(EngineError::ProjectDeclarationNotFound(package_path))
    }
}

/// Update exact resource versions and warm their caches.
pub fn update_resources(options: UpdateOptions) -> Result<SpageDeclaration, EngineError> {
    let package_path = options.work_dir.join("package.json");
    let mut package: serde_json::Value = read_json(&package_path)?;
    let mut declaration: SpageDeclaration = serde_json::from_value(
        package
            .get("spage")
            .cloned()
            .ok_or_else(|| EngineError::ProjectDeclarationNotFound(package_path.clone()))?,
    )?;
    let resolver = project_resolver(
        &options.work_dir,
        options.package_cache_dir.as_deref(),
        options.registry_url,
    );

    if matches!(options.target, UpdateTarget::All | UpdateTarget::Core) {
        validate_core_name(&declaration.core)?;
        declaration.core = update_package(&declaration.core, &resolver, Some(&engine_version()))?;
    }
    if matches!(options.target, UpdateTarget::All | UpdateTarget::Plugins) {
        declaration.plugins = declaration
            .plugins
            .iter()
            .map(|plugin| update_package(plugin, &resolver, None))
            .collect::<Result<_, _>>()?;
    }

    package["spage"] = serde_json::to_value(&declaration)?;
    fs::write(package_path, serde_json::to_string_pretty(&package)? + "\n")?;
    Ok(declaration)
}

fn update_package(
    current: &PackageSpec,
    options: &PackageResolverOptions,
    compatible_with: Option<&Version>,
) -> Result<PackageSpec, EngineError> {
    let registry = registry_url(options);
    let metadata = fetch_metadata(current.name(), registry)?;
    let (version, tarball) = metadata
        .versions
        .iter()
        .filter_map(|(raw, meta)| {
            let version = Version::parse(raw).ok()?;
            (version.pre.is_empty()
                && compatible_with.is_none_or(|engine| same_release_line(&version, engine)))
            .then(|| (version, meta.dist.tarball.as_str()))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .ok_or_else(|| EngineError::PackageVersionNotFound {
            name: current.name().into(),
            version: compatible_with
                .map(|v| format!("compatible with spage-engine {v}"))
                .unwrap_or_else(|| "stable".into()),
            registry: registry.into(),
        })?;
    let updated = PackageSpec(format!("{}@{version}", current.name()));
    prepare_package(&updated, options, Some(tarball))?;
    Ok(updated)
}

fn prepare_package(
    spec: &PackageSpec,
    options: &PackageResolverOptions,
    known_tarball: Option<&str>,
) -> Result<PathBuf, EngineError> {
    let package_dir = cache_path(spec, options);
    if package_dir.join(CACHE_MARKER).is_file() && package_dir.join("package.json").is_file() {
        return Ok(package_dir);
    }

    let registry = registry_url(options);
    let tarball = match known_tarball {
        Some(url) => url.into(),
        None => fetch_metadata(spec.name(), registry)?
            .versions
            .get(spec.version())
            .map(|v| v.dist.tarball.clone())
            .ok_or_else(|| EngineError::PackageVersionNotFound {
                name: spec.name().into(),
                version: spec.version().into(),
                registry: registry.into(),
            })?,
    };
    let response = attohttpc::get(&tarball)
        .send()
        .map_err(|e| network_error(&spec.0, &tarball, e))?;
    if !response.is_success() {
        return Err(network_error(
            &spec.0,
            &tarball,
            format!("HTTP {}", response.status()),
        ));
    }
    let bytes = response
        .bytes()
        .map_err(|e| network_error(&spec.0, &tarball, e))?;

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir)?;
    }
    fs::create_dir_all(&package_dir)?;
    let unpacked = unpack_package(&bytes, &package_dir, spec).and_then(|_| {
        package_dir
            .join("package.json")
            .is_file()
            .then_some(())
            .ok_or_else(|| EngineError::InvalidPackageCache {
                package: spec.0.clone(),
                reason: "archive does not contain package/package.json".into(),
            })
    });
    if let Err(error) = unpacked {
        let _ = fs::remove_dir_all(&package_dir);
        return Err(error);
    }
    fs::write(package_dir.join(CACHE_MARKER), [])?;
    Ok(package_dir)
}

fn fetch_metadata(name: &str, registry: &str) -> Result<RegistryMetadata, EngineError> {
    let url = format!(
        "{}/{}",
        registry.trim_end_matches('/'),
        name.replace('/', "%2F")
    );
    let response = attohttpc::get(&url)
        .send()
        .map_err(|e| network_error(name, &url, e))?;
    if response.status().as_u16() == 404 {
        return Err(EngineError::PackageNotFound {
            name: name.into(),
            registry: registry.into(),
        });
    }
    if !response.is_success() {
        return Err(network_error(
            name,
            &url,
            format!("HTTP {}", response.status()),
        ));
    }
    response
        .json()
        .map_err(|e| network_error(name, &url, format!("invalid registry metadata: {e}")))
}

fn unpack_package(bytes: &[u8], output: &Path, spec: &PackageSpec) -> Result<(), EngineError> {
    let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(bytes)));
    let entries = archive
        .entries()
        .map_err(|e| invalid_cache(spec, "cannot read tarball", e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| invalid_cache(spec, "cannot read tar entry", e))?;
        let archive_path = entry
            .path()
            .map_err(|e| invalid_cache(spec, "cannot read tar path", e))?;
        let Some(relative) = safe_archive_path(&archive_path, spec)? else {
            continue;
        };
        let kind = entry.header().entry_type();
        if !kind.is_dir() && !kind.is_file() {
            return Err(unsafe_archive(spec, &archive_path));
        }
        if let Some(parent) = output.join(&relative).parent() {
            fs::create_dir_all(parent)?;
        }
        entry
            .unpack(output.join(relative))
            .map_err(|e| invalid_cache(spec, "cannot unpack tar entry", e))?;
    }
    Ok(())
}

fn safe_archive_path(path: &Path, spec: &PackageSpec) -> Result<Option<PathBuf>, EngineError> {
    let relative = path
        .strip_prefix("package")
        .map_err(|_| unsafe_archive(spec, path))?;
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(unsafe_archive(spec, path));
    }
    Ok((!relative.as_os_str().is_empty()).then(|| relative.into()))
}

fn validate_core(spec: &PackageSpec) -> Result<(), EngineError> {
    validate_core_name(spec)?;
    let engine = engine_version();
    let core = Version::parse(spec.version()).expect("PackageSpec version was validated");
    same_release_line(&core, &engine)
        .then_some(())
        .ok_or_else(|| EngineError::CoreVersionMismatch {
            core: spec.version().into(),
            engine: engine.to_string(),
        })
}

fn validate_core_name(spec: &PackageSpec) -> Result<(), EngineError> {
    (spec.name() == CORE_PACKAGE)
        .then_some(())
        .ok_or_else(|| invalid_spec(&spec.0, format!("spage.core must reference {CORE_PACKAGE}")))
}

fn same_release_line(resource: &Version, engine: &Version) -> bool {
    resource.major == engine.major && (engine.major > 0 || resource.minor == engine.minor)
}

fn engine_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version must be semver")
}

fn core_shell(spec: &PackageSpec, package_dir: &Path) -> Result<PathBuf, EngineError> {
    let shell = package_dir.join("dist/shell");
    shell
        .join("index.html")
        .is_file()
        .then_some(shell)
        .ok_or_else(|| EngineError::InvalidPackageCache {
            package: spec.0.clone(),
            reason: "package does not contain dist/shell/index.html".into(),
        })
}

fn project_resolver(
    work_dir: &Path,
    cache_dir: Option<&Path>,
    registry_url: Option<String>,
) -> PackageResolverOptions {
    PackageResolverOptions {
        cache_dir: Some(work_dir.join(cache_dir.unwrap_or(Path::new(".cache/packages")))),
        registry_url,
    }
}

fn read_json(path: &Path) -> Result<serde_json::Value, EngineError> {
    let raw = fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            EngineError::ProjectDeclarationNotFound(path.into())
        } else {
            EngineError::Io(e)
        }
    })?;
    serde_json::from_str(&raw).map_err(EngineError::Json)
}

fn cache_path(spec: &PackageSpec, options: &PackageResolverOptions) -> PathBuf {
    options
        .cache_dir
        .clone()
        .unwrap_or_else(|| ".cache/packages".into())
        .join(spec.name().replace('/', "__"))
        .join(spec.version())
}

fn registry_url(options: &PackageResolverOptions) -> &str {
    options.registry_url.as_deref().unwrap_or(DEFAULT_REGISTRY)
}

fn invalid_spec(spec: &str, reason: impl Into<String>) -> EngineError {
    EngineError::InvalidPackageSpec {
        spec: spec.into(),
        reason: reason.into(),
    }
}

fn invalid_cache(spec: &PackageSpec, action: &str, error: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidPackageCache {
        package: spec.0.clone(),
        reason: format!("{action}: {error}"),
    }
}

fn unsafe_archive(spec: &PackageSpec, path: &Path) -> EngineError {
    EngineError::UnsafePackageArchive {
        package: spec.0.clone(),
        path: path.display().to_string(),
    }
}

fn network_error(package: &str, url: &str, error: impl std::fmt::Display) -> EngineError {
    EngineError::PackageNetwork {
        package: package.into(),
        url: url.into(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn legacy_shell_does_not_require_dependency_declaration() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join("node_modules/@s-page/core/dist/shell");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(temp.path().join("package.json"), "{}").unwrap();
        assert_eq!(
            resolve_project_shell(temp.path(), None, None).unwrap(),
            legacy
        );
    }

    #[test]
    fn update_ignores_latest_tag_and_removes_requires() {
        let compatible = compatible_version(2);
        let incompatible = incompatible_version();
        let tarball = package_tarball(&[
            ("package/package.json", b"{}"),
            ("package/dist/shell/index.html", b"<html></html>"),
        ]);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let metadata = format!(
            r#"{{"versions":{{"{compatible}":{{"dist":{{"tarball":"http://{address}/core.tgz"}}}},"{incompatible}":{{"dist":{{"tarball":"http://invalid/latest.tgz"}}}}}},"dist-tags":{{"latest":"{incompatible}"}}}}"#
        );
        let server = serve(listener, vec![(200, metadata.into_bytes()), (200, tarball)]);
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("package.json"),
            format!(
                r#"{{"spage":{{"requires":">=0","core":"@s-page/core@{}","plugins":[]}}}}"#,
                compatible_version(1)
            ),
        )
        .unwrap();

        let result = update_resources(UpdateOptions {
            work_dir: temp.path().into(),
            target: UpdateTarget::Core,
            registry_url: Some(format!("http://{address}")),
            ..Default::default()
        })
        .unwrap();
        server.join().unwrap();
        assert_eq!(result.core.version(), compatible);
        assert!(resolve_project_shell(temp.path(), None, None)
            .unwrap()
            .ends_with("dist/shell"));
        let written: serde_json::Value = read_json(&temp.path().join("package.json")).unwrap();
        assert!(written["spage"].get("requires").is_none());
    }

    #[test]
    fn registry_404_and_network_errors_are_distinct() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = serve(listener, vec![(404, b"{}".to_vec())]);
        let spec = PackageSpec::parse("@s-page/core@0.6.10").unwrap();
        let temp = tempfile::tempdir().unwrap();
        let options = |url| PackageResolverOptions {
            cache_dir: Some(temp.path().into()),
            registry_url: Some(url),
        };
        assert!(matches!(
            ensure_package(&spec, &options(format!("http://{address}"))),
            Err(EngineError::PackageNotFound { .. })
        ));
        server.join().unwrap();
        assert!(matches!(
            ensure_package(&spec, &options("http://127.0.0.1:1".into())),
            Err(EngineError::PackageNetwork { .. })
        ));
    }

    #[test]
    fn archive_paths_cannot_escape_package_prefix() {
        let spec = PackageSpec::parse("@s-page/core@0.6.10").unwrap();
        assert!(PackageSpec::parse("../outside@0.6.10").is_err());
        assert!(safe_archive_path(Path::new("package/dist/index.html"), &spec).is_ok());
        assert!(safe_archive_path(Path::new("package/../outside"), &spec).is_err());
        assert!(safe_archive_path(Path::new("other/file"), &spec).is_err());
    }

    #[test]
    fn declared_core_must_match_the_engine_release_line() {
        let spec = PackageSpec::parse(&format!("@s-page/core@{}", incompatible_version())).unwrap();
        assert!(matches!(
            validate_core(&spec),
            Err(EngineError::CoreVersionMismatch { .. })
        ));
    }

    fn compatible_version(patch: u64) -> String {
        let engine = engine_version();
        if engine.major == 0 {
            format!("0.{}.{patch}", engine.minor)
        } else {
            format!("{}.0.{patch}", engine.major)
        }
    }

    fn incompatible_version() -> String {
        let engine = engine_version();
        if engine.major == 0 {
            format!("0.{}.0", engine.minor + 1)
        } else {
            format!("{}.0.0", engine.major + 1)
        }
    }

    fn package_tarball(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        for (path, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, path, *body).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    fn serve(listener: TcpListener, responses: Vec<(u16, Vec<u8>)>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let _ = stream.read(&mut [0; 1024]);
                write!(
                    stream,
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        })
    }
}
