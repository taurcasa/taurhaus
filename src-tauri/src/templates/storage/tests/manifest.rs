use super::*;

// Regression: 9a6b9596 deleted ~28 role YAMLs from the bundled catalog, and
// because efb1b00b had made bundled-template discovery directory-open, NSIS
// upgrades left the deleted files in the install dir where discovery
// resurrected them — forcing the hand-written purge shipped in release 0.8.8.
#[test]
fn packaged_manifest_excludes_stray_templates_from_listing_and_seeding() {
    let (_root, app_data, builtins) = setup_dirs();
    let packaged = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let mut shipped_paths = BTreeSet::new();

    for dirname in [ROLES_DIRNAME, PRESETS_DIRNAME] {
        for entry in fs::read_dir(packaged.join(dirname)).expect("read packaged templates") {
            let source = entry.expect("read packaged template entry").path();
            if !source.is_file() || !is_yaml_file(&source) {
                continue;
            }
            let relative = PathBuf::from(dirname).join(
                source
                    .file_name()
                    .expect("template file should have a name"),
            );
            fs::copy(&source, builtins.join(&relative)).expect("copy packaged template");
            shipped_paths.insert(relative);
        }
    }

    write(
        &builtins.join("roles/stale-installer-role.yaml"),
        &agent_role_yaml("stale-installer-role", "must stay ignored"),
    );
    write(
        &builtins.join("presets/stale-installer-preset.yaml"),
        "schema:\n  kind: team_preset\n  version: 1\npreset_id: stale-installer-preset\nname: Stale Team\ndescription: Must stay ignored\nversion: \"1.0.0\"\nlead_role_id: v3-lead-claude\nagent_slots:\n  - role_id: v4-developer-codex\n    count: 1\n    project_binding: lead_project\ndefaults:\n  team_name_pattern: \"{project}-team\"\n  tmux_layout: tiled\n",
    );

    let store = TemplateStore::with_packaged_builtins_dir(app_data.clone(), builtins);
    let listed_role_ids = store
        .list_roles()
        .expect("list packaged roles")
        .into_iter()
        .map(|record| record.template.role_id)
        .collect::<BTreeSet<_>>();
    let listed_preset_ids = store
        .list_presets()
        .expect("list packaged presets")
        .into_iter()
        .map(|record| record.template.preset_id)
        .collect::<BTreeSet<_>>();
    let shipped_role_ids = shipped_paths
        .iter()
        .filter(|path| path.starts_with(ROLES_DIRNAME))
        .map(|path| {
            path.file_stem()
                .expect("role path should have a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    let shipped_preset_ids = shipped_paths
        .iter()
        .filter(|path| path.starts_with(PRESETS_DIRNAME))
        .map(|path| {
            path.file_stem()
                .expect("preset path should have a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(listed_role_ids, shipped_role_ids);
    assert_eq!(listed_preset_ids, shipped_preset_ids);

    store
        .ensure_repo_for_mutation()
        .expect("seed packaged templates");
    for relative in &shipped_paths {
        assert!(
            app_data.join("templates").join(relative).is_file(),
            "shipped template {} should be seeded",
            relative.display()
        );
    }
    assert!(!app_data
        .join("templates/roles/stale-installer-role.yaml")
        .exists());
    assert!(!app_data
        .join("templates/presets/stale-installer-preset.yaml")
        .exists());
}
