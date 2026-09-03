//! Daemon-owned hosts for team resume and member reonboarding.

use std::sync::Arc;

use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
use crate::coordination::domain::MemberRole;
use crate::coordination::reinjection::CompactionReinjectionService;
use crate::coordination::requests::{
    DeliveryRequest, DeliveryResult, OperatorNoticeDelivery, ReonboardRequest,
};
use crate::coordination::state::CoordinationState;
use crate::daemon::coordination_runs::{
    prepare_daemon_launch_inputs_for_tools, CoordinationRunKind, CoordinationRunRegistry,
    CoordinationRunReport, RunOutcome,
};
use crate::daemon::protocol::{
    CoordinationReonboardOutcome, CoordinationReonboardParams, CoordinationReonboardStatus,
    CoordinationResumeTeamOutcome, CoordinationResumeTeamParams, CoordinationResumeTeamStatus,
    CoordinationSwitchTeamAccountOutcome, CoordinationSwitchTeamAccountParams,
    CoordinationSwitchTeamAccountStatus,
};
use crate::models::CliCommandSettings;

type PrepareResumeTeamLaunchInputs = dyn Fn(
        &crate::coordination::requests::ResumeTeamRequest,
        &mut CliCommandSettings,
    ) -> Result<(), String>
    + Send
    + Sync;

#[derive(Clone)]
pub(crate) struct TeamOperationsService {
    registry: CoordinationRunRegistry,
    state: Arc<CoordinationState>,
    prepare_resume_team_launch_inputs: Arc<PrepareResumeTeamLaunchInputs>,
}

impl TeamOperationsService {
    pub(crate) fn for_process_default(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
    ) -> Self {
        let teams_dir = state.teams_dir().clone();
        Self::with_state_and_prepare(
            state,
            registry,
            Arc::new(move |request, commands| {
                prepare_resume_team_launch_inputs(&teams_dir, request, commands)
            }),
        )
    }

    fn with_state_and_prepare(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
        prepare_resume_team_launch_inputs: Arc<PrepareResumeTeamLaunchInputs>,
    ) -> Self {
        Self {
            registry,
            state,
            prepare_resume_team_launch_inputs,
        }
    }

