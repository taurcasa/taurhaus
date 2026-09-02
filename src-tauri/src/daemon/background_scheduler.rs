//! Daemon-owned self-heal and pending-effort scheduler.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::coordination::state::{BackgroundSelfHealPassResult, CoordinationState};
use crate::daemon::protocol::{
    CoordinationPutLaunchSettingsParams, CoordinationPutLaunchSettingsResult,
};

const INITIAL_DELAY: Duration = Duration::from_secs(5);
const CHECK_INTERVAL: Duration = Duration::from_secs(30);
const STOP_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SHUTDOWN_JOIN_GRACE: Duration = Duration::from_secs(1);

/// Process-local launch settings pushed by the paired app.
///
/// The daemon deliberately does not invent defaults or persist this snapshot.
#[derive(Debug, Clone, Default)]
pub(crate) struct LaunchSettingsStore {
    current: Arc<Mutex<Option<CoordinationPutLaunchSettingsParams>>>,
}

impl LaunchSettingsStore {
    pub(crate) fn put(
        &self,
        incoming: CoordinationPutLaunchSettingsParams,
    ) -> CoordinationPutLaunchSettingsResult {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current
            .as_ref()
            .is_some_and(|snapshot| snapshot.version > incoming.version)
        {
            return CoordinationPutLaunchSettingsResult {
                accepted: false,
                version: current.as_ref().map_or(0, |snapshot| snapshot.version),
            };
        }

        let version = incoming.version;
        *current = Some(incoming);
        CoordinationPutLaunchSettingsResult {
            accepted: true,
            version,
        }
    }

