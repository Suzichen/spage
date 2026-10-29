// Feature: engine-cli-commands, Property 7: Directory isolation
//
// build() must not create files outside output_dir.
// serve() data generation must not create files outside cache_dir.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use proptest::prelude::*;
use spage_engine::build::{build, BuildOptions};
use spage_engine::serve::{serve, ServeOptions};

/// Collect all file paths under a directory recursively.
fn collect_files(dir: &Path) -> HashSet<std::path::PathBuf> {
    let mut files = HashSet::new();
    if !dir.exists() {
        return files;
    }
    for entry in walkdir(dir) {
        files.insert(entry);
    }
    files
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut result = vec![];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir(&path));
            } else {
                result.push(path);
            }
        }
    }
    result
}

/// Create a minimal valid project structure for build.
fn setup_project(dir: &Path, shell_dir: &Path) {
    fs::create_dir_all(dir.join("posts")).unwrap();
    fs::write(
        dir.join("config.json"),
        r#"{"title":"T","description":"D","logo":"/l.png","favicon":"/f.ico"}"#,
    )
    .unwrap();
    fs::create_dir_all(shell_dir).unwrap();
    fs::write(shell_dir.join("index.html"), "<html></html>").unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn prop_build_no_files_outside_output_dir(seed in 0u32..1000u32) {
        let _ = seed;
        let tmp = tempfile::tempdir().unwrap();
        let work_dir = tmp.path().join("project");
        let output_dir = tmp.path().join("output");
        let shell_dir = tmp.path().join("shell");

        setup_project(&work_dir, &shell_dir);

        // Snapshot files before build (excluding output_dir)
        let before = collect_files(tmp.path());

        let opts = BuildOptions {
            work_dir: work_dir.clone(),
            output_dir: output_dir.clone(),
            shell_dir: Some(shell_dir.clone()),
            package_cache_dir: None,
        };
        let _ = build(opts);

        // Check: all new files must be inside output_dir
        let after = collect_files(tmp.path());
        let new_files: Vec<_> = after.difference(&before).collect();
        for f in &new_files {
            prop_assert!(
                f.starts_with(&output_dir),
                "file {:?} created outside output_dir {:?}",
                f,
                output_dir
            );
        }
    }

    #[test]
    fn prop_serve_no_files_outside_cache_dir(seed in 0u32..1000u32) {
        let _ = seed;
        let tmp = tempfile::tempdir().unwrap();
        let work_dir = tmp.path().join("project");
        let cache_dir = tmp.path().join("cache");
        let shell_dir = tmp.path().join("shell");

        setup_project(&work_dir, &shell_dir);

        // Snapshot before serve
        let before = collect_files(tmp.path());

        let opts = ServeOptions {
            work_dir: work_dir.clone(),
            cache_dir: cache_dir.clone(),
            shell_dir: Some(shell_dir.clone()),
            package_cache_dir: None,
            port: 0, // Use port 0 to let OS assign
        };
        let result = serve(opts);
        if let Ok(mut handle) = result {
            handle.shutdown();
        }

        // Check: all new files must be inside cache_dir
        let after = collect_files(tmp.path());
        let new_files: Vec<_> = after.difference(&before).collect();
        for f in &new_files {
            prop_assert!(
                f.starts_with(&cache_dir),
                "file {:?} created outside cache_dir {:?}",
                f,
                cache_dir
            );
        }
    }
}