    pub(crate) fn start_resume_team(
        &self,
        params: CoordinationResumeTeamParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::ResumeTeam);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let prepare_launch_inputs = self.prepare_resume_team_launch_inputs.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!(
                "coordination-team-resume-{}",
                run_id
                    .rsplit('_')
                    .next()
                    .and_then(|tail| tail.get(..8))
                    .unwrap_or(run_id.as_str())
            ))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut cli_commands = params.cli_commands;
                    prepare_launch_inputs(&params.request, &mut cli_commands)?;
                    let report = crate::daemon::member_runs::execute_resume_team_pipeline(
                        state.as_ref(),
                        &params.request,
                        &cli_commands,
                        &params.tmux_layout,
                        Some(&mut |progress| {
                            let event = crate::commands::coordination::resume_team_progress_event(
                                &params.request.team_name,
                                &progress,
                            );
                            crate::commands::coordination::emit_resume_team_progress_log_event(
                                &event,
                            );
                            let _ = registry.record_resume_team_step(&run_id_for_task, progress);
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                    crate::coordination::stores::active_project::sync_team_from_config(
                        state.teams_dir(),
                        &report.team_name,
                    )
                    .map_err(|error| error.to_string())?;
                    Ok::<_, String>(report)
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry
                            .complete(&run_id_for_task, CoordinationRunReport::ResumeTeam(report));
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry
                            .fail(&run_id_for_task, "resume-team worker panicked".to_string());
                    }
                }
            });
        finish_spawn(self, run_id, spawn_result, "resume-team")
    }

    pub(crate) fn start_reonboard(
        &self,
        params: CoordinationReonboardParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::Reonboard);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!(
                "coordination-reonboard-{}",
                run_id
                    .rsplit('_')
                    .next()
                    .and_then(|tail| tail.get(..8))
                    .unwrap_or(run_id.as_str())
            ))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let CoordinationReonboardParams {
                        request,
                        cli_commands: _,
                        tmux_layout: _,
                        operational_snapshot,
                        task_state_changed_at,
                    } = params;
                    let report = execute_reonboard_pipeline(state.as_ref(), &request)
                        .map_err(|error| error.to_string())?;
                    finalize_reonboard_state(
                        state.teams_dir(),
                        &request,
                        operational_snapshot.as_ref(),
                        task_state_changed_at,
                    )
                    .map_err(|error| error.to_string())?;
                    Ok::<_, String>(report)
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry
                            .complete(&run_id_for_task, CoordinationRunReport::Reonboard(report));
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry
                            .fail(&run_id_for_task, "reonboard worker panicked".to_string());
                    }
                }
            });
        finish_spawn(self, run_id, spawn_result, "reonboard")
    }

    pub(crate) fn start_switch_team_account(
        &self,
        params: CoordinationSwitchTeamAccountParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::SwitchTeamAccount);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let prepare_launch_inputs = self.prepare_resume_team_launch_inputs.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!(
                "coordination-account-switch-{}",
                run_id
                    .rsplit('_')
                    .next()
                    .and_then(|tail| tail.get(..8))
                    .unwrap_or(run_id.as_str())
            ))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut cli_commands = params.cli_commands;
                    prepare_launch_inputs(
                        &crate::coordination::requests::ResumeTeamRequest {
                            team_name: params.request.team_name.clone(),
                        },
                        &mut cli_commands,
                    )?;
                    execute_switch_team_account(
                        state.as_ref(),
                        &params.request,
                        &cli_commands,
                        &params.tmux_layout,
                    )
                    .map_err(|error| error.to_string())
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry.complete(
                            &run_id_for_task,
                            CoordinationRunReport::SwitchTeamAccount(Box::new(report)),
                        );
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry.fail(
                            &run_id_for_task,
                            "account-switch worker panicked".to_string(),
                        );
                    }
                }
            });
        finish_spawn(self, run_id, spawn_result, "account-switch")
    }

    pub(crate) fn resume_team_status(&self, run_id: &str) -> Option<CoordinationResumeTeamStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::ResumeTeam {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationResumeTeamOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::ResumeTeam(report),
            } => CoordinationResumeTeamOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationResumeTeamOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationResumeTeamStatus {
            run_id: status.run_id,
            steps: status.resume_team_steps,
            outcome,
        })
    }

    pub(crate) fn reonboard_status(&self, run_id: &str) -> Option<CoordinationReonboardStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::Reonboard {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationReonboardOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::Reonboard(report),
            } => CoordinationReonboardOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationReonboardOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationReonboardStatus {
            run_id: status.run_id,
            outcome,
        })
    }

    pub(crate) fn switch_team_account_status(
        &self,
        run_id: &str,
    ) -> Option<CoordinationSwitchTeamAccountStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::SwitchTeamAccount {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationSwitchTeamAccountOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::SwitchTeamAccount(report),
            } => CoordinationSwitchTeamAccountOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationSwitchTeamAccountOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationSwitchTeamAccountStatus {
            run_id: status.run_id,
            outcome,
        })
    }
}

fn finish_spawn(
    service: &TeamOperationsService,
    run_id: String,
    spawn_result: std::io::Result<std::thread::JoinHandle<()>>,
    operation: &str,
) -> Result<String, String> {
    match spawn_result {
        Ok(_) => Ok(run_id),
        Err(error) => {
            let message = format!("failed to start {operation} worker: {error}");
            let _ = service.registry.fail(&run_id, message.clone());
            Err(message)
        }
    }
}

fn prepare_resume_team_launch_inputs(
    teams_dir: &std::path::Path,
    request: &crate::coordination::requests::ResumeTeamRequest,
    commands: &mut CliCommandSettings,
) -> Result<(), String> {
    let config = crate::coordination::stores::TeamConfigStore::load(teams_dir, &request.team_name)
        .map_err(|error| error.to_string())?;
    let tools = config
        .members
        .iter()
        .map(|member| member.cli_tool)
        .collect::<Vec<_>>();
    // The named authority for "team has a managed Codex member" — hook_trust
    // only coincides with it while Codex is the sole trusted harness, and
    // the identity check itself must stay inside the capability slice.
    let has_managed_codex = crate::coordination::compact_hook::team_has_managed_codex_member(
        teams_dir,
        &request.team_name,
    )
    .unwrap_or(false);
    prepare_daemon_launch_inputs_for_tools(teams_dir, has_managed_codex, tools, commands);
    Ok(())
}

