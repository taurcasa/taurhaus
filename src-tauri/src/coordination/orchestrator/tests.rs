use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tempfile::TempDir;

use super::*;
use crate::coordination::backend::fake::FakeBackend;
use crate::coordination::backend::MeshBridgedBackend;
use crate::coordination::backend::{BackendKind, CoordinationBackend};
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::{
    AddAgentRequest, AgentSetupConfig, DeliveryMethod, DeliveryRequest, DeliveryResult,
    InitializeTeamRequest, LaunchRequest, LaunchResult, LeadMode, OperatorNoticeDelivery,
    ProbeEvidence, ProbeRequest, ProbeResult, ResumeTeamRequest, StepStatus, TeardownRequest,
    TeardownResult,
};
use crate::coordination::runtime::{
    CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
};
use crate::coordination::stores::{
    MemberRuntimeRecord, MemberRuntimeStore, MeshInboxStore, TeamConfigStore,
};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::{
    ActivityAttribution, ActivityConfidence, RuntimeSession, SessionGroupKind, SessionState,
};

fn sample_member(name: &str, tool: CliTool) -> Member {
    Member {
        name: name.to_string(),
        role: MemberRole::Agent,
        role_id: None,
        role_name: None,
        focus_area: None,
        context_summary: None,
        behavior_summary: None,
        communication_style: None,
        runtime_compact_summary: None,
        instructions: Some("focus on implementation".to_string()),
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
        project_path: PathBuf::from("/tmp/taurhaus"),
        cli_tool: tool,
        extra: Default::default(),
    }
}

fn write_lead_credential(teams_dir: &std::path::Path, team_name: &str, lead_name: &str) {
    if let Ok(mut config) = TeamConfigStore::load(teams_dir, team_name) {
        let lead = config
            .members
            .iter_mut()
            .find(|member| member.name == lead_name)
            .expect("lead member");
        lead.extra.insert(
            "controlAuthTokenHash".to_string(),
            serde_json::Value::String("sha256:test-token".to_string()),
        );
        lead.extra
            .insert("isActive".to_string(), serde_json::Value::Bool(true));
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save lead auth hash");
    }
    let credential_dir = teams_dir.join(team_name).join("state").join("control_auth");
    std::fs::create_dir_all(&credential_dir).expect("credential dir");
    std::fs::write(
        credential_dir.join(format!("{lead_name}.json")),
        format!(r#"{{"name":"{lead_name}","token":"test-token"}}"#),
    )
    .expect("lead credential");
}

fn new_orchestrator(tmp: &TempDir) -> CoordinationOrchestrator {
    CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        Arc::new(FakeBackend::default()),
        Arc::new(RecordingCoordinationRuntime::default()),
    )
}

fn new_orchestrator_with_backend(
    tmp: &TempDir,
    backend: Arc<dyn CoordinationBackend>,
) -> CoordinationOrchestrator {
    CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        Arc::new(RecordingCoordinationRuntime::default()),
    )
}

fn new_orchestrator_with_recording_runtime(
    tmp: &TempDir,
) -> (CoordinationOrchestrator, Arc<RecordingCoordinationRuntime>) {
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        Arc::new(FakeBackend::default()),
        runtime.clone(),
    );
    (orchestrator, runtime)
}

#[derive(Debug)]
struct UndeliveredBackend;

impl CoordinationBackend for UndeliveredBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::ClaudeNative
    }

    fn capabilities(&self) -> crate::coordination::backend::BackendCapabilities {
        crate::coordination::backend::BackendCapabilities::claude_native()
    }

    fn launch(&self, _req: LaunchRequest) -> Result<LaunchResult, CoordinationError> {
        unreachable!("launch is not used in this test")
    }

    fn deliver(&self, _req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        Ok(DeliveryResult {
            delivered: false,
            method: DeliveryMethod::NativeMessageApi,
        })
    }

    fn probe(&self, _req: ProbeRequest) -> Result<ProbeResult, CoordinationError> {
        Ok(ProbeResult {
            alive: false,
            health: HealthState::SessionDead,
            evidence: ProbeEvidence::None,
        })
    }

    fn teardown(&self, _req: TeardownRequest) -> Result<TeardownResult, CoordinationError> {
        Ok(TeardownResult { success: false })
    }
}

#[derive(Debug)]
struct InboxFileBackend;

impl CoordinationBackend for InboxFileBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::MeshBridged
    }

    fn capabilities(&self) -> crate::coordination::backend::BackendCapabilities {
        crate::coordination::backend::BackendCapabilities::mesh_bridged()
    }

    fn launch(&self, _req: LaunchRequest) -> Result<LaunchResult, CoordinationError> {
        unreachable!("launch is not used in this test")
    }

    fn deliver(&self, _req: DeliveryRequest) -> Result<DeliveryResult, CoordinationError> {
        Ok(DeliveryResult {
            delivered: true,
            method: DeliveryMethod::InboxFile,
        })
    }

    fn probe(&self, _req: ProbeRequest) -> Result<ProbeResult, CoordinationError> {
        unreachable!("probe is not used in this test")
    }

    fn teardown(&self, _req: TeardownRequest) -> Result<TeardownResult, CoordinationError> {
        unreachable!("teardown is not used in this test")
    }
}

#[derive(Debug)]
struct MeshPreAddRuntime {
    inner: RecordingCoordinationRuntime,
    teams_dir: PathBuf,
    preadded_project_path: PathBuf,
}

impl MeshPreAddRuntime {
    fn new(teams_dir: PathBuf, preadded_project_path: PathBuf) -> Self {
        Self {
            inner: RecordingCoordinationRuntime::default(),
            teams_dir,
            preadded_project_path,
        }
    }
}

impl CoordinationRuntime for MeshPreAddRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.send_tmux_keys_with_enter(pane_id, keys)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.inner.detect_session_id(pane_id, cli_tool)
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.join_mesh(
            team_name,
            member_name,
            project_id,
            member_type,
            model,
            claude_dir,
        )?;

        let mut config = TeamConfigStore::load(&self.teams_dir, team_name)?;
        if !config
            .members
            .iter()
            .any(|member| member.name == member_name)
        {
            config.members.push(Member {
                name: member_name.to_string(),
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
                project_path: self.preadded_project_path.clone(),
                cli_tool: CliTool::Codex,
                model: Some(model.to_string()),
                reasoning_effort: None,
                extra: Default::default(),
            });
            TeamConfigStore::save(&self.teams_dir, team_name, &config)?;
        }
        Ok(())
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        self.inner.pane_belongs_to_project(pane_id, project_id)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_exists(pane_id)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_dead(pane_id)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_shell(pane_id)
    }

    fn pane_current_command(&self, _pane_id: &str) -> Result<Option<String>, CoordinationError> {
        Ok(None)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.inner.kill_aitx_pane(pane_id)
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.inner.terminate_process_by_pid(pid)
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.is_process_running_by_pid(pid)
    }
}

#[derive(Debug)]
struct PaneOwnershipRuntime {
    inner: RecordingCoordinationRuntime,
    ownership_matches: bool,
}

impl PaneOwnershipRuntime {
    fn new(ownership_matches: bool) -> Self {
        Self {
            inner: RecordingCoordinationRuntime::default(),
            ownership_matches,
        }
    }

    fn calls(&self) -> Vec<RuntimeCall> {
        self.inner.calls()
    }
}

impl CoordinationRuntime for PaneOwnershipRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.send_tmux_keys_with_enter(pane_id, keys)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.inner.detect_session_id(pane_id, cli_tool)
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.join_mesh(
            team_name,
            member_name,
            project_id,
            member_type,
            model,
            claude_dir,
        )
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        let _ = self.inner.pane_belongs_to_project(pane_id, project_id)?;
        Ok(self.ownership_matches)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_exists(pane_id)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_dead(pane_id)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_shell(pane_id)
    }

    fn pane_current_command(&self, _pane_id: &str) -> Result<Option<String>, CoordinationError> {
        Ok(None)
    }

    fn live_pane(
        &self,
        pane_id: &str,
    ) -> Result<Option<crate::coordination::runtime::LivePane>, CoordinationError> {
        let mut live_pane = self.inner.live_pane(pane_id)?;
        if let Some(live_pane) = &mut live_pane {
            live_pane.current_path = Some(PathBuf::from(if self.ownership_matches {
                "/tmp/taurhaus"
            } else {
                "/recording-runtime/foreign-project"
            }));
        }
        Ok(live_pane)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.inner.kill_aitx_pane(pane_id)
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.inner.terminate_process_by_pid(pid)
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.is_process_running_by_pid(pid)
    }
}

#[derive(Debug)]
struct ProjectPathCheckingRuntime {
    inner: RecordingCoordinationRuntime,
}

impl ProjectPathCheckingRuntime {
    fn new() -> Self {
        Self {
            inner: RecordingCoordinationRuntime::default(),
        }
    }

    fn calls(&self) -> Vec<RuntimeCall> {
        self.inner.calls()
    }
}

impl CoordinationRuntime for ProjectPathCheckingRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.send_tmux_keys_with_enter(pane_id, keys)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.inner.detect_session_id(pane_id, cli_tool)
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        if !std::path::Path::new(project_id).is_dir() {
            return Err(CoordinationError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("project path does not exist: {project_id}"),
            )));
        }
        self.inner.join_mesh(
            team_name,
            member_name,
            project_id,
            member_type,
            model,
            claude_dir,
        )
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        self.inner.pane_belongs_to_project(pane_id, project_id)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_exists(pane_id)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_dead(pane_id)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_shell(pane_id)
    }

    fn pane_current_command(&self, _pane_id: &str) -> Result<Option<String>, CoordinationError> {
        Ok(None)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.inner.kill_aitx_pane(pane_id)
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.inner.terminate_process_by_pid(pid)
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.is_process_running_by_pid(pid)
    }
}

#[derive(Debug)]
struct SelectiveJoinFailureRuntime {
    inner: RecordingCoordinationRuntime,
    fail_members: HashSet<String>,
}

impl SelectiveJoinFailureRuntime {
    fn new(fail_members: &[&str]) -> Self {
        Self {
            inner: RecordingCoordinationRuntime::default(),
            fail_members: fail_members
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
        }
    }
}

impl CoordinationRuntime for SelectiveJoinFailureRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.send_tmux_keys_with_enter(pane_id, keys)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.inner.detect_session_id(pane_id, cli_tool)
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        if self.fail_members.contains(member_name) {
            return Err(CoordinationError::Backend(format!(
                "programmed join_mesh failure for '{member_name}'"
            )));
        }
        self.inner.join_mesh(
            team_name,
            member_name,
            project_id,
            member_type,
            model,
            claude_dir,
        )
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        self.inner.pane_belongs_to_project(pane_id, project_id)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_exists(pane_id)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_dead(pane_id)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_shell(pane_id)
    }

    fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError> {
        self.inner.pane_current_command(pane_id)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.inner.kill_aitx_pane(pane_id)
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.inner.terminate_process_by_pid(pid)
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.is_process_running_by_pid(pid)
    }
}

#[derive(Debug)]
struct ClaudeLaunchRosterRuntime {
    inner: RecordingCoordinationRuntime,
    teams_dir: PathBuf,
    team_name: String,
    member_name: String,
}

