use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use chrono::{DateTime, Utc};
use tempfile::{NamedTempFile, TempDir};

use super::*;
use crate::commands::projects::DbState;
use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::{ActiveProjectTeamStore, MemberRuntimeStore, TeamConfigStore};
use crate::daemon::protocol;
use taurhaus_lib::ProviderState;

/// Test-only team setup fixture. Production initialization is daemon-owned;
/// this directly hosts that pipeline only to prepare state for command tests.
fn initialize_team_pipeline_test_fixture(
    state: &CoordinationState,
    _db: Option<&DbState>,
    request: InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<InitializeReport, String> {
    let request = hydrate_initialize_request_role_metadata(state, request)?;
    validate_initialize_request_fields(&request)?;
    let contract_request = map_initialize_request_to_contract(&request);
    let adapter = InitializeBatchStageProgressAdapter::new(&request.team_name);
    let report = crate::daemon::initialize_runs::execute_initialize_pipeline(
        state,
        &contract_request,
        cli_commands,
        tmux_layout,
        Some(&mut |progress| {
            if let Some(emit) = emit.as_deref_mut() {
                emit(&adapter.event(&progress.step, progress.status, progress.message));
            }
        }),
    )
    .map(map_initialize_report_from_contract)
    .map_err(map_coordination_error)?;
    if report.failed_step.is_none()
        && report
            .succeeded_steps
            .iter()
            .any(|step| step == "create_team")
    {
        sync_active_team_projects_after_change(state, &report.team_name)
            .map_err(map_coordination_error)?;
    }
    Ok(report)
}

/// Test-only hot-add fixture. Production add-agent execution is daemon-owned.
fn add_agent_pipeline_test_fixture(
    state: &CoordinationState,
    db: Option<&DbState>,
    request: AddAgentRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<AddAgentReport, String> {
    let request = hydrate_add_agent_request_role_metadata(state, request)?;
    validate_add_agent_request_fields(&request)?;
    let report = crate::daemon::member_runs::execute_add_agent_pipeline(
        state,
        &map_add_agent_request_to_contract(&request),
        cli_commands,
        tmux_layout,
    )
    .map(map_add_agent_report_from_contract)
    .map_err(map_coordination_error)?;
    if report.failed_step.is_none() {
        if let Some(db) = db {
            sync_member_snapshot_after_change(state, db, &report.team_name, &report.member_name)
                .map_err(map_coordination_error)?;
        }
        sync_active_team_projects_after_change(state, &report.team_name)
            .map_err(map_coordination_error)?;
    }
    emit_progress_events(add_agent_progress_events(&report), emit);
    Ok(report)
}

/// Test-only resume fixture. Production member resume execution is daemon-owned.
fn resume_member_pipeline_test_fixture(
    state: &CoordinationState,
    db: Option<&DbState>,
    request: ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) -> Result<ResumeAgentReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;
    let contract_request = map_resume_member_request_to_contract(&request);
    let report = crate::daemon::member_runs::execute_resume_member_pipeline(
        state,
        &contract_request,
        cli_commands,
        tmux_layout,
        Some(&mut |progress| {
            let event = resume_member_progress_event_for_stage(
                &request.team_name,
                progress.stage,
                progress.status,
                progress.message,
            );
            emit_progress_event(event, &mut emit);
        }),
    )
    .map(map_resume_agent_report_from_contract)
    .map_err(map_coordination_error)?;
    if let Some(db) = db {
        sync_member_snapshot_after_change(state, db, &report.team_name, &report.member_name)
            .map_err(map_coordination_error)?;
    }
    sync_active_team_projects_after_change(state, &report.team_name)
        .map_err(map_coordination_error)?;
    Ok(report)
}

/// Test-only team-resume fixture. Production team resume execution is daemon-owned.
fn coordination_resume_team_internal(
    state: &CoordinationState,
    request: ResumeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(&ResumeTeamProgressEvent)>,
) -> Result<ResumeTeamReport, String> {
    validate_non_empty("team_name", &request.team_name)?;
    let report = crate::daemon::member_runs::execute_resume_team_pipeline(
        state,
        &map_resume_team_request_to_contract(&request),
        cli_commands,
        tmux_layout,
        Some(&mut |progress| {
            if let Some(emit) = emit.as_deref_mut() {
                emit(&resume_team_progress_event(&request.team_name, &progress));
            }
        }),
    )
    .map(map_resume_team_report_from_contract)
    .map_err(map_coordination_error)?;
    sync_active_team_projects_after_change(state, &report.team_name)
        .map_err(map_coordination_error)?;
    Ok(report)
}

/// Test-only reonboard fixture. Production reonboard execution is daemon-owned.
fn coordination_reonboard_impl(
    db: Option<&DbState>,
    state: &CoordinationState,
    request: ReonboardRequest,
) -> Result<DeliveryResult, String> {
    validate_non_empty("team_name", &request.team_name)?;
    validate_non_empty("member_name", &request.member_name)?;
    let result = crate::daemon::team_runs::execute_reonboard_pipeline(
        state,
        &map_reonboard_request_to_contract(&request),
    )
    .map_err(map_coordination_error)?;
    if let Some(db) = db {
        sync_member_snapshot_after_change(state, db, &request.team_name, &request.member_name)
            .map_err(map_coordination_error)?;
    }
    Ok(result)
}

/// Test-only standalone create fixture. Production creation is daemon-owned.
fn coordination_create_team_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    state
        .with_orchestrator(|orchestrator| orchestrator.create_team(&team_name, None).map(|_| ()))
        .map_err(map_coordination_error)
}

/// Test-only disband fixture. Production teardown and cleanup are daemon-owned.
fn coordination_disband_team_impl(
    state: &CoordinationState,
    team_name: String,
) -> Result<DisbandTeamResponse, String> {
    validate_non_empty("team_name", &team_name)?;
    let result = state
        .with_orchestrator(|orchestrator| orchestrator.disband_team(&team_name, None))
        .map_err(map_coordination_error)?;
    ActiveProjectTeamStore::clear_team(state.teams_dir(), &result.team_name)
        .map_err(map_coordination_error)?;
    Ok(DisbandTeamResponse {
        message: if result.already_disbanded {
            "team already disbanded".to_string()
        } else {
            "team disbanded".to_string()
        },
        team_name: result.team_name,
        disbanded: result.disbanded,
        already_disbanded: result.already_disbanded,
    })
}

/// Test-only config roster fixture. Production roster edits are daemon-owned.
fn coordination_add_member_impl(
    state: &CoordinationState,
    team_name: String,
    member_name: String,
    backend_kind: String,
    project_path: Option<String>,
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    validate_non_empty("backend_kind", &backend_kind)?;
    let cli_tool = crate::session_scanner::cli_tool::CliTool::from_alias(&backend_kind)
        .map_err(|_| format!("Validation error: unsupported backend_kind '{backend_kind}'"))?;
    state
        .with_orchestrator(|orchestrator| {
            let status = orchestrator.get_team_status(&team_name)?;
            let project_path = match project_path.as_deref() {
                Some(path) if path.trim().is_empty() => {
                    return Err(CoordinationError::Validation(
                        "project_path must not be empty".to_string(),
                    ));
                }
                Some(path) => std::path::PathBuf::from(path.trim()),
                None => status
                    .config
                    .members
                    .iter()
                    .find(|member| member.role == MemberRole::Lead)
                    .or_else(|| status.config.members.first())
                    .map(|member| member.project_path.clone())
                    .ok_or_else(|| {
                        CoordinationError::Validation(
                            "project_path must be provided for legacy add-member when team has no members"
                                .to_string(),
                        )
                    })?,
            };
            orchestrator.add_member(
                &team_name,
                crate::coordination::domain::Member {
                    name: member_name,
                    role: MemberRole::Agent,
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
                    account_id: None,
                    project_path,
                    cli_tool,
                    extra: Default::default(),
                },
            )
        })
        .map_err(map_coordination_error)
}

// Regression: 5cebfef8 installed the initialize pipeline behind the desktop
// command's process-local orchestrator, so Windows mutated team state across
// the WSL 9p bridge instead of in the daemon's ext4/flock domain.
#[test]
fn initialize_command_has_no_local_pipeline_execution_path() {
    let source = include_str!("../coordination.rs");
    assert!(
        !source.contains("orchestrator.initialize_team_with_cli_commands_and_layout_and_progress")
    );
    assert!(!source.contains("execute_initialize_pipeline("));
    assert!(source.contains("COORDINATION_INITIALIZE_TEAM"));
    assert!(source.contains("COORDINATION_INITIALIZE_STATUS"));
}

// Regression: 9cd9c2d5 left add-agent execution in the desktop process after
// initialization moved into the daemon, preserving the Windows 9p writer.
#[test]
fn add_agent_command_has_no_local_pipeline_execution_path() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("orchestrator.add_agent_to_team_with_cli_commands_and_layout"));
    assert!(!source.contains("execute_add_agent_pipeline("));
    assert!(source.contains("COORDINATION_ADD_AGENT"));
    assert!(source.contains("COORDINATION_ADD_AGENT_STATUS"));
}

// Regression: 9cd9c2d5 left resume-member execution in the desktop process
// after initialization moved into the daemon, preserving the Windows writer.
#[test]
fn resume_member_command_has_no_local_pipeline_execution_path() {
    let source = include_str!("../coordination.rs");
    assert!(
        !source.contains("orchestrator.resume_member_with_cli_commands_and_layout_and_progress")
    );
    assert!(!source.contains("execute_resume_member_pipeline("));
    assert!(source.contains("COORDINATION_RESUME_MEMBER"));
    assert!(source.contains("COORDINATION_RESUME_MEMBER_STATUS"));
}

// Regression: 03eb3a2c made remove-member the app's roster-removal path but
// left a cfg(test) client for the superseded stop-member wire methods.
#[test]
fn stop_member_wire_client_is_removed() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("stop_member_through_daemon_with"));
    assert!(!source.contains("CoordinationStopMemberParams"));
    assert!(!source.contains("COORDINATION_STOP_MEMBER"));
}

// Regression: c6e81abc kept team resume in the desktop process, so reopening a
// Windows project still mutated coordination state over the WSL 9p bridge.
#[test]
fn resume_team_command_has_no_local_pipeline_execution_path() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("orchestrator.resume_team_with_cli_commands_and_layout"));
    assert!(!source.contains("execute_resume_team_pipeline("));
    assert!(source.contains("COORDINATION_RESUME_TEAM"));
    assert!(source.contains("COORDINATION_RESUME_TEAM_STATUS"));
}

// Regression: 5cebfef8 installed reonboard as a desktop-process delivery, so
// its inbox append and wake remained outside the daemon's filesystem domain.
#[test]
fn reonboard_command_has_no_local_pipeline_execution_path() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("orchestrator.deliver_message"));
    assert!(!source.contains("execute_reonboard_pipeline("));
    assert!(source.contains("COORDINATION_REONBOARD"));
    assert!(source.contains("COORDINATION_REONBOARD_STATUS"));
}

// Regression: 5cebfef8 installed standalone team creation in the desktop
// process, so Windows persisted config over the WSL 9p bridge.
#[test]
fn create_team_command_has_no_local_mutation_path() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("orchestrator.create_team"));
    assert!(source.contains("COORDINATION_CREATE_TEAM"));
    assert!(source.contains("COORDINATION_CREATE_TEAM_STATUS"));
}

// Regression: 439d04b1 left disband teardown and its active-project cleanup
// in the desktop process, splitting one operation across the 9p boundary.
#[test]
fn disband_team_command_has_no_local_mutation_path() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("orchestrator.disband_team"));
    assert!(!source.contains("ActiveProjectTeamStore::clear_team"));
    assert!(source.contains("COORDINATION_DISBAND_TEAM"));
    assert!(source.contains("COORDINATION_DISBAND_TEAM_STATUS"));
}

// Regression: commits 460e5df3 and 439d04b1 left live-status presence
// reconciliation in the desktop process, including runtime-record commits.
#[test]
fn live_status_has_no_local_presence_mutation_path() {
    let source = include_str!("live_status.rs");

    assert!(!source.contains("orchestrator.reconcile_team_presence_for_live_status"));
    assert!(source.contains("COORDINATION_RECONCILE_LIVE_PRESENCE"));
}

// Regression: commit 439d04b1 introduced app-side active-project cleanup and
// commit, extending the cross-9p writer set during project discovery.
#[test]
fn project_discovery_has_no_local_active_team_mutation_path() {
    let source = include_str!("live_status.rs");

    assert!(!source.contains("ActiveProjectTeamStore::clear_project"));
    assert!(!source.contains("ActiveProjectTeamStore::set_active_team"));
    assert!(source.contains("COORDINATION_SET_ACTIVE_PROJECT_TEAM"));
}

// Regression: d593f81b gave the final protocol-22 state-write clients a two-second
// transport timeout, so ordinary orchestrator mutex contention disconnected the pool.
#[test]
fn protocol_22_state_write_clients_use_the_coordination_timeout_and_reconnect() {
    let live_status = include_str!("live_status.rs");
    let task_sync = include_str!("../../services/task_sync.rs");

    assert!(live_status.contains("super::COORDINATION_DAEMON_REQUEST_TIMEOUT"));
    assert!(task_sync.contains("COORDINATION_DAEMON_REQUEST_TIMEOUT"));
    assert!(!live_status.contains("Duration::from_secs(2)"));
    assert!(!task_sync.contains("Duration::from_secs(2)"));
    assert!(live_status.contains("if !daemon.is_connected()"));
    assert!(live_status.contains("if !daemon.try_reconnect()"));
}

// Regression: d593f81b emitted one WARN for every two-second live-status poll
// during an outage and let orchestrator contention become a transport timeout;
// d9c3f354 bounded it with one process-global latch, so one team's recovery
// reset another team's outage state.
#[test]
fn protocol_22_live_presence_degrade_warning_is_bounded_and_skips_are_debug_only() {
    let source = include_str!("live_status.rs");

    // The latch is app-managed on CoordinationState (not a global static), so
    // a fresh state starts clean and one team's recovery never touches another.
    let state = test_state(tempfile::tempdir().expect("tempdir").keep());
    assert!(state.mark_live_presence_degraded("team-a"));
    assert!(!state.mark_live_presence_degraded("team-a"));
    assert!(state.mark_live_presence_degraded("team-b"));
    state.mark_live_presence_recovered("team-a");
    assert!(state.mark_live_presence_degraded("team-a"));
    assert!(!state.mark_live_presence_degraded("team-b"));

    assert!(source.contains("state.mark_live_presence_degraded"));
    assert!(!source.contains("AtomicBool"));
    assert!(source.contains("CoordinationReconcileLivePresenceOutcome::Skipped"));
    assert!(!source.contains("is_busy_transport_error(&error)"));
    assert!(source.contains("tracing::debug!"));
}

// Regression: 03eb3a2c polled multi-step disband teardown at the 25 ms
// stop-member interval, producing excessive daemon RPC and JSONL traffic.
#[test]
fn disband_team_uses_the_long_running_daemon_poll_interval() {
    let source = include_str!("../coordination.rs");
    let client = source
        .split("fn disband_team_through_daemon(")
        .nth(1)
        .expect("disband daemon client")
        .split("fn disband_team_through_daemon_with(")
        .next()
        .expect("disband daemon client body");
    assert!(client.contains("COORDINATION_DAEMON_POLL_INTERVAL"));
    assert!(!client.contains("COORDINATION_ROSTER_DAEMON_POLL_INTERVAL"));
}

