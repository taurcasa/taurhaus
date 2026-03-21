//! Delivery payload renderer for tmux-injectable content.

use crate::coordination::requests::{
    BootstrapDelivery, DeliveryRequest, OperatorNoticeDelivery, RecoveryNudgeDelivery,
};
use crate::templates::types::BehavioralContract;

/// Renders typed delivery payloads into deterministic tmux text.
#[derive(Debug, Default)]
pub struct DeliveryRenderer;

impl DeliveryRenderer {
    /// Render a deterministic onboarding template for non-Claude agents.
    pub fn render_onboarding(
        team_name: &str,
        member_name: &str,
        lead_name: &str,
        role_id: Option<&str>,
        communication_style: Option<&str>,
        instructions: Option<&str>,
        behavioral_contract: Option<&BehavioralContract>,
        quality_gates: Option<&[String]>,
        definition_of_done: Option<&[String]>,
        capabilities: Option<&[String]>,
    ) -> String {
        let mut rendered = format!(
            concat!(
                "[taurhaus] onboarding\n",
                "Identity:\n",
                "You are \"{member_name}\" on team \"{team_name}\". Your team lead is \"{lead_name}\".\n",
                "\n",
                "Read loop:\n",
                "mesh read --unread --mark-read --team {team_name} --name {member_name}\n",
                "\n",
                "Reply:\n",
                "mesh send {{recipient}} \"{{msg}}\" --team {team_name} --name {member_name} --summary \"brief\"\n",
                "\n",
                "Tasks:\n",
                "mesh task list/get/update --team {team_name} --name {member_name}\n",
                "mesh task list --team {team_name} --name {member_name}\n",
                "mesh task get <id> --team {team_name} --name {member_name}\n",
                "mesh task update <id> --status completed --team {team_name} --name {member_name}\n",
                "\n",
                "Work contract:\n",
                "Acknowledge assignment, execute, then report completion with artifacts and test results.\n",
                "\n",
                "Compaction safety:\n",
                "If context compaction happens and you have no unread messages or your current task is unclear, immediately message {lead_name} and ask for your current assignment.\n",
                "Do not assume you are done — compaction may have dropped your active task context.\n",
                "\n",
                "Escalation:\n",
                "If blocked, send blocker details to {lead_name} immediately. Do not stall silently."
            ),
            team_name = team_name,
            member_name = member_name,
            lead_name = lead_name
        );
        Self::append_role_context_sections(
            &mut rendered,
            role_id,
            communication_style,
            instructions,
            behavioral_contract,
            quality_gates,
            definition_of_done,
            capabilities,
        );
        rendered
    }

    /// Render role context for Claude agents as an initial team message.
    pub fn render_claude_role_context(
        team_name: &str,
        member_name: &str,
        lead_name: &str,
        role_id: Option<&str>,
        communication_style: Option<&str>,
        instructions: Option<&str>,
        behavioral_contract: Option<&BehavioralContract>,
        quality_gates: Option<&[String]>,
        definition_of_done: Option<&[String]>,
        capabilities: Option<&[String]>,
    ) -> String {
        let mut rendered = format!(
            concat!(
                "[taurhaus] role_context\n",
                "Identity:\n",
                "You are \"{member_name}\" on team \"{team_name}\". Your team lead is \"{lead_name}\".\n",
                "\n",
                "Use internal team tools (for example TaskList/SendMessage) to coordinate work.\n",
                "When using SendMessage with string content, always include a non-empty summary.\n",
                "Example: SendMessage type=\"message\" recipient=\"{lead_name}\" content=\"Status update\" summary=\"Status update\"\n",
                "\n",
                "Work contract:\n",
                "Do the assigned work first, then report completion with artifacts and test results.\n",
                "Do not send a pure acknowledgment before you have either completed the work or identified a real blocker.\n",
                "\n",
                "Compaction safety:\n",
                "If context compaction happens and you have no unread messages or your current task is unclear, immediately message {lead_name} and ask for your current assignment.\n",
                "Do not assume you are done — compaction may have dropped your active task context.\n",
                "\n",
                "Escalation:\n",
                "If blocked, send blocker details to {lead_name} immediately. Do not stall silently."
            ),
            team_name = team_name,
            member_name = member_name,
            lead_name = lead_name
        );
        Self::append_role_context_sections(
            &mut rendered,
            role_id,
            communication_style,
            instructions,
            behavioral_contract,
            quality_gates,
            definition_of_done,
            capabilities,
        );
        rendered
    }

