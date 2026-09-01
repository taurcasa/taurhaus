//! Daemon-owned lifecycle registry for asynchronous team initialization.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::coordination::requests::{InitializeReport, StepProgress};
use crate::coordination::state::CoordinationState;
pub(crate) use crate::daemon::protocol::{
    CoordinationInitializeOutcome as InitializeRunOutcome,
    CoordinationInitializeStatus as InitializeRunStatus,
};
use crate::daemon::protocol::{CoordinationInitializeParams, LaunchMode};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

pub(crate) const INITIALIZE_RUN_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug)]
struct InitializeRunRecord {
    steps: Vec<StepProgress>,
    outcome: InitializeRunOutcome,
    terminal_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct InitializeRunRegistry {
    records: Arc<Mutex<HashMap<String, InitializeRunRecord>>>,
    ttl: Duration,
}

impl Default for InitializeRunRegistry {
    fn default() -> Self {
        Self::with_ttl(INITIALIZE_RUN_TTL)
    }
}

impl InitializeRunRegistry {
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    pub(crate) fn start(&self) -> String {
        self.start_at(Instant::now())
    }

    fn start_at(&self, now: Instant) -> String {
        let run_id = format!("init_{}", uuid::Uuid::new_v4().simple());
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, now);
        records.insert(
            run_id.clone(),
            InitializeRunRecord {
                steps: Vec::new(),
                outcome: InitializeRunOutcome::Running,
                terminal_at: None,
            },
        );
        run_id
    }

    pub(crate) fn record_step(&self, run_id: &str, step: StepProgress) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "team initialization run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("team initialization run '{run_id}' was not found"))?;
        if record.outcome != InitializeRunOutcome::Running {
            return Err(format!(
                "team initialization run '{run_id}' is already terminal"
            ));
        }
        record.steps.push(step);
        Ok(())
    }

    pub(crate) fn complete(&self, run_id: &str, report: InitializeReport) -> Result<(), String> {
        self.complete_at(run_id, report, Instant::now())
    }

    fn complete_at(
        &self,
        run_id: &str,
        report: InitializeReport,
        now: Instant,
    ) -> Result<(), String> {
        self.finish_at(run_id, InitializeRunOutcome::Completed { report }, now)
    }

    pub(crate) fn fail(&self, run_id: &str, error: String) -> Result<(), String> {
        self.finish_at(
            run_id,
            InitializeRunOutcome::Failed { error },
            Instant::now(),
        )
    }

    fn finish_at(
        &self,
        run_id: &str,
        outcome: InitializeRunOutcome,
        now: Instant,
    ) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "team initialization run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("team initialization run '{run_id}' was not found"))?;
        if record.outcome != InitializeRunOutcome::Running {
            return Err(format!(
                "team initialization run '{run_id}' is already terminal"
            ));
        }
        record.outcome = outcome;
        record.terminal_at = Some(now);
        Ok(())
    }

    pub(crate) fn status(&self, run_id: &str) -> Option<InitializeRunStatus> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, Instant::now());
        records.get(run_id).map(|record| InitializeRunStatus {
            run_id: run_id.to_string(),
            steps: record.steps.clone(),
            outcome: record.outcome.clone(),
        })
    }

    fn prune_at(&self, now: Instant) {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, now);
    }

    fn prune_locked(
        records: &mut HashMap<String, InitializeRunRecord>,
        ttl: Duration,
        now: Instant,
    ) {
        records.retain(|_, record| {
            record
                .terminal_at
                .is_none_or(|terminal_at| now.saturating_duration_since(terminal_at) <= ttl)
        });
    }
}

type PrepareLaunchInputs = dyn Fn(&crate::coordination::requests::InitializeTeamRequest, &mut CliCommandSettings)
    + Send
    + Sync;

/// Daemon-local host for the existing initialization pipeline.
#[derive(Clone)]
pub(crate) struct InitializeTeamService {
    registry: InitializeRunRegistry,
    state: Arc<CoordinationState>,
    prepare_launch_inputs: Arc<PrepareLaunchInputs>,
}

impl InitializeTeamService {
    pub(crate) fn for_process_default(state: Arc<CoordinationState>) -> Self {
        let teams_dir = state.teams_dir().clone();
        Self::with_state_and_prepare(
            state,
            Arc::new(move |request, commands| {
                prepare_daemon_launch_inputs(&teams_dir, request, commands)
            }),
        )
    }