// Regression: 8bb45dab made daemon launch settings process-local but relied on
// the health monitor to repush them, and d593f81b added two protocol-22 inline
// reconnects without that repush. Any inline reconnect can restore the cached
// connection without entering the recovery hook, leaving the daemon effort
// sweep disabled for its lifetime.
#[test]
fn inline_daemon_reconnect_paths_repush_launch_settings() {
    let coordination = include_str!("../coordination.rs");
    let coordination_call = coordination
        .split("fn call_coordination_daemon(")
        .nth(1)
        .expect("coordination daemon call")
        .split("#[tauri::command]")
        .next()
        .expect("coordination daemon call body");
    let task_sync = include_str!("../../services/task_sync.rs");
    let task_scan = task_sync
        .split("fn scan_tasks_from_files(")
        .nth(1)
        .expect("task scan")
        .split("fn tasks_from_daemon_result(")
        .next()
        .expect("task scan body");
    let snapshot_publish = task_sync
        .split("fn publish_operational_snapshots_through_daemon(")
        .nth(1)
        .expect("snapshot publication client")
        .split("#[cfg(test)]")
        .next()
        .expect("snapshot publication client body");
    let live_status = include_str!("live_status.rs");
    let live_state_write = live_status
        .split("fn call_state_write<")
        .nth(1)
        .expect("live state-write client")
        .split("fn reconcile_live_presence_through_daemon(")
        .next()
        .expect("live state-write client body");
    let runtime_snapshot = include_str!("../runtime_snapshot.rs");
    let runtime_snapshot_call = runtime_snapshot
        .split("pub(crate) fn daemon_runtime_session_snapshot(")
        .nth(1)
        .expect("runtime snapshot call")
        .split("pub(crate) fn request_daemon_runtime_session_snapshot(")
        .next()
        .expect("runtime snapshot call body");
    let daemon_commands = include_str!("../daemon.rs");
    let start_daemon = daemon_commands
        .split("pub fn start_daemon(")
        .nth(1)
        .expect("start daemon command")
        .split("// ---------------------------------------------------------------------------")
        .next()
        .expect("start daemon command body");
    let accounts = include_str!("../accounts/mod.rs");
    let resolve_launch_base = accounts
        .split("fn daemon_resolve_launch_base_tracked(")
        .nth(1)
        .expect("daemon launch-base resolver")
        .split("fn resolved_base_from(")
        .next()
        .expect("daemon launch-base resolver body");
    let project_transcript = accounts
        .split("fn daemon_project_transcript_lookup(")
        .nth(1)
        .expect("daemon project-transcript lookup")
        .split("fn transcript_lookup_from(")
        .next()
        .expect("daemon project-transcript lookup body");
    let list_accounts = accounts
        .split("fn daemon_accounts(")
        .nth(1)
        .expect("daemon accounts lookup")
        .split("fn daemon_answer<")
        .next()
        .expect("daemon accounts lookup body");
    let workflow_runs = include_str!("../../workflow_runs/mod.rs");
    let workflow_request = workflow_runs
        .split("fn daemon_request<T, P>(")
        .nth(1)
        .expect("workflow daemon request")
        .split("#[cfg(test)]")
        .next()
        .expect("workflow daemon request body");

    assert!(coordination_call.contains("push_launch_settings_to_daemon"));
    assert!(task_scan.contains("push_launch_settings_to_daemon"));
    assert!(snapshot_publish.contains("repush_cached_launch_settings_to_daemon"));
    assert!(live_state_write.contains("repush_cached_launch_settings_to_daemon"));
    assert_eq!(
        runtime_snapshot_call
            .matches("repush_cached_launch_settings_to_daemon")
            .count(),
        2,
        "both runtime-snapshot reconnect paths must repush launch settings"
    );
    assert!(start_daemon.contains("push_launch_settings_to_daemon"));
    assert!(resolve_launch_base.contains("repush_cached_launch_settings_to_daemon"));
    assert!(project_transcript.contains("repush_cached_launch_settings_to_daemon"));
    assert!(list_accounts.contains("repush_cached_launch_settings_to_daemon"));
    assert!(workflow_request.contains("repush_cached_launch_settings_to_daemon"));
}

// Regression: 1e1dcea5 kept the config-only roster add in the desktop
// process, including its project-path resolution and runtime-record write.
#[test]
fn add_member_command_has_no_local_mutation_path() {
    let source = include_str!("../coordination.rs");
    assert!(!source.contains("orchestrator.add_member"));
    assert!(!source.contains("resolve_legacy_member_project_path"));
    assert!(source.contains("COORDINATION_ADD_MEMBER"));
    assert!(source.contains("COORDINATION_ADD_MEMBER_STATUS"));
}

// Regression: 639b340e routed roster removal through activation-class stop,
// leaving no distinct daemon intent to pin the interactive roster contract.
#[test]
fn remove_member_command_uses_the_roster_daemon_intent() {
    let source = include_str!("../coordination.rs");
    let command = source
        .split("pub async fn coordination_remove_member")
        .nth(1)
        .expect("remove-member command")
        .split("#[tauri::command")
        .next()
        .expect("remove-member body");
    assert!(command.contains("remove_member_through_daemon"));
    assert!(!command.contains("stop_member_through_daemon"));
    assert!(!command.contains("CoordinationStopMemberParams"));
    assert!(source.contains("COORDINATION_REMOVE_MEMBER"));
    assert!(source.contains("COORDINATION_REMOVE_MEMBER_STATUS"));
}

#[test]
fn initialize_daemon_poll_reemits_the_existing_progress_contract() {
    let params = protocol::CoordinationInitializeParams {
        request: map_initialize_request_to_contract(&sample_preflight_request()),
        cli_commands: crate::models::CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
        operational_snapshots: Vec::new(),
    };
    let progress = crate::coordination::requests::StepProgress {
        step: "validate_configuration".to_string(),
        status: StepStatus::Succeeded,
        message: Some("configuration validated".to_string()),
    };
    let report = crate::coordination::requests::InitializeReport {
        team_name: params.request.team_name.clone(),
        succeeded_steps: vec![progress.step.clone()],
        failed_step: None,
        retryable: false,
        message: "team initialized".to_string(),
        steps: vec![progress.clone()],
    };
    let mut responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationInitializeAccepted {
            run_id: "init-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationInitializeStatus {
            run_id: "init-test".to_string(),
            steps: vec![progress.clone()],
            outcome: protocol::CoordinationInitializeOutcome::Completed {
                report: report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut methods = Vec::new();
    let mut emitted = Vec::new();

    let result = initialize_team_through_daemon_with(
        params,
        Some(&mut |event: &StepProgressEvent| emitted.push(event.clone())),
        std::time::Duration::ZERO,
        |method, _params| {
            methods.push(method.to_string());
            responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected extra daemon call".to_string())
            })
        },
    )
    .expect("daemon initialization completes");

    assert_eq!(result, report);
    assert_eq!(
        methods,
        vec![
            protocol::method::COORDINATION_INITIALIZE_TEAM,
            protocol::method::COORDINATION_INITIALIZE_STATUS,
        ]
    );
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].team_name, "architecture-final");
    assert_eq!(emitted[0].operation, "initialize_team");
    assert_eq!(emitted[0].progress, progress);
    assert_eq!(
        emitted[0].canonical_stages,
        crate::coordination::requests::canonical_member_activation_stages(
            "initialize",
            "validate_configuration",
        )
    );
}

// Regression: 3f8b44ae made one transient status-poll transport failure abort
// the app-side wait while the accepted daemon initialization kept running.
#[test]
fn initialize_daemon_poll_tolerates_a_transient_transport_failure() {
    let params = protocol::CoordinationInitializeParams {
        request: map_initialize_request_to_contract(&sample_preflight_request()),
        cli_commands: crate::models::CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
        operational_snapshots: Vec::new(),
    };
    let report = crate::coordination::requests::InitializeReport {
        team_name: params.request.team_name.clone(),
        succeeded_steps: Vec::new(),
        failed_step: None,
        retryable: false,
        message: "team initialized".to_string(),
        steps: Vec::new(),
    };
    let mut responses = std::collections::VecDeque::from([
        Ok(
            serde_json::to_value(protocol::CoordinationInitializeAccepted {
                run_id: "init-test".to_string(),
            })
            .expect("accepted payload"),
        ),
        Err(CoordinationDaemonCallError::Transport(
            "connection reset by peer".to_string(),
        )),
        Ok(
            serde_json::to_value(protocol::CoordinationInitializeStatus {
                run_id: "init-test".to_string(),
                steps: Vec::new(),
                outcome: protocol::CoordinationInitializeOutcome::Completed {
                    report: report.clone(),
                },
            })
            .expect("status payload"),
        ),
    ]);

    let result = initialize_team_through_daemon_with(
        params,
        None,
        std::time::Duration::ZERO,
        |_method, _params| {
            responses.pop_front().unwrap_or_else(|| {
                Err(CoordinationDaemonCallError::Transport(
                    "unexpected extra daemon call".to_string(),
                ))
            })
        },
    );

    assert_eq!(result, Ok(report));
}

#[test]
fn add_agent_daemon_poll_reemits_the_existing_progress_contract() {
    let request = map_add_agent_request_to_contract(&sample_add_agent_request("arch", "builder"));
    let params = protocol::CoordinationAddAgentParams {
        request,
        cli_commands: CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
        operational_snapshot: None,
        task_state_changed_at: None,
    };
    let progress = crate::coordination::requests::StepProgress {
        step: "validate".to_string(),
        status: StepStatus::Succeeded,
        message: Some("member prepared".to_string()),
    };
    let report = crate::coordination::requests::AddAgentReport {
        team_name: "arch".to_string(),
        member_name: "builder".to_string(),
        succeeded_steps: vec![progress.step.clone()],
        failed_step: None,
        retryable: false,
        message: "member added".to_string(),
        steps: vec![progress.clone()],
        warnings: Vec::new(),
    };
    let mut responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationAddAgentAccepted {
            run_id: "add-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationAddAgentStatus {
            run_id: "add-test".to_string(),
            steps: vec![progress.clone()],
            outcome: protocol::CoordinationAddAgentOutcome::Completed {
                report: report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut methods = Vec::new();
    let mut emitted = Vec::new();

    let result = add_agent_through_daemon_with(
        params,
        Some(&mut |event: &StepProgressEvent| emitted.push(event.clone())),
        std::time::Duration::ZERO,
        |method, _params| {
            methods.push(method.to_string());
            responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected extra daemon call".to_string())
            })
        },
    )
    .expect("daemon add-agent completes");

    assert_eq!(result, report);
    assert_eq!(
        methods,
        vec![
            protocol::method::COORDINATION_ADD_AGENT,
            protocol::method::COORDINATION_ADD_AGENT_STATUS,
        ]
    );
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].operation, "add_agent");
    assert_eq!(emitted[0].progress, progress);
    assert_eq!(
        emitted[0].canonical_stages,
        vec![MemberActivationStage::PrepareMember]
    );
}

#[test]
fn resume_daemon_client_uses_its_run_status_method() {
    let resume_params = protocol::CoordinationResumeMemberParams {
        request: crate::coordination::requests::ResumeMemberRequest {
            team_name: "arch".to_string(),
            member_name: "builder".to_string(),
            reasoning_effort_override: None,
        },
        cli_commands: CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
        operational_snapshot: None,
        task_state_changed_at: None,
    };
    let resume_report = crate::coordination::requests::ResumeAgentReport {
        team_name: "arch".to_string(),
        member_name: "builder".to_string(),
        resumed: true,
        succeeded_steps: Vec::new(),
        failed_step: None,
        retryable: false,
        message: "member resumed".to_string(),
        steps: Vec::new(),
        warnings: Vec::new(),
        pane_id: Some("%2".to_string()),
        reused_pane: false,
    };
    let resume_steps = vec![
        StepProgress {
            step: MemberActivationStage::PrepareMember.as_str().to_string(),
            status: StepStatus::Running,
            message: None,
        },
        StepProgress {
            step: MemberActivationStage::CommitRuntime.as_str().to_string(),
            status: StepStatus::Succeeded,
            message: Some("runtime committed".to_string()),
        },
    ];
    let mut resume_responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationResumeMemberAccepted {
            run_id: "resume-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationResumeMemberStatus {
            run_id: "resume-test".to_string(),
            steps: resume_steps.clone(),
            outcome: protocol::CoordinationResumeMemberOutcome::Completed {
                report: resume_report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut resume_methods = Vec::new();
    let mut emitted_resume_steps = Vec::new();
    let mut emit_resume = |event: &StepProgressEvent| emitted_resume_steps.push(event.clone());
    let resumed = resume_member_through_daemon_with(
        resume_params,
        Some(&mut emit_resume),
        std::time::Duration::ZERO,
        |method, _params| {
            resume_methods.push(method.to_string());
            resume_responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected extra daemon call".to_string())
            })
        },
    )
    .expect("daemon resume completes");
    assert_eq!(resumed, resume_report);
    // Regression: 639b340e dropped the daemon's canonical resume stage while
    // rebuilding the frontend event through the legacy resume-step mapping.
    assert_eq!(emitted_resume_steps.len(), 2);
    assert_eq!(emitted_resume_steps[0].progress, resume_steps[0]);
    assert_eq!(
        emitted_resume_steps[0].canonical_stages,
        vec![MemberActivationStage::PrepareMember]
    );
    assert_eq!(emitted_resume_steps[1].progress, resume_steps[1]);
    assert_eq!(
        emitted_resume_steps[1].canonical_stages,
        vec![MemberActivationStage::CommitRuntime]
    );
    assert_eq!(
        resume_methods,
        vec![
            protocol::method::COORDINATION_RESUME_MEMBER,
            protocol::method::COORDINATION_RESUME_MEMBER_STATUS,
        ]
    );
}

#[test]
fn resume_team_daemon_poll_reemits_the_existing_canonical_progress_contract() {
    let params = protocol::CoordinationResumeTeamParams {
        request: crate::coordination::requests::ResumeTeamRequest {
            team_name: "arch".to_string(),
        },
        cli_commands: CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
    };
    let progress = crate::coordination::requests::ResumeTeamProgress {
        member_name: "builder".to_string(),
        member_index: 2,
        member_count: 3,
        stage: MemberActivationStage::CommitRuntime,
        status: StepStatus::Succeeded,
        message: Some("runtime committed".to_string()),
    };
    let report = crate::coordination::requests::ResumeTeamReport {
        team_name: "arch".to_string(),
        resumed: true,
        total_members: 3,
        resumed_members: vec!["team-lead".to_string(), "builder".to_string()],
        failed_members: Vec::new(),
        warnings: Vec::new(),
        started_team_daemon: true,
        team_daemon_warning: None,
    };
    let mut responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationResumeTeamAccepted {
            run_id: "team-resume-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationResumeTeamStatus {
            run_id: "team-resume-test".to_string(),
            steps: vec![progress.clone()],
            outcome: protocol::CoordinationResumeTeamOutcome::Completed {
                report: report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut methods = Vec::new();
    let mut emitted = Vec::new();

    let resumed = resume_team_through_daemon_with(
        params,
        Some(&mut |event: &ResumeTeamProgressEvent| emitted.push(event.clone())),
        std::time::Duration::ZERO,
        |method, _params| {
            methods.push(method.to_string());
            responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected extra daemon call".to_string())
            })
        },
    )
    .expect("daemon resume-team completes");

    assert_eq!(resumed, report);
    assert_eq!(
        methods,
        [
            protocol::method::COORDINATION_RESUME_TEAM,
            protocol::method::COORDINATION_RESUME_TEAM_STATUS,
        ]
    );
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].operation, "resume_team");
    assert_eq!(emitted[0].team_name, "arch");
    assert_eq!(emitted[0].member_name, progress.member_name);
    assert_eq!(emitted[0].member_index, progress.member_index);
    assert_eq!(emitted[0].member_count, progress.member_count);
    assert_eq!(
        emitted[0].stage,
        MemberActivationStage::CommitRuntime,
        "the shipped polled path must preserve the canonical activation stage"
    );
    assert_eq!(emitted[0].status, progress.status);
    assert_eq!(emitted[0].message, progress.message);
}

