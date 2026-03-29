use std::path::PathBuf;

use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::AgentSetupConfig;
use crate::session_scanner::cli_tool::CliTool;

/// Wrapper-level operation kind for shared member activation planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberActivationOperationKind {
    Initialize,
    Resume,
    AddAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberActivationPanePolicy {
    CreateNew,
    ReuseOrCreate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberActivationDeliveryPolicy {
    DeferredBarrier,
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberActivationRosterPolicy {
    Preseeded,
    CreateMember,
    ExistingMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberActivationRuntimeCommitPolicy {
    Staged,
    FinalizeAtEnd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadIdentity {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberIdentity {
    pub name: String,
    pub role: MemberRole,
    pub cli_tool: CliTool,
    pub project_path: PathBuf,
}

/// Canonical wrapper-neutral input for shared member activation helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberActivationContext {
    pub operation: MemberActivationOperationKind,
    pub team_name: String,
    pub lead: LeadIdentity,
    pub member: MemberIdentity,
    pub pane_policy: MemberActivationPanePolicy,
    pub delivery_policy: MemberActivationDeliveryPolicy,
    pub roster_policy: MemberActivationRosterPolicy,
    pub runtime_commit_policy: MemberActivationRuntimeCommitPolicy,
}

impl MemberActivationContext {
    pub fn for_initialize_member(
        team_name: &str,
        lead_name: &str,
        member: &AgentSetupConfig,
        role: MemberRole,
    ) -> Result<Self, CoordinationError> {
        Ok(Self {
            operation: MemberActivationOperationKind::Initialize,
            team_name: team_name.to_string(),
            lead: LeadIdentity {
                name: lead_name.to_string(),
            },
            member: member_identity_from_agent_setup(member, role)?,
            pane_policy: MemberActivationPanePolicy::CreateNew,
            delivery_policy: MemberActivationDeliveryPolicy::DeferredBarrier,
            roster_policy: MemberActivationRosterPolicy::Preseeded,
            runtime_commit_policy: MemberActivationRuntimeCommitPolicy::Staged,
        })
    }

    pub fn for_add_agent(
        team_name: &str,
        lead_name: &str,
        member: &AgentSetupConfig,
    ) -> Result<Self, CoordinationError> {
        Ok(Self {
            operation: MemberActivationOperationKind::AddAgent,
            team_name: team_name.to_string(),
            lead: LeadIdentity {
                name: lead_name.to_string(),
            },
            member: member_identity_from_agent_setup(member, MemberRole::Agent)?,
            pane_policy: MemberActivationPanePolicy::CreateNew,
            delivery_policy: MemberActivationDeliveryPolicy::Immediate,
            roster_policy: MemberActivationRosterPolicy::CreateMember,
            runtime_commit_policy: MemberActivationRuntimeCommitPolicy::FinalizeAtEnd,
        })
    }

    pub fn for_resume_member(team_name: &str, lead_name: &str, member: &Member) -> Self {
        Self {
            operation: MemberActivationOperationKind::Resume,
            team_name: team_name.to_string(),
            lead: LeadIdentity {
                name: lead_name.to_string(),
            },
            member: MemberIdentity {
                name: member.name.clone(),
                role: member.role,
                cli_tool: member.cli_tool,
                project_path: member.project_path.clone(),
            },
            pane_policy: MemberActivationPanePolicy::ReuseOrCreate,
            delivery_policy: MemberActivationDeliveryPolicy::Immediate,
            roster_policy: MemberActivationRosterPolicy::ExistingMember,
            runtime_commit_policy: MemberActivationRuntimeCommitPolicy::FinalizeAtEnd,
        }
    }
}

fn member_identity_from_agent_setup(
    member: &AgentSetupConfig,
    role: MemberRole,
) -> Result<MemberIdentity, CoordinationError> {
    let cli_tool = CliTool::from_alias(&member.cli_tool)
        .map_err(|err| CoordinationError::Validation(err.to_string()))?;
    Ok(MemberIdentity {
        name: member.name.clone(),
        role,
        cli_tool,
        project_path: PathBuf::from(&member.project_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::domain::MemberRole;
    use crate::coordination::requests::AgentDefinition;

    fn sample_agent(name: &str, cli_tool: &str, project_id: &str) -> AgentDefinition {
        AgentDefinition {
            name: name.to_string(),
            cli_tool: cli_tool.to_string(),
            model: "gpt-5.4".to_string(),
            project_id: project_id.to_string(),
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
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
        }
    }

    #[test]
    fn initialize_member_context_uses_preseeded_deferred_barrier_and_staged_runtime() {
        let member = sample_agent("frontend-dev", "codex", "/tmp/taurhaus");

        let context = MemberActivationContext::for_initialize_member(
            "architecture-final",
            "team-lead",
            &member,
            MemberRole::Agent,
        )
        .expect("initialize context should build");

        assert_eq!(context.operation, MemberActivationOperationKind::Initialize);
        assert_eq!(context.team_name, "architecture-final");
        assert_eq!(context.lead.name, "team-lead");
        assert_eq!(context.member.name, "frontend-dev");
        assert_eq!(context.member.role, MemberRole::Agent);
        assert_eq!(context.member.cli_tool, CliTool::Codex);
        assert_eq!(context.member.project_path, PathBuf::from("/tmp/taurhaus"));
        assert_eq!(context.pane_policy, MemberActivationPanePolicy::CreateNew);
        assert_eq!(
            context.delivery_policy,
            MemberActivationDeliveryPolicy::DeferredBarrier
        );
        assert_eq!(
            context.roster_policy,
            MemberActivationRosterPolicy::Preseeded
        );
        assert_eq!(
            context.runtime_commit_policy,
            MemberActivationRuntimeCommitPolicy::Staged
        );
    }

    #[test]
    fn add_agent_context_uses_create_member_and_finalize_at_end() {
        let member = sample_agent("backend-dev", "gemini", "/tmp/api");

        let context =
            MemberActivationContext::for_add_agent("architecture-final", "team-lead", &member)
                .expect("add-agent context should build");

        assert_eq!(context.operation, MemberActivationOperationKind::AddAgent);
        assert_eq!(context.member.role, MemberRole::Agent);
        assert_eq!(context.member.cli_tool, CliTool::Gemini);
        assert_eq!(context.pane_policy, MemberActivationPanePolicy::CreateNew);
        assert_eq!(
            context.delivery_policy,
            MemberActivationDeliveryPolicy::Immediate
        );
        assert_eq!(
            context.roster_policy,
            MemberActivationRosterPolicy::CreateMember
        );
        assert_eq!(
            context.runtime_commit_policy,
            MemberActivationRuntimeCommitPolicy::FinalizeAtEnd
        );
    }

    #[test]
    fn resume_member_context_uses_existing_member_and_reuse_or_create() {
        let member = Member {
            name: "reviewer".to_string(),
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
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            project_path: PathBuf::from("/tmp/review"),
            cli_tool: CliTool::Claude,
        };

        let context =
            MemberActivationContext::for_resume_member("architecture-final", "team-lead", &member);

        assert_eq!(context.operation, MemberActivationOperationKind::Resume);
        assert_eq!(context.lead.name, "team-lead");
        assert_eq!(context.member.name, "reviewer");
        assert_eq!(context.member.role, MemberRole::Agent);
        assert_eq!(context.member.cli_tool, CliTool::Claude);
        assert_eq!(context.member.project_path, PathBuf::from("/tmp/review"));
        assert_eq!(
            context.pane_policy,
            MemberActivationPanePolicy::ReuseOrCreate
        );
        assert_eq!(
            context.delivery_policy,
            MemberActivationDeliveryPolicy::Immediate
        );
        assert_eq!(
            context.roster_policy,
            MemberActivationRosterPolicy::ExistingMember
        );
        assert_eq!(
            context.runtime_commit_policy,
            MemberActivationRuntimeCommitPolicy::FinalizeAtEnd
        );
    }
}