impl ClaudeLaunchRosterRuntime {
    fn new(teams_dir: PathBuf, team_name: &str, member_name: &str) -> Self {
        Self {
            inner: RecordingCoordinationRuntime::default(),
            teams_dir,
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        }
    }
}

impl CoordinationRuntime for ClaudeLaunchRosterRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.inner.create_aitx_pane(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        if keys.contains("--agent-name") && keys.contains(self.member_name.as_str()) {
            let config = TeamConfigStore::load(&self.teams_dir, &self.team_name)?;
            if !config
                .members
                .iter()
                .any(|member| member.name == self.member_name)
            {
                return Err(CoordinationError::Backend(format!(
                    "member '{}' missing from roster before claude launch",
                    self.member_name
                )));
            }
        }
        self.inner.send_tmux_keys_with_enter(pane_id, keys)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.inner.detect_session_id(pane_id, cli_tool)
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        self.inner.join_mesh(
            team_name,
            member_name,
            project_id,
            member_type,
            model,
            claude_dir,
        )
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner
            .spawn_mesh_daemon(pane_id, team_name, member_name)
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.inner.spawn_team_daemon(team_name, operator_name)
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        self.inner.pane_belongs_to_project(pane_id, project_id)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_exists(pane_id)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_dead(pane_id)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.inner.pane_is_shell(pane_id)
    }

    fn pane_current_command(&self, _pane_id: &str) -> Result<Option<String>, CoordinationError> {
        Ok(None)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.inner.kill_aitx_pane(pane_id)
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.inner.terminate_process_by_pid(pid)
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.inner.is_process_running_by_pid(pid)
    }
}

fn initialize_request(team_name: &str) -> InitializeTeamRequest {
    InitializeTeamRequest {
        team_name: team_name.to_string(),
        team_description: Some("init pipeline test".to_string()),
        lead_mode: LeadMode::LaunchNew,
        lead: AgentSetupConfig {
            name: "team-lead".to_string(),
            cli_tool: "claude".to_string(),
            model: "opus".to_string(),
            reasoning_effort: None,
            project_id: "/tmp/lead".to_string(),
            description: Some("lead".to_string()),
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
        },
        agents: vec![
            AgentSetupConfig {
                name: "frontend-dev".to_string(),
                cli_tool: "codex".to_string(),
                model: "gpt-5.4".to_string(),
                reasoning_effort: None,
                project_id: "/tmp/frontend".to_string(),
                description: Some("frontend".to_string()),
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
            },
            AgentSetupConfig {
                name: "reviewer".to_string(),
                cli_tool: "agy".to_string(),
                model: "pro".to_string(),
                reasoning_effort: None,
                project_id: "/tmp/reviewer".to_string(),
                description: Some("review".to_string()),
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
            },
        ],
    }
}

fn add_agent_request(team_name: &str, agent_name: &str, cli_tool: &str) -> AddAgentRequest {
    AddAgentRequest {
        team_name: team_name.to_string(),
        agent: AgentSetupConfig {
            name: agent_name.to_string(),
            cli_tool: cli_tool.to_string(),
            model: "model".to_string(),
            reasoning_effort: None,
            project_id: format!("/tmp/{agent_name}"),
            description: Some("hot-added".to_string()),
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
        },
    }
}

fn create_running_team(orchestrator: &mut CoordinationOrchestrator, team_name: &str) {
    orchestrator
        .create_team(team_name, Some("running".to_string()))
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member("existing-dev", CliTool::Codex))
        .expect("add existing member");
    write_lead_credential(&orchestrator.teams_dir, team_name, "team-lead");
}

fn member_with_project(name: &str, role: MemberRole, tool: CliTool, project_path: &str) -> Member {
    Member {
        name: name.to_string(),
        role,
        role_id: None,
        role_name: None,
        focus_area: None,
        context_summary: None,
        behavior_summary: None,
        communication_style: None,
        runtime_compact_summary: None,
        instructions: Some("resume me".to_string()),
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
        project_path: PathBuf::from(project_path),
        cli_tool: tool,
        extra: Default::default(),
    }
}

fn mark_member_offline(tmp: &TempDir, team_name: &str, member_name: &str) {
    let mut runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    runtime.health = HealthState::SessionDead;
    runtime.pane_id = None;
    runtime.daemon_pid = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime).expect("runtime saved");
}

fn create_resumable_team(
    orchestrator: &mut CoordinationOrchestrator,
    tmp: &TempDir,
    team_name: &str,
    lead_tool: CliTool,
) {
    orchestrator
        .create_team(team_name, Some("resumable".to_string()))
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, lead_tool, "/tmp/lead"),
        )
        .expect("add lead");
    orchestrator
        .add_member(
            team_name,
            member_with_project("builder", MemberRole::Agent, CliTool::Codex, "/tmp/lead"),
        )
        .expect("add builder");
    orchestrator
        .add_member(
            team_name,
            member_with_project("reviewer", MemberRole::Agent, CliTool::Agy, "/tmp/reviewer"),
        )
        .expect("add reviewer");

    for member_name in ["team-lead", "builder", "reviewer"] {
        mark_member_offline(tmp, team_name, member_name);
    }
    write_lead_credential(tmp.path(), team_name, "team-lead");
}

fn assert_conflict(err: CoordinationError) {
    match err {
        CoordinationError::Conflict(_) => {}
        other => panic!("expected conflict, got {other:?}"),
    }
}

fn assert_not_found(err: CoordinationError) {
    match err {
        CoordinationError::NotFound(_) => {}
        other => panic!("expected not_found, got {other:?}"),
    }
}

#[test]
fn resume_team_lead_first_then_same_project_then_cross_project() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    create_resumable_team(
        &mut orchestrator,
        &tmp,
        "architecture-final",
        CliTool::Claude,
    );

    let report = orchestrator
        .resume_team_with_cli_commands_and_layout(
            &ResumeTeamRequest {
                team_name: "architecture-final".to_string(),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("resume team");

    assert!(report.resumed);
    assert_eq!(
        report.resumed_members,
        vec![
            "team-lead".to_string(),
            "builder".to_string(),
            "reviewer".to_string()
        ]
    );

    let calls = runtime.calls();
    let send_keys: Vec<String> = calls
        .iter()
        .filter_map(|call| match call {
            RuntimeCall::SendKeys { keys, .. } => Some(keys.clone()),
            _ => None,
        })
        .collect();
    assert!(
        send_keys
            .first()
            .expect("lead launch should be first")
            .contains("team-lead"),
        "first launch should belong to the lead"
    );

    let join_order: Vec<String> = calls
        .iter()
        .filter_map(|call| match call {
            RuntimeCall::JoinMesh { member_name, .. } => Some(member_name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        join_order,
        vec![
            "team-lead".to_string(),
            "builder".to_string(),
            "reviewer".to_string()
        ]
    );
    let team_daemon_spawns = calls
        .iter()
        .filter(|call| {
            matches!(
                call,
                RuntimeCall::SpawnTeamDaemon {
                    team_name,
                    operator_name,
                } if team_name == "architecture-final" && operator_name == "team-lead"
            )
        })
        .count();
    assert_eq!(team_daemon_spawns, 1);
}

#[test]
fn resume_team_reports_partial_success_when_middle_member_fails() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(SelectiveJoinFailureRuntime::new(&["builder"]));
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);
    create_resumable_team(
        &mut orchestrator,
        &tmp,
        "architecture-final",
        CliTool::Claude,
    );

    let report = orchestrator
        .resume_team_with_cli_commands_and_layout(
            &ResumeTeamRequest {
                team_name: "architecture-final".to_string(),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("resume team");

    assert!(report.resumed);
    assert_eq!(report.total_members, 3);
    assert_eq!(
        report.resumed_members,
        vec!["team-lead".to_string(), "reviewer".to_string()]
    );
    assert_eq!(report.failed_members.len(), 1);
    assert_eq!(report.failed_members[0].member_name, "builder");
}

#[test]
fn resume_team_does_not_roll_back_earlier_successes_when_later_member_fails() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(SelectiveJoinFailureRuntime::new(&["reviewer"]));
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);
    create_resumable_team(
        &mut orchestrator,
        &tmp,
        "architecture-final",
        CliTool::Claude,
    );

    let report = orchestrator
        .resume_team_with_cli_commands_and_layout(
            &ResumeTeamRequest {
                team_name: "architecture-final".to_string(),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("resume team");

    assert!(report.resumed);
    assert_eq!(
        report.resumed_members,
        vec!["team-lead".to_string(), "builder".to_string()]
    );
    assert_eq!(report.failed_members.len(), 1);
    assert_eq!(report.failed_members[0].member_name, "reviewer");
}

#[test]
fn resume_team_reports_full_failure_when_all_members_fail() {
    let tmp = TempDir::new().expect("tempdir");
    let backend = Arc::new(FakeBackend::default());
    let runtime = Arc::new(SelectiveJoinFailureRuntime::new(&[
        "team-lead",
        "builder",
        "reviewer",
    ]));
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);
    create_resumable_team(
        &mut orchestrator,
        &tmp,
        "architecture-final",
        CliTool::Codex,
    );

    let report = orchestrator
        .resume_team_with_cli_commands_and_layout(
            &ResumeTeamRequest {
                team_name: "architecture-final".to_string(),
            },
            &CliCommandSettings::default(),
            "new_window",
        )
        .expect("resume team");

    assert!(!report.resumed);
    assert!(report.resumed_members.is_empty());
    assert_eq!(report.failed_members.len(), 3);
    assert_eq!(
        report
            .failed_members
            .iter()
            .map(|failure| failure.member_name.as_str())
            .collect::<Vec<_>>(),
        vec!["team-lead", "builder", "reviewer"]
    );
}

#[test]
fn create_team_then_list() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator
        .create_team("architecture-final", Some("desc".to_string()))
        .expect("create should succeed");

    let teams = orchestrator.list_teams().expect("list should succeed");
    assert_eq!(teams, vec!["architecture-final".to_string()]);
}

#[test]
fn discover_teams_resolves_lead_project_anchor() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    create_running_team(&mut orchestrator, team_name);

    let discovery = orchestrator
        .discover_teams()
        .expect("discover should succeed");
    assert_eq!(discovery.warnings.len(), 0);
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, team_name);
    assert_eq!(
        discovery.teams[0].lead_project_path.as_deref(),
        Some(std::path::Path::new("/tmp/lead"))
    );
}

#[test]
fn discover_teams_skips_corrupt_folder_with_warning() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let valid_team = "alpha";
    create_running_team(&mut orchestrator, valid_team);

    let broken_dir = tmp.path().join("broken-team");
    std::fs::create_dir_all(&broken_dir).expect("create broken dir");
    std::fs::write(broken_dir.join("config.json"), "{ broken json").expect("write broken");

    let discovery = orchestrator
        .discover_teams()
        .expect("discover should succeed");
    assert_eq!(discovery.teams.len(), 1);
    assert_eq!(discovery.teams[0].team_name, valid_team);
    assert_eq!(discovery.warnings.len(), 1);
    assert!(discovery.warnings[0].contains("broken-team"));

    let teams = orchestrator.list_teams().expect("list should succeed");
    assert_eq!(teams, vec![valid_team.to_string()]);
}