#[test]
fn reonboard_daemon_client_uses_its_run_status_method() {
    let params = protocol::CoordinationReonboardParams {
        request: crate::coordination::requests::ReonboardRequest {
            team_name: "arch".to_string(),
            member_name: "builder".to_string(),
        },
        cli_commands: CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
        operational_snapshot: None,
        task_state_changed_at: None,
    };
    let report = crate::coordination::requests::DeliveryResult {
        delivered: true,
        method: crate::coordination::requests::DeliveryMethod::InboxFile,
        durable: true,
        wake: crate::coordination::requests::WakeDisposition::AlreadyLive,
        post_write_warnings: Vec::new(),
    };
    let mut responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationReonboardAccepted {
            run_id: "reonboard-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationReonboardStatus {
            run_id: "reonboard-test".to_string(),
            outcome: protocol::CoordinationReonboardOutcome::Completed {
                report: report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut methods = Vec::new();

    let delivered =
        reonboard_through_daemon_with(params, std::time::Duration::ZERO, |method, _params| {
            methods.push(method.to_string());
            responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected extra daemon call".to_string())
            })
        })
        .expect("daemon reonboard completes");

    assert_eq!(delivered, report);
    assert_eq!(
        methods,
        [
            protocol::method::COORDINATION_REONBOARD,
            protocol::method::COORDINATION_REONBOARD_STATUS,
        ]
    );
}

#[test]
fn switch_team_account_daemon_client_uses_accept_then_poll() {
    let params = protocol::CoordinationSwitchTeamAccountParams {
        request: crate::coordination::requests::SwitchTeamAccountRequest {
            team_name: "arch".to_string(),
            cli_tool: CliTool::Codex,
            account_id: "work".to_string(),
        },
        cli_commands: CliCommandSettings::default(),
        tmux_layout: "new_window".to_string(),
    };
    let report = crate::coordination::requests::SwitchTeamAccountReport {
        team_name: "arch".to_string(),
        cli_tool: CliTool::Codex,
        account_id: "work".to_string(),
        account_label: "Work".to_string(),
        switched_members: vec!["builder".to_string()],
        handoff_manifest_count: 2,
        resume: crate::coordination::requests::ResumeTeamReport {
            team_name: "arch".to_string(),
            resumed: true,
            total_members: 1,
            resumed_members: vec!["builder".to_string()],
            failed_members: Vec::new(),
            warnings: Vec::new(),
            started_team_daemon: true,
            team_daemon_warning: None,
        },
    };
    let mut responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationSwitchTeamAccountAccepted {
            run_id: "account-switch-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationSwitchTeamAccountStatus {
            run_id: "account-switch-test".to_string(),
            outcome: protocol::CoordinationSwitchTeamAccountOutcome::Completed {
                report: report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut methods = Vec::new();

    let switched = switch_team_account_through_daemon_with(
        params,
        std::time::Duration::ZERO,
        |method, _params| {
            methods.push(method.to_string());
            responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected daemon call".to_string())
            })
        },
    )
    .expect("account switch completes");

    assert_eq!(switched, report);
    assert_eq!(
        methods,
        [
            protocol::method::COORDINATION_SWITCH_TEAM_ACCOUNT,
            protocol::method::COORDINATION_SWITCH_TEAM_ACCOUNT_STATUS,
        ]
    );
}

#[test]
fn roster_daemon_clients_use_their_distinct_run_status_methods() {
    let create_params = protocol::CoordinationCreateTeamParams {
        request: crate::coordination::requests::CreateTeamRequest {
            team_name: "arch".to_string(),
        },
    };
    let mut create_responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationCreateTeamAccepted {
            run_id: "create-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationCreateTeamStatus {
            run_id: "create-test".to_string(),
            outcome: protocol::CoordinationCreateTeamOutcome::Completed,
        })
        .expect("status payload"),
    ]);
    let mut create_methods = Vec::new();
    create_team_through_daemon_with(
        create_params,
        std::time::Duration::ZERO,
        |method, _params| {
            create_methods.push(method.to_string());
            create_responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected daemon call".to_string())
            })
        },
    )
    .expect("daemon create completes");
    assert_eq!(
        create_methods,
        [
            protocol::method::COORDINATION_CREATE_TEAM,
            protocol::method::COORDINATION_CREATE_TEAM_STATUS,
        ]
    );

    let disband_report = crate::coordination::requests::DisbandTeamReport {
        team_name: "arch".to_string(),
        disbanded: true,
        already_disbanded: false,
        message: "team disbanded".to_string(),
    };
    let mut disband_responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationDisbandTeamAccepted {
            run_id: "disband-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationDisbandTeamStatus {
            run_id: "disband-test".to_string(),
            outcome: protocol::CoordinationDisbandTeamOutcome::Completed {
                report: disband_report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut disband_methods = Vec::new();
    let disbanded = disband_team_through_daemon_with(
        protocol::CoordinationDisbandTeamParams {
            request: crate::coordination::requests::DisbandTeamRequest {
                team_name: "arch".to_string(),
            },
        },
        std::time::Duration::ZERO,
        |method, _params| {
            disband_methods.push(method.to_string());
            disband_responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected daemon call".to_string())
            })
        },
    )
    .expect("daemon disband completes");
    assert_eq!(disbanded, disband_report);
    assert_eq!(
        disband_methods,
        [
            protocol::method::COORDINATION_DISBAND_TEAM,
            protocol::method::COORDINATION_DISBAND_TEAM_STATUS,
        ]
    );

    let mut add_responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationAddMemberAccepted {
            run_id: "member-add-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationAddMemberStatus {
            run_id: "member-add-test".to_string(),
            outcome: protocol::CoordinationAddMemberOutcome::Completed,
        })
        .expect("status payload"),
    ]);
    let mut add_methods = Vec::new();
    add_member_through_daemon_with(
        protocol::CoordinationAddMemberParams {
            request: crate::coordination::requests::AddMemberRequest {
                team_name: "arch".to_string(),
                member_name: "builder".to_string(),
                backend_kind: "codex".to_string(),
                project_path: Some("/work/arch".to_string()),
            },
        },
        std::time::Duration::ZERO,
        |method, _params| {
            add_methods.push(method.to_string());
            add_responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected daemon call".to_string())
            })
        },
    )
    .expect("daemon roster add completes");
    assert_eq!(
        add_methods,
        [
            protocol::method::COORDINATION_ADD_MEMBER,
            protocol::method::COORDINATION_ADD_MEMBER_STATUS,
        ]
    );

    let remove_report = crate::coordination::requests::StopMemberReport {
        team_name: "arch".to_string(),
        member_name: "builder".to_string(),
        removed: true,
        message: "member removed".to_string(),
        steps: Vec::new(),
        warnings: Vec::new(),
    };
    let mut remove_responses = std::collections::VecDeque::from([
        serde_json::to_value(protocol::CoordinationRemoveMemberAccepted {
            run_id: "member-remove-test".to_string(),
        })
        .expect("accepted payload"),
        serde_json::to_value(protocol::CoordinationRemoveMemberStatus {
            run_id: "member-remove-test".to_string(),
            outcome: protocol::CoordinationRemoveMemberOutcome::Completed {
                report: remove_report.clone(),
            },
        })
        .expect("status payload"),
    ]);
    let mut remove_methods = Vec::new();
    let removed = remove_member_through_daemon_with(
        protocol::CoordinationRemoveMemberParams {
            request: crate::coordination::requests::RemoveMemberRequest {
                team_name: "arch".to_string(),
                member_name: "builder".to_string(),
            },
        },
        std::time::Duration::ZERO,
        |method, _params| {
            remove_methods.push(method.to_string());
            remove_responses.pop_front().ok_or_else(|| {
                CoordinationDaemonCallError::Transport("unexpected daemon call".to_string())
            })
        },
    )
    .expect("daemon roster removal completes");
    assert_eq!(removed, remove_report);
    assert_eq!(
        remove_methods,
        [
            protocol::method::COORDINATION_REMOVE_MEMBER,
            protocol::method::COORDINATION_REMOVE_MEMBER_STATUS,
        ]
    );
}

#[test]
fn roster_mutations_use_a_snappy_daemon_poll_interval() {
    // Regression: 639b340e made the formerly inline roster interaction wait
    // up to the shared 500 ms long-running-operation poll interval.
    assert!(COORDINATION_ROSTER_DAEMON_POLL_INTERVAL <= std::time::Duration::from_millis(50));
}

#[test]
fn task_effort_client_carries_fresh_settings_and_polls_the_shared_registry() {
    let mut commands = crate::models::CliCommandSettings::default();
    commands.claude.resume = "claude2 --resume".to_string();
    let params = crate::daemon::protocol::CoordinationApplyTaskEffortParams {
        project_path: "/tmp/task-project".to_string(),
        cli_commands: commands,
        tmux_layout: "split".to_string(),
    };
    let mut calls = Vec::new();

    let report = apply_task_effort_through_daemon_with(
        params,
        std::time::Duration::ZERO,
        |method, params| {
            calls.push((method.to_string(), params.clone()));
            match method {
                crate::daemon::protocol::method::COORDINATION_APPLY_TASK_EFFORT => {
                    Ok(serde_json::json!({ "run_id": "effort_test" }))
                }
                crate::daemon::protocol::method::COORDINATION_APPLY_TASK_EFFORT_STATUS => {
                    Ok(serde_json::json!({
                        "run_id": "effort_test",
                        "outcome": {
                            "status": "completed",
                            "report": {
                                "switched": ["builder"],
                                "failed": [],
                                "skipped_teams": []
                            }
                        }
                    }))
                }
                _ => panic!("unexpected method: {method}"),
            }
        },
    )
    .expect("task-effort intent completes");

    assert_eq!(report.switched, vec!["builder"]);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].1["project_path"], "/tmp/task-project");
    assert_eq!(calls[0].1["tmux_layout"], "split");
    assert_eq!(
        calls[0].1["cli_commands"]["claude"]["resume"],
        "claude2 --resume"
    );
}

// Regression: 8bb45dab routed task-effort relaunches through the multi-second
// daemon run registry but polled them at the 25 ms roster-interaction cadence,
// creating unnecessary status traffic while no progress is rendered.
#[test]
fn task_effort_uses_the_long_running_daemon_poll_interval() {
    let source = include_str!("../coordination.rs");
    let client = source
        .split("fn apply_task_effort_through_daemon(")
        .nth(1)
        .expect("task-effort daemon client")
        .split("fn apply_task_effort_through_daemon_with(")
        .next()
        .expect("task-effort daemon client body");

    assert!(client.contains("COORDINATION_DAEMON_POLL_INTERVAL"));
    assert!(!client.contains("COORDINATION_ROSTER_DAEMON_POLL_INTERVAL"));
}

#[test]
fn app_process_background_pass_owners_are_removed() {
    let coordination = include_str!("../coordination.rs");
    let orchestration = include_str!("../../startup/orchestration.rs");
    let telemetry = include_str!("../../startup/telemetry.rs");

    assert!(!coordination.contains("run_background_self_heal_pass"));
    assert!(!coordination.contains(".apply_task_effort_for_project_with_launch_resolution("));
    assert!(!orchestration.contains("spawn_coordination_self_heal_monitor"));
    assert!(!telemetry.contains("startup.self_heal."));
}

#[test]
fn codex_hook_reconcile_failure_is_degraded_for_managed_launches() {
    // Regression: 6fe0aa3 made Codex hook filesystem errors abort initialize,
    // add, and resume before the otherwise valid coordination pipeline ran.
    let source = include_str!("../terminal_settings.rs");
    assert!(source.contains("compaction.codex_hook.degraded"));
}

#[test]
fn successful_team_commands_do_not_reconcile_the_codex_hook_twice() {
    // Regression: 6fe0aa3 reconciled Codex both before launch and again after a
    // successful pipeline, doubling writes and structured reconciliation events.
    let source = include_str!("../coordination.rs");
    let helper = source
        .split("fn maybe_ensure_compact_hooks_for_team")
        .nth(1)
        .expect("post-pipeline hook helper")
        .split("fn emit_initialize_pipeline_result")
        .next()
        .expect("post-pipeline hook helper body");
    assert!(!helper.contains("reconcile_codex_before_managed_launch"));
}

#[test]
fn daemon_owned_member_launches_do_not_reconcile_codex_in_the_app() {
    // Regression: 639b340e left app-side Codex reconciliation in add/resume
    // after their launch decisions moved into the daemon, doubling host writes.
    let source = include_str!("../coordination.rs");
    for (command, next_boundary) in [
        (
            "coordination_add_agent",
            "pub async fn coordination_resume_member",
        ),
        (
            "coordination_resume_member",
            "pub async fn coordination_resume_team",
        ),
    ] {
        let body = source
            .split(&format!("pub async fn {command}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{command} command body"))
            .split(next_boundary)
            .next()
            .expect("next command boundary");
        assert!(
            !body.contains("reconcile_codex_before_managed_launch"),
            "{command} must leave Codex hook reconciliation to the daemon"
        );
    }
}

#[test]
fn every_registered_roster_mutation_reconciles_the_global_harness_hooks() {
    // Regression: commit 86601a2 reconciled grok's one global hook from the
    // add-agent, remove-member and disband paths but left the registered
    // `coordination_add_member` mutation alone, so adding the first grok member
    // through that IPC left grok without its hook until some later trigger.
    const ROSTER_MUTATIONS: &[&str] = &[
        "coordination_initialize_team",
        "coordination_add_agent",
        "coordination_add_member",
        "coordination_remove_member",
        "coordination_disband_team",
    ];
    let source = include_str!("../coordination.rs");

    let unreconciled = source
        .split("#[tauri::command")
        .skip(1)
        .filter_map(|command| {
            let name = command
                .split_once("pub fn ")
                .or_else(|| command.split_once("pub async fn "))
                .and_then(|(_, rest)| rest.split_once('('))
                .map(|(name, _)| name)?;
            let mutates_roster = ROSTER_MUTATIONS.contains(&name);
            let reconciles = command.contains("reconcile_global_harness_hooks")
                || command.contains("maybe_ensure_compact_hooks_for_team");
            (mutates_roster && !reconciles).then_some(name)
        })
        .collect::<Vec<_>>();

    assert!(
        unreconciled.is_empty(),
        "roster mutations that never reconcile the global harness hooks: {unreconciled:?}"
    );
}

#[derive(Debug, Default)]
struct MockBinaryLookup {
    available: HashSet<String>,
}

impl MockBinaryLookup {
    fn with_available(names: &[&str]) -> Self {
        Self {
            available: names.iter().map(|name| (*name).to_string()).collect(),
        }
    }
}

impl BinaryLookup for MockBinaryLookup {
    fn is_available(&self, binary_name: &str) -> bool {
        self.available.contains(binary_name)
    }
}

