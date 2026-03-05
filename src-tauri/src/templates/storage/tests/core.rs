use super::*;

#[test]
fn ensure_directories_creates_expected_structure() {
    let (_root, app_data, builtins) = setup_dirs();
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);

    store.ensure_directories().expect("ensure directories");

    let templates = app_data.join("templates");
    assert!(templates.join("roles").is_dir());
    assert!(templates.join("presets").is_dir());
    assert!(templates.join("_meta").is_dir());
}

#[test]
fn ensure_repo_for_mutation_initializes_repo_copies_builtins_and_writes_gitignore() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);

    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins.clone());
    let repo = store
        .ensure_repo_for_mutation()
        .expect("ensure repo")
        .expect("repo should initialize");

    assert!(repo.path().exists());
    assert!(app_data
        .join("templates")
        .join("roles")
        .join("lead.yaml")
        .exists());
    assert!(app_data
        .join("templates")
        .join("presets")
        .join("base.yaml")
        .exists());

    let gitignore =
        fs::read_to_string(app_data.join("templates").join(".gitignore")).expect("read gitignore");
    assert!(gitignore.contains("_meta/state.json"));
}

#[test]
fn ensure_repo_for_mutation_falls_back_when_existing_git_dir_is_invalid() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);

    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure dirs");
    fs::create_dir_all(app_data.join("templates").join(".git")).expect("create fake git dir");

    let repo = store.ensure_repo_for_mutation().expect("ensure repo");
    assert!(
        repo.is_none(),
        "invalid git dir should trigger plain filesystem fallback"
    );
}

#[test]
fn load_catalog_merges_builtins_with_user_overrides() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);

    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins.clone());
    store.ensure_directories().expect("ensure dirs");

    write(
        &app_data.join("templates").join("roles").join("dev.yaml"),
        &agent_role_yaml("dev", "dev user override"),
    );

    let catalog = store.load_catalog().expect("load catalog");
    let dev = catalog
        .roles
        .iter()
        .find(|role| role.role_id == "dev")
        .expect("dev role exists");
    assert_eq!(dev.instructions, "dev user override");
    assert!(catalog
        .presets
        .iter()
        .any(|preset| preset.preset_id == "base"));
}

#[test]
fn write_template_file_is_atomic_and_writes_content() {
    let (_root, app_data, builtins) = setup_dirs();
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);

    let rel = Path::new("roles/new-role.yaml");
    store
        .write_template_file(rel, b"content-v1")
        .expect("write file");

    let path = app_data.join("templates").join(rel);
    let tmp = path.with_extension("yaml.tmp");
    assert_eq!(fs::read_to_string(path).expect("read file"), "content-v1");
    assert!(!tmp.exists(), "tmp file should be cleaned up");
}

#[test]
fn recover_dirty_tree_auto_commits_changes() {
    let (_root, app_data, builtins) = setup_dirs();
    seed_valid_catalog(&builtins);

    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store
        .ensure_repo_for_mutation()
        .expect("init repo")
        .expect("repo exists");

    store
        .write_template_file(
            Path::new("roles/lead.yaml"),
            lead_role_yaml("lead", "lead v1").as_bytes(),
        )
        .expect("write lead");
    store
        .write_template_file(
            Path::new("roles/dev.yaml"),
            agent_role_yaml("dev", "dev v1").as_bytes(),
        )
        .expect("write dev");
    store
        .write_template_file(
            Path::new("presets/base.yaml"),
            preset_yaml("base").as_bytes(),
        )
        .expect("write preset");

    let initial_commit = store
        .commit_paths(
            &[
                PathBuf::from("roles/lead.yaml"),
                PathBuf::from("roles/dev.yaml"),
                PathBuf::from("presets/base.yaml"),
            ],
            "templates: seed baseline",
        )
        .expect("initial commit");
    assert!(initial_commit.is_none(), "baseline should be debounced");
    let flushed = store
        .flush_pending_commits()
        .expect("flush baseline pending commit");
    assert!(flushed.is_some(), "baseline should commit when flushed");

    store
        .write_template_file(
            Path::new("roles/dev.yaml"),
            agent_role_yaml("dev", "dev v2").as_bytes(),
        )
        .expect("modify role");

    let recovery_commit = store.recover_dirty_tree().expect("recovery run");
    assert!(
        recovery_commit.is_some(),
        "dirty tree should auto-commit on recovery"
    );

    let repo = Repository::open(app_data.join("templates")).expect("open repo");
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut opts)).expect("statuses");
    let managed_dirty = statuses.iter().any(|entry| {
        entry
            .path()
            .map(|path| is_managed_template_path(Path::new(path)))
            .unwrap_or(false)
    });
    assert!(
        !managed_dirty,
        "managed template files should be clean after recovery commit"
    );
}

#[test]
fn state_round_trip_persists_pending_actions() {
    let (_root, app_data, builtins) = setup_dirs();
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let state = TemplateStoreState {
        pending_actions: vec![PendingAction {
            action: "update".to_string(),
            kind: "role".to_string(),
            id: "dev".to_string(),
            changed_paths: vec!["roles/dev.yaml".to_string()],
            first_seen_at: 1,
            last_seen_at: 2,
        }],
        last_commit_at: Some(99),
        repo_initialized: true,
    };

    store.save_state(&state).expect("save state");
    let loaded = store.load_state().expect("load state");

    assert_eq!(loaded, state);
}
