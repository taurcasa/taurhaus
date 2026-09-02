//! Applying assignment effort to harnesses that must resume with a launch flag.
//!
//! This module owns the pending-effort state machine: target calculation,
//! command rewriting, stop-before-resume sequencing, bounded retries, and
//! durable failure recording. The member-activation pipeline only asks for the
//! effort of an open assignment when it launches a member.

use std::fs;
use std::path::{Path, PathBuf};

use crate::coordination::domain::{HealthState, Member};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::ResumeMemberRequest;
use crate::coordination::stores::{
    EffortResumeFailure, MemberRuntimeStore, OperationalContextSnapshotStore, TeamConfigStore,
};
use crate::coordination::task_effort;
use crate::coordination::validation::validate_team_name;
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

/// One member's pending effort switch.
pub(super) struct PendingEffort {
    /// The assignment whose notice mesh is holding, when its projections say.
    pub(super) task_id: String,
    pub(super) requested: String,
    pub(super) applied: Option<String>,
    /// Failures already spent on this level, from the runtime record.
    failed_attempts: u32,
}

/// Typed result of one or more assignment-effort applications.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffortPassOutcome {
    /// Members whose requested effort was put into force.
    pub switched: Vec<String>,
    /// Members whose switch was attempted or refused, with the reason.
    pub failed: Vec<(String, String)>,
    /// Teams that could not be inspected or processed, with the reason.
    pub skipped_teams: Vec<(String, String)>,
}

/// How often one requested level is retried before the pass leaves it alone.
///
/// A relaunch stops the member before it launches, so a level that keeps
/// failing must not stop it again on every pass. Any launch that commits clears
/// the count, so a member that comes back can reach the level later.
const MAX_EFFORT_RESUME_ATTEMPTS: u32 = 3;
const MAX_ASSIGNMENT_RECORD_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone)]
struct AssignmentTarget {
    task_id: String,
    level: String,
    blocked: bool,
}

pub(super) fn attempt_is_allowed(
    _scope: task_effort::EffortPassScope,
    failed_attempts: u32,
) -> bool {
    failed_attempts < MAX_EFFORT_RESUME_ATTEMPTS
}

/// The launch settings an effort relaunch renders from, or `None` when no
/// command it could render carries `level`.
///
/// A base that pins nothing is used as it stands — the renderer appends the
/// requested level itself. A base that pins one has that value replaced, and
/// the result is read back before the caller stops anything.
fn effort_launch_commands(
    cli_commands: &CliCommandSettings,
    tool: CliTool,
    level: &str,
) -> Option<CliCommandSettings> {
    let mode = crate::daemon::protocol::LaunchMode::Resume;
    let resolved_base = cli_commands.resolved_bases.get(&(tool, mode));
    let base = resolved_base
        .map(|base| base.command.as_str())
        .unwrap_or_else(|| crate::session_scanner::launch::base_command(cli_commands, tool, mode));
    if !task_effort::base_pins_effort(tool, base) {
        return Some(cli_commands.clone());
    }
    let rewritten = task_effort::base_with_effort(tool, base, level)?;
    if !task_effort::pinned_base_effort(tool, &rewritten)
        .is_some_and(|pinned| pinned.eq_ignore_ascii_case(level))
    {
        return None;
    }
    let mut commands = cli_commands.clone();
    match commands.resolved_bases.get_mut(&(tool, mode)) {
        Some(base) => base.command = rewritten,
        None => commands.get_mut(tool)?.resume = rewritten,
    }
    Some(commands)
}

