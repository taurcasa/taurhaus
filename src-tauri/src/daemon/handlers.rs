#[cfg(feature = "mesh-bridged-backend")]
use std::collections::HashSet;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::daemon::protocol::{self, DaemonRequest, DaemonResponse};
use crate::daemon::watch::{handle_unwatch, handle_watch, WatchRuntime};
use crate::project_provider::ProjectProvider;
use crate::task_scanner::claude_index::{
    build_claude_source_index_with_live_sessions, ClaudeSourceIndex,
};

#[derive(Debug, Clone)]
struct ProjectTaskScanCache {
    cycle_id: u64,
    sessions: Vec<crate::session_scanner::RuntimeSession>,
    claude_index: ClaudeSourceIndex,
}

#[cfg(feature = "mesh-bridged-backend")]
type TerminalObservationKey = (std::path::PathBuf, String, String, String, bool);

#[derive(Debug, Default)]
pub(crate) struct ProjectTaskScanCacheState {
    cache: Mutex<Option<ProjectTaskScanCache>>,
    #[cfg(feature = "mesh-bridged-backend")]
    terminal_observations: Mutex<HashSet<TerminalObservationKey>>,
}

/// Dispatch a request to the appropriate handler.
pub(crate) fn dispatch(
    request: &DaemonRequest,
    provider: &dyn ProjectProvider,
    start_time: Instant,
    writer: &Arc<Mutex<TcpStream>>,
    watch_runtime: &mut WatchRuntime,
    project_task_scan_cache: &ProjectTaskScanCacheState,
    #[cfg(feature = "mesh-bridged-backend")] coordination_services: (
        &crate::daemon::initialize_runs::InitializeTeamService,
        &crate::daemon::member_runs::MemberOperationsService,
        &crate::daemon::team_runs::TeamOperationsService,
        &crate::daemon::roster_runs::RosterOperationsService,
        &crate::daemon::effort_runs::EffortOperationsService,
        &crate::daemon::background_scheduler::LaunchSettingsStore,
        &crate::coordination::state::CoordinationState,
    ),
) -> DaemonResponse {
    #[cfg(feature = "mesh-bridged-backend")]
    let (
        initialize_service,
        member_operations_service,
        team_operations_service,
        roster_operations_service,
        effort_operations_service,
        launch_settings,
        coordination_state,
    ) = coordination_services;
    tracing::debug!(method = %request.method, id = %request.id, "Received request");
    match request.method.as_str() {
        protocol::method::PING => handle_ping(&request.id, start_time),
        protocol::method::GIT_STATUS => handle_git_status(&request.id, &request.params, provider),
        protocol::method::GIT_LOG => handle_git_log(&request.id, &request.params, provider),
        protocol::method::GIT_LATEST_COMMIT_TIME => {
            handle_git_latest_commit_time(&request.id, &request.params, provider)
        }
        protocol::method::FILE_TREE => handle_file_tree(&request.id, &request.params, provider),
        protocol::method::READ_FILE => handle_read_file(&request.id, &request.params, provider),
        protocol::method::READ_README => handle_read_readme(&request.id, &request.params, provider),
        protocol::method::READ_ASSET => handle_read_asset(&request.id, &request.params, provider),
        protocol::method::SCAN_SESSIONS => {
            handle_scan_sessions(&request.id, &request.params, provider)
        }
        protocol::method::LIST_DISPLAY_SESSIONS => handle_list_display_sessions(&request.id),
        protocol::method::GET_RUNTIME_SESSION_SNAPSHOT => {
            handle_get_runtime_session_snapshot(&request.id)
        }
        protocol::method::LIST_RUNTIME_SESSIONS => handle_list_runtime_sessions(&request.id),
        protocol::method::WAIT_SESSION_UPDATES => {
            handle_wait_session_updates(&request.id, &request.params)
        }
        protocol::method::LAUNCH_SESSION => handle_launch_session(&request.id, &request.params),
        protocol::method::STOP_SESSION => handle_stop_session(&request.id, &request.params),
        protocol::method::NAVIGATE_TO_SESSION => {
            handle_navigate_to_session(&request.id, &request.params)
        }
        protocol::method::GET_PROJECT_TASKS => {
            handle_get_project_tasks(&request.id, &request.params, project_task_scan_cache)
        }
        protocol::method::GIT_COMMITS_IN_RANGE => {
            handle_git_commits_in_range(&request.id, &request.params, provider)
        }
        protocol::method::GIT_COMMIT_FILES => {
            handle_git_commit_files(&request.id, &request.params, provider)
        }
        protocol::method::GIT_COMMIT_DIFF => {
            handle_git_commit_diff(&request.id, &request.params, provider)
        }
        protocol::method::WATCH => {
            handle_watch(&request.id, &request.params, writer, watch_runtime)
        }
        protocol::method::UNWATCH => handle_unwatch(&request.id, &request.params, watch_runtime),
        protocol::method::SHUTDOWN => {
            DaemonResponse::ok(&request.id, serde_json::json!({"ok": true}))
        }
        protocol::method::LIST_ACCOUNTS => handle_list_accounts(&request.id, &request.params),
        protocol::method::PROJECT_TRANSCRIPT => {
            handle_project_transcript(&request.id, &request.params)
        }
        protocol::method::REFRESH_USAGE => handle_refresh_usage(&request.id, &request.params),
        protocol::method::RESOLVE_LAUNCH_BASE => {
            handle_resolve_launch_base(&request.id, &request.params)
        }
        protocol::method::LIST_WORKFLOW_RUNS => {
            handle_list_workflow_runs(&request.id, &request.params)
        }
        protocol::method::GET_WORKFLOW_RUN => handle_get_workflow_run(&request.id, &request.params),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_INITIALIZE_TEAM => {
            handle_coordination_initialize_team(&request.id, &request.params, initialize_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_INITIALIZE_STATUS => {
            handle_coordination_initialize_status(&request.id, &request.params, initialize_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_ADD_AGENT => {
            handle_coordination_add_agent(&request.id, &request.params, member_operations_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_ADD_AGENT_STATUS => handle_coordination_add_agent_status(
            &request.id,
            &request.params,
            member_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_RESUME_MEMBER => handle_coordination_resume_member(
            &request.id,
            &request.params,
            member_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_RESUME_MEMBER_STATUS => {
            handle_coordination_resume_member_status(
                &request.id,
                &request.params,
                member_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_RESUME_TEAM => {
            handle_coordination_resume_team(&request.id, &request.params, team_operations_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_RESUME_TEAM_STATUS => {
            handle_coordination_resume_team_status(
                &request.id,
                &request.params,
                team_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_SWITCH_TEAM_ACCOUNT => {
            handle_coordination_switch_team_account(
                &request.id,
                &request.params,
                team_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_SWITCH_TEAM_ACCOUNT_STATUS => {
            handle_coordination_switch_team_account_status(
                &request.id,
                &request.params,
                team_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_REONBOARD => {
            handle_coordination_reonboard(&request.id, &request.params, team_operations_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_REONBOARD_STATUS => handle_coordination_reonboard_status(
            &request.id,
            &request.params,
            team_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_CREATE_TEAM => {
            handle_coordination_create_team(&request.id, &request.params, roster_operations_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_CREATE_TEAM_STATUS => {
            handle_coordination_create_team_status(
                &request.id,
                &request.params,
                roster_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_DISBAND_TEAM => handle_coordination_disband_team(
            &request.id,
            &request.params,
            roster_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_DISBAND_TEAM_STATUS => {
            handle_coordination_disband_team_status(
                &request.id,
                &request.params,
                roster_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_ADD_MEMBER => {
            handle_coordination_add_member(&request.id, &request.params, roster_operations_service)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_ADD_MEMBER_STATUS => handle_coordination_add_member_status(
            &request.id,
            &request.params,
            roster_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_REMOVE_MEMBER => handle_coordination_remove_member(
            &request.id,
            &request.params,
            roster_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_REMOVE_MEMBER_STATUS => {
            handle_coordination_remove_member_status(
                &request.id,
                &request.params,
                roster_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_PUT_LAUNCH_SETTINGS => {
            handle_coordination_put_launch_settings(&request.id, &request.params, launch_settings)
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_APPLY_TASK_EFFORT => handle_coordination_apply_task_effort(
            &request.id,
            &request.params,
            effort_operations_service,
        ),
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_APPLY_TASK_EFFORT_STATUS => {
            handle_coordination_apply_task_effort_status(
                &request.id,
                &request.params,
                effort_operations_service,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_PUBLISH_OPERATIONAL_SNAPSHOTS => {
            handle_coordination_publish_operational_snapshots(
                &request.id,
                &request.params,
                coordination_state,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_RECONCILE_LIVE_PRESENCE => {
            handle_coordination_reconcile_live_presence(
                &request.id,
                &request.params,
                coordination_state,
            )
        }
        #[cfg(feature = "mesh-bridged-backend")]
        protocol::method::COORDINATION_SET_ACTIVE_PROJECT_TEAM => {
            handle_coordination_set_active_project_team(
                &request.id,
                &request.params,
                coordination_state,
            )
        }
        _ => DaemonResponse::err(
            &request.id,
            "UNKNOWN_METHOD",
            format!("Unknown method: {}", request.method),
        ),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_publish_operational_snapshots(
    id: &str,
    params: &serde_json::Value,
    state: &crate::coordination::state::CoordinationState,
) -> DaemonResponse {
    let params = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match crate::daemon::state_writes::publish_operational_snapshots_for_state(state, params) {
        Ok(result) => DaemonResponse::ok(id, result),
        Err(error) => DaemonResponse::err(id, "SNAPSHOT_PUBLICATION_FAILED", error.to_string()),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_reconcile_live_presence(
    id: &str,
    params: &serde_json::Value,
    state: &crate::coordination::state::CoordinationState,
) -> DaemonResponse {
    let params = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match crate::daemon::state_writes::reconcile_live_presence(state, params) {
        Ok(result) => DaemonResponse::ok(id, result),
        Err(error) => DaemonResponse::err(id, "LIVE_PRESENCE_RECONCILE_FAILED", error.to_string()),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_set_active_project_team(
    id: &str,
    params: &serde_json::Value,
    state: &crate::coordination::state::CoordinationState,
) -> DaemonResponse {
    let params = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match crate::daemon::state_writes::set_active_project_team_for_state(state, params) {
        Ok(result) => DaemonResponse::ok(id, result),
        Err(error) => {
            DaemonResponse::err(id, "ACTIVE_PROJECT_TEAM_UPDATE_FAILED", error.to_string())
        }
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_initialize_team(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::initialize_runs::InitializeTeamService,
) -> DaemonResponse {
    let params: protocol::CoordinationInitializeParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationInitializeAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "INITIALIZE_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_initialize_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::initialize_runs::InitializeTeamService,
) -> DaemonResponse {
    let params: protocol::CoordinationInitializeStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => DaemonResponse::err(
            id,
            "INITIALIZE_RUN_NOT_FOUND",
            format!("team initialization run '{}' was not found", params.run_id),
        ),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_add_agent(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::member_runs::MemberOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationAddAgentParams = match serde_json::from_value(params.clone())
    {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match service.start_add_agent(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationAddAgentAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "ADD_AGENT_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_add_agent_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::member_runs::MemberOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationAddAgentStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.add_agent_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "add-agent", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_resume_member(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::member_runs::MemberOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationResumeMemberParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start_resume_member(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationResumeMemberAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "RESUME_MEMBER_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_resume_member_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::member_runs::MemberOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationResumeMemberStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.resume_member_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "resume-member", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_resume_team(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::team_runs::TeamOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationResumeTeamParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start_resume_team(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationResumeTeamAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "RESUME_TEAM_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_resume_team_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::team_runs::TeamOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationResumeTeamStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.resume_team_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "resume-team", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_switch_team_account(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::team_runs::TeamOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationSwitchTeamAccountParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start_switch_team_account(params) {
        Ok(run_id) => DaemonResponse::ok(
            id,
            protocol::CoordinationSwitchTeamAccountAccepted { run_id },
        ),
        Err(error) => DaemonResponse::err(id, "SWITCH_TEAM_ACCOUNT_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_switch_team_account_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::team_runs::TeamOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationSwitchTeamAccountStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.switch_team_account_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "account-switch", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_reonboard(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::team_runs::TeamOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationReonboardParams = match serde_json::from_value(params.clone())
    {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match service.start_reonboard(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationReonboardAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "REONBOARD_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_reonboard_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::team_runs::TeamOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationReonboardStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.reonboard_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "reonboard", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_create_team(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationCreateTeamParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start_create_team(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationCreateTeamAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "CREATE_TEAM_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_create_team_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationCreateTeamStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.create_team_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "create-team", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_disband_team(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationDisbandTeamParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start_disband_team(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationDisbandTeamAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "DISBAND_TEAM_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_disband_team_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationDisbandTeamStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.disband_team_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "disband-team", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_add_member(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationAddMemberParams = match serde_json::from_value(params.clone())
    {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match service.start_add_member(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationAddMemberAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "ADD_MEMBER_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_add_member_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationAddMemberStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.add_member_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "add-member", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_remove_member(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationRemoveMemberParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start_remove_member(params) {
        Ok(run_id) => DaemonResponse::ok(id, protocol::CoordinationRemoveMemberAccepted { run_id }),
        Err(error) => DaemonResponse::err(id, "REMOVE_MEMBER_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_remove_member_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::roster_runs::RosterOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationRemoveMemberStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.remove_member_status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "remove-member", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_put_launch_settings(
    id: &str,
    params: &serde_json::Value,
    store: &crate::daemon::background_scheduler::LaunchSettingsStore,
) -> DaemonResponse {
    let params: protocol::CoordinationPutLaunchSettingsParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    DaemonResponse::ok(id, store.put(params))
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_apply_task_effort(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::effort_runs::EffortOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationApplyTaskEffortParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.start(params) {
        Ok(run_id) => {
            DaemonResponse::ok(id, protocol::CoordinationApplyTaskEffortAccepted { run_id })
        }
        Err(error) => DaemonResponse::err(id, "APPLY_TASK_EFFORT_START_FAILED", error),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn handle_coordination_apply_task_effort_status(
    id: &str,
    params: &serde_json::Value,
    service: &crate::daemon::effort_runs::EffortOperationsService,
) -> DaemonResponse {
    let params: protocol::CoordinationApplyTaskEffortStatusParams =
        match serde_json::from_value(params.clone()) {
            Ok(params) => params,
            Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
        };
    match service.status(&params.run_id) {
        Some(status) => DaemonResponse::ok(id, status),
        None => coordination_run_not_found(id, "task-effort", &params.run_id),
    }
}

#[cfg(feature = "mesh-bridged-backend")]
fn coordination_run_not_found(id: &str, operation: &str, run_id: &str) -> DaemonResponse {
    DaemonResponse::err(
        id,
        "COORDINATION_RUN_NOT_FOUND",
        format!("{operation} run '{run_id}' was not found"),
    )
}

/// Tool accounts on the daemon's host — the Windows app cannot read the WSL
/// homes itself.
fn handle_list_accounts(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::ListAccountsParams = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    let mut accounts = crate::session_scanner::accounts::detect(params.tool);
    crate::daemon::usage_poller::attach_usage(params.tool, &mut accounts);
    DaemonResponse::ok(
        id,
        protocol::AccountsResult {
            accounts,
            degraded: false,
            error: None,
        },
    )
}

/// What the pane shell on this host makes of a base command. The Windows app
/// cannot read the WSL shell's aliases itself.
fn handle_resolve_launch_base(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::ResolveLaunchBaseParams = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    if params.force {
        crate::session_scanner::launch_base::invalidate_base_command_cache();
    }
    let probe = crate::session_scanner::launch_base::ShellAliasProbe::for_pane();
    DaemonResponse::ok(
        id,
        crate::session_scanner::launch_base::resolve_base_command_cached(
            &params.base,
            params.tool,
            &probe,
        ),
    )
}

fn handle_refresh_usage(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::ListAccountsParams = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    DaemonResponse::ok(
        id,
        serde_json::json!({"started": crate::daemon::usage_poller::refresh(params.tool)}),
    )
}

/// The newest transcript a project has under any detected account dir.
fn handle_project_transcript(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::ProjectTranscriptParams = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };

    let config_dirs = crate::session_scanner::accounts::transcript_dirs(params.tool);
    DaemonResponse::ok(
        id,
        protocol::ProjectTranscriptResult {
            transcript: crate::session_scanner::accounts::newest_project_transcript(
                params.tool,
                &config_dirs,
                &params.project,
            )
            .map(|path| path.display().to_string()),
        },
    )
}

fn handle_list_workflow_runs(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::WorkflowSessionParams = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match crate::workflow_runs::list_runs_for_session_id(&params.session_id) {
        Ok(runs) => DaemonResponse::ok(id, runs),
        Err(error) => DaemonResponse::err(id, "WORKFLOW_RUN_ERROR", error),
    }
}

fn handle_get_workflow_run(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::WorkflowRunParams = match serde_json::from_value(params.clone()) {
        Ok(params) => params,
        Err(error) => return DaemonResponse::err(id, "INVALID_PARAMS", error.to_string()),
    };
    match crate::workflow_runs::get_run_for_session_id(&params.session_id, &params.run_id) {
        Ok(run) => DaemonResponse::ok(id, run),
        Err(error) => DaemonResponse::err(id, "WORKFLOW_RUN_ERROR", error),
    }
}

pub(crate) fn handle_ping(id: &str, start_time: Instant) -> DaemonResponse {
    let data_root = crate::daemon_api::data_identity_paths();
    DaemonResponse::ok(
        id,
        protocol::PingResult {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: protocol::PROTOCOL_VERSION,
            uptime_secs: start_time.elapsed().as_secs(),
            data_root: data_root.display().to_string(),
        },
    )
}

pub(crate) fn handle_git_status(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.git_status(&params.path) {
        Ok(status) => DaemonResponse::ok(id, status),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_log(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitLogParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.all_commits(&params.path, params.limit, params.offset) {
        Ok(commits) => DaemonResponse::ok(id, commits),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_latest_commit_time(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.latest_commit_time(&params.path) {
        Ok(time) => DaemonResponse::ok(
            id,
            protocol::LatestCommitTimeResult {
                timestamp: time.map(|t| t.to_rfc3339()),
            },
        ),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_commits_in_range(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitCommitsInRangeParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.commits_in_range(
        &params.path,
        &params.after,
        &params.before,
        params.commit_limit,
    ) {
        Ok(result) => DaemonResponse::ok(
            id,
            protocol::GitCommitsInRangeResult {
                commits: result.commits,
                files: result.files,
                truncated: result.truncated,
                total_count: result.total_count,
            },
        ),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_commit_files(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitCommitFilesParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.commit_files(&params.path, &params.hash) {
        Ok(files) => DaemonResponse::ok(id, protocol::GitCommitFilesResult { files }),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_git_commit_diff(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::GitCommitDiffParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.commit_diff(&params.path, &params.hash, &params.file_path) {
        Ok(hunks) => DaemonResponse::ok(id, protocol::GitCommitDiffResult { hunks }),
        Err(e) => DaemonResponse::err(id, "GIT_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_file_tree(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.file_tree(&params.path) {
        Ok(tree) => DaemonResponse::ok(id, tree),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_read_file(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::ReadFileParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_file(&params.path, &params.relative) {
        Ok(content) => DaemonResponse::ok(id, content),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_read_readme(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_readme(&params.path) {
        Ok(content) => DaemonResponse::ok(id, content),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_read_asset(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::ReadFileParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.read_asset(&params.path, &params.relative) {
        Ok(bytes) => {
            let data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
            DaemonResponse::ok(id, protocol::ReadAssetResult { data })
        }
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_scan_sessions(
    id: &str,
    params: &serde_json::Value,
    provider: &dyn ProjectProvider,
) -> DaemonResponse {
    let params: protocol::PathParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match provider.scan_session_files(&params.path) {
        Ok(paths) => DaemonResponse::ok(
            id,
            protocol::ScanSessionsResult {
                paths: paths
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect(),
            },
        ),
        Err(e) => DaemonResponse::err(id, "FS_ERROR", e.to_string()),
    }
}

pub(crate) fn handle_list_display_sessions(id: &str) -> DaemonResponse {
    let sessions = crate::daemon::session_activity::SessionActivityHub::global()
        .snapshot()
        .sessions;
    DaemonResponse::ok(id, sessions)
}

pub(crate) fn handle_get_runtime_session_snapshot(id: &str) -> DaemonResponse {
    let snapshot = crate::daemon::session_activity::SessionActivityHub::global().runtime_snapshot();
    DaemonResponse::ok(
        id,
        protocol::RuntimeSessionSnapshotResult {
            version: snapshot.version,
            display_sessions: snapshot.display_sessions,
            runtime_sessions: snapshot.runtime_sessions,
            account_observations: snapshot.account_observations,
            focus: snapshot.focus,
            foreground_project_path: snapshot.focus_project_path,
            degraded: snapshot.degraded,
            degraded_revision: snapshot.degraded_revision,
        },
    )
}

pub(crate) fn handle_list_runtime_sessions(id: &str) -> DaemonResponse {
    let sessions = crate::daemon::session_activity::SessionActivityHub::global()
        .runtime_snapshot()
        .runtime_sessions;
    DaemonResponse::ok(id, sessions)
}

pub(crate) fn handle_wait_session_updates(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::WaitSessionUpdatesParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };

    let update = crate::daemon::session_activity::SessionActivityHub::global().wait_for_update(
        params.since_version,
        params.since_degraded_revision,
        std::time::Duration::from_millis(params.timeout_ms),
    );

    DaemonResponse::ok(
        id,
        protocol::WaitSessionUpdatesResult {
            version: update.snapshot.version,
            changed: update.changed,
            sessions: update.snapshot.sessions,
            account_observations: update.account_observations,
            focus: update.focus,
            focus_project_path: update.focus_project_path,
            degraded: update.degraded,
            degraded_revision: update.degraded_revision,
        },
    )
}

pub(crate) fn handle_launch_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::LaunchSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::launch_in_tmux_with_layout(
        &params.project_path,
        params.mode,
        params.cli_tool,
        &params.tmux_layout,
        params.command_override.as_deref(),
    ) {
        Ok((session, window, pane)) => DaemonResponse::ok(
            id,
            protocol::LaunchSessionResult {
                tmux_session: Some(session),
                tmux_window: window,
                tmux_pane: pane,
                ..Default::default()
            },
        ),
        Err(e) => DaemonResponse::err(id, "LAUNCH_ERROR", e),
    }
}

pub(crate) fn handle_stop_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::StopSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::stop_session(&params.tmux_pane, params.cli_tool) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, "STOP_ERROR", e),
    }
}

pub(crate) fn handle_navigate_to_session(id: &str, params: &serde_json::Value) -> DaemonResponse {
    let params: protocol::NavigateToSessionParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
    };
    match crate::session_scanner::control::navigate_to_pane(
        &params.tmux_session,
        &params.tmux_window,
        &params.tmux_pane,
    ) {
        Ok(()) => DaemonResponse::ok(id, serde_json::json!({"ok": true})),
        Err(e) => DaemonResponse::err(id, "NAVIGATE_ERROR", e),
    }
}

pub(crate) fn handle_get_project_tasks(
    id: &str,
    params: &serde_json::Value,
    project_task_scan_cache: &ProjectTaskScanCacheState,
) -> DaemonResponse {
    let params: protocol::ProjectTasksParams = match serde_json::from_value(params.clone()) {
        Ok(p) => p,
        Err(_) => match serde_json::from_value::<protocol::PathParams>(params.clone()) {
            Ok(p) => protocol::ProjectTasksParams {
                path: p.path,
                scan_cycle_id: None,
            },
            Err(e) => return DaemonResponse::err(id, "INVALID_PARAMS", e.to_string()),
        },
    };

    let (all_sessions, claude_index) =
        load_project_task_scan_inputs(params.scan_cycle_id, project_task_scan_cache);
    let project_sessions: Vec<crate::session_scanner::RuntimeSession> = all_sessions
        .into_iter()
        .filter(|s| s.project_path == params.path)
        .collect();

    let result = crate::task_scanner::get_tasks_for_project_with_index(
        &params.path,
        &project_sessions,
        Some(&claude_index),
    );
    #[cfg(feature = "mesh-bridged-backend")]
    record_terminal_task_observations(&result, &claude_index, project_task_scan_cache);
    DaemonResponse::ok(id, result)
}

#[cfg(feature = "mesh-bridged-backend")]
fn record_terminal_task_observations(
    result: &crate::task_scanner::TaskResult,
    index: &ClaudeSourceIndex,
    state: &ProjectTaskScanCacheState,
) -> usize {
    let mut recorded = 0;
    for task in result.tasks.iter().filter(|task| {
        matches!(
            task.status,
            crate::task_scanner::TaskStatus::Completed | crate::task_scanner::TaskStatus::Stale
        )
    }) {
        let Some(teams_dir) = index.team_teams_dir(&task.source_key) else {
            continue;
        };
        let key = (
            teams_dir.clone(),
            task.source_key.clone(),
            task.id.clone(),
            task.status.to_string(),
            task.has_review_ruling,
        );
        if !state
            .terminal_observations
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(key)
        {
            continue;
        }
        crate::coordination::stores::telemetry::record_completion_observed(
            &teams_dir,
            &task.source_key,
            &task.id,
            &task.status.to_string(),
            task.has_review_ruling,
        );
        recorded += 1;
    }
    recorded
}

fn load_project_task_scan_inputs(
    cycle_id: Option<u64>,
    project_task_scan_cache: &ProjectTaskScanCacheState,
) -> (
    Vec<crate::session_scanner::RuntimeSession>,
    ClaudeSourceIndex,
) {
    if let Some(cycle_id) = cycle_id {
        let mut guard = project_task_scan_cache
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(ref cached) = *guard {
            if cached.cycle_id == cycle_id {
                return (cached.sessions.clone(), cached.claude_index.clone());
            }
        }

        // Continuity read: task-source lookup only (see bootstrap.rs); a
        // degraded scan keeps the last good snapshot, nothing is bound to it.
        let (sessions, _degraded) = crate::session_scanner::scan_sessions_for_runtime();
        let claude_index = build_claude_source_index_with_live_sessions(&sessions);
        *guard = Some(ProjectTaskScanCache {
            cycle_id,
            sessions: sessions.clone(),
            claude_index: claude_index.clone(),
        });
        return (sessions, claude_index);
    }

    // Continuity read: same task-source lookup as above, uncached.
    let (sessions, _degraded) = crate::session_scanner::scan_sessions_for_runtime();
    let claude_index = build_claude_source_index_with_live_sessions(&sessions);
    (sessions, claude_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::accounts::{install_detection_override, AccountScan};
    use crate::session_scanner::cli_tool::CliTool;
    use tempfile::TempDir;

    // Regression: 13111833 repeated one lock-and-full-read per terminal task
    // on every task RPC, even after that exact terminal state was observed.
    #[test]
    fn terminal_task_observation_is_process_deduplicated() {
        use std::collections::BTreeSet;

        use crate::task_scanner::claude_index::ClaudeTaskRoot;

        let root = TempDir::new().expect("root");
        let account_dir = root.path().join("account");
        let teams_dir = account_dir.join("teams");
        crate::coordination::stores::telemetry::record_launch_rendered(
            &teams_dir,
            "routing-team",
            Some("42"),
            "builder",
            "rust-developer",
            CliTool::Codex,
            Some("gpt-5.6-sol"),
            Some("high"),
        );
        let index = ClaudeSourceIndex {
            task_roots: vec![ClaudeTaskRoot {
                path: account_dir.join("tasks"),
                authoritative_teams: BTreeSet::from(["routing-team".to_string()]),
            }],
            ..Default::default()
        };
        let result = crate::task_scanner::TaskResult {
            tasks: vec![crate::task_scanner::UnifiedTask {
                id: "42".to_string(),
                source_key: "routing-team".to_string(),
                subject: "Finish telemetry".to_string(),
                description: None,
                active_form: None,
                status: crate::task_scanner::TaskStatus::Completed,
                source: CliTool::Claude,
                blocks: Vec::new(),
                blocked_by: Vec::new(),
                owner: Some("builder".to_string()),
                session_id: Some("routing-team".to_string()),
                state_changed_at: None,
                updated_at: None,
                archived_at: None,
                last_status: None,
                archived_reason: None,
                effort: None,
                effort_why: None,
                deadline_minutes: None,
                has_review_ruling: true,
            }],
            errors: Vec::new(),
            source_outcomes: Vec::new(),
        };
        let state = ProjectTaskScanCacheState::default();

        assert_eq!(
            record_terminal_task_observations(&result, &index, &state),
            1
        );
        assert_eq!(
            record_terminal_task_observations(&result, &index, &state),
            0
        );
        let path = teams_dir.join("routing-team/state/telemetry/42.jsonl");
        assert_eq!(
            crate::coordination::stores::telemetry::read_task_telemetry(&path).len(),
            2,
            "one render plus one terminal observation"
        );
    }

    // Regression: 760f776 answered `claude-project-transcript` from the config
    // dirs of successfully parsed accounts only. On Windows this handler is the
    // *only* thing that can see the transcripts, and a `.claude.json` caught
    // mid-rewrite names no account — so the daemon reported no history and the
    // app resumed the project in whichever subscription its own choice named.
    #[test]
    fn the_transcript_handler_reads_config_dirs_that_name_no_account() {
        let home = TempDir::new().expect("home");
        let config_dir = home.path().join(".claude-account2");
        let project_path = "/home/user/projects/daemon-side";
        let dir = config_dir
            .join("projects")
            .join(crate::session_scanner::idle::path_to_slug(project_path));
        std::fs::create_dir_all(&dir).expect("transcript dir");
        let transcript = dir.join("abc.jsonl");
        std::fs::write(&transcript, "{}\n").expect("transcript");
        let _scan = install_detection_override(
            CliTool::Claude,
            AccountScan {
                config_dirs: vec![config_dir],
                accounts: Vec::new(),
            },
        );

        let response = handle_project_transcript(
            "req-1",
            &serde_json::json!({ "tool": "claude", "project": project_path }),
        );

        assert!(response.is_ok(), "{response:?}");
        let result: protocol::ProjectTranscriptResult =
            serde_json::from_value(response.result.expect("result")).expect("decode");
        assert_eq!(
            result.transcript.as_deref(),
            Some(transcript.display().to_string().as_str())
        );
    }

    #[test]
    fn workflow_handlers_scan_the_daemon_hosts_scratch_session() {
        let config = TempDir::new().expect("config");
        let session_dir = config.path().join("projects/project/session-123");
        let run_id = "wf_daemon-123";
        std::fs::create_dir_all(session_dir.join("subagents/workflows").join(run_id))
            .expect("run dir");
        std::fs::create_dir_all(session_dir.join("workflows/scripts")).expect("scripts dir");
        std::fs::write(
            session_dir
                .join("workflows/scripts")
                .join(format!("daemon-{run_id}.js")),
            "export const meta = { name: 'daemon', description: 'daemon fixture', phases: [{ title: 'Run' }] }\n",
        )
        .expect("script");
        std::fs::write(
            session_dir
                .join("subagents/workflows")
                .join(run_id)
                .join("journal.jsonl"),
            "",
        )
        .expect("journal");
        let workflow_tool = crate::session_scanner::cli_tool::all()
            .iter()
            .find(|entry| entry.capabilities.workflow_runs)
            .expect("workflow tool")
            .tool;
        let _scan = install_detection_override(
            workflow_tool,
            AccountScan {
                config_dirs: vec![config.path().to_path_buf()],
                accounts: Vec::new(),
            },
        );

        let listed =
            handle_list_workflow_runs("req-list", &serde_json::json!({"session_id":"session-123"}));
        assert!(listed.is_ok(), "{listed:?}");
        let summaries: Vec<crate::workflow_runs::WorkflowRunSummary> =
            serde_json::from_value(listed.result.expect("list result")).expect("decode list");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].run_id, run_id);

        let fetched = handle_get_workflow_run(
            "req-get",
            &serde_json::json!({"session_id":"session-123","run_id":run_id}),
        );
        assert!(fetched.is_ok(), "{fetched:?}");
        let run: crate::workflow_runs::WorkflowRun =
            serde_json::from_value(fetched.result.expect("get result")).expect("decode run");
        assert_eq!(run.name, "daemon");
        assert_eq!(run.status, crate::workflow_runs::WorkflowRunStatus::Live);
    }

    #[test]
    fn put_launch_settings_handler_acknowledges_only_monotonic_snapshots() {
        let store = crate::daemon::background_scheduler::LaunchSettingsStore::default();
        let mut commands = crate::models::CliCommandSettings::default();
        commands.claude.resume = "claude2 --resume".to_string();

        let accepted = handle_coordination_put_launch_settings(
            "settings-1",
            &serde_json::json!({
                "version": 4,
                "cli_commands": commands,
                "tmux_layout": "new_window"
            }),
            &store,
        );
        let stale = handle_coordination_put_launch_settings(
            "settings-2",
            &serde_json::json!({
                "version": 3,
                "cli_commands": crate::models::CliCommandSettings::default(),
                "tmux_layout": "split"
            }),
            &store,
        );

        let accepted: protocol::CoordinationPutLaunchSettingsResult =
            serde_json::from_value(accepted.result.expect("accepted result")).expect("decode");
        let stale: protocol::CoordinationPutLaunchSettingsResult =
            serde_json::from_value(stale.result.expect("stale result")).expect("decode");
        assert!(accepted.accepted);
        assert!(!stale.accepted);
        assert_eq!(stale.version, 4);
    }
}