pub(crate) fn execute_switch_team_account(
    state: &CoordinationState,
    request: &crate::coordination::requests::SwitchTeamAccountRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<
    crate::coordination::requests::SwitchTeamAccountReport,
    crate::coordination::errors::CoordinationError,
> {
    use crate::coordination::errors::CoordinationError;
    use crate::coordination::requests::{AccountSwitchHandoffManifest, SwitchTeamAccountReport};
    use crate::coordination::stores::{
        AccountSwitchManifestStore, MemberRuntimeStore, TeamConfigStore,
    };

    crate::coordination::validation::validate_team_name(&request.team_name)?;
    let capabilities = crate::session_scanner::cli_tool::spec(request.cli_tool).capabilities;
    if capabilities.team_config_namespace {
        return Err(CoordinationError::Validation(
            "Claude accounts belong to the whole team; mixed-account Claude teams are impossible"
                .to_string(),
        ));
    }
    if capabilities.account_selector.is_none() {
        return Err(CoordinationError::Validation(format!(
            "{} does not support account switching",
            request.cli_tool
        )));
    }
    let requested_account_id = request.account_id.trim();
    if requested_account_id.is_empty() {
        return Err(CoordinationError::Validation(
            "account id must not be empty".to_string(),
        ));
    }
    let target = cli_commands
        .managed_accounts
        .get(&request.cli_tool)
        .and_then(|accounts| {
            accounts
                .iter()
                .find(|account| account.id == requested_account_id && account.logged_in)
        })
        .ok_or_else(|| {
            CoordinationError::Validation(format!(
                "account '{requested_account_id}' is unavailable or signed out"
            ))
        })?
        .clone();

    state.with_orchestrator(|orchestrator| {
        let mut config = TeamConfigStore::load(state.teams_dir(), &request.team_name)?;
        let switched_members = config
            .members
            .iter()
            .filter(|member| member.cli_tool == request.cli_tool)
            .map(|member| member.name.clone())
            .collect::<Vec<_>>();
        if switched_members.is_empty() {
            return Err(CoordinationError::Validation(format!(
                "team '{}' has no {} members",
                request.team_name, request.cli_tool
            )));
        }
        let lead_name = config
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());

        let handoffs = switched_members
            .iter()
            .map(|member_name| {
                let member = config
                    .members
                    .iter()
                    .find(|member| member.name == *member_name)
                    .expect("switched member came from config");
                let runtime = MemberRuntimeStore::load(
                    state.teams_dir(),
                    &request.team_name,
                    member_name,
                )
                .ok();
                account_switch_handoff(member, runtime.as_ref())
            })
            .collect::<Vec<_>>();

        orchestrator.stop_team_daemon_best_effort(&request.team_name);
        for member in &config.members {
            orchestrator
                .stop_member_for_account_switch(&request.team_name, member)
                .map_err(CoordinationError::Backend)?;
        }

        for member in &mut config.members {
            if member.cli_tool == request.cli_tool {
                member.account_id = Some(target.id.clone());
            }
        }
        let manifest = AccountSwitchHandoffManifest {
            switched_at: chrono::Utc::now(),
            cli_tool: request.cli_tool,
            account_id: target.id.clone(),
            account_label: target.label.clone(),
            members: handoffs.clone(),
        };
        let handoff_manifest_count = AccountSwitchManifestStore::append(
            state.teams_dir(),
            &request.team_name,
            manifest,
        )?;
        TeamConfigStore::save(state.teams_dir(), &request.team_name, &config)?;

        let resume = orchestrator.resume_team_with_cli_commands_and_layout(
            &crate::coordination::requests::ResumeTeamRequest {
                team_name: request.team_name.clone(),
            },
            cli_commands,
            tmux_layout,
        )?;
        for handoff in &handoffs {
            if !resume.resumed_members.contains(&handoff.member_name) {
                continue;
            }
            let previous_label = handoff
                .previous_account_label
                .as_deref()
                .or(handoff.previous_account_id.as_deref())
                .unwrap_or("Default");
            let transcript = handoff.transcript_path.as_deref().unwrap_or("unavailable");
            let message = format!(
                "[taurhaus] Account switch complete: {previous_label} → {}.\nPrevious session: {}.\nPrevious transcript: {transcript}.\n{}\nThe transcript remains in its original location; this handoff only points to it. Rebuild task context with `mesh task get ID` before continuing.",
                target.label,
                handoff.session_id.as_deref().unwrap_or("unavailable"),
                handoff.last_activity_line,
            );
            if let Err(error) = orchestrator.deliver_message(DeliveryRequest::operator_notice(
                OperatorNoticeDelivery {
                    member_name: handoff.member_name.clone(),
                    team_name: request.team_name.clone(),
                    message,
                    sender_name: None,
                    operational_context: None,
                },
            )) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %handoff.member_name,
                    error = %error,
                    "failed to deliver account-switch onboarding"
                );
            }
        }
        if resume.resumed_members.contains(&lead_name) {
            let previous_accounts = handoffs
                .iter()
                .map(|handoff| {
                    handoff
                        .previous_account_label
                        .as_deref()
                        .or(handoff.previous_account_id.as_deref())
                        .unwrap_or("Default")
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ");
            let member_entries = handoffs
                .iter()
                .map(|handoff| {
                    format!(
                        "- {}: previous session {}; transcript {} ({})",
                        handoff.member_name,
                        handoff.session_id.as_deref().unwrap_or("unavailable"),
                        handoff.transcript_path.as_deref().unwrap_or("unavailable"),
                        handoff.last_activity_line,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let message = format!(
                "[taurhaus] Team account switch complete: {previous_accounts} → {}.\nPrior-run entry points (not required reading):\n{member_entries}\nSkim transcript tails only if context is missing; canonical state is the task ledger. Rebuild task context with `mesh task get ID` before continuing.",
                target.label,
            );
            if let Err(error) = orchestrator.deliver_message(DeliveryRequest::operator_notice(
                OperatorNoticeDelivery {
                    member_name: lead_name.clone(),
                    team_name: request.team_name.clone(),
                    message,
                    sender_name: None,
                    operational_context: None,
                },
            )) {
                tracing::warn!(
                    team = %request.team_name,
                    member = %lead_name,
                    error = %error,
                    "failed to deliver account-switch lead onboarding"
                );
            }
        }

        Ok(SwitchTeamAccountReport {
            team_name: request.team_name.clone(),
            cli_tool: request.cli_tool,
            account_id: target.id,
            account_label: target.label,
            switched_members,
            handoff_manifest_count,
            resume,
        })
    })
}

fn account_switch_handoff(
    member: &crate::coordination::domain::Member,
    runtime: Option<&crate::coordination::stores::MemberRuntimeRecord>,
) -> crate::coordination::requests::AccountSwitchMemberHandoff {
    let last_activity = runtime
        .and_then(|runtime| runtime.last_seen_at.or(runtime.attached_at))
        .map(|timestamp| timestamp.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string());
    crate::coordination::requests::AccountSwitchMemberHandoff {
        member_name: member.name.clone(),
        previous_account_id: runtime
            .and_then(|runtime| runtime.launch_account.account_id.clone())
            .or_else(|| member.account_id.clone()),
        previous_account_label: runtime
            .and_then(|runtime| runtime.launch_account.account_label.clone()),
        session_id: runtime.and_then(|runtime| runtime.session_id.clone()),
        transcript_path: runtime
            .and_then(|runtime| runtime.jsonl_path.as_ref())
            .map(|path| path.display().to_string()),
        last_activity_line: format!("Last activity: {last_activity}"),
    }
}

pub(crate) fn execute_reonboard_pipeline(
    state: &CoordinationState,
    request: &ReonboardRequest,
) -> Result<DeliveryResult, crate::coordination::errors::CoordinationError> {
    state.with_orchestrator(|orchestrator| {
        let team = orchestrator.get_team_status(&request.team_name)?;
        let lead_name = team
            .config
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());
        let member = team
            .config
            .members
            .iter()
            .find(|member| member.name == request.member_name)
            .ok_or_else(|| {
                crate::coordination::errors::CoordinationError::NotFound(format!(
                    "member '{}' not found in team '{}'",
                    request.member_name, request.team_name
                ))
            })?;
        let role_context = RoleContext {
            role_id: member.role_id.as_deref(),
            communication_style: member.communication_style.as_deref(),
            instructions: member.instructions.as_deref(),
            behavioral_contract: member.behavioral_contract.as_ref(),
            quality_gates: member.quality_gates.as_deref(),
            handoff_expectations: member.handoff_expectations.as_deref(),
            definition_of_done: member.definition_of_done.as_deref(),
            capabilities: member.capabilities.as_deref(),
        };
        let tool_spec = crate::session_scanner::cli_tool::spec(member.cli_tool);
        let mut message = if tool_spec.capabilities.native_inbox_poller {
            DeliveryRenderer::render_onboarding(
                &request.team_name,
                &request.member_name,
                &lead_name,
                role_context,
            )
        } else {
            DeliveryRenderer::render_for_tool(
                member.cli_tool,
                &request.team_name,
                &request.member_name,
                &lead_name,
                true,
                role_context,
            )
            .ok_or_else(|| {
                crate::coordination::errors::CoordinationError::Validation(
                    "onboarding is not required for this harness".to_string(),
                )
            })?
        };
        CompactionReinjectionService::append_member_lease_context(
            &mut message,
            &orchestrator.teams_dir,
            &request.team_name,
            &request.member_name,
        );

        orchestrator.deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: request.member_name.clone(),
            team_name: request.team_name.clone(),
            message,
            sender_name: Some(lead_name),
            operational_context: None,
        }))
    })
}

