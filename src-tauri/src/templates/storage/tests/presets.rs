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