fn test_state(teams_dir: PathBuf) -> CoordinationState {
    CoordinationState::with_components_and_runtime(
        teams_dir,
        BackendSelector::m0(),
        Arc::new(|_kind, _teams_dir| {
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    )
}

fn test_state_with_runtime(
    teams_dir: PathBuf,
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

#[allow(clippy::useless_conversion)]
fn test_db_state() -> (DbState, NamedTempFile) {
    let tmp = NamedTempFile::new().expect("temp db");
    let conn = taurhaus_lib::db::init_db(tmp.path()).expect("init db");
    (DbState(Mutex::new(conn).into()), tmp)
}

fn test_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("valid timestamp")
        .with_timezone(&Utc)
}

struct StubDaemon {
    addr: String,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for StubDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn start_live_status_stub_daemon(
    snapshot_response: serde_json::Value,
    state: Arc<CoordinationState>,
) -> StubDaemon {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub daemon");
    let addr = listener.local_addr().expect("stub daemon addr");
    let addr_string = format!("127.0.0.1:{}", addr.port());

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept daemon client");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut writer = stream;

        for _ in 0..2 {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read request");
            let request: protocol::DaemonRequest =
                serde_json::from_str(&line).expect("parse daemon request");
            let response = match request.method.as_str() {
                protocol::method::GET_RUNTIME_SESSION_SNAPSHOT => snapshot_response.clone(),
                protocol::method::COORDINATION_RECONCILE_LIVE_PRESENCE => {
                    let params = serde_json::from_value(request.params)
                        .expect("parse live presence reconciliation params");
                    let response = match crate::daemon::state_writes::reconcile_live_presence(
                        &state, params,
                    ) {
                        Ok(result) => protocol::DaemonResponse::ok(&request.id, result),
                        Err(error) => protocol::DaemonResponse::err(
                            &request.id,
                            "LIVE_PRESENCE_RECONCILE_FAILED",
                            error.to_string(),
                        ),
                    };
                    serde_json::to_value(response).expect("serialize live presence response")
                }
                method => serde_json::to_value(protocol::DaemonResponse::err(
                    &request.id,
                    "UNKNOWN_METHOD",
                    format!("Unknown method: {method}"),
                ))
                .expect("serialize unknown method response"),
            };
            let mut response = response;
            if let Some(map) = response.as_object_mut() {
                map.insert("id".to_string(), serde_json::Value::String(request.id));
            }
            let response_line = format!(
                "{}\n",
                serde_json::to_string(&response).expect("serialize daemon response")
            );
            writer
                .write_all(response_line.as_bytes())
                .expect("write daemon response");
            writer.flush().expect("flush daemon response");
        }
    });

    StubDaemon {
        addr: addr_string,
        handle: Some(handle),
    }
}

fn sample_preflight_request() -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Cross-project implementation team".to_string()),
        preset_id: None,
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            role_id: Some("claude-orchestrator".to_string()),
            role_name: Some("Claude Orchestrator".to_string()),
            focus_area: Some("Team sequencing and escalation".to_string()),
            context_summary: Some(
                "Keeps the full delivery plan and blocker state in view.".to_string(),
            ),
            behavior_summary: Some("Coordinates specialists and escalates blockers.".to_string()),
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: Some("Own orchestration".to_string()),
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            reasoning_effort: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            account_id: None,
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                role_id: Some("codex-architect".to_string()),
                role_name: Some("Codex Architect".to_string()),
                focus_area: Some("Architecture decisions and structural review".to_string()),
                context_summary: Some(
                    "Carries long-lived context around module boundaries and reviews.".to_string(),
                ),
                behavior_summary: Some(
                    "Handles pattern choices and escalates direction changes.".to_string(),
                ),
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "agy".to_string(),
                model: "pro".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-api".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
        ],
    }
}

fn sample_add_agent_request(team_name: &str, member_name: &str) -> AddAgentRequest {
    AddAgentRequest {
        team_name: team_name.to_string(),
        agent: AgentSetupConfig {
            name: member_name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-api".to_string(),
            description: Some("API ownership".to_string()),
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            reasoning_effort: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            account_id: None,
        },
    }
}

fn sample_resume_request(team_name: &str, member_name: &str) -> ResumeMemberRequest {
    ResumeMemberRequest {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    }
}

#[test]
fn team_summary_serialization_round_trip() {
    let value = TeamSummary {
        team_name: "architecture-final".to_string(),
        lead_project_path: Some("/tmp/taurhaus".to_string()),
    };
    let json = serde_json::to_string(&value).expect("serialize team summary");
    let decoded: TeamSummary = serde_json::from_str(&json).expect("deserialize team summary");
    assert_eq!(decoded, value);
}

#[test]
fn team_discovery_response_serialization_round_trip() {
    let value = TeamDiscoveryResponse {
        teams: vec![TeamSummary {
            team_name: "architecture-final".to_string(),
            lead_project_path: Some("/tmp/taurhaus".to_string()),
        }],
        warnings: vec!["skipped team folder 'broken-team'".to_string()],
    };
    let json = serde_json::to_string(&value).expect("serialize team discovery response");
    let decoded: TeamDiscoveryResponse =
        serde_json::from_str(&json).expect("deserialize team discovery response");
    assert_eq!(decoded, value);
}

#[test]
fn team_status_serialization_round_trip() {
    let value = TeamStatus {
        team_name: "architecture-final".to_string(),
        members: vec!["team-lead".to_string(), "codex-reviewer".to_string()],
    };
    let json = serde_json::to_string(&value).expect("serialize team status");
    let decoded: TeamStatus = serde_json::from_str(&json).expect("deserialize team status");
    assert_eq!(decoded, value);
}

#[test]
fn disband_team_response_serialization_round_trip() {
    let value = DisbandTeamResponse {
        team_name: "architecture-final".to_string(),
        disbanded: false,
        already_disbanded: true,
        message: "team already disbanded".to_string(),
    };
    let json = serde_json::to_string(&value).expect("serialize disband response");
    let decoded: DisbandTeamResponse =
        serde_json::from_str(&json).expect("deserialize disband response");
    assert_eq!(decoded, value);
}

#[test]
fn create_team_rejects_empty_or_whitespace_name() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let err = coordination_create_team_impl(&state, "".to_string()).expect_err("empty should fail");
    assert!(err.contains("team_name"));

    let err = coordination_create_team_impl(&state, "   \n\t  ".to_string())
        .expect_err("whitespace should fail");
    assert!(err.contains("team_name"));
}

#[test]
fn member_commands_validate_all_required_fields() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let err = coordination_disband_team_impl(&state, "   ".to_string())
        .expect_err("blank team should fail");
    assert!(err.contains("team_name"));

    let err = coordination_add_member_impl(
        &state,
        "team".to_string(),
        "".to_string(),
        "mesh".to_string(),
        None,
    )
    .expect_err("empty member should fail");
    assert!(err.contains("member_name"));

    let err = coordination_add_member_impl(
        &state,
        "team".to_string(),
        "alice".to_string(),
        "".to_string(),
        None,
    )
    .expect_err("empty backend should fail");
    assert!(err.contains("backend_kind"));
}

#[test]
fn get_team_status_validates_non_empty_team_name() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let err =
        coordination_get_team_status_impl(&state, " ".to_string()).expect_err("whitespace invalid");
    assert!(err.contains("team_name"));
}

#[test]
fn preflight_all_tools_present_returns_clean_report() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "codex", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(report.can_initialize);
    assert!(report.blocking_errors.is_empty());
    assert!(report.agent_warnings.is_empty());
}

#[test]
fn preflight_mesh_missing_returns_blocking_error() {
    let lookup = MockBinaryLookup::with_available(&["tmux", "claude", "codex", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert!(report.blocking_errors.contains(
        &"Mesh CLI not found. Install it to enable multi-agent collaboration.".to_string()
    ));
}

#[test]
fn preflight_tmux_missing_returns_blocking_error() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "claude", "codex", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert!(report
        .blocking_errors
        .contains(&"tmux is required for multi-agent sessions.".to_string()));
}

#[test]
fn preflight_agent_tool_missing_returns_warning() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux", "claude", "agy"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(report.can_initialize);
    assert!(report.blocking_errors.is_empty());
    assert_eq!(report.agent_warnings.len(), 1);
    assert_eq!(report.agent_warnings[0].agent_name, "frontend-dev");
    assert_eq!(report.agent_warnings[0].cli_tool, "codex");
    assert!(report.agent_warnings[0]
        .message
        .contains("Codex CLI not found"));
}

#[test]
fn preflight_multiple_issues_reports_all() {
    let lookup = MockBinaryLookup::with_available(&["codex"]);
    let report = coordination_preflight_check_with_lookup(sample_preflight_request(), &lookup)
        .expect("preflight should succeed");
    assert!(!report.can_initialize);
    assert_eq!(report.blocking_errors.len(), 2);
    assert_eq!(report.agent_warnings.len(), 2);
    assert!(report
        .agent_warnings
        .iter()
        .any(|w| w.agent_name == "team-lead" && w.message.contains("Claude CLI not found")));
    assert!(report
        .agent_warnings
        .iter()
        .any(|w| w.agent_name == "reviewer" && w.message.contains("Antigravity CLI not found")));
}

#[test]
fn feature_availability_reports_missing_mesh_and_tmux() {
    let lookup = MockBinaryLookup::with_available(&["claude"]);
    let report = coordination_get_feature_availability_with_lookup(&lookup);
    assert!(!report.can_initialize);
    assert!(!report.mesh_available);
    assert!(!report.tmux_available);
    assert_eq!(report.blocking_errors.len(), 2);
}

#[test]
fn feature_availability_reports_ready_when_required_tools_exist() {
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);
    let report = coordination_get_feature_availability_with_lookup(&lookup);
    assert!(report.can_initialize);
    assert!(report.mesh_available);
    assert!(report.tmux_available);
    assert!(report.blocking_errors.is_empty());
}

#[test]
fn initialize_test_fixture_uses_the_daemon_pipeline_host() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let (db, _db_file) = test_db_state();
    let request = sample_preflight_request();

    let report = initialize_team_pipeline_test_fixture(
        &state,
        Some(&db),
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should return a report");

    assert_eq!(report.team_name, "architecture-final");
    assert!(report.failed_step.is_none());
    assert!(!report.steps.is_empty());
    assert_eq!(report.steps[0].step, "validate_configuration");
}

#[test]
fn initialize_progress_events_are_emitted_in_step_order() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let request = sample_preflight_request();

    let mut emitted = Vec::new();
    let mut emit = |event: &StepProgressEvent| {
        emitted.push(event.clone());
    };
    let report = initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("initialize should return a report");

    assert_eq!(emitted.len(), report.steps.len() * 2);
    for (idx, step) in report.steps.iter().enumerate() {
        let running = &emitted[idx * 2];
        let completed = &emitted[idx * 2 + 1];
        assert_eq!(running.operation, "initialize_team");
        assert_eq!(running.progress.step, step.step);
        assert_eq!(running.progress.status, StepStatus::Running);
        assert_eq!(completed.progress.step, step.step);
        assert_eq!(completed.progress.status, step.status);
    }
}

#[test]
fn initialize_error_case_returns_structured_failed_step_report() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let mut request = sample_preflight_request();
    request.agents[0].name = request.lead.name.clone(); // duplicate name -> validation step failure

    let report = initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should return structured failure report");
    assert_eq!(
        report.failed_step.as_deref(),
        Some("validate_configuration")
    );
    assert!(report.retryable);
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].status, StepStatus::Failed);
}

#[test]
fn initialize_failure_after_team_creation_does_not_get_rewritten_to_config_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = FakeBackend::default();
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding delivery failure".to_string(),
    ));
    let state = CoordinationState::with_components_and_runtime(
        tmp.path().to_path_buf(),
        BackendSelector::m0(),
        Arc::new({
            let fake = fake.clone();
            move |_kind, _teams_dir| Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    );
    let (db, _db_file) = test_db_state();

    // Regression: when initialize failed after create_team, the command layer
    // still ran post-initialize config sync and overwrote the real failed step
    // with a raw "team config not found" error after cleanup deleted the team.
    let report = initialize_team_pipeline_test_fixture(
        &state,
        Some(&db),
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should return structured failure report");

    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(report.retryable);
    assert!(
        report
            .message
            .contains("simulated onboarding delivery failure"),
        "expected original onboarding failure, got: {}",
        report.message
    );
}

#[test]
fn add_agent_ipc_returns_add_agent_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let report = add_agent_pipeline_test_fixture(
        &state,
        None,
        sample_add_agent_request("arch", "bob"),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("add-agent should return report");

    assert_eq!(report.team_name, "arch");
    assert_eq!(report.member_name, "bob");
    assert!(report.failed_step.is_none());
    assert!(report
        .succeeded_steps
        .contains(&"update_roster".to_string()));
    assert!(!report.steps.is_empty());
}

#[test]
fn add_agent_progress_events_are_emitted_in_step_order() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let mut emitted = Vec::new();
    let mut emit = |event: &StepProgressEvent| emitted.push(event.clone());
    let report = add_agent_pipeline_test_fixture(
        &state,
        None,
        sample_add_agent_request("arch", "bob"),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("add-agent should return report");

    assert_eq!(emitted.len(), report.steps.len() * 2);
    for (idx, step) in report.steps.iter().enumerate() {
        let running = &emitted[idx * 2];
        let completed = &emitted[idx * 2 + 1];
        assert_eq!(running.operation, "add_agent");
        assert_eq!(running.progress.step, step.step);
        assert_eq!(running.progress.status, StepStatus::Running);
        assert_eq!(completed.progress.step, step.step);
        assert_eq!(completed.progress.status, step.status);
    }
}

#[test]
fn reonboard_succeeds_for_existing_member() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = FakeBackend::default();
    let state = CoordinationState::with_components_and_runtime(
        tmp.path().to_path_buf(),
        BackendSelector::m0(),
        Arc::new({
            let fake = fake.clone();
            move |_kind, _teams_dir| Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    );
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let result = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "architecture-final".to_string(),
            member_name: "team-lead".to_string(),
        },
    )
    .expect("reonboard should succeed");

    assert!(result.delivered);
    let requests = fake.delivered_requests();
    let DeliveryRequest::OperatorNotice(delivery) = requests.last().expect("reonboard delivery")
    else {
        panic!("expected operator notice")
    };
    // Regression: commit efcd7d2 silently replaced Claude re-onboarding with
    // the lifecycle-only role-context block, dropping the explicit mesh loop.
    assert!(delivery.message.starts_with("[taurhaus] onboarding"));
    assert!(delivery.message.contains("mesh read --unread --mark-read"));
}

#[test]
fn reonboard_fails_for_nonexistent_team_or_member() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let missing_team = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "missing".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("missing team should fail");
    assert!(missing_team.contains("Not found"));

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let missing_member = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "arch".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("missing member should fail");
    assert!(missing_member.contains("Not found"));
}

#[test]
fn add_agent_and_reonboard_validate_empty_strings() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let add_agent_err = add_agent_pipeline_test_fixture(
        &state,
        None,
        AddAgentRequest {
            team_name: " ".to_string(),
            agent: AgentSetupConfig {
                name: "".to_string(),
                cli_tool: "".to_string(),
                model: "gpt-5.4".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty add-agent fields should fail");
    assert!(add_agent_err.contains("team_name"));

    let reonboard_team_err = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "".to_string(),
            member_name: "bob".to_string(),
        },
    )
    .expect_err("empty team_name should fail");
    assert!(reonboard_team_err.contains("team_name"));

    let reonboard_member_err = coordination_reonboard_impl(
        None,
        &state,
        ReonboardRequest {
            team_name: "arch".to_string(),
            member_name: "  ".to_string(),
        },
    )
    .expect_err("empty member_name should fail");
    assert!(reonboard_member_err.contains("member_name"));
}

#[test]
fn resume_member_validates_empty_fields() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err = resume_member_pipeline_test_fixture(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "".to_string(),
            member_name: "agent".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty team_name should fail");
    assert!(err.contains("team_name"));

    let err = resume_member_pipeline_test_fixture(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "arch".to_string(),
            member_name: " ".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("blank member_name should fail");
    assert!(err.contains("member_name"));
}

#[test]
fn resume_member_ipc_returns_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
    )
    .expect("member runtime");
    runtime.health = crate::coordination::domain::HealthState::SessionDead;
    crate::coordination::stores::MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
        &runtime,
    )
    .expect("save runtime");

    let report = resume_member_pipeline_test_fixture(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("resume should return a report");

    assert_eq!(report.team_name, "architecture-final");
    assert_eq!(report.member_name, "frontend-dev");
    assert!(report.resumed);
    assert_eq!(report.failed_step, None);
    assert!(!report.steps.is_empty());
}