    /// Render an OperatorNotice into a tmux-injectable string.
    pub fn render_operator_notice(payload: &OperatorNoticeDelivery) -> String {
        format!(
            "[taurhaus] operator_notice from {}: {}",
            payload.team_name, payload.message
        )
    }

    /// Render a Bootstrap delivery into a tmux-injectable string.
    pub fn render_bootstrap(payload: &BootstrapDelivery) -> String {
        format!(
            "[taurhaus] bootstrap for {} on {}: {}",
            payload.member_name, payload.team_name, payload.message
        )
    }

    /// Render a RecoveryNudge into a tmux-injectable string.
    pub fn render_recovery_nudge(payload: &RecoveryNudgeDelivery) -> String {
        format!(
            "[taurhaus] recovery_nudge for {} on {}: {}",
            payload.member_name, payload.team_name, payload.reason
        )
    }

    /// Render any delivery request variant into tmux-injectable text.
    pub fn render(request: &DeliveryRequest) -> String {
        match request {
            DeliveryRequest::Bootstrap(payload) => Self::render_bootstrap(payload),
            DeliveryRequest::RecoveryNudge(payload) => Self::render_recovery_nudge(payload),
            DeliveryRequest::OperatorNotice(payload) => Self::render_operator_notice(payload),
        }
    }

    fn append_role_context_sections(
        rendered: &mut String,
        role_id: Option<&str>,
        communication_style: Option<&str>,
        instructions: Option<&str>,
        behavioral_contract: Option<&BehavioralContract>,
        quality_gates: Option<&[String]>,
        definition_of_done: Option<&[String]>,
        capabilities: Option<&[String]>,
    ) {
        let role_id = role_id.map(str::trim).filter(|value| !value.is_empty());
        let communication_style = communication_style
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let instructions = instructions
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let behavioral_contract = behavioral_contract.filter(|contract| {
            !contract.communication.is_empty()
                || !contract.execution.is_empty()
                || !contract.escalation.is_empty()
        });
        let has_quality_gates = quality_gates
            .map(Self::has_non_empty_items)
            .unwrap_or(false);
        let has_definition_of_done = definition_of_done
            .map(Self::has_non_empty_items)
            .unwrap_or(false);
        let has_capabilities = capabilities.map(Self::has_non_empty_items).unwrap_or(false);

        if role_id.is_none()
            && communication_style.is_none()
            && instructions.is_none()
            && behavioral_contract.is_none()
            && !has_quality_gates
            && !has_definition_of_done
            && !has_capabilities
        {
            return;
        }

        if let Some(role_id) = role_id {
            rendered.push_str("\n\nRole: ");
            rendered.push_str(role_id);
        }

        if let Some(communication_style) = communication_style {
            rendered.push_str("\n\nCommunication Style:\n");
            rendered.push_str(communication_style);
        }

        if let Some(instructions) = instructions {
            rendered.push_str("\n\nInstructions:\n");
            rendered.push_str(instructions);
        }

        if let Some(contract) = behavioral_contract {
            rendered.push_str("\n\nBehavioral Contract:");
            Self::append_titled_bullets(rendered, "Communication", &contract.communication);
            Self::append_titled_bullets(rendered, "Execution", &contract.execution);
            Self::append_titled_bullets(rendered, "Escalation", &contract.escalation);
        }

        if let Some(quality_gates) = quality_gates {
            if has_quality_gates {
                rendered.push_str("\n\nQuality Gates:\n");
                Self::append_bullets(rendered, quality_gates);
            }
        }

        if let Some(definition_of_done) = definition_of_done {
            if has_definition_of_done {
                rendered.push_str("\n\nDefinition of Done:\n");
                Self::append_bullets(rendered, definition_of_done);
            }
        }

        if let Some(capabilities) = capabilities {
            if has_capabilities {
                rendered.push_str("\n\nCapabilities:\n");
                Self::append_bullets(rendered, capabilities);
            }
        }
    }

    fn has_non_empty_items(items: &[String]) -> bool {
        items.iter().any(|item| !item.trim().is_empty())
    }

    fn append_titled_bullets(rendered: &mut String, title: &str, items: &[String]) {
        if !items.iter().any(|item| !item.trim().is_empty()) {
            return;
        }
        rendered.push('\n');
        rendered.push_str(title);
        rendered.push_str(":\n");
        Self::append_bullets(rendered, items);
    }