#[test]
fn create_team_duplicate_returns_conflict() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator
        .create_team("architecture-final", None)
        .expect("first create should succeed");
    let err = orchestrator
        .create_team("architecture-final", None)
        .expect_err("duplicate create should fail");
    assert_conflict(err);
}

#[test]
fn disband_team_removes_directory() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    let result = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("disband should succeed");
    assert!(result.disbanded);
    assert!(!result.already_disbanded);

    assert!(
        !tmp.path().join(team_name).exists(),
        "team directory should be removed"
    );
}

#[test]
fn disband_team_stops_team_daemon_best_effort() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");

    orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("disband should succeed");

    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::StopTeamDaemon { team_name: recorded_team } if recorded_team == team_name
    )));
}

#[test]
fn disband_nonexistent_team_returns_already_disbanded() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    let result = orchestrator
        .disband_team("missing-team", None)
        .expect("idempotent disband should succeed");
    assert!(!result.disbanded);
    assert!(result.already_disbanded);
}

#[test]
fn disband_is_idempotent_and_does_not_invoke_backend_controls() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    let first = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("first disband");
    let second = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("second disband");

    assert!(first.disbanded);
    assert!(!first.already_disbanded);
    assert!(!second.disbanded);
    assert!(second.already_disbanded);
    assert_eq!(
        fake.call_counts(),
        (0, 0, 0, 0),
        "disband should not touch backend session controls"
    );
}

#[test]
fn disband_tears_down_non_lead_members() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";
    create_running_team(&mut orchestrator, team_name);

    let result = orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("disband should succeed");
    assert!(result.disbanded);
    assert!(!result.already_disbanded);
    assert_eq!(
        fake.call_counts(),
        (0, 0, 0, 1),
        "disband should call backend teardown once for one non-lead member"
    );
}

#[test]
fn disband_tears_down_mesh_backed_lead_resources() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "architecture-final-mesh-lead";

    orchestrator
        .create_team(team_name, Some("mesh lead".to_string()))
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Codex, "/tmp/lead"),
        )
        .expect("add lead");

    let mut runtime_record =
        MemberRuntimeStore::load(tmp.path(), team_name, "team-lead").expect("load runtime");
    runtime_record.pane_id = Some("%42".to_string());
    runtime_record.daemon_pid = Some(4242);
    runtime_record.attached_at = Some(Utc::now());
    runtime_record.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, "team-lead", &runtime_record)
        .expect("save runtime");

    orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("disband should succeed");

    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 4242)));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::ClearDaemonPidFile { team_name: recorded_team, member_name }
            if recorded_team == team_name && member_name == "team-lead"
    )));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%42")));
    assert_eq!(
        fake.call_counts(),
        (0, 0, 0, 1),
        "disband should leave mesh for the mesh-backed lead"
    );
}

#[test]
fn disband_preserves_attach_existing_claude_lead() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "architecture-final-attach-lead";

    orchestrator
        .create_team(team_name, Some("attach existing".to_string()))
        .expect("create team");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add lead");

    orchestrator
        .disband_team(team_name, Some("cleanup".to_string()))
        .expect("disband should succeed");

    let calls = runtime.calls();
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { .. })),
        "attach-existing Claude lead should not have pane teardown forced"
    );
    assert!(
        !calls.iter().any(|call| matches!(call, RuntimeCall::ClearDaemonPidFile { member_name, .. } if member_name == "team-lead")),
        "attach-existing Claude lead should not run member daemon cleanup"
    );
    assert_eq!(
        fake.call_counts(),
        (0, 0, 0, 0),
        "attach-existing Claude lead should not trigger backend teardown on disband"
    );
}

#[test]
fn add_member_then_get_status() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert_eq!(status.config.members.len(), 2);
    assert!(status
        .config
        .members
        .iter()
        .any(|member| member.name == "team-lead"));
    assert!(status
        .config
        .members
        .iter()
        .any(|member| member.name == member_name));
    assert_eq!(status.members_runtime.len(), 2);
    assert!(status
        .members_runtime
        .iter()
        .any(|(name, runtime)| name == member_name && runtime.health == HealthState::SessionDead));
}

#[test]
fn get_team_status_fast_returns_disk_snapshot_without_runtime_calls() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-fast-read";
    let member_name = "existing-dev";

    create_running_team(&mut orchestrator, team_name);

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    let status = orchestrator
        .get_team_status_fast(team_name)
        .expect("status should load");

    assert_eq!(status.config.name, team_name);
    assert_eq!(status.config.members.len(), 2);
    assert_eq!(status.members_runtime.len(), 2);
    assert!(status
        .members_runtime
        .iter()
        .any(|(name, runtime_record)| name == member_name && runtime_record == &record));
    assert!(
        runtime.calls().is_empty(),
        "fast read should not touch tmux or process runtime checks"
    );
}

#[test]
fn get_team_status_fast_matches_existing_status_path_before_reconciliation() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-fast-read-match";
    let member_name = "existing-dev";

    create_running_team(&mut orchestrator, team_name);

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    let fast = orchestrator
        .get_team_status_fast(team_name)
        .expect("fast status should load");
    let existing = orchestrator
        .get_team_status(team_name)
        .expect("existing status should load");

    assert_eq!(fast, existing);
}

#[test]
fn add_duplicate_member_returns_conflict() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member = sample_member("codex-reviewer", CliTool::Codex);

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, member.clone())
        .expect("first add should succeed");
    let err = orchestrator
        .add_member(team_name, member)
        .expect_err("duplicate add should fail");
    assert_conflict(err);
}

#[test]
fn remove_member_cleans_runtime() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    let report = orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");
    assert!(report.removed);
    assert!(report.steps.iter().any(|step| step.step == "update_config"));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "delete_runtime"));

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert_eq!(status.config.members.len(), 1);
    assert_eq!(status.config.members[0].name, "team-lead");
    assert_eq!(status.members_runtime.len(), 1);
    assert_eq!(status.members_runtime[0].0, "team-lead");
}

#[test]
fn remove_member_tears_down_runtime_resources() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime exists");
    runtime.pane_id = Some("%9".to_string());
    runtime.daemon_pid = Some(u32::MAX);
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime).expect("runtime saved");

    let report = orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");
    assert!(report.removed);
    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "verify_pane_ownership" && step.success));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "kill_pane" && step.success));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "notify_lead" && step.success));
    assert_eq!(
        fake.call_counts(),
        (0, 1, 0, 1),
        "remove_member should invoke backend teardown and notify lead"
    );
    let delivered = fake.delivered_requests();
    assert_eq!(delivered.len(), 1);
    match &delivered[0] {
        DeliveryRequest::OperatorNotice(payload) => {
            assert_eq!(payload.member_name, "team-lead");
            assert_eq!(payload.team_name, team_name);
            assert!(payload.message.contains(member_name));
            assert!(payload.message.contains(" by '"));
            assert!(payload.message.contains("Cleanup: complete"));
        }
        other => panic!("expected operator notice, got {other:?}"),
    }
}

#[test]
fn remove_member_discovers_and_terminates_daemon_when_runtime_pid_is_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let fake = Arc::new(FakeBackend::default());
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), fake, runtime.clone());
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut runtime_record =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime exists");
    runtime_record.pane_id = Some("%9".to_string());
    runtime_record.daemon_pid = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime_record)
        .expect("runtime saved");
    runtime.set_matching_daemon_pids("%9", team_name, member_name, &[5555]);

    let report = orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");

    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "clear_daemon_pid_file" && step.success));
    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::FindDaemon {
            pane_id,
            team_name: recorded_team,
            member_name: recorded_member
        } if pane_id == "%9" && recorded_team == team_name && recorded_member == member_name
    )));
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 5555)));
    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::ClearDaemonPidFile {
            team_name: recorded_team,
            member_name: recorded_member
        } if recorded_team == team_name && recorded_member == member_name
    )));
}

#[test]
fn remove_member_discovers_and_terminates_daemon_from_pidfile_when_runtime_attachment_is_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let fake = Arc::new(FakeBackend::default());
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), fake, runtime.clone());
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut runtime_record =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime exists");
    runtime_record.pane_id = None;
    runtime_record.daemon_pid = None;
    runtime_record.health = HealthState::SessionDead;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime_record)
        .expect("runtime saved");
    runtime.set_member_daemon_pid_match(team_name, member_name, Some(5555));

    let report = orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");

    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "clear_daemon_pid_file" && step.success));
    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::FindDaemonByMember {
            team_name: recorded_team,
            member_name: recorded_member
        } if recorded_team == team_name && recorded_member == member_name
    )));
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 5555)));
}

#[test]
fn remove_member_rejects_lead_removal() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");

    let err = orchestrator
        .remove_member(team_name, "team-lead", None)
        .expect_err("lead removal should be blocked");
    match err {
        CoordinationError::Validation(message) => {
            assert!(message.contains("cannot be removed"));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn remove_member_skips_pane_kill_on_ownership_mismatch() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(PaneOwnershipRuntime::new(false));
    let fake = Arc::new(FakeBackend::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        fake.clone(),
        runtime.clone(),
    );
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("lead".to_string()),
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
                project_path: PathBuf::from("/tmp/lead"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            },
        )
        .expect("add lead");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut runtime_record =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime exists");
    runtime_record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime_record)
        .expect("runtime saved");

    let report = orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");

    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "verify_pane_ownership" && !step.success));
    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "kill_pane" && !step.success));
    assert!(!report.warnings.is_empty());
    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "notify_lead" && step.success));

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::InspectPane { pane_id } if pane_id == "%9")),
        "member identity inspection should run before pane kill"
    );
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            RuntimeCall::KillPane { pane_id } if pane_id == "%9"
        )),
        "pane kill should be skipped on ownership mismatch"
    );

    let delivered = fake.delivered_requests();
    assert_eq!(delivered.len(), 1);
    match &delivered[0] {
        DeliveryRequest::OperatorNotice(payload) => {
            assert_eq!(payload.member_name, "team-lead");
            assert!(payload.message.contains(member_name));
            assert!(payload.message.contains(" by '"));
            assert!(payload.message.contains("Cleanup: partial"));
        }
        other => panic!("expected operator notice, got {other:?}"),
    }
}

#[test]
fn remove_member_does_not_kill_same_project_pane_owned_by_another_process() {
    // Regression: 4344edb4 gated teardown only by project path, so one member
    // could kill a reused pane belonging to a sibling in the same checkout.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let backend = Arc::new(FakeBackend::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "teardown-foreign-identity";
    create_running_team(&mut orchestrator, team_name);
    let mut record =
        MemberRuntimeStore::load(tmp.path(), team_name, "existing-dev").expect("member runtime");
    record.pane_id = Some("%9".to_string());
    record.pane_pid = Some(9001);
    record.pane_start_time = Some(1_755_000_009);
    record.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, "existing-dev", &record).expect("save runtime");
    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_current_path("%9", Some("/tmp/taurhaus"));
    runtime.set_pane_current_command("%9", Some("codex"));
    runtime.set_pane_identity("%9", Some(9002), Some(1_755_000_009));

    let report = orchestrator
        .remove_member(team_name, "existing-dev", Some("cleanup".to_string()))
        .expect("remove report");

    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "verify_pane_ownership" && !step.success));
    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%9")),
        "a same-path pane with another pid must not be killed"
    );
}

