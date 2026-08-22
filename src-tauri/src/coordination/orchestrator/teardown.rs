use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value};

use crate::coordination::domain::MemberRole;
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    AddAgentRequest, InitializeTeamRequest, ResumeMemberRequest, ResumeTeamRequest, TeardownMode,
    TeardownRequest,
};
use crate::coordination::stores::{MemberRuntimeRecord, TeamConfigStore};

use super::{CoordinationOrchestrator, RemoveMemberStepResult};

static TEAM_DAEMON_SKIP_EVENTS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
const MISSING_LEAD_CREDENTIAL_REASON: &str = "missing_lead_control_credential";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct TeardownDiagnostics {
    pub(super) steps: Vec<RemoveMemberStepResult>,
    pub(super) warnings: Vec<String>,
}

impl CoordinationOrchestrator {
    pub(super) fn teardown_member_resources_best_effort(
        &self,
        team_name: &str,
        member_name: &str,
        member_project_path: Option<&Path>,
        runtime: Option<&MemberRuntimeRecord>,
    ) -> TeardownDiagnostics {
        let mut diagnostics = TeardownDiagnostics::default();
        let pane_id = runtime.and_then(|record| record.pane_id.as_deref());

        let mut daemon_pids = Vec::new();
        if let Some(pid) = runtime.and_then(|record| record.daemon_pid) {
            daemon_pids.push(pid);
        }
        match self
            .runtime
            .find_existing_mesh_daemon_pid_by_member(team_name, member_name)
        {
            Ok(Some(pid)) => daemon_pids.push(pid),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    error = %err,
                    "failed to discover mesh daemon from member pid file during teardown"
                );
                diagnostics.steps.push(step_failed(
                    "discover_daemon_pidfile",
                    format!("failed to discover daemon pid file state: {err}"),
                ));
                diagnostics
                    .warnings
                    .push(format!("failed to discover daemon pid file state: {err}"));
            }
        }
        if let Some(pane_id) = pane_id {
            match self
                .runtime
                .find_existing_mesh_daemon_pids(pane_id, team_name, member_name)
            {
                Ok(found_pids) => daemon_pids.extend(found_pids),
                Err(err) => {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pane_id = %pane_id,
                        error = %err,
                        "failed to discover mesh daemons during teardown"
                    );
                    diagnostics.steps.push(step_failed(
                        "discover_daemon",
                        format!("failed to discover daemon state for pane {pane_id}: {err}"),
                    ));
                    diagnostics.warnings.push(format!(
                        "failed to discover daemon state for pane {pane_id}: {err}"
                    ));
                }
            }
        }
        daemon_pids.sort_unstable();
        daemon_pids.dedup();

        if daemon_pids.is_empty() {
            diagnostics
                .steps
                .push(step_succeeded("terminate_daemon", "no daemon pid recorded"));
        } else {
            let mut terminated = Vec::new();
            for pid in daemon_pids {
                if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                    tracing::warn!(
                        team = %team_name,
                        member = %member_name,
                        pid = pid,
                        error = %err,
                        "failed to terminate daemon during teardown"
                    );
                    diagnostics.steps.push(step_failed(
                        "terminate_daemon",
                        format!("failed to terminate daemon pid {pid}: {err}"),
                    ));
                    diagnostics
                        .warnings
                        .push(format!("failed to terminate daemon pid {pid}: {err}"));
                } else {
                    terminated.push(pid);
                }
            }

            if !terminated.is_empty() {
                diagnostics.steps.push(step_succeeded(
                    "terminate_daemon",
                    format!(
                        "terminated daemon pid{} {}",
                        if terminated.len() == 1 { "" } else { "s" },
                        terminated
                            .iter()
                            .map(u32::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        }

        if let Err(err) = self
            .runtime
            .clear_mesh_daemon_pid_file(team_name, member_name)
        {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                error = %err,
                "failed to clear daemon pid file during teardown"
            );
            diagnostics.steps.push(step_failed(
                "clear_daemon_pid_file",
                format!("failed to clear daemon pid file: {err}"),
            ));
            diagnostics
                .warnings
                .push(format!("failed to clear daemon pid file: {err}"));
        } else {
            diagnostics.steps.push(step_succeeded(
                "clear_daemon_pid_file",
                "daemon pid file cleared",
            ));
        }

        if let Err(err) = self.backend.teardown(TeardownRequest {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            mode: TeardownMode::Graceful,
        }) {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                error = %err,
                "failed to leave mesh during teardown"
            );
            diagnostics.steps.push(step_failed(
                "leave_mesh",
                format!("failed to leave mesh: {err}"),
            ));
            diagnostics
                .warnings
                .push(format!("failed to leave mesh membership: {err}"));
        } else {
            diagnostics
                .steps
                .push(step_succeeded("leave_mesh", "mesh presence removed"));
        }

        if let Some(pane_id) = pane_id {
            match member_project_path {
                Some(project_path) => {
                    let project_path = project_path.display().to_string();
                    match self
                        .runtime
                        .pane_belongs_to_project(pane_id, project_path.as_str())
                    {
                        Ok(true) => {
                            diagnostics.steps.push(step_succeeded(
                                "verify_pane_ownership",
                                format!("pane {pane_id} matched project {project_path}"),
                            ));
                            if let Err(err) = self.runtime.kill_aitx_pane(pane_id) {
                                tracing::warn!(
                                    team = %team_name,
                                    member = %member_name,
                                    pane_id = %pane_id,
                                    error = %err,
                                    "failed to kill pane during teardown"
                                );
                                diagnostics.steps.push(step_failed(
                                    "kill_pane",
                                    format!("failed to kill pane {pane_id}: {err}"),
                                ));
                                diagnostics
                                    .warnings
                                    .push(format!("failed to kill pane {pane_id}: {err}"));
                            } else {
                                diagnostics.steps.push(step_succeeded(
                                    "kill_pane",
                                    format!("pane {pane_id} terminated"),
                                ));
                            }
                        }
                        Ok(false) => {
                            diagnostics.steps.push(step_failed(
                                "verify_pane_ownership",
                                format!(
                                    "pane {pane_id} did not match expected project {project_path}"
                                ),
                            ));
                            diagnostics.warnings.push(format!(
                                "skipped pane teardown for {pane_id}: ownership mismatch for {project_path}"
                            ));
                            diagnostics.steps.push(step_failed(
                                "kill_pane",
                                format!(
                                    "skipped pane kill for {pane_id} due to ownership mismatch"
                                ),
                            ));
                        }
                        Err(err) => {
                            tracing::warn!(
                                team = %team_name,
                                member = %member_name,
                                pane_id = %pane_id,
                                error = %err,
                                "failed to verify pane ownership during teardown"
                            );
                            diagnostics.steps.push(step_failed(
                                "verify_pane_ownership",
                                format!("failed to verify pane ownership for {pane_id}: {err}"),
                            ));
                            diagnostics.warnings.push(format!(
                                "skipped pane teardown for {pane_id}: ownership check failed ({err})"
                            ));
                            diagnostics.steps.push(step_failed(
                                "kill_pane",
                                format!(
                                    "skipped pane kill for {pane_id} because ownership check failed"
                                ),
                            ));
                        }
                    }
                }
                None => {
                    diagnostics.steps.push(step_failed(
                        "verify_pane_ownership",
                        format!("no project path recorded for member '{member_name}'"),
                    ));
                    diagnostics.warnings.push(format!(
                        "skipped pane teardown for {pane_id}: missing project path for ownership check"
                    ));
                    diagnostics.steps.push(step_failed(
                        "kill_pane",
                        format!("skipped pane kill for {pane_id} because project path is missing"),
                    ));
                }
            }
        } else {
            diagnostics.steps.push(step_succeeded(
                "verify_pane_ownership",
                "no pane id recorded",
            ));
            diagnostics
                .steps
                .push(step_succeeded("kill_pane", "no pane id recorded"));
        }

        diagnostics
    }

    pub(crate) fn ensure_team_daemon_running_best_effort(&self, team_name: &str) -> bool {
        let operator_name = match self.team_daemon_operator_name(team_name) {
            Ok(operator_name) => operator_name,
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    error = %err,
                    "failed to resolve lead identity for team daemon startup"
                );
                return false;
            }
        };
        if !self.team_daemon_credential_exists(team_name, &operator_name) {
            self.emit_team_daemon_skipped_once(team_name, &operator_name);
            return false;
        }
        match self.runtime.spawn_team_daemon(team_name, &operator_name) {
            Ok(pid) => {
                tracing::info!(
                    team = %team_name,
                    operator = %operator_name,
                    pid = pid,
                    "team daemon ensured running"
                );
                true
            }
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    operator = %operator_name,
                    error = %err,
                    "failed to ensure team daemon is running"
                );
                false
            }
        }
    }

    pub(crate) fn stop_team_daemon_best_effort(&self, team_name: &str) {
        if let Err(err) = self.runtime.stop_team_daemon(team_name) {
            tracing::warn!(
                team = %team_name,
                error = %err,
                "failed to stop team daemon during teardown"
            );
        }
    }

    pub(crate) fn ensure_team_daemon_for_wrapper(
        &self,
        team_name: &str,
    ) -> Result<(bool, Option<String>), CoordinationError> {
        let operator_name = self.team_daemon_operator_name(team_name)?;
        if !self.team_daemon_credential_exists(team_name, &operator_name) {
            self.emit_team_daemon_skipped_once(team_name, &operator_name);
            return Ok((
                false,
                Some(format!(
                    "team daemon skipped: lead control credential is missing for '{operator_name}'"
                )),
            ));
        }
        match self.runtime.spawn_team_daemon(team_name, &operator_name) {
            Ok(pid) => {
                tracing::info!(
                    team = %team_name,
                    operator = %operator_name,
                    pid = pid,
                    "team daemon ensured running after coordination wrapper"
                );
                Ok((true, None))
            }
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    operator = %operator_name,
                    error = %err,
                    "failed to ensure team daemon is running after coordination wrapper"
                );
                Ok((false, Some(err.to_string())))
            }
        }
    }

    pub(crate) fn ensure_team_daemon_for_wrapper_best_effort(&self, team_name: &str) {
        if let Err(err) = self.ensure_team_daemon_for_wrapper(team_name) {
            tracing::warn!(
                team = %team_name,
                error = %err,
                "failed to resolve team daemon operator after coordination wrapper"
            );
        }
    }

    pub(crate) fn ensure_team_daemon_after_initialize(&self, request: &InitializeTeamRequest) {
        self.ensure_team_daemon_for_wrapper_best_effort(&request.team_name);
    }

    pub(crate) fn ensure_team_daemon_after_add_agent(&self, request: &AddAgentRequest) {
        self.ensure_team_daemon_for_wrapper_best_effort(&request.team_name);
    }

    pub(crate) fn ensure_team_daemon_after_resume_member(&self, request: &ResumeMemberRequest) {
        self.ensure_team_daemon_for_wrapper_best_effort(&request.team_name);
    }

    pub(crate) fn ensure_team_daemon_after_resume_team(
        &self,
        request: &ResumeTeamRequest,
    ) -> Result<(bool, Option<String>), CoordinationError> {
        self.ensure_team_daemon_for_wrapper(&request.team_name)
    }

    pub(crate) fn team_daemon_operator_name(
        &self,
        team_name: &str,
    ) -> Result<String, CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        config
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .ok_or_else(|| {
                CoordinationError::Conflict(format!(
                    "team '{team_name}' has no configured lead for team-daemon control"
                ))
            })
    }

    fn team_daemon_credential_path(&self, team_name: &str, operator_name: &str) -> PathBuf {
        self.teams_dir
            .join(team_name)
            .join("state")
            .join("control_auth")
            .join(format!("{operator_name}.json"))
    }

    fn team_daemon_credential_exists(&self, team_name: &str, operator_name: &str) -> bool {
        self.team_daemon_credential_path(team_name, operator_name)
            .is_file()
    }

    fn emit_team_daemon_skipped_once(&self, team_name: &str, operator_name: &str) {
        let credential_path = self.team_daemon_credential_path(team_name, operator_name);
        let emitted = TEAM_DAEMON_SKIP_EVENTS.get_or_init(|| Mutex::new(HashSet::new()));
        let should_emit = emitted
            .lock()
            .map(|mut paths| paths.insert(credential_path.clone()))
            .unwrap_or(true);
        if !should_emit {
            return;
        }

        tracing::info!(
            team = %team_name,
            operator = %operator_name,
            reason = MISSING_LEAD_CREDENTIAL_REASON,
            credential_path = %credential_path.display(),
            "team daemon skipped because the lead control credential is missing"
        );
        let mut fields = Map::new();
        fields.insert("team_name".into(), Value::String(team_name.to_string()));
        fields.insert(
            "operator_name".into(),
            Value::String(operator_name.to_string()),
        );
        fields.insert(
            "reason".into(),
            Value::String(MISSING_LEAD_CREDENTIAL_REASON.to_string()),
        );
        fields.insert(
            "credential_path".into(),
            Value::String(credential_path.display().to_string()),
        );
        taurhaus_lib::logging::emit_global(
            "info",
            "coordination",
            "coordination.team_daemon.skipped",
            Some("Team daemon skipped because the lead credential is missing".to_string()),
            fields,
        );
    }
}

