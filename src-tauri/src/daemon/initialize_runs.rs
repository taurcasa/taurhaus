//! Daemon-owned host for asynchronous team initialization.

use std::path::Path;
use std::sync::Arc;

use crate::coordination::requests::{InitializeReport, StepProgress};
use crate::coordination::state::CoordinationState;
use crate::daemon::coordination_runs::{
    prepare_daemon_launch_inputs_for_tools, CoordinationRunKind, CoordinationRunRegistry,
    CoordinationRunReport, RunOutcome,
};
use crate::daemon::protocol::CoordinationInitializeParams;
pub(crate) use crate::daemon::protocol::{
    CoordinationInitializeOutcome as InitializeRunOutcome,
    CoordinationInitializeStatus as InitializeRunStatus,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

type PrepareLaunchInputs = dyn Fn(&crate::coordination::requests::InitializeTeamRequest, &mut CliCommandSettings)
    + Send
    + Sync;

/// Daemon-local host for the existing initialization pipeline.
#[derive(Clone)]
pub(crate) struct InitializeTeamService {
    registry: CoordinationRunRegistry,
    state: Arc<CoordinationState>,
    prepare_launch_inputs: Arc<PrepareLaunchInputs>,
}

impl InitializeTeamService {
    pub(crate) fn for_process_default(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
    ) -> Self {
        let teams_dir = state.teams_dir().clone();
        Self::with_state_and_prepare(
            state,
            registry,
            Arc::new(move |request, commands| {
                prepare_daemon_launch_inputs(&teams_dir, request, commands)
            }),
        )
    }

    fn with_state_and_prepare(
        state: Arc<CoordinationState>,
        registry: CoordinationRunRegistry,
        prepare_launch_inputs: Arc<PrepareLaunchInputs>,
    ) -> Self {
        Self {
            registry,
            state,
            prepare_launch_inputs,
        }
    }

    pub(crate) fn start(&self, params: CoordinationInitializeParams) -> Result<String, String> {
        let run_id = self.registry.start(CoordinationRunKind::InitializeTeam);
        let run_id_for_task = run_id.clone();
        let registry = self.registry.clone();
        let state = self.state.clone();
        let prepare_launch_inputs = self.prepare_launch_inputs.clone();
        let spawn_result = std::thread::Builder::new()
            .name(format!("coordination-init-{}", &run_id[5..13]))
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut cli_commands = params.cli_commands;
                    prepare_launch_inputs(&params.request, &mut cli_commands);
                    let previous_root = state.team_teams_dir(&params.request.team_name)?;
                    let target_root =
                        selected_team_root(&params.request, &cli_commands, state.teams_dir())?;
                    pin_team_root_selector(&params.request, &target_root, &mut cli_commands)?;
                    ensure_team_name_available_at_root(
                        state.as_ref(),
                        &params.request.team_name,
                        &target_root,
                    )?;
                    let execution = execute_initialize_pipeline_at_root(
                        state.as_ref(),
                        &target_root,
                        &params.request,
                        &cli_commands,
                        &params.tmux_layout,
                        Some(&mut |progress| {
                            emit_initialize_step_log(&params.request.team_name, &progress);
                            let _ = registry.record_step(&run_id_for_task, progress);
                        }),
                    );
                    match execution {
                        Ok(report) if report.failed_step.is_none() => {
                            if let Err(error) = state
                                .team_root_registry()
                                .set(&params.request.team_name, &target_root)
                            {
                                if let Err(cleanup_error) =
                                    state.with_root_orchestrator(&target_root, |orchestrator| {
                                        orchestrator
                                            .disband_team(&params.request.team_name, None)
                                            .map(|_| ())
                                    })
                                {
                                    tracing::warn!(
                                        team = %params.request.team_name,
                                        root = %target_root.display(),
                                        error = %cleanup_error,
                                        "failed to clean up team after registry commit failure"
                                    );
                                }
                                state
                                    .team_root_registry()
                                    .set(&params.request.team_name, &previous_root)?;
                                return Err(error);
                            }
                            Ok((report, target_root))
                        }
                        Ok(report) => Ok((report, target_root)),
                        Err(error) => Err(error),
                    }
                }));
                match result {
                    Ok(Ok((report, target_root))) => {
                        let finalize = if report.failed_step.is_none() {
                            finalize_initialize_state(
                                &target_root,
                                &report.team_name,
                                &params.operational_snapshots,
                            )
                        } else {
                            Ok(())
                        };
                        match finalize {
                            Ok(()) => {
                                let _ = registry.complete(
                                    &run_id_for_task,
                                    CoordinationRunReport::Initialize(report),
                                );
                            }
                            Err(error) => {
                                let _ = registry.fail(&run_id_for_task, error.to_string());
                            }
                        }
                    }
                    Ok(Err(error)) => {
                        let _ = registry.fail(&run_id_for_task, error.to_string());
                    }
                    Err(_) => {
                        let _ = registry.fail(
                            &run_id_for_task,
                            "team initialization worker panicked".to_string(),
                        );
                    }
                }
            });
        match spawn_result {
            Ok(_) => Ok(run_id),
            Err(error) => {
                let message = format!("failed to start team initialization worker: {error}");
                let _ = self.registry.fail(&run_id, message.clone());
                Err(message)
            }
        }
    }

    pub(crate) fn status(&self, run_id: &str) -> Option<InitializeRunStatus> {
        let status = self.registry.status(run_id)?;
        if status.kind != CoordinationRunKind::InitializeTeam {
            return None;
        }
        let outcome = match status.outcome {
            RunOutcome::Running => InitializeRunOutcome::Running,
            RunOutcome::Completed {
                report: CoordinationRunReport::Initialize(report),
            } => InitializeRunOutcome::Completed { report },
            RunOutcome::Failed { error } => InitializeRunOutcome::Failed { error },
            RunOutcome::Completed { .. } => return None,
        };
        Some(InitializeRunStatus {
            run_id: status.run_id,
            steps: status.steps,
            outcome,
        })
    }
}

