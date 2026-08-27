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

fn source_without_test_only_items(source: &str) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    let mut runtime = String::new();
    let mut index = 0;

    while index < lines.len() {
        if lines[index].trim() != "#[cfg(test)]" {
            runtime.push_str(lines[index]);
            runtime.push('\n');
            index += 1;
            continue;
        }

        index += 1;
        while index < lines.len() && lines[index].trim_start().starts_with("#[") {
            index += 1;
        }

        let mut brace_depth = 0_i32;
        let mut saw_open_brace = false;
        while index < lines.len() {
            let line = lines[index];
            brace_depth += line.matches('{').count() as i32;
            brace_depth -= line.matches('}').count() as i32;
            saw_open_brace |= line.contains('{');
            index += 1;

            if (saw_open_brace && brace_depth == 0) || (!saw_open_brace && line.contains(';')) {
                break;
            }
        }
    }

    runtime
}

fn cli_tool_literal_count(source: &str) -> usize {
    ["CliTool::Claude", "CliTool::Codex", "CliTool::Gemini"]
        .into_iter()
        .map(|literal| source.match_indices(literal).count())
        .sum()
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

#[test]
fn cli_tool_identity_branches_stay_inside_capability_slices() {
    // Regression: commit 9a66d1c distributed CliTool identity branches across
    // runtime consumers; new harness behavior must live in the registry or a
    // declared per-tool capability slice instead.
    const ALLOWED_RUNTIME_FILES: &[&str] = &[
        "src/coordination/compact_hook.rs",
        "src/models/mod.rs",
        "src/session_scanner/cli_tool.rs",
        "src/session_scanner/compaction_extractor.rs",
        "src/session_scanner/idle/claude.rs",
        "src/session_scanner/idle/codex.rs",
        "src/session_scanner/launch.rs",
        "src/task_scanner/claude.rs",
        "src/task_scanner/codex.rs",
        "src/task_scanner/gemini.rs",
    ];
    const EXPECTED_RUNTIME_LITERAL_COUNT: usize = 60;

    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src"), &mut files);

    let mut allowed_count = 0;
    let mut violations = Vec::new();
    for path in files {
        if path.file_name().and_then(|name| name.to_str()) == Some("tests.rs") {
            continue;
        }
        let relative = path
            .strip_prefix(crate_root())
            .expect("source lives under crate root")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path).expect("Rust source should be readable");
        let runtime = source_without_test_only_items(&source);
        let count = cli_tool_literal_count(&runtime);
        if count == 0 {
            continue;
        }
        if ALLOWED_RUNTIME_FILES.contains(&relative.as_str()) {
            allowed_count += count;
        } else {
            violations.push(format!("{relative}: {count}"));
        }
    }

    assert!(
        violations.is_empty(),
        "CliTool identity literals escaped the registry/capability slices: {violations:?}"
    );
    assert_eq!(
        allowed_count, EXPECTED_RUNTIME_LITERAL_COUNT,
        "update consumers instead of growing the pinned CliTool literal count"
    );
}
