use std::path::PathBuf;

use chrono::Utc;

use crate::commands::coordination_types::{
    AddAgentReport, AddAgentRequest, AgentSetupConfig, InitializeReport, InitializeTeamRequest,
    StepProgress, StepStatus,
};
use crate::coordination::delivery::DeliveryRenderer;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfigStore};
use crate::coordination::validation::{
    validate_member_name, validate_non_empty, validate_team_name,
};
use crate::session_scanner::cli_tool::CliTool;

impl CoordinationOrchestrator {
    /// Initialize a team via the high-level multi-step pipeline.
    ///
    /// Pipeline steps:
    /// 1. validate_configuration
    /// 2. create_team
    /// 3. add_lead
    /// 4. create_panes (stubbed)
    /// 5. launch_sessions (stubbed)
    /// 6. join_mesh (stubbed)
    /// 7. start_daemons (stubbed)
    /// 8. send_onboarding (render + deliver)
    pub fn initialize_team(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<InitializeReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();

        if let Err(err) = self.validate_initialize_configuration(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "validate_configuration",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "validate_configuration",
            "configuration validated",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.create_team(&request.team_name, request.team_description.clone()) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "create_team",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_team",
            "team created",
            &mut succeeded_steps,
            &mut steps,
        );

        let lead_member = match member_from_agent_setup(&request.lead, MemberRole::Lead) {
            Ok(member) => member,
            Err(err) => {
                return Ok(failed_initialize_report(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                ))
            }
        };
        if let Err(err) = self.add_member(&request.team_name, lead_member) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "add_lead",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("add_lead", "lead added", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.create_panes_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "create_panes",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_panes",
            "agent panes created",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.launch_sessions_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "launch_sessions",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "launch_sessions",
            "cli sessions launched",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.join_mesh_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.start_daemons_stub(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "start_daemons",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "start_daemons",
            "mesh daemons started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.send_onboarding_messages(request) {
            return Ok(failed_initialize_report(
                &request.team_name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "send_onboarding",
            "onboarding messages sent",
            &mut succeeded_steps,
            &mut steps,
        );

        Ok(InitializeReport {
            team_name: request.team_name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
            steps,
        })
    }

    /// Hot-add a single agent to an already running team.
    ///
    /// Pipeline steps:
    /// 1. validate
    /// 2. create_pane (stubbed)
    /// 3. launch_session (stubbed)
    /// 4. join_mesh (stubbed)
    /// 5. start_daemon (stubbed)
    /// 6. send_onboarding (render + backend delivery)
    /// 7. update_roster
    pub fn add_agent_to_team(
        &mut self,
        request: &AddAgentRequest,
    ) -> Result<AddAgentReport, CoordinationError> {
        let mut succeeded_steps = Vec::new();
        let mut steps = Vec::new();

        if let Err(err) = self.validate_add_agent_request(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "validate",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "validate",
            "request validated",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.create_pane_for_agent_stub(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "create_pane",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "create_pane",
            "agent pane created",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.launch_session_for_agent_stub(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "launch_session",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "launch_session",
            "cli session launched",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.join_mesh_for_agent_stub(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.start_daemon_for_agent_stub(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "start_daemon",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "start_daemon",
            "mesh daemon started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.send_onboarding_for_agent(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "send_onboarding",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "send_onboarding",
            "onboarding delivered",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.update_roster_with_agent(request) {
            return Ok(failed_add_agent_report(
                &request.team_name,
                &request.agent.name,
                "update_roster",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded(
            "update_roster",
            "team roster updated",
            &mut succeeded_steps,
            &mut steps,
        );

        Ok(AddAgentReport {
            team_name: request.team_name.clone(),
            member_name: request.agent.name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "agent added".to_string(),
            steps,
        })
    }

    fn validate_initialize_configuration(
        &self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        validate_team_name(&request.team_name)?;
        validate_non_empty("lead name", &request.lead.name)?;
        validate_non_empty("lead cli tool", &request.lead.cli_tool)?;

        let mut seen = std::collections::HashSet::new();
        seen.insert(request.lead.name.trim().to_string());
        for agent in &request.agents {
            validate_non_empty("agent name", &agent.name)?;
            validate_non_empty("agent cli tool", &agent.cli_tool)?;
            let inserted = seen.insert(agent.name.trim().to_string());
            if !inserted {
                return Err(CoordinationError::Validation(format!(
                    "duplicate member name '{}' in initialize request",
                    agent.name
                )));
            }
        }

        Ok(())
    }

    fn create_panes_stub(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        for (idx, agent) in request.agents.iter().enumerate() {
            let member = member_from_agent_setup(agent, MemberRole::Agent)?;
            self.add_member(&request.team_name, member.clone())?;

            if let Ok(mut runtime) =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &member.name)
            {
                runtime.pane_id = Some(format!("%{}", idx + 1));
                runtime.attached_at = Some(Utc::now());
                runtime.health = HealthState::Healthy;
                MemberRuntimeStore::save(
                    &self.teams_dir,
                    &request.team_name,
                    &member.name,
                    &runtime,
                )?;
            }
        }
        Ok(())
    }

    fn launch_sessions_stub(
        &self,
        _request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn join_mesh_stub(&self, _request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn start_daemons_stub(
        &self,
        _request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn send_onboarding_messages(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        for agent in &request.agents {
            let cli_tool = parse_cli_tool(&agent.cli_tool)?;
            if cli_tool == CliTool::Claude {
                continue;
            }

            let onboarding = DeliveryRenderer::render_onboarding(
                &request.team_name,
                &agent.name,
                &request.lead.name,
            );
            self.deliver_message(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
            }))?;
        }
        Ok(())
    }

    fn validate_add_agent_request(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        validate_team_name(&request.team_name)?;
        validate_member_name(&request.agent.name)?;
        validate_non_empty("agent project id", &request.agent.project_id)?;
        validate_non_empty("agent cli tool", &request.agent.cli_tool)?;
        let config = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        if config
            .members
            .iter()
            .any(|member| member.name == request.agent.name)
        {
            return Err(CoordinationError::Conflict(format!(
                "member '{}' already exists in team '{}'",
                request.agent.name, request.team_name
            )));
        }
        Ok(())
    }

    fn create_pane_for_agent_stub(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let _ = parse_cli_tool(&request.agent.cli_tool)?;
        Ok(())
    }

    fn launch_session_for_agent_stub(
        &self,
        _request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }

    fn join_mesh_for_agent_stub(&self, request: &AddAgentRequest) -> Result<(), CoordinationError> {
        let _ = parse_cli_tool(&request.agent.cli_tool)?;
        Ok(())
    }

    fn start_daemon_for_agent_stub(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let _ = parse_cli_tool(&request.agent.cli_tool)?;
        Ok(())
    }

    fn send_onboarding_for_agent(
        &self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let cli_tool = parse_cli_tool(&request.agent.cli_tool)?;
        if cli_tool == CliTool::Claude {
            return Ok(());
        }
        let team = TeamConfigStore::load(&self.teams_dir, &request.team_name)?;
        let lead_name = team
            .members
            .iter()
            .find(|member| member.role == MemberRole::Lead)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| "team-lead".to_string());

        let onboarding = DeliveryRenderer::render_onboarding(
            &request.team_name,
            &request.agent.name,
            &lead_name,
        );
        self.backend
            .deliver(DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
                member_name: request.agent.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
            }))?;
        Ok(())
    }

    fn update_roster_with_agent(
        &mut self,
        request: &AddAgentRequest,
    ) -> Result<(), CoordinationError> {
        let member = member_from_agent_setup(&request.agent, MemberRole::Agent)?;
        self.add_member(&request.team_name, member)
    }
}

fn mark_step_succeeded(
    step: &str,
    message: &str,
    succeeded_steps: &mut Vec<String>,
    steps: &mut Vec<StepProgress>,
) {
    succeeded_steps.push(step.to_string());
    steps.push(StepProgress {
        step: step.to_string(),
        status: StepStatus::Succeeded,
        message: Some(message.to_string()),
    });
}

fn failed_initialize_report(
    team_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
) -> InitializeReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    InitializeReport {
        team_name: team_name.to_string(),
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
    }
}

fn failed_add_agent_report(
    team_name: &str,
    member_name: &str,
    failed_step: &str,
    err: CoordinationError,
    succeeded_steps: Vec<String>,
    steps: &mut Vec<StepProgress>,
) -> AddAgentReport {
    steps.push(StepProgress {
        step: failed_step.to_string(),
        status: StepStatus::Failed,
        message: Some(err.to_string()),
    });
    AddAgentReport {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        succeeded_steps,
        failed_step: Some(failed_step.to_string()),
        retryable: true,
        message: err.to_string(),
        steps: std::mem::take(steps),
    }
}

fn parse_cli_tool(raw: &str) -> Result<CliTool, CoordinationError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "claude" | "claude_native" => Ok(CliTool::Claude),
        "codex" | "mesh" | "mesh_bridged" => Ok(CliTool::Codex),
        "gemini" => Ok(CliTool::Gemini),
        other => Err(CoordinationError::Validation(format!(
            "unsupported cli tool '{other}'"
        ))),
    }
}

fn member_from_agent_setup(
    setup: &AgentSetupConfig,
    role: MemberRole,
) -> Result<Member, CoordinationError> {
    validate_member_name(&setup.name)?;
    validate_non_empty("agent project id", &setup.project_id)?;
    Ok(Member {
        name: setup.name.clone(),
        role,
        instructions: setup.description.clone(),
        project_path: PathBuf::from(&setup.project_id),
        cli_tool: parse_cli_tool(&setup.cli_tool)?,
    })
}