fn ensure_team_name_available_at_root(
    state: &CoordinationState,
    team_name: &str,
    target_root: &Path,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    for root in state.team_root_registry().roots()? {
        if root == target_root {
            continue;
        }
        match crate::coordination::stores::TeamConfigStore::load(&root, team_name) {
            Ok(_) => {
                return Err(crate::coordination::errors::CoordinationError::Conflict(
                    format!(
                        "team '{team_name}' already exists under another account root ({})",
                        root.display()
                    ),
                ));
            }
            Err(crate::coordination::errors::CoordinationError::NotFound(_)) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn selected_team_root(
    request: &crate::coordination::requests::InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    default_teams_dir: &Path,
) -> Result<std::path::PathBuf, crate::coordination::errors::CoordinationError> {
    let mut requested = std::collections::BTreeSet::new();
    let mut root_tool = None;
    for member in std::iter::once(&request.lead).chain(request.agents.iter()) {
        let Ok(tool) = CliTool::from_alias(&member.cli_tool) else {
            continue;
        };
        if !crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .team_config_namespace
        {
            continue;
        }
        root_tool = Some(tool);
        if let Some(account_id) = member
            .account_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            requested.insert(account_id.to_string());
        }
    }
    if requested.len() > 1 {
        return Err(crate::coordination::errors::CoordinationError::Validation(
            "a Claude team must use one team account".to_string(),
        ));
    }
    let Some(account_id) = requested.into_iter().next() else {
        return Ok(default_teams_dir.to_path_buf());
    };
    let tool = root_tool.expect("a requested team account has a namespace tool");
    let account = cli_commands
        .managed_accounts
        .get(&tool)
        .into_iter()
        .flatten()
        .find(|account| account.id == account_id && account.logged_in)
        .ok_or_else(|| {
            crate::coordination::errors::CoordinationError::Validation(format!(
                "account '{account_id}' is unavailable or signed out"
            ))
        })?;
    Ok(account.dir.join("teams"))
}

fn pin_team_root_selector(
    request: &crate::coordination::requests::InitializeTeamRequest,
    teams_dir: &Path,
    cli_commands: &mut CliCommandSettings,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    let Some(account_dir) = teams_dir.parent() else {
        return Err(crate::coordination::errors::CoordinationError::Validation(
            format!(
                "team root '{}' has no account directory",
                teams_dir.display()
            ),
        ));
    };
    for member in std::iter::once(&request.lead).chain(request.agents.iter()) {
        let Ok(tool) = CliTool::from_alias(&member.cli_tool) else {
            continue;
        };
        let capabilities = crate::session_scanner::cli_tool::spec(tool).capabilities;
        if capabilities.team_config_namespace {
            if let Some(selector) = capabilities.account_selector {
                cli_commands
                    .account_selector_dirs
                    .insert(selector.to_string(), account_dir.to_path_buf());
            }
        }
    }
    Ok(())
}

fn finalize_initialize_state(
    teams_dir: &Path,
    team_name: &str,
    operational_snapshots: &[crate::coordination::stores::OperationalContextSnapshot],
) -> Result<(), crate::coordination::errors::CoordinationError> {
    let config = crate::coordination::stores::TeamConfigStore::load(teams_dir, team_name)?;
    for snapshot in operational_snapshots {
        let belongs_to_team = snapshot.team_name == team_name
            && config
                .members
                .iter()
                .any(|member| member.name == snapshot.member_name);
        if !belongs_to_team {
            // Snapshots derive from the very request that just initialized
            // this team, so a mismatch is unreachable in practice — and a
            // successful pipeline must never be reported as a failed
            // initialization over a skippable snapshot.
            tracing::warn!(
                member = %snapshot.member_name,
                team = team_name,
                "skipping an operational snapshot that does not belong to the initialized team"
            );
            continue;
        }
        crate::coordination::operational_context::publish_initialize_snapshot(teams_dir, snapshot)?;
    }
    crate::coordination::stores::active_project::sync_team_from_config(teams_dir, team_name)
}

pub(crate) fn execute_initialize_pipeline(
    state: &CoordinationState,
    request: &crate::coordination::requests::InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    emit: Option<&mut dyn FnMut(StepProgress)>,
) -> Result<InitializeReport, crate::coordination::errors::CoordinationError> {
    let teams_dir = state.team_teams_dir(&request.team_name)?;
    execute_initialize_pipeline_at_root(state, &teams_dir, request, cli_commands, tmux_layout, emit)
}

fn execute_initialize_pipeline_at_root(
    state: &CoordinationState,
    teams_dir: &Path,
    request: &crate::coordination::requests::InitializeTeamRequest,
    cli_commands: &CliCommandSettings,
    tmux_layout: &str,
    mut emit: Option<&mut dyn FnMut(StepProgress)>,
) -> Result<InitializeReport, crate::coordination::errors::CoordinationError> {
    state.with_root_orchestrator(teams_dir, |orchestrator| {
        orchestrator.initialize_team_with_cli_commands_and_layout_and_progress(
            request,
            cli_commands,
            tmux_layout,
            Some(&mut |step, status, message| {
                if let Some(emit) = emit.as_deref_mut() {
                    emit(StepProgress {
                        step: step.to_string(),
                        status,
                        message,
                    });
                }
            }),
        )
    })
}

fn prepare_daemon_launch_inputs(
    teams_dir: &Path,
    request: &crate::coordination::requests::InitializeTeamRequest,
    commands: &mut CliCommandSettings,
) {
    let has_codex = std::iter::once(&request.lead)
        .chain(request.agents.iter())
        .filter_map(|member| CliTool::from_alias(&member.cli_tool).ok())
        .any(|tool| {
            crate::session_scanner::cli_tool::spec(tool)
                .capabilities
                .hook_trust
        });
    let tools = std::iter::once(&request.lead)
        .chain(request.agents.iter())
        .filter_map(|member| {
            CliTool::from_alias(&member.cli_tool)
                .ok()
                .map(|tool| (tool, member.account_id.clone()))
        })
        .collect::<Vec<_>>();
    prepare_daemon_launch_inputs_for_tools(teams_dir, has_codex, tools, commands);
}

fn emit_initialize_step_log(team_name: &str, progress: &StepProgress) {
    crate::commands::coordination::emit_progress_log_event(
        &crate::commands::coordination::StepProgressEvent {
            team_name: team_name.to_string(),
            operation: "initialize_team".to_string(),
            progress: progress.clone(),
            canonical_stages: crate::coordination::requests::canonical_member_activation_stages(
                "initialize",
                &progress.step,
            )
            .to_vec(),
        },
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{InitializeRunOutcome, InitializeTeamService};
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::requests::{
        AgentDefinition, InitializeTeamRequest, LeadMode, StepStatus,
    };
    use crate::coordination::runtime::{
        CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
    };
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::TeamConfigStore;
    use crate::daemon::coordination_runs::CoordinationRunRegistry;
    use crate::daemon::protocol::CoordinationInitializeParams;
    use crate::models::CliCommandSettings;

    fn agent(name: &str, project: &str) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
            account_id: None,
            project_id: project.to_string(),
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

    fn initialize_params(project: &std::path::Path) -> CoordinationInitializeParams {
        let builder_project = project.join("builder").display().to_string();
        CoordinationInitializeParams {
            request: InitializeTeamRequest {
                team_name: "daemon-init".to_string(),
                team_description: Some("daemon pipeline test".to_string()),
                lead_mode: LeadMode::LaunchNew,
                lead: agent("team-lead", &project.join("lead").display().to_string()),
                agents: vec![agent("builder", &builder_project)],
            },
            cli_commands: CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
            operational_snapshots: vec![crate::coordination::stores::OperationalContextSnapshot {
                version: 1,
                team_name: "daemon-init".to_string(),
                member_name: "builder".to_string(),
                updated_at: chrono::Utc::now(),
                task: Default::default(),
                assignment_footer: Default::default(),
                ownership: Default::default(),
                working_set: crate::coordination::stores::OperationalWorkingSetSnapshot {
                    project_path: builder_project,
                    focal_files: Vec::new(),
                },
            }],
        }
    }

    #[test]
    fn initialize_request_executes_the_pipeline_through_daemon_state() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let teams_dir = temp.path().join("teams");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(&teams_dir);
        let runtime_for_factory = runtime.clone();
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            teams_dir.clone(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ));
        let service = InitializeTeamService::with_state_and_prepare(
            state,
            CoordinationRunRegistry::default(),
            Arc::new(|_request, _commands| {}),
        );

        let run_id = service
            .start(initialize_params(temp.path()))
            .expect("daemon worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            let status = service.status(&run_id).expect("run remains registered");
            if status.outcome != InitializeRunOutcome::Running {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "daemon initialize did not finish"
            );
            std::thread::sleep(Duration::from_millis(5));
        };

        let InitializeRunOutcome::Completed { report } = status.outcome else {
            panic!("initialize should complete: {:?}", status.outcome);
        };
        assert!(report.failed_step.is_none(), "{report:?}");
        assert_eq!(
            status
                .steps
                .iter()
                .map(|step| (step.step.as_str(), step.status))
                .collect::<Vec<_>>(),
            vec![
                ("validate_configuration", StepStatus::Running),
                ("validate_configuration", StepStatus::Succeeded),
                ("create_team", StepStatus::Running),
                ("create_team", StepStatus::Succeeded),
                ("add_lead", StepStatus::Running),
                ("add_lead", StepStatus::Succeeded),
                ("create_panes", StepStatus::Running),
                ("create_panes", StepStatus::Succeeded),
                ("launch_sessions", StepStatus::Running),
                ("launch_sessions", StepStatus::Succeeded),
                ("join_mesh", StepStatus::Running),
                ("join_mesh", StepStatus::Succeeded),
                ("start_daemons", StepStatus::Running),
                ("start_daemons", StepStatus::Succeeded),
                ("send_onboarding", StepStatus::Running),
                ("send_onboarding", StepStatus::Succeeded),
            ]
        );
        assert_eq!(
            TeamConfigStore::load(&teams_dir, "daemon-init")
                .expect("team config")
                .members
                .len(),
            2
        );
        assert_eq!(
            crate::coordination::stores::ActiveProjectTeamStore::load_active_team(
                &teams_dir,
                &temp.path().join("lead").display().to_string(),
            )
            .expect("active project mapping"),
            Some("daemon-init".to_string())
        );
        assert_eq!(
            crate::coordination::stores::OperationalContextSnapshotStore::load(
                &teams_dir,
                "daemon-init",
                "builder",
            )
            .expect("operational snapshot load")
            .expect("daemon saved the fat-intent snapshot")
            .working_set
            .project_path,
            temp.path().join("builder").display().to_string()
        );
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::JoinMesh { team_name, .. } if team_name == "daemon-init"
        )));
    }

    #[test]
    fn initialize_places_a_claude_team_in_the_selected_account_root() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams = temp.path().join("default/teams");
        let work_account = temp.path().join("claude-work");
        let work_teams = work_account.join("teams");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(&work_teams);
        let runtime_for_factory = runtime.clone();
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            default_teams.clone(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ));
        let service = InitializeTeamService::with_state_and_prepare(
            state.clone(),
            CoordinationRunRegistry::default(),
            Arc::new(move |_request, commands| {
                commands.managed_accounts.insert(
                    crate::session_scanner::cli_tool::CliTool::Claude,
                    vec![crate::models::ManagedLaunchAccount {
                        id: "claude-work".to_string(),
                        label: "Work".to_string(),
                        dir: work_account.clone(),
                        logged_in: true,
                        is_default: false,
                    }],
                );
            }),
        );
        let mut params = initialize_params(temp.path());
        params.request.lead.cli_tool = "claude".to_string();
        params.request.lead.model = "opus".to_string();
        params.request.lead.account_id = Some("claude-work".to_string());

        let run_id = service.start(params).expect("daemon worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = service.status(&run_id).expect("run remains registered");
            if status.outcome != InitializeRunOutcome::Running {
                assert!(
                    matches!(status.outcome, InitializeRunOutcome::Completed { .. }),
                    "{status:?}"
                );
                break;
            }
            assert!(Instant::now() < deadline, "daemon initialize timed out");
            std::thread::sleep(Duration::from_millis(5));
        }

        assert!(TeamConfigStore::load(&work_teams, "daemon-init").is_ok());
        assert!(!default_teams.join("daemon-init").exists());
        assert_eq!(
            state
                .team_teams_dir("daemon-init")
                .expect("registered root"),
            work_teams
        );
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::JoinMesh { claude_dir, .. }
                if claude_dir == &temp.path().join("claude-work").display().to_string()
        )));
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::SpawnDaemonAtRoot { member_name, claude_dir, .. }
                if member_name == "builder"
                    && claude_dir == &temp.path().join("claude-work").display().to_string()
        )));
    }

    #[test]
    fn initialize_rejects_a_same_named_team_in_another_root() {
        // Regression: 18810949 committed selected-root authority before create,
        // allowing a new team to shadow a same-named default-root team.
        let temp = tempfile::TempDir::new().expect("tempdir");
        let default_teams = temp.path().join("default/teams");
        let work_account = temp.path().join("claude-work");
        let work_teams = work_account.join("teams");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(&work_teams);
        let runtime_for_factory = runtime.clone();
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            default_teams.clone(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ));
        state
            .with_root_orchestrator(&default_teams, |orchestrator| {
                orchestrator.create_team("daemon-init", None).map(|_| ())
            })
            .expect("seed default-root team");

        let service = InitializeTeamService::with_state_and_prepare(
            state.clone(),
            CoordinationRunRegistry::default(),
            Arc::new(move |_request, commands| {
                commands.managed_accounts.insert(
                    crate::session_scanner::cli_tool::CliTool::Claude,
                    vec![crate::models::ManagedLaunchAccount {
                        id: "claude-work".to_string(),
                        label: "Work".to_string(),
                        dir: work_account.clone(),
                        logged_in: true,
                        is_default: false,
                    }],
                );
            }),
        );
        let mut params = initialize_params(temp.path());
        params.request.lead.cli_tool = "claude".to_string();
        params.request.lead.model = "opus".to_string();
        params.request.lead.account_id = Some("claude-work".to_string());

        let run_id = service.start(params).expect("daemon worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        let status = loop {
            let status = service.status(&run_id).expect("run remains registered");
            if status.outcome != InitializeRunOutcome::Running {
                break status;
            }
            assert!(Instant::now() < deadline, "daemon initialize timed out");
            std::thread::sleep(Duration::from_millis(5));
        };

        let InitializeRunOutcome::Failed { error } = status.outcome else {
            panic!("same-named cross-root initialize should fail: {status:?}");
        };
        assert!(error.contains("already exists"), "{error}");
        assert!(TeamConfigStore::load(&default_teams, "daemon-init").is_ok());
        assert!(!work_teams.join("daemon-init").exists());
        assert_eq!(
            state.team_teams_dir("daemon-init").expect("root authority"),
            default_teams
        );
    }

    // Regression: 3f8b44ae unconditionally published the pre-pipeline snapshot,
    // overwriting any fresher operational context written while init ran.
    #[test]
    fn initialize_finalization_preserves_a_newer_operational_snapshot() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let teams_dir = temp.path().join("teams");
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_mesh_join_teams_dir(&teams_dir);
        let runtime_for_factory = runtime.clone();
        let state = Arc::new(CoordinationState::with_components_and_runtime(
            teams_dir.clone(),
            BackendSelector::m0(),
            Arc::new(|_kind, _teams_dir| {
                Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
            }),
            Arc::new(move || runtime_for_factory.clone() as Arc<dyn CoordinationRuntime>),
        ));
        let service = InitializeTeamService::with_state_and_prepare(
            state,
            CoordinationRunRegistry::default(),
            Arc::new(|_request, _commands| {}),
        );
        let mut params = initialize_params(temp.path());
        let prepared = params.operational_snapshots.remove(0);

        let run_id = service.start(params).expect("daemon worker starts");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = service.status(&run_id).expect("run remains registered");
            if status.outcome != InitializeRunOutcome::Running {
                assert!(matches!(
                    status.outcome,
                    InitializeRunOutcome::Completed { .. }
                ));
                break;
            }
            assert!(
                Instant::now() < deadline,
                "daemon initialize did not finish"
            );
            std::thread::sleep(Duration::from_millis(5));
        }

        let mut newer = prepared.clone();
        newer.updated_at = prepared.updated_at + chrono::Duration::seconds(1);
        newer.working_set.focal_files = vec!["src/current.rs".to_string()];
        newer.ownership.override_allowed = true;
        crate::coordination::stores::OperationalContextSnapshotStore::save(&teams_dir, &newer)
            .expect("save newer snapshot");

        super::finalize_initialize_state(&teams_dir, "daemon-init", &[prepared])
            .expect("finalize initialization state");

        let saved = crate::coordination::stores::OperationalContextSnapshotStore::load(
            &teams_dir,
            "daemon-init",
            "builder",
        )
        .expect("load snapshot")
        .expect("snapshot exists");
        assert_eq!(saved.working_set.focal_files, vec!["src/current.rs"]);
        assert!(saved.ownership.override_allowed);
    }

    #[test]
    fn daemon_resolves_launch_bases_and_team_selector_dirs_locally() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let params = initialize_params(temp.path());
        let mut commands = params.cli_commands;
        let mut resolved = Vec::new();

        let tools = [crate::session_scanner::cli_tool::CliTool::Codex];
        crate::commands::accounts::apply_team_account_selector_dirs_with(
            &mut commands,
            tools,
            |tool| temp.path().join(format!("{tool}-home")),
        );
        crate::commands::accounts::apply_team_launch_base_resolutions_with(
            &mut commands,
            tools,
            |base, tool| {
                resolved.push((base.to_string(), tool));
                (
                    crate::session_scanner::launch_base::ResolvedBase {
                        command: format!("resolved-{base}"),
                        expansions: Vec::new(),
                        opaque_head: None,
                    },
                    true,
                )
            },
        );

        assert_eq!(resolved.len(), 2, "one tool has fresh and resume bases");
        assert_eq!(
            commands.account_selector_dirs.get("CODEX_HOME"),
            Some(&temp.path().join("codex-home"))
        );
        assert!(commands.resolved_bases.contains_key(&(
            crate::session_scanner::cli_tool::CliTool::Codex,
            crate::daemon::protocol::LaunchMode::Fresh,
        )));
        assert!(commands.resolved_bases.contains_key(&(
            crate::session_scanner::cli_tool::CliTool::Codex,
            crate::daemon::protocol::LaunchMode::Resume,
        )));
    }
}