#[test]
fn remove_member_does_not_kill_pane_id_only_record_in_another_project() {
    // Regression: a0c53db8 treated the ownership detector's evidence-free
    // default as positive kill authorization and bypassed the configured path.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let backend = Arc::new(FakeBackend::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "teardown-pane-id-only";
    create_running_team(&mut orchestrator, team_name);
    std::fs::write(
        tmp.path()
            .join(team_name)
            .join("runtime")
            .join("existing-dev.json"),
        r#"{"schema_version":1,"member_name":"existing-dev","pane_id":"%9","health":"healthy"}"#,
    )
    .expect("write legacy runtime");
    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_current_path("%9", Some("/somewhere/else"));

    let report = orchestrator
        .remove_member(team_name, "existing-dev", Some("cleanup".to_string()))
        .expect("remove report");

    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "verify_pane_ownership" && !step.success));
    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%9")),
        "a pane-id-only record must still be checked against the configured project"
    );
}

#[test]
fn remove_member_kills_the_pane_with_its_recorded_identity() {
    // Regression: 4344edb4 did not use the durable member identity as its
    // positive teardown gate.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let backend = Arc::new(FakeBackend::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "teardown-owned-identity";
    create_running_team(&mut orchestrator, team_name);
    let mut record =
        MemberRuntimeStore::load(tmp.path(), team_name, "existing-dev").expect("member runtime");
    record.pane_id = Some("%9".to_string());
    record.pane_pid = Some(9001);
    record.pane_start_time = Some(1_755_000_009);
    record.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, "existing-dev", &record).expect("save runtime");
    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_current_path("%9", Some("/tmp/taurhaus"));
    runtime.set_pane_current_command("%9", Some("codex"));
    runtime.set_pane_identity("%9", Some(9001), Some(1_755_000_009));

    let report = orchestrator
        .remove_member(team_name, "existing-dev", Some("cleanup".to_string()))
        .expect("remove report");

    assert!(report
        .steps
        .iter()
        .any(|step| step.step == "verify_pane_ownership" && step.success));
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%9")));
}

#[test]
fn startup_reconcile_clears_stale_daemon_pid() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime exists");
    runtime.daemon_pid = Some(u32::MAX);
    runtime.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &runtime).expect("runtime saved");

    orchestrator
        .reconcile_runtime_state_on_startup()
        .expect("startup reconcile should succeed");

    let updated =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime reloaded");
    assert_eq!(updated.daemon_pid, None);
    assert_eq!(updated.health, HealthState::SessionDead);
}

#[test]
fn startup_reconcile_removes_orphan_runtime_records() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");

    let orphan_runtime = MemberRuntimeRecord {
        schema_version: 3,
        member_name: "orphan-agent".to_string(),
        cli_tool: None,
        project_path: None,
        pane_id: Some("%7".to_string()),
        pane_pid: None,
        pane_start_time: None,
        session_id: None,
        jsonl_path: None,
        daemon_pid: None,
        health: HealthState::SessionDead,
        delivery_lease: None,
        attached_at: None,
        last_seen_at: None,
        applied_effort: None,
        effort_resume_failure: None,
        extra: Default::default(),
    };
    MemberRuntimeStore::save(tmp.path(), team_name, "orphan-agent", &orphan_runtime)
        .expect("orphan runtime saved");

    orchestrator
        .reconcile_runtime_state_on_startup()
        .expect("startup reconcile should succeed");

    let err = MemberRuntimeStore::load(tmp.path(), team_name, "orphan-agent")
        .expect_err("orphan runtime should be deleted");
    assert_not_found(err);
    assert_eq!(
        fake.call_counts(),
        (0, 0, 0, 1),
        "orphan runtime reconcile should attempt backend teardown"
    );
}

#[test]
fn startup_reconcile_does_not_kill_orphan_record_without_ownership_evidence() {
    // Regression: a0c53db8 authorized a kill from pane identity absence when
    // tolerant decoding surfaced a mesh-authored partial orphan record.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let backend = Arc::new(FakeBackend::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "orphan-pane-id-only";
    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    std::fs::create_dir_all(tmp.path().join(team_name).join("runtime"))
        .expect("create runtime dir");
    std::fs::write(
        tmp.path()
            .join(team_name)
            .join("runtime")
            .join("orphan-agent.json"),
        r#"{"paneId":"%7","appliedEffort":"medium"}"#,
    )
    .expect("write partial runtime");
    runtime.set_pane_exists("%7", true);
    runtime.set_pane_dead("%7", false);

    orchestrator
        .reconcile_runtime_state_on_startup()
        .expect("startup reconcile should succeed");

    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%7")),
        "a partial orphan record must not authorize a pane kill"
    );
}

#[test]
fn liveness_reconcile_marks_missing_pane_id_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.pane_id = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert!(
        !runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPaneExists { .. }
                | RuntimeCall::CheckPaneDead { .. }
                | RuntimeCall::CheckPaneShell { .. }
                | RuntimeCall::InspectPane { .. }
        )),
        "missing pane id should not query tmux pane state"
    );
}

#[test]
fn liveness_reconcile_marks_missing_pane_target_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", false);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::InspectPane { pane_id } if pane_id == "%9")));
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPaneDead { .. } | RuntimeCall::CheckPaneShell { .. }
        )),
        "missing pane target should short-circuit dead/shell checks"
    );
}

#[test]
fn liveness_reconcile_marks_dead_pane_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", true);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::InspectPane { pane_id } if pane_id == "%9")));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CheckPaneShell { .. })),
        "dead pane should short-circuit shell checks"
    );
}

#[test]
fn liveness_reconcile_marks_shell_pane_offline() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", true);
    runtime.set_pane_current_command("%9", Some("bash"));

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::InspectPane { pane_id } if pane_id == "%9")));
}

#[test]
fn liveness_reconcile_keeps_alive_cli_pane_healthy() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_pid_running(4242, true);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(
        updated, record,
        "active CLI pane should not be marked offline"
    );
    let calls = runtime.calls();
    // Regression: 39eeb33 made four tmux subprocess calls before every
    // liveness decision even though InspectPane returns the same state.
    assert_eq!(
        calls
            .iter()
            .filter(|call| matches!(call, RuntimeCall::InspectPane { pane_id } if pane_id == "%9"))
            .count(),
        1
    );
    assert!(calls.iter().all(|call| !matches!(
        call,
        RuntimeCall::CheckPaneExists { .. }
            | RuntimeCall::CheckPaneDead { .. }
            | RuntimeCall::CheckPaneShell { .. }
    )));
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 4242)),
        "healthy member should verify daemon liveness"
    );
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            RuntimeCall::TerminatePid { .. } | RuntimeCall::SpawnDaemon { .. }
        )),
        "healthy member should not trigger daemon restart/cleanup"
    );
}

#[test]
fn liveness_reconcile_quarantines_foreign_member_without_blocking_team_daemon() {
    // Regression: aecc8ac let one non-lead foreign pane permanently block the
    // whole team's daemon and left the stale pane binding latched forever.
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("pane-foreign-events.jsonl");
    let log_state = taurhaus_lib::logging::LogFileState::new(log_path.clone()).expect("log state");
    taurhaus_lib::logging::install_global_sink(&log_state);

    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add lead should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    write_lead_credential(tmp.path(), team_name, "team-lead");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("stale-codex-session".to_string());
    record.pane_id = Some("%9".to_string());
    record.daemon_pid = Some(4242);
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");
    let config = TeamConfigStore::load(tmp.path(), team_name).expect("load config");
    TeamConfigStore::save(tmp.path(), team_name, &config).expect("sync pane metadata");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_pane_current_command("%9", Some("claude"));
    runtime.set_pid_running(4242, true);
    runtime.set_team_daemon_current_mesh_binary(team_name, false);

    orchestrator
        .trigger_team_self_heal(team_name)
        .expect("first self-heal should succeed");
    orchestrator
        .trigger_team_self_heal(team_name)
        .expect("second self-heal should be idempotent");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert_eq!(updated.daemon_pid, None);
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid: 4242 })));
    assert_eq!(updated.pane_id, None);
    assert_eq!(updated.pane_pid, None);
    assert_eq!(updated.pane_start_time, None);
    assert!(runtime
        .calls()
        .iter()
        .all(|call| !matches!(call, RuntimeCall::SpawnDaemon { .. })));
    assert!(runtime
        .calls()
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnTeamDaemon { .. })));
    let config_raw = std::fs::read_to_string(tmp.path().join(team_name).join("config.json"))
        .expect("read config");
    let config: serde_json::Value = serde_json::from_str(&config_raw).expect("parse config");
    let member = config["members"]
        .as_array()
        .expect("members")
        .iter()
        .find(|candidate| candidate["name"] == member_name)
        .expect("member config");
    assert!(member.get("tmuxPaneId").is_none());

    log_state
        .flush_for_test()
        .expect("flush structured log sink");
    let events = std::fs::read_to_string(log_path)
        .expect("read structured log")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse log record"))
        .filter(|record| {
            record["event"] == "coordination.pane.foreign"
                && record["team"] == team_name
                && record["member"] == member_name
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["team"], team_name);
    assert_eq!(events[0]["member"], member_name);
    assert_eq!(events[0]["pane_id"], "%9");
    assert!(events[0]["reason"]
        .as_str()
        .is_some_and(|reason| reason.contains("claude")));
}

#[test]
fn live_status_ignores_cached_snapshot_pane_when_record_has_newer_pane() {
    // Regression: aecc8ac compared a cached scanner pane with another pane's
    // persisted PID identity, marking a just-resumed healthy member dead.
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add member");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.pane_id = Some("%20".to_string());
    record.pane_pid = Some(2020);
    record.pane_start_time = Some(220);
    record.daemon_pid = Some(4242);
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");
    runtime.set_pane_identity("%20", Some(2020), Some(220));
    runtime.set_pane_current_command("%20", Some("codex"));
    runtime.set_pane_current_command("%9", Some("claude"));

    let cached_snapshot = RuntimeSession {
        pid: 9009,
        project_path: "/tmp/taurhaus".to_string(),
        tty: "/dev/pts/9".to_string(),
        args: "claude".to_string(),
        cli_tool: CliTool::Codex,
        tmux_session: Some("taurhaus".to_string()),
        tmux_window: Some("1".to_string()),
        tmux_pane: Some("%9".to_string()),
        tmux_window_name: Some("stale".to_string()),
        state: SessionState::Active,
        session_id: Some("cached-session".to_string()),
        jsonl_path: None,
        recent_io: false,
        last_output_age_secs: None,
        activity_confidence: ActivityConfidence::High,
        activity_attribution: ActivityAttribution::Attributed,
        project_unattributed_active: false,
        group_kind: SessionGroupKind::MeshTeam,
        group_id: Some(team_name.to_string()),
        group_label: Some(team_name.to_string()),
        member_name: Some(member_name.to_string()),
        workflow_activity: None,
    };

    let reconciled = orchestrator
        .reconcile_team_presence_for_live_status_with_runtime_sessions(
            team_name,
            &[cached_snapshot],
        )
        .expect("presence reconcile");

    assert!(reconciled.is_empty());
    assert_eq!(
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload"),
        record
    );
    assert!(runtime
        .calls()
        .iter()
        .all(|call| !matches!(call, RuntimeCall::TerminatePid { pid: 4242 })));
}

