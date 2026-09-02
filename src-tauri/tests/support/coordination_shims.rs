pub mod errors {
    pub use taurhaus_lib::errors::*;
}

pub mod models {
    pub use taurhaus_lib::models::*;
}

pub mod templates {
    pub mod composition {
        pub use taurhaus_lib::templates::composition::{
            compose_team, CompositionOverrides, ResolvedMember,
        };
    }

    pub mod storage {
        pub use taurhaus_lib::templates::storage::{TemplateStore, TemplateStoreError};
    }

    pub mod types {
        pub use taurhaus_lib::templates::types::{
            BehavioralContract, RoleKind, RoleTemplate, RuntimeCompactSummary,
        };
    }
}

pub mod provider {
    pub mod daemon_client {
        pub use taurhaus_lib::provider::daemon_client::*;
    }

    pub mod path {
        pub use taurhaus_lib::provider::path::*;
    }

    pub mod platform_paths {
        pub use taurhaus_lib::provider::platform_paths::*;
    }
}

pub mod tmux_layout {
    pub use taurhaus_lib::tmux_layout::*;
}

pub mod session_scanner {
    pub use taurhaus_lib::session_scanner::{
        scan_sessions_for_display, scan_sessions_for_runtime, ActivityAttribution,
        ActivityConfidence, DisplaySession, RuntimeSession, SessionGroupKind, SessionState,
    };

    pub mod cli_tool {
        pub use taurhaus_lib::session_scanner::cli_tool::*;
    }

    pub mod accounts {
        pub use taurhaus_lib::session_scanner::accounts::{
            configured_default_dir, to_launch_namespace,
        };
    }

    pub mod process {
        pub use taurhaus_lib::session_scanner::process::detect_cli_tool;
    }

    pub mod transcript_boundary {
        pub use taurhaus_lib::session_scanner::transcript_boundary::*;
    }

    pub mod launch {
        use crate::coordination::domain::MemberRole;
        use crate::daemon::protocol::LaunchMode;
        use crate::session_scanner::cli_tool::CliTool;

        pub use taurhaus_lib::session_scanner::launch::{
            base_command, command_contains_flag, redact_command_for_logging, shell_escape,
            LaunchNote, ModelSpec, RenderedLaunch,
        };

        pub struct TeamContext<'a> {
            pub team_name: &'a str,
            pub agent_name: &'a str,
            pub role: MemberRole,
        }

        pub struct LaunchSpec<'a> {
            pub tool: CliTool,
            pub mode: LaunchMode,
            pub base: &'a str,
            pub model: ModelSpec,
            pub team: Option<TeamContext<'a>>,
            pub codex_bypass_hook_trust: bool,
            pub codex_notify_executable: Option<&'a std::path::Path>,
            pub account_dir: Option<&'a std::path::Path>,
            pub selector: Option<&'static str>,
        }

        impl LaunchSpec<'_> {
            pub fn render(&self) -> RenderedLaunch {
                taurhaus_lib::session_scanner::launch::LaunchSpec {
                    tool: self.tool,
                    mode: self.mode,
                    base: self.base,
                    model: self.model.clone(),
                    codex_bypass_hook_trust: self.codex_bypass_hook_trust,
                    codex_notify_executable: self.codex_notify_executable,
                    account_dir: self.account_dir,
                    selector: self.selector,
                    team: self.team.as_ref().map(|team| {
                        taurhaus_lib::session_scanner::launch::TeamContext {
                            team_name: team.team_name,
                            agent_name: team.agent_name,
                            role: match team.role {
                                MemberRole::Lead => {
                                    taurhaus_lib::coordination::domain::MemberRole::Lead
                                }
                                MemberRole::Agent => {
                                    taurhaus_lib::coordination::domain::MemberRole::Agent
                                }
                            },
                        }
                    }),
                }
                .render()
            }
        }
    }

    pub mod control {
        pub use taurhaus_lib::session_scanner::control::{
            launch_command_in_tmux_with_layout, split_command_in_tmux_target_pane,
            TMUX_SESSION_NAME,
        };

        // Mirrors `session_scanner::control::validate_command_override`:
        // commands are free-form; only empty/multi-line input is rejected.
        pub(crate) fn validate_command_override(cmd: &str) -> Result<(), String> {
            if cmd.trim().is_empty() {
                return Err("Command override is empty".to_string());
            }
            if let Some(c) = cmd.chars().find(|c| matches!(c, '\n' | '\r' | '\0')) {
                return Err(format!(
                    "Command override must be a single line without control characters, found: {c:?}"
                ));
            }
            Ok(())
        }
    }
}

