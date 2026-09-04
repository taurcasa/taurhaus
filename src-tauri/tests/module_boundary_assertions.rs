use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_source(path: &str) -> String {
    fs::read_to_string(crate_root().join(path)).expect("source file should be readable")
}

fn quality_gate_workflow() -> String {
    fs::read_to_string(crate_root().join("../.github/workflows/quality-gate.yml"))
        .expect("quality-gate workflow should be readable")
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

fn braced_body(source: &str, function_start: usize) -> Option<(String, usize)> {
    let open = function_start + source[function_start..].find('{')?;
    let mut depth = 0_usize;
    for (offset, byte) in source.as_bytes()[open..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    let end = open + offset;
                    return Some((source[open + 1..end].to_string(), end + 1));
                }
            }
            _ => {}
        }
    }
    None
}

fn named_function_bodies(source: &str) -> BTreeMap<String, String> {
    let mut bodies = BTreeMap::new();
    let mut cursor = 0_usize;
    while let Some(relative) = source[cursor..].find("fn ") {
        let function_start = cursor + relative;
        let name_start = function_start + 3;
        let name_end = source[name_start..]
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = &source[name_start..name_end];
        let Some((body, end)) = braced_body(source, function_start) else {
            cursor = name_end;
            continue;
        };
        if !name.is_empty() {
            bodies.insert(name.to_string(), body);
        }
        cursor = end;
    }
    bodies
}

fn command_functions(source: &str) -> Vec<(String, String, bool)> {
    let mut commands = Vec::new();
    let mut cursor = 0_usize;
    while let Some(relative) = source[cursor..].find("#[tauri::command") {
        let attribute_start = cursor + relative;
        let Some(attribute_end_offset) = source[attribute_start..].find(']') else {
            break;
        };
        let attribute_end = attribute_start + attribute_end_offset + 1;
        let Some(fn_offset) = source[attribute_end..].find("fn ") else {
            break;
        };
        let function_start = attribute_end + fn_offset;
        let asynchronous = source[attribute_start..attribute_end].contains("(async)")
            || source[attribute_end..function_start].contains("async");
        let name_start = function_start + 3;
        let name_end = source[name_start..]
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .map(|offset| name_start + offset)
            .unwrap_or(source.len());
        let name = source[name_start..name_end].to_string();
        let Some((body, end)) = braced_body(source, function_start) else {
            break;
        };
        commands.push((name, body, asynchronous));
        cursor = end;
    }
    commands
}

fn io_markers(source: &str) -> Vec<&'static str> {
    [
        "send_status_request",
        "daemon_answer",
        "Command::new",
        "std::process",
        "wsl",
        "resolve_launch_base",
        "providers.",
        "provider.",
        "fs::read",
        "fs::metadata",
    ]
    .into_iter()
    .filter(|marker| source.contains(marker))
    .collect()
}

fn sync_io_command_violations(sources: &[(String, String)]) -> Vec<String> {
    let mut implementations = BTreeMap::<String, String>::new();
    for (name, body) in sources
        .iter()
        .flat_map(|(_, source)| named_function_bodies(source))
    {
        implementations.entry(name).or_default().push_str(&body);
    }
    let mut violations = Vec::new();

    for (path, source) in sources {
        for (name, body, asynchronous) in command_functions(source) {
            let mut inspected = body.clone();
            let mut frontier = vec![body];
            let mut visited = BTreeSet::from([name.clone()]);
            while !frontier.is_empty() {
                let mut next = Vec::new();
                for caller in frontier {
                    for (called_name, called_body) in &implementations {
                        if !visited.contains(called_name)
                            && caller.contains(&format!("{called_name}("))
                        {
                            visited.insert(called_name.clone());
                            inspected.push_str(called_body);
                            next.push(called_body.clone());
                        }
                    }
                }
                frontier = next;
            }
            let markers = io_markers(&inspected);
            if !asynchronous && !markers.is_empty() {
                violations.push(format!("{path}::{name}: {markers:?}"));
            }
        }
    }
    violations
}

fn holds_db_guard_across_search_lock(body: &str) -> bool {
    let Some(db_lock) = body.find("let conn = db.0.lock") else {
        return false;
    };
    let Some(search_lock_offset) = body[db_lock..].find("search.0.lock()") else {
        return false;
    };
    let search_lock = db_lock + search_lock_offset;
    let between = &body[db_lock..search_lock];
    if between.contains("drop(conn)") {
        return false;
    }

    let depth_at_db = body[..db_lock].matches('{').count() as isize
        - body[..db_lock].matches('}').count() as isize;
    let mut depth = depth_at_db;
    for byte in between.bytes() {
        match byte {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        if depth < depth_at_db {
            return false;
        }
    }
    true
}

fn collect_repo_source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("directory should be readable") {
        let entry = entry.expect("directory entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if ![".git", "node_modules", "target", "dist"].contains(&name) {
                collect_repo_source_files(&path, out);
            }
            continue;
        }
        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "js" | "svelte" | "ts" | "json" | "yaml" | "yml" | "txt" | "sql")
        ) {
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
    [
        "CliTool::Claude",
        "CliTool::Codex",
        "CliTool::Agy",
        "CliTool::Grok",
    ]
    .into_iter()
    .map(|literal| source.match_indices(literal).count())
    .sum()
}