#[test]
fn resume_member_progress_events_are_emitted_from_canonical_member_stages() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
    )
    .expect("member runtime");
    runtime.health = crate::coordination::domain::HealthState::SessionDead;
    runtime.pane_id = None;
    runtime.daemon_pid = None;
    crate::coordination::stores::MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
        &runtime,
    )
    .expect("save runtime");

    let mut emitted = Vec::new();
    let mut emit = |event: &StepProgressEvent| emitted.push(event.clone());
    let report = resume_member_pipeline_test_fixture(
        &state,
        None,
        ResumeMemberRequest {
            team_name: "architecture-final".to_string(),
            member_name: "frontend-dev".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("resume should return a report");

    assert!(report.resumed);
    assert!(!emitted.is_empty());
    assert!(emitted
        .iter()
        .all(|event| event.operation == "resume_member"));
    assert!(emitted.iter().any(|event| {
        event.progress.step == "prepare_member"
            && event.progress.status == StepStatus::Running
            && event.canonical_stages == vec![MemberActivationStage::PrepareMember]
    }));
    assert!(emitted.iter().any(|event| {
        event.progress.step == "commit_runtime"
            && event.progress.status == StepStatus::Succeeded
            && event.canonical_stages == vec![MemberActivationStage::CommitRuntime]
    }));
}

#[test]
fn resume_team_validates_empty_team_name() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err = coordination_resume_team_internal(
        &state,
        ResumeTeamRequest {
            team_name: "".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect_err("empty team_name should fail");
    assert!(err.contains("team_name"));
}

#[test]
fn resume_team_ipc_returns_report_shape() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
            tmp.path(),
            "architecture-final",
            member_name,
        )
        .expect("member runtime");
        runtime.health = crate::coordination::domain::HealthState::SessionDead;
        runtime.pane_id = None;
        runtime.daemon_pid = None;
        crate::coordination::stores::MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            member_name,
            &runtime,
        )
        .expect("save runtime");
    }

    let report = coordination_resume_team_internal(
        &state,
        ResumeTeamRequest {
            team_name: "architecture-final".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("resume should return a report");

    assert_eq!(report.team_name, "architecture-final");
    assert!(report.resumed);
    assert_eq!(report.total_members, 3);
    assert_eq!(report.resumed_members.len(), 3);
    assert!(report.failed_members.is_empty());
}

#[test]
fn resume_team_progress_events_are_emitted_per_member_stage() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let mut runtime = crate::coordination::stores::MemberRuntimeStore::load(
            tmp.path(),
            "architecture-final",
            member_name,
        )
        .expect("member runtime");
        runtime.health = crate::coordination::domain::HealthState::SessionDead;
        runtime.pane_id = None;
        runtime.daemon_pid = None;
        crate::coordination::stores::MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            member_name,
            &runtime,
        )
        .expect("save runtime");
    }

    let mut emitted = Vec::new();
    let mut emit = |event: &ResumeTeamProgressEvent| emitted.push(event.clone());
    let report = coordination_resume_team_internal(
        &state,
        ResumeTeamRequest {
            team_name: "architecture-final".to_string(),
        },
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        Some(&mut emit),
    )
    .expect("resume should return a report");

    assert!(report.resumed);
    assert!(!emitted.is_empty());
    assert!(emitted.iter().all(|event| event.operation == "resume_team"));
    assert!(emitted.iter().all(|event| event.member_count == 3));
    assert_eq!(
        emitted
            .iter()
            .filter(|event| {
                event.status == StepStatus::Running
                    && event.stage == MemberActivationStage::PrepareMember
            })
            .map(|event| (event.member_name.clone(), event.member_index))
            .collect::<Vec<_>>(),
        vec![
            ("team-lead".to_string(), 1),
            ("frontend-dev".to_string(), 2),
            ("reviewer".to_string(), 3),
        ]
    );
    assert!(emitted.iter().any(|event| {
        event.member_name == "reviewer"
            && event.stage == MemberActivationStage::CommitRuntime
            && event.status == StepStatus::Succeeded
    }));
}

#[test]
fn initialize_success_surfaces_terminal_with_taurhaus_tmux_session() {
    let (db, _db_file) = test_db_state();
    let expected_emulator =
        crate::commands::terminal_settings::load_terminal_settings(&db).emulator;
    let _ = crate::terminal::take_recorded_terminal_intents();

    // Regression: before this fix, successful team initialization returned its
    // report without surfacing the terminal; no single breaking commit was
    // identified, but the command layer omitted the shared EnsureOpen path.
    maybe_surface_terminal_after_initialize(
        &db,
        Some("Ubuntu".to_string()),
        &InitializeReport {
            team_name: "architecture-final".to_string(),
            succeeded_steps: vec!["launch_sessions".to_string()],
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps: Vec::new(),
        },
    );

    assert_eq!(
        crate::terminal::take_recorded_terminal_intents(),
        vec![crate::terminal::TerminalIntent::EnsureOpen {
            distro: Some("Ubuntu".to_string()),
            tmux_session: "taurhaus".to_string(),
            emulator: expected_emulator,
            custom_command: String::new(),
        }]
    );
}

#[test]
fn initialize_failure_does_not_surface_terminal() {
    let (db, _db_file) = test_db_state();
    let _ = crate::terminal::take_recorded_terminal_intents();

    maybe_surface_terminal_after_initialize(
        &db,
        Some("Ubuntu".to_string()),
        &InitializeReport {
            team_name: "architecture-final".to_string(),
            succeeded_steps: vec!["create_team".to_string()],
            failed_step: Some("launch_sessions".to_string()),
            retryable: true,
            message: "launch failed".to_string(),
            steps: Vec::new(),
        },
    );

    assert!(crate::terminal::take_recorded_terminal_intents().is_empty());
}

#[test]
fn resume_team_surfaces_terminal_only_when_members_resumed() {
    let (db, _db_file) = test_db_state();
    let expected_emulator =
        crate::commands::terminal_settings::load_terminal_settings(&db).emulator;
    let _ = crate::terminal::take_recorded_terminal_intents();

    // Regression: before this fix, successful team resume rebuilt panes and
    // sessions but left the terminal hidden; no single breaking commit was
    // identified, but the command layer never called the shared terminal path.
    maybe_surface_terminal_after_resume_team(
        &db,
        Some("Ubuntu".to_string()),
        &ResumeTeamReport {
            team_name: "architecture-final".to_string(),
            resumed: true,
            total_members: 3,
            resumed_members: vec![
                "team-lead".to_string(),
                "frontend-dev".to_string(),
                "reviewer".to_string(),
            ],
            failed_members: Vec::new(),
            warnings: Vec::new(),
            started_team_daemon: true,
            team_daemon_warning: None,
        },
    );

    assert_eq!(
        crate::terminal::take_recorded_terminal_intents(),
        vec![crate::terminal::TerminalIntent::EnsureOpen {
            distro: Some("Ubuntu".to_string()),
            tmux_session: "taurhaus".to_string(),
            emulator: expected_emulator,
            custom_command: String::new(),
        }]
    );

    maybe_surface_terminal_after_resume_team(
        &db,
        Some("Ubuntu".to_string()),
        &ResumeTeamReport {
            team_name: "architecture-final".to_string(),
            resumed: false,
            total_members: 3,
            resumed_members: Vec::new(),
            failed_members: vec![ResumeTeamMemberFailure {
                member_name: "team-lead".to_string(),
                message: "resume failed".to_string(),
                retryable: true,
            }],
            warnings: Vec::new(),
            started_team_daemon: false,
            team_daemon_warning: Some("not started".to_string()),
        },
    );

    assert!(crate::terminal::take_recorded_terminal_intents().is_empty());
}

#[test]
fn create_team_happy_path_persists_team() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create should succeed");
    let discovery = coordination_list_teams_impl(&state).expect("list should succeed");
    assert_eq!(
        discovery.teams,
        vec![TeamSummary {
            team_name: "arch".to_string(),
            lead_project_path: None,
        }]
    );
    assert!(discovery.warnings.is_empty());
}

#[test]
fn create_team_error_mapping_conflict() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("first create");
    let err = coordination_create_team_impl(&state, "arch".to_string())
        .expect_err("duplicate should fail");
    assert!(err.contains("Conflict"));
}

#[test]
fn disband_team_happy_path_removes_team() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let result = coordination_disband_team_impl(&state, "arch".to_string()).expect("disband");
    assert!(result.disbanded);
    assert!(!result.already_disbanded);
    assert_eq!(result.message, "team disbanded");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert!(discovery.teams.is_empty());
    assert!(discovery.warnings.is_empty());
}

#[test]
fn disband_team_is_idempotent_and_reports_already_disbanded() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let result = coordination_disband_team_impl(&state, "missing".to_string())
        .expect("idempotent disband should succeed");
    assert!(!result.disbanded);
    assert!(result.already_disbanded);
    assert_eq!(result.message, "team already disbanded");
}

#[test]
fn add_member_happy_path_persists_member() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add member");
    let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
    assert_eq!(status.members, vec!["alice".to_string()]);
}

#[test]
fn the_first_grok_member_added_through_this_mutation_earns_the_hook() {
    // Regression: commit 86601a2 left `coordination_add_member` outside hook
    // reconciliation, so the grok member it persists was invisible to the hook
    // installer until an unrelated roster change ran.
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));
    let grok_home = tmp.path().join("grok-home");
    let exe = tmp.path().join("taurhaus");
    std::fs::write(&exe, b"fixture").expect("executable fixture");

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "grok".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add grok member");

    // What the reconciliation this command now runs sees on that roster.
    assert!(
        crate::coordination::compact_hook::any_managed_grok_member(state.teams_dir())
            .expect("managed grok discovery"),
        "the member this mutation persists is a managed grok member"
    );
    crate::coordination::compact_hook::ensure_grok_compact_hook_installed_at(&grok_home, &exe)
        .expect("install grok hook");
    assert!(
        crate::coordination::compact_hook::grok_compact_hook_is_installed_at(&grok_home),
        "the first grok member on the roster earns grok's one global hook"
    );
}

#[test]
fn add_member_error_mapping_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err = coordination_add_member_impl(
        &state,
        "missing".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        None,
    )
    .expect_err("missing team");
    assert!(err.contains("Not found"));
}

#[test]
fn add_member_requires_project_path_when_team_has_no_members() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    let err = coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        None,
    )
    .expect_err("missing project path should fail for empty team");
    assert!(err.contains("project_path must be provided"));
}

#[test]
fn add_member_defaults_to_lead_project_path_instead_of_process_cwd() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let request = sample_preflight_request();

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    coordination_add_member_impl(
        &state,
        "architecture-final".to_string(),
        "legacy-dev".to_string(),
        "codex".to_string(),
        None,
    )
    .expect("add member");

    let member_project_path = state
        .with_orchestrator(|orchestrator| {
            let status = orchestrator.get_team_status("architecture-final")?;
            let member = status
                .config
                .members
                .iter()
                .find(|member| member.name == "legacy-dev")
                .ok_or_else(|| {
                    CoordinationError::NotFound("member 'legacy-dev' not found".to_string())
                })?;
            Ok(member.project_path.clone())
        })
        .expect("member should be persisted");

    assert_eq!(member_project_path, PathBuf::from("proj-core"));
}

#[test]
fn derive_cross_project_status_returns_false_for_same_normalized_path() {
    let status = derive_cross_project_status(
        Path::new("/home/user/projects/taurhaus"),
        Path::new("/home/user/projects/taurhaus/"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn derive_cross_project_status_returns_true_for_different_project_path() {
    let status = derive_cross_project_status(
        Path::new("/home/user/projects/taurhaus"),
        Path::new("/home/user/projects/mesh"),
    );

    assert!(status.is_cross_project);
    assert_eq!(status.project_label, "mesh");
}

#[test]
fn derive_cross_project_status_matches_windows_and_linux_forms() {
    let status = derive_cross_project_status(
        Path::new("/mnt/c/Users/me/code/taurhaus"),
        Path::new(r"C:\Users\me\code\taurhaus"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn derive_cross_project_status_matches_case_variant_windows_paths() {
    let status = derive_cross_project_status(
        Path::new(r"C:\Users\Me\Code\Taurhaus"),
        Path::new(r"c:\users\me\code\taurhaus"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn derive_cross_project_status_matches_wsl_unc_and_linux_forms() {
    let status = derive_cross_project_status(
        Path::new("/home/user/projects/mesh"),
        Path::new(r"\\wsl.localhost\Ubuntu\home\user\projects\mesh"),
    );

    assert!(!status.is_cross_project);
    assert_eq!(status.project_label, "");
}

#[test]
fn list_teams_happy_path_returns_sorted_summaries() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "zeta".to_string()).expect("create zeta");
    coordination_create_team_impl(&state, "alpha".to_string()).expect("create alpha");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert_eq!(
        discovery.teams,
        vec![
            TeamSummary {
                team_name: "alpha".to_string(),
                lead_project_path: None,
            },
            TeamSummary {
                team_name: "zeta".to_string(),
                lead_project_path: None,
            }
        ]
    );
    assert!(discovery.warnings.is_empty());
}

#[test]
fn list_teams_error_mapping_io() {
    let tmp = TempDir::new().expect("tempdir");
    let file_path = tmp.path().join("teams-file");
    std::fs::write(&file_path, "not a directory").expect("write marker");
    let state = test_state(file_path);

    let err = coordination_list_teams_impl(&state).expect_err("list should fail");
    assert!(err.contains("IO error"));
}

#[test]
fn list_teams_includes_lead_project_anchor() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let request = sample_preflight_request();
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, "architecture-final");
    assert_eq!(
        discovery.teams[0].lead_project_path.as_deref(),
        Some("proj-core")
    );
}

#[test]
fn list_teams_skips_corrupt_folders_with_warning() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "good".to_string()).expect("create");
    let broken_dir = tmp.path().join("broken-team");
    std::fs::create_dir_all(&broken_dir).expect("create broken dir");
    std::fs::write(broken_dir.join("config.json"), "{ invalid").expect("write broken");

    let discovery = coordination_list_teams_impl(&state).expect("list");
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, "good");
    assert_eq!(discovery.warnings.len(), 1);
    assert!(discovery.warnings[0].contains("broken-team"));
}

#[test]
fn get_team_status_happy_path_returns_team_and_members() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    coordination_create_team_impl(&state, "arch".to_string()).expect("create");
    coordination_add_member_impl(
        &state,
        "arch".to_string(),
        "alice".to_string(),
        "mesh".to_string(),
        Some("/tmp/arch".to_string()),
    )
    .expect("add");
    let status = coordination_get_team_status_impl(&state, "arch".to_string()).expect("status");
    assert_eq!(status.team_name, "arch");
    assert_eq!(status.members, vec!["alice".to_string()]);
}

#[test]
fn get_team_status_error_mapping_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());

    let err =
        coordination_get_team_status_impl(&state, "missing".to_string()).expect_err("missing team");
    assert!(err.contains("Not found"));
}

#[test]
fn project_mesh_snapshot_returns_null_team_when_project_has_no_match() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        "/projects/missing".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert!(snapshot.mesh_available);
    assert!(snapshot.tmux_available);
    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::None);
    assert_eq!(snapshot.team_name, None);
    assert_eq!(snapshot.team_status, None);
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn project_mesh_snapshot_classifies_active_when_all_members_are_live() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::Active);
}