/// Assignment target selection for a member with more than one open task.
///
/// mesh's attention projection is authoritative when it names a task whose
/// notice is still held for this member. Once that notice is delivered, an
/// open task selected in the operational snapshot keeps the level already
/// applied for it; the highest requested effort is only the tiebreak when no
/// open task owns the applied level. Only when mesh-owned task records are
/// unavailable does the snapshot provide the single-assignment compatibility
/// fallback. No inbox text or timestamp heuristic is used to invent a held
/// task.
fn assignment_target(
    orchestrator: &CoordinationOrchestrator,
    team_name: &str,
    member_name: &str,
    applied_effort: Option<&str>,
) -> Option<AssignmentTarget> {
    let assignments = open_mesh_assignments(&orchestrator.teams_dir, team_name, member_name);
    let has_non_blocked = assignments.iter().any(|assignment| !assignment.blocked);
    let assignments: Vec<_> = assignments
        .into_iter()
        .filter(|assignment| !has_non_blocked || !assignment.blocked)
        .collect();
    if let Some(held_task_id) = held_task_id(&orchestrator.teams_dir, team_name, member_name) {
        if let Some(held) = assignments
            .iter()
            .find(|assignment| assignment.task_id == held_task_id)
        {
            return Some(held.clone());
        }
    }
    if let Some(applied_effort) = applied_effort {
        let current_task_id =
            OperationalContextSnapshotStore::load(&orchestrator.teams_dir, team_name, member_name)
                .ok()
                .flatten()
                .map(|snapshot| snapshot.task.id);
        if let Some(current_task_id) = current_task_id {
            if let Some(current) = assignments.iter().find(|assignment| {
                assignment.task_id == current_task_id
                    && assignment.level.eq_ignore_ascii_case(applied_effort)
            }) {
                return Some(current.clone());
            }
        }
    }
    if let Some(highest) = assignments.into_iter().max_by(|left, right| {
        effort_rank(&left.level)
            .cmp(&effort_rank(&right.level))
            .then_with(|| left.task_id.cmp(&right.task_id))
    }) {
        return Some(highest);
    }

    let snapshot =
        OperationalContextSnapshotStore::load(&orchestrator.teams_dir, team_name, member_name)
            .ok()??;
    let task_id = snapshot.task.id.clone();
    task_effort::active_task_effort(&snapshot).map(|effort| AssignmentTarget {
        task_id,
        level: effort.level,
        blocked: false,
    })
}

fn open_mesh_assignments(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> Vec<AssignmentTarget> {
    let Some(tasks_dir) = mesh_tasks_dir(teams_dir, team_name) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(tasks_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| read_assignment_task(&entry.path(), member_name))
        .collect()
}

pub(crate) fn mesh_tasks_dir(teams_dir: &Path, team_name: &str) -> Option<PathBuf> {
    if teams_dir.file_name()?.to_str()? != "teams" {
        return None;
    }
    Some(teams_dir.parent()?.join("tasks").join(team_name))
}

fn read_assignment_task(path: &Path, member_name: &str) -> Option<AssignmentTarget> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("json")
        || fs::metadata(path).ok()?.len() > MAX_ASSIGNMENT_RECORD_BYTES
    {
        return None;
    }
    let task: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if task.get("owner").and_then(serde_json::Value::as_str) != Some(member_name) {
        return None;
    }
    let status = task.get("status").and_then(serde_json::Value::as_str)?;
    let status_blocked = match status.trim() {
        "pending" | "in_progress" => false,
        "blocked" => true,
        _ => return None,
    };
    let dependency_blocked = task
        .get("blockedBy")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|blockers| !blockers.is_empty());
    let task_id = non_empty_json_string(task.get("id"))?;
    let level = non_empty_json_string(task.get("metadata")?.get("effort"))?.to_ascii_lowercase();
    (effort_rank(&level) > 0).then_some(AssignmentTarget {
        task_id,
        level,
        blocked: status_blocked || dependency_blocked,
    })
}

fn held_task_id(teams_dir: &Path, team_name: &str, member_name: &str) -> Option<String> {
    let attention_dir = teams_dir
        .join(team_name)
        .join("state/projections/attention");
    let entries = fs::read_dir(attention_dir).ok()?;
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| read_held_attention(&entry.path(), member_name))
        .min()
        .map(|(_, task_id)| task_id)
}

/// Mirrors mesh's `is_awaiting_assignment_delivery` and runtime-notification
/// retry vocabulary. The projection's own `attentionState` is authoritative;
/// older projections without it use the equivalent delivery-state set.
fn read_held_attention(path: &Path, member_name: &str) -> Option<(String, String)> {
    if path.extension().and_then(|extension| extension.to_str()) != Some("json")
        || fs::metadata(path).ok()?.len() > MAX_ASSIGNMENT_RECORD_BYTES
    {
        return None;
    }
    let attention: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    let is_held = match attention
        .get("attentionState")
        .and_then(serde_json::Value::as_str)
    {
        Some(state) => matches!(state, "assigned_pending_delivery" | "delivery_failed"),
        None => matches!(
            attention
                .get("deliveryState")
                .and_then(serde_json::Value::as_str),
            Some("pending" | "unknown" | "failed")
        ),
    };
    if attention
        .get("assignedTo")
        .and_then(serde_json::Value::as_str)
        != Some(member_name)
        || !is_held
        || attention
            .get("deliveredAt")
            .is_some_and(|delivered| !delivered.is_null())
    {
        return None;
    }
    Some((
        non_empty_json_string(attention.get("assignedAt")).unwrap_or_default(),
        non_empty_json_string(attention.get("taskId"))?,
    ))
}