#[test]
fn liveness_reconcile_refreshes_stale_claude_session_metadata_on_live_pane() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "claude-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Claude))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.pane_id = Some("%9".to_string());
    record.session_id = Some("stale-session".to_string());
    record.jsonl_path = Some(PathBuf::from("/tmp/stale-session.jsonl"));
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_detected_runtime_session(
        "%9",
        CliTool::Claude,
        Some("fresh-session"),
        Some("/tmp/fresh-session.jsonl"),
    );

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.session_id.as_deref(), Some("fresh-session"));
    assert_eq!(
        updated.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/fresh-session.jsonl"))
    );
    assert!(
        updated.last_seen_at.is_some(),
        "healthy refresh should stamp last_seen_at"
    );
    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::DetectSessionId { pane_id, cli_tool }
            if pane_id == "%9" && *cli_tool == CliTool::Claude
    )));
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPid { .. }
                | RuntimeCall::TerminatePid { .. }
                | RuntimeCall::SpawnDaemon { .. }
        )),
        "Claude live-pane refresh should not touch mesh daemon lifecycle"
    );
}

#[test]
fn liveness_reconcile_promotes_stale_session_dead_record_when_pane_is_alive() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::SessionDead;
    record.session_id = None;
    record.daemon_pid = None;
    record.pane_id = Some("%9".to_string());
    record.attached_at = None;
    record.last_seen_at = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_detected_runtime_session(
        "%9",
        CliTool::Codex,
        Some("session-%9"),
        Some("/tmp/codex-reviewer.jsonl"),
    );

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert!(
        updated.last_seen_at.is_some(),
        "healthy repair should stamp last_seen_at"
    );
    assert_eq!(updated.pane_id.as_deref(), Some("%9"));
    assert_eq!(updated.session_id.as_deref(), Some("session-%9"));
    assert_eq!(
        updated.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/codex-reviewer.jsonl"))
    );
    assert_eq!(updated.daemon_pid, Some(10000));
    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::DetectSessionId { pane_id, cli_tool }
            if pane_id == "%9" && *cli_tool == CliTool::Codex
    )));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { pane_id, team_name, member_name } if pane_id == "%9" && team_name == "architecture-final" && member_name == "codex-reviewer")));
    assert!(
        !calls.iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPid { .. } | RuntimeCall::TerminatePid { .. }
        )),
        "session-dead repair should start a daemon without daemon cleanup"
    );
}

#[test]
fn add_agent_persists_runtime_jsonl_path() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final-hot-add-jsonl";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");
    runtime.set_detected_runtime_session(
        "test-pane-1",
        CliTool::Codex,
        Some("session-test-pane-1"),
        Some("/tmp/new-agent.jsonl"),
    );

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none());

    let runtime_record = MemberRuntimeStore::load(tmp.path(), team_name, "new-agent")
        .expect("runtime state should exist");
    assert_eq!(
        runtime_record.jsonl_path.as_deref(),
        Some(std::path::Path::new("/tmp/new-agent.jsonl"))
    );
    assert_eq!(
        runtime_record.session_id.as_deref(),
        Some("session-test-pane-1")
    );
}

#[test]
fn liveness_reconcile_restarts_stale_non_running_daemon_for_live_pane() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_pid_running(4242, false);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.session_id, Some("session-%9".to_string()));
    assert_eq!(updated.daemon_pid, Some(10000));
    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 4242)));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { pane_id, team_name, member_name } if pane_id == "%9" && team_name == "architecture-final" && member_name == "codex-reviewer")));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 4242)),
        "dead daemon pid should be replaced, not terminated"
    );
}

#[test]
fn liveness_reconcile_restarts_running_daemon_when_binary_has_drifted() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_pid_running(4242, true);
    runtime.set_pid_current_mesh_binary(4242, false);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.session_id, Some("session-%9".to_string()));
    assert_eq!(updated.daemon_pid, Some(10000));

    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 4242)));
    assert!(calls.iter().any(
        |call| matches!(call, RuntimeCall::CheckPidCurrentMeshBinary { pid } if *pid == 4242)
    ));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::SpawnDaemon { pane_id, team_name, member_name } if pane_id == "%9" && team_name == "architecture-final" && member_name == "codex-reviewer")));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 4242)));
}

#[test]
fn trigger_team_self_heal_cycles_stale_team_daemon_and_restarts_drifted_member_daemon() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(
            team_name,
            member_with_project("team-lead", MemberRole::Lead, CliTool::Claude, "/tmp/lead"),
        )
        .expect("add should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    write_lead_credential(tmp.path(), team_name, "team-lead");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-%9".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_pid_running(4242, true);
    runtime.set_pid_current_mesh_binary(4242, false);
    runtime.set_team_daemon_current_mesh_binary(team_name, false);

    let result = orchestrator
        .trigger_team_self_heal(team_name)
        .expect("self-heal should succeed");

    assert!(result.runtime_candidate_found);
    assert!(result.member_liveness_reconciled);
    assert!(result.team_daemon_ensured);

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.daemon_pid, Some(10000));

    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::CheckTeamDaemonCurrentMeshBinary { team_name: recorded_team }
            if recorded_team == team_name
    )));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnTeamDaemon {
            team_name: recorded_team,
            operator_name,
        } if recorded_team == team_name && operator_name == "team-lead"
    )));
    assert!(calls.iter().any(
        |call| matches!(call, RuntimeCall::CheckPidCurrentMeshBinary { pid } if *pid == 4242)
    ));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 4242)));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnDaemon {
            pane_id,
            team_name: recorded_team,
            member_name: recorded_member,
        } if pane_id == "%9" && recorded_team == team_name && recorded_member == member_name
    )));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::StopTeamDaemon { team_name: recorded_team } if recorded_team == team_name
    )));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnTeamDaemon { team_name: recorded_team, .. } if recorded_team == team_name
    )));
}

#[test]
fn liveness_reconcile_adopts_existing_daemon_when_runtime_pid_is_missing() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = None;
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_matching_daemon_pids("%9", team_name, member_name, &[5555]);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.session_id, Some("session-%9".to_string()));
    assert_eq!(updated.daemon_pid, Some(5555));
    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::FindDaemon {
            pane_id,
            team_name,
            member_name
        } if pane_id == "%9" && team_name == "architecture-final" && member_name == "codex-reviewer"
    )));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })),
        "existing daemon should be adopted without respawn"
    );
}

#[test]
fn liveness_reconcile_adopts_existing_daemon_when_runtime_pid_is_stale() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_pid_running(4242, false);
    runtime.set_matching_daemon_pids("%9", team_name, member_name, &[5555]);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::Healthy);
    assert_eq!(updated.daemon_pid, Some(5555));
    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 4242)));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })),
        "stale runtime pid should adopt the live daemon instead of spawning a duplicate"
    );
}

#[test]
fn liveness_reconcile_terminates_duplicate_daemons_after_adopting_one() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = None;
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    runtime.set_pane_exists("%9", true);
    runtime.set_pane_dead("%9", false);
    runtime.set_pane_shell("%9", false);
    runtime.set_matching_daemon_pids("%9", team_name, member_name, &[5555, 6666]);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.daemon_pid, Some(5555));
    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 6666)));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::SpawnDaemon { .. })),
        "duplicate discovery should clean up extras instead of respawning"
    );
}

#[test]
fn liveness_reconcile_terminates_running_non_claude_daemon() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");
    runtime.set_pid_running(4242, true);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert_eq!(updated.daemon_pid, None);
    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 4242)));
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 4242)));
}

#[test]
fn liveness_reconcile_clears_non_running_non_claude_daemon_pid_without_terminate() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");
    runtime.set_pid_running(4242, false);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert_eq!(updated.daemon_pid, None);

    let calls = runtime.calls();
    assert!(calls
        .iter()
        .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 4242)));
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 4242)),
        "non-running daemon pid should be cleared without terminate call"
    );
}

#[test]
fn liveness_reconcile_skips_daemon_cleanup_for_claude_members() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "claude-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Claude))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::Healthy;
    record.session_id = Some("session-123".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = None;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(updated.health, HealthState::SessionDead);
    assert_eq!(updated.session_id, None);
    assert_eq!(updated.daemon_pid, Some(4242));
    assert!(
        !runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPid { .. } | RuntimeCall::TerminatePid { .. }
        )),
        "claude members should not run daemon pid cleanup"
    );
}

#[test]
fn liveness_reconcile_is_write_on_drift() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let mut record = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
    record.health = HealthState::SessionDead;
    record.session_id = Some("stale-session".to_string());
    record.daemon_pid = Some(4242);
    record.pane_id = Some("%9".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");
    runtime.set_pane_exists("%9", false);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
    assert_eq!(
        updated, record,
        "no write should occur without health drift"
    );
    assert!(
        !runtime.calls().iter().any(|call| matches!(
            call,
            RuntimeCall::CheckPid { .. } | RuntimeCall::TerminatePid { .. }
        )),
        "daemon cleanup should be skipped when health is already SessionDead"
    );
}

#[test]
fn liveness_reconcile_updates_only_drifted_members() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final";
    let drifted_member = "drifted-member";
    let healthy_member = "healthy-member";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(drifted_member, CliTool::Codex))
        .expect("add drifted member");
    orchestrator
        .add_member(team_name, sample_member(healthy_member, CliTool::Codex))
        .expect("add healthy member");

    let mut drifted =
        MemberRuntimeStore::load(tmp.path(), team_name, drifted_member).expect("load");
    drifted.health = HealthState::Healthy;
    drifted.session_id = Some("session-drifted".to_string());
    drifted.pane_id = None;
    MemberRuntimeStore::save(tmp.path(), team_name, drifted_member, &drifted)
        .expect("save drifted");

    let mut healthy =
        MemberRuntimeStore::load(tmp.path(), team_name, healthy_member).expect("load");
    healthy.health = HealthState::Healthy;
    healthy.session_id = Some("session-%11".to_string());
    healthy.daemon_pid = Some(9999);
    healthy.pane_id = Some("%11".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, healthy_member, &healthy)
        .expect("save healthy");

    runtime.set_pane_exists("%11", true);
    runtime.set_pane_dead("%11", false);
    runtime.set_pane_shell("%11", false);
    runtime.set_pid_running(9999, true);

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    let drifted_updated =
        MemberRuntimeStore::load(tmp.path(), team_name, drifted_member).expect("reload drifted");
    assert_eq!(drifted_updated.health, HealthState::SessionDead);
    assert_eq!(drifted_updated.session_id, None);

    let healthy_updated =
        MemberRuntimeStore::load(tmp.path(), team_name, healthy_member).expect("reload healthy");
    assert_eq!(
        healthy_updated, healthy,
        "member without drift should not be rewritten"
    );
    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CheckPid { pid } if *pid == 9999)),
        "healthy member should verify daemon liveness"
    );
    assert!(
        !calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::TerminatePid { pid } if *pid == 9999)),
        "healthy member should not trigger daemon cleanup"
    );
}

