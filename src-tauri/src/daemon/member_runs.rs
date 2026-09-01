//! Daemon-owned hosts for add-agent, resume-member, and stop-member pipelines.

use std::sync::Arc;

use crate::coordination::requests::{
    AddAgentReport, AddAgentRequest, ResumeAgentReport, ResumeMemberRequest, StepProgress,
    StopMemberReport, StopMemberRequest,
};
use crate::coordination::state::CoordinationState;
use crate::daemon::coordination_runs::{
    prepare_daemon_launch_inputs_for_tools, CoordinationRunKind, CoordinationRunRegistry,
    CoordinationRunReport, RunOutcome,
};
use crate::daemon::protocol::{
    CoordinationAddAgentOutcome, CoordinationAddAgentParams, CoordinationAddAgentStatus,
    CoordinationResumeMemberOutcome, CoordinationResumeMemberParams,
    CoordinationResumeMemberStatus, CoordinationStopMemberOutcome, CoordinationStopMemberParams,
    CoordinationStopMemberStatus,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

type PrepareAddLaunchInputs =
    dyn Fn(&AddAgentRequest, &mut CliCommandSettings) -> Result<(), String> + Send + Sync;
type PrepareResumeLaunchInputs =
    dyn Fn(&ResumeMemberRequest, &mut CliCommandSettings) -> Result<(), String> + Send + Sync;

#[derive(Clone)]
pub(crate) struct MemberOperationsService {
    registry: CoordinationRunRegistry,
    state: Arc<CoordinationState>,
    prepare_add_launch_inputs: Arc<PrepareAddLaunchInputs>,
    prepare_resume_launch_inputs: Arc<PrepareResumeLaunchInputs>,
}

impl MemberOperationsService {
    pub(crate) fn for_process_default(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
    ) -> Self {
        let add_teams_dir = state.teams_dir().clone();
        let resume_teams_dir = add_teams_dir.clone();
        Self::with_state_and_prepare(
            state,
            registry,
            Arc::new(move |request, commands| {
                prepare_add_launch_inputs(&add_teams_dir, request, commands)
            }),
            Arc::new(move |request, commands| {
                prepare_resume_launch_inputs(&resume_teams_dir, request, commands)
            }),
        )
    }

    fn with_state_and_prepare(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
        prepare_add_launch_inputs: Arc<PrepareAddLaunchInputs>,
        prepare_resume_launch_inputs: Arc<PrepareResumeLaunchInputs>,
    ) -> Self {
        Self {
            registry,
            state,
            prepare_add_launch_inputs,
            prepare_resume_launch_inputs,
        }
    }

    pub(crate) fn start_add_agent(
        &self,
        params: CoordinationAddAgentParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::AddAgent);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let prepare_launch_inputs = self.prepare_add_launch_inputs.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!("coordination-add-{}", &run_id[4..12]))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut cli_commands = params.cli_commands;
                    prepare_launch_inputs(&params.request, &mut cli_commands)?;
                    let report = execute_add_agent_pipeline(
                        state.as_ref(),
                        &params.request,
                        &cli_commands,
                        &params.tmux_layout,
                    )
                    .map_err(|error| error.to_string())?;
                    for event in crate::commands::coordination::progress_events_for_steps(
                        &report.team_name,
                        "add_agent",
                        &report.steps,
                    ) {
                        crate::commands::coordination::emit_progress_log_event(&event);
                        let _ = registry.record_step(&run_id_for_task, event.progress);
                    }
                    if report.failed_step.is_none() {
                        finalize_member_state(
                            state.teams_dir(),
                            &report.team_name,
                            &report.member_name,
                            params.operational_snapshot.as_ref(),
                        )
                        .map_err(|error| error.to_string())?;
                    }
                    Ok::<_, String>(report)
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry
                            .complete(&run_id_for_task, CoordinationRunReport::AddAgent(report));
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry
                            .fail(&run_id_for_task, "add-agent worker panicked".to_string());
                    }
                }
            });
        finish_spawn(self, run_id, spawn_result, "add-agent")
    }

    pub(crate) fn start_resume_member(
        &self,
        params: CoordinationResumeMemberParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::ResumeMember);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let prepare_launch_inputs = self.prepare_resume_launch_inputs.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!("coordination-resume-{}", &run_id[7..15]))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut cli_commands = params.cli_commands;
                    prepare_launch_inputs(&params.request, &mut cli_commands)?;
                    let report = execute_resume_member_pipeline(
                        state.as_ref(),
                        &params.request,
                        &cli_commands,
                        &params.tmux_layout,
                        Some(&mut |progress| {
                            let event = crate::commands::coordination::resume_member_progress_event_for_stage(
                                &params.request.team_name,
                                progress.stage,
                                progress.status,
                                progress.message,
                            );
                            crate::commands::coordination::emit_progress_log_event(&event);
                            let _ = registry.record_step(&run_id_for_task, event.progress);
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                    finalize_member_state(
                        state.teams_dir(),
                        &report.team_name,
                        &report.member_name,
                        params.operational_snapshot.as_ref(),
                    )
                    .map_err(|error| error.to_string())?;
                    Ok::<_, String>(report)
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry.complete(
                            &run_id_for_task,
                            CoordinationRunReport::ResumeMember(report),
                        );
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry.fail(
                            &run_id_for_task,
                            "resume-member worker panicked".to_string(),
                        );
                    }
                }
            });
        finish_spawn(self, run_id, spawn_result, "resume-member")
    }

    pub(crate) fn start_stop_member(
        &self,
        params: CoordinationStopMemberParams,
    ) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::StopMember);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!("coordination-stop-{}", &run_id[5..13]))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let report = execute_stop_member_pipeline(state.as_ref(), &params.request)
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
                            .complete(&run_id_for_task, CoordinationRunReport::StopMember(report));
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error);
                    }
                    Err(_) => {
                        let _ = registry
                            .fail(&run_id_for_task, "stop-member worker panicked".to_string());
                    }
                }
            });
        finish_spawn(self, run_id, spawn_result, "stop-member")
    }

    pub(crate) fn add_agent_status(&self, run_id: &str) -> Option<CoordinationAddAgentStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::AddAgent {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationAddAgentOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::AddAgent(report),
            } => CoordinationAddAgentOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationAddAgentOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationAddAgentStatus {
            run_id: status.run_id,
            steps: status.steps,
            outcome,
        })
    }

    pub(crate) fn resume_member_status(
        &self,
        run_id: &str,
    ) -> Option<CoordinationResumeMemberStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::ResumeMember {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationResumeMemberOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::ResumeMember(report),
            } => CoordinationResumeMemberOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationResumeMemberOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationResumeMemberStatus {
            run_id: status.run_id,
            steps: status.steps,
            outcome,
        })
    }

    pub(crate) fn stop_member_status(&self, run_id: &str) -> Option<CoordinationStopMemberStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::StopMember {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => CoordinationStopMemberOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::StopMember(report),
            } => CoordinationStopMemberOutcome::Completed { report },
            RunOutcome::Failed { error } => CoordinationStopMemberOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(CoordinationStopMemberStatus {
            run_id: status.run_id,
            steps: status.steps,
            outcome,
        })
    }
}

