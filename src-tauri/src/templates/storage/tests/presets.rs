use super::*;

#[test]
fn list_presets_merges_sources_and_marks_read_only() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("presets").join("base.yaml"),
        &preset_yaml("base"),
    );

    let presets = store.list_presets().expect("list presets");
    let base = presets
        .iter()
        .find(|preset| preset.template.preset_id == "base")
        .expect("base preset");
    assert_eq!(base.source, TemplateSource::User);
    assert!(!base.read_only);
}

#[test]
fn get_preset_prefers_user_override() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");
    write(
        &app_data.join("templates").join("presets").join("base.yaml"),
        &preset_yaml("base"),
    );

    let preset = store.get_preset("base").expect("get preset");
    assert_eq!(preset.source, TemplateSource::User);
}

// Regression: ff40911 left preset overrides in the combined legacy spelling,
// so per-slot effort could not survive composition and persistence.
#[test]
fn preset_loader_splits_legacy_slot_override_model_and_effort() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    write(
        &builtins.join("presets/legacy-effort.yaml"),
        &preset_yaml_with_agent("legacy-effort", "dev").replace(
            "    project_binding: lead_project\n",
            "    project_binding: lead_project\n    overrides:\n      model: gpt-5.4-high\n",
        ),
    );
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let preset = store.get_preset("legacy-effort").expect("load preset");
    let overrides = preset.template.agent_slots[0]
        .overrides
        .as_ref()
        .expect("slot overrides");
    assert_eq!(overrides.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(overrides.reasoning_effort.as_deref(), Some("high"));
}

#[test]
fn create_preset_validates_writes_and_commits() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let preset = parse_preset(&preset_yaml_with_agent("qa-team", "dev"));
    let result = store.create_preset(&preset).expect("create preset");

    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    assert!(app_data
        .join("templates")
        .join("presets")
        .join("qa-team.yaml")
        .exists());
}

#[test]
fn create_preset_rejects_unknown_role_reference() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let preset = parse_preset(&preset_yaml_with_agent("bad", "missing-role"));
    let err = store.create_preset(&preset).expect_err("must fail");
    assert!(matches!(err, TemplateStoreError::Validation(_)));
}

#[test]
fn create_preset_blocks_built_in_collision() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let preset = parse_preset(&preset_yaml("base"));
    let err = store.create_preset(&preset).expect_err("must fail");
    assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
}

#[test]
fn update_preset_creates_user_override_for_built_in() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let preset = parse_preset(&preset_yaml("base"));
    let result = store.update_preset("base", &preset).expect("update base");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");

    let loaded = store.get_preset("base").expect("get base");
    assert_eq!(loaded.source, TemplateSource::User);
    assert!(app_data
        .join("templates")
        .join("presets")
        .join("base.yaml")
        .exists());
}

#[test]
fn update_preset_fails_when_missing() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let preset = parse_preset(&preset_yaml_with_agent("missing", "dev"));
    let err = store
        .update_preset("missing", &preset)
        .expect_err("missing preset");
    assert!(matches!(err, TemplateStoreError::NotFound(_)));
}

#[test]
fn delete_preset_removes_user_preset_and_commits() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let preset = parse_preset(&preset_yaml_with_agent("tmp", "dev"));
    store.create_preset(&preset).expect("create preset");

    let result = store.delete_preset("tmp").expect("delete preset");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    assert!(!app_data
        .join("templates")
        .join("presets")
        .join("tmp.yaml")
        .exists());
}

#[test]
fn delete_preset_blocks_built_in_delete() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let err = store
        .delete_preset("base")
        .expect_err("built-in delete blocked");
    assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
}

#[test]
fn import_preset_validates_and_writes_to_user_directory() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let external = app_data.join("external-preset.yaml");
    write(&external, &preset_yaml_with_agent("external", "dev"));

    let result = store.import_preset(&external).expect("import preset");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");

    let preset = store.get_preset("external").expect("get imported");
    assert_eq!(preset.source, TemplateSource::User);
}

// Regression: a79d392 reserialized every imported preset, dropping comments and
// schema-extension keys even when model normalization made no change.
#[test]
fn import_canonical_preset_preserves_source_text() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    let external = app_data.join("commented-preset.yaml");
    let raw = preset_yaml_with_agent("commented", "dev").replace(
        "name: Base Team\n",
        "# Keep this operator note.\nname: Base Team\nfuture_extension: keep-me\n",
    );
    write(&external, &raw);

    store.import_preset(&external).expect("import preset");

    let imported = fs::read_to_string(
        app_data
            .join("templates")
            .join("presets")
            .join("commented.yaml"),
    )
    .expect("read imported preset");
    assert_eq!(imported, raw);
}

#[test]
fn list_presets_picks_up_external_files_added_to_presets_directory() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("presets").join("ext.yaml"),
        &preset_yaml_with_agent("ext", "dev"),
    );

    let presets = store.list_presets().expect("list presets");
    let ext = presets
        .iter()
        .find(|preset| preset.template.preset_id == "ext")
        .expect("external preset present");
    assert_eq!(ext.source, TemplateSource::User);
}

// Regression: 27c3e32e made one dangling user preset fail every catalog read,
// so operators could not reach otherwise valid built-in presets to recover.
#[test]
fn invalid_user_preset_is_skipped_without_hiding_valid_builtin() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");
    write(
        &app_data.join("templates/presets/base.yaml"),
        &preset_yaml_with_agent("base", "retired-role"),
    );

    let listed = store
        .list_presets()
        .expect("invalid override should not fail the catalog");
    let base = listed
        .iter()
        .find(|preset| preset.template.preset_id == "base")
        .expect("valid built-in remains visible");
    assert_eq!(base.source, TemplateSource::BuiltIn);

    let fetched = store
        .get_preset("base")
        .expect("get should fall back from an invalid user override");
    assert_eq!(fetched.source, TemplateSource::BuiltIn);

    let catalog = store
        .load_catalog()
        .expect("invalid override should not brick merged catalog consumers");
    assert_eq!(catalog.presets, vec![parse_preset(&preset_yaml("base"))]);
}