fn integration_test_binaries_on_disk() -> BTreeSet<String> {
    fs::read_dir(crate_root().join("tests"))
        .expect("integration test directory should be readable")
        .map(|entry| {
            entry
                .expect("integration test entry should be readable")
                .path()
        })
        .filter(|path| {
            path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs")
        })
        .map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .expect("integration test filename should be UTF-8")
                .to_owned()
        })
        .collect()
}

fn show_recipe(recipe: &str) -> String {
    let crate_dir = crate_root();
    let repository = crate_dir.parent().expect("crate lives in repository");
    let output = Command::new("just")
        .current_dir(repository)
        .args(["--show", recipe])
        .output()
        .expect("just should show repository recipes");
    assert!(
        output.status.success(),
        "just --show {recipe} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("shown recipe should be UTF-8")
}

fn evaluate_just_variable(variable: &str) -> String {
    let crate_dir = crate_root();
    let repository = crate_dir.parent().expect("crate lives in repository");
    let output = Command::new("just")
        .current_dir(repository)
        .args(["--evaluate", variable])
        .output()
        .expect("just should evaluate repository variables");
    assert!(
        output.status.success(),
        "just --evaluate {variable} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("evaluated variable should be UTF-8")
}

fn named_test_binaries(recipe: &str) -> BTreeSet<String> {
    let tokens = recipe.split_whitespace().collect::<Vec<_>>();
    tokens
        .windows(2)
        .filter(|window| window[0] == "--test")
        .map(|window| {
            window[1]
                .trim_matches(|character| matches!(character, '\'' | '"' | ';'))
                .to_owned()
        })
        .collect()
}

fn values_after_flag(recipe: &str, flag: &str) -> BTreeSet<String> {
    let tokens = recipe.split_whitespace().collect::<Vec<_>>();
    tokens
        .windows(2)
        .filter(|window| window[0] == flag && !window[1].starts_with('-'))
        .map(|window| {
            window[1]
                .trim_matches(|character| matches!(character, '\'' | '"' | ';'))
                .to_owned()
        })
        .collect()
}

#[test]
fn io_commands_use_tauris_async_execution_context() {
    // Regression: 0.8.4 / PR #75 left every Tauri command synchronous. A slow
    // `resolve_launch_base` daemon RPC in `list_accounts` then held the IPC
    // dispatcher while unrelated project reads hit their 5 s frontend timeout.
    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src/commands"), &mut files);
    collect_rs_files(&crate_root().join("src/workflow_runs"), &mut files);
    let sources = files
        .into_iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some("tests.rs"))
        .map(|path| {
            let relative = path
                .strip_prefix(crate_root())
                .expect("command source lives below the crate root")
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(&path).expect("command source should be readable");
            (relative, source_without_test_only_items(&source))
        })
        .collect::<Vec<_>>();

    let violations = sync_io_command_violations(&sources);
    assert!(
        violations.is_empty(),
        "I/O-capable Tauri commands can block the dispatcher; add #[tauri::command(async)]:\n{}",
        violations.join("\n")
    );
}

#[test]
fn async_execution_guard_follows_one_impl_call() {
    let sources = vec![(
        "src/commands/example.rs".to_string(),
        r#"
#[tauri::command]
pub fn newly_added_probe() {
    newly_added_probe_impl();
}

fn newly_added_probe_impl() {
    daemon.send_status_request(&request);
}
"#
        .to_string(),
    )];

    assert_eq!(
        sync_io_command_violations(&sources),
        vec!["src/commands/example.rs::newly_added_probe: [\"send_status_request\"]".to_string()],
        "a newly added sync RPC command must turn the boundary lane red"
    );
}

#[test]
fn async_execution_guard_follows_transitive_command_helpers() {
    let sources = vec![(
        "src/commands/accounts.rs".to_string(),
        r#"
#[tauri::command]
pub fn list_accounts() {
    list_accounts_impl();
}

fn list_accounts_impl() {
    accounts_report();
}

fn accounts_report() {
    daemon_accounts_report();
}

fn daemon_accounts_report() {
    daemon.send_status_request(&request);
}
"#
        .to_string(),
    )];

    assert_eq!(
        sync_io_command_violations(&sources),
        vec!["src/commands/accounts.rs::list_accounts: [\"send_status_request\"]".to_string()],
        "the flagship read command must stay guarded through ordinary helpers"
    );
}

#[test]
fn async_execution_guard_recognizes_provider_io() {
    let sources = vec![(
        "src/commands/files.rs".to_string(),
        r#"
#[tauri::command]
pub fn read_file(providers: State<'_, ProviderState>) {
    providers.resolve("/project").get_file_content("README.md");
}
"#
        .to_string(),
    )];

    assert_eq!(
        sync_io_command_violations(&sources),
        vec!["src/commands/files.rs::read_file: [\"providers.\"]".to_string()],
        "provider-backed filesystem and daemon calls must stay off the dispatcher"
    );
}

#[test]
fn project_removal_releases_the_db_before_waiting_for_search() {
    // Regression: 47fac80d made `remove_project` and `rebuild_index`
    // concurrent while they acquired DB/search and search/DB respectively.
    // The AB/BA cycle permanently wedged both commands and every later DB IPC.
    let source = source_without_test_only_items(&read_source("src/commands/projects.rs"));
    let functions = named_function_bodies(&source);
    let body = functions
        .get("remove_project")
        .expect("remove_project command body");

    assert!(
        !holds_db_guard_across_search_lock(body),
        "remove_project holds the DB mutex while waiting for the search mutex"
    );
}

#[test]
fn rust_integration_recipe_runs_every_test_binary() {
    // Regression guard (Opus review of the manifest lane, 2026-08-30): the first
    // draft invoked `just` unconditionally, so a bare
    // `cargo test` panicked when that development tool was not installed.
    if Command::new("just").arg("--version").output().is_err() {
        // `just` is how every lane in this repo is invoked, so its absence means
        // a bare `cargo test`; this suite should not fail over a missing
        // development tool.
        eprintln!("skipping: `just` is not installed");
        return;
    }

    // Regression: commit 831571da replaced the integration lane with a
    // hand-maintained target list; later binaries compiled under `cargo check`
    // but never ran because nothing checked that list against `tests/*.rs`.
    let expected = integration_test_binaries_on_disk();
    let shown_recipe = show_recipe("test-rust-integration");
    // Regression guard (same review): the first draft silently fell back to a dry-run parser that
    // cannot evaluate the recipe's backtick-derived integration target list.
    assert!(
        shown_recipe.contains("{{ integration_test_args }}"),
        "the integration recipe no longer derives its targets from integration_test_args"
    );
    let actual = named_test_binaries(&evaluate_just_variable("integration_test_args"));
    let missing = expected.difference(&actual).collect::<Vec<_>>();
    let stale = actual.difference(&expected).collect::<Vec<_>>();

    assert!(
        missing.is_empty() && stale.is_empty(),
        "integration test manifest drifted; missing: {missing:?}; stale: {stale:?}"
    );
}

#[test]
fn heavy_unit_skips_match_integration_reruns() {
    // Regression guard (Opus review of the manifest lane, 2026-08-30): the first
    // draft invoked `just` unconditionally, so a bare
    // `cargo test` panicked when that development tool was not installed.
    if Command::new("just").arg("--version").output().is_err() {
        // `just` is how every lane in this repo is invoked, so its absence means
        // a bare `cargo test`; this suite should not fail over a missing
        // development tool.
        eprintln!("skipping: `just` is not installed");
        return;
    }

    // Regression: commit 831571da introduced separate heavy-suite skip and
    // rerun lists, so one side could change while the other silently drifted.
    let unit_recipe = show_recipe("test-rust-unit");
    let integration_recipe = show_recipe("test-rust-integration");
    let shared_reference = "{{ heavy_rust_test_filters }}";
    let unit_uses_shared_list = unit_recipe.contains(shared_reference);
    let integration_uses_shared_list = integration_recipe.contains(shared_reference);

    assert!(
        unit_uses_shared_list && integration_uses_shared_list,
        "both Rust lanes must source heavy filters from the same manifest"
    );

    let shared = evaluate_just_variable("heavy_rust_test_filters")
        .split_whitespace()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();

    assert!(
        !shared.is_empty(),
        "the heavy manifest must name at least one filter"
    );

    // Regression guard (same review): the first draft compared the shared manifest with itself, so
    // a literal filter appended to either recipe could drift without detection.
    // A `$`-value is only ever the loop variable each lane expands the shared
    // list through; any other shell variable would be a second list the guard
    // cannot see (Opus review of the manifest lane, 2026-08-30).
    let loop_variables = [
        "$skip_args",
        "\"$skip_args\"",
        "$test_filter",
        "\"$test_filter\"",
    ];
    let split_literals = |values: BTreeSet<String>| {
        let mut literals = BTreeSet::new();
        for value in values {
            if value.starts_with('$') || value.starts_with("\"$") {
                assert!(
                    loop_variables.contains(&value.as_str()),
                    "a heavy filter must come from the shared manifest, not another shell variable: {value}"
                );
            } else {
                literals.insert(value);
            }
        }
        literals
    };
    let literal_unit_skips = split_literals(values_after_flag(&unit_recipe, "--skip"));
    let literal_integration_reruns =
        split_literals(values_after_flag(&integration_recipe, "--lib"));
    let unexpected_unit_skips = literal_unit_skips.difference(&shared).collect::<Vec<_>>();
    let unexpected_integration_reruns = literal_integration_reruns
        .difference(&shared)
        .collect::<Vec<_>>();

    assert!(
        unexpected_unit_skips.is_empty(),
        "unit lane has literal heavy skips outside the shared manifest: {unexpected_unit_skips:?}"
    );
    assert!(
        unexpected_integration_reruns.is_empty(),
        "integration lane has literal heavy reruns outside the shared manifest: {unexpected_integration_reruns:?}"
    );

    let mut skipped = shared.clone();
    skipped.extend(literal_unit_skips);
    let mut rerun = shared;
    rerun.extend(literal_integration_reruns);
    assert_eq!(
        skipped, rerun,
        "unit skips and integration reruns must stay in parity"
    );
}

#[test]
fn daemon_listener_fixture_suites_run_without_a_global_thread_pin() {
    // Regression: commit 831571dac globally serialized the daemon fixture
    // suites to contain their released-listener race instead of retaining the
    // already-bound ephemeral sockets across the serving-thread handoff.
    if Command::new("just").arg("--version").output().is_err() {
        eprintln!("skipping: `just` is not installed");
        return;
    }

    let recipe = show_recipe("test-rust-integration");
    let daemon_branch = recipe
        .split("daemon::server::tests::|provider::daemon_client::tests::)")
        .nth(1)
        .and_then(|remainder| remainder.split(";;").next())
        .expect("integration recipe should have a daemon server/client case branch");
    assert!(
        daemon_branch.contains("cargo test --lib"),
        "daemon server/client branch must execute its lib-test filter: {daemon_branch:?}"
    );
    assert!(
        !daemon_branch.contains("--test-threads=1"),
        "daemon server/client fixtures must use Cargo's default parallel test runner: {daemon_branch:?}"
    );
}

#[test]
fn rust_integration_ci_runs_on_every_pull_request() {
    // Regression: commit 797bae05 kept the integration job label-gated after
    // measured warm runs met the PR budget, leaving unlabeled PRs without the
    // seven integration binaries that the recipe currently owns.
    let workflow = quality_gate_workflow();
    let integration_job = workflow
        .split_once("\n  rust-integration:\n")
        .map(|(_, job)| job)
        .expect("quality gate should define the Rust integration job");

    assert!(
        !integration_job.contains("\n    if:"),
        "the Rust integration job must not be gated away on pull requests"
    );
    assert!(
        !workflow.contains("rust-integration')") && !workflow.contains("labeled]"),
        "the measured budget requires the every-PR path, not a label trigger"
    );
}

#[test]
fn main_push_integration_runs_are_never_cancelled_by_newer_pushes() {
    // Regression: commit 797bae05 put the only automatic main-push integration
    // run under unconditional workflow cancellation, so a quick later merge
    // could cancel coverage for the earlier commit without reporting failure.
    let workflow = quality_gate_workflow();

    assert!(
        workflow.contains("cancel-in-progress: ${{ github.event_name == 'pull_request' }}"),
        "only superseded pull-request runs may be cancelled"
    );
    assert!(
        !workflow.contains("cancel-in-progress: true"),
        "main-push integration runs must never use unconditional cancellation"
    );
}

#[test]
fn rust_integration_ci_caches_failed_builds() {
    // Regression: commit 797bae05 cached failed unit builds but not failed
    // integration builds, forcing the next retry to rebuild the same artifacts
    // before it could exercise the integration recipe again.
    let workflow = quality_gate_workflow();
    let integration_job = workflow
        .split_once("\n  rust-integration:\n")
        .map(|(_, job)| job)
        .expect("quality gate should define the Rust integration job");

    assert!(
        integration_job.contains("shared-key: rust-integration\n          cache-on-failure: true"),
        "failed integration builds should seed the next retry's Rust cache"
    );
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
fn task_deadline_policy_stays_pure_and_outside_placeholder_health_framework() {
    let source = read_source("src/coordination/task_deadline.rs");
    let runtime = runtime_section(&source);

    for forbidden in [
        "crate::coordination::health",
        "super::health",
        "health::transition",
        "RecoveryPolicy",
        "Utc::now",
        "Local::now",
        "SystemTime::now",
        "Instant::now",
        "std::fs",
        "std::process",
    ] {
        assert!(
            !runtime.contains(forbidden),
            "task deadline policy crossed its pure-module fence via {forbidden}"
        );
    }
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
// Regression: d593f81b declared the protocol-22 writer boundary closed while
// the app-side Command Center still entered the resume pipeline through an
// allowlisted wrapper and committed team runtime/config state in-process.
fn team_state_write_apis_stay_daemon_or_native_hook_owned() {
    // Each exception is a WSL-side owner or shared implementation reached only
    // from one. Keeping this list explicit makes every new caller a reviewable
    // architecture change.
    const ALLOWED_WRITERS: &[(&str, &str)] = &[
        (
            "src/coordination/backend/bridged.rs",
            "daemon-hosted mesh delivery appends inbox records",
        ),
        (
            "src/coordination/backend/claude.rs",
            "daemon-hosted Claude delivery appends inbox records",
        ),
        (
            "src/coordination/compact_hook.rs",
            "WSL-native compact-hook process publishes reinjection state",
        ),
        (
            "src/coordination/state.rs",
            "daemon coordination state owns the registry and root-keyed orchestrator cache",
        ),
        (
            "src/coordination/operational_context.rs",
            "shared newer-wins publisher is invoked by daemon services",
        ),
        (
            "src/coordination/orchestrator/delivery.rs",
            "daemon-hosted delivery records last-seen and daemon pid after send",
        ),
        (
            "src/coordination/orchestrator/lifecycle.rs",
            "daemon-hosted roster lifecycle owns config/runtime commits",
        ),
        (
            "src/coordination/orchestrator/liveness.rs",
            "daemon-hosted liveness pass owns runtime reconciliation",
        ),
        (
            "src/coordination/pipelines/initialize.rs",
            "daemon-hosted initialize pipeline creates team state",
        ),
        (
            "src/coordination/pipelines/effort.rs",
            "daemon-hosted effort pipeline records applied effort transitions",
        ),
        (
            "src/coordination/pipelines/helpers.rs",
            "daemon-hosted managed launch seam records rendered launch telemetry",
        ),
        (
            "src/coordination/pipelines/lifecycle.rs",
            "daemon-hosted member lifecycle owns config/runtime commits",
        ),
        (
            "src/coordination/pipelines/members.rs",
            "daemon-hosted member pipeline owns runtime CAS commits",
        ),
        (
            "src/coordination/reinjection.rs",
            "WSL-native compact-hook delivery may append mesh inbox state",
        ),
        (
            "src/coordination/runtime/mod.rs",
            "shared daemon runtime quarantine helper clears foreign bindings",
        ),
        (
            "src/coordination/stores/active_project.rs",
            "active-project store implements its own locked write surface",
        ),
        (
            "src/coordination/stores/compaction.rs",
            "compaction store implements the hook-process write surface (WSL-side, plus the native-Windows compact-hook wrapper)",
        ),
        (
            "src/coordination/stores/mesh_task.rs",
            "mesh-task store implements daemon deadline CAS writes",
        ),
        (
            "src/coordination/stores/operational.rs",
            "operational store implements its own locked write surface",
        ),
        (
            "src/coordination/stores/telemetry.rs",
            "routing telemetry store implements the daemon-only append surface",
        ),
        (
            "src/coordination/stores/runtime.rs",
            "runtime store prunes stale WSL-native compaction state during locked saves",
        ),
        (
            "src/coordination/task_deadline_pass.rs",
            "daemon deadline scheduler owns task and snapshot CAS writes",
        ),
        (
            "src/coordination/task_effort.rs",
            "daemon effort outcome emitter records effort-switch telemetry",
        ),
        (
            "src/commands/accounts/mod.rs",
            "read-only account relationship lookup constructs the registry without writing it",
        ),
        (
            "src/services/task_sync.rs",
            "read-only task synchronization enumerates registry-authoritative team locations",
        ),
        (
            "src/task_scanner/claude_index.rs",
            "read-only Claude task indexing enumerates registry-authoritative account roots",
        ),
        (
            "src/daemon/initialize_runs.rs",
            "daemon initialization finalizer publishes snapshots and mappings",
        ),
        (
            "src/daemon/handlers.rs",
            "daemon task scanner records terminal ledger observations",
        ),
        (
            "src/daemon/member_runs.rs",
            "daemon member-run finalizer publishes snapshots and mappings",
        ),
        (
            "src/daemon/roster_runs.rs",
            "daemon roster service owns active-project cleanup",
        ),
        (
            "src/daemon/team_runs.rs",
            "daemon team service refreshes active-project mappings",
        ),
        (
            "src/daemon/state_writes.rs",
            "daemon service hosts the final synchronous writer intents",
        ),
        (
            "src/daemon/session_activity.rs",
            "daemon session-activity hub exports snapshots and reads registry authority",
        ),
        (
            "src/coordination/activity_export.rs",
            "daemon-hosted store module that defines and applies the activity-export writers",
        ),
        (
            "src/coordination/orchestrator/teardown.rs",
            "daemon-hosted teardown quarantines foreign panes during member teardown",
        ),
        (
            "src/commands/coordination/state_sync.rs",
            "cfg(test)-only command fixture (the cfg gate lives on its `mod` declaration in coordination.rs, not in this file) exercises daemon-owned active-project sync",
        ),
    ];
    // KNOWN LIMIT: this is an enumerative scan over write-API spellings, so it
    // cannot be complete against wrapper creation — an allowlisted file that
    // exports a new pub wrapper around a store write mints a spelling this
    // list does not know (the B3 review demonstrated the class twice). The
    // structural closure is the TeamStateWriter capability-token follow-up
    // (a witness type constructible only in daemon/hook entry points, taken
    // by every store write signature); until it lands, every new wrapper
    // around a teams-dir write MUST be added here.
    // Regression: 1d85421a renamed the activity-export writer wrapper without
    // updating this marker, silently dropping its daemon-boundary coverage.
    let markers = [
        ".team_root_registry()",
        "TeamRootRegistry::new(",
        "quarantine_foreign_member(",
        "export_activity_snapshots_for_team_locations(",
        "publish_initialize_snapshot(",
        "apply_delivery_context(",
        "active_project::sync_team_from_config(",
        "ActiveProjectTeamStore::sync_team(",
        "ActiveProjectTeamStore::clear_team(",
        "ActiveProjectTeamStore::clear_project(",
        "ActiveProjectTeamStore::set_active_team(",
        "MemberCompactionStore::save(",
        "MemberCompactionStore::delete(",
        "MemberCompactionStore::delete_without_lock(",
        "prune_state_if_session_mismatch(",
        "record_delivery_at(",
        "TeamConfigStore::save(",
        "AccountSwitchManifestStore::append(",
        "TeamConfigStore::clear_member_pane_binding(",
        "TeamConfigStore::delete(",
        "MeshInboxStore::append(",
        "commit_status_if_unchanged(",
        "append_task_telemetry(",
        "append_task_telemetry_fail_soft(",
        "record_completion_observed(",
        "record_launch_rendered(",
        "record_effort_switch(",
        "record_deadline_action(",
        "OperationalContextSnapshotStore::save(",
        "OperationalContextSnapshotStore::save_locked(",
        "OperationalContextSnapshotStore::commit_if_unchanged(",
        "write_snapshot(",
        "MemberRuntimeStore::save(",
        "MemberRuntimeStore::save_locked(",
        "MemberRuntimeStore::save_preserving_applied_effort(",
        "MemberRuntimeStore::save_preserving_applied_effort_locked(",
        "MemberRuntimeStore::commit_if_unchanged(",
        "MemberRuntimeStore::delete(",
        "MemberRuntimeStore::update(",
        "MemberRuntimeStore::cleanup_stale(",
        "reconcile_team_presence_for_live_status_with_runtime_sessions(",
        "resume_member_with_cli_commands_and_layout(",
        "commit_member_runtime(",
        "publish_member_operation_snapshot(",
    ];

    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src"), &mut files);
    let mut violations = Vec::new();
    let mut used_allowlist_entries = BTreeSet::new();
    for path in files {
        if path.file_name().and_then(|name| name.to_str()) == Some("tests.rs") {
            continue;
        }
        let relative = path
            .strip_prefix(crate_root())
            .expect("source lives below crate root")
            .to_string_lossy()
            .replace('\\', "/");
        let source = fs::read_to_string(&path).expect("Rust source should be readable");
        let runtime = source_without_test_only_items(&source);
        let used = markers
            .iter()
            .filter(|marker| runtime.contains(**marker))
            .copied()
            .collect::<Vec<_>>();
        if used.is_empty() {
            continue;
        }
        let allowed = ALLOWED_WRITERS.iter().any(|(allowed, justification)| {
            assert!(
                !justification.trim().is_empty(),
                "writer exception needs a justification"
            );
            relative == *allowed
        });
        if !allowed {
            violations.push(format!("{relative}: {used:?}"));
        } else {
            used_allowlist_entries.insert(relative);
        }
    }

    let unused_allowlist_entries = ALLOWED_WRITERS
        .iter()
        .map(|(path, _)| *path)
        .filter(|path| !used_allowlist_entries.contains(*path))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "team-state writer escaped the daemon/native-hook boundary:\n{}",
        violations.join("\n")
    );
    assert!(
        unused_allowlist_entries.is_empty(),
        "team-state writer allowlist contains stale exceptions:\n{}",
        unused_allowlist_entries.join("\n")
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
    // Regression: commit 35daf4b added a cfg(test)-only IPC fixture module whose
    // synthetic descriptors are test data, not runtime identity branches.
    // Regression: commit 7e0b2455 exempted that fixture by path, so removing its
    // cfg(test) declaration would silently let runtime identity branches escape.
    const ALLOWED_RUNTIME_FILES: &[&str] = &[
        "src/coordination/compact_hook.rs",
        "src/daemon/agy_hooks.rs",
        "src/models/mod.rs",
        "src/session_scanner/cli_tool.rs",
        "src/session_scanner/accounts/claude.rs",
        "src/session_scanner/accounts/codex.rs",
        "src/session_scanner/accounts/agy.rs",
        "src/session_scanner/accounts/legacy_statusline.rs",
        "src/session_scanner/idle/claude.rs",
        "src/session_scanner/idle/codex.rs",
        "src/session_scanner/idle/agy.rs",
        "src/session_scanner/launch.rs",
        "src/session_scanner/transcript_boundary.rs",
        "src/task_scanner/claude.rs",
        "src/task_scanner/codex.rs",
        "src/templates/adapters.rs",
    ];
    // 86 since the registry gained `command_settings_for_mut`, the mutable
    // mirror of `command_settings_for` that the task-effort relaunch needs to
    // rewrite one tool's configured resume base. Field selection per tool has
    // to name the tools; the registry is where that is allowed to happen.
    const EXPECTED_RUNTIME_LITERAL_COUNT: usize = 86;

    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src"), &mut files);
    let lib_source = read_source("src/lib.rs");
    let lib_lines = lib_source.lines().collect::<Vec<_>>();
    let test_only_module_paths = lib_lines
        .windows(2)
        .filter_map(|lines| {
            if lines[0].trim() != "#[cfg(test)]" {
                return None;
            }
            let module = lines[1].trim().strip_prefix("mod ")?.strip_suffix(';')?;
            Some(format!("src/{module}.rs"))
        })
        .collect::<Vec<_>>();

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
        if test_only_module_paths.contains(&relative) {
            continue;
        }
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

#[test]
fn transcript_compaction_pipeline_stays_retired() {
    // Regression: commit 27770fbd introduced the Codex transcript extractor,
    // watcher, and processor. Native compact hooks now own delivery, so none
    // of those alternate-owner entry points may return.
    for relative in [
        "src/session_scanner/compaction_extractor.rs",
        "src/session_scanner/compaction_watcher.rs",
        "src/coordination/compaction_processor.rs",
        "src/coordination/stores/compaction_signal.rs",
        "src/startup/compaction.rs",
        "src/daemon/compaction.rs",
    ] {
        assert!(
            !crate_root().join(relative).exists(),
            "retired transcript pipeline file returned: {relative}"
        );
    }

    for relative in [
        "src/session_scanner/mod.rs",
        "src/coordination/mod.rs",
        "src/daemon/mod.rs",
    ] {
        let source = read_source(relative);
        assert!(!source.contains("compaction_extractor"), "{relative}");
        assert!(!source.contains("compaction_watcher"), "{relative}");
        assert!(!source.contains("compaction_processor"), "{relative}");
    }
}

#[test]
fn retired_gemini_tool_literal_does_not_return() {
    // Regression: 9a66d1c made Gemini CLI a persisted tool identity throughout
    // the repository. Antigravity is a different binary and must not acquire a
    // compatibility alias; only explicit unknown-value migration tests and old
    // database migrations may retain the retired wire value.
    // Regression: 9a6b9596 reintroduced the retired literal in shipped role
    // prose, bypassing the registry's `agy` identity in user-facing content.
    const ALLOWED_MIGRATION_FILES: &[&str] = &[
        "src/lib/toolRegistry.test.js",
        "src-tauri/src/db/migrations/006_tasks.sql",
        "src-tauri/src/db/migrations/009_task_source_key_identity.sql",
        "src-tauri/src/models/mod.rs",
        "src-tauri/src/session_scanner/cli_tool.rs",
        "src-tauri/src/services/task_query.rs",
        "src-tauri/src/templates/storage/tests/roles.rs",
        "src-tauri/src/coordination/stores/config.rs",
    ];
    const CAPTURED_USAGE_FIXTURE: &str = "src-tauri/src/daemon/fixtures/agy-usage-1.1.22.json";

    let crate_dir = crate_root();
    let repository = crate_dir.parent().expect("crate lives in repository");
    let mut files = Vec::new();
    collect_repo_source_files(repository, &mut files);
    let violations = files
        .into_iter()
        .flat_map(|path| {
            let relative = path
                .strip_prefix(repository)
                .expect("source lives in repository")
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "src-tauri/tests/module_boundary_assertions.rs"
                || relative.starts_with("docs/")
                || relative == CAPTURED_USAGE_FIXTURE
                || ALLOWED_MIGRATION_FILES.contains(&relative.as_str())
            {
                return Vec::new();
            }
            fs::read_to_string(&path)
                .unwrap_or_default()
                .lines()
                .enumerate()
                .filter_map(|(index, line)| {
                    let line = line.to_ascii_lowercase();
                    if !line.contains("gemini") {
                        return None;
                    }
                    // `roles/gemini-*` / `*-gemini.yaml` are historical
                    // bundled-template paths in the storage migration
                    // fingerprint table — data required to recognize and
                    // remove those very files, not a returning identity.
                    let verified_antigravity_data = line.contains("roles/gemini-")
                        || line.contains("-gemini.yaml")
                        || line.contains("gemini-3.")
                        || line.contains("gemini 3.")
                        || line.contains("gemini-\\d")
                        || line.contains("starts_with(\"gemini-\")")
                        || line.contains(".gemini")
                        || line.contains("gemini models")
                        || line.contains("gemini-weekly")
                        || line.contains("gemini-5h")
                        || line.contains("gemini_md")
                        || line.contains("geminimd")
                        || line.contains("gemini.md");
                    (!verified_antigravity_data)
                        .then(|| format!("{relative}:{}: {line}", index + 1))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "retired Gemini tool literals returned outside verified Antigravity data and migration coverage: {violations:?}"
    );
}

#[test]
fn retired_claude_account_bridge_identifiers_do_not_return() {
    // Regression: commits d6839a3 and a574720 made the account pipeline and
    // status-line usage bridge Claude-named end to end, preventing another
    // provider from sharing the core.
    let mut files = Vec::new();
    collect_rs_files(&crate_root().join("src"), &mut files);
    let forbidden = ["claude_accounts", "claude_usage", "claude_statusline"];
    let violations = files
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).ok()?;
            let found = forbidden
                .iter()
                .copied()
                .filter(|identifier| source.contains(identifier))
                .collect::<Vec<_>>();
            (!found.is_empty()).then(|| {
                format!(
                    "{}: {found:?}",
                    path.strip_prefix(crate_root()).unwrap_or(&path).display()
                )
            })
        })
        .collect::<Vec<_>>();
    assert!(
        violations.is_empty(),
        "retired Claude-specific account identifiers returned: {violations:?}"
    );
}

#[test]
fn generic_account_core_contains_no_tool_identity_literals() {
    // Regression: commits d6839a3 and a574720 put tool identity in generic
    // account consumers, then c11770e hardcoded Claude's credential filename
    // in the poller; both identities belong in provider slices only.
    const GENERIC_ACCOUNT_FILES: &[&str] = &[
        "src/session_scanner/accounts/mod.rs",
        "src/daemon/usage_poller.rs",
        "src/commands/accounts/mod.rs",
    ];
    let literals = [
        "CliTool::Claude",
        "CliTool::Codex",
        "CliTool::Agy",
        "CliTool::Grok",
        "\"claude\"",
        "\"codex\"",
        "\"agy\"",
        "\"grok\"",
        "\".credentials.json\"",
    ];
    let violations = GENERIC_ACCOUNT_FILES
        .iter()
        .filter_map(|path| {
            let source = read_source(path);
            let runtime = source_without_test_only_items(&source);
            let found = literals
                .iter()
                .copied()
                .filter(|literal| runtime.contains(literal))
                .collect::<Vec<_>>();
            (!found.is_empty()).then(|| format!("{path}: {found:?}"))
        })
        .collect::<Vec<_>>();
    assert!(
        violations.is_empty(),
        "tool literals escaped provider slices: {violations:?}"
    );
}

#[test]
fn claude_provider_has_no_parallel_generic_account_stack() {
    // Regression: b2ad272 moved detection and resolution behind the generic
    // provider but retained a second public Claude-only cache and resolver,
    // allowing future callers to select the wrong precedence contract.
    let source = read_source("src/session_scanner/accounts/claude.rs");
    for superseded in [
        "pub struct AccountRequest",
        "pub struct AccountResolution",
        "pub struct ClaudeScan",
        "pub fn detect_accounts_in",
        "pub fn scan_claude_config_cached",
        "pub fn detect_accounts_cached",
        "pub fn transcript_config_dirs",
        "pub fn newest_project_transcript",
        "pub fn resolve_launch_account",
    ] {
        assert!(
            !source.contains(superseded),
            "superseded Claude-only account authority remains: {superseded}"
        );
    }
}

#[test]
fn account_usage_http_uses_one_shared_existing_tls_client() {
    // Regression: 2f8246c selected reqwest's rustls feature, which defaults to
    // aws-lc-rs and added a second native crypto toolchain (aws-lc-sys) to the
    // Windows and macOS release graph even though git2 already carries OpenSSL.
    let manifest = read_source("Cargo.toml");
    let reqwest = manifest
        .lines()
        .find(|line| line.starts_with("reqwest = "))
        .expect("direct reqwest dependency");
    assert!(
        reqwest.contains("\"native-tls\""),
        "reqwest must reuse the native TLS stack already in the graph: {reqwest}"
    );
    assert!(
        !reqwest.contains("\"rustls\""),
        "reqwest must not add the rustls/aws-lc stack: {reqwest}"
    );

    let accounts =
        source_without_test_only_items(&read_source("src/session_scanner/accounts/mod.rs"));
    assert!(accounts.contains("static REQWEST_HTTP_CLIENT"));
    assert_eq!(
        accounts
            .match_indices("reqwest::blocking::Client::builder()")
            .count(),
        1,
        "the shared client should be the only client construction site"
    );
}

#[test]
fn scanner_account_memory_never_opens_the_app_database() {
    // Regression: 967f956 opened taurhaus.db from the scanner, so the WSL
    // daemon wrote the Windows app's WAL database through /mnt drvfs and the
    // native daemon raced the app through an unconfigured second connection.
    let accounts =
        source_without_test_only_items(&read_source("src/session_scanner/accounts/mod.rs"));
    assert!(
        !accounts.contains("rusqlite::Connection::open"),
        "account memory must use the app-owned DbState connection"
    );

    let scanner_cache = read_source("src/session_scanner/cache.rs");
    assert!(
        !scanner_cache.contains("record_live_session_accounts"),
        "the scanner may emit observations but must never persist account memory"
    );
}

#[test]
fn configured_account_root_remains_override_only() {
    // Regression: b2ad272 treated Windows' derived WSL Claude root as an
    // explicit override and changed selector-free launch renderings.
    let source = read_source("src/session_scanner/accounts/mod.rs");
    let function = source
        .split("pub fn configured_default_dir")
        .nth(1)
        .and_then(|tail| tail.split("pub fn to_launch_namespace").next())
        .expect("configured_default_dir body");
    assert!(function.contains("claude_dir_override()"));
    assert!(!function.contains("PlatformPaths::claude_dir()"));
}

#[test]
fn legacy_settings_replacement_stays_std_only_and_atomic() {
    // Regression: d91737a rewrote settings.json in place; the first fix then
    // pulled tempfile into production for one same-directory rename.
    let legacy_source = read_source("src/session_scanner/accounts/legacy_statusline.rs");
    let source = runtime_section(&legacy_source);
    assert!(source.contains("settings.json.tmp"));
    assert!(source.contains("fs::rename(&staged_path, settings_path)"));
    assert!(!source.contains("tempfile::"));

    let manifest = read_source("Cargo.toml");
    let production = manifest
        .split("[dev-dependencies]")
        .next()
        .expect("production dependency section");
    assert!(
        !production
            .lines()
            .any(|line| line.starts_with("tempfile = ")),
        "tempfile must remain test-only"
    );
}