fn finish_spawn(
    service: &MemberOperationsService,
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

fn prepare_add_launch_inputs(
    teams_dir: &std::path::Path,
    request: &AddAgentRequest,
    commands: &mut CliCommandSettings,
) -> Result<(), String> {
    let tool = CliTool::from_alias(&request.agent.cli_tool).map_err(|error| error.to_string())?;
    let has_managed_codex = crate::session_scanner::cli_tool::spec(tool)
        .capabilities
        .hook_trust;
    prepare_daemon_launch_inputs_for_tools(teams_dir, has_managed_codex, vec![tool], commands);
    Ok(())
}

fn prepare_resume_launch_inputs(
    teams_dir: &std::path::Path,
    request: &ResumeMemberRequest,
    commands: &mut CliCommandSettings,
) -> Result<(), String> {
    let config = crate::coordination::stores::TeamConfigStore::load(teams_dir, &request.team_name)
        .map_err(|error| error.to_string())?;
    let tool = config
        .members
        .iter()
        .find(|member| member.name == request.member_name)
        .map(|member| member.cli_tool)
        .ok_or_else(|| {
            format!(
                "member '{}' not found in team '{}'",
                request.member_name, request.team_name
            )
        })?;
    let has_managed_codex = config.members.iter().any(|member| {
        crate::session_scanner::cli_tool::spec(member.cli_tool)
            .capabilities
            .hook_trust
    });
    prepare_daemon_launch_inputs_for_tools(teams_dir, has_managed_codex, vec![tool], commands);
    Ok(())
}

#[derive(Debug)]
pub(crate) struct ResumeMemberProgress {
    pub(crate) stage: crate::coordination::requests::MemberActivationStage,
    pub(crate) status: crate::coordination::requests::StepStatus,
    pub(crate) message: Option<String>,
}

pub(crate) fn execute_add_agent_pipeline(
    state: &CoordinationState,
    request: &AddAgentRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
) -> Result<AddAgentReport, crate::coordination::errors::CoordinationError> {
    state.with_orchestrator(|orchestrator| {
        orchestrator.add_agent_to_team_with_cli_commands_and_layout(
            request,
            cli_commands,
            tmux_layout,
        )
    })
}

pub(crate) fn execute_resume_member_pipeline(
    state: &CoordinationState,
    request: &ResumeMemberRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(ResumeMemberProgress)>,
) -> Result<ResumeAgentReport, crate::coordination::errors::CoordinationError> {
    state.with_orchestrator(|orchestrator| {
        orchestrator.resume_member_with_cli_commands_and_layout_and_progress(
            request,
            cli_commands,
            tmux_layout,
            1,
            1,
            Some(&mut |_, _, _, stage, status, message| {
                if let Some(emit) = emit.as_deref_mut() {
                    emit(ResumeMemberProgress {
                        stage,
                        status,
                        message,
                    });
                }
            }),
        )
    })
}

pub(crate) fn execute_stop_member_pipeline(
    state: &CoordinationState,
    request: &StopMemberRequest,
) -> Result<StopMemberReport, crate::coordination::errors::CoordinationError> {
    let result = state.with_orchestrator(|orchestrator| {
        orchestrator.remove_member(&request.team_name, &request.member_name, None)
    })?;
    let steps = result
        .steps
        .into_iter()
        .map(|step| StepProgress {
            step: step.step,
            status: if step.success {
                crate::coordination::requests::StepStatus::Succeeded
            } else {
                crate::coordination::requests::StepStatus::Failed
            },
            message: step.message,
        })
        .collect::<Vec<_>>();
    let warning_count = result.warnings.len();
    let message = if warning_count == 0 {
        "member removed".to_string()
    } else {
        format!(
            "member removed with {warning_count} warning{}",
            if warning_count == 1 { "" } else { "s" }
        )
    };
    Ok(StopMemberReport {
        team_name: result.team_name,
        member_name: result.member_name,
        removed: result.removed,
        message,
        steps,
        warnings: result.warnings,
    })
}

fn finalize_member_state(
    teams_dir: &std::path::Path,
    team_name: &str,
    member_name: &str,
    snapshot: Option<&crate::coordination::stores::OperationalContextSnapshot>,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    if let Some(snapshot) = snapshot {
        let belongs_to_member =
            snapshot.team_name == team_name && snapshot.member_name == member_name;
        if belongs_to_member {
            crate::coordination::operational_context::publish_initialize_snapshot(
                teams_dir, snapshot,
            )?;
        } else {
            tracing::warn!(
                team = team_name,
                member = member_name,
                snapshot_team = %snapshot.team_name,
                snapshot_member = %snapshot.member_name,
                "skipping an operational snapshot that does not belong to the completed member run"
            );
        }
    }
    crate::coordination::stores::active_project::sync_team_from_config(teams_dir, team_name)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::MemberOperationsService;
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::requests::{
        AddAgentRequest, AgentDefinition, StepStatus, StopMemberRequest,
    };
    use crate::coordination::runtime::{CoordinationRuntime, RecordingCoordinationRuntime};
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::{MemberRuntimeStore, TeamConfigStore};
    use crate::daemon::coordination_runs::CoordinationRunRegistry;
    use crate::daemon::protocol::{
        CoordinationAddAgentOutcome, CoordinationAddAgentParams, CoordinationResumeMemberOutcome,
        CoordinationResumeMemberParams, CoordinationStopMemberOutcome,
        CoordinationStopMemberParams,
    };
    use crate::models::CliCommandSettings;

    fn state(root: &std::path::Path) -> Arc<CoordinationState> {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(root);
        let runtime_for_factory = runtime.clone();
        Arc::new(CoordinationState::with_components_and_runtime(
            root.to_path_buf(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ))
    }

    fn agent(name: &str, project: &std::path::Path) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
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

    fn snapshot(
        team_name: &str,
        member_name: &str,
        project: &std::path::Path,
    ) -> crate::coordination::stores::OperationalContextSnapshot {
        crate::coordination::stores::OperationalContextSnapshot {
            version: 1,
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            updated_at: chrono::Utc::now(),
            task: Default::default(),
            assignment_footer: Default::default(),
            ownership: Default::default(),
            working_set: crate::coordination::stores::OperationalWorkingSetSnapshot {
                project_path: project.display().to_string(),
                focal_files: Vec::new(),
            },
        }
    }

    fn service(state: Arc<CoordinationState>) -> MemberOperationsService {
        MemberOperationsService::with_state_and_prepare(
            state,
            CoordinationRunRegistry::default(),
            Arc::new(|_request, _commands| Ok(())),
            Arc::new(|_request, _commands| Ok(())),
        )
    }

    fn wait_add(
        service: &MemberOperationsService,
        run_id: &str,
    ) -> crate::daemon::protocol::CoordinationAddAgentStatus {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = service.add_agent_status(run_id).expect("run registered");
            if status.outcome != CoordinationAddAgentOutcome::Running {
                return status;
            }
            assert!(Instant::now() < deadline, "add-agent run did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn add_agent_executes_in_daemon_state_and_publishes_fat_intent_snapshot() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let state = state(temp.path());
        state
            .with_orchestrator(|orchestrator| orchestrator.create_team("arch", None))
            .expect("create team");
        let service = service(state);
        let project = temp.path().join("builder");

        let run_id = service
            .start_add_agent(CoordinationAddAgentParams {
                request: AddAgentRequest {
                    team_name: "arch".to_string(),
                    agent: agent("builder", &project),
                },
                cli_commands: CliCommandSettings::default(),
                tmux_layout: "new_window".to_string(),
                operational_snapshot: Some(snapshot("arch", "builder", &project)),
            })
            .expect("daemon worker starts");
        let status = wait_add(&service, &run_id);

        let CoordinationAddAgentOutcome::Completed { report } = status.outcome else {
            panic!("add-agent should complete: {:?}", status.outcome);
        };
        assert!(report.failed_step.is_none(), "{report:?}");
        assert_eq!(status.steps.len(), report.steps.len() * 2);
        assert!(TeamConfigStore::load(temp.path(), "arch")
            .expect("team config")
            .members
            .iter()
            .any(|member| member.name == "builder"));
        assert!(
            crate::coordination::stores::OperationalContextSnapshotStore::load(
                temp.path(),
                "arch",
                "builder",
            )
            .expect("snapshot load")
            .is_some()
        );
        assert_eq!(
            crate::coordination::stores::ActiveProjectTeamStore::load_active_team(
                temp.path(),
                &project.display().to_string(),
            )
            .expect("active project mapping"),
            Some("arch".to_string())
        );
    }

    #[test]
    fn resume_member_executes_in_daemon_state_and_streams_canonical_steps() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let state = state(temp.path());
        state
            .with_orchestrator(|orchestrator| orchestrator.create_team("arch", None))
            .expect("create team");
        let project = temp.path().join("builder");
        let service = service(state);
        let add_id = service
            .start_add_agent(CoordinationAddAgentParams {
                request: AddAgentRequest {
                    team_name: "arch".to_string(),
                    agent: agent("builder", &project),
                },
                cli_commands: CliCommandSettings::default(),
                tmux_layout: "new_window".to_string(),
                operational_snapshot: None,
            })
            .expect("add worker starts");
        let added = wait_add(&service, &add_id);
        assert!(matches!(
            added.outcome,
            CoordinationAddAgentOutcome::Completed { .. }
        ));
        let mut runtime =
            MemberRuntimeStore::load(temp.path(), "arch", "builder").expect("runtime record");
        runtime.health = crate::coordination::domain::HealthState::SessionDead;
        runtime.pane_id = None;
        runtime.daemon_pid = None;
        MemberRuntimeStore::save(temp.path(), "arch", "builder", &runtime)
            .expect("save stopped runtime");

        let run_id = service
            .start_resume_member(CoordinationResumeMemberParams {
                request: crate::coordination::requests::ResumeMemberRequest {
                    team_name: "arch".to_string(),
                    member_name: "builder".to_string(),
                    reasoning_effort_override: None,
                },
                cli_commands: CliCommandSettings::default(),
                tmux_layout: "new_window".to_string(),
                operational_snapshot: Some(snapshot("arch", "builder", &project)),
            })
            .expect("resume worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            let status = service
                .resume_member_status(&run_id)
                .expect("run registered");
            if status.outcome != CoordinationResumeMemberOutcome::Running {
                break status;
            }
            assert!(Instant::now() < deadline, "resume run did not finish");
            std::thread::sleep(Duration::from_millis(5));
        };

        let CoordinationResumeMemberOutcome::Completed { report } = status.outcome else {
            panic!("resume should complete: {:?}", status.outcome);
        };
        assert!(report.resumed, "{report:?}");
        assert!(status
            .steps
            .iter()
            .any(|step| { step.step == "prepare_member" && step.status == StepStatus::Running }));
        assert!(status
            .steps
            .iter()
            .any(|step| { step.step == "commit_runtime" && step.status == StepStatus::Succeeded }));
    }

    #[test]
    fn stop_member_executes_in_daemon_state_and_clears_project_mapping() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let state = state(temp.path());
        let project = temp.path().join("builder");
        state
            .with_orchestrator(|orchestrator| {
                orchestrator.create_team("arch", None)?;
                orchestrator.add_member(
                    "arch",
                    Member {
                        name: "builder".to_string(),
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
                        project_path: project.clone(),
                        cli_tool: crate::session_scanner::cli_tool::CliTool::Codex,
                        extra: Default::default(),
                    },
                )
            })
            .expect("prepare member");
        crate::coordination::stores::active_project::sync_team_from_config(temp.path(), "arch")
            .expect("seed active mapping");
        let service = service(state);

        let run_id = service
            .start_stop_member(CoordinationStopMemberParams {
                request: StopMemberRequest {
                    team_name: "arch".to_string(),
                    member_name: "builder".to_string(),
                },
            })
            .expect("stop worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            let status = service.stop_member_status(&run_id).expect("run registered");
            if status.outcome != CoordinationStopMemberOutcome::Running {
                break status;
            }
            assert!(Instant::now() < deadline, "stop run did not finish");
            std::thread::sleep(Duration::from_millis(5));
        };

        let CoordinationStopMemberOutcome::Completed { report } = status.outcome else {
            panic!("stop should complete: {:?}", status.outcome);
        };
        assert!(report.removed, "{report:?}");
        assert!(!TeamConfigStore::load(temp.path(), "arch")
            .expect("team config")
            .members
            .iter()
            .any(|member| member.name == "builder"));
        assert_eq!(
            crate::coordination::stores::ActiveProjectTeamStore::load_active_team(
                temp.path(),
                &project.display().to_string(),
            )
            .expect("active project mapping"),
            None
        );
    }
}