    fn append_bullets(rendered: &mut String, items: &[String]) {
        for item in items {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            rendered.push_str("- ");
            rendered.push_str(item);
            rendered.push('\n');
        }
        if rendered.ends_with('\n') {
            rendered.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_operator_notice_format() {
        let payload = OperatorNoticeDelivery {
            member_name: "codex-reviewer".to_string(),
            team_name: "architecture-final".to_string(),
            message: "Please report status".to_string(),
            sender_name: None,
            operational_context: None,
        };

        let rendered = DeliveryRenderer::render_operator_notice(&payload);
        assert_eq!(
            rendered,
            "[taurhaus] operator_notice from architecture-final: Please report status"
        );
    }

    #[test]
    fn render_bootstrap_format() {
        let payload = BootstrapDelivery {
            member_name: "codex-reviewer".to_string(),
            team_name: "architecture-final".to_string(),
            message: "Welcome aboard".to_string(),
        };

        let rendered = DeliveryRenderer::render_bootstrap(&payload);
        assert_eq!(
            rendered,
            "[taurhaus] bootstrap for codex-reviewer on architecture-final: Welcome aboard"
        );
    }

    #[test]
    fn render_recovery_nudge_format() {
        let payload = RecoveryNudgeDelivery {
            member_name: "codex-reviewer".to_string(),
            team_name: "architecture-final".to_string(),
            reason: "No response observed".to_string(),
        };

        let rendered = DeliveryRenderer::render_recovery_nudge(&payload);
        assert_eq!(
            rendered,
            "[taurhaus] recovery_nudge for codex-reviewer on architecture-final: No response observed"
        );
    }

    #[test]
    fn render_dispatches_correctly() {
        let bootstrap = DeliveryRequest::Bootstrap(BootstrapDelivery {
            member_name: "member-a".to_string(),
            team_name: "team-a".to_string(),
            message: "boot".to_string(),
        });
        let recovery = DeliveryRequest::RecoveryNudge(RecoveryNudgeDelivery {
            member_name: "member-b".to_string(),
            team_name: "team-b".to_string(),
            reason: "nudge".to_string(),
        });
        let notice = DeliveryRequest::OperatorNotice(Box::new(OperatorNoticeDelivery {
            member_name: "member-c".to_string(),
            team_name: "team-c".to_string(),
            message: "notice".to_string(),
            sender_name: None,
            operational_context: None,
        }));

        assert_eq!(
            DeliveryRenderer::render(&bootstrap),
            "[taurhaus] bootstrap for member-a on team-a: boot"
        );
        assert_eq!(
            DeliveryRenderer::render(&recovery),
            "[taurhaus] recovery_nudge for member-b on team-b: nudge"
        );
        assert_eq!(
            DeliveryRenderer::render(&notice),
            "[taurhaus] operator_notice from team-c: notice"
        );
    }

    #[test]
    fn render_onboarding_includes_required_commands_with_substitution() {
        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        assert!(rendered.contains("You are \"codex-reviewer\" on team \"architecture-final\"."));
        assert!(rendered.contains(
            "mesh read --unread --mark-read --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains(
            "mesh send {recipient} \"{msg}\" --team architecture-final --name codex-reviewer --summary \"brief\""
        ));
        assert!(rendered
            .contains("mesh task list/get/update --team architecture-final --name codex-reviewer"));
        assert!(rendered.contains("If context compaction happens and you have no unread messages"));
        assert!(rendered.contains("Do not assume you are done"));
        assert!(rendered.contains("If blocked, send blocker details to team-lead immediately."));
    }

    #[test]
    fn render_onboarding_snapshot_format() {
        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let expected = concat!(
            "[taurhaus] onboarding\n",
            "Identity:\n",
            "You are \"codex-reviewer\" on team \"architecture-final\". Your team lead is \"team-lead\".\n",
            "\n",
            "Read loop:\n",
            "mesh read --unread --mark-read --team architecture-final --name codex-reviewer\n",
            "\n",
            "Reply:\n",
            "mesh send {recipient} \"{msg}\" --team architecture-final --name codex-reviewer --summary \"brief\"\n",
            "\n",
            "Tasks:\n",
            "mesh task list/get/update --team architecture-final --name codex-reviewer\n",
            "mesh task list --team architecture-final --name codex-reviewer\n",
            "mesh task get <id> --team architecture-final --name codex-reviewer\n",
            "mesh task update <id> --status completed --team architecture-final --name codex-reviewer\n",
            "\n",
            "Work contract:\n",
            "Acknowledge assignment, execute, then report completion with artifacts and test results.\n",
            "\n",
            "Compaction safety:\n",
            "If context compaction happens and you have no unread messages or your current task is unclear, immediately message team-lead and ask for your current assignment.\n",
            "Do not assume you are done — compaction may have dropped your active task context.\n",
            "\n",
            "Escalation:\n",
            "If blocked, send blocker details to team-lead immediately. Do not stall silently."
        );

        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_onboarding_appends_role_context_when_present() {
        let contract = BehavioralContract {
            communication: vec!["Post concise updates.".to_string()],
            execution: vec!["Ship reviewed patches.".to_string()],
            escalation: vec!["Escalate blockers quickly.".to_string()],
        };
        let capabilities = vec!["code-review".to_string(), "testing".to_string()];
        let quality_gates = vec!["Run the scoped test lane.".to_string()];
        let definition_of_done = vec!["Report the exact shipped behavior.".to_string()];

        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
            Some("codex-reviewer"),
            Some("Brief, evidence-backed updates."),
            Some("Review architecture patches and propose fixes."),
            Some(&contract),
            Some(&quality_gates),
            Some(&definition_of_done),
            Some(&capabilities),
        );

        assert!(rendered.contains("Role: codex-reviewer"));
        assert!(rendered.contains("Communication Style:\nBrief, evidence-backed updates."));
        assert!(rendered.contains("Instructions:\nReview architecture patches and propose fixes."));
        assert!(rendered.contains("Behavioral Contract:"));
        assert!(rendered.contains("Communication:\n- Post concise updates."));
        assert!(rendered.contains("Execution:\n- Ship reviewed patches."));
        assert!(rendered.contains("Escalation:\n- Escalate blockers quickly."));
        assert!(rendered.contains("Quality Gates:\n- Run the scoped test lane."));
        assert!(rendered.contains("Definition of Done:\n- Report the exact shipped behavior."));
        assert!(rendered.contains("Capabilities:"));
        assert!(rendered.contains("- code-review"));
        assert!(rendered.contains("- testing"));
        assert!(rendered.contains(
            "mesh read --unread --mark-read --team architecture-final --name codex-reviewer"
        ));
    }

    #[test]
    fn render_claude_role_context_uses_internal_tools_and_role_sections() {
        let contract = BehavioralContract {
            communication: vec!["Share progress updates.".to_string()],
            execution: vec!["Implement scoped fixes.".to_string()],
            escalation: vec!["Raise blockers immediately.".to_string()],
        };
        let capabilities = vec!["implementation".to_string()];
        let quality_gates = vec!["Run quick verification.".to_string()];
        let definition_of_done = vec!["Ship the requested fix.".to_string()];

        let rendered = DeliveryRenderer::render_claude_role_context(
            "architecture-final",
            "claude-dev",
            "team-lead",
            Some("claude-developer"),
            Some("Crisp internal handoffs."),
            Some("Implement role-specific changes."),
            Some(&contract),
            Some(&quality_gates),
            Some(&definition_of_done),
            Some(&capabilities),
        );

        assert!(rendered.contains("[taurhaus] role_context"));
        assert!(rendered.contains("Use internal team tools"));
        assert!(rendered.contains(
            "When using SendMessage with string content, always include a non-empty summary."
        ));
        assert!(rendered.contains(
            "Example: SendMessage type=\"message\" recipient=\"team-lead\" content=\"Status update\" summary=\"Status update\""
        ));
        assert!(rendered.contains(
            "Do not send a pure acknowledgment before you have either completed the work or identified a real blocker."
        ));
        assert!(rendered.contains("Compaction safety:"));
        assert!(rendered.contains("Do not assume you are done"));
        assert!(rendered.contains("Role: claude-developer"));
        assert!(rendered.contains("Communication Style:\nCrisp internal handoffs."));
        assert!(rendered.contains("Quality Gates:\n- Run quick verification."));
        assert!(rendered.contains("Definition of Done:\n- Ship the requested fix."));
        assert!(rendered.contains("Capabilities:"));
        assert!(rendered.contains("- implementation"));
        assert!(!rendered.contains("mesh read --unread"));
    }
}
