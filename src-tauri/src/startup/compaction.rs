use std::path::Path;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager};

use crate::coordination::compaction_events::{
    emit_compaction_owner_failed, emit_compaction_owner_selected,
};
use crate::coordination::compaction_processor::{
    CompactionSignalProcessOutcome, CompactionSignalProcessor,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::TeamConfigStore;
use crate::session_scanner::compaction_extractor;
use crate::session_scanner::compaction_watcher::CompactionSignalWatcherService;
use crate::session_scanner::scan_sessions_for_runtime;

#[cfg(test)]
use crate::session_scanner::compaction_watcher::CompactionSignalWatcher;

pub struct CompactionWatcherState {
    ownership: Mutex<ManagedCompactionOwnership>,
}

struct AppOwnedCompactionRuntime {
    _watcher_service: CompactionSignalWatcherService,
}

struct ManagedCompactionOwnership {
    runtime: Option<AppOwnedCompactionRuntime>,
    external_owner: Option<CompactionOwner>,
}

impl CompactionWatcherState {
    fn inactive() -> Self {
        Self {
            ownership: Mutex::new(ManagedCompactionOwnership {
                runtime: None,
                external_owner: None,
            }),
        }
    }

    fn activate(
        &self,
        start: impl FnOnce() -> Result<AppOwnedCompactionRuntime, CoordinationError>,
    ) -> Result<(), CoordinationError> {
        let mut ownership = self
            .ownership
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if ownership.runtime.is_some() {
            return Ok(());
        }

        ownership.runtime = Some(start()?);
        ownership.external_owner = None;
        Ok(())
    }

    fn select_external(&self, owner: CompactionOwner) -> bool {
        debug_assert!(owner != CompactionOwner::App);
        let runtime = {
            let mut ownership = self
                .ownership
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            ownership.external_owner = Some(owner);
            ownership.runtime.take()
        };
        let released = runtime.is_some();
        drop(runtime);
        if released {
            compaction_extractor::stop_compaction_extractor_service();
        }
        released
    }

    fn owner(&self) -> CompactionOwner {
        let ownership = self
            .ownership
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if ownership.runtime.is_some() {
            CompactionOwner::App
        } else {
            ownership
                .external_owner
                .unwrap_or(CompactionOwner::DaemonPending)
        }
    }
}

impl Drop for CompactionWatcherState {
    fn drop(&mut self) {
        let runtime = self
            .ownership
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .runtime
            .take();
        let was_active = runtime.is_some();
        drop(runtime);
        if was_active {
            compaction_extractor::stop_compaction_extractor_service();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompactionOwner {
    App,
    DaemonPending,
    Daemon,
    Hooks,
}

pub(crate) fn configured_compaction_owner(
    daemon_configured: bool,
    daemon_connected: bool,
) -> CompactionOwner {
    match (daemon_configured, daemon_connected) {
        (false, _) => CompactionOwner::App,
        (true, false) => CompactionOwner::DaemonPending,
        (true, true) => CompactionOwner::Daemon,
    }
}

pub(crate) fn compaction_owner_after_daemon_bootstrap(daemon_connected: bool) -> CompactionOwner {
    if daemon_connected {
        CompactionOwner::Daemon
    } else {
        CompactionOwner::App
    }
}

pub(crate) fn initialize(
    app: &mut tauri::App,
    daemon_configured: bool,
    daemon_connected: bool,
) -> Result<(), CoordinationError> {
    ensure_managed_state(app.handle());
    let mode = crate::commands::terminal_settings::load_terminal_settings(
        &app.state::<crate::commands::projects::DbState>(),
    )
    .harness
    .codex_compaction;
    if hooks_are_active(mode) {
        if daemon_connected {
            notify_daemon_mode(app.handle(), crate::models::CodexCompactionMode::Hooks)?;
        }
        select_hooks(app.handle(), "codex_hook_installed");
        return Ok(());
    }
    if mode == crate::models::CodexCompactionMode::Hooks {
        emit_inactive_hooks_failure();
    }
    match configured_compaction_owner(daemon_configured, daemon_connected) {
        CompactionOwner::App => {
            initialize_app_owned_fallback(app.handle(), "daemon_not_configured")
        }
        CompactionOwner::DaemonPending => {
            let state = app.state::<CompactionWatcherState>();
            if state.owner() == CompactionOwner::DaemonPending {
                emit_compaction_owner_selected("daemon", "pending", "daemon_bootstrap_pending");
            }
            Ok(())
        }
        CompactionOwner::Daemon => {
            notify_daemon_mode(app.handle(), crate::models::CodexCompactionMode::Transcript)?;
            select_daemon(app.handle(), "daemon_connected_at_startup");
            Ok(())
        }
        CompactionOwner::Hooks => unreachable!("hooks are selected before transcript ownership"),
    }
}

pub(crate) fn initialize_app_owned_fallback<R: tauri::Runtime>(
    app: &AppHandle<R>,
    reason: &str,
) -> Result<(), CoordinationError> {
    ensure_managed_state(app);
    let mode = crate::commands::terminal_settings::load_terminal_settings(
        &app.state::<crate::commands::projects::DbState>(),
    )
    .harness
    .codex_compaction;
    if hooks_are_active(mode) {
        select_hooks(app, "codex_hook_installed");
        return Ok(());
    }
    let state = app.state::<CompactionWatcherState>();

    match state.activate(start_app_owned_runtime) {
        Ok(()) => {
            if state.owner() == CompactionOwner::App {
                emit_compaction_owner_selected("app", "active", reason);
            }
            Ok(())
        }
        Err(error) => {
            emit_compaction_owner_failed("app", reason, &error.to_string());
            Err(error)
        }
    }
}

pub(crate) fn release_app_owned_compaction<R: tauri::Runtime>(app: &AppHandle<R>, reason: &str) {
    ensure_managed_state(app);
    let mode = crate::commands::terminal_settings::load_terminal_settings(
        &app.state::<crate::commands::projects::DbState>(),
    )
    .harness
    .codex_compaction;
    let effective_mode = if hooks_are_active(mode) {
        crate::models::CodexCompactionMode::Hooks
    } else {
        crate::models::CodexCompactionMode::Transcript
    };
    if let Err(error) = notify_daemon_mode(app, effective_mode) {
        emit_compaction_owner_failed("daemon", reason, &error.to_string());
        tracing::warn!(reason, error = %error, "failed to reconcile daemon compaction mode");
        return;
    }
    match effective_mode {
        crate::models::CodexCompactionMode::Hooks => select_hooks(app, reason),
        crate::models::CodexCompactionMode::Transcript => select_daemon(app, reason),
    }
}

pub(crate) fn reconcile_compaction_runtime<R: tauri::Runtime>(
    app: &AppHandle<R>,
    mode: crate::models::CodexCompactionMode,
    reason: &str,
) -> Result<(), CoordinationError> {
    ensure_managed_state(app);
    let effective_mode = if hooks_are_active(mode) {
        crate::models::CodexCompactionMode::Hooks
    } else {
        if mode == crate::models::CodexCompactionMode::Hooks {
            emit_inactive_hooks_failure();
        }
        crate::models::CodexCompactionMode::Transcript
    };

    let daemon_connected = app
        .try_state::<crate::ProviderState>()
        .and_then(|provider| provider.daemon.as_ref().map(|daemon| daemon.is_connected()))
        .unwrap_or(false);
    if daemon_connected {
        notify_daemon_mode(app, effective_mode)?;
        match effective_mode {
            crate::models::CodexCompactionMode::Hooks => select_hooks(app, reason),
            crate::models::CodexCompactionMode::Transcript => select_daemon(app, reason),
        }
        return Ok(());
    }

    match effective_mode {
        crate::models::CodexCompactionMode::Hooks => {
            select_hooks(app, reason);
            Ok(())
        }
        crate::models::CodexCompactionMode::Transcript => {
            initialize_app_owned_fallback(app, reason)
        }
    }
}

fn hooks_are_active(mode: crate::models::CodexCompactionMode) -> bool {
    effective_compaction_mode_with_support(
        mode,
        crate::coordination::compact_hook::codex_compact_hook_is_installed(),
        crate::models::CliVersions::current().codex_compaction_hooks_support(),
    ) == crate::models::CodexCompactionMode::Hooks
}

fn emit_inactive_hooks_failure() {
    let (reason, message) = match crate::models::CliVersions::current()
        .codex_compaction_hooks_support()
    {
        None => (
            "codex_version_unknown",
            "Codex version could not be resolved and no installed hook was observed; transcript fallback remains active",
        ),
        Some(false) => (
            "codex_version_unsupported",
            "installed Codex CLI predates native hooks; transcript fallback remains active",
        ),
        Some(true) => (
            "codex_hook_not_installed",
            "configured Codex hook was not observed; transcript fallback remains active",
        ),
    };
    emit_compaction_owner_failed("hooks", reason, message);
}

fn effective_compaction_mode(
    configured: crate::models::CodexCompactionMode,
    hook_installed: bool,
) -> crate::models::CodexCompactionMode {
    if configured == crate::models::CodexCompactionMode::Hooks && hook_installed {
        crate::models::CodexCompactionMode::Hooks
    } else {
        crate::models::CodexCompactionMode::Transcript
    }
}

fn effective_compaction_mode_with_support(
    configured: crate::models::CodexCompactionMode,
    hook_installed: bool,
    hooks_supported: Option<bool>,
) -> crate::models::CodexCompactionMode {
    if hooks_supported == Some(false) {
        return crate::models::CodexCompactionMode::Transcript;
    }
    effective_compaction_mode(configured, hook_installed)
}

fn notify_daemon_mode<R: tauri::Runtime>(
    app: &AppHandle<R>,
    mode: crate::models::CodexCompactionMode,
) -> Result<(), CoordinationError> {
    let Some(provider) = app.try_state::<crate::ProviderState>() else {
        return Ok(());
    };
    let Some(daemon) = provider.daemon.as_ref() else {
        return Ok(());
    };
    if !daemon.is_connected() {
        return Ok(());
    }
    daemon.set_codex_compaction_mode(mode).map_err(|error| {
        CoordinationError::Backend(format!("failed to set daemon compaction mode: {error}"))
    })
}

fn select_hooks<R: tauri::Runtime>(app: &AppHandle<R>, reason: &str) {
    app.state::<CompactionWatcherState>()
        .select_external(CompactionOwner::Hooks);
    emit_compaction_owner_selected("hooks", "active", reason);
}

fn select_daemon<R: tauri::Runtime>(app: &AppHandle<R>, reason: &str) {
    app.state::<CompactionWatcherState>()
        .select_external(CompactionOwner::Daemon);
    emit_compaction_owner_selected("daemon", "active", reason);
}

fn ensure_managed_state<R: tauri::Runtime>(app: &AppHandle<R>) {
    if app.try_state::<CompactionWatcherState>().is_none() {
        let _ = app.manage(CompactionWatcherState::inactive());
    }
}

fn start_app_owned_runtime() -> Result<AppOwnedCompactionRuntime, CoordinationError> {
    let teams_dir = crate::provider::platform_paths::PlatformPaths::teams_dir();
    // Continuity read: this only seeds the extractor's set of Codex transcripts
    // to tail; every later healthy scan republishes that set and supersedes
    // the seed, so a degraded seed is the last good set, not a binding.
    let (initial_sessions, _degraded) = scan_sessions_for_runtime();
    start_app_owned_runtime_at(&teams_dir, initial_sessions)
}

fn start_app_owned_runtime_at(
    teams_dir: &Path,
    initial_sessions: Vec<crate::session_scanner::RuntimeSession>,
) -> Result<AppOwnedCompactionRuntime, CoordinationError> {
    compaction_extractor::start_compaction_extractor_service_at(
        teams_dir.to_path_buf(),
        initial_sessions,
    )?;

    let watcher_service = match start_team_watcher_service(teams_dir) {
        Ok(watcher_service) => watcher_service,
        Err(error) => {
            compaction_extractor::stop_compaction_extractor_service();
            return Err(error);
        }
    };
    Ok(AppOwnedCompactionRuntime {
        _watcher_service: watcher_service,
    })
}

fn start_team_watcher_service(
    teams_dir: &Path,
) -> Result<CompactionSignalWatcherService, CoordinationError> {
    let processor = Arc::new(
        |signal: &crate::coordination::stores::CompactionSignalRecord| {
            match CompactionSignalProcessor::process_signal(signal) {
                CompactionSignalProcessOutcome::Failed { error_message, .. } => Err(error_message),
                _ => Ok(()),
            }
        },
    );

    let mut team_names = Vec::new();
    for team_name in TeamConfigStore::list(teams_dir)? {
        match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    team_name,
                    error = %error,
                    "skipping compaction watcher startup for team without valid config"
                );
                continue;
            }
        }
        team_names.push(team_name);
    }
    CompactionSignalWatcherService::start_at(teams_dir, team_names, processor, Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use chrono::{DateTime, Utc};

    use crate::coordination::compaction_processor::CompactionSignalProcessOutcome;
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::coordination::stores::{
        MemberRuntimeRecord, MemberRuntimeStore, MeshInboxStore,
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfig, TeamConfigStore,
    };
    use crate::session_scanner::cli_tool::CliTool;
    use crate::session_scanner::compaction_extractor::{
        load_compaction_extractor_diagnostics_at, start_compaction_extractor_service_for_test,
        stop_compaction_extractor_service_for_test,
    };
    use crate::session_scanner::compaction_watcher::CompactionSignalWatcherConfig;
    use crate::session_scanner::{
        ActivityAttribution, ActivityConfidence, RuntimeSession, SessionGroupKind, SessionState,
    };

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_member(name: &str, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: Some(format!("{name}-role")),
            role_name: Some(format!("{name} role")),
            focus_area: Some("Keep task execution aligned".to_string()),
            context_summary: Some("Maintains project context".to_string()),
            behavior_summary: Some("Stay concrete and report blockers".to_string()),
            communication_style: None,
            runtime_compact_summary: None,
            instructions: Some("Implement assigned work".to_string()),
            behavioral_contract: None,
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            model: None,
            reasoning_effort: None,
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
        }
    }

    fn sample_snapshot(
        team_name: &str,
        member_name: &str,
        project_path: &str,
    ) -> OperationalContextSnapshot {
        OperationalContextSnapshot {
            version: 1,
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            updated_at: timestamp("2026-03-08T14:10:00Z"),
            task: OperationalTaskSnapshot {
                id: "735".to_string(),
                subject: "Wire extractor watcher processor".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "implement".to_string(),
                file_ownership_boundary: vec![
                    "src-tauri/src/session_scanner/compaction_extractor.rs".to_string(),
                    "src-tauri/src/session_scanner/compaction_watcher.rs".to_string(),
                ],
                adjacent_fix_policy: "local validation only".to_string(),
                validation_expectation: "just check-quick".to_string(),
                response_expectation: "report-on-completion".to_string(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: project_path.to_string(),
                focal_files: vec![
                    "src-tauri/src/session_scanner/compaction_extractor.rs".to_string(),
                    "src-tauri/src/session_scanner/compaction_watcher.rs".to_string(),
                ],
            },
        }
    }

    fn save_team_fixture(teams_dir: &Path, team_name: &str, member: &Member) {
        let config = TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: timestamp("2026-03-08T14:00:00Z"),
            members: vec![member.clone()],
            extra: Default::default(),
        };
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save team config");

        let runtime = MemberRuntimeRecord {
            schema_version: 3,
            member_name: member.name.clone(),
            cli_tool: Some(member.cli_tool),
            project_path: Some(member.project_path.clone()),
            pane_id: Some("%7".to_string()),
            pane_pid: None,
            pane_start_time: None,
            session_id: Some("session-1".to_string()),
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::Healthy,
            delivery_lease: None,
            attached_at: Some(timestamp("2026-03-08T14:01:00Z")),
            last_seen_at: Some(timestamp("2026-03-08T14:02:00Z")),
        };
        MemberRuntimeStore::save(teams_dir, team_name, &member.name, &runtime)
            .expect("save runtime");
        OperationalContextSnapshotStore::save(
            teams_dir,
            &sample_snapshot(
                team_name,
                &member.name,
                &member.project_path.display().to_string(),
            ),
        )
        .expect("save snapshot");
    }

    fn sample_session(project_path: &str, jsonl_path: &Path) -> RuntimeSession {
        RuntimeSession {
            pid: 1234,
            project_path: project_path.to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "codex resume --last".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("main".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some("%7".to_string()),
            tmux_window_name: Some("taurhaus".to_string()),
            state: SessionState::Active,
            session_id: Some("session-1".to_string()),
            jsonl_path: Some(jsonl_path.display().to_string()),
            recent_io: false,
            last_output_age_secs: Some(0),
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if predicate() {
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("condition was not met before timeout");
    }

    fn wait_for_extractor_tracking(teams_dir: &Path, team_name: &str, jsonl_path: &Path) {
        wait_until(Duration::from_secs(5), || {
            load_compaction_extractor_diagnostics_at(teams_dir, team_name)
                .map(|diagnostics| {
                    diagnostics.heartbeat_at.is_some()
                        && diagnostics.active_files.iter().any(|file| {
                            Path::new(&file.jsonl_path) == jsonl_path && file.offset > 0
                        })
                })
                .unwrap_or(false)
        });
    }

    #[test]
    fn extractor_watcher_processor_pipeline_delivers_inbox_message() {
        // Regression: a89ea4c kept a process-global extractor singleton but serialized
        // startup and daemon compaction tests with different module-local mutexes.
        let _extractor_guard = crate::test_support::acquire_compaction_extractor_test_guard();
        stop_compaction_extractor_service_for_test();

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let team_name = "taurhaus-team";
        let project_path = "/home/user/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(&teams_dir, team_name, &member);

        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(
            &jsonl_path,
            "{\"timestamp\":\"2026-03-08T13:46:00.000Z\",\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/home/user/projects/taurhaus\"}}\n",
        )
        .expect("write baseline jsonl");

        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex"));

        let processor_teams_dir = teams_dir.clone();
        let processor_runtime = runtime.clone();
        let _watcher = CompactionSignalWatcher::start_at(
            teams_dir.clone(),
            team_name,
            Arc::new(move |signal: &crate::coordination::stores::CompactionSignalRecord| {
                match crate::coordination::compaction_processor::CompactionSignalProcessor::process_signal_at(
                    signal,
                    &processor_teams_dir,
                    processor_runtime.as_ref(),
                    timestamp("2026-03-08T13:46:42Z"),
                ) {
                    CompactionSignalProcessOutcome::Failed { error_message, .. } => {
                        Err(error_message)
                    }
                    _ => Ok(()),
                }
            }),
            CompactionSignalWatcherConfig {
                reconciliation_interval: Duration::from_millis(100),
                loop_tick: Duration::from_millis(25),
            },
        )
        .expect("start watcher");

        start_compaction_extractor_service_for_test(
            teams_dir.clone(),
            vec![sample_session(project_path, &jsonl_path)],
            Duration::from_millis(25),
        )
        .expect("start extractor service");

        wait_for_extractor_tracking(&teams_dir, team_name, &jsonl_path);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&jsonl_path)
            .expect("open jsonl for append")
            .write_all(
                br#"{"timestamp":"2026-03-08T13:46:41.037Z","type":"compacted","payload":{"replacement_history":[]}}
"#,
            )
            .expect("append compaction line");

        wait_until(Duration::from_secs(5), || {
            MeshInboxStore::load(&teams_dir, team_name, "developer2")
                .map(|messages| !messages.is_empty())
                .unwrap_or(false)
        });

        let inbox = MeshInboxStore::load(&teams_dir, team_name, "developer2").expect("load inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].summary.as_deref(), Some("post_compaction_context"));

        stop_compaction_extractor_service_for_test();
    }

    #[test]
    fn start_team_watcher_service_skips_orphaned_team_dirs_without_failing() {
        let _extractor_guard = crate::test_support::acquire_compaction_extractor_test_guard();

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(teams_dir.join("default")).expect("create orphaned team dir");

        let member = sample_member("developer2", "/home/user/projects/taurhaus");
        save_team_fixture(&teams_dir, "taurhaus-team", &member);

        let _watcher_service =
            start_team_watcher_service(&teams_dir).expect("start watcher service");
    }

    #[test]
    fn failed_daemon_bootstrap_returns_compaction_ownership_to_app() {
        // Regression: a89ea4c treated the presence of a disconnected daemon provider
        // as ownership, leaving no extractor or watcher when daemon bootstrap failed.
        assert_eq!(
            configured_compaction_owner(true, false),
            CompactionOwner::DaemonPending
        );
        assert_eq!(
            compaction_owner_after_daemon_bootstrap(false),
            CompactionOwner::App
        );
    }

    #[test]
    fn daemon_recovery_revokes_app_owned_compaction_fallback() {
        // Regression: 9f723d3 made daemon-bootstrap failure fall back to an app-owned
        // compaction runtime, but neither recovery path revoked it when the daemon
        // later connected, leaving both processes tailing and rewriting the same files.
        let _extractor_guard = crate::test_support::acquire_compaction_extractor_test_guard();
        stop_compaction_extractor_service_for_test();

        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        std::fs::create_dir_all(&teams_dir).expect("create teams dir");
        let state = CompactionWatcherState::inactive();
        state
            .activate(|| start_app_owned_runtime_at(&teams_dir, Vec::new()))
            .expect("start app-owned fallback");
        assert_eq!(state.owner(), CompactionOwner::App);
        assert!(
            crate::session_scanner::compaction_extractor::compaction_extractor_service_is_running_for_test()
        );

        assert!(state.select_external(CompactionOwner::Daemon));
        assert_eq!(state.owner(), CompactionOwner::Daemon);
        assert!(
            !crate::session_scanner::compaction_extractor::compaction_extractor_service_is_running_for_test()
        );
        state
            .activate(|| start_app_owned_runtime_at(&teams_dir, Vec::new()))
            .expect("settings-driven fallback can replace daemon selection");
        assert_eq!(state.owner(), CompactionOwner::App);
        assert!(
            crate::session_scanner::compaction_extractor::compaction_extractor_service_is_running_for_test()
        );

        assert!(state.select_external(CompactionOwner::Daemon));

        for recovery_path in [
            include_str!("daemon.rs"),
            include_str!("../daemon_lifecycle.rs"),
        ] {
            assert!(
                recovery_path.contains("release_app_owned_compaction"),
                "every daemon recovery path must revoke the app-owned fallback"
            );
        }
    }

    #[test]
    fn hooks_mode_requires_an_observed_installed_hook_before_disabling_transcript() {
        // Regression: 6fe0aa3 selected hooks from the setting alone, so a missing
        // or failed hook install silently left no compaction source running.
        assert_eq!(
            effective_compaction_mode(crate::models::CodexCompactionMode::Hooks, false),
            crate::models::CodexCompactionMode::Transcript
        );
        assert_eq!(
            effective_compaction_mode(crate::models::CodexCompactionMode::Hooks, true),
            crate::models::CodexCompactionMode::Hooks
        );
        assert_eq!(
            effective_compaction_mode(crate::models::CodexCompactionMode::Transcript, true),
            crate::models::CodexCompactionMode::Transcript
        );
    }

    #[test]
    fn unknown_codex_version_keeps_an_installed_hook_active() {
        // Regression: c0aa59a collapsed an unresolved Codex version to false in
        // runtime ownership, demoting a hook that reconciliation deliberately kept.
        assert_eq!(
            effective_compaction_mode_with_support(
                crate::models::CodexCompactionMode::Hooks,
                true,
                None,
            ),
            crate::models::CodexCompactionMode::Hooks
        );
        assert_eq!(
            effective_compaction_mode_with_support(
                crate::models::CodexCompactionMode::Hooks,
                true,
                Some(false),
            ),
            crate::models::CodexCompactionMode::Transcript
        );
    }

    #[test]
    fn settings_mode_changes_reconcile_the_running_compaction_owner() {
        // Regression: 6fe0aa3 changed only hooks.json on settings updates, leaving
        // hooks->transcript with no source and transcript->hooks with two sources.
        let settings_source = include_str!("../commands/settings.rs");
        assert!(settings_source.contains("reconcile_compaction_runtime"));
    }
}