pub(super) fn step_succeeded(step: &str, message: impl Into<String>) -> RemoveMemberStepResult {
    RemoveMemberStepResult {
        step: step.to_string(),
        success: true,
        message: Some(message.into()),
    }
}

pub(super) fn step_failed(step: &str, message: impl Into<String>) -> RemoveMemberStepResult {
    RemoveMemberStepResult {
        step: step.to_string(),
        success: false,
        message: Some(message.into()),
    }
}

pub(super) fn removal_actor_identity() -> String {
    std::env::var("TAURHAUS_OPERATOR")
        .ok()
        .and_then(non_empty_trimmed)
        .or_else(|| std::env::var("USER").ok().and_then(non_empty_trimmed))
        .or_else(|| std::env::var("USERNAME").ok().and_then(non_empty_trimmed))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn non_empty_trimmed(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn render_member_removed_notice(
    team_name: &str,
    removed_member: &str,
    removed_by: &str,
    cleanup_is_partial: bool,
    warning_count: usize,
) -> String {
    let cleanup = if cleanup_is_partial {
        format!(
            "partial ({warning_count} warning{})",
            if warning_count == 1 { "" } else { "s" }
        )
    } else {
        "complete".to_string()
    };

    format!(
        "[taurhaus] member_removed: '{removed_member}' was removed from team '{team_name}' by '{removed_by}'. Cleanup: {cleanup}."
    )
}
