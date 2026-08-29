use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use taurhaus_lib::logging::emit_global;

use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::requests::AgentSetupConfig;
use crate::models::ModelCatalog;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::launch::ModelSpec;
use crate::templates::storage::{TemplateStore, TemplateStoreError};
use crate::templates::types::RoleTemplate;

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
    pub model: String,
    pub reasoning_effort: Option<String>,
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
    /// Conversation this activation must land in, rather than a fresh one.
    ///
    /// Set only where taurhaus relaunches a session it owns for a reason of
    /// its own — the task-effort pass, which is the one harness path that
    /// cannot change effort in the running prompt. An operator-driven resume
    /// leaves it unset and keeps starting fresh: members share a project, and a
    /// checkpoint-based resume would pick up another member's conversation.
    pub resume_session_id: Option<String>,
    /// Account directory this activation launches on, resolved once from the
    /// operator's launch settings.
    ///
    /// The launch command and anything that edits that account's own files —
    /// capturing the operator's effort default, putting it back — read the
    /// same value, so taurhaus never writes to a directory the member's
    /// process does not read.
    pub account_dir: Option<PathBuf>,
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
            resume_session_id: None,
            account_dir: None,
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
            resume_session_id: None,
            account_dir: None,
        })
    }

    pub fn for_resume_member(team_name: &str, lead_name: &str, member: &Member) -> Self {
        let mut member = member.clone();
        hydrate_member_model_fields(&mut member, None);
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
                model: member
                    .model
                    .or_else(|| {
                        ModelCatalog::default_for(member.cli_tool).map(|entry| entry.id.clone())
                    })
                    .unwrap_or_default(),
                reasoning_effort: member.reasoning_effort,
                project_path: member.project_path.clone(),
            },
            pane_policy: MemberActivationPanePolicy::ReuseOrCreate,
            delivery_policy: MemberActivationDeliveryPolicy::Immediate,
            roster_policy: MemberActivationRosterPolicy::ExistingMember,
            runtime_commit_policy: MemberActivationRuntimeCommitPolicy::FinalizeAtEnd,
            resume_session_id: None,
            account_dir: None,
        }
    }
}

fn member_identity_from_agent_setup(
    member: &AgentSetupConfig,
    role: MemberRole,
) -> Result<MemberIdentity, CoordinationError> {
    let cli_tool = CliTool::from_alias(&member.cli_tool)
        .map_err(|err| CoordinationError::Validation(err.to_string()))?;
    let declared = declared_model_fields(&member.model, member.reasoning_effort.clone());
    Ok(MemberIdentity {
        name: member.name.clone(),
        role,
        cli_tool,
        model: declared
            .model
            .or_else(|| ModelCatalog::default_for(cli_tool).map(|entry| entry.id.clone()))
            .unwrap_or_default(),
        reasoning_effort: declared.reasoning_effort,
        project_path: PathBuf::from(&member.project_id),
    })
}

pub(crate) fn hydrate_member_model_fields(member: &mut Member, role: Option<&RoleTemplate>) {
    let declared = declared_model_fields(
        member.model.as_deref().unwrap_or_default(),
        member.reasoning_effort.clone(),
    );
    let role_defaults = role.map(|role| {
        declared_model_fields(&role.defaults.model, role.defaults.reasoning_effort.clone())
    });
    let catalog_default = ModelCatalog::default_for(member.cli_tool);
    let role_model = role_defaults
        .as_ref()
        .and_then(|fields| fields.model.as_deref())
        .and_then(|model| {
            validated_role_model(member.cli_tool, model, &member.name, "resume_hydration")
        });

    member.model = declared
        .model
        .or(role_model)
        .or_else(|| catalog_default.map(|entry| entry.id.clone()));
    member.reasoning_effort = declared.reasoning_effort.or_else(|| {
        role_defaults
            .as_ref()
            .and_then(|fields| fields.reasoning_effort.clone())
    });
}

fn declared_model_fields(model: &str, reasoning_effort: Option<String>) -> ModelSpec {
    let mut parsed = ModelSpec::parse_legacy(model);
    if reasoning_effort.is_some() {
        parsed.reasoning_effort = reasoning_effort;
    }
    parsed
}

pub(crate) fn validated_role_model(
    tool: CliTool,
    model: &str,
    member_name: &str,
    operation: &str,
) -> Option<String> {
    if ModelCatalog::entry_for(tool, model).is_some() {
        return Some(model.to_string());
    }

    let belongs_to_another_tool = crate::session_scanner::cli_tool::all()
        .iter()
        .map(|entry| entry.tool)
        .any(|candidate| candidate != tool && ModelCatalog::entry_for(candidate, model).is_some());
    if !belongs_to_another_tool {
        return Some(model.to_string());
    }

    let replacement = ModelCatalog::default_for(tool).map(|entry| entry.id.clone());
    tracing::warn!(
        member = member_name,
        operation,
        tool = %tool,
        found = model,
        replacement = ?replacement,
        "role model is not valid for the member CLI; using the catalog default"
    );
    let mut fields = Map::new();
    fields.insert("member".to_string(), Value::String(member_name.to_string()));
    fields.insert(
        "operation".to_string(),
        Value::String(operation.to_string()),
    );
    fields.insert("tool".to_string(), Value::String(tool.to_string()));
    fields.insert("found".to_string(), Value::String(model.to_string()));
    fields.insert(
        "replacement".to_string(),
        replacement.map(Value::String).unwrap_or(Value::Null),
    );
    emit_global(
        "warn",
        "coordination",
        "launch.model.invalid",
        Some("Role model is not valid for the member CLI; using the catalog default".to_string()),
        fields,
    );
    None
}