#[test]
fn project_mesh_snapshot_classifies_degraded_when_live_and_offline_members_mix() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let offline_pane = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("load runtime")
        .pane_id
        .expect("pane id");
    runtime.set_pane_exists(&offline_pane, false);
    let mut offline_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
            .expect("reload runtime");
    offline_runtime.health = HealthState::SessionDead;
    offline_runtime.session_id = None;
    offline_runtime.daemon_pid = None;
    MemberRuntimeStore::save(
        tmp.path(),
        "architecture-final",
        "frontend-dev",
        &offline_runtime,
    )
    .expect("persist offline runtime");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::Degraded);
    let team_status = snapshot.team_status.expect("team status");
    let frontend_dev = team_status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev");
    assert_eq!(frontend_dev.session_status, SessionStatus::Offline);
}

#[test]
fn project_mesh_snapshot_classifies_cold_resume_when_all_members_are_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    for member_name in ["team-lead", "frontend-dev", "reviewer"] {
        let mut runtime_record =
            MemberRuntimeStore::load(tmp.path(), "architecture-final", member_name)
                .expect("load runtime");
        let pane_id = runtime_record.pane_id.clone().expect("pane id");
        runtime.set_pane_exists(&pane_id, false);
        runtime_record.health = HealthState::SessionDead;
        runtime_record.session_id = None;
        runtime_record.daemon_pid = None;
        MemberRuntimeStore::save(
            tmp.path(),
            "architecture-final",
            member_name,
            &runtime_record,
        )
        .expect("persist offline runtime");
    }

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::ColdResume);
    let team_status = snapshot.team_status.expect("team status");
    assert!(team_status
        .members
        .iter()
        .all(|member| member.session_status == SessionStatus::Offline));
}

#[test]
fn project_mesh_snapshot_uses_fast_snapshot_without_runtime_reconcile_calls() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("load runtime");
    let pane_id = record.pane_id.clone().expect("pane id");
    record.health = HealthState::SessionDead;
    record.session_id = None;
    record.daemon_pid = None;
    record.last_seen_at = None;
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    runtime.set_pane_exists(&pane_id, true);
    runtime.set_pane_dead(&pane_id, false);
    runtime.set_pane_shell(&pane_id, false);
    runtime.set_matching_daemon_pids(&pane_id, "architecture-final", "frontend-dev", &[5555]);
    let call_count_before = runtime.calls().len();

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    let frontend_dev = snapshot
        .team_status
        .expect("team status")
        .members
        .into_iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev");
    assert_eq!(frontend_dev.session_status, SessionStatus::Offline);
    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("reload runtime");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.daemon_pid, None);
    let calls = runtime.calls();
    let snapshot_calls = &calls[call_count_before..];
    assert!(
        !snapshot_calls.iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPaneExists { .. }
                | RuntimeCall::CheckPaneDead { .. }
                | RuntimeCall::CheckPaneShell { .. }
                | RuntimeCall::FindDaemon { .. }
                | RuntimeCall::SpawnDaemon { .. }
        )),
        "snapshot path should not execute runtime liveness probes"
    );
}

#[test]
fn project_mesh_snapshot_returns_fast_team_snapshot_for_matching_project() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("load runtime");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::Active);
    assert_eq!(snapshot.team_name.as_deref(), Some("architecture-final"));
    assert!(snapshot.warnings.is_empty());

    let team_status = snapshot.team_status.expect("team status should be present");
    assert_eq!(team_status.lead_name, "team-lead");
    assert_eq!(team_status.members.len(), 3);

    let frontend_dev = team_status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev should be present");
    assert_eq!(frontend_dev.role, AgentRole::Member);
    assert_eq!(frontend_dev.cli_tool, "codex");
    assert_eq!(frontend_dev.project_id, "proj-web");
    assert!(frontend_dev.is_cross_project);
    assert_eq!(frontend_dev.project_label, "proj-web");
    assert_eq!(frontend_dev.session_status, SessionStatus::Active);
    assert_eq!(frontend_dev.pane_id.as_deref(), Some("%9"));
    assert_eq!(frontend_dev.role_id.as_deref(), Some("codex-architect"));
    assert_eq!(frontend_dev.role_name.as_deref(), Some("Codex Architect"));
    assert_eq!(
        frontend_dev.focus_area.as_deref(),
        Some("Architecture decisions and structural review")
    );
    assert_eq!(
        frontend_dev.context_summary.as_deref(),
        Some("Carries long-lived context around module boundaries and reviews.")
    );
    assert_eq!(
        frontend_dev.behavior_summary.as_deref(),
        Some("Handles pattern choices and escalates direction changes.")
    );
}

#[test]
fn project_mesh_snapshot_prefers_persisted_active_team_when_multiple_teams_match_project() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let mut old_request = sample_preflight_request();
    old_request.team_name = "towerhouse-product-team".to_string();
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        old_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize old team");

    let mut active_request = sample_preflight_request();
    active_request.team_name = "taurhaus-team".to_string();
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        active_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize active team");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("taurhaus-team"));
    assert!(snapshot.team_status.is_some());
}

#[test]
fn project_mesh_snapshot_recovers_missing_active_team_mapping_from_runtime_signal() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let mut old_request = sample_preflight_request();
    old_request.team_name = "towerhouse-product-team".to_string();
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        old_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize old team");

    let mut active_request = sample_preflight_request();
    active_request.team_name = "taurhaus-team".to_string();
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        active_request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize active team");

    ActiveProjectTeamStore::clear_project(tmp.path(), "proj-web").expect("clear active mapping");
    assert_eq!(
        ActiveProjectTeamStore::load_active_team(tmp.path(), "proj-web").expect("load active team"),
        None
    );

    let stale_seen_at = test_timestamp("2026-03-01T10:00:00Z");
    let live_seen_at = test_timestamp("2026-03-11T05:30:00Z");
    for (index, member_name) in ["team-lead", "frontend-dev", "reviewer"]
        .into_iter()
        .enumerate()
    {
        let mut stale_runtime =
            MemberRuntimeStore::load(tmp.path(), "towerhouse-product-team", member_name)
                .expect("load stale runtime");
        stale_runtime.health = HealthState::SessionDead;
        stale_runtime.session_id = None;
        stale_runtime.daemon_pid = None;
        stale_runtime.last_seen_at = Some(stale_seen_at);
        stale_runtime.attached_at = Some(stale_seen_at);
        MemberRuntimeStore::save(
            tmp.path(),
            "towerhouse-product-team",
            member_name,
            &stale_runtime,
        )
        .expect("save stale runtime");

        let mut live_runtime = MemberRuntimeStore::load(tmp.path(), "taurhaus-team", member_name)
            .expect("load live runtime");
        live_runtime.health = HealthState::Healthy;
        live_runtime.session_id = Some(format!("live-session-{member_name}"));
        live_runtime.daemon_pid = Some(9_000 + index as u32);
        live_runtime.last_seen_at = Some(live_seen_at);
        live_runtime.attached_at = Some(live_seen_at);
        if live_runtime.pane_id.is_none() {
            live_runtime.pane_id = Some(format!("%{}", index + 10));
        }
        MemberRuntimeStore::save(tmp.path(), "taurhaus-team", member_name, &live_runtime)
            .expect("save live runtime");
    }

    // Regression: commit 439d04b preferred persisted active-team mappings but still fell back to
    // first-match folder order when the mapping file was missing, so a stale older team could win.
    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("taurhaus-team"));
    assert!(snapshot.team_status.is_some());
    assert_eq!(
        ActiveProjectTeamStore::load_active_team(tmp.path(), "proj-web")
            .expect("load repaired active team")
            .as_deref(),
        Some("taurhaus-team")
    );
}

#[test]
fn project_mesh_snapshot_resolves_role_metadata_when_initialize_request_only_has_role_ids() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let request = InitializeTeamRequest {
        team_name: "review-team".to_string(),
        team_description: Some("Review-focused team".to_string()),
        preset_id: None,
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "fable".to_string(),
            role_id: Some("v3-lead-claude".to_string()),
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: None,
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            reasoning_effort: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            account_id: None,
        },
        agents: vec![
            AgentSetupConfig {
                name: "reviewer-1".to_string(),
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                role_id: Some("adversarial-reviewer-claude".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
            AgentSetupConfig {
                name: "reviewer-2".to_string(),
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                role_id: Some("adversarial-reviewer-claude".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
        ],
    };

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");
    let team_status = snapshot.team_status.expect("team status");

    let reviewer = team_status
        .members
        .iter()
        .find(|member| member.name == "reviewer-1")
        .expect("reviewer-1 should be present");

    assert_eq!(
        reviewer.role_id.as_deref(),
        Some("adversarial-reviewer-claude")
    );
    assert_eq!(
        reviewer.role_name.as_deref(),
        Some("Adversarial Reviewer (Claude)")
    );
    assert_eq!(
        reviewer.focus_area.as_deref(),
        Some("Adversarial correctness review with evidence-backed findings")
    );
    assert!(reviewer
        .context_summary
        .as_deref()
        .unwrap_or_default()
        .contains("defect hot spots"));
    assert!(reviewer
        .behavior_summary
        .as_deref()
        .unwrap_or_default()
        .contains("Assumes defects exist"));
    assert!(reviewer
        .description
        .as_deref()
        .unwrap_or_default()
        .contains("Default: Opus 5"));
}

// Regression: a79d392 hydrated role models without checking the requested CLI,
// so request normalization could pair Claude with a Codex-only model slug.
#[test]
fn role_hydration_cli_mismatch_uses_the_requested_tools_catalog_default() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));
    let mut request = sample_preflight_request();
    request.agents[0].cli_tool = "claude".to_string();
    request.agents[0].model.clear();
    request.agents[0].role_id = Some("v4-developer-codex".to_string());
    request.agents[0].role_name = None;

    let hydrated = hydrate_initialize_request_role_metadata(&state, request)
        .expect("role hydration should succeed");

    assert_eq!(hydrated.agents[0].model, "opus");
}

// Regression: b345de1 (PR 5c) gave presets a lead pin for model and effort, but
// preset-driven request hydration composed with `CompositionOverrides::default()`,
// so a minimal initialize payload launched the lead on the lead role's defaults and
// silently dropped the pin the preset stores.
#[test]
fn initialize_request_hydration_applies_the_preset_lead_pin() {
    let tmp = TempDir::new().expect("tempdir");
    let presets_dir = tmp.path().join("templates").join("presets");
    std::fs::create_dir_all(&presets_dir).expect("presets dir");
    std::fs::write(
        presets_dir.join("lead-pinned.yaml"),
        concat!(
            "schema:\n",
            "  kind: team_preset\n",
            "  version: 1\n",
            "preset_id: lead-pinned\n",
            "name: Lead Pinned\n",
            "description: Lead pinned to a model and effort\n",
            "version: \"1.0.0\"\n",
            "lead_role_id: v3-lead-claude\n",
            "lead_overrides:\n",
            "  model: claude-sonnet-4-5\n",
            "  reasoning_effort: xhigh\n",
            "agent_slots:\n",
            "  - role_id: quick-dev-codex\n",
            "    count: 1\n",
            "    project_binding: lead_project\n",
            "defaults:\n",
            "  team_name_pattern: \"{project}-team\"\n",
            "  tmux_layout: tiled\n",
        ),
    )
    .expect("write preset");

    let state = test_state(tmp.path().join("teams"));
    let mut request = sample_preflight_request();
    request.preset_id = Some("lead-pinned".to_string());
    request.lead.cli_tool.clear();
    request.lead.model.clear();
    request.lead.reasoning_effort = None;
    request.lead.role_id = None;
    request.lead.role_name = None;
    request.agents.truncate(1);
    request.agents[0].cli_tool.clear();
    request.agents[0].model.clear();
    request.agents[0].role_id = None;
    request.agents[0].role_name = None;

    let hydrated = hydrate_initialize_request_role_metadata(&state, request)
        .expect("preset hydration should succeed");

    assert_eq!(hydrated.lead.model, "claude-sonnet-4-5");
    assert_eq!(hydrated.lead.reasoning_effort.as_deref(), Some("xhigh"));
}

#[test]
fn initialize_request_hydrates_from_preset_when_frontend_sends_minimal_payload() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().join("teams"));

    let request = InitializeTeamRequest {
        team_name: "dev-team".to_string(),
        team_description: Some("Dev-focused team".to_string()),
        preset_id: Some("dev-team".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: String::new(),
            model: String::new(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: None,
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            reasoning_effort: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            account_id: None,
        },
        agents: vec![
            AgentSetupConfig {
                name: "dev-1".to_string(),
                cli_tool: String::new(),
                model: String::new(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-core".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
            AgentSetupConfig {
                name: "dev-2".to_string(),
                cli_tool: String::new(),
                model: String::new(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-core".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
        ],
    };

    let report = initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    assert_eq!(report.team_name, "dev-team");

    let stored = TeamConfigStore::load(state.teams_dir(), "dev-team").expect("load team config");
    let lead = stored
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .expect("lead");
    let developer = stored
        .members
        .iter()
        .find(|member| member.name == "dev-1")
        .expect("developer");

    assert_eq!(lead.role_id.as_deref(), Some("v3-lead-claude"));
    assert_eq!(lead.role_name.as_deref(), Some("Team Lead (Claude)"));
    assert_eq!(lead.cli_tool, CliTool::Claude);
    assert_eq!(developer.role_id.as_deref(), Some("v4-developer-codex"));
    assert_eq!(developer.role_name.as_deref(), Some("Developer (Codex)"));
    assert_eq!(developer.cli_tool, CliTool::Codex);
    assert_eq!(developer.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(developer.reasoning_effort.as_deref(), Some("medium"));
    assert!(developer
        .handoff_expectations
        .as_ref()
        .is_some_and(|items| !items.is_empty()));
    assert!(developer
        .instructions
        .as_deref()
        .unwrap_or("")
        .contains("one user-visible behavior"));
}

#[test]
fn project_mesh_snapshot_matches_windows_project_path_to_linux_team_config() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    let mut request = sample_preflight_request();
    request.lead.project_id = "/home/user/projects/lead".to_string();
    request.agents[0].project_id = "/mnt/c/Users/me/code/taurhaus".to_string();
    request.agents[1].project_id = "/home/user/projects/reviewer".to_string();

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        request,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        r"C:\Users\me\code\taurhaus".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("architecture-final"));
    assert!(snapshot.team_status.is_some());
    assert!(snapshot.warnings.is_empty());
}

#[test]
fn project_mesh_snapshot_reports_mesh_unavailable_when_binary_is_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["tmux"]);

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        "proj-core".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert!(!snapshot.mesh_available);
    assert!(snapshot.tmux_available);
    assert_eq!(snapshot.team_runtime_state, TeamRuntimeState::None);
    assert_eq!(snapshot.team_name, None);
}

#[test]
fn project_mesh_snapshot_skips_missing_config_dirs_without_warning() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    std::fs::create_dir_all(tmp.path().join("default").join("inboxes")).expect("create stale dir");
    std::fs::write(
        tmp.path()
            .join("default")
            .join("inboxes")
            .join("team-lead.json"),
        "{}",
    )
    .expect("write stale inbox");

    let snapshot = coordination_get_project_mesh_snapshot_with_lookup(
        &state,
        "proj-core".to_string(),
        &lookup,
    )
    .expect("snapshot should succeed");

    assert_eq!(snapshot.team_name.as_deref(), Some("architecture-final"));
    assert!(
        snapshot.warnings.is_empty(),
        "missing config folders should be silently skipped"
    );
}