fn finalize_reonboard_state(
    teams_dir: &std::path::Path,
    request: &ReonboardRequest,
    snapshot: Option<&crate::coordination::stores::OperationalContextSnapshot>,
    task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    if let Some(snapshot) = snapshot {
        let belongs_to_member =
            snapshot.team_name == request.team_name && snapshot.member_name == request.member_name;
        if belongs_to_member {
            crate::coordination::operational_context::publish_member_operation_snapshot(
                teams_dir,
                snapshot,
                task_state_changed_at,
            )?;
        } else {
            tracing::warn!(
                team = %request.team_name,
                member = %request.member_name,
                snapshot_team = %snapshot.team_name,
                snapshot_member = %snapshot.member_name,
                "skipping an operational snapshot that does not belong to the reonboard run"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::TeamOperationsService;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::requests::{
        AgentDefinition, InitializeTeamRequest, LeadMode, MemberActivationStage, StepStatus,
        SwitchTeamAccountRequest,
    };
    use crate::coordination::runtime::{CoordinationRuntime, RecordingCoordinationRuntime};
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::{
        AccountSwitchManifestStore, MemberRuntimeStore, MeshInboxStore,
        OperationalContextSnapshotStore, TeamConfigStore,
    };
    use crate::daemon::coordination_runs::CoordinationRunRegistry;
    use crate::daemon::protocol::{
        CoordinationReonboardOutcome, CoordinationReonboardParams, CoordinationResumeTeamOutcome,
        CoordinationResumeTeamParams,
    };
    use crate::models::{CliCommandSettings, ManagedLaunchAccount};
    use crate::session_scanner::cli_tool::CliTool;

    fn agent(name: &str, project: &std::path::Path) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
            account_id: None,
            project_id: project.display().to_string(),
            description: None,
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
        }
    }

    fn state(
        root: &std::path::Path,
    ) -> (
        Arc<CoordinationState>,
        FakeBackend,
        Arc<RecordingCoordinationRuntime>,
    ) {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(root);
        let runtime_for_factory = runtime.clone();
        let backend = FakeBackend::default();
        let backend_for_factory = backend.clone();
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            root.to_path_buf(),
            BackendSelector::m0(),
            Arc::new(move |_kind, _teams_dir| {
                Ok(Arc::new(backend_for_factory.clone()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ));
        (state, backend, runtime)
    }

    fn initialize_team(state: &CoordinationState, project: &std::path::Path) {
        crate::daemon::initialize_runs::execute_initialize_pipeline(
            state,
            &InitializeTeamRequest {
                team_name: "arch".to_string(),
                team_description: None,
                lead_mode: LeadMode::LaunchNew,
                lead: agent("team-lead", &project.join("lead")),
                agents: vec![agent("builder", &project.join("builder"))],
            },
            &CliCommandSettings::default(),
            "new_window",
            None,
        )
        .expect("initialize pipeline");
    }

    fn service(state: Arc<CoordinationState>) -> TeamOperationsService {
        TeamOperationsService::with_state_and_prepare(
            state,
            CoordinationRunRegistry::default(),
            Arc::new(|_request, _commands| Ok(())),
        )
    }

    #[test]
    fn resume_team_executes_in_daemon_state_and_streams_canonical_member_stages() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (state, _backend, _runtime) = state(temp.path());
        initialize_team(state.as_ref(), temp.path());
        for member_name in ["team-lead", "builder"] {
            let mut runtime =
                MemberRuntimeStore::load(temp.path(), "arch", member_name).expect("runtime");
            runtime.health = crate::coordination::domain::HealthState::SessionDead;
            runtime.pane_id = None;
            runtime.daemon_pid = None;
            MemberRuntimeStore::save(temp.path(), "arch", member_name, &runtime)
                .expect("save stopped runtime");
        }
        let service = service(state);

        let run_id = service
            .start_resume_team(CoordinationResumeTeamParams {
                request: crate::coordination::requests::ResumeTeamRequest {
                    team_name: "arch".to_string(),
                },
                cli_commands: CliCommandSettings::default(),
                tmux_layout: "new_window".to_string(),
            })
            .expect("resume worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            let status = service.resume_team_status(&run_id).expect("run registered");
            if status.outcome != CoordinationResumeTeamOutcome::Running {
                break status;
            }
            assert!(Instant::now() < deadline, "resume-team run did not finish");
            std::thread::sleep(Duration::from_millis(5));
        };

        let CoordinationResumeTeamOutcome::Completed { report } = status.outcome else {
            panic!("resume-team should complete: {:?}", status.outcome);
        };
        assert!(report.resumed, "{report:?}");
        assert_eq!(report.resumed_members, ["team-lead", "builder"]);
        assert!(status.steps.iter().any(|step| {
            step.member_name == "team-lead"
                && step.member_index == 1
                && step.member_count == 2
                && step.stage == MemberActivationStage::PrepareMember
                && step.status == StepStatus::Running
        }));
        assert!(status.steps.iter().any(|step| {
            step.member_name == "builder"
                && step.stage == MemberActivationStage::CommitRuntime
                && step.status == StepStatus::Succeeded
        }));
    }

    #[test]
    fn reonboard_executes_delivery_and_publishes_the_fat_intent_snapshot() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (state, backend, _runtime) = state(temp.path());
        initialize_team(state.as_ref(), temp.path());
        let leases_dir = temp.path().join("arch").join("state").join("leases");
        std::fs::create_dir_all(&leases_dir).expect("create leases dir");
        std::fs::write(
            leases_dir.join("delivery-renderer.json"),
            r#"{"name":"delivery-renderer","state":"held","holder":"builder","waiters":[]}"#,
        )
        .expect("write held lease");
        let service = service(state);
        let snapshot = crate::coordination::stores::OperationalContextSnapshot {
            version: 1,
            team_name: "arch".to_string(),
            member_name: "builder".to_string(),
            updated_at: chrono::Utc::now(),
            task: Default::default(),
            assignment_footer: Default::default(),
            ownership: Default::default(),
            working_set: crate::coordination::stores::OperationalWorkingSetSnapshot {
                project_path: temp.path().join("builder").display().to_string(),
                focal_files: vec!["src/current.rs".to_string()],
            },
        };

        let run_id = service
            .start_reonboard(CoordinationReonboardParams {
                request: crate::coordination::requests::ReonboardRequest {
                    team_name: "arch".to_string(),
                    member_name: "builder".to_string(),
                },
                cli_commands: CliCommandSettings::default(),
                tmux_layout: "new_window".to_string(),
                operational_snapshot: Some(snapshot),
                task_state_changed_at: None,
            })
            .expect("reonboard worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            let status = service.reonboard_status(&run_id).expect("run registered");
            if status.outcome != CoordinationReonboardOutcome::Running {
                break status;
            }
            assert!(Instant::now() < deadline, "reonboard run did not finish");
            std::thread::sleep(Duration::from_millis(5));
        };

        let CoordinationReonboardOutcome::Completed { report } = status.outcome else {
            panic!("reonboard should complete: {:?}", status.outcome);
        };
        assert!(report.delivered);
        let requests = backend.delivered_requests();
        let crate::coordination::requests::DeliveryRequest::OperatorNotice(delivery) =
            requests.last().expect("reonboard delivery")
        else {
            panic!("expected operator notice")
        };
        assert!(delivery.message.starts_with("[taurhaus] onboarding"));
        assert!(delivery.message.contains("mesh read --unread --mark-read"));
        assert!(delivery.message.contains("Leases: held delivery-renderer."));
        assert_eq!(
            OperationalContextSnapshotStore::load(temp.path(), "arch", "builder")
                .expect("load snapshot")
                .expect("snapshot published")
                .working_set
                .focal_files,
            ["src/current.rs"]
        );
    }

    #[test]
    fn switch_team_account_stops_rewrites_resumes_and_accumulates_pointer_handoffs() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (state, backend, _runtime) = state(temp.path());
        initialize_team(state.as_ref(), temp.path());

        let mut config = TeamConfigStore::load(temp.path(), "arch").expect("config");
        for member in &mut config.members {
            if member.role == crate::coordination::domain::MemberRole::Lead {
                member.cli_tool = CliTool::Claude;
            } else {
                member.account_id = Some("personal".to_string());
            }
        }
        TeamConfigStore::save(temp.path(), "arch", &config).expect("seed member accounts");
        for member_name in ["team-lead", "builder"] {
            MemberRuntimeStore::update(temp.path(), "arch", member_name, |runtime| {
                runtime.session_id = Some(format!("session-{member_name}"));
                runtime.jsonl_path = Some(temp.path().join(format!("{member_name}.jsonl")));
                runtime.last_seen_at = Some(chrono::Utc::now());
                runtime.launch_account.account_id = Some("personal".to_string());
                runtime.launch_account.account_label = Some("Personal".to_string());
                runtime.launch_account.account_applied = Some(true);
            })
            .expect("seed runtime identity");
        }
        let mut commands = CliCommandSettings::default();
        commands.managed_accounts.insert(
            CliTool::Codex,
            vec![ManagedLaunchAccount {
                id: "work".to_string(),
                label: "Work".to_string(),
                dir: temp.path().join("codex-work"),
                logged_in: true,
                is_default: false,
            }],
        );

        let report = super::execute_switch_team_account(
            state.as_ref(),
            &SwitchTeamAccountRequest {
                team_name: "arch".to_string(),
                cli_tool: CliTool::Codex,
                account_id: "work".to_string(),
            },
            &commands,
            "new_window",
        )
        .expect("switch account");

        assert_eq!(report.account_label, "Work");
        assert_eq!(report.switched_members, ["builder"]);
        assert_eq!(report.handoff_manifest_count, 1);
        let config = TeamConfigStore::load(temp.path(), "arch").expect("updated config");
        assert_eq!(
            config
                .members
                .iter()
                .find(|member| member.name == "builder")
                .and_then(|member| member.account_id.as_deref()),
            Some("work")
        );
        let manifests =
            AccountSwitchManifestStore::load(temp.path(), "arch").expect("persisted manifests");
        assert_eq!(manifests.len(), 1);
        assert_eq!(
            manifests[0].members[0].previous_account_label.as_deref(),
            Some("Personal")
        );
        assert!(manifests[0].members[0]
            .transcript_path
            .as_deref()
            .expect("pointer")
            .ends_with("builder.jsonl"));
        assert!(manifests[0].members[0]
            .last_activity_line
            .starts_with("Last activity: "));
        assert!(report.resume.resumed);
        let notices = backend.delivered_requests();
        assert!(notices.iter().any(|request| matches!(
            request,
            crate::coordination::requests::DeliveryRequest::OperatorNotice(notice)
                if notice.message.contains("Account switch complete")
                    && notice.message.contains("Previous transcript:")
        )));
        // Regression: 0bc79ceb delivered switch onboarding only to members of
        // the switched tool, so a Claude lead never received the handoff map.
        let lead_inbox =
            MeshInboxStore::load(temp.path(), "arch", "team-lead").expect("Claude lead inbox");
        assert!(lead_inbox.iter().any(|notice| {
            notice.text.contains("builder")
                && notice.text.contains("builder.jsonl")
                && notice.text.contains("mesh task get")
        }));

        let second = super::execute_switch_team_account(
            state.as_ref(),
            &SwitchTeamAccountRequest {
                team_name: "arch".to_string(),
                cli_tool: CliTool::Codex,
                account_id: "work".to_string(),
            },
            &commands,
            "new_window",
        )
        .expect("second switch appends another handoff");
        assert_eq!(second.handoff_manifest_count, 2);
        assert_eq!(
            AccountSwitchManifestStore::load(temp.path(), "arch")
                .expect("persisted manifests")
                .len(),
            2
        );
    }

    #[test]
    fn switch_team_account_rejects_per_member_claude_selection() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let (state, _backend, _runtime) = state(temp.path());
        let error = super::execute_switch_team_account(
            state.as_ref(),
            &SwitchTeamAccountRequest {
                team_name: "arch".to_string(),
                cli_tool: CliTool::Claude,
                account_id: "claude-work".to_string(),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect_err("Claude has one account namespace per team");
        assert!(error
            .to_string()
            .contains("mixed-account Claude teams are impossible"));
    }
}
