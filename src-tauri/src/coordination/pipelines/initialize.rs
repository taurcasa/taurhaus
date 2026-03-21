use super::*;

use chrono::Utc;

use crate::coordination::audit::{AuditEvent, MemberAddedEvent};
use crate::coordination::backend::BackendKind;
use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
use crate::coordination::domain::{HealthState, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{
    DeliveryRequest, InitializeReport, InitializeTeamRequest, LeadMode, OperatorNoticeDelivery,
};
use crate::coordination::stores::{MemberRuntimeStore, TeamConfig, TeamConfigStore};
use crate::coordination::validation::{validate_non_empty, validate_team_name};
use crate::models::CliCommandSettings;
use crate::session_scanner::cli_tool::CliTool;

impl CoordinationOrchestrator {
    pub fn initialize_team(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<InitializeReport, CoordinationError> {
        self.initialize_team_with_cli_commands_and_layout(
            request,
            &CliCommandSettings::default(),
            "new_window",
        )
    }

    pub fn initialize_team_with_cli_commands(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
    ) -> Result<InitializeReport, CoordinationError> {
        self.initialize_team_with_cli_commands_and_layout(request, cli_commands, "new_window")
    }

    pub fn initialize_team_with_cli_commands_and_layout(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
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
                self.cleanup_initialize_failure(&request.team_name);
                return Ok(failed_initialize_report(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                ));
            }
        };
        let agent_members = match request
            .agents
            .iter()
            .map(|agent| member_from_agent_setup(agent, MemberRole::Agent))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(members) => members,
            Err(err) => {
                self.cleanup_initialize_failure(&request.team_name);
                return Ok(failed_initialize_report(
                    &request.team_name,
                    "add_lead",
                    err,
                    succeeded_steps,
                    &mut steps,
                ));
            }
        };
        if let Err(err) = self.seed_initialize_roster(
            &request.team_name,
            request.team_description.clone(),
            lead_member,
            &agent_members,
        ) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "add_lead",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("add_lead", "lead added", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.create_panes(request, cli_commands, tmux_layout) {
            self.cleanup_initialize_failure(&request.team_name);
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
            "panes opened and sessions started",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.launch_sessions(request, cli_commands) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "launch_sessions",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        if let Err(err) = self.sync_team_config_metadata(&request.team_name) {
            self.cleanup_initialize_failure(&request.team_name);
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
            "launched sessions verified",
            &mut succeeded_steps,
            &mut steps,
        );

        if let Err(err) = self.join_mesh(request) {
            self.cleanup_initialize_failure(&request.team_name);
            return Ok(failed_initialize_report(
                &request.team_name,
                "join_mesh",
                err,
                succeeded_steps,
                &mut steps,
            ));
        }
        mark_step_succeeded("join_mesh", "mesh joined", &mut succeeded_steps, &mut steps);

        if let Err(err) = self.start_daemons(request) {
            self.cleanup_initialize_failure(&request.team_name);
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
            self.cleanup_initialize_failure(&request.team_name);
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

        self.ensure_team_daemon_running_best_effort(&request.team_name);

        Ok(InitializeReport {
            team_name: request.team_name.clone(),
            succeeded_steps,
            failed_step: None,
            retryable: false,
            message: "team initialized".to_string(),
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
        if request.lead_mode == LeadMode::AttachExisting && should_use_mesh_sidecar(&request.lead)?
        {
            return Err(CoordinationError::Validation(format!(
                "attach-existing is not supported yet for '{}' leads; use launch-new",
                request.lead.cli_tool
            )));
        }

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

    fn create_panes(
        &mut self,
        request: &InitializeTeamRequest,
        cli_commands: &CliCommandSettings,
        tmux_layout: &str,
    ) -> Result<(), CoordinationError> {
        let mut per_project_anchor_panes = std::collections::HashMap::<String, String>::new();

        if request.lead_mode == LeadMode::LaunchNew {
            let lead_member = member_from_agent_setup(&request.lead, MemberRole::Lead)?;
            let launch_cmd = build_cli_launch_command(
                &request.lead,
                &request.team_name,
                MemberRole::Lead,
                cli_commands,
            )?;
            let pane_id = launch_member_pane(
                self.runtime.as_ref(),
                &mut per_project_anchor_panes,
                tmux_layout,
                &lead_member.project_path.to_string_lossy(),
                &launch_cmd,
            )?;
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.lead.name)?;
            runtime.cli_tool = Some(lead_member.cli_tool);
            runtime.project_path = Some(lead_member.project_path);
            runtime.pane_id = Some(pane_id);
            runtime.session_id = None;
            runtime.daemon_pid = None;
            runtime.attached_at = Some(Utc::now());
            runtime.health = HealthState::Healthy;
            MemberRuntimeStore::save(
                &self.teams_dir,
                &request.team_name,
                &request.lead.name,
                &runtime,
            )?;
        }

        for agent in &request.agents {
            let member = member_from_agent_setup(agent, MemberRole::Agent)?;
            let launch_cmd = build_cli_launch_command(
                agent,
                &request.team_name,
                MemberRole::Agent,
                cli_commands,
            )?;
            let pane_id = launch_member_pane(
                self.runtime.as_ref(),
                &mut per_project_anchor_panes,
                tmux_layout,
                &member.project_path.to_string_lossy(),
                &launch_cmd,
            )?;

            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &member.name)?;
            runtime.cli_tool = Some(member.cli_tool);
            runtime.project_path = Some(member.project_path.clone());
            runtime.pane_id = Some(pane_id);
            runtime.session_id = None;
            runtime.daemon_pid = None;
            runtime.attached_at = Some(Utc::now());
            runtime.health = HealthState::Healthy;
            MemberRuntimeStore::save(&self.teams_dir, &request.team_name, &member.name, &runtime)?;
        }
        Ok(())
    }

    fn seed_initialize_roster(
        &mut self,
        team_name: &str,
        team_description: Option<String>,
        lead_member: crate::coordination::domain::Member,
        agent_members: &[crate::coordination::domain::Member],
    ) -> Result<(), CoordinationError> {
        let created_at = Utc::now();
        let mut members = Vec::with_capacity(1 + agent_members.len());
        members.push(lead_member);
        members.extend(agent_members.iter().cloned());

        TeamConfigStore::save(
            &self.teams_dir,
            team_name,
            &TeamConfig {
                schema_version: 1,
                name: team_name.to_string(),
                description: team_description,
                created_at,
                members: members.clone(),
            },
        )?;

        for member in members {
            MemberRuntimeStore::save(
                &self.teams_dir,
                team_name,
                &member.name,
                &crate::coordination::stores::MemberRuntimeRecord {
                    schema_version: 3,
                    member_name: member.name.clone(),
                    cli_tool: Some(member.cli_tool),
                    project_path: Some(member.project_path.clone()),
                    pane_id: None,
                    session_id: None,
                    jsonl_path: None,
                    daemon_pid: None,
                    health: HealthState::SessionDead,
                    delivery_lease: None,
                    attached_at: None,
                    last_seen_at: None,
                },
            )?;

            self.audit_log
                .push(AuditEvent::MemberAdded(MemberAddedEvent {
                    team_name: team_name.to_string(),
                    member_name: member.name,
                    role: member.role,
                    backend: backend_kind_for_member_tool(member.cli_tool),
                    added_at: created_at,
                }));
        }

        Ok(())
    }

    fn launch_sessions(
        &self,
        request: &InitializeTeamRequest,
        _cli_commands: &CliCommandSettings,
    ) -> Result<(), CoordinationError> {
        if request.lead_mode == LeadMode::LaunchNew {
            let runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.lead.name)?;
            let pane_id = runtime.pane_id.ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    request.lead.name, request.team_name
                ))
            })?;
            self.capture_session_id_for_member(
                &request.team_name,
                &request.lead.name,
                &pane_id,
                &request.lead,
            )?;
        }

        for agent in &request.agents {
            let runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &agent.name)?;
            let pane_id = runtime.pane_id.ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    agent.name, request.team_name
                ))
            })?;
            self.capture_session_id_for_member(&request.team_name, &agent.name, &pane_id, agent)?;
        }
        Ok(())
    }

    fn join_mesh(&self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if should_use_mesh_sidecar(&request.lead)? {
            self.runtime.join_mesh(
                &request.team_name,
                &request.lead.name,
                &request.lead.project_id,
            )?;
        }
        for agent in &request.agents {
            if should_use_mesh_sidecar(agent)? {
                self.runtime
                    .join_mesh(&request.team_name, &agent.name, &agent.project_id)?;
            }
        }
        Ok(())
    }

    fn start_daemons(&self, request: &InitializeTeamRequest) -> Result<(), CoordinationError> {
        if request.lead_mode == LeadMode::LaunchNew && should_use_mesh_sidecar(&request.lead)? {
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &request.lead.name)?;
            let pane_id = runtime.pane_id.clone().ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    request.lead.name, request.team_name
                ))
            })?;
            let pid =
                self.runtime
                    .spawn_mesh_daemon(&pane_id, &request.team_name, &request.lead.name)?;
            runtime.daemon_pid = Some(pid);
            MemberRuntimeStore::save(
                &self.teams_dir,
                &request.team_name,
                &request.lead.name,
                &runtime,
            )?;
            tracing::info!(
                team = %request.team_name,
                member = %request.lead.name,
                pane_id = %pane_id,
                pid = pid,
                "mesh daemon started"
            );
        }

        for agent in &request.agents {
            if !should_use_mesh_sidecar(agent)? {
                continue;
            }
            let mut runtime =
                MemberRuntimeStore::load(&self.teams_dir, &request.team_name, &agent.name)?;
            let pane_id = runtime.pane_id.clone().ok_or_else(|| {
                CoordinationError::Backend(format!(
                    "missing pane id for member '{}' in team '{}'",
                    agent.name, request.team_name
                ))
            })?;
            let pid = self
                .runtime
                .spawn_mesh_daemon(&pane_id, &request.team_name, &agent.name)?;
            runtime.daemon_pid = Some(pid);
            MemberRuntimeStore::save(&self.teams_dir, &request.team_name, &agent.name, &runtime)?;
            tracing::info!(
                team = %request.team_name,
                member = %agent.name,
                pane_id = %pane_id,
                pid = pid,
                "mesh daemon started"
            );
        }
        Ok(())
    }

    fn send_onboarding_messages(
        &mut self,
        request: &InitializeTeamRequest,
    ) -> Result<(), CoordinationError> {
        // Deliver to lead + all agents — the lead is a CLI agent too.
        let all_members = std::iter::once(&request.lead).chain(request.agents.iter());
        for member in all_members {
            let cli_tool = parse_cli_tool(&member.cli_tool)?;
            let onboarding = if cli_tool == CliTool::Claude {
                if !agent_has_role_context(member) {
                    continue;
                }
                DeliveryRenderer::render_claude_role_context(
                    &request.team_name,
                    &member.name,
                    &request.lead.name,
                    RoleContext {
                        role_id: member.role_id.as_deref(),
                        communication_style: member.communication_style.as_deref(),
                        instructions: agent_instructions(member),
                        behavioral_contract: member.behavioral_contract.as_ref(),
                        quality_gates: member.quality_gates.as_deref(),
                        definition_of_done: member.definition_of_done.as_deref(),
                        capabilities: member.capabilities.as_deref(),
                    },
                )
            } else {
                DeliveryRenderer::render_onboarding(
                    &request.team_name,
                    &member.name,
                    &request.lead.name,
                    RoleContext {
                        role_id: member.role_id.as_deref(),
                        communication_style: member.communication_style.as_deref(),
                        instructions: agent_instructions(member),
                        behavioral_contract: member.behavioral_contract.as_ref(),
                        quality_gates: member.quality_gates.as_deref(),
                        definition_of_done: member.definition_of_done.as_deref(),
                        capabilities: member.capabilities.as_deref(),
                    },
                )
            };
            self.deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                member_name: member.name.clone(),
                team_name: request.team_name.clone(),
                message: onboarding,
                sender_name: Some(request.lead.name.clone()),
                operational_context: None,
            }))?;
        }
        Ok(())
    }
}

fn launch_member_pane(
    runtime: &dyn crate::coordination::runtime::CoordinationRuntime,
    per_project_anchor_panes: &mut std::collections::HashMap<String, String>,
    tmux_layout: &str,
    project_path: &str,
    launch_cmd: &str,
) -> Result<String, CoordinationError> {
    if tmux_layout == "per_project" {
        if let Some(anchor_pane) = per_project_anchor_panes.get(project_path) {
            return runtime.create_aitx_pane_and_launch_in_target(
                project_path,
                anchor_pane,
                launch_cmd,
            );
        }
    }

    let pane_id = runtime.create_aitx_pane_and_launch(project_path, tmux_layout, launch_cmd)?;
    if tmux_layout == "per_project" {
        per_project_anchor_panes
            .entry(project_path.to_string())
            .or_insert_with(|| pane_id.clone());
    }
    Ok(pane_id)
}

fn backend_kind_for_member_tool(tool: CliTool) -> BackendKind {
    match tool {
        CliTool::Claude => BackendKind::ClaudeNative,
        CliTool::Codex | CliTool::Gemini => BackendKind::MeshBridged,
    }
}