#[test]
fn initialize_team_request_round_trip() {
    let value = InitializeTeamRequest {
        team_name: "architecture-final".to_string(),
        team_description: Some("Cross-project implementation team".to_string()),
        preset_id: None,
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-core".to_string(),
            description: Some("Own orchestration".to_string()),
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            reasoning_effort: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            account_id: None,
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "agy".to_string(),
                model: "pro".to_string(),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                project_id: "proj-api".to_string(),
                description: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                reasoning_effort: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                account_id: None,
            },
        ],
    };

    let json = serde_json::to_string(&value).expect("serialize init request");
    let decoded: InitializeTeamRequest =
        serde_json::from_str(&json).expect("deserialize init request");
    assert_eq!(decoded, value);
}

#[test]
fn initialize_report_round_trip_includes_required_fields() {
    let value = InitializeReport {
        team_name: "architecture-final".to_string(),
        succeeded_steps: vec![
            "validate_configuration".to_string(),
            "create_team".to_string(),
        ],
        failed_step: Some("launch_sessions".to_string()),
        retryable: true,
        message: "launch failed for one member".to_string(),
        steps: vec![
            StepProgress {
                step: "validate_configuration".to_string(),
                status: StepStatus::Succeeded,
                message: Some("ok".to_string()),
            },
            StepProgress {
                step: "launch_sessions".to_string(),
                status: StepStatus::Failed,
                message: Some("codex binary missing".to_string()),
            },
        ],
    };

    let json = serde_json::to_string(&value).expect("serialize init report");
    let decoded: InitializeReport = serde_json::from_str(&json).expect("deserialize init report");
    assert_eq!(decoded, value);
    assert_eq!(decoded.failed_step, Some("launch_sessions".to_string()));
    assert!(decoded.retryable);
}

#[test]
fn add_agent_request_and_report_round_trip() {
    let request = AddAgentRequest {
        team_name: "architecture-final".to_string(),
        agent: AgentSetupConfig {
            name: "backend-dev".to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            communication_style: None,
            runtime_compact_summary: None,
            project_id: "proj-api".to_string(),
            description: Some("API ownership".to_string()),
            instructions: None,
            behavioral_contract: None,
            quality_gates: None,
            reasoning_effort: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            account_id: None,
        },
    };
    let req_json = serde_json::to_string(&request).expect("serialize add-agent request");
    let req_decoded: AddAgentRequest =
        serde_json::from_str(&req_json).expect("deserialize add-agent request");
    assert_eq!(req_decoded, request);

    let report = AddAgentReport {
        team_name: "architecture-final".to_string(),
        member_name: "backend-dev".to_string(),
        succeeded_steps: vec![
            "create_pane".to_string(),
            "launch_session".to_string(),
            "join_mesh".to_string(),
        ],
        failed_step: None,
        retryable: false,
        message: "agent added".to_string(),
        steps: vec![StepProgress {
            step: "join_mesh".to_string(),
            status: StepStatus::Succeeded,
            message: Some("joined".to_string()),
        }],
        warnings: vec!["onboarding wake failed".to_string()],
    };
    let report_json = serde_json::to_string(&report).expect("serialize add-agent report");
    let report_decoded: AddAgentReport =
        serde_json::from_str(&report_json).expect("deserialize add-agent report");
    assert_eq!(report_decoded, report);
}

#[test]
fn resume_member_request_and_report_round_trip() {
    let request = sample_resume_request("architecture-final", "backend-dev");
    let req_json = serde_json::to_string(&request).expect("serialize resume-member request");
    let req_decoded: ResumeMemberRequest =
        serde_json::from_str(&req_json).expect("deserialize resume-member request");
    assert_eq!(req_decoded, request);

    let report = ResumeAgentReport {
        team_name: "architecture-final".to_string(),
        member_name: "backend-dev".to_string(),
        resumed: true,
        succeeded_steps: vec![
            "validate".to_string(),
            "resolve_pane".to_string(),
            "launch_session".to_string(),
            "update_runtime".to_string(),
        ],
        failed_step: None,
        retryable: false,
        message: "member resumed".to_string(),
        steps: vec![StepProgress {
            step: "launch_session".to_string(),
            status: StepStatus::Succeeded,
            message: Some("session launched".to_string()),
        }],
        warnings: vec![],
        pane_id: Some("%12".to_string()),
        reused_pane: true,
    };
    let report_json = serde_json::to_string(&report).expect("serialize resume-member report");
    let report_decoded: ResumeAgentReport =
        serde_json::from_str(&report_json).expect("deserialize resume-member report");
    assert_eq!(report_decoded, report);
}

#[test]
fn resume_team_request_and_report_round_trip() {
    let request = ResumeTeamRequest {
        team_name: "architecture-final".to_string(),
    };
    let req_json = serde_json::to_string(&request).expect("serialize resume-team request");
    let req_decoded: ResumeTeamRequest =
        serde_json::from_str(&req_json).expect("deserialize resume-team request");
    assert_eq!(req_decoded, request);

    let report = ResumeTeamReport {
        team_name: "architecture-final".to_string(),
        resumed: true,
        total_members: 3,
        resumed_members: vec!["team-lead".to_string(), "reviewer".to_string()],
        failed_members: vec![ResumeTeamMemberFailure {
            member_name: "builder".to_string(),
            message: "mesh join failed".to_string(),
            retryable: true,
        }],
        warnings: vec!["builder: created a replacement pane".to_string()],
        started_team_daemon: false,
        team_daemon_warning: Some("team daemon start not implemented".to_string()),
    };
    let report_json = serde_json::to_string(&report).expect("serialize resume-team report");
    let report_decoded: ResumeTeamReport =
        serde_json::from_str(&report_json).expect("deserialize resume-team report");
    assert_eq!(report_decoded, report);
}

#[test]
fn remove_agent_report_round_trip() {
    let value = RemoveAgentReport {
        team_name: "architecture-final".to_string(),
        member_name: "backend-dev".to_string(),
        removed: true,
        message: "member removed with 1 warning".to_string(),
        steps: vec![
            StepProgress {
                step: "leave_mesh".to_string(),
                status: StepStatus::Succeeded,
                message: Some("mesh presence removed".to_string()),
            },
            StepProgress {
                step: "kill_pane".to_string(),
                status: StepStatus::Failed,
                message: Some("skipped pane kill for %2 due to ownership mismatch".to_string()),
            },
        ],
        warnings: vec!["skipped pane teardown for %2: ownership mismatch".to_string()],
    };
    let json = serde_json::to_string(&value).expect("serialize remove-agent report");
    let decoded: RemoveAgentReport =
        serde_json::from_str(&json).expect("deserialize remove-agent report");
    assert_eq!(decoded, value);
}

#[test]
fn live_team_status_round_trip() {
    let value = LiveTeamStatus {
        team_name: "architecture-final".to_string(),
        lead_name: "team-lead".to_string(),
        runtime_snapshot_freshness:
            crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::Fresh,
        members: vec![
            LiveAgentStatus {
                name: "team-lead".to_string(),
                role: AgentRole::Lead,
                cli_tool: "claude".to_string(),
                model: "opus".to_string(),
                reasoning_effort: None,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                project_id: "proj-core".to_string(),
                is_cross_project: false,
                project_label: String::new(),
                description: Some("orchestrates work".to_string()),
                session_status: SessionStatus::Active,
                pane_id: Some("%1".to_string()),
                session_id: Some("sess-lead".to_string()),
                workflow_activity: None,
                task_effort: None,
                task_effort_why: None,
                account_applied: None,
                account_note: None,
                account_note_detail: None,
            },
            LiveAgentStatus {
                name: "frontend-dev".to_string(),
                role: AgentRole::Member,
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                reasoning_effort: Some("high".to_string()),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                project_id: "proj-web".to_string(),
                is_cross_project: true,
                project_label: "proj-web".to_string(),
                description: None,
                session_status: SessionStatus::Idle,
                pane_id: Some("%2".to_string()),
                session_id: None,
                workflow_activity: None,
                task_effort: None,
                task_effort_why: None,
                account_applied: Some(false),
                account_note: Some("opaque_base_command".to_string()),
                account_note_detail: Some("team-wrapper".to_string()),
            },
        ],
    };

    let json = serde_json::to_string(&value).expect("serialize live team status");
    let decoded: LiveTeamStatus =
        serde_json::from_str(&json).expect("deserialize live team status");
    assert_eq!(decoded, value);
}

#[test]
fn project_mesh_snapshot_round_trip() {
    let value = ProjectMeshSnapshotResponse {
        mesh_available: true,
        tmux_available: true,
        team_runtime_state: TeamRuntimeState::Active,
        team_name: Some("architecture-final".to_string()),
        team_status: Some(FastTeamSnapshot {
            lead_name: "team-lead".to_string(),
            members: vec![FastAgentSnapshot {
                name: "frontend-dev".to_string(),
                role: AgentRole::Member,
                cli_tool: "codex".to_string(),
                model: Some("gpt-5.4".to_string()),
                reasoning_effort: Some("high".to_string()),
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                project_id: "proj-web".to_string(),
                is_cross_project: true,
                project_label: "proj-web".to_string(),
                description: Some("UI implementation".to_string()),
                session_status: SessionStatus::Idle,
                pane_id: Some("%2".to_string()),
                session_id: Some("sess-frontend".to_string()),
                workflow_activity: None,
                task_effort: None,
                task_effort_why: None,
                account_applied: Some(false),
                account_note: Some("opaque_base_command".to_string()),
                account_note_detail: Some("team-wrapper".to_string()),
            }],
        }),
        warnings: vec!["skipped team folder 'broken-team'".to_string()],
    };

    let json = serde_json::to_string(&value).expect("serialize project mesh snapshot");
    let decoded: ProjectMeshSnapshotResponse =
        serde_json::from_str(&json).expect("deserialize project mesh snapshot");
    assert_eq!(decoded, value);
}

#[test]
fn live_status_test_helper_invokes_live_status_impl() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let status =
        coordination_get_live_team_status_for_tests(&state, "architecture-final".to_string())
            .expect("live status should succeed");
    assert_eq!(status.team_name, "architecture-final");
    assert_eq!(status.lead_name, "team-lead");
    assert_eq!(
        status.runtime_snapshot_freshness,
        crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::AttachmentsOnly
    );
    assert!(!status.members.is_empty());
    let frontend_dev = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev should be present");
    assert!(frontend_dev.is_cross_project);
    assert_eq!(frontend_dev.project_label, "proj-web");
}

#[test]
fn live_status_uses_lightweight_presence_reconcile_without_heavy_daemon_calls() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime.clone());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let call_count_before_live_status = runtime.calls().len();

    let status =
        coordination_get_live_team_status_for_tests(&state, "architecture-final".to_string())
            .expect("live status should succeed");

    assert_eq!(status.team_name, "architecture-final");
    assert_eq!(
        status.runtime_snapshot_freshness,
        crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::AttachmentsOnly
    );
    let delta = &runtime.calls()[call_count_before_live_status..];
    assert!(
        !delta.is_empty(),
        "live status should do pane-presence checks"
    );
    assert!(delta.iter().all(|call| {
        matches!(
            call,
            RuntimeCall::CheckPaneExists { .. }
                | RuntimeCall::CheckPaneDead { .. }
                | RuntimeCall::CheckPaneShell { .. }
                | RuntimeCall::InspectPane { .. }
        )
    }));
}

#[test]
fn live_status_provider_snapshot_yields_to_current_pane_loss() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = Arc::new(test_state_with_runtime(
        tmp.path().to_path_buf(),
        runtime.clone(),
    ));
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize");

    let frontend_runtime =
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
            .expect("frontend runtime");
    let lead_runtime = MemberRuntimeStore::load(tmp.path(), "architecture-final", "team-lead")
        .expect("lead runtime");
    let reviewer_runtime = MemberRuntimeStore::load(tmp.path(), "architecture-final", "reviewer")
        .expect("reviewer runtime");

    let frontend_pane_id = frontend_runtime.pane_id.clone().expect("frontend pane id");
    runtime.set_pane_exists(&frontend_pane_id, false);

    let snapshot_payload = serde_json::json!({
        "version": 3,
        "display_sessions": [],
        "runtime_sessions": [
            {
                "pid": 101,
                "project_path": "/projects/core",
                "tty": "pts/1",
                "args": "claude",
                "cli_tool": "claude",
                "tmux_session": "taurhaus",
                "tmux_window": "@1",
                "tmux_pane": lead_runtime.pane_id.clone().expect("lead pane id"),
                "tmux_window_name": "architecture-final:team-lead",
                "state": "active",
                "session_id": lead_runtime.session_id.clone(),
                "jsonl_path": null,
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "mesh_team",
                "group_id": "architecture-final",
                "group_label": "architecture-final",
                "member_name": "team-lead"
            },
            {
                "pid": 202,
                "project_path": "/projects/web",
                "tty": "pts/2",
                "args": "codex",
                "cli_tool": "codex",
                "tmux_session": "taurhaus",
                "tmux_window": "@2",
                "tmux_pane": frontend_pane_id,
                "tmux_window_name": "architecture-final:frontend-dev",
                "state": "active",
                "session_id": frontend_runtime.session_id.clone(),
                "jsonl_path": null,
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "mesh_team",
                "group_id": "architecture-final",
                "group_label": "architecture-final",
                "member_name": "frontend-dev"
            },
            {
                "pid": 303,
                "project_path": "/projects/api",
                "tty": "pts/3",
                "args": "agy",
                "cli_tool": "agy",
                "tmux_session": "taurhaus",
                "tmux_window": "@3",
                "tmux_pane": reviewer_runtime.pane_id.clone().expect("reviewer pane id"),
                "tmux_window_name": "architecture-final:reviewer",
                "state": "active",
                "session_id": reviewer_runtime.session_id.clone(),
                "jsonl_path": null,
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "mesh_team",
                "group_id": "architecture-final",
                "group_label": "architecture-final",
                "member_name": "reviewer"
            }
        ],
        "focus": null,
        "foreground_project_path": "/projects/core"
    });
    let decoded_snapshot =
        crate::commands::runtime_snapshot::decode_daemon_runtime_session_snapshot(Some(
            snapshot_payload.clone(),
        ))
        .expect("decode runtime snapshot payload");
    assert_eq!(decoded_snapshot.runtime_sessions.len(), 3);

    let daemon = start_live_status_stub_daemon(
        serde_json::json!({
            "result": snapshot_payload,
            "error": null
        }),
        state.clone(),
    );
    let provider = ProviderState {
        local: taurhaus_lib::provider::local::LocalProvider,
        daemon: Some(
            taurhaus_lib::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };

    // Regression: d593f81b moved presence writes behind the daemon without
    // teaching this provider-backed path to observe the daemon's reconciliation
    // result, so pane-loss members stayed active in the returned roster.
    let status = coordination_get_live_team_status_impl(
        &state,
        Some(&provider),
        "architecture-final".to_string(),
    )
    .expect("live status should succeed");

    assert_eq!(
        status.runtime_snapshot_freshness,
        crate::commands::coordination_types::LiveRuntimeSnapshotFreshness::Fresh
    );
    let frontend_dev = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev should exist");
    assert_eq!(frontend_dev.session_status, SessionStatus::Offline);

    let updated = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("reload frontend runtime");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
}

#[test]
fn step_progress_event_round_trip() {
    let value = StepProgressEvent {
        team_name: "architecture-final".to_string(),
        operation: "initialize_team".to_string(),
        progress: StepProgress {
            step: "send_onboarding".to_string(),
            status: StepStatus::Running,
            message: Some("sending to frontend-dev".to_string()),
        },
        canonical_stages: vec![MemberActivationStage::DeliverOnboarding],
    };

    let json = serde_json::to_string(&value).expect("serialize step progress event");
    let decoded: StepProgressEvent =
        serde_json::from_str(&json).expect("deserialize step progress event");
    assert_eq!(decoded, value);
}