pub mod daemon {
    pub mod state_writes {
        pub(crate) fn reconcile_live_presence(
            state: &crate::coordination::state::CoordinationState,
            params: crate::daemon::protocol::CoordinationReconcileLivePresenceParams,
        ) -> Result<
            crate::daemon::protocol::CoordinationReconcileLivePresenceResult,
            crate::coordination::errors::CoordinationError,
        > {
            let reconciled = state.with_orchestrator(|orchestrator| {
                orchestrator.reconcile_team_presence_for_live_status_with_runtime_sessions(
                    &params.team_name,
                    &params.runtime_sessions,
                )
            })?;
            let mut reconciled_offline_members = reconciled.into_iter().collect::<Vec<_>>();
            reconciled_offline_members.sort();
            Ok(
                crate::daemon::protocol::CoordinationReconcileLivePresenceResult {
                    reconciled_offline_members,
                },
            )
        }

        pub(crate) fn set_active_project_team(
            teams_dir: &std::path::Path,
            params: crate::daemon::protocol::CoordinationSetActiveProjectTeamParams,
        ) -> Result<
            crate::daemon::protocol::CoordinationSetActiveProjectTeamResult,
            crate::coordination::errors::CoordinationError,
        > {
            match params.team_name {
                Some(team_name) => {
                    crate::coordination::stores::ActiveProjectTeamStore::set_active_team(
                        teams_dir,
                        &params.project_path,
                        &team_name,
                    )?
                }
                None => crate::coordination::stores::ActiveProjectTeamStore::clear_project(
                    teams_dir,
                    &params.project_path,
                )?,
            }
            Ok(crate::daemon::protocol::CoordinationSetActiveProjectTeamResult { updated: true })
        }
    }

    pub mod initialize_runs {
        use crate::coordination::requests::{InitializeReport, StepProgress};
        use crate::coordination::state::CoordinationState;
        use crate::models::CliCommandSettings;

        pub(crate) fn execute_initialize_pipeline(
            state: &CoordinationState,
            request: &crate::coordination::requests::InitializeTeamRequest,
            cli_commands: &CliCommandSettings,
            tmux_layout: &str,
            mut emit: Option<&mut dyn FnMut(StepProgress)>,
        ) -> Result<InitializeReport, crate::coordination::errors::CoordinationError> {
            state.with_orchestrator(|orchestrator| {
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
    }

    pub mod member_runs {
        use crate::coordination::requests::{
            AddAgentReport, AddAgentRequest, ResumeAgentReport, ResumeMemberRequest,
        };
        use crate::coordination::state::CoordinationState;
        use crate::models::CliCommandSettings;

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

        pub(crate) fn execute_resume_team_pipeline(
            state: &CoordinationState,
            request: &crate::coordination::requests::ResumeTeamRequest,
            cli_commands: &CliCommandSettings,
            tmux_layout: &str,
            mut emit: Option<&mut dyn FnMut(crate::coordination::requests::ResumeTeamProgress)>,
        ) -> Result<
            crate::coordination::requests::ResumeTeamReport,
            crate::coordination::errors::CoordinationError,
        > {
            state.with_orchestrator(|orchestrator| {
                orchestrator.resume_team_with_cli_commands_and_layout_and_progress(
                    request,
                    cli_commands,
                    tmux_layout,
                    Some(
                        &mut |member_name, member_index, member_count, stage, status, message| {
                            if let Some(emit) = emit.as_deref_mut() {
                                emit(crate::coordination::requests::ResumeTeamProgress {
                                    member_name: member_name.to_string(),
                                    member_index,
                                    member_count,
                                    stage,
                                    status,
                                    message,
                                });
                            }
                        },
                    ),
                )
            })
        }
    }

    pub mod team_runs {
        // Deliberate copy of daemon::team_runs::execute_reonboard_pipeline:
        // this crate recompiles the sources, so its CoordinationState is a
        // nominally different type from taurhaus_lib's and the shipped fn
        // cannot be forwarded to across the crate boundary. The lib-crate
        // run of the same tests covers the shipped rule; keep the two
        // bodies in sync when the rendering rule changes.
        use crate::coordination::delivery::{DeliveryRenderer, RoleContext};
        use crate::coordination::domain::MemberRole;
        use crate::coordination::requests::{
            DeliveryRequest, DeliveryResult, OperatorNoticeDelivery, ReonboardRequest,
        };
        use crate::coordination::state::CoordinationState;

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
                let message = if tool_spec.capabilities.native_inbox_poller {
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
                orchestrator.deliver_message(DeliveryRequest::operator_notice(
                    OperatorNoticeDelivery {
                        member_name: request.member_name.clone(),
                        team_name: request.team_name.clone(),
                        message,
                        sender_name: Some(lead_name),
                        operational_context: None,
                    },
                ))
            })
        }
    }