fn non_empty_json_string(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn effort_rank(level: &str) -> u8 {
    match level.trim().to_ascii_lowercase().as_str() {
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "xhigh" => 4,
        _ => 0,
    }
}

impl CoordinationOrchestrator {
    /// Put every pending assignment effort into force for this team.
    ///
    /// mesh types `/effort` into the pane itself, before it delivers the
    /// notice, for every harness that takes the command in its own prompt. This
    /// pass covers the one harness it cannot reach — Codex, which has no such
    /// grammar — by stopping the member and resuming its own conversation with
    /// the effort flag. Returns the members whose level it put into force.
    ///
    /// Both task events and background sweeps may start any switch the member
    /// owes. The scope remains part of failure handling, while the shared
    /// attempt budget bounds either caller.
    ///
    /// **Notice-gated by bundled mesh 0.2.28.** mesh owns both the assignment
    /// record and the inbox, so it holds a Codex notice while `appliedEffort`
    /// differs from the assignment. This pass closes that gate by committing
    /// the level the rendered resume actually carries; mesh's bounded wait is
    /// the fail-open path when a member cannot be switched.
    pub fn apply_pending_task_effort(
        &mut self,
        team_name: &str,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        scope: task_effort::EffortPassScope,
    ) -> Result<Vec<String>, CoordinationError> {
        let mut cli_commands = cli_commands.clone();
        self.apply_pending_task_effort_outcome(
            team_name,
            &mut cli_commands,
            tmux_layout,
            scope,
            &mut |_, _| {},
        )
        .map(|outcome| outcome.switched)
    }

    pub(crate) fn apply_pending_task_effort_outcome(
        &mut self,
        team_name: &str,
        cli_commands: &mut CliCommandSettings,
        tmux_layout: &str,
        scope: task_effort::EffortPassScope,
        resolve_launch_base: &mut dyn FnMut(CliTool, &mut CliCommandSettings),
    ) -> Result<EffortPassOutcome, CoordinationError> {
        validate_team_name(team_name)?;
        let mut outcome = EffortPassOutcome::default();
        let config = match TeamConfigStore::load(&self.teams_dir, team_name) {
            Ok(config) => config,
            Err(err) => {
                outcome
                    .skipped_teams
                    .push((team_name.to_string(), err.to_string()));
                return Ok(outcome);
            }
        };

        for member in &config.members {
            if !task_effort::relaunches_for_effort(member.cli_tool) {
                continue;
            }
            let pending = match pending_member_effort_outcome(self, team_name, member, scope) {
                Ok(Some(pending)) => pending,
                Ok(None) => continue,
                Err(reason) => {
                    outcome.failed.push((member.name.clone(), reason));
                    continue;
                }
            };
            resolve_launch_base(member.cli_tool, cli_commands);
            // The renderer keeps an effort the operator's own base already
            // pins and drops the requested one, so a base that pins gets the
            // assignment's level written into it. Whatever the relaunch will
            // render from is checked here, before anything is stopped: a
            // member is only taken down for a command that carries the level.
            let launch_commands =
                match effort_launch_commands(cli_commands, member.cli_tool, &pending.requested) {
                    Some(commands) => commands,
                    None => {
                        self.record_failed_effort_attempt(
                            team_name,
                            &member.name,
                            &pending.task_id,
                            &pending.requested,
                            pending.failed_attempts + 1,
                        );
                        task_effort::emit_effort_resume(
                            "effort.resume.failed",
                            team_name,
                            &member.name,
                            &pending.task_id,
                            &pending.requested,
                            pending.applied.as_deref(),
                            Some("configured launch command cannot carry the effort"),
                        );
                        outcome.failed.push((
                            member.name.clone(),
                            "configured launch command cannot carry the effort".to_string(),
                        ));
                        continue;
                    }
                };
            task_effort::emit_effort_resume(
                "effort.resume.started",
                team_name,
                &member.name,
                &pending.task_id,
                &pending.requested,
                pending.applied.as_deref(),
                None,
            );

            // A relaunch that resumed a member whose session is still running
            // would render a second one beside it, so a stop that did not land
            // ends the switch here rather than in the pipeline.
            if let Err(reason) = self.stop_member_for_effort_resume(team_name, member) {
                self.record_failed_effort_attempt(
                    team_name,
                    &member.name,
                    &pending.task_id,
                    &pending.requested,
                    pending.failed_attempts + 1,
                );
                task_effort::emit_effort_resume(
                    "effort.resume.failed",
                    team_name,
                    &member.name,
                    &pending.task_id,
                    &pending.requested,
                    pending.applied.as_deref(),
                    Some(&reason),
                );
                outcome.failed.push((member.name.clone(), reason));
                continue;
            }
            let request = ResumeMemberRequest {
                team_name: team_name.to_string(),
                member_name: member.name.clone(),
                reasoning_effort_override: Some(pending.requested.clone()),
            };
            let resume_result = self.resume_member_with_cli_commands_and_layout(
                &request,
                &launch_commands,
                tmux_layout,
            );
            let failure = match &resume_result {
                Ok(report) if report.resumed => None,
                Ok(report) => Some(report.message.clone()),
                Err(err) => Some(err.to_string()),
            };

            match failure {
                None => {
                    outcome.switched.push(member.name.clone());
                    task_effort::emit_effort_resume(
                        "effort.resume.completed",
                        team_name,
                        &member.name,
                        &pending.task_id,
                        &pending.requested,
                        pending.applied.as_deref(),
                        None,
                    );
                }
                Some(reason) => {
                    // The level never took effect and the member is stopped:
                    // recording it as applied would report success and
                    // suppress every later retry. Count the attempt instead.
                    self.record_failed_effort_attempt(
                        team_name,
                        &member.name,
                        &pending.task_id,
                        &pending.requested,
                        pending.failed_attempts + 1,
                    );
                    task_effort::emit_effort_resume(
                        "effort.resume.failed",
                        team_name,
                        &member.name,
                        &pending.task_id,
                        &pending.requested,
                        pending.applied.as_deref(),
                        Some(&reason),
                    );
                    outcome.failed.push((member.name.clone(), reason));
                }
            }
        }

        Ok(outcome)
    }

    /// The level the member's current assignment asks for, for a member being
    /// started rather than switched.
    pub(super) fn open_assignment_effort(
        &self,
        team_name: &str,
        member: &Member,
    ) -> Option<String> {
        if !task_effort::relaunches_for_effort(member.cli_tool) {
            return None;
        }
        let assigned = assignment_target(self, team_name, &member.name, None)?;
        task_effort::resume_effort_target(member.cli_tool, Some(&assigned.level), None)
    }

    /// Take a live member's session down so the resume pipeline can relaunch it.
    fn stop_member_for_effort_resume(
        &mut self,
        team_name: &str,
        member: &Member,
    ) -> Result<(), String> {
        let runtime = match MemberRuntimeStore::load(&self.teams_dir, team_name, &member.name) {
            Ok(runtime) if runtime.health != HealthState::SessionDead => runtime,
            _ => return Ok(()),
        };
        let diagnostics = self.teardown_member_resources_best_effort(
            team_name,
            &member.name,
            Some(member.project_path.as_path()),
            Some(&runtime),
        );
        if let Some(failed) = diagnostics
            .steps
            .iter()
            .find(|step| step.step == "kill_pane" && !step.success)
        {
            return Err(format!(
                "stop failed: {}",
                failed
                    .message
                    .clone()
                    .unwrap_or_else(|| "pane was not terminated".to_string())
            ));
        }
        if let Err(err) =
            MemberRuntimeStore::update(&self.teams_dir, team_name, &member.name, |record| {
                record.health = HealthState::SessionDead;
                record.daemon_pid = None;
            })
        {
            tracing::warn!(
                team = %team_name,
                member = %member.name,
                error = %err,
                "failed to mark a member offline before its effort resume"
            );
        }
        Ok(())
    }

    fn record_failed_effort_attempt(
        &self,
        team_name: &str,
        member_name: &str,
        task_id: &str,
        level: &str,
        attempts: u32,
    ) {
        if let Err(err) =
            MemberRuntimeStore::update(&self.teams_dir, team_name, member_name, |record| {
                record.effort_resume_failure = Some(EffortResumeFailure {
                    task_id: task_id.to_string(),
                    level: level.to_string(),
                    attempts,
                    reason: None,
                });
            })
        {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                error = %err,
                "failed to record the attempt count after a failed effort resume"
            );
        }
    }

    fn record_effort_budget_exhaustion(
        &self,
        team_name: &str,
        member_name: &str,
        task_id: &str,
        level: &str,
    ) -> Result<bool, String> {
        let mut newly_exhausted = false;
        MemberRuntimeStore::update(&self.teams_dir, team_name, member_name, |record| {
            let Some(failure) = record.effort_resume_failure.as_mut() else {
                return;
            };
            if (failure.task_id.is_empty() || failure.task_id == task_id)
                && failure.level.eq_ignore_ascii_case(level)
                && failure.attempts >= MAX_EFFORT_RESUME_ATTEMPTS
                && failure.reason.as_deref() != Some("budget_exhausted")
            {
                failure.task_id = task_id.to_string();
                failure.attempts = MAX_EFFORT_RESUME_ATTEMPTS;
                failure.reason = Some("budget_exhausted".to_string());
                newly_exhausted = true;
            }
        })
        .map_err(|err| format!("could not record exhausted effort budget: {err}"))?;
        Ok(newly_exhausted)
    }
}

