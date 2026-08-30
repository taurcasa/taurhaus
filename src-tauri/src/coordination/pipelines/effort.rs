//! Applying assignment effort to harnesses that must resume with a launch flag.
//!
//! This module owns the pending-effort state machine: target calculation,
//! command rewriting, stop-before-resume sequencing, bounded retries, and
//! durable failure recording. The member-activation pipeline only asks for the
//! effort of an open assignment when it launches a member.

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
    pub(super) requested: String,
    pub(super) applied: Option<String>,
    /// Failures already spent on this level, from the runtime record.
    failed_attempts: u32,
}

/// How often one requested level is retried before the pass leaves it alone.
///
/// A relaunch stops the member before it launches, so a level that keeps
/// failing must not stop it again on every pass. Any launch that commits clears
/// the count, so a member that comes back can reach the level later.
const MAX_EFFORT_RESUME_ATTEMPTS: u32 = 3;

pub(super) fn attempt_is_allowed(
    scope: task_effort::EffortPassScope,
    failed_attempts: u32,
) -> bool {
    failed_attempts < MAX_EFFORT_RESUME_ATTEMPTS
        && (scope == task_effort::EffortPassScope::TaskChanged || failed_attempts > 0)
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
    let base = crate::session_scanner::launch::base_command(
        cli_commands,
        tool,
        crate::daemon::protocol::LaunchMode::Resume,
    );
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
    commands.get_mut(tool)?.resume = rewritten;
    Some(commands)
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
    /// `scope` decides what may be started here: a task event starts any switch
    /// the member owes, a background sweep only retries one already recorded as
    /// failed.
    ///
    /// **Best-effort by design, and behind the notice.** mesh owns both the
    /// assignment record and the inbox, and nothing on taurhaus's side gates
    /// either, so a Codex member can read its assignment at its previous effort
    /// for the seconds between the notice landing and this resume completing.
    /// Closing that window means gating the notice on `appliedEffort` in mesh,
    /// which owns both ends; it is the W5a follow-up. Until then a member that
    /// cannot be switched keeps running at its previous level and still has the
    /// notice, which carries the line.
    pub fn apply_pending_task_effort(
        &mut self,
        team_name: &str,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
        scope: task_effort::EffortPassScope,
    ) -> Result<Vec<String>, CoordinationError> {
        validate_team_name(team_name)?;
        let config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        let mut resumed = Vec::new();

        for member in &config.members {
            if !task_effort::relaunches_for_effort(member.cli_tool) {
                continue;
            }
            let Some(pending) = pending_member_effort(self, team_name, member, scope) else {
                continue;
            };
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
                            &pending.requested,
                            pending.failed_attempts + 1,
                        );
                        task_effort::emit_effort_resume(
                            "effort.resume.failed",
                            team_name,
                            &member.name,
                            &pending.requested,
                            pending.applied.as_deref(),
                            Some("configured launch command cannot carry the effort"),
                        );
                        continue;
                    }
                };
            task_effort::emit_effort_resume(
                "effort.resume.started",
                team_name,
                &member.name,
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
                    &pending.requested,
                    pending.failed_attempts + 1,
                );
                task_effort::emit_effort_resume(
                    "effort.resume.failed",
                    team_name,
                    &member.name,
                    &pending.requested,
                    pending.applied.as_deref(),
                    Some(&reason),
                );
                continue;
            }
            let request = ResumeMemberRequest {
                team_name: team_name.to_string(),
                member_name: member.name.clone(),
                reasoning_effort_override: Some(pending.requested.clone()),
            };
            let outcome = self.resume_member_with_cli_commands_and_layout(
                &request,
                &launch_commands,
                tmux_layout,
            );
            let failure = match &outcome {
                Ok(report) if report.resumed => None,
                Ok(report) => Some(report.message.clone()),
                Err(err) => Some(err.to_string()),
            };

            match failure {
                None => {
                    resumed.push(member.name.clone());
                    task_effort::emit_effort_resume(
                        "effort.resume.completed",
                        team_name,
                        &member.name,
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
                        &pending.requested,
                        pending.failed_attempts + 1,
                    );
                    task_effort::emit_effort_resume(
                        "effort.resume.failed",
                        team_name,
                        &member.name,
                        &pending.requested,
                        pending.applied.as_deref(),
                        Some(&reason),
                    );
                }
            }
        }

        Ok(resumed)
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
        let assigned = self.active_task_effort(team_name, &member.name)?;
        task_effort::resume_effort_target(member.cli_tool, Some(&assigned.level), None)
    }

    /// The effort of the task taurhaus currently selected.
    fn active_task_effort(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Option<task_effort::AssignmentEffort> {
        let snapshot =
            OperationalContextSnapshotStore::load(&self.teams_dir, team_name, member_name)
                .ok()??;
        task_effort::active_task_effort(&snapshot)
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
        level: &str,
        attempts: u32,
    ) {
        if let Err(err) =
            MemberRuntimeStore::update(&self.teams_dir, team_name, member_name, |record| {
                record.effort_resume_failure = Some(EffortResumeFailure {
                    level: level.to_string(),
                    attempts,
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
}

/// The effort taurhaus must put into force for a member, if any.
pub(super) fn pending_member_effort(
    orchestrator: &CoordinationOrchestrator,
    team_name: &str,
    member: &Member,
    scope: task_effort::EffortPassScope,
) -> Option<PendingEffort> {
    // No runtime record means no session to switch: relaunching here would
    // start a member the operator never launched.
    let runtime =
        MemberRuntimeStore::load(&orchestrator.teams_dir, team_name, &member.name).ok()?;
    // Only a live member is switched. A member that is down is either one the
    // operator stopped — an assignment is no reason to start it again — or one
    // this pass itself stopped for a switch that failed, which the failure
    // record names and which stays retryable.
    if runtime.health == HealthState::SessionDead && runtime.effort_resume_failure.is_none() {
        return None;
    }
    let assigned = orchestrator.active_task_effort(team_name, &member.name)?;
    let requested = task_effort::resume_effort_target(
        member.cli_tool,
        Some(&assigned.level),
        runtime.applied_effort.as_deref(),
    )?;
    let failed_attempts = runtime
        .effort_resume_failure
        .as_ref()
        .filter(|failure| failure.level.eq_ignore_ascii_case(&requested))
        .map_or(0, |failure| failure.attempts);
    if !attempt_is_allowed(scope, failed_attempts) {
        return None;
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
            &requested,
            failed_attempts + 1,
        );
        task_effort::emit_effort_resume(
            "effort.resume.failed",
            team_name,
            &member.name,
            &requested,
            runtime.applied_effort.as_deref(),
            Some("member has no recorded session to resume"),
        );
        return None;
    }
    Some(PendingEffort {
        requested,
        applied: runtime.applied_effort,
        failed_attempts,
    })
}