    fn with_state_and_prepare(
        state: Arc<CoordinationState>,
        prepare_launch_inputs: Arc<PrepareLaunchInputs>,
    ) -> Self {
        Self {
            registry: InitializeRunRegistry::default(),
            state,
            prepare_launch_inputs,
        }
    }

    pub(crate) fn start(&self, params: CoordinationInitializeParams) -> Result<String, String> {
        let run_id = self.registry.start();
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
                    state.with_orchestrator(|orchestrator| {
                        orchestrator.initialize_team_with_cli_commands_and_layout_and_progress(
                            &params.request,
                            &cli_commands,
                            &params.tmux_layout,
                            Some(&mut |step, status, message| {
                                let progress = StepProgress {
                                    step: step.to_string(),
                                    status,
                                    message,
                                };
                                emit_initialize_step_log(&params.request.team_name, &progress);
                                let _ = registry.record_step(&run_id_for_task, progress);
                            }),
                        )
                    })
                }));
                match result {
                    Ok(Ok(report)) => {
                        let _ = registry.complete(&run_id_for_task, report);
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
        self.registry.status(run_id)
    }
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
    let codex_bypass_hook_trust =
        crate::commands::terminal_settings::managed_codex_hook_trust_for_launch(
            teams_dir, has_codex,
        );
    crate::commands::terminal_settings::apply_managed_codex_launch_inputs(
        commands,
        has_codex,
        codex_bypass_hook_trust,
    );

    let probe = crate::session_scanner::launch_base::ShellAliasProbe::for_pane();
    apply_local_account_and_base_resolutions_with(
        request,
        commands,
        |base, tool| {
            crate::session_scanner::launch_base::resolve_base_command_cached(base, tool, &probe)
        },
        crate::provider::platform_paths::PlatformPaths::tool_home,
    );
}

fn apply_local_account_and_base_resolutions_with(
    request: &crate::coordination::requests::InitializeTeamRequest,
    commands: &mut CliCommandSettings,
    mut resolve: impl FnMut(&str, CliTool) -> crate::session_scanner::launch_base::ResolvedBase,
    mut tool_home: impl FnMut(CliTool) -> std::path::PathBuf,
) {
    let tools = std::iter::once(&request.lead)
        .chain(request.agents.iter())
        .filter_map(|member| CliTool::from_alias(&member.cli_tool).ok());
    let mut seen = std::collections::HashSet::new();
    for tool in tools.into_iter().filter(|tool| seen.insert(*tool)) {
        if let Some(selector) = crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .account_selector
        {
            commands
                .account_selector_dirs
                .entry(selector.to_string())
                .or_insert_with(|| tool_home(tool));
        }
        for mode in [LaunchMode::Fresh, LaunchMode::Resume] {
            let base =
                crate::session_scanner::launch::base_command(commands, tool, mode).to_string();
            let resolved = resolve(&base, tool);
            commands.resolved_bases.insert((tool, mode), resolved);
        }
    }
}

fn emit_initialize_step_log(team_name: &str, progress: &StepProgress) {
    let (level, event) = match progress.status {
        crate::coordination::requests::StepStatus::Pending => {
            ("debug", "coordination.step.pending")
        }
        crate::coordination::requests::StepStatus::Running => ("info", "coordination.step.started"),
        crate::coordination::requests::StepStatus::Succeeded => {
            ("info", "coordination.step.completed")
        }
        crate::coordination::requests::StepStatus::Failed => ("warn", "coordination.step.failed"),
    };
    let mut fields = serde_json::Map::new();
    fields.insert(
        "team_name".to_string(),
        serde_json::Value::String(team_name.to_string()),
    );
    fields.insert(
        "operation".to_string(),
        serde_json::Value::String("initialize_team".to_string()),
    );
    fields.insert(
        "step".to_string(),
        serde_json::Value::String(progress.step.clone()),
    );
    fields.insert(
        "status".to_string(),
        serde_json::Value::String(step_status_name(progress.status).to_string()),
    );
    if let Some(message) = progress.message.as_ref() {
        fields.insert(
            "message".to_string(),
            serde_json::Value::String(message.clone()),
        );
    }
    crate::commands::logging::emit_global(
        level,
        "backend",
        event,
        Some("Coordination step lifecycle event".to_string()),
        fields,
    );
}

