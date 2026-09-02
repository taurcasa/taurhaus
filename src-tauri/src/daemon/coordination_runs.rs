//! Shared lifecycle registry for daemon-owned interactive coordination runs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::coordination::requests::{
    AddAgentReport, DeliveryResult, DisbandTeamReport, InitializeReport, ResumeAgentReport,
    ResumeTeamProgress, ResumeTeamReport, StepProgress, StopMemberReport,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

pub(crate) const COORDINATION_RUN_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoordinationRunKind {
    InitializeTeam,
    AddAgent,
    ResumeMember,
    ResumeTeam,
    Reonboard,
    CreateTeam,
    DisbandTeam,
    AddMember,
    RemoveMember,
    ApplyTaskEffort,
}

impl CoordinationRunKind {
    fn id_prefix(self) -> &'static str {
        match self {
            Self::InitializeTeam => "init",
            Self::AddAgent => "add",
            Self::ResumeMember => "resume",
            Self::ResumeTeam => "team-resume",
            Self::Reonboard => "reonboard",
            Self::CreateTeam => "create",
            Self::DisbandTeam => "disband",
            Self::AddMember => "member-add",
            Self::RemoveMember => "member-remove",
            Self::ApplyTaskEffort => "effort",
        }
    }

    fn operation_name(self) -> &'static str {
        match self {
            Self::InitializeTeam => "initialize_team",
            Self::AddAgent => "add_agent",
            Self::ResumeMember => "resume_member",
            Self::ResumeTeam => "resume_team",
            Self::Reonboard => "reonboard",
            Self::CreateTeam => "create_team",
            Self::DisbandTeam => "disband_team",
            Self::AddMember => "add_member",
            Self::RemoveMember => "remove_member",
            Self::ApplyTaskEffort => "apply_task_effort",
        }
    }

    fn accepts(self, report: &CoordinationRunReport) -> bool {
        matches!(
            (self, report),
            (Self::InitializeTeam, CoordinationRunReport::Initialize(_))
                | (Self::AddAgent, CoordinationRunReport::AddAgent(_))
                | (Self::ResumeMember, CoordinationRunReport::ResumeMember(_))
                | (Self::ResumeTeam, CoordinationRunReport::ResumeTeam(_))
                | (Self::Reonboard, CoordinationRunReport::Reonboard(_))
                | (Self::CreateTeam, CoordinationRunReport::CreateTeam)
                | (Self::DisbandTeam, CoordinationRunReport::DisbandTeam(_))
                | (Self::AddMember, CoordinationRunReport::AddMember)
                | (Self::RemoveMember, CoordinationRunReport::RemoveMember(_))
                | (
                    Self::ApplyTaskEffort,
                    CoordinationRunReport::ApplyTaskEffort(_)
                )
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CoordinationRunReport {
    Initialize(InitializeReport),
    AddAgent(AddAgentReport),
    ResumeMember(ResumeAgentReport),
    ResumeTeam(ResumeTeamReport),
    Reonboard(DeliveryResult),
    CreateTeam,
    DisbandTeam(DisbandTeamReport),
    AddMember,
    RemoveMember(StopMemberReport),
    ApplyTaskEffort(crate::daemon::protocol::CoordinationApplyTaskEffortReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunOutcome {
    Running,
    Completed { report: CoordinationRunReport },
    Failed { error: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoordinationRunStatus {
    pub(crate) run_id: String,
    pub(crate) kind: CoordinationRunKind,
    pub(crate) steps: Vec<StepProgress>,
    pub(crate) resume_team_steps: Vec<ResumeTeamProgress>,
    pub(crate) outcome: RunOutcome,
}

#[derive(Debug)]
struct CoordinationRunRecord {
    kind: CoordinationRunKind,
    steps: Vec<StepProgress>,
    resume_team_steps: Vec<ResumeTeamProgress>,
    outcome: RunOutcome,
    terminal_at: Option<Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct CoordinationRunRegistry {
    records: Arc<Mutex<HashMap<String, CoordinationRunRecord>>>,
    ttl: Duration,
}

impl Default for CoordinationRunRegistry {
    fn default() -> Self {
        Self::with_ttl(COORDINATION_RUN_TTL)
    }
}

impl CoordinationRunRegistry {
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self {
            records: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    pub(crate) fn start(&self, kind: CoordinationRunKind) -> String {
        self.start_at(kind, Instant::now())
    }

    fn start_at(&self, kind: CoordinationRunKind, now: Instant) -> String {
        let run_id = format!("{}_{}", kind.id_prefix(), uuid::Uuid::new_v4().simple());
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, now);
        records.insert(
            run_id.clone(),
            CoordinationRunRecord {
                kind,
                steps: Vec::new(),
                resume_team_steps: Vec::new(),
                outcome: RunOutcome::Running,
                terminal_at: None,
            },
        );
        run_id
    }

    pub(crate) fn record_step(&self, run_id: &str, step: StepProgress) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "coordination run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("coordination run '{run_id}' was not found"))?;
        if record.outcome != RunOutcome::Running {
            return Err(format!("coordination run '{run_id}' is already terminal"));
        }
        record.steps.push(step);
        Ok(())
    }

    pub(crate) fn record_resume_team_step(
        &self,
        run_id: &str,
        step: ResumeTeamProgress,
    ) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "coordination run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("coordination run '{run_id}' was not found"))?;
        if record.outcome != RunOutcome::Running {
            return Err(format!("coordination run '{run_id}' is already terminal"));
        }
        if record.kind != CoordinationRunKind::ResumeTeam {
            return Err(format!(
                "coordination run '{run_id}' does not accept team-resume progress"
            ));
        }
        record.resume_team_steps.push(step);
        Ok(())
    }

    pub(crate) fn complete(
        &self,
        run_id: &str,
        report: CoordinationRunReport,
    ) -> Result<(), String> {
        self.complete_at(run_id, report, Instant::now())
    }

    fn complete_at(
        &self,
        run_id: &str,
        report: CoordinationRunReport,
        now: Instant,
    ) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "coordination run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("coordination run '{run_id}' was not found"))?;
        if record.outcome != RunOutcome::Running {
            return Err(format!("coordination run '{run_id}' is already terminal"));
        }
        if !record.kind.accepts(&report) {
            let error = format!(
                "coordination {} run '{run_id}' cannot complete with a different report kind",
                record.kind.operation_name()
            );
            record.outcome = RunOutcome::Failed {
                error: error.clone(),
            };
            record.terminal_at = Some(now);
            return Err(error);
        }
        record.outcome = RunOutcome::Completed { report };
        record.terminal_at = Some(now);
        Ok(())
    }

    pub(crate) fn fail(&self, run_id: &str, error: String) -> Result<(), String> {
        self.finish_at(run_id, RunOutcome::Failed { error }, Instant::now())
    }

    fn finish_at(&self, run_id: &str, outcome: RunOutcome, now: Instant) -> Result<(), String> {
        let mut records = self
            .records
            .lock()
            .map_err(|_| "coordination run registry mutex poisoned".to_string())?;
        let record = records
            .get_mut(run_id)
            .ok_or_else(|| format!("coordination run '{run_id}' was not found"))?;
        if record.outcome != RunOutcome::Running {
            return Err(format!("coordination run '{run_id}' is already terminal"));
        }
        record.outcome = outcome;
        record.terminal_at = Some(now);
        Ok(())
    }

    pub(crate) fn status(&self, run_id: &str) -> Option<CoordinationRunStatus> {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, Instant::now());
        records.get(run_id).map(|record| CoordinationRunStatus {
            run_id: run_id.to_string(),
            kind: record.kind,
            steps: record.steps.clone(),
            resume_team_steps: record.resume_team_steps.clone(),
            outcome: record.outcome.clone(),
        })
    }

    #[cfg(test)]
    fn prune_at(&self, now: Instant) {
        let mut records = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Self::prune_locked(&mut records, self.ttl, now);
    }

    fn prune_locked(
        records: &mut HashMap<String, CoordinationRunRecord>,
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

/// Resolve launch inputs on the daemon host, where pane-shell aliases and
/// managed account homes actually live.
pub(crate) fn prepare_daemon_launch_inputs_for_tools(
    teams_dir: &std::path::Path,
    has_managed_codex: bool,
    tools: Vec<CliTool>,
    commands: &mut CliCommandSettings,
) {
    let codex_bypass_hook_trust =
        crate::commands::terminal_settings::managed_codex_hook_trust_for_launch(
            teams_dir,
            has_managed_codex,
        );
    crate::commands::terminal_settings::apply_managed_codex_launch_inputs(
        commands,
        has_managed_codex,
        codex_bypass_hook_trust,
    );

    let probe = crate::session_scanner::launch_base::ShellAliasProbe::for_pane();
    crate::commands::accounts::apply_team_account_selector_dirs(commands, tools.iter().copied());
    crate::commands::accounts::apply_team_launch_base_resolutions_with(
        commands,
        tools,
        |base, tool| {
            (
                crate::session_scanner::launch_base::resolve_base_command_cached(
                    base, tool, &probe,
                ),
                true,
            )
        },
    );
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{CoordinationRunKind, CoordinationRunRegistry, CoordinationRunReport, RunOutcome};

    #[test]
    fn registry_accepts_every_interactive_coordination_run_kind() {
        let registry = CoordinationRunRegistry::with_ttl(Duration::from_secs(600));

        for (kind, prefix) in [
            (CoordinationRunKind::InitializeTeam, "init_"),
            (CoordinationRunKind::AddAgent, "add_"),
            (CoordinationRunKind::ResumeMember, "resume_"),
            (CoordinationRunKind::ResumeTeam, "team-resume_"),
            (CoordinationRunKind::Reonboard, "reonboard_"),
            (CoordinationRunKind::CreateTeam, "create_"),
            (CoordinationRunKind::DisbandTeam, "disband_"),
            (CoordinationRunKind::AddMember, "member-add_"),
            (CoordinationRunKind::RemoveMember, "member-remove_"),
            (CoordinationRunKind::ApplyTaskEffort, "effort_"),
        ] {
            let run_id = registry.start(kind);
            assert!(run_id.starts_with(prefix), "unexpected run id: {run_id}");
            let status = registry.status(&run_id).expect("run registered");
            assert_eq!(status.kind, kind);
            assert_eq!(status.outcome, RunOutcome::Running);
        }
    }

    #[test]
    fn registry_rejects_a_completed_report_for_the_wrong_run_kind() {
        // Regression: 3b81da38 left a mismatched completion Running forever,
        // so the terminal-record pruner could never remove it.
        let registry = CoordinationRunRegistry::with_ttl(Duration::from_secs(600));
        let run_id = registry.start(CoordinationRunKind::ResumeMember);

        let error = registry
            .complete(
                &run_id,
                CoordinationRunReport::Initialize(
                    crate::coordination::requests::InitializeReport {
                        team_name: "arch".to_string(),
                        succeeded_steps: Vec::new(),
                        failed_step: None,
                        retryable: false,
                        message: "done".to_string(),
                        steps: Vec::new(),
                    },
                ),
            )
            .expect_err("mismatched report rejected");

        assert!(error.contains("resume_member"));
        assert!(matches!(
            registry
                .status(&run_id)
                .expect("failed run remains registered")
                .outcome,
            RunOutcome::Failed { error } if error.contains("different report kind")
        ));
    }

    #[test]
    fn terminal_runs_expire_after_ttl_but_running_runs_do_not() {
        let registry = CoordinationRunRegistry::with_ttl(Duration::from_secs(10));
        let started_at = Instant::now();
        let running_id = registry.start_at(CoordinationRunKind::AddAgent, started_at);
        let completed_id = registry.start_at(CoordinationRunKind::InitializeTeam, started_at);
        registry
            .complete_at(
                &completed_id,
                CoordinationRunReport::Initialize(
                    crate::coordination::requests::InitializeReport {
                        team_name: "arch".to_string(),
                        succeeded_steps: Vec::new(),
                        failed_step: None,
                        retryable: false,
                        message: "done".to_string(),
                        steps: Vec::new(),
                    },
                ),
                started_at,
            )
            .expect("run completed");

        registry.prune_at(started_at + Duration::from_secs(11));

        assert!(registry.status(&completed_id).is_none());
        assert!(registry.status(&running_id).is_some());
    }

    #[test]
    fn failed_run_preserves_recorded_steps_and_terminal_error() {
        let registry = CoordinationRunRegistry::with_ttl(Duration::from_secs(600));
        let run_id = registry.start(CoordinationRunKind::ResumeMember);
        registry
            .record_step(
                &run_id,
                crate::coordination::requests::StepProgress {
                    step: "prepare_member".to_string(),
                    status: crate::coordination::requests::StepStatus::Running,
                    message: None,
                },
            )
            .expect("step recorded");
        registry
            .fail(&run_id, "pipeline panicked".to_string())
            .expect("run failed");

        let status = registry.status(&run_id).expect("failed status");
        assert_eq!(status.steps.len(), 1);
        assert_eq!(
            status.outcome,
            RunOutcome::Failed {
                error: "pipeline panicked".to_string()
            }
        );
    }
}
