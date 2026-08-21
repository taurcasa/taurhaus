use std::fs;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_source(path: &str) -> String {
    fs::read_to_string(crate_root().join(path)).expect("source file should be readable")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn runtime_section(source: &str) -> &str {
    source
        .split("\n#[cfg(test)]")
        .next()
        .expect("split always yields at least one section")
}

#[test]
fn coordination_modules_do_not_import_commands_layer() {
    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src/coordination"), &mut files);

    let violations: Vec<String> = files
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).ok()?;
            if source.contains("crate::commands::") {
                return Some(path.display().to_string());
            }
            None
        })
        .collect();

    assert!(
        violations.is_empty(),
        "coordination modules must not import commands layer: {violations:?}"
    );
}

#[test]
fn daemon_provider_boundary_uses_contract_modules() {
    let handlers = read_source("src/daemon/handlers.rs");
    assert!(
        !runtime_section(&handlers).contains("crate::provider::"),
        "daemon handlers must not import provider module directly"
    );

    let server = read_source("src/daemon/server.rs");
    assert!(
        !runtime_section(&server).contains("crate::provider::"),
        "daemon server runtime must not import provider concrete types"
    );

    let daemon_client = read_source("src/provider/daemon_client.rs");
    let daemon_client_runtime = runtime_section(&daemon_client);
    assert!(
        !daemon_client_runtime.contains("crate::daemon::protocol"),
        "daemon client should use daemon_api protocol boundary"
    );
    assert!(
        !daemon_client_runtime.contains("crate::daemon::auth"),
        "daemon client should use daemon_api auth boundary"
    );
}

#[test]
fn commands_coordination_types_avoids_domain_enum_definitions() {
    let source = read_source("src/commands/coordination_types.rs");
    assert!(
        !source.contains("pub enum LeadMode"),
        "LeadMode must be sourced from coordination contracts"
    );
    assert!(
        !source.contains("pub enum StepStatus"),
        "StepStatus must be sourced from coordination contracts"
    );
    assert!(
        !source.contains("pub enum SessionStatus"),
        "SessionStatus must be sourced from coordination contracts"
    );
}

#[test]
fn mutable_scan_caches_are_not_global_statics() {
    let tasks = read_source("src/commands/tasks.rs");
    assert!(
        !tasks.contains("APPLIED_SCAN_GENERATIONS"),
        "task scan generation cache must be app-managed state, not a static"
    );

    let handlers = read_source("src/daemon/handlers.rs");
    assert!(
        !handlers.contains("PROJECT_TASK_SCAN_CACHE"),
        "daemon project task scan cache must not be a mutable static"
    );
}

#[test]
fn ensure_taurhaus_session_does_not_manage_tmux_focus_hooks() {
    // Regression: commits a53ad31 (removal added) and f9c1e89 (None => remove-all)
    // let an env-less daemon launch path remove every app-owned focus hook.
    let source = read_source("src/session_scanner/control.rs");
    let ensure_body = source
        .split("fn ensure_taurhaus_session()")
        .nth(1)
        .and_then(|tail| {
            tail.split("/// Propagate important environment variables")
                .next()
        })
        .expect("ensure_taurhaus_session source body");

    for hook_fn in [
        "remove_legacy_tmux_focus_hooks",
        "remove_stale_tmux_focus_hooks",
        "install_tmux_focus_hooks",
        "ensure_tmux_focus_hooks_for_path",
        "reconcile_tmux_focus_hooks_for_path",
    ] {
        assert!(
            !ensure_body.contains(hook_fn),
            "daemon launch boundary must not reference app-owned hook function {hook_fn}"
        );
    }
}

#[test]
fn launch_cli_session_repairs_app_owned_tmux_focus_hooks_after_success() {
    // Regression: commit 55fcf0c removed launch-time hook installation without
    // repairing from the app after a cold tmux server was created.
    let source = read_source("src/commands/command_center/mod.rs");
    let launch_body = source
        .split("pub fn launch_cli_session(")
        .nth(1)
        .and_then(|tail| tail.split("#[tauri::command]").next())
        .expect("launch_cli_session source body");

    assert!(
        launch_body.contains("if result.is_ok()")
            && launch_body.contains("crate::startup::watchers::repair_tmux_focus_hooks()"),
        "the app IPC boundary must repair focus hooks after every successful launch"
    );
}

#[test]
fn tmux_focus_cleanup_keeps_the_ea3b44f_regression_guard() {
    // Regression: commit 55fcf0c replaced the ea3b44f ownership regression
    // guard with a None-only test that short-circuited all hook filtering.
    let source = read_source("src/session_scanner/control.rs");

    assert!(
        source.contains("fn legacy_tmux_focus_hook_names_match_only_taurhaus_hooks()"),
        "the ea3b44f hook ownership regression test must remain in the suite"
    );
}

#[test]
fn tmux_focus_reconciliation_uses_one_global_hook_probe() {
    // Regression: commit 55fcf0c made every periodic repair spawn two tmux/WSL
    // probes by inspecting the same global hook state twice.
    let source = read_source("src/session_scanner/control.rs");
    let repair_body = source
        .split("pub(crate) fn reconcile_tmux_focus_hooks_for_path(")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn remove_stale_tmux_focus_hooks(")
                .next()
        })
        .expect("tmux focus reconciliation source body");

    assert_eq!(
        repair_body.matches("show-hooks").count(),
        1,
        "one reconciliation cycle must inspect global tmux hooks only once"
    );
}
