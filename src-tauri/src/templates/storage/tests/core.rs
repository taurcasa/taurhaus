use super::*;
use git2::StatusOptions;
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
fn load_catalog_accepts_repo_builtins_roles_and_presets() {
    let root = TempDir::new().expect("tempdir");
    let app_data = root.path().join("app-data");
    let builtins = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("templates");
    let store = TemplateStore::with_builtins_dir(app_data, builtins);

    let catalog = store.load_catalog().expect("load real built-in catalog");

    for role_id in [
        "claude-design-lead",
        "claude-product-checker",
        "codex-orchestrator",
        "v4-developer-codex",
    ] {
        assert!(
            catalog.roles.iter().any(|role| role.role_id == role_id),
            "expected built-in role {role_id} to load"
        );
    }
    assert!(
        !catalog.presets.is_empty(),
        "expected built-in presets to load with the role catalog"
    );
}

#[test]
fn packaged_builtins_dir_candidates_cover_installed_windows_and_macos_layouts() {
    let windows_exe = PathBuf::from("/installed/taurhaus/taurhaus.exe");
    let windows_candidates = packaged_builtins_dir_candidates(&windows_exe);
    assert_eq!(
        windows_candidates[0],
        PathBuf::from("/installed/taurhaus/resources/templates")
    );

    let macos_exe = PathBuf::from("/Applications/taurhaus.app/Contents/MacOS/taurhaus");
    let macos_candidates = packaged_builtins_dir_candidates(&macos_exe);
    assert!(macos_candidates.contains(&PathBuf::from(
        "/Applications/taurhaus.app/Contents/Resources/resources/templates"
    )));
}

#[test]
fn tauri_bundle_resources_include_template_directories() {
    let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tauri.conf.json"))
        .expect("read tauri config");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse tauri config");
    let resources = json["bundle"]["resources"]
        .as_object()
        .expect("bundle.resources object");
    assert_eq!(
        resources
            .get("resources/templates/roles")
            .and_then(serde_json::Value::as_str),
        Some("resources/templates/roles")
    );
    assert_eq!(
        resources
            .get("resources/templates/presets")
            .and_then(serde_json::Value::as_str),
        Some("resources/templates/presets")
    );
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
        builtin_catalog_revision: 0,
    };

    store.save_state(&state).expect("save state");
    let loaded = store.load_state().expect("load state");

    assert_eq!(loaded, state);
}

#[test]
fn replacing_without_an_atomic_rename_never_leaves_a_partial_file() {
    // Regression: the branch that runs when Windows reports an unsupported
    // rename fell back to `fs::write`, truncating the live file and rewriting
    // it in place — a reader could see half a definition, and an interrupted
    // write left one on disk. Every observable state has to be a whole file.
    let root = TempDir::new().expect("tempdir");
    let target = root.path().join("agents").join("reviewer.md");
    fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
    fs::write(&target, "old contents").expect("existing file");

    let tmp = temp_path_for(&target);
    fs::write(&tmp, "new contents").expect("staged replacement");

    replace_without_atomic_rename(&tmp, &target).expect("replacement succeeds");

    assert_eq!(
        fs::read_to_string(&target).expect("replaced file"),
        "new contents"
    );
    assert!(!tmp.exists(), "the staged replacement was left behind");
    // Deferred cleanup: the displaced copy stays until the next swap's
    // aside-rename replaces it — unlinking it while a handle may hold it is
    // deferred by some servers to handle close and can destroy the TARGET
    // (proven live in coordination::stores::lock). It must hold the whole
    // previous content, never a partial state.
    let displaced = target.with_file_name("reviewer.md.displaced");
    assert_eq!(
        fs::read_to_string(&displaced).expect("displaced copy"),
        "old contents"
    );

    let tmp = temp_path_for(&target);
    fs::write(&tmp, "third contents").expect("staged again");
    replace_without_atomic_rename(&tmp, &target).expect("second replacement settles");
    assert_eq!(
        fs::read_to_string(&target).expect("replaced file"),
        "third contents"
    );
    assert_eq!(
        fs::read_to_string(&displaced).expect("displaced holds the second publish"),
        "new contents"
    );
}

#[test]
fn replacing_without_an_atomic_rename_creates_a_file_that_was_not_there() {
    let root = TempDir::new().expect("tempdir");
    let target = root.path().join("reviewer.md");
    let tmp = temp_path_for(&target);
    fs::write(&tmp, "new contents").expect("staged replacement");

    replace_without_atomic_rename(&tmp, &target).expect("replacement succeeds");

    assert_eq!(
        fs::read_to_string(&target).expect("written file"),
        "new contents"
    );
}

#[test]
fn a_failed_replacement_without_an_atomic_rename_keeps_the_old_file() {
    // Nothing may be reported as written that was not, and a failure has to
    // leave the file that was already there exactly as it was.
    let root = TempDir::new().expect("tempdir");
    let target = root.path().join("reviewer.md");
    fs::write(&target, "old contents").expect("existing file");
    let missing = temp_path_for(&target);

    let error = replace_without_atomic_rename(&missing, &target)
        .expect_err("a replacement that cannot be staged fails");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(
        fs::read_to_string(&target).expect("untouched file"),
        "old contents"
    );
    let leftovers: Vec<_> = fs::read_dir(root.path())
        .expect("read dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(leftovers, vec!["reviewer.md".to_string()]);
}
