use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value};

use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    AddAgentRequest, InitializeTeamRequest, ResumeMemberRequest, ResumeTeamRequest, TeardownMode,
    TeardownRequest,
};
use crate::coordination::runtime::{
    emit_foreign_pane_event, pane_belongs_to_member, PaneOwnership,
};
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};

use super::{CoordinationOrchestrator, RemoveMemberStepResult};

static TEAM_DAEMON_SKIP_EVENTS: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
const MISSING_LEAD_CREDENTIAL_REASON: &str = "missing_lead_control_credential";
const MISSING_LEAD_CONFIG_HASH_REASON: &str = "missing_lead_control_auth_token_hash";
const INACTIVE_LEAD_REASON: &str = "inactive_lead_control_identity";

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
        match self.quarantine_foreign_pane_before_team_daemon(team_name) {
            Ok(Some(reason)) => {
                tracing::warn!(
                    team = %team_name,
                    reason,
                    "skipping team daemon restart because a member pane is foreign"
                );
                return false;
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    error = %err,
                    "failed to verify pane ownership before team daemon restart"
                );
                return false;
            }
        }
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
        let skip_reason = match self.team_daemon_skip_reason(team_name, &operator_name) {
            Ok(reason) => reason,
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    operator = %operator_name,
                    error = %err,
                    "failed to verify team daemon authentication state"
                );
                return false;
            }
        };
        if let Some(reason) = skip_reason {
            self.emit_team_daemon_skipped_once(team_name, &operator_name, reason);
            return false;
        }
        self.clear_team_daemon_skip_state(team_name, &operator_name);
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
        if let Some(reason) = self.quarantine_foreign_pane_before_team_daemon(team_name)? {
            return Ok((
                false,
                Some(format!(
                    "team daemon skipped because a member pane is foreign: {reason}"
                )),
            ));
        }
        let operator_name = self.team_daemon_operator_name(team_name)?;
        if let Some(reason) = self.team_daemon_skip_reason(team_name, &operator_name)? {
            self.emit_team_daemon_skipped_once(team_name, &operator_name, reason);
            let detail = match reason {
                MISSING_LEAD_CREDENTIAL_REASON => {
                    format!("lead control credential is missing for '{operator_name}'")
                }
                MISSING_LEAD_CONFIG_HASH_REASON => {
                    format!("lead controlAuthTokenHash is missing for '{operator_name}'")
                }
                INACTIVE_LEAD_REASON => {
                    format!("lead control identity is inactive for '{operator_name}'")
                }
                _ => format!("lead authentication is unavailable for '{operator_name}'"),
            };
            return Ok((false, Some(format!("team daemon skipped: {detail}"))));
        }
        self.clear_team_daemon_skip_state(team_name, &operator_name);
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

    fn quarantine_foreign_pane_before_team_daemon(
        &self,
        team_name: &str,
    ) -> Result<Option<String>, CoordinationError> {
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let members = config
            .members
            .into_iter()
            .map(|member| (member.name.clone(), member))
            .collect::<HashMap<_, _>>();
        let mut first_foreign = None;
        for (member_name, mut record) in MemberRuntimeStore::load_all(&self.teams_dir, team_name)? {
            let Some(member) = members.get(&member_name) else {
                continue;
            };
            let Some(pane_id) = record.pane_id.clone() else {
                continue;
            };
            let Some(live_pane) = self.runtime.live_pane(&pane_id)? else {
                continue;
            };
            if live_pane.is_dead {
                continue;
            }
            record.cli_tool.get_or_insert(member.cli_tool);
            record
                .project_path
                .get_or_insert_with(|| member.project_path.clone());
            let PaneOwnership::Foreign { reason } = pane_belongs_to_member(&record, &live_pane)
            else {
                continue;
            };

            let should_emit =
                record.health != HealthState::SessionDead || record.daemon_pid.is_some();
            if let Some(pid) = record.daemon_pid {
                if self.runtime.is_process_running_by_pid(pid).unwrap_or(false) {
                    if let Err(err) = self.runtime.terminate_process_by_pid(pid) {
                        tracing::warn!(
                            team = %team_name,
                            member = %member_name,
                            pid,
                            error = %err,
                            "failed to terminate foreign-pane daemon before team daemon restart"
                        );
                    }
                }
            }
            record.health = HealthState::SessionDead;
            record.session_id = None;
            record.jsonl_path = None;
            record.daemon_pid = None;
            MemberRuntimeStore::save(&self.teams_dir, team_name, &member_name, &record)?;
            if let Err(err) = self
                .runtime
                .clear_mesh_daemon_pid_file(team_name, &member_name)
            {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    error = %err,
                    "failed to clear foreign-pane daemon pid file before team daemon restart"
                );
            }
            if should_emit {
                emit_foreign_pane_event(team_name, &member_name, &pane_id, &reason);
            }
            first_foreign.get_or_insert_with(|| format!("{member_name}:{pane_id}:{reason}"));
        }
        Ok(first_foreign)
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

    fn team_daemon_skip_reason(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<Option<&'static str>, CoordinationError> {
        if !self
            .team_daemon_credential_path(team_name, operator_name)
            .is_file()
        {
            return Ok(Some(MISSING_LEAD_CREDENTIAL_REASON));
        }

        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let lead = config
            .members
            .iter()
            .find(|member| member.name == operator_name)
            .ok_or_else(|| {
                CoordinationError::Conflict(format!(
                    "team '{team_name}' has no configured lead member '{operator_name}'"
                ))
            })?;
        let has_hash = lead
            .extra
            .get("controlAuthTokenHash")
            .and_then(Value::as_str)
            .is_some_and(|hash| !hash.trim().is_empty());
        if !has_hash {
            return Ok(Some(MISSING_LEAD_CONFIG_HASH_REASON));
        }
        if lead.extra.get("isActive").and_then(Value::as_bool) == Some(false) {
            return Ok(Some(INACTIVE_LEAD_REASON));
        }
        Ok(None)
    }

    fn emit_team_daemon_skipped_once(
        &self,
        team_name: &str,
        operator_name: &str,
        reason: &'static str,
    ) {
        let credential_path = self.team_daemon_credential_path(team_name, operator_name);
        let emitted = TEAM_DAEMON_SKIP_EVENTS.get_or_init(|| Mutex::new(HashMap::new()));
        let should_emit = emitted
            .lock()
            .map(|mut paths| {
                if paths
                    .get(&credential_path)
                    .is_some_and(|previous| previous == reason)
                {
                    false
                } else {
                    paths.insert(credential_path.clone(), reason.to_string());
                    true
                }
            })
            .unwrap_or(true);
        if !should_emit {
            return;
        }

        tracing::info!(
            team = %team_name,
            operator = %operator_name,
            reason,
            credential_path = %credential_path.display(),
            "team daemon skipped because lead authentication is unavailable"
        );
        let mut fields = Map::new();
        fields.insert("team_name".into(), Value::String(team_name.to_string()));
        fields.insert(
            "operator_name".into(),
            Value::String(operator_name.to_string()),
        );
        fields.insert("reason".into(), Value::String(reason.to_string()));
        fields.insert(
            "credential_path".into(),
            Value::String(credential_path.display().to_string()),
        );
        taurhaus_lib::logging::emit_global(
            "info",
            "coordination",
            "coordination.team_daemon.skipped",
            Some("Team daemon skipped because lead authentication is unavailable".to_string()),
            fields,
        );
    }

    fn clear_team_daemon_skip_state(&self, team_name: &str, operator_name: &str) {
        let credential_path = self.team_daemon_credential_path(team_name, operator_name);
        if let Some(emitted) = TEAM_DAEMON_SKIP_EVENTS.get() {
            if let Ok(mut paths) = emitted.lock() {
                paths.remove(&credential_path);
            }
        }
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