    pub mod protocol {
        use serde::{Deserialize, Serialize};

        pub use taurhaus_lib::daemon_api::protocol::{DaemonRequest, DaemonResponse, LaunchMode};

        pub mod method {
            pub use taurhaus_lib::daemon_api::protocol::method::{
                COORDINATION_APPLY_TASK_EFFORT, COORDINATION_APPLY_TASK_EFFORT_STATUS,
                COORDINATION_RECONCILE_LIVE_PRESENCE, COORDINATION_SET_ACTIVE_PROJECT_TEAM,
                GET_RUNTIME_SESSION_SNAPSHOT,
            };

            pub const COORDINATION_INITIALIZE_TEAM: &str = "coordination.initialize_team";
            pub const COORDINATION_INITIALIZE_STATUS: &str = "coordination.initialize_status";
            pub const COORDINATION_ADD_AGENT: &str = "coordination.add_agent";
            pub const COORDINATION_ADD_AGENT_STATUS: &str = "coordination.add_agent_status";
            pub const COORDINATION_RESUME_MEMBER: &str = "coordination.resume_member";
            pub const COORDINATION_RESUME_MEMBER_STATUS: &str = "coordination.resume_member_status";
            pub const COORDINATION_RESUME_TEAM: &str = "coordination.resume_team";
            pub const COORDINATION_RESUME_TEAM_STATUS: &str = "coordination.resume_team_status";
            pub const COORDINATION_REONBOARD: &str = "coordination.reonboard";
            pub const COORDINATION_REONBOARD_STATUS: &str = "coordination.reonboard_status";
            pub const COORDINATION_CREATE_TEAM: &str = "coordination.create_team";
            pub const COORDINATION_CREATE_TEAM_STATUS: &str = "coordination.create_team_status";
            pub const COORDINATION_DISBAND_TEAM: &str = "coordination.disband_team";
            pub const COORDINATION_DISBAND_TEAM_STATUS: &str = "coordination.disband_team_status";
            pub const COORDINATION_ADD_MEMBER: &str = "coordination.add_member";
            pub const COORDINATION_ADD_MEMBER_STATUS: &str = "coordination.add_member_status";
            pub const COORDINATION_REMOVE_MEMBER: &str = "coordination.remove_member";
            pub const COORDINATION_REMOVE_MEMBER_STATUS: &str = "coordination.remove_member_status";
        }

