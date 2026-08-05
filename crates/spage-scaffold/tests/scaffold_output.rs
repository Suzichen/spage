//! Checks the scaffolded project as it lands on disk, not just the generated strings.

use std::fs;

fn scratch_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("spage-scaffold-{name}-{}", std::process::id()))
}

#[test]
fn a_new_project_validates_in_an_editor_before_its_first_build() {
    let target = scratch_dir("schemas");
    let _ = fs::remove_dir_all(&target);

    spage_scaffold::scaffold(&spage_scaffold::ScaffoldInput {
        target_dir: target.to_string_lossy().into_owned(),
        name: "my-blog".into(),
        description: "Scaffold output test".into(),
        author: String::new(),
        site_url: None,
        timezone: None,
    })
    .unwrap();

    // Every generated config must point at a schema file that already exists on disk.
    for config_name in ["config.json", "album.config.json", "memo.config.json"] {
        let config = fs::read_to_string(target.join(config_name)).unwrap();
        let reference = config
            .lines()
            .find_map(|line| line.trim().strip_prefix(r#""$schema": ""#))
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_else(|| panic!("{config_name} declares no $schema"));
        let resolved = target.join(reference.trim_start_matches("./"));
        assert!(
            resolved.is_file(),
            "{config_name} $schema does not resolve: {reference}"
        );
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&resolved).unwrap())
            .unwrap_or_else(|e| panic!("{reference} is not valid JSON: {e}"));
    }

    // The seeded copies are engine-managed build output and must stay out of git.
    let gitignore = fs::read_to_string(target.join(".gitignore")).unwrap();
    assert!(gitignore.lines().any(|line| line.trim() == ".cache"));

    fs::remove_dir_all(&target).unwrap();
}