fn step_status_name(status: crate::coordination::requests::StepStatus) -> &'static str {
    match status {
        crate::coordination::requests::StepStatus::Pending => "pending",
        crate::coordination::requests::StepStatus::Running => "running",
        crate::coordination::requests::StepStatus::Succeeded => "succeeded",
        crate::coordination::requests::StepStatus::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{InitializeRunOutcome, InitializeRunRegistry, InitializeTeamService};
    use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
    use crate::coordination::requests::{
        AgentDefinition, InitializeReport, InitializeTeamRequest, LeadMode, StepProgress,
        StepStatus,
    };
    use crate::coordination::runtime::{
        CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
    };
    use crate::coordination::state::CoordinationState;
    use crate::coordination::stores::TeamConfigStore;
    use crate::daemon::protocol::CoordinationInitializeParams;
    use crate::models::CliCommandSettings;

    fn progress(step: &str, status: StepStatus) -> StepProgress {
        StepProgress {
            step: step.to_string(),
            status,
            message: None,
        }
    }

    fn report() -> InitializeReport {
        InitializeReport {
            team_name: "daemon-init".to_string(),
            succeeded_steps: vec!["validate_configuration".to_string()],
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps: vec![progress("validate_configuration", StepStatus::Succeeded)],
        }
    }

    fn agent(name: &str, project: &str) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            cli_tool: "codex".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
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
        CoordinationInitializeParams {
            request: InitializeTeamRequest {
                team_name: "daemon-init".to_string(),
                team_description: Some("daemon pipeline test".to_string()),
                lead_mode: LeadMode::LaunchNew,
                lead: agent("team-lead", &project.join("lead").display().to_string()),
                agents: vec![agent(
                    "builder",
                    &project.join("builder").display().to_string(),
                )],
            },
            cli_commands: CliCommandSettings::default(),
            tmux_layout: "new_window".to_string(),
        }
    }

    #[test]
    fn run_lifecycle_mirrors_steps_and_completed_outcome() {
        let registry = InitializeRunRegistry::with_ttl(Duration::from_secs(600));
        let run_id = registry.start();

        registry
            .record_step(
                &run_id,
                progress("validate_configuration", StepStatus::Running),
            )
            .expect("running step recorded");
        registry
            .record_step(
                &run_id,
                progress("validate_configuration", StepStatus::Succeeded),
            )
            .expect("terminal step recorded");

        let running = registry.status(&run_id).expect("running status");
        assert_eq!(running.steps.len(), 2);
        assert_eq!(running.outcome, InitializeRunOutcome::Running);

        registry.complete(&run_id, report()).expect("run completed");
        let completed = registry.status(&run_id).expect("completed status");
        assert_eq!(
            completed.outcome,
            InitializeRunOutcome::Completed { report: report() }
        );
    }

    #[test]
    fn failed_run_preserves_steps_and_terminal_error() {
        let registry = InitializeRunRegistry::with_ttl(Duration::from_secs(600));
        let run_id = registry.start();
        registry
            .record_step(&run_id, progress("create_team", StepStatus::Running))
            .expect("step recorded");
        registry
            .fail(&run_id, "pipeline panicked".to_string())
            .expect("run failed");

        let status = registry.status(&run_id).expect("failed status");
        assert_eq!(status.steps.len(), 1);
        assert_eq!(
            status.outcome,
            InitializeRunOutcome::Failed {
                error: "pipeline panicked".to_string()
            }
        );
    }

    #[test]
    fn terminal_runs_expire_after_ttl_but_running_runs_do_not() {
        let registry = InitializeRunRegistry::with_ttl(Duration::from_secs(10));
        let started_at = Instant::now();
        let running_id = registry.start_at(started_at);
        let completed_id = registry.start_at(started_at);
        registry
            .complete_at(&completed_id, report(), started_at)
            .expect("run completed");

        registry.prune_at(started_at + Duration::from_secs(11));

        assert!(registry.status(&completed_id).is_none());
        assert!(registry.status(&running_id).is_some());
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
        assert!(runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::JoinMesh { team_name, .. } if team_name == "daemon-init"
        )));
    }

    #[test]
    fn daemon_resolves_launch_bases_and_team_selector_dirs_locally() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        let params = initialize_params(temp.path());
        let mut commands = params.cli_commands;
        let mut resolved = Vec::new();

        super::apply_local_account_and_base_resolutions_with(
            &params.request,
            &mut commands,
            |base, tool| {
                resolved.push((base.to_string(), tool));
                crate::session_scanner::launch_base::ResolvedBase {
                    command: format!("resolved-{base}"),
                    expansions: Vec::new(),
                    opaque_head: None,
                }
            },
            |tool| temp.path().join(format!("{tool}-home")),
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