#[test]
fn remove_nonexistent_member_returns_not_found() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    let err = orchestrator
        .remove_member(team_name, "missing-member", None)
        .expect_err("expected not_found");
    assert_not_found(err);
}

#[test]
fn audit_log_captures_events() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    orchestrator
        .remove_member(team_name, member_name, None)
        .expect("remove should succeed");

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec!["team_created", "member_added", "member_removed"]
    );
}

#[test]
fn deliver_operator_notice_succeeds() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let result = orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect("delivery should succeed");
    assert!(result.delivered);

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_succeeded"
        ]
    );
}

#[test]
fn delivery_audit_reports_the_inbox_file_method_that_actually_ran() {
    // Regression: mesh-findings H6; the MeshBridged audit path reported
    // TmuxInjection even when operator traffic was durably appended to an inbox.
    let tmp = TempDir::new().expect("tempdir");
    let backend: Arc<dyn CoordinationBackend> = Arc::new(InboxFileBackend);
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "truthful-delivery-audit";
    let member_name = "codex-reviewer";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add member");

    let result = orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect("deliver");
    assert_eq!(result.method, DeliveryMethod::InboxFile);

    let audit = orchestrator.drain_audit_log();
    let attempted = audit.iter().find_map(|event| match event {
        AuditEvent::DeliveryAttempted(event) => Some(event.method),
        _ => None,
    });
    let succeeded = audit.iter().find_map(|event| match event {
        AuditEvent::DeliverySucceeded(event) => Some(event.method),
        _ => None,
    });
    assert_eq!(attempted, Some(DeliveryMethod::InboxFile));
    assert_eq!(succeeded, Some(DeliveryMethod::InboxFile));
}

#[test]
fn inbox_delivery_ensures_the_non_claude_member_daemon() {
    // Regression: mesh-findings H2; bypassing `mesh send` also bypassed its
    // wake path unless Taurhaus explicitly ensured the recipient daemon.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let backend: Arc<dyn CoordinationBackend> = Arc::new(MeshBridgedBackend::new_with_teams_dir(
        tmp.path().to_path_buf(),
    ));
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "direct-inbox-wake";
    let member_name = "codex-reviewer";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add member");
    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    member_runtime.pane_id = Some("%31".to_string());
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &member_runtime)
        .expect("save runtime");
    // The tmux floor permits unknown foreground commands such as `cat`; only
    // a known mismatched agent CLI proves that a legacy pane is foreign.
    runtime.set_pane_current_command("%31", Some("cat"));

    orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "wake".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect("deliver");

    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnDaemon {
            pane_id,
            team_name: recorded_team,
            member_name: recorded_member,
        } if pane_id == "%31" && recorded_team == team_name && recorded_member == member_name
    )));
    let saved_runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("saved runtime");
    assert_eq!(saved_runtime.daemon_pid, Some(10000));
    assert_eq!(
        MeshInboxStore::load(tmp.path(), team_name, member_name)
            .expect("inbox")
            .len(),
        1
    );
}

#[test]
fn inbox_delivery_does_not_wake_a_foreign_cli_pane() {
    // Regression: mesh-findings P3, tmux reused pane ids; daemons for
    // taurrust/gotaurus/espn pointed at claude panes.
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let backend: Arc<dyn CoordinationBackend> = Arc::new(MeshBridgedBackend::new_with_teams_dir(
        tmp.path().to_path_buf(),
    ));
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let team_name = "foreign-inbox-wake";
    let member_name = "codex-reviewer";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add member");
    let mut member_runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("runtime");
    member_runtime.pane_id = Some("%31".to_string());
    member_runtime.health = HealthState::Healthy;
    MemberRuntimeStore::save(tmp.path(), team_name, member_name, &member_runtime)
        .expect("save runtime");
    runtime.set_pane_current_command("%31", Some("claude"));

    orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "do not wake the foreign pane".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect("durable inbox delivery should still succeed");

    assert!(runtime
        .calls()
        .iter()
        .all(|call| !matches!(call, RuntimeCall::SpawnDaemon { .. })));
    let saved_runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("saved runtime");
    assert_eq!(saved_runtime.health, HealthState::SessionDead);
    assert_eq!(saved_runtime.daemon_pid, None);
    assert_eq!(
        MeshInboxStore::load(tmp.path(), team_name, member_name)
            .expect("inbox")
            .len(),
        1
    );
}

#[test]
fn team_daemon_is_skipped_without_the_lead_control_credential() {
    // Regression: commit 76c284e skipped mesh join for Claude leads, while the
    // liveness loop kept attempting an unauthenticated team-daemon start.
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "claude-only-no-auth";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    let mut lead = sample_member("team-lead", CliTool::Claude);
    lead.role = MemberRole::Lead;
    orchestrator.add_member(team_name, lead).expect("add lead");

    let (started, warning) = orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("credential absence is a skip, not an error");

    assert!(!started);
    assert!(warning
        .as_deref()
        .is_some_and(|message| message.contains("control credential")));
    assert!(runtime
        .calls()
        .iter()
        .all(|call| !matches!(call, RuntimeCall::SpawnTeamDaemon { .. })));
}

#[test]
fn team_daemon_is_skipped_when_the_credential_file_has_no_config_hash() {
    // Regression: 694b130 gated only on state/control_auth/<lead>.json even
    // though mesh authenticates team-daemon against controlAuthTokenHash too.
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "claude-lead-missing-hash";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    let mut lead = sample_member("team-lead", CliTool::Claude);
    lead.role = MemberRole::Lead;
    orchestrator.add_member(team_name, lead).expect("add lead");
    let credential_dir = tmp
        .path()
        .join(team_name)
        .join("state")
        .join("control_auth");
    std::fs::create_dir_all(&credential_dir).expect("credential dir");
    std::fs::write(
        credential_dir.join("team-lead.json"),
        r#"{"name":"team-lead","token":"test-token"}"#,
    )
    .expect("credential file");

    let (started, warning) = orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("missing config hash is a skip, not an error");

    assert!(!started);
    assert!(warning
        .as_deref()
        .is_some_and(|message| message.contains("controlAuthTokenHash")));
    assert!(runtime
        .calls()
        .iter()
        .all(|call| !matches!(call, RuntimeCall::SpawnTeamDaemon { .. })));
}

#[test]
fn missing_team_daemon_credential_emits_one_reason_event_per_team() {
    // Regression: commit 76c284e left Claude-only teams retrying an
    // unauthenticated team daemon without one actionable skip event.
    let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("tempdir");
    let log_path = tmp.path().join("events.jsonl");
    let log_state = taurhaus_lib::logging::LogFileState::new(log_path.clone()).expect("log state");
    taurhaus_lib::logging::install_global_sink(&log_state);

    let (mut orchestrator, _) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "claude-only-skip-event";
    orchestrator
        .create_team(team_name, None)
        .expect("create team");
    let mut lead = sample_member("team-lead", CliTool::Claude);
    lead.role = MemberRole::Lead;
    orchestrator.add_member(team_name, lead).expect("add lead");

    orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("first skip");
    orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("second skip");

    let mut config = TeamConfigStore::load(tmp.path(), team_name).expect("load config");
    config.members[0].extra.insert(
        "controlAuthTokenHash".to_string(),
        serde_json::Value::String("sha256:test-token".to_string()),
    );
    TeamConfigStore::save(tmp.path(), team_name, &config).expect("save hash");
    let credential_dir = tmp
        .path()
        .join(team_name)
        .join("state")
        .join("control_auth");
    std::fs::create_dir_all(&credential_dir).expect("credential dir");
    let credential_path = credential_dir.join("team-lead.json");
    std::fs::write(
        &credential_path,
        r#"{"name":"team-lead","token":"test-token"}"#,
    )
    .expect("credential file");
    orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("authenticated transition");
    std::fs::remove_file(&credential_path).expect("remove credential");
    orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("new degraded transition");
    orchestrator
        .ensure_team_daemon_for_wrapper(team_name)
        .expect("deduplicated degraded state");
    log_state
        .flush_for_test()
        .expect("flush structured log sink");

    let events = std::fs::read_to_string(log_path)
        .expect("read structured log")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("parse log record"))
        .filter(|record| {
            record["event"] == "coordination.team_daemon.skipped"
                && record["team_name"] == team_name
        })
        .collect::<Vec<_>>();
    // Regression: 694b130 used a monotonic process-global set, suppressing all
    // later skip events after a team recovered and degraded again.
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["reason"], "missing_lead_control_credential");
    assert_eq!(events[1]["reason"], "missing_lead_control_credential");
}

#[test]
fn deliver_to_nonexistent_member_fails() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");

    let err = orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: "missing-member".to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect_err("delivery should fail");
    assert_not_found(err);
}

#[test]
fn deliver_updates_runtime_last_seen() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let before = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
        .expect("runtime should exist before delivery");
    assert!(before.last_seen_at.is_none());

    orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect("delivery should succeed");

    let after = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
        .expect("runtime should exist after delivery");
    assert!(after.last_seen_at.is_some());
}

#[test]
fn deliver_backend_failure_emits_failed_event() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated delivery failure".to_string(),
    ));
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");

    let err = orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "status?".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect_err("delivery should fail");
    match err {
        CoordinationError::Backend(msg) => assert!(msg.contains("simulated")),
        other => panic!("expected backend error, got {other:?}"),
    }

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_failed"
        ]
    );
}

#[test]
fn deliver_false_result_is_treated_as_failure() {
    let tmp = TempDir::new().expect("tempdir");
    let backend: Arc<dyn CoordinationBackend> = Arc::new(UndeliveredBackend);
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "taurhaus-team";
    let member_name = "design-taurhaus";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Claude))
        .expect("add should succeed");

    // Regression: the orchestrator used to record success and update runtime
    // state even when the backend explicitly reported `delivered: false`.
    let err = orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "ACTION REQUIRED: Review the packet.".to_string(),
            sender_name: Some("team-lead".to_string()),
            operational_context: None,
        }))
        .expect_err("undelivered result should fail");
    match err {
        CoordinationError::Backend(message) => {
            assert!(message.contains("backend reported undelivered"))
        }
        other => panic!("expected backend error, got {other:?}"),
    }

    let after = MemberRuntimeStore::load(tmp.path(), team_name, member_name)
        .expect("runtime should still exist");
    assert!(after.last_seen_at.is_none());

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_failed"
        ]
    );
}

#[test]
fn full_lifecycle() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, Some("lifecycle".to_string()))
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member("member-a", CliTool::Codex))
        .expect("add a should succeed");
    orchestrator
        .add_member(team_name, sample_member("member-b", CliTool::Claude))
        .expect("add b should succeed");
    orchestrator
        .remove_member(team_name, "member-a", Some("done".to_string()))
        .expect("remove should succeed");
    orchestrator
        .disband_team(team_name, Some("shutdown".to_string()))
        .expect("disband should succeed");

    let events = orchestrator.drain_audit_log();
    let event_types: Vec<&str> = events.iter().map(|event| event.event_type()).collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "member_added",
            "delivery_attempted",
            "delivery_succeeded",
            "member_removed",
            "team_disbanded"
        ]
    );
}

