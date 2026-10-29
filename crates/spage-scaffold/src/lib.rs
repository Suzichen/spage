//! Core scaffold logic for spage — template copying and config injection.
//!
//! This crate is consumed by:
//! - `packages/create-spage` (via NAPI bindings) for the CLI
//! - `spage-admin` (Tauri) directly as a Rust dependency

use std::fs;
use std::path::Path;

use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use thiserror::Error;

static TEMPLATE: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/create-spage/template");
static CONFIG_SCHEMAS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/core/schemas");
const SCHEMA_DIR: &str = "./.cache/generated/schemas";

#[derive(Debug, Error)]
pub enum ScaffoldError {
    #[error("Target directory already exists: {0}")]
    DirectoryExists(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Input parameters for scaffolding a new blog project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaffoldInput {
    pub target_dir: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub site_url: Option<String>,
    pub timezone: Option<String>,
}

/// Scaffold a new spage project.
///
/// 1. Validates target doesn't exist
/// 2. Extracts embedded template to target
/// 3. Renames `_gitignore` → `.gitignore`
/// 4. Generates customized `package.json`
/// 5. Generates `config.json` (JSONC with comments)
/// 6. Generates `album.config.json` (JSONC with comments)
/// 7. Generates `memo.config.json` (JSONC with comments, disabled by default)
/// 8. Seeds `.cache/generated/schemas` so editors resolve `$schema` before the first build
pub fn scaffold(input: &ScaffoldInput) -> Result<(), ScaffoldError> {
    let target = Path::new(&input.target_dir);

    if target.exists() {
        return Err(ScaffoldError::DirectoryExists(input.target_dir.clone()));
    }

    // Extract embedded template
    extract_dir(&TEMPLATE, target)?;

    // Rename _gitignore → .gitignore
    let gitignore_src = target.join("_gitignore");
    if gitignore_src.exists() {
        fs::rename(&gitignore_src, target.join(".gitignore"))?;
    }

    // Rename _env.example → .env.example
    let env_example_src = target.join("_env.example");
    if env_example_src.exists() {
        fs::rename(&env_example_src, target.join(".env.example"))?;
    }

    // Generate package.json
    let package_json = generate_package_json(input);
    fs::write(target.join("package.json"), package_json + "\n")?;

    // Generate config.json
    let config_json = generate_config_json(input);
    fs::write(target.join("config.json"), config_json + "\n")?;

    // Generate album.config.json
    let album_config = generate_album_config_json();
    fs::write(target.join("album.config.json"), album_config + "\n")?;

    // Generate memo.config.json
    let memo_config = generate_memo_config_json();
    fs::write(target.join("memo.config.json"), memo_config + "\n")?;

    extract_dir(&CONFIG_SCHEMAS, &target.join(SCHEMA_DIR))?;

    Ok(())
}

/// Clean up a failed scaffold attempt.
pub fn cleanup(target_dir: &str) {
    let target = Path::new(target_dir);
    if target.exists() {
        let _ = fs::remove_dir_all(target);
    }
}

fn extract_dir(dir: &Dir<'_>, dest: &Path) -> Result<(), ScaffoldError> {
    fs::create_dir_all(dest)?;
    for file in dir.files() {
        let file_name = file.path().file_name().unwrap();
        let path = dest.join(file_name);
        fs::write(&path, file.contents())?;
    }
    for sub in dir.dirs() {
        let dir_name = sub.path().file_name().unwrap();
        extract_dir(sub, &dest.join(dir_name))?;
    }
    Ok(())
}

fn generate_package_json(input: &ScaffoldInput) -> String {
    let mut lines = Vec::new();
    lines.push("{".to_string());
    lines.push(format!("  \"name\": {},", serde_json::to_string(&input.name).unwrap()));
    lines.push("  \"private\": true,".to_string());
    lines.push("  \"version\": \"0.0.0\",".to_string());
    lines.push("  \"type\": \"module\",".to_string());
    lines.push(format!("  \"description\": {},", serde_json::to_string(&input.description).unwrap()));
    if !input.author.is_empty() {
        lines.push(format!("  \"author\": {},", serde_json::to_string(&input.author).unwrap()));
    }
    lines.push("  \"spage\": {".to_string());
    lines.push("    \"core\": \"@s-page/core@0.6.10\",".to_string());
    lines.push("    \"plugins\": []".to_string());
    lines.push("  },".to_string());
    lines.push("  \"scripts\": {".to_string());
    lines.push("    \"dev\": \"spage serve\",".to_string());
    lines.push("    \"build\": \"spage build\",".to_string());
    lines.push("    \"update\": \"spage update\",".to_string());
    lines.push("    \"sync\": \"spage sync --media\"".to_string());
    lines.push("  },".to_string());
    lines.push("  \"devDependencies\": {".to_string());
    lines.push("    \"@s-page/engine\": \"0.6.8\"".to_string());
    lines.push("  }".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

fn generate_config_json(input: &ScaffoldInput) -> String {
    let mut lines = Vec::new();
    lines.push("{".to_string());
    lines.push(format!(r#"  "$schema": "{SCHEMA_DIR}/config.schema.json","#));
    lines.push("  // Site title displayed in header and browser tab".to_string());
    lines.push(format!("  \"title\": {},", serde_json::to_string(&input.name).unwrap()));
    lines.push("  // Site description for SEO meta tags".to_string());
    lines.push(format!("  \"description\": {},", serde_json::to_string(&input.description).unwrap()));
    lines.push(r#"  "logo": "/logo.svg","#.to_string());
    lines.push(r#"  "favicon": "/favicon.svg","#.to_string());

    match &input.site_url {
        Some(url) if !url.is_empty() => {
            lines.push("  // Production URL — required for sitemap, RSS, Open Graph".to_string());
            lines.push(format!("  \"siteUrl\": {},", serde_json::to_string(url).unwrap()));
        }
        _ => {
            lines.push("  // Production URL — required for sitemap, RSS, Open Graph".to_string());
            lines.push("  // \"siteUrl\": \"https://example.com\",".to_string());
        }
    }

    if !input.author.is_empty() {
        lines.push(format!("  \"author\": {},", serde_json::to_string(&input.author).unwrap()));
    } else {
        lines.push("  // \"author\": \"Your Name\",".to_string());
    }

    lines.push("  // Default language: \"en\", \"zh-CN\", or \"ja\"".to_string());
    lines.push(r#"  "language": "en","#.to_string());

    match &input.timezone {
        Some(tz) if !tz.is_empty() => {
            lines.push("  // IANA timezone — ensures correct post dates on CI builds".to_string());
            lines.push(format!("  \"timezone\": {},", serde_json::to_string(tz).unwrap()));
        }
        _ => {
            lines.push("  // IANA timezone — ensures correct post dates on CI builds".to_string());
            lines.push("  // \"timezone\": \"Asia/Tokyo\",".to_string());
        }
    }

    lines.push("  // Sub-directory deployment path (e.g., \"/blog\"). Defaults to \"/\"".to_string());
    lines.push("  // \"basePath\": \"/blog\",".to_string());
    lines.push(r#"  "links": {"#.to_string());
    lines.push(r#"    "enabled": true,"#.to_string());
    lines.push(r#"    "items": {"#.to_string());
    lines.push(r#"      "Spage": "https://spage.me""#.to_string());
    lines.push("    }".to_string());
    lines.push("  },".to_string());
    lines.push("  // Built-in platforms: github, rss, x, twitter, weibo, zhihu, bilibili, email, facebook, instagram, tiktok".to_string());
    lines.push(r#"  "socialLinks": {"#.to_string());
    lines.push(r#"    "enabled": true,"#.to_string());
    lines.push(r#"    "items": ["#.to_string());
    lines.push(r#"      { "platform": "rss" },"#.to_string());
    lines.push(r#"      { "platform": "github", "url": "https://github.com/Suzichen/spage" }"#.to_string());
    lines.push("    ]".to_string());
    lines.push("  }".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

fn generate_album_config_json() -> String {
    let mut lines = Vec::new();
    lines.push("{".to_string());
    lines.push(format!(r#"  "$schema": "{SCHEMA_DIR}/album.config.schema.json","#));
    lines.push("  // Set to false to disable the album feature entirely".to_string());
    lines.push(r#"  "enabled": true,"#.to_string());
    lines.push(r#"  "albums": ["#.to_string());
    lines.push("    // \"dir\": folder name, \"name\": display name, \"desc\": multiline description, \"cover\": cover filename".to_string());
    lines.push(r#"    { "dir": "blog", "name": "My Album", "desc": "Optional description.\nLine breaks are supported." }"#.to_string());
    lines.push("  ]".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

fn generate_memo_config_json() -> String {
    let mut lines = Vec::new();
    lines.push("{".to_string());
    lines.push(format!(r#"  "$schema": "{SCHEMA_DIR}/memo.config.schema.json","#));
    lines.push("  // Set to true and configure serverUrl to enable the Memo module".to_string());
    lines.push(r#"  "enabled": false,"#.to_string());
    lines.push("  // Data provider — currently only \"ech0\" is supported".to_string());
    lines.push(r#"  "provider": "ech0","#.to_string());
    lines.push("  // Your Ech0 instance URL (e.g., https://ech0.example.com)".to_string());
    lines.push(r#"  "serverUrl": "https://your-ech0-instance.com","#.to_string());
    lines.push("  // Number of memos to load per page".to_string());
    lines.push(r#"  "pageSize": 20"#.to_string());
    lines.push("  // Custom page title (optional, falls back to i18n default)".to_string());
    lines.push("  // \"title\": \"Memo\"".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `x.y.z` with an optional prerelease suffix — rejects ranges like `^0.6.10`.
    fn is_exact_version(value: &str) -> bool {
        let numeric = value.split('-').next().unwrap_or_default();
        let parts: Vec<&str> = numeric.split('.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    }

    #[test]
    fn generated_project_uses_spage_resource_declaration() {
        let input = ScaffoldInput {
            target_dir: "unused".into(),
            name: "my-blog".into(),
            description: "Test blog".into(),
            author: "Test Author".into(),
            site_url: None,
            timezone: None,
        };

        let package: serde_json::Value = serde_json::from_str(&generate_package_json(&input)).unwrap();
        assert!(package["spage"].get("requires").is_none());
        assert_eq!(package["spage"]["plugins"], serde_json::json!([]));
        assert!(package.get("dependencies").is_none());

        // Versions here are rewritten by scripts/bump-create.js, so assert shape, not literals.
        let core = package["spage"]["core"].as_str().unwrap();
        let core_version = core
            .strip_prefix("@s-page/core@")
            .expect("spage.core must reference @s-page/core");
        assert!(is_exact_version(core_version), "core must be exact: {core}");

        let engine = package["devDependencies"]["@s-page/engine"].as_str().unwrap();
        assert!(is_exact_version(engine), "engine must be exact: {engine}");

        // A new project must not start life with a CoreVersionMismatch.
        let line = |v: &str| v.split('.').take(2).map(str::to_owned).collect::<Vec<_>>();
        if core_version.starts_with("0.") {
            assert_eq!(line(core_version), line(engine), "0.x needs the same major/minor");
        } else {
            assert_eq!(line(core_version)[0], line(engine)[0], "1.x+ needs the same major");
        }

        let config = generate_config_json(&input);
        // The schema path is version-free and refreshed from the resolved core on every
        // serve/build, so it always describes the core this project actually uses.
        assert!(config.contains("./.cache/generated/schemas/config.schema.json"));
        assert!(!config.contains("node_modules/@s-page/core"));
        assert!(!config.contains("@s-page/core@"));
    }

    #[test]
    fn every_generated_schema_reference_is_seeded() {
        let input = ScaffoldInput {
            target_dir: "unused".into(),
            name: "my-blog".into(),
            description: "Test blog".into(),
            author: String::new(),
            site_url: None,
            timezone: None,
        };

        for config in [
            generate_config_json(&input),
            generate_album_config_json(),
            generate_memo_config_json(),
        ] {
            let reference = config
                .lines()
                .find_map(|line| line.trim().strip_prefix(r#""$schema": ""#))
                .and_then(|rest| rest.split('"').next())
                .expect("every generated config declares $schema");
            let name = reference
                .strip_prefix(&format!("{SCHEMA_DIR}/"))
                .unwrap_or_else(|| panic!("{reference} must live under {SCHEMA_DIR}"));
            let schema = CONFIG_SCHEMAS
                .get_file(name)
                .unwrap_or_else(|| panic!("{name} is not embedded"));
            serde_json::from_slice::<serde_json::Value>(schema.contents())
                .unwrap_or_else(|e| panic!("embedded {name} is not valid JSON: {e}"));
        }
    }
}
