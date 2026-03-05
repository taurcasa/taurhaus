use super::*;

#[test]
fn debounce_coalesces_repeated_role_updates_into_single_commit() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa_v1 = parse_role(&agent_role_yaml("qa", "qa v1"));
    let qa_v2 = parse_role(&agent_role_yaml("qa", "qa v2"));
    assert!(!store.create_role(&qa_v1).expect("create").committed);
    assert!(!store.update_role("qa", &qa_v2).expect("update").committed);

    let state = store.load_state().expect("load state");
    assert_eq!(state.pending_actions.len(), 1, "same role should coalesce");
    assert_eq!(state.pending_actions[0].action, "update");
    assert_eq!(state.pending_actions[0].id, "qa");

    assert!(store
        .maybe_flush_pending_commits()
        .expect("maybe flush before debounce")
        .is_none());
    age_pending_actions(&store, 31);
    let commit_id = store
        .maybe_flush_pending_commits()
        .expect("flush after debounce");
    assert!(commit_id.is_some());
    assert_eq!(
        latest_commit_message(&app_data.join("templates")),
        "templates: update role qa"
    );
}

#[test]
fn debounce_uses_batch_message_for_multiple_pending_actions() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    let preset = parse_preset(&preset_yaml_with_agent("qa-team", "qa"));
    assert!(!store.create_role(&qa).expect("create role").committed);
    assert!(
        !store
            .create_preset(&preset)
            .expect("create preset")
            .committed
    );

    let state = store.load_state().expect("load state");
    assert_eq!(state.pending_actions.len(), 2);

    age_pending_actions(&store, 31);
    let commit_id = store
        .maybe_flush_pending_commits()
        .expect("flush pending batch");
    assert!(commit_id.is_some());
    assert_eq!(
        latest_commit_message(&app_data.join("templates")),
        "templates: batch 2 changes"
    );
}

#[test]
fn shutdown_flush_uses_shutdown_message() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    assert!(!store.create_role(&qa).expect("create role").committed);

    let commit_id = store
        .flush_pending_commits_on_shutdown()
        .expect("shutdown flush");
    assert!(commit_id.is_some());
    assert_eq!(
        latest_commit_message(&app_data.join("templates")),
        "templates: shutdown flush 1 changes"
    );
}

#[test]
fn stale_pending_actions_flush_before_enqueueing_new_mutation() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    assert!(!store.create_role(&qa).expect("create qa").committed);
    age_pending_actions(&store, 31);

    let qb = parse_role(&agent_role_yaml("qb", "qb role"));
    let second = store.create_role(&qb).expect("create qb");
    assert!(
        second.committed,
        "creating qb should flush stale qa action before enqueueing qb"
    );

    let state = store.load_state().expect("load state");
    assert_eq!(state.pending_actions.len(), 1);
    assert_eq!(state.pending_actions[0].id, "qb");
    assert_eq!(
        latest_commit_message(&app_data.join("templates")),
        "templates: create role qa"
    );
}

#[test]
fn precommit_validation_failure_preserves_pending_actions() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);
    let store = TemplateStore::with_builtins_and_debounce(app_data.clone(), builtins, 30);
    store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo");

    let qa = parse_role(&agent_role_yaml("qa", "qa role"));
    assert!(!store.create_role(&qa).expect("create role").committed);

    write(
        &app_data
            .join("templates")
            .join("presets")
            .join("invalid.yaml"),
        "not: valid: yaml",
    );
    age_pending_actions(&store, 31);

    let flush_result = store
        .maybe_flush_pending_commits()
        .expect("flush should not error");
    assert!(flush_result.is_none(), "invalid schema should skip commit");

    let state = store.load_state().expect("load state");
    assert!(
        !state.pending_actions.is_empty(),
        "pending actions should remain for later retry"
    );
}