#[test]
fn flush_audit_to_log_clears_buffer() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator
        .create_team("architecture-final", Some("desc".to_string()))
        .expect("create should succeed");
    assert!(
        !orchestrator.drain_audit_log().is_empty(),
        "sanity: event should exist"
    );

    orchestrator
        .create_team("second-team", Some("desc".to_string()))
        .expect("create should succeed");
    orchestrator.flush_audit_to_log();
    assert!(
        orchestrator.drain_audit_log().is_empty(),
        "flush should clear buffered events"
    );
}

#[test]
fn lease_claimed_emits_event() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator.record_lease_claimed("architecture-final", "codex-reviewer", 4242, "inst-1");

    let events = orchestrator.drain_audit_log();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AuditEvent::LeaseClaimed(payload) => {
            assert_eq!(payload.team_name, "architecture-final");
            assert_eq!(payload.member_name, "codex-reviewer");
            assert_eq!(payload.owner_pid, 4242);
            assert_eq!(payload.instance_uuid, "inst-1");
        }
        other => panic!("expected lease_claimed event, got {other:?}"),
    }
}

#[test]
fn lease_reclaimed_emits_event() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    orchestrator.record_lease_reclaimed("architecture-final", "codex-reviewer", 1111, 2222);

    let events = orchestrator.drain_audit_log();
    assert_eq!(events.len(), 1);
    match &events[0] {
        AuditEvent::LeaseReclaimed(payload) => {
            assert_eq!(payload.team_name, "architecture-final");
            assert_eq!(payload.member_name, "codex-reviewer");
            assert_eq!(payload.previous_pid, 1111);
            assert_eq!(payload.new_pid, 2222);
        }
        other => panic!("expected lease_reclaimed event, got {other:?}"),
    }
}

#[test]
fn all_mutations_emit_events() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";
    let member_name = "codex-reviewer";

    orchestrator
        .create_team(team_name, Some("audit coverage".to_string()))
        .expect("create should succeed");
    orchestrator
        .add_member(team_name, sample_member(member_name, CliTool::Codex))
        .expect("add should succeed");
    orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: member_name.to_string(),
            team_name: team_name.to_string(),
            message: "check status".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect("delivery should succeed");
    orchestrator.record_lease_claimed(team_name, member_name, 4242, "inst-1");
    orchestrator.record_lease_reclaimed(team_name, member_name, 4242, 5252);
    orchestrator
        .remove_member(team_name, member_name, Some("cleanup".to_string()))
        .expect("remove should succeed");
    orchestrator
        .disband_team(team_name, Some("shutdown".to_string()))
        .expect("disband should succeed");

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert_eq!(
        event_types,
        vec![
            "team_created",
            "member_added",
            "delivery_attempted",
            "delivery_succeeded",
            "lease_claimed",
            "lease_reclaimed",
            "member_removed",
            "team_disbanded"
        ]
    );
}

#[test]
fn invalid_team_name_is_rejected_for_create() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    let err = orchestrator
        .create_team("bad/name", None)
        .expect_err("path separators must be rejected");
    match err {
        CoordinationError::Validation(message) => assert!(message.contains("must not contain")),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn invalid_member_name_is_rejected_for_add_member() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final";

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");

    let err = orchestrator
        .add_member(team_name, sample_member("bad/member", CliTool::Codex))
        .expect_err("invalid member name should fail");
    match err {
        CoordinationError::Validation(message) => assert!(message.contains("path separators")),
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn deliver_to_nonexistent_team_fails_without_delivery_audit_events() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);

    let err = orchestrator
        .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
            member_name: "codex-reviewer".to_string(),
            team_name: "missing-team".to_string(),
            message: "status?".to_string(),
            sender_name: None,
            operational_context: None,
        }))
        .expect_err("delivery should fail");
    assert_not_found(err);

    let event_types: Vec<&str> = orchestrator
        .drain_audit_log()
        .into_iter()
        .map(|event| event.event_type())
        .collect();
    assert!(
        event_types.is_empty(),
        "no delivery audit event should be emitted before team lookup succeeds"
    );
}

#[test]
fn initialize_team_full_success_path() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = initialize_request("architecture-final-init");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.team_name, "architecture-final-init");
    assert!(report.failed_step.is_none());
    assert!(!report.retryable);
    assert_eq!(
        report.succeeded_steps,
        vec![
            "validate_configuration",
            "create_team",
            "add_lead",
            "create_panes",
            "launch_sessions",
            "join_mesh",
            "start_daemons",
            "send_onboarding",
        ]
    );

    let lead_runtime = MemberRuntimeStore::load(tmp.path(), "architecture-final-init", "team-lead")
        .expect("lead runtime should exist");
    assert!(
        lead_runtime.pane_id.is_some(),
        "lead pane should be created when lead_mode=launch_new"
    );
    assert!(
        lead_runtime.daemon_pid.is_none(),
        "claude lead should not start mesh daemon when launched natively"
    );
}

#[test]
fn initialize_team_ensures_team_daemon_running() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    runtime.set_mesh_join_teams_dir(tmp.path());
    let request = initialize_request("architecture-final-init");
    write_lead_credential(tmp.path(), "architecture-final-init", "team-lead");

    orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");

    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnTeamDaemon {
            team_name,
            operator_name,
        } if team_name == "architecture-final-init" && operator_name == "team-lead"
    )));
}

#[test]
fn initialize_team_attach_existing_skips_lead_launch() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let mut request = initialize_request("architecture-final-attach-existing");
    request.lead_mode = LeadMode::AttachExisting;

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none());

    let lead_runtime = MemberRuntimeStore::load(
        tmp.path(),
        "architecture-final-attach-existing",
        "team-lead",
    )
    .expect("lead runtime should exist");
    assert!(
        lead_runtime.pane_id.is_none(),
        "lead pane should remain unset when lead_mode=attach_existing"
    );
    assert!(
        lead_runtime.daemon_pid.is_none(),
        "lead daemon should remain unset when lead_mode=attach_existing"
    );
}

fn assert_non_claude_lead_launch_new_uses_sidecar(
    lead_tool: &str,
    lead_model: &str,
    team_name: &str,
    expect_session_capture: bool,
) {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let mut request = initialize_request(team_name);
    request.lead.cli_tool = lead_tool.to_string();
    request.lead.model = lead_model.to_string();

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert!(
        report.failed_step.is_none(),
        "non-Claude launch-new lead should initialize cleanly"
    );

    let lead_runtime =
        MemberRuntimeStore::load(tmp.path(), team_name, "team-lead").expect("lead runtime");
    assert!(
        lead_runtime.pane_id.is_some(),
        "launch-new lead should still allocate a pane"
    );
    let expected_session_id = expect_session_capture.then(|| {
        lead_runtime
            .pane_id
            .as_deref()
            .map(|pane_id| format!("session-{pane_id}"))
            .expect("lead pane id")
    });
    assert_eq!(lead_runtime.session_id, expected_session_id);
    assert_eq!(
        lead_runtime.daemon_pid,
        Some(10000),
        "non-Claude leads should start through the mesh sidecar"
    );

    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::JoinMesh { team_name: call_team, member_name, .. }
            if call_team == team_name && member_name == "team-lead"
    )));
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnDaemon { team_name: call_team, member_name, .. }
            if call_team == team_name && member_name == "team-lead"
    )));
    let detected_session_id = calls.iter().any(|call| {
        matches!(
            call,
            RuntimeCall::DetectSessionId { pane_id, cli_tool }
                if pane_id == lead_runtime.pane_id.as_deref().unwrap_or_default()
                    && *cli_tool == CliTool::from_alias(lead_tool).expect("known tool")
        )
    });
    assert_eq!(detected_session_id, expect_session_capture);
}

#[test]
fn initialize_team_codex_lead_launch_new_uses_sidecar_lifecycle() {
    assert_non_claude_lead_launch_new_uses_sidecar(
        "codex",
        "gpt-5.4",
        "architecture-final-init-codex-lead",
        true,
    );
}

#[test]
fn initialize_team_agy_lead_launch_new_uses_sidecar_lifecycle() {
    // Regression: e86980b used the Google harness's project-scoped SessionSource as a
    // per-pane runtime identity source, persisting the wrong session id.
    assert_non_claude_lead_launch_new_uses_sidecar(
        "agy",
        "gemini-3.7-flash-high",
        "architecture-final-init-agy-lead",
        false,
    );
}

#[test]
fn initialize_team_rejects_attach_existing_for_non_claude_lead() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let mut request = initialize_request("architecture-final-attach-existing-codex");
    request.lead_mode = LeadMode::AttachExisting;
    request.lead.cli_tool = "codex".to_string();
    request.lead.model = "gpt-5.4".to_string();

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(
        report.failed_step.as_deref(),
        Some("validate_configuration")
    );
    assert!(
        report
            .message
            .contains("attach-existing is not supported yet for 'codex' leads"),
        "report should explain the unsupported lead mode clearly"
    );
}

#[test]
fn initialize_team_duplicate_team_returns_partial_failure_report() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = initialize_request("architecture-final-init");

    orchestrator
        .create_team("architecture-final-init", None)
        .expect("seed team");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("create_team"));
    assert!(report.retryable);
    assert_eq!(report.succeeded_steps, vec!["validate_configuration"]);
    assert_eq!(report.steps[0].step, "validate_configuration");
    assert_eq!(report.steps[1].step, "create_team");
    assert_eq!(report.steps[1].status, StepStatus::Failed);
}

#[test]
fn initialize_team_agent_addition_failure_is_partial() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let mut request = initialize_request("architecture-final-init");
    request.agents[1].name = "bad/member".to_string();

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("add_lead"));
    assert!(report.retryable);
    assert_eq!(
        report.succeeded_steps,
        vec!["validate_configuration", "create_team"]
    );
    assert_eq!(
        report
            .steps
            .iter()
            .map(|step| step.step.as_str())
            .collect::<Vec<_>>(),
        vec!["validate_configuration", "create_team", "add_lead",]
    );
}

#[test]
fn initialize_team_join_mesh_failure_reports_partial_and_cleans_up() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(ProjectPathCheckingRuntime::new());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        Arc::new(FakeBackend::default()),
        runtime,
    );
    let request = initialize_request("architecture-final-join-failure");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");

    assert_eq!(report.failed_step.as_deref(), Some("join_mesh"));
    assert_eq!(
        report.succeeded_steps,
        vec![
            "validate_configuration",
            "create_team",
            "add_lead",
            "create_panes",
            "launch_sessions",
        ]
    );
    assert!(
        !tmp.path().join("architecture-final-join-failure").exists(),
        "team should be cleaned up when join_mesh fails"
    );
}