#[test]
fn initialize_progress_events_include_canonical_stage_metadata() {
    let report = InitializeReport {
        team_name: "architecture-final".to_string(),
        succeeded_steps: vec!["create_panes".to_string()],
        failed_step: None,
        retryable: false,
        message: "ok".to_string(),
        steps: vec![StepProgress {
            step: "create_panes".to_string(),
            status: StepStatus::Succeeded,
            message: Some("opened panes".to_string()),
        }],
    };

    let events = super::progress::initialize_progress_events(&report);

    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].canonical_stages,
        vec![
            MemberActivationStage::AcquirePane,
            MemberActivationStage::LaunchSession,
        ]
    );
    assert_eq!(events[1].canonical_stages, events[0].canonical_stages);
}

#[test]
fn initialize_stage_mapping_preserves_existing_batch_steps() {
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::PrepareMember),
        Some("validate_configuration")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::AcquirePane),
        Some("create_panes")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::LaunchSession),
        Some("create_panes")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(
            MemberActivationStage::CaptureSessionIdentity
        ),
        Some("launch_sessions")
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::CommitRuntime),
        None
    );
    assert_eq!(
        super::progress::initialize_step_for_member_stage(MemberActivationStage::DeliverOnboarding),
        Some("send_onboarding")
    );
}

#[test]
fn resume_member_progress_event_helper_uses_canonical_stage_step_name() {
    let event = super::progress::resume_member_progress_event_for_stage(
        "architecture-final",
        MemberActivationStage::CommitRuntime,
        StepStatus::Running,
        Some("writing runtime".to_string()),
    );

    assert_eq!(event.team_name, "architecture-final");
    assert_eq!(event.operation, "resume_member");
    assert_eq!(event.progress.step, "commit_runtime");
    assert_eq!(event.progress.status, StepStatus::Running);
    assert_eq!(
        event.canonical_stages,
        vec![MemberActivationStage::CommitRuntime]
    );
}

#[test]
fn resume_team_progress_event_round_trip() {
    let value = ResumeTeamProgressEvent {
        operation: "resume_team".to_string(),
        team_name: "architecture-final".to_string(),
        member_name: "frontend-dev".to_string(),
        member_index: 2,
        member_count: 3,
        stage: MemberActivationStage::CommitRuntime,
        status: StepStatus::Running,
        message: Some("writing runtime state".to_string()),
    };

    let json = serde_json::to_string(&value).expect("serialize resume-team progress event");
    let decoded: ResumeTeamProgressEvent =
        serde_json::from_str(&json).expect("deserialize resume-team progress event");
    assert_eq!(decoded, value);
}

// Regression: add-agent pipeline failure with db present must not mask the
// real error. Before the fix, cleanup_add_agent_failure removed the member,
// then sync_member_snapshot_after_change tried to look up the now-deleted
// member and threw "member not found in team", hiding the actual pipeline
// failure. Commit 077d57d.
#[test]
fn add_agent_pipeline_failure_does_not_mask_error_with_member_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = FakeBackend::default();
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding delivery failure".to_string(),
    ));
    let state = CoordinationState::with_components_and_runtime(
        tmp.path().to_path_buf(),
        BackendSelector::m0(),
        Arc::new({
            let fake = fake.clone();
            move |_kind, _teams_dir| Ok(Arc::new(fake.clone()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(|| Arc::new(RecordingCoordinationRuntime::default())),
    );
    coordination_create_team_impl(&state, "arch".to_string()).expect("create team");
    let (db, _db_file) = test_db_state();

    let report = add_agent_pipeline_test_fixture(
        &state,
        Some(&db),
        sample_add_agent_request("arch", "dev2"),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("add-agent should return structured failure report, not Err");

    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(
        report
            .message
            .contains("simulated onboarding delivery failure"),
        "expected original pipeline error, got: {}",
        report.message
    );
}

// Regression: add-agent onboarding for Claude agents must route through
// deliver_message (per-member backend selection) rather than the default
// backend directly. Before the fix, Claude agents got routed through
// MeshBridgedBackend instead of ClaudeNativeBackend, causing delivery
// failures. Commit 0a0ec11.
#[test]
fn add_agent_onboarding_routes_through_deliver_message_audit_trail() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    coordination_create_team_impl(&state, "arch".to_string()).expect("create");

    let mut claude_agent = sample_add_agent_request("arch", "claude-dev");
    claude_agent.agent.cli_tool = "claude".to_string();
    claude_agent.agent.focus_area = Some("backend".to_string());
    claude_agent.agent.context_summary = Some("Rust backend developer".to_string());

    let report = add_agent_pipeline_test_fixture(
        &state,
        None,
        claude_agent,
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("add-agent should return report");

    assert!(
        report.failed_step.is_none(),
        "add-agent failed at step {:?}: {}",
        report.failed_step,
        report.message
    );
    assert!(report
        .succeeded_steps
        .contains(&"send_onboarding".to_string()));

    // deliver_message emits audit events; verify the onboarding delivery
    // went through the audited path (not self.backend.deliver directly)
    let audit = state
        .with_orchestrator(|orchestrator| {
            Ok::<_, CoordinationError>(orchestrator.drain_audit_log())
        })
        .expect("drain audit");
    let event_types: Vec<&str> = audit.iter().map(|event| event.event_type()).collect();
    assert!(
        event_types.contains(&"delivery_attempted"),
        "expected delivery_attempted audit event for Claude onboarding, got: {:?}",
        event_types
    );
}

#[test]
fn live_team_status_carries_the_member_runtime_session_id() {
    // Regression: 9e15e4e keyed the mesh canvas run tree on a node's Claude
    // session, but LiveAgentStatus never serialized the session the runtime
    // record already held, so no runtime node could ask for its workflow runs.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("member runtime");
    record.session_id = Some("sess-frontend".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");

    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.session_id.as_deref(), Some("sess-frontend"));
}

#[test]
fn live_team_status_carries_the_opaque_base_account_note() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let note = taurhaus_lib::session_scanner::launch_base::LaunchAccountResult::for_opaque_head(
        Some("team-wrapper"),
    );
    MemberRuntimeStore::update(tmp.path(), "architecture-final", "frontend-dev", |record| {
        record.launch_account = note.clone();
    })
    .expect("update runtime");
    assert_eq!(
        MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
            .expect("reload runtime")
            .launch_account,
        note
    );

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");
    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.account_applied, Some(false));
    assert_eq!(member.account_note.as_deref(), Some("opaque_base_command"));
    assert_eq!(member.account_note_detail.as_deref(), Some("team-wrapper"));
}

#[test]
fn live_team_status_carries_the_task_effort_the_lead_asked_for() {
    // The node shows the launch effort; the level the lead attached to the
    // current assignment is a different number and has to travel beside it.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    seed_assignment_effort(tmp.path(), "frontend-dev", "high", "irreversible migration");

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");

    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.task_effort.as_deref(), Some("high"));
    assert_eq!(
        member.task_effort_why.as_deref(),
        Some("irreversible migration")
    );

    let untouched = status
        .members
        .iter()
        .find(|member| member.name != "frontend-dev")
        .expect("the team has another member");
    assert_eq!(untouched.task_effort, None);
    assert_eq!(untouched.task_effort_why, None);
}

#[test]
fn project_mesh_snapshot_carries_the_task_effort_the_lead_asked_for() {
    // The cold-start roster feeds the same canvas, so it carries the same pair.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    seed_assignment_effort(tmp.path(), "frontend-dev", "medium", "routine lane work");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    let member = snapshot
        .team_status
        .expect("team status")
        .members
        .into_iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.task_effort.as_deref(), Some("medium"));
    assert_eq!(member.task_effort_why.as_deref(), Some("routine lane work"));
}

fn seed_assignment_effort(teams_dir: &std::path::Path, member_name: &str, level: &str, why: &str) {
    // mesh writes the effort onto the task record it assigns; taurhaus reads
    // it back off the task the member is on, so that is what a fixture seeds.
    let config =
        crate::coordination::stores::TeamConfigStore::load(teams_dir, "architecture-final")
            .expect("team config");
    let project_path = config
        .members
        .iter()
        .find(|member| member.name == member_name)
        .expect("member is on the roster")
        .project_path
        .display()
        .to_string();

    let db = tempfile::NamedTempFile::new().expect("temp db");
    let conn = taurhaus_lib::db::init_db(db.path()).expect("db");
    taurhaus_lib::db::task_queries::upsert_task(
        &conn,
        &taurhaus_lib::db::task_queries::PersistedTask {
            project_path,
            source: "claude".to_string(),
            source_key: "session-1".to_string(),
            source_task_id: "42".to_string(),
            subject: "Run the migration".to_string(),
            description: None,
            active_form: None,
            status: "in_progress".to_string(),
            blocks: vec![],
            blocked_by: vec![],
            owner: Some(member_name.to_string()),
            session_id: None,
            first_seen_at: "2026-08-29T09:00:00Z".to_string(),
            state_changed_at: Some("2026-08-29T09:00:00Z".to_string()),
            updated_at: "2026-08-29T09:00:00Z".to_string(),
            archived_at: None,
            last_status: Some("in_progress".to_string()),
            archived_reason: None,
            effort: Some(level.to_string()),
            effort_why: Some(why.to_string()),
            deadline_minutes: None,
        },
    )
    .expect("upsert assigned task");

    crate::coordination::operational_context::sync_team_snapshots(
        teams_dir,
        &conn,
        "architecture-final",
    )
    .expect("sync snapshots");
}

#[test]
fn project_mesh_snapshot_carries_the_member_runtime_session_id() {
    // Regression: the fast snapshot fed the same canvas and dropped the same
    // field, so the cold-start roster could not load runs either (9e15e4e).
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);
    let lookup = MockBinaryLookup::with_available(&["mesh", "tmux"]);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("member runtime");
    record.session_id = Some("sess-frontend".to_string());
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let snapshot =
        coordination_get_project_mesh_snapshot_with_lookup(&state, "proj-web".to_string(), &lookup)
            .expect("snapshot should succeed");

    let member = snapshot
        .team_status
        .expect("team status")
        .members
        .into_iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(member.session_id.as_deref(), Some("sess-frontend"));
}

#[test]
fn live_team_status_carries_the_member_workflow_activity() {
    // Regression: d442cf6 gave a runtime node its Claude session but not the
    // workflow hint, so a member whose run tree was visibly live still read
    // Active or Idle on the canvas — the node carried nothing but coordination
    // health, and `activitySignal` had no workflow evidence to promote.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let state = test_state_with_runtime(tmp.path().to_path_buf(), runtime);

    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    // A scratch transcript with one summary-less run whose agent just wrote.
    let transcripts = tmp.path().join("transcripts");
    let transcript = transcripts.join("sess-frontend.jsonl");
    let run_dir = transcripts.join("sess-frontend/subagents/workflows/wf_live");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(&transcript, "").expect("transcript");
    std::fs::write(run_dir.join("agent-a1.jsonl"), "{}\n").expect("agent transcript");

    let mut record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "frontend-dev")
        .expect("member runtime");
    record.cli_tool = Some(CliTool::Claude);
    record.session_id = Some("sess-frontend".to_string());
    record.jsonl_path = Some(transcript);
    MemberRuntimeStore::save(tmp.path(), "architecture-final", "frontend-dev", &record)
        .expect("save runtime");

    let status =
        coordination_get_live_team_status_impl(&state, None, "architecture-final".to_string())
            .expect("live status should succeed");

    let member = status
        .members
        .iter()
        .find(|member| member.name == "frontend-dev")
        .expect("frontend-dev is on the roster");
    assert_eq!(
        member
            .workflow_activity
            .as_ref()
            .map(|activity| activity.live_runs),
        Some(1)
    );
}

fn daemon_runtime_session(
    team_name: &str,
    member_name: &str,
    jsonl_path: &str,
    workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
) -> crate::session_scanner::RuntimeSession {
    crate::session_scanner::RuntimeSession {
        pid: 4242,
        project_path: "/tmp/taurhaus".to_string(),
        tty: "/dev/pts/3".to_string(),
        args: "claude".to_string(),
        cli_tool: CliTool::Claude,
        tmux_session: Some("taurhaus".to_string()),
        tmux_window: None,
        tmux_pane: Some("%17".to_string()),
        tmux_window_name: None,
        state: crate::session_scanner::SessionState::Active,
        session_id: Some("sess-frontend".to_string()),
        jsonl_path: Some(jsonl_path.to_string()),
        recent_io: false,
        last_output_age_secs: None,
        activity_confidence: Default::default(),
        activity_attribution: Default::default(),
        project_unattributed_active: false,
        group_kind: crate::session_scanner::SessionGroupKind::MeshTeam,
        group_id: Some(team_name.to_string()),
        group_label: None,
        member_name: Some(member_name.to_string()),
        workflow_activity,
    }
}

// Regression: acefb7a answered "is this member running a workflow?" by
// rescanning the transcript in the desktop process. The daemon already computed
// that hint and ships it on the runtime session, and on Windows the transcript
// it names is a WSL path the desktop cannot open — the rescan found nothing and
// the member never showed Working beside its live run tree. The daemon's value
// wins; a local scan is only the fallback, and only for a path this host can
// actually read.
#[test]
fn member_workflow_activity_prefers_the_daemon_hint_over_a_local_rescan() {
    let tmp = TempDir::new().expect("tempdir");
    let state = test_state(tmp.path().to_path_buf());
    initialize_team_pipeline_test_fixture(
        &state,
        None,
        sample_preflight_request(),
        &crate::models::CliCommandSettings::default(),
        DEFAULT_TMUX_LAYOUT,
        None,
    )
    .expect("initialize should succeed");

    // A transcript this host really can read, whose session dir holds one live run.
    let transcripts = tmp.path().join("transcripts");
    let local_transcript = transcripts.join("sess-frontend.jsonl");
    let run_dir = transcripts.join("sess-frontend/subagents/workflows/wf_live");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    std::fs::write(&local_transcript, "").expect("transcript");
    std::fs::write(run_dir.join("agent-a1.jsonl"), "{}\n").expect("agent transcript");
    let local_transcript = local_transcript.display().to_string();
    // The path a WSL daemon reports to a Windows desktop.
    let remote_transcript = "/home/daemon-host/.claude/projects/-tmp-taurhaus/sess-frontend.jsonl";

    let member_view = |jsonl_path: &str, live_runs: Option<u32>| {
        let sessions = vec![daemon_runtime_session(
            "architecture-final",
            "frontend-dev",
            jsonl_path,
            live_runs.map(|live_runs| crate::workflow_runs::WorkflowActivity {
                live_runs,
                last_write_at: 1_772_000_000_000,
            }),
        )];
        crate::coordination::roster::get_team_roster_with_runtime_sessions(
            tmp.path(),
            "architecture-final",
            &sessions,
        )
        .expect("roster")
        .into_iter()
        .find(|member| member.member_name == "frontend-dev")
        .expect("frontend-dev is on the roster")
    };

    // A path this host cannot read: the daemon's count is all there is, and it stands.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(remote_transcript, Some(3)))
            .map(|activity| activity.live_runs),
        Some(3)
    );
    // A readable path whose scan would disagree: the daemon's count still wins,
    // which is only possible if no local scan was consulted.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(&local_transcript, Some(3)))
            .map(|activity| activity.live_runs),
        Some(3)
    );
    // No daemon value and a readable path: the local scan still answers.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(&local_transcript, None))
            .map(|activity| activity.live_runs),
        Some(1)
    );
    // No daemon value and a path this host cannot read: nothing to say.
    assert_eq!(
        live_status::member_workflow_activity(&member_view(remote_transcript, None)),
        None
    );
}
