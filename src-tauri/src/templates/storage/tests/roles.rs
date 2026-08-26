use super::*;

#[test]
fn list_roles_merges_sources_and_marks_read_only() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("dev.yaml"),
        &agent_role_yaml("dev", "user override"),
    );

    let roles = store.list_roles().expect("list roles");
    let lead = roles
        .iter()
        .find(|role| role.template.role_id == "lead")
        .expect("lead role");
    assert_eq!(lead.source, TemplateSource::BuiltIn);
    assert!(lead.read_only);

    let dev = roles
        .iter()
        .find(|role| role.template.role_id == "dev")
        .expect("dev role");
    assert_eq!(dev.source, TemplateSource::User);
    assert!(!dev.read_only);
    assert_eq!(dev.template.instructions, "user override");
}

#[test]
fn get_role_prefers_user_override() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("dev.yaml"),
        &agent_role_yaml("dev", "user override"),
    );

    let role = store.get_role("dev").expect("get role");
    assert_eq!(role.source, TemplateSource::User);
    assert_eq!(role.template.instructions, "user override");
}

// Regression: ff40911 stripped legacy effort suffixes only at launch time,
// leaving template storage unable to preserve the requested effort separately.
#[test]
fn role_loaders_split_legacy_model_and_effort_for_builtins_and_user_files() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates/roles/dev-dash.yaml"),
        &agent_role_yaml("dev-dash", "user dash").replace("gpt-5.4 high", "gpt-5.4-high"),
    );
    write(
        &app_data.join("templates/roles/dev-bare.yaml"),
        &agent_role_yaml("dev-bare", "user bare").replace("gpt-5.4 high", "gpt-5.4"),
    );

    let roles = store.list_roles().expect("list roles");
    for role_id in ["dev", "dev-dash"] {
        let defaults = &roles
            .iter()
            .find(|role| role.template.role_id == role_id)
            .expect("role")
            .template
            .defaults;
        assert_eq!(defaults.model, "gpt-5.4");
        assert_eq!(defaults.reasoning_effort.as_deref(), Some("high"));
    }

    let bare = &roles
        .iter()
        .find(|role| role.template.role_id == "dev-bare")
        .expect("bare role")
        .template
        .defaults;
    assert_eq!(bare.model, "gpt-5.4");
    assert_eq!(bare.reasoning_effort, None);
}

// Regression: ff40911 left the legacy model suffix in persisted role files,
// so an editor save could not preserve effort in its own schema field.
#[test]
fn save_after_legacy_load_writes_canonical_model_and_effort_keys() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let loaded = store.get_role("dev").expect("load legacy builtin");
    store
        .update_role("dev", &loaded.template)
        .expect("save canonical override");

    let raw =
        fs::read_to_string(app_data.join("templates/roles/dev.yaml")).expect("read saved role");
    assert!(raw.contains("model: gpt-5.4"));
    assert!(raw.contains("reasoning_effort: high"));
    assert!(!raw.contains("gpt-5.4 high"));
    assert!(!raw.contains("gpt-5.4-high"));
}

// Regression: ff40911 split legacy model/effort values at runtime, but the
// bundled role catalog kept combined values and could silently lose effort.
#[test]
fn bundled_roles_use_canonical_model_and_reasoning_effort() {
    let roles_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates")
        .join("roles");
    let mut paths = fs::read_dir(&roles_dir)
        .expect("read bundled roles")
        .map(|entry| entry.expect("read role entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("yaml"))
        .collect::<Vec<_>>();
    paths.sort();

    assert_eq!(paths.len(), 38, "bundled role count changed");

    let mut high_effort_roles = Vec::new();
    for path in paths {
        let raw = fs::read_to_string(&path).expect("read bundled role");
        assert!(
            raw.lines()
                .any(|line| line.trim_start().starts_with("reasoning_effort:")),
            "{} must declare defaults.reasoning_effort explicitly",
            path.display()
        );
        let role: RoleTemplate = serde_norway::from_str(&raw).expect("parse bundled role");
        let parsed = crate::session_scanner::launch::ModelSpec::parse_legacy(&role.defaults.model);

        assert_eq!(
            parsed.model.as_deref(),
            Some(role.defaults.model.as_str()),
            "{} must use a bare model slug",
            path.display()
        );
        assert_eq!(
            parsed.reasoning_effort,
            None,
            "{} still embeds reasoning effort in defaults.model",
            path.display()
        );

        if role.defaults.reasoning_effort.as_deref() == Some("high") {
            high_effort_roles.push(role.role_id);
        }
    }

    assert_eq!(high_effort_roles.len(), 14);
    assert!(high_effort_roles
        .iter()
        .any(|role| role == "quick-dev-codex"));
}

#[test]
fn create_role_validates_writes_and_commits() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let template = parse_role(&agent_role_yaml("qa", "qa role"));
    let result = store.create_role(&template).expect("create role");

    assert!(!result.committed);
    assert!(result.commit_id.is_none());
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    assert!(app_data
        .join("templates")
        .join("roles")
        .join("qa.yaml")
        .exists());
}

#[test]
fn create_role_blocks_built_in_collision() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let template = parse_role(&lead_role_yaml("lead", "override"));
    let err = store.create_role(&template).expect_err("should fail");
    assert!(matches!(err, TemplateStoreError::ReadOnly(_)));
}