#[test]
fn initialize_team_join_mesh_uses_member_project_paths() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(ProjectPathCheckingRuntime::new());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        Arc::new(FakeBackend::default()),
        runtime.clone(),
    );

    let lead_project = tmp.path().join("proj-core");
    let frontend_project = tmp.path().join("proj-web");
    let reviewer_project = tmp.path().join("proj-api");
    std::fs::create_dir_all(&lead_project).expect("lead project");
    std::fs::create_dir_all(&frontend_project).expect("frontend project");
    std::fs::create_dir_all(&reviewer_project).expect("reviewer project");

    let mut request = initialize_request("architecture-final-join-paths");
    request.lead.project_id = lead_project.to_string_lossy().to_string();
    request.agents[0].project_id = frontend_project.to_string_lossy().to_string();
    request.agents[1].project_id = reviewer_project.to_string_lossy().to_string();

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none(), "initialize should succeed");

    let join_calls: Vec<(String, String)> = runtime
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            RuntimeCall::JoinMesh {
                member_name,
                project_id,
                ..
            } => Some((member_name, project_id)),
            _ => None,
        })
        .collect();

    assert!(join_calls
        .iter()
        .any(|(member, path)| member == "frontend-dev" && path == &request.agents[0].project_id));
    assert!(join_calls
        .iter()
        .any(|(member, path)| member == "reviewer" && path == &request.agents[1].project_id));
}

#[test]
fn initialize_failure_send_onboarding_triggers_disband_teardown() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding failure".to_string(),
    ));
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let request = initialize_request("architecture-final-init-cleanup");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(!tmp.path().join("architecture-final-init-cleanup").exists());
    assert_eq!(
        fake.call_counts(),
        (0, 1, 0, 3),
        "initialize cleanup should tear down the app-owned lead and both non-lead members"
    );
}

#[test]
fn initialize_failure_send_onboarding_cleans_up_mesh_backed_lead() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding failure".to_string(),
    ));
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let mut orchestrator = CoordinationOrchestrator::new_with_runtime(
        tmp.path().to_path_buf(),
        backend,
        runtime.clone(),
    );
    let mut request = initialize_request("architecture-final-init-mesh-lead-cleanup");
    request.lead.cli_tool = "codex".to_string();
    request.lead.model = "gpt-5.4".to_string();

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(!tmp
        .path()
        .join("architecture-final-init-mesh-lead-cleanup")
        .exists());

    let calls = runtime.calls();
    assert!(calls.iter().any(|call| matches!(
        call,
        RuntimeCall::ClearDaemonPidFile { team_name, member_name }
            if team_name == "architecture-final-init-mesh-lead-cleanup" && member_name == "team-lead"
    )));
    assert_eq!(
        fake.call_counts(),
        (0, 1, 0, 3),
        "initialize cleanup should tear down the mesh-backed lead and both agents"
    );
}

#[test]
fn initialize_team_steps_are_ordered() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = initialize_request("architecture-final-order");

    let report = orchestrator
        .initialize_team(&request)
        .expect("pipeline should return report");
    let step_names: Vec<&str> = report.steps.iter().map(|step| step.step.as_str()).collect();
    assert_eq!(
        step_names,
        vec![
            "validate_configuration",
            "create_team",
            "add_lead",
            "create_panes",
            "launch_sessions",
            "join_mesh",
            "start_daemons",
            "send_onboarding",
        ]
    );
}

#[test]
fn add_agent_to_team_full_success() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final-hot-add";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none());
    assert!(!report.retryable);
    assert_eq!(report.member_name, "new-agent");
    assert_eq!(
        report.succeeded_steps,
        vec![
            "validate",
            "create_pane",
            "launch_session",
            "join_mesh",
            "start_daemon",
            "send_onboarding",
            "update_roster",
        ]
    );

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert!(status
        .config
        .members
        .iter()
        .any(|member| member.name == "new-agent"));
    assert!(runtime.calls().iter().any(|call| matches!(
        call,
        RuntimeCall::SpawnTeamDaemon {
            team_name,
            operator_name,
        } if team_name == "architecture-final-hot-add" && operator_name == "team-lead"
    )));
}

#[test]
fn add_agent_join_mesh_uses_selected_project_path() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "architecture-final-hot-add-join-path";
    create_running_team(&mut orchestrator, team_name);
    let mut request = add_agent_request(team_name, "new-agent", "codex");
    request.agent.project_id = "/tmp/selected-project".to_string();

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert!(report.failed_step.is_none());

    let join_call = runtime
        .calls()
        .into_iter()
        .find_map(|call| match call {
            RuntimeCall::JoinMesh {
                team_name,
                member_name,
                project_id,
                model,
                ..
            } => Some((team_name, member_name, project_id, model)),
            _ => None,
        })
        .expect("join_mesh call should be recorded");
    assert_eq!(join_call.0, team_name);
    assert_eq!(join_call.1, "new-agent");
    assert_eq!(join_call.3, "model");
    assert_eq!(join_call.2, "/tmp/selected-project");
}

#[test]
fn add_agent_update_roster_is_idempotent_when_mesh_preadds_member() {
    let tmp = TempDir::new().expect("tempdir");
    let runtime = Arc::new(MeshPreAddRuntime::new(
        tmp.path().to_path_buf(),
        PathBuf::from("/tmp/app-data-fallback"),
    ));
    let backend: Arc<dyn CoordinationBackend> = Arc::new(FakeBackend::default());
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);
    let team_name = "architecture-final-hot-add-idempotent";
    create_running_team(&mut orchestrator, team_name);
    let mut request = add_agent_request(team_name, "new-agent", "codex");
    request.agent.project_id = "/tmp/selected-project".to_string();

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert!(
        report.failed_step.is_none(),
        "add-agent should succeed even if mesh pre-added member: {}",
        report.message
    );

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    let matching_members: Vec<&Member> = status
        .config
        .members
        .iter()
        .filter(|member| member.name == "new-agent")
        .collect();
    assert_eq!(matching_members.len(), 1, "member should not be duplicated");
    assert_eq!(
        matching_members[0].project_path,
        PathBuf::from("/tmp/selected-project"),
        "project path should reflect user-selected dropdown value"
    );

    let runtime_record = MemberRuntimeStore::load(tmp.path(), team_name, "new-agent")
        .expect("runtime state should exist");
    assert_eq!(
        runtime_record.pane_id.as_deref(),
        Some("test-pane-1"),
        "runtime should still capture pane created during hot-add"
    );
}

#[test]
fn add_claude_agent_registers_member_before_launch() {
    let tmp = TempDir::new().expect("tempdir");
    let team_name = "architecture-final-hot-add-claude";
    let member_name = "ponyhof-asset-generator";
    let runtime = Arc::new(ClaudeLaunchRosterRuntime::new(
        tmp.path().to_path_buf(),
        team_name,
        member_name,
    ));
    let backend: Arc<dyn CoordinationBackend> = Arc::new(FakeBackend::default());
    let mut orchestrator =
        CoordinationOrchestrator::new_with_runtime(tmp.path().to_path_buf(), backend, runtime);
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, member_name, "claude");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");

    assert!(
        report.failed_step.is_none(),
        "claude hot-add should not launch before roster registration: {}",
        report.message
    );

    let status = orchestrator
        .get_team_status(team_name)
        .expect("status should load");
    assert!(status
        .config
        .members
        .iter()
        .any(|member| member.name == member_name && member.cli_tool == CliTool::Claude));
}

#[test]
fn add_agent_duplicate_name_rejected() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final-hot-add";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "existing-dev", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("validate"));
    assert!(report.retryable);
    assert!(report.succeeded_steps.is_empty());
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].status, StepStatus::Failed);
}

#[test]
fn add_agent_team_not_found_fails_before_pipeline_progress() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let request = add_agent_request("missing-team", "new-agent", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("validate"));
    assert!(report.succeeded_steps.is_empty());
    assert_eq!(report.steps.len(), 1);
    assert_eq!(report.steps[0].step, "validate");
}

#[test]
fn add_agent_mid_flow_failure_preserves_existing_team_state() {
    let tmp = TempDir::new().expect("tempdir");
    let fake = Arc::new(FakeBackend::default());
    fake.set_deliver_error(CoordinationError::Backend(
        "simulated onboarding failure".to_string(),
    ));
    let backend: Arc<dyn CoordinationBackend> = fake.clone();
    let mut orchestrator = new_orchestrator_with_backend(&tmp, backend);
    let team_name = "architecture-final-hot-add";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");

    let before = orchestrator
        .get_team_status(team_name)
        .expect("status before")
        .config
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    assert_eq!(report.failed_step.as_deref(), Some("send_onboarding"));
    assert!(report.retryable);

    let after = orchestrator
        .get_team_status(team_name)
        .expect("status after")
        .config
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(before, after, "existing team roster should be unchanged");
    assert!(!after.contains(&"new-agent".to_string()));
    assert_eq!(
        fake.call_counts(),
        (0, 1, 0, 1),
        "failed hot-add should roll back mesh membership"
    );
}

#[test]
fn add_agent_step_ordering_is_stable() {
    let tmp = TempDir::new().expect("tempdir");
    let mut orchestrator = new_orchestrator(&tmp);
    let team_name = "architecture-final-hot-add-order";
    create_running_team(&mut orchestrator, team_name);
    let request = add_agent_request(team_name, "new-agent", "codex");

    let report = orchestrator
        .add_agent_to_team(&request)
        .expect("pipeline should return report");
    let step_names: Vec<&str> = report.steps.iter().map(|step| step.step.as_str()).collect();
    assert_eq!(
        step_names,
        vec![
            "validate",
            "create_pane",
            "launch_session",
            "join_mesh",
            "start_daemon",
            "send_onboarding",
            "update_roster",
        ]
    );
}

#[test]
fn grok_runtime_identity_is_backfilled_once_its_registry_appears() {
    // Regression: commit 16de5ec declared `runtime_session_capture: false` for
    // grok, so a managed member never recorded a session id. grok writes its
    // `active_sessions.json` row at the first prompt rather than at process
    // start, so the identity has to be backfilled by liveness — without it the
    // compaction bridge falls back to matching by cwd, which two grok members
    // on one project share, and neither of them is reinjected.
    let tmp = TempDir::new().expect("tempdir");
    let (mut orchestrator, runtime) = new_orchestrator_with_recording_runtime(&tmp);
    let team_name = "grok-pair";
    let members = [
        ("grok-one", "%11", "01a04585-2d53-7123-8000-00000000000a"),
        ("grok-two", "%12", "01a04585-2d53-7123-8000-00000000000b"),
    ];

    orchestrator
        .create_team(team_name, None)
        .expect("create should succeed");
    for (member_name, pane_id, session_id) in members {
        orchestrator
            .add_member(
                team_name,
                member_with_project(member_name, MemberRole::Agent, CliTool::Grok, "/tmp/shared"),
            )
            .expect("add should succeed");
        let mut record =
            MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("load");
        record.health = HealthState::Healthy;
        record.pane_id = Some(pane_id.to_string());
        // The registry row does not exist until the member's first prompt.
        record.session_id = None;
        MemberRuntimeStore::save(tmp.path(), team_name, member_name, &record).expect("save");
        runtime.set_detected_runtime_session(
            pane_id,
            CliTool::Grok,
            Some(session_id),
            Some(&format!("/tmp/{session_id}/events.jsonl")),
        );
    }

    orchestrator
        .reconcile_team_liveness(team_name)
        .expect("reconcile should succeed");

    for (member_name, _, session_id) in members {
        let updated = MemberRuntimeStore::load(tmp.path(), team_name, member_name).expect("reload");
        assert_eq!(
            updated.session_id.as_deref(),
            Some(session_id),
            "{member_name} keeps its own grok session identity"
        );
    }
}
