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
fn the_tmux_focus_path_contains_no_hook_code() {
    // Regression: commits a53ad31 (hook removal added) and f9c1e89 (focus path
    // None => remove every taurhaus hook) let an env-less daemon launch strip
    // every focus hook, and two installers (app + daemon) with two path
    // authorities could ping-pong `set-hook -g` forever. Focus is now a field
    // of the daemon hub's versioned snapshot: no hooks, no focus file, no
    // inotify watch, so none of that machinery may come back.
    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src"), &mut files);

    let violations: Vec<String> = files
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).ok()?;
            let runtime = runtime_section(&source);
            let markers: Vec<&str> = ["tmux-focus.json", "set-hook", "show-hooks", "focus_hook"]
                .into_iter()
                .filter(|marker| runtime.contains(marker))
                .collect();
            if markers.is_empty() {
                return None;
            }
            Some(format!("{}: {markers:?}", path.display()))
        })
        .collect();

    assert!(
        violations.is_empty(),
        "the tmux focus path must contain no hook or focus-file code: {violations:?}"
    );
}

#[test]
fn coordination_cli_log_sink_test_uses_shared_global_guard() {
    // Regression: ede23e50 added a test that replaces the process-global log sink
    // while holding only a module-local environment lock, racing every guarded log test.
    let source = read_source("src/lib.rs");
    let test_body = source
        .split("fn coordination_cli_log_sink_installs_jsonl_emitter()")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn claude_compact_hook_stdout_writer_emits_only_json_payload()")
                .next()
        })
        .expect("coordination CLI log-sink test body");

    let guard = test_body
        .find("acquire_global_log_test_guard()")
        .expect("global log-sink tests must acquire the shared guard");
    let install = test_body
        .find("init_coordination_cli_log_sink()")
        .expect("test should install the coordination CLI log sink");
    assert!(
        guard < install,
        "the shared global-log guard must be acquired before the sink is replaced"
    );
}