#[test]
fn update_role_creates_user_override_for_built_in() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let template = parse_role(&lead_role_yaml("lead", "lead override"));
    let result = store
        .update_role("lead", &template)
        .expect("update built-in via override");

    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    let role = store.get_role("lead").expect("get role");
    assert_eq!(role.source, TemplateSource::User);
    assert_eq!(role.template.instructions, "lead override");
    assert!(app_data
        .join("templates")
        .join("roles")
        .join("lead.yaml")
        .exists());
}

#[test]
fn update_role_fails_when_missing() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let template = parse_role(&agent_role_yaml("does-not-exist", "new"));
    let err = store
        .update_role("does-not-exist", &template)
        .expect_err("update should fail");
    assert!(matches!(err, TemplateStoreError::NotFound(_)));
}

#[test]
fn delete_role_blocks_when_referenced_by_preset() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    store.create_role(&qa).expect("create qa");
    write(
        &app_data.join("templates").join("presets").join("qa.yaml"),
        &preset_yaml_with_agent("qa-preset", "qa"),
    );

    let err = store.delete_role("qa").expect_err("delete should fail");
    assert!(matches!(err, TemplateStoreError::Conflict(_)));
}

#[test]
fn delete_role_removes_user_template_and_commits() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    store.create_role(&qa).expect("create qa");

    let result = store.delete_role("qa").expect("delete qa");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");
    assert!(!app_data
        .join("templates")
        .join("roles")
        .join("qa.yaml")
        .exists());
}

#[test]
fn import_role_validates_and_writes_to_user_directory() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let external = app_data.join("external-role.yaml");
    write(&external, &agent_role_yaml("researcher", "research role"));

    let result = store.import_role(&external).expect("import role");
    assert!(!result.committed);
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");

    let role = store.get_role("researcher").expect("get imported role");
    assert_eq!(role.source, TemplateSource::User);
    assert_eq!(role.template.instructions, "research role");
}

#[test]
fn import_markdown_role_persists_provenance_and_records_import_commit_message() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let external = app_data.join("imported-role.md");
    write(
        &external,
        r#"---
name: Imported Reviewer
model: claude-opus-4-6
tools:
  - read
  - bash
---
Review structural changes and summarize risks.
"#,
    );

    store.import_role(&external).expect("import markdown role");
    let flushed = store.flush_pending_commits().expect("flush pending");
    assert!(flushed.is_some(), "flush should create commit");

    let role = store
        .get_role("imported-reviewer")
        .expect("get imported role");
    let provenance = role
        .template
        .provenance
        .as_ref()
        .expect("provenance should persist");
    assert_eq!(
        provenance.source_path.as_deref(),
        Some(external.to_string_lossy().as_ref())
    );
    assert_eq!(
        provenance.source_format,
        crate::templates::adapters::RoleExportFormat::ClaudeAgent
    );
    assert_eq!(
        latest_commit_message(&app_data.join("templates")),
        "templates: import role imported-reviewer from claude_agent"
    );
}

#[test]
fn import_role_conflicts_when_role_id_already_exists() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);

    let external = app_data.join("duplicate.md");
    write(
        &external,
        r#"---
name: Dev
description: Imported duplicate
model: gpt-5
---
Imported duplicate instructions.
"#,
    );

    let err = store
        .import_role(&external)
        .expect_err("duplicate import should fail");
    assert!(matches!(err, TemplateStoreError::Conflict(_)));
}

#[test]
fn list_roles_picks_up_external_files_added_to_roles_directory() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("ext.yaml"),
        &agent_role_yaml("ext", "external file"),
    );

    let roles = store.list_roles().expect("list roles");
    let ext = roles
        .iter()
        .find(|role| role.template.role_id == "ext")
        .expect("external role present");
    assert_eq!(ext.source, TemplateSource::User);
    assert_eq!(ext.template.instructions, "external file");
}
