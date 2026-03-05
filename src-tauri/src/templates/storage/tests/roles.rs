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
