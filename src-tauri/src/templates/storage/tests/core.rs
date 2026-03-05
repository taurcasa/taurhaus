use super::*;
use std::sync::mpsc;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

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
    assert_eq!(fs::read_to_string(path).expect("read file"), "content-v1");
    let role_dir = app_data.join("templates").join("roles");
    let tmp_entries: Vec<_> = fs::read_dir(&role_dir)
        .expect("read role dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|entry| {
            entry
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.contains(".tmp."))
                .unwrap_or(false)
        })
        .collect();
    assert!(tmp_entries.is_empty(), "tmp file should be cleaned up");
}

#[test]
fn write_atomic_file_supports_concurrent_writers_with_unique_temp_paths() {
    let (_root, app_data, _builtins) = setup_dirs();
    let target = app_data
        .join("templates")
        .join("roles")
        .join("concurrent.yaml");
    fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");

    const WRITERS: usize = 8;
    const WRITES_PER_WRITER: usize = 25;
    let start = Arc::new(Barrier::new(WRITERS));
    let mut handles = Vec::with_capacity(WRITERS);

    for writer_idx in 0..WRITERS {
        let target_path = target.clone();
        let start_barrier = start.clone();
        handles.push(thread::spawn(move || -> Result<(), TemplateStoreError> {
            start_barrier.wait();
            for write_idx in 0..WRITES_PER_WRITER {
                let payload = format!("writer-{writer_idx}-write-{write_idx}");
                write_atomic_file(&target_path, payload.as_bytes())?;
            }
            Ok(())
        }));
    }

    for handle in handles {
        handle
            .join()
            .expect("writer thread should not panic")
            .expect("concurrent atomic writes should succeed");
    }

    let final_payload = fs::read_to_string(&target).expect("read final payload");
    assert!(
        final_payload.starts_with("writer-"),
        "final payload should be one fully written record"
    );
}

#[test]
fn lock_fallback_uses_lockfile_and_serializes_writers() {
    let (_root, app_data, builtins) = setup_dirs();
    let store = TemplateStore::with_builtins_dir(app_data.clone(), builtins);
    store.ensure_directories().expect("ensure directories");

    let lockfile_path = app_data.join("templates").join(LOCK_FALLBACK_FILENAME);
    struct EnvRestore {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match self.previous.as_ref() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
    let env_restore = EnvRestore {
        key: "TAURHAUS_FORCE_TEMPLATE_LOCK_FALLBACK",
        previous: std::env::var_os("TAURHAUS_FORCE_TEMPLATE_LOCK_FALLBACK"),
    };
    std::env::set_var(env_restore.key, "1");

    let guard = store.acquire_lock().expect("first lock acquisition");
    assert!(lockfile_path.exists(), "fallback lockfile should exist");

    let (tx, rx) = mpsc::channel();
    let worker_store = store.clone();
    let worker = thread::spawn(move || {
        let _lock = worker_store
            .acquire_lock()
            .expect("second lock acquisition should eventually succeed");
        tx.send(()).expect("signal lock acquisition");
    });

    thread::sleep(Duration::from_millis(120));
    assert!(
        rx.try_recv().is_err(),
        "second writer should be blocked while first lock is held"
    );

    drop(guard);
    rx.recv_timeout(Duration::from_secs(2))
        .expect("second writer should acquire lock after release");
    worker.join().expect("worker should join");

    assert!(
        !lockfile_path.exists(),
        "fallback lockfile should be cleaned up after release"
    );
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