    pub(crate) fn get(&self) -> Option<CoordinationPutLaunchSettingsParams> {
        self.current
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

pub(crate) struct BackgroundScheduler {
    handle: Option<thread::JoinHandle<()>>,
}

impl BackgroundScheduler {
    pub(crate) fn start(
        state: Arc<CoordinationState>,
        launch_settings: LaunchSettingsStore,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        Self::start_with_cadence(
            state,
            launch_settings,
            shutdown,
            INITIAL_DELAY,
            CHECK_INTERVAL,
        )
    }

    fn start_with_cadence(
        state: Arc<CoordinationState>,
        launch_settings: LaunchSettingsStore,
        shutdown: Arc<AtomicBool>,
        initial_delay: Duration,
        check_interval: Duration,
    ) -> Self {
        let handle = thread::Builder::new()
            .name("coordination-self-heal".to_string())
            .spawn(move || {
                if wait_or_shutdown(&shutdown, initial_delay) {
                    return;
                }
                let mut awaiting_settings_emitted = false;
                while !shutdown.load(Ordering::Relaxed) {
                    let started = Instant::now();
                    match run_pass(state.as_ref(), launch_settings.get()) {
                        Ok((summary, awaiting_settings)) => {
                            if awaiting_settings && !awaiting_settings_emitted {
                                emit_awaiting_settings();
                                awaiting_settings_emitted = true;
                            }
                            emit_pass_completed(&summary, started.elapsed());
                            if summary.team_errors > 0 {
                                tracing::warn!(
                                    teams_scanned = summary.teams_scanned,
                                    team_errors = summary.team_errors,
                                    "daemon coordination self-heal pass completed with errors"
                                );
                            }
                        }
                        Err(error) => {
                            emit_pass_failed(&error.to_string(), started.elapsed(), "pass");
                            tracing::warn!(error = %error, "daemon coordination self-heal pass failed");
                        }
                    }
                    if wait_or_shutdown(&shutdown, check_interval) {
                        return;
                    }
                }
            });
        match handle {
            Ok(handle) => Self {
                handle: Some(handle),
            },
            Err(error) => {
                emit_pass_failed(&error.to_string(), Duration::ZERO, "spawn");
                tracing::warn!(error = %error, "daemon coordination self-heal scheduler not spawned");
                Self { handle: None }
            }
        }
    }

    pub(crate) fn join(mut self) {
        if let Some(handle) = self.handle.take() {
            let deadline = Instant::now() + SHUTDOWN_JOIN_GRACE;
            while !handle.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if !handle.is_finished() {
                tracing::warn!("daemon coordination self-heal scheduler detached during shutdown");
                return;
            }
            let _ = handle.join();
        }
    }
}

fn run_pass(
    state: &CoordinationState,
    launch_settings: Option<CoordinationPutLaunchSettingsParams>,
) -> Result<(BackgroundSelfHealPassResult, bool), crate::coordination::errors::CoordinationError> {
    let prepare_launch_inputs =
        crate::daemon::coordination_runs::daemon_launch_resolver_for(state.teams_dir().clone());
    run_pass_with_launch_resolution(state, launch_settings, &mut |tool, commands| {
        prepare_launch_inputs(tool, commands)
    })
}

fn run_pass_with_launch_resolution(
    state: &CoordinationState,
    launch_settings: Option<CoordinationPutLaunchSettingsParams>,
    resolve_launch_base: &mut dyn FnMut(
        crate::session_scanner::cli_tool::CliTool,
        &mut crate::models::CliCommandSettings,
    ),
) -> Result<(BackgroundSelfHealPassResult, bool), crate::coordination::errors::CoordinationError> {
    let mut summary = state.run_background_self_heal_core_pass()?;
    let Some(launch_settings) = launch_settings else {
        return Ok((summary, true));
    };

    let mut cli_commands = launch_settings.cli_commands;
    let effort = state.run_background_effort_retry_pass_with_launch_resolution(
        &mut cli_commands,
        &launch_settings.tmux_layout,
        resolve_launch_base,
    )?;
    summary.team_errors += effort.team_errors;
    summary.members_effort_resumed += effort.members_effort_resumed;
    Ok((summary, false))
}

fn emit_awaiting_settings() {
    taurhaus_lib::logging::emit_global(
        "info",
        "coordination",
        "effort.sweep.awaiting_settings",
        Some("Daemon effort sweep is awaiting app launch settings".to_string()),
        Map::new(),
    );
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u64::MAX as u128) as u64
}

fn emit_pass_completed(summary: &BackgroundSelfHealPassResult, duration: Duration) {
    let mut fields = Map::new();
    fields.insert(
        "teams_scanned".to_string(),
        Value::from(summary.teams_scanned),
    );
    fields.insert(
        "teams_skipped".to_string(),
        Value::from(summary.teams_skipped),
    );
    fields.insert(
        "teams_reconciled".to_string(),
        Value::from(summary.teams_reconciled),
    );
    fields.insert(
        "team_daemons_ensured".to_string(),
        Value::from(summary.team_daemons_ensured),
    );
    fields.insert("team_errors".to_string(), Value::from(summary.team_errors));
    fields.insert(
        "members_effort_resumed".to_string(),
        Value::from(summary.members_effort_resumed),
    );
    fields.insert(
        "duration_ms".to_string(),
        Value::from(duration_millis(duration)),
    );
    let level = if summary.teams_reconciled
        + summary.team_daemons_ensured
        + summary.team_errors
        + summary.members_effort_resumed
        > 0
    {
        "info"
    } else {
        "debug"
    };
    taurhaus_lib::logging::emit_global(
        level,
        "coordination",
        "self_heal.pass.completed",
        Some("Daemon coordination self-heal pass completed".to_string()),
        fields,
    );
}

fn emit_pass_failed(error: &str, duration: Duration, phase: &str) {
    let mut fields = Map::new();
    fields.insert("error".to_string(), Value::String(error.to_string()));
    fields.insert("phase".to_string(), Value::String(phase.to_string()));
    fields.insert(
        "duration_ms".to_string(),
        Value::from(duration_millis(duration)),
    );
    taurhaus_lib::logging::emit_global(
        "warn",
        "coordination",
        "self_heal.pass.failed",
        Some("Daemon coordination self-heal pass failed".to_string()),
        fields,
    );
}

fn wait_or_shutdown(shutdown: &AtomicBool, duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            return true;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        thread::sleep(remaining.min(STOP_POLL_INTERVAL));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;

    use super::{
        emit_pass_completed, run_pass_with_launch_resolution, BackgroundScheduler,
        LaunchSettingsStore,
    };
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::{
        EffortResumeFailure, MemberRuntimeStore, OperationalAssignmentFooterSnapshot,
        OperationalContextSnapshot, OperationalContextSnapshotStore, OperationalOwnershipSnapshot,
        OperationalTaskSnapshot, OperationalWorkingSetSnapshot, TeamConfigStore,
    };
    use crate::daemon::protocol::CoordinationPutLaunchSettingsParams;
    use crate::session_scanner::cli_tool::CliTool;

    fn settings(version: u64, base: &str) -> CoordinationPutLaunchSettingsParams {
        let mut cli_commands = crate::models::CliCommandSettings::default();
        cli_commands.claude.resume = base.to_string();
        CoordinationPutLaunchSettingsParams {
            version,
            cli_commands,
            tmux_layout: "new_window".to_string(),
        }
    }

    #[test]
    fn launch_settings_snapshot_is_highest_version_wins() {
        let store = LaunchSettingsStore::default();

        let first = store.put(settings(7, "claude2 --resume"));
        let stale = store.put(settings(6, "claude --resume"));
        let snapshot = store.get().expect("newest snapshot retained");

        assert!(first.accepted);
        assert!(!stale.accepted);
        assert_eq!(stale.version, 7);
        assert_eq!(snapshot.version, 7);
        assert_eq!(snapshot.cli_commands.claude.resume, "claude2 --resume");
    }

    fn install_log_tap(root: &std::path::Path) -> std::sync::mpsc::Receiver<serde_json::Value> {
        let log_state =
            crate::commands::logging::LogFileState::new(root.join("taurhaus.log.jsonl"))
                .expect("log state");
        crate::commands::logging::install_global_sink(&log_state);
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        crate::commands::logging::install_test_tap(event_tx);
        event_rx
    }

    fn receive_event(
        event_rx: &std::sync::mpsc::Receiver<serde_json::Value>,
        expected: &str,
    ) -> serde_json::Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let record = event_rx
                .recv_timeout(remaining)
                .unwrap_or_else(|_| panic!("{expected} event within two seconds"));
            if record["event"] == expected {
                return record;
            }
        }
    }

    fn state(teams_dir: std::path::PathBuf) -> CoordinationState {
        state_with_runtime(teams_dir, Arc::new(RecordingCoordinationRuntime::default()))
    }

    fn state_with_runtime(
        teams_dir: std::path::PathBuf,
        runtime: Arc<RecordingCoordinationRuntime>,
    ) -> CoordinationState {
        CoordinationState::with_components_and_runtime(
            teams_dir,
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime.clone()),
        )
    }

    fn member(name: &str, role: MemberRole, tool: CliTool, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role,
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            instructions: None,
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
            project_path: std::path::PathBuf::from(project_path),
            cli_tool: tool,
            extra: Default::default(),
        }
    }

    fn write_lead_credential(teams_dir: &std::path::Path, team_name: &str) {
        let mut config = TeamConfigStore::load(teams_dir, team_name).expect("team config");
        let lead = config
            .members
            .iter_mut()
            .find(|member| member.role == MemberRole::Lead)
            .expect("lead member");
        lead.extra.insert(
            "controlAuthTokenHash".to_string(),
            serde_json::Value::String("sha256:test-token".to_string()),
        );
        lead.extra
            .insert("isActive".to_string(), serde_json::Value::Bool(true));
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save lead auth hash");
        let dir = teams_dir.join(team_name).join("state/control_auth");
        std::fs::create_dir_all(&dir).expect("credential dir");
        std::fs::write(
            dir.join("team-lead.json"),
            r#"{"name":"team-lead","token":"test-token"}"#,
        )
        .expect("lead credential");
    }

    fn assign_task(teams_dir: &std::path::Path) {
        OperationalContextSnapshotStore::save(
            teams_dir,
            &OperationalContextSnapshot {
                version: 1,
                team_name: "effort-team".to_string(),
                member_name: "builder".to_string(),
                updated_at: Utc::now(),
                task: OperationalTaskSnapshot {
                    id: "42".to_string(),
                    subject: "Run the migration".to_string(),
                    status: "in_progress".to_string(),
                    ..Default::default()
                },
                assignment_footer: OperationalAssignmentFooterSnapshot {
                    task_effort: "high".to_string(),
                    task_effort_why: "the migration is irreversible".to_string(),
                    ..Default::default()
                },
                ownership: OperationalOwnershipSnapshot::default(),
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "/tmp/app".to_string(),
                    focal_files: Vec::new(),
                },
            },
        )
        .expect("write operational snapshot");
    }

    #[test]
    fn scheduler_without_settings_self_heals_and_emits_one_bounded_awaiting_record() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let event_rx = install_log_tap(temp.path());
        let teams_dir = temp.path().join("teams");
        std::fs::create_dir_all(&teams_dir).expect("teams dir");
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler = BackgroundScheduler::start_with_cadence(
            Arc::new(state(teams_dir)),
            LaunchSettingsStore::default(),
            shutdown.clone(),
            Duration::ZERO,
            Duration::from_millis(10),
        );

        let awaiting = receive_event(&event_rx, "effort.sweep.awaiting_settings");
        let completed = receive_event(&event_rx, "self_heal.pass.completed");
        let _second_completed = receive_event(&event_rx, "self_heal.pass.completed");
        shutdown.store(true, Ordering::Relaxed);
        scheduler.join();
        let remaining: Vec<_> = event_rx.try_iter().collect();
        crate::commands::logging::clear_test_tap();

        assert_eq!(awaiting["component"], "coordination");
        assert_eq!(completed["fields"]["teams_scanned"], 0);
        assert_eq!(completed["fields"]["members_effort_resumed"], 0);
        assert_eq!(
            remaining
                .iter()
                .filter(|record| record["event"] == "effort.sweep.awaiting_settings")
                .count(),
            0,
            "the absent snapshot is reported once, not once per cycle"
        );
    }

    #[test]
    fn scheduler_emits_a_canonical_failure_record() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let event_rx = install_log_tap(temp.path());
        let teams_dir = temp.path().join("teams-not-a-directory");
        std::fs::write(&teams_dir, "not a directory").expect("invalid teams root");
        let shutdown = Arc::new(AtomicBool::new(false));
        let scheduler = BackgroundScheduler::start_with_cadence(
            Arc::new(state(teams_dir)),
            LaunchSettingsStore::default(),
            shutdown.clone(),
            Duration::ZERO,
            Duration::from_millis(10),
        );

        let failed = receive_event(&event_rx, "self_heal.pass.failed");
        shutdown.store(true, Ordering::Relaxed);
        scheduler.join();
        crate::commands::logging::clear_test_tap();

        assert_eq!(failed["component"], "coordination");
        assert!(failed["fields"]["duration_ms"].is_number());
        assert!(failed["fields"]["error"].is_string());
    }

    // Regression: 50251e68 emitted every successful protocol-21 self-heal pass
    // at debug, so a pass that actually repaired a team or relaunched a member
    // left no operator-visible INFO completion record.
    #[test]
    fn actionable_self_heal_completion_is_emitted_at_info() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let event_rx = install_log_tap(temp.path());
        let summary = crate::coordination::state::BackgroundSelfHealPassResult {
            teams_scanned: 1,
            teams_reconciled: 1,
            ..Default::default()
        };

        emit_pass_completed(&summary, Duration::from_millis(7));
        let completed = receive_event(&event_rx, "self_heal.pass.completed");
        crate::commands::logging::clear_test_tap();

        assert_eq!(completed["level"], "INFO");
        assert_eq!(completed["fields"]["teams_reconciled"], 1);
    }

    // Regression: 50251e68 omitted per-team errors from the protocol-21
    // completion level decision, hiding an errors-only pass from INFO JSONL.
    #[test]
    fn self_heal_completion_with_team_errors_is_emitted_at_info() {
        let _log_guard = crate::test_support::acquire_global_log_test_guard();
        let temp = tempfile::TempDir::new().expect("tempdir");
        let event_rx = install_log_tap(temp.path());
        let summary = crate::coordination::state::BackgroundSelfHealPassResult {
            teams_scanned: 1,
            team_errors: 1,
            ..Default::default()
        };

        emit_pass_completed(&summary, Duration::from_millis(7));
        let completed = receive_event(&event_rx, "self_heal.pass.completed");
        crate::commands::logging::clear_test_tap();

        assert_eq!(completed["level"], "INFO");
        assert_eq!(completed["fields"]["team_errors"], 1);
    }

    // Regression: 06575d68 resolved launch bases for every configured team
    // tool while assembling an idle background pass. Resolution must remain
    // deferred until the unchanged effort state machine selects a relaunch.
    #[test]
    fn idle_effort_sweep_does_not_resolve_launch_bases() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let state = state(temp.path().to_path_buf());
        state
            .with_orchestrator(|orchestrator| {
                orchestrator.create_team("idle-team", None)?;
                orchestrator.add_member(
                    "idle-team",
                    member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                orchestrator.add_member(
                    "idle-team",
                    member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/app"),
                )
            })
            .expect("seed idle team");
        let mut resolutions = 0;

        run_pass_with_launch_resolution(
            &state,
            Some(settings(1, "claude --resume")),
            &mut |_, _| resolutions += 1,
        )
        .expect("idle pass");

        assert_eq!(resolutions, 0, "idle teams must not probe launch bases");
    }

    // Regression: d19ce6a8 limited daemon sweeps to recorded launch failures,
    // so an assignment first seen while the app was not scanning tasks never
    // started its owed effort switch.
    #[test]
    fn effort_sweep_starts_a_new_assignment_without_an_app_trigger() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = state_with_runtime(temp.path().to_path_buf(), runtime.clone());
        state
            .with_orchestrator(|orchestrator| {
                orchestrator.create_team("effort-team", None)?;
                orchestrator.add_member(
                    "effort-team",
                    member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                let mut builder = member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/app");
                builder.reasoning_effort = Some("low".to_string());
                orchestrator.add_member("effort-team", builder)
            })
            .expect("seed team");
        write_lead_credential(temp.path(), "effort-team");
        runtime.set_pane_exists("%21", true);
        runtime.set_pane_dead("%21", false);
        runtime.set_pane_shell("%21", false);
        runtime.set_pane_current_command("%21", Some("codex"));
        runtime.set_pane_current_path("%21", Some("/tmp/app"));
        runtime.set_pane_identity("%21", Some(2021), Some(1_755_000_021));
        runtime.set_detected_runtime_session(
            "%21",
            CliTool::Codex,
            Some("session-effort"),
            Some("/tmp/effort.jsonl"),
        );
        MemberRuntimeStore::update(temp.path(), "effort-team", "builder", |record| {
            record.pane_id = Some("%21".to_string());
            record.pane_pid = Some(2021);
            record.pane_start_time = Some(1_755_000_021);
            record.health = HealthState::Healthy;
            record.session_id = Some("session-effort".to_string());
            record.applied_effort = Some("low".to_string());
            record.effort_resume_failure = None;
        })
        .expect("seed running member");
        assign_task(temp.path());

        let (summary, awaiting) = run_pass_with_launch_resolution(
            &state,
            Some(settings(1, "claude --resume")),
            &mut |_, _| {},
        )
        .expect("daemon sweep");

        assert!(!awaiting);
        assert_eq!(summary.members_effort_resumed, 1);
        assert_eq!(
            MemberRuntimeStore::load(temp.path(), "effort-team", "builder")
                .expect("runtime record")
                .applied_effort
                .as_deref(),
            Some("high")
        );
    }

    // Regression: 25293092 let the background effort path render from stock
    // defaults, moving members launched through account-pinning aliases such
    // as `claude2` onto another account. No pushed snapshot means no render;
    // once pushed, that exact base must reach the renderer.
    #[test]
    fn effort_sweep_waits_for_pushed_settings_and_renders_the_pushed_base() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let state = state_with_runtime(temp.path().to_path_buf(), runtime.clone());
        state
            .with_orchestrator(|orchestrator| {
                orchestrator.create_team("effort-team", None)?;
                orchestrator.add_member(
                    "effort-team",
                    member("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
                )?;
                let mut builder = member("builder", MemberRole::Agent, CliTool::Codex, "/tmp/app");
                builder.reasoning_effort = Some("low".to_string());
                orchestrator.add_member("effort-team", builder)
            })
            .expect("seed team");
        write_lead_credential(temp.path(), "effort-team");
        MemberRuntimeStore::update(temp.path(), "effort-team", "builder", |record| {
            record.health = HealthState::SessionDead;
            record.session_id = Some("session-effort".to_string());
            record.applied_effort = Some("low".to_string());
            record.effort_resume_failure = Some(EffortResumeFailure {
                task_id: "42".to_string(),
                level: "high".to_string(),
                attempts: 1,
                reason: Some("launch failed".to_string()),
            });
        })
        .expect("seed pending retry");
        assign_task(temp.path());

        let sends_before = runtime
            .calls()
            .iter()
            .filter(|call| matches!(call, RuntimeCall::SendKeys { .. }))
            .count();
        let (_summary, awaiting) = run_pass_with_launch_resolution(&state, None, &mut |_, _| {})
            .expect("config-free self-heal pass");
        assert!(awaiting);
        assert_eq!(
            runtime
                .calls()
                .iter()
                .filter(|call| matches!(call, RuntimeCall::SendKeys { .. }))
                .count(),
            sends_before,
            "no launch is rendered before the app pushes settings"
        );
        assert_eq!(
            MemberRuntimeStore::load(temp.path(), "effort-team", "builder")
                .expect("runtime record")
                .effort_resume_failure
                .expect("retry remains pending")
                .attempts,
            1,
            "skipping without settings does not spend the retry budget"
        );

        let pushed = settings(9, "claude2 --resume");
        let mut pushed = pushed;
        pushed.cli_commands.codex.resume = "codex2 resume --last".to_string();
        let (summary, awaiting) =
            run_pass_with_launch_resolution(&state, Some(pushed), &mut |_, _| {})
                .expect("effort retry pass");

        assert!(!awaiting);
        assert_eq!(summary.members_effort_resumed, 1);
        let rendered = runtime
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                RuntimeCall::SendKeys { keys, .. } => Some(keys),
                _ => None,
            })
            .rfind(|keys| keys.contains("codex2"))
            .expect("pushed base reached the launch renderer");
        assert!(rendered.contains("session-effort"), "{rendered}");
    }
}