/// The effort taurhaus must put into force for a member, if any.
#[cfg(test)]
pub(super) fn pending_member_effort(
    orchestrator: &CoordinationOrchestrator,
    team_name: &str,
    member: &Member,
    scope: task_effort::EffortPassScope,
) -> Option<PendingEffort> {
    pending_member_effort_outcome(orchestrator, team_name, member, scope)
        .ok()
        .flatten()
}

fn pending_member_effort_outcome(
    orchestrator: &CoordinationOrchestrator,
    team_name: &str,
    member: &Member,
    scope: task_effort::EffortPassScope,
) -> Result<Option<PendingEffort>, String> {
    // No runtime record means no session to switch: relaunching here would
    // start a member the operator never launched.
    let runtime = match MemberRuntimeStore::load(&orchestrator.teams_dir, team_name, &member.name) {
        Ok(runtime) => runtime,
        Err(CoordinationError::NotFound(_)) => return Ok(None),
        Err(_) if scope == task_effort::EffortPassScope::RetryPending => return Ok(None),
        Err(err) => return Err(format!("could not load member runtime: {err}")),
    };
    // Only a live member is switched. A member that is down is either one the
    // operator stopped — an assignment is no reason to start it again — or one
    // this pass itself stopped for a switch that failed, which the failure
    // record names and which stays retryable.
    if runtime.health == HealthState::SessionDead && runtime.effort_resume_failure.is_none() {
        return Ok(None);
    }
    let Some(assigned) = assignment_target(
        orchestrator,
        team_name,
        &member.name,
        runtime.applied_effort.as_deref(),
    ) else {
        return Ok(None);
    };
    let Some(requested) = task_effort::resume_effort_target(
        member.cli_tool,
        Some(&assigned.level),
        runtime.applied_effort.as_deref(),
    ) else {
        return Ok(None);
    };
    let matching_failure = runtime.effort_resume_failure.as_ref().filter(|failure| {
        (failure.task_id.is_empty() || failure.task_id == assigned.task_id)
            && failure.level.eq_ignore_ascii_case(&requested)
    });
    let failed_attempts = matching_failure.map_or(0, |failure| failure.attempts);
    let budget_already_exhausted =
        matching_failure.and_then(|failure| failure.reason.as_deref()) == Some("budget_exhausted");
    if !attempt_is_allowed(scope, failed_attempts) {
        if failed_attempts >= MAX_EFFORT_RESUME_ATTEMPTS
            && !budget_already_exhausted
            && orchestrator.record_effort_budget_exhaustion(
                team_name,
                &member.name,
                &assigned.task_id,
                &requested,
            )?
        {
            task_effort::emit_effort_budget_exhausted(
                team_name,
                &member.name,
                &assigned.task_id,
                &requested,
                runtime.applied_effort.as_deref(),
                MAX_EFFORT_RESUME_ATTEMPTS,
            );
            return Err("budget_exhausted".to_string());
        }
        return Ok(None);
    }
    // The relaunch resumes the member's own conversation. Without a session
    // id the resume pipeline would render a fresh launch, throwing away the
    // context the assignment builds on, so the switch is refused outright.
    if runtime
        .session_id
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        orchestrator.record_failed_effort_attempt(
            team_name,
            &member.name,
            &assigned.task_id,
            &requested,
            failed_attempts + 1,
        );
        task_effort::emit_effort_resume(
            "effort.resume.failed",
            team_name,
            &member.name,
            &assigned.task_id,
            &requested,
            runtime.applied_effort.as_deref(),
            Some("member has no recorded session to resume"),
        );
        return Err("member has no recorded session to resume".to_string());
    }
    Ok(Some(PendingEffort {
        task_id: assigned.task_id,
        requested,
        applied: runtime.applied_effort,
        failed_attempts,
    }))
}