        pub use taurhaus_lib::daemon_api::protocol::{
            CoordinationApplyTaskEffortAccepted, CoordinationApplyTaskEffortOutcome,
            CoordinationApplyTaskEffortParams, CoordinationApplyTaskEffortReport,
            CoordinationApplyTaskEffortStatus, CoordinationApplyTaskEffortStatusParams,
            CoordinationReconcileLivePresenceParams, CoordinationReconcileLivePresenceResult,
            CoordinationSetActiveProjectTeamParams, CoordinationSetActiveProjectTeamResult,
        };

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeParams {
            pub request: crate::coordination::requests::InitializeTeamRequest,
            pub cli_commands: crate::models::CliCommandSettings,
            pub tmux_layout: String,
            #[serde(default)]
            pub operational_snapshots: Vec<crate::coordination::stores::OperationalContextSnapshot>,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeStatusParams {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationInitializeOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::InitializeReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationInitializeStatus {
            pub run_id: String,
            pub steps: Vec<crate::coordination::requests::StepProgress>,
            pub outcome: CoordinationInitializeOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationAddAgentParams {
            pub request: crate::coordination::requests::AddAgentRequest,
            pub cli_commands: crate::models::CliCommandSettings,
            pub tmux_layout: String,
            #[serde(default)]
            pub operational_snapshot:
                Option<crate::coordination::stores::OperationalContextSnapshot>,
            #[serde(default)]
            pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationAddAgentAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationAddAgentOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::AddAgentReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationAddAgentStatus {
            pub run_id: String,
            pub steps: Vec<crate::coordination::requests::StepProgress>,
            pub outcome: CoordinationAddAgentOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationResumeMemberParams {
            pub request: crate::coordination::requests::ResumeMemberRequest,
            pub cli_commands: crate::models::CliCommandSettings,
            pub tmux_layout: String,
            #[serde(default)]
            pub operational_snapshot:
                Option<crate::coordination::stores::OperationalContextSnapshot>,
            #[serde(default)]
            pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationResumeMemberAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationResumeMemberOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::ResumeAgentReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationResumeMemberStatus {
            pub run_id: String,
            pub steps: Vec<crate::coordination::requests::StepProgress>,
            pub outcome: CoordinationResumeMemberOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationResumeTeamParams {
            pub request: crate::coordination::requests::ResumeTeamRequest,
            pub cli_commands: crate::models::CliCommandSettings,
            pub tmux_layout: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationResumeTeamAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationResumeTeamOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::ResumeTeamReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationResumeTeamStatus {
            pub run_id: String,
            pub steps: Vec<crate::coordination::requests::ResumeTeamProgress>,
            pub outcome: CoordinationResumeTeamOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationReonboardParams {
            pub request: crate::coordination::requests::ReonboardRequest,
            pub cli_commands: crate::models::CliCommandSettings,
            pub tmux_layout: String,
            #[serde(default)]
            pub operational_snapshot:
                Option<crate::coordination::stores::OperationalContextSnapshot>,
            #[serde(default)]
            pub task_state_changed_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationReonboardAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationReonboardOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::DeliveryResult,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationReonboardStatus {
            pub run_id: String,
            pub outcome: CoordinationReonboardOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationCreateTeamParams {
            pub request: crate::coordination::requests::CreateTeamRequest,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationCreateTeamAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationCreateTeamOutcome {
            Running,
            Completed,
            Failed { error: String },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationCreateTeamStatus {
            pub run_id: String,
            pub outcome: CoordinationCreateTeamOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationDisbandTeamParams {
            pub request: crate::coordination::requests::DisbandTeamRequest,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationDisbandTeamAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationDisbandTeamOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::DisbandTeamReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationDisbandTeamStatus {
            pub run_id: String,
            pub outcome: CoordinationDisbandTeamOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationAddMemberParams {
            pub request: crate::coordination::requests::AddMemberRequest,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationAddMemberAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationAddMemberOutcome {
            Running,
            Completed,
            Failed { error: String },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationAddMemberStatus {
            pub run_id: String,
            pub outcome: CoordinationAddMemberOutcome,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationRemoveMemberParams {
            pub request: crate::coordination::requests::RemoveMemberRequest,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationRemoveMemberAccepted {
            pub run_id: String,
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case", tag = "status")]
        pub enum CoordinationRemoveMemberOutcome {
            Running,
            Completed {
                report: crate::coordination::requests::StopMemberReport,
            },
            Failed {
                error: String,
            },
        }

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
        pub struct CoordinationRemoveMemberStatus {
            pub run_id: String,
            pub outcome: CoordinationRemoveMemberOutcome,
        }
    }
}

pub mod workflow_runs {
    pub use taurhaus_lib::workflow_runs::{activity_for_transcript, WorkflowActivity};
}