pub(crate) fn load_role_for_member_hydration(
    template_root: &Path,
    role_id: &str,
    member_name: &str,
    operation: &str,
) -> Option<RoleTemplate> {
    match TemplateStore::new(template_root.to_path_buf()).get_role(role_id) {
        Ok(record) => Some(record.template),
        Err(TemplateStoreError::NotFound(_)) => None,
        Err(error) => {
            tracing::warn!(
                role_id,
                member = member_name,
                operation,
                error = %error,
                "coordination role hydration failed; continuing without role defaults"
            );
            let mut fields = Map::new();
            fields.insert("role_id".to_string(), Value::String(role_id.to_string()));
            fields.insert("member".to_string(), Value::String(member_name.to_string()));
            fields.insert(
                "operation".to_string(),
                Value::String(operation.to_string()),
            );
            fields.insert("error".to_string(), Value::String(error.to_string()));
            emit_global(
                "warn",
                "coordination",
                "coordination.role.load_failed",
                Some(
                    "Role defaults unavailable; continuing with declared or catalog values"
                        .to_string(),
                ),
                fields,
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::*;
    use crate::coordination::domain::MemberRole;
    use crate::coordination::requests::AgentDefinition;

    fn wait_for_log_contents(log_path: &Path, expected_event: &str) -> String {
        for _ in 0..20 {
            let contents = fs::read_to_string(log_path).unwrap_or_default();
            if contents.contains(expected_event) {
                return contents;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        fs::read_to_string(log_path).unwrap_or_default()
    }

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
            reasoning_effort: None,
            handoff_expectations: None,
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
        assert_eq!(context.member.model, "gpt-5.4");
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
        let mut member = sample_agent("backend-dev", "agy", "/tmp/api");
        member.model = "gemini-3.7-flash-high".to_string();

        let context =
            MemberActivationContext::for_add_agent("architecture-final", "team-lead", &member)
                .expect("add-agent context should build");

        assert_eq!(context.operation, MemberActivationOperationKind::AddAgent);
        assert_eq!(context.member.role, MemberRole::Agent);
        assert_eq!(context.member.cli_tool, CliTool::Agy);
        assert_eq!(context.member.model, "gemini-3.7-flash-high");
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
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            model: None,
            reasoning_effort: None,
            project_path: PathBuf::from("/tmp/review"),
            cli_tool: CliTool::Claude,
            extra: Default::default(),
        };

        let context =
            MemberActivationContext::for_resume_member("architecture-final", "team-lead", &member);

        assert_eq!(context.operation, MemberActivationOperationKind::Resume);
        assert_eq!(context.lead.name, "team-lead");
        assert_eq!(context.member.name, "reviewer");
        assert_eq!(context.member.role, MemberRole::Agent);
        assert_eq!(context.member.cli_tool, CliTool::Claude);
        assert_eq!(context.member.model, "opus");
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

    // Regression: a79d392 hydrated role models without checking the member CLI,
    // so a Claude member with a Codex role resumed as `claude --model gpt-5.4`.
    #[test]
    fn role_model_for_a_different_cli_falls_back_to_catalog_default() {
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let log_path = tmp.path().join("invalid-role-model.log.jsonl");
        let log_state =
            taurhaus_lib::logging::LogFileState::new(log_path.clone()).expect("log state");
        taurhaus_lib::logging::install_global_sink(&log_state);
        let role: RoleTemplate = serde_norway::from_str(include_str!(
            "../../resources/templates/roles/v3-developer-codex.yaml"
        ))
        .expect("bundled Codex role");
        let mut member = Member {
            name: "reviewer".to_string(),
            role: MemberRole::Agent,
            role_id: Some(role.role_id.clone()),
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
            project_path: PathBuf::from("/tmp/review"),
            cli_tool: CliTool::Claude,
            extra: Default::default(),
        };

        hydrate_member_model_fields(&mut member, Some(&role));

        assert_eq!(member.model.as_deref(), Some("opus"));
        let contents = wait_for_log_contents(&log_path, "launch.model.invalid");
        let event = contents
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|event| event["event"] == "launch.model.invalid" && event["member"] == "reviewer")
            .expect("invalid role model event");
        assert_eq!(event["found"], "gpt-5.4");
        assert_eq!(event["replacement"], "opus");
    }

    // Regression: dd8d1fe treated the static catalog as a closed allowlist,
    // replacing valid newer Codex model slugs with the catalog default.
    #[test]
    fn unknown_role_model_for_same_cli_is_preserved() {
        assert_eq!(
            validated_role_model(
                CliTool::Codex,
                "gpt-5.3-codex",
                "reviewer",
                "resume_hydration"
            ),
            Some("gpt-5.3-codex".to_string())
        );
    }

    // Regression: 0f973a6 routed an expected missing role through the warning
    // path, emitting coordination.role.load_failed on every affected activation.
    #[test]
    fn missing_role_does_not_emit_load_failed_warning() {
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let log_path = tmp.path().join("member-activation.log.jsonl");
        let log_state =
            taurhaus_lib::logging::LogFileState::new(log_path.clone()).expect("log state");
        taurhaus_lib::logging::install_global_sink(&log_state);

        let loaded =
            load_role_for_member_hydration(tmp.path(), "missing-role", "reviewer", "resume");

        assert!(loaded.is_none());
        emit_global(
            "info",
            "coordination",
            "coordination.role.test_completed",
            None,
            Map::new(),
        );
        let contents = wait_for_log_contents(&log_path, "coordination.role.test_completed");
        assert!(
            !contents.contains("coordination.role.load_failed"),
            "missing roles are expected compatibility state, not load failures"
        );
    }
}
