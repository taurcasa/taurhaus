//! Delivery payload renderer for tmux-injectable content.

use crate::coordination::requests::{
    BootstrapDelivery, DeliveryRequest, OperatorNoticeDelivery, RecoveryNudgeDelivery,
};
use crate::templates::types::{BehavioralContract, RoleTemplate};

#[derive(Clone, Copy, Debug, Default)]
pub struct RoleContext<'a> {
    pub(crate) role_id: Option<&'a str>,
    pub(crate) communication_style: Option<&'a str>,
    pub(crate) instructions: Option<&'a str>,
    pub(crate) behavioral_contract: Option<&'a BehavioralContract>,
    pub(crate) quality_gates: Option<&'a [String]>,
    pub(crate) handoff_expectations: Option<&'a [String]>,
    pub(crate) definition_of_done: Option<&'a [String]>,
    pub(crate) capabilities: Option<&'a [String]>,
}

impl<'a> From<&'a RoleTemplate> for RoleContext<'a> {
    fn from(role: &'a RoleTemplate) -> Self {
        Self {
            role_id: Some(&role.role_id),
            communication_style: role.communication_style.as_deref(),
            instructions: Some(&role.instructions),
            behavioral_contract: Some(&role.behavioral_contract),
            quality_gates: role.quality_gates.as_deref(),
            handoff_expectations: role.handoff_expectations.as_deref(),
            definition_of_done: role.definition_of_done.as_deref(),
            capabilities: Some(&role.capabilities),
        }
    }
}

/// Renders typed delivery payloads into deterministic tmux text.
#[derive(Debug, Default)]
pub struct DeliveryRenderer;

impl DeliveryRenderer {
    /// Render the onboarding contract selected by the harness registry.
    pub fn render_for_tool(
        tool: crate::session_scanner::cli_tool::CliTool,
        team_name: &str,
        member_name: &str,
        lead_name: &str,
        has_role_context: bool,
        role_context: RoleContext<'_>,
    ) -> Option<String> {
        let tool_spec = crate::session_scanner::cli_tool::spec(tool);
        if tool_spec.capabilities.native_inbox_poller {
            return has_role_context.then(|| {
                Self::render_claude_role_context(team_name, member_name, lead_name, role_context)
            });
        }

        let mut rendered = Self::render_onboarding(team_name, member_name, lead_name, role_context);
        if tool_spec.onboarding_exit_hint {
            rendered.push_str(&format!(
                "\n\n{} session:\nInbox file: ~/.claude/teams/{team_name}/inboxes/{member_name}.json (use mesh read above to consume it).\nTo stop cleanly, enter {}.",
                tool_spec.label, tool_spec.exit_command
            ));
            if let Some(hint) = tool_spec.onboarding_delivery_hint {
                rendered.push('\n');
                rendered.push_str(hint);
            }
        }
        Some(rendered)
    }

    /// Render a deterministic onboarding template for non-Claude agents.
    pub fn render_onboarding(
        team_name: &str,
        member_name: &str,
        lead_name: &str,
        role_context: RoleContext<'_>,
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
                "mesh tasks --team {team_name} --name {member_name}\n",
                "mesh task get <id> --team {team_name} --name {member_name}\n",
                "Assignment token: use the token from your assignment notice or mesh task get <id>.\n",
                "mesh task accept <id> --assignment <token> --team {team_name} --name {member_name}\n",
                "mesh task start <id> --assignment <token> --team {team_name} --name {member_name} --active-form \"<working>\"\n",
                "mesh task progress <id> --summary \"<update>\" --team {team_name} --name {member_name}\n",
                "mesh task block <id> --reason \"<blocked>\" --team {team_name} --name {member_name}\n",
                "mesh task review <id> --summary \"<handoff>\" --team {team_name} --name {member_name}\n",
                "mesh task complete <id> --summary \"<result>\" --team {team_name} --name {member_name}\n",
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
        Self::append_role_context_sections(&mut rendered, &role_context);
        rendered
    }

    /// Render role context for Claude agents as an initial team message.
    pub fn render_claude_role_context(
        team_name: &str,
        member_name: &str,
        lead_name: &str,
        role_context: RoleContext<'_>,
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
        Self::append_role_context_sections(&mut rendered, &role_context);
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

    /// The role steering text that follows every onboarding contract. It is
    /// also the body of a generated Claude Code agent definition, so a role
    /// steers a mesh member and a subagent with the very same words.
    pub fn render_role_sections(role_context: &RoleContext<'_>) -> String {
        let mut blocks: Vec<String> = Vec::new();

        if let Some(role_id) = Self::trimmed(role_context.role_id) {
            blocks.push(format!("Role: {role_id}"));
        }

        if let Some(communication_style) = Self::trimmed(role_context.communication_style) {
            blocks.push(format!("Communication Style:\n{communication_style}"));
        }

        if let Some(instructions) = Self::trimmed(role_context.instructions) {
            blocks.push(format!("Instructions:\n{instructions}"));
        }

        if let Some(contract) = role_context.behavioral_contract.filter(|contract| {
            !contract.communication.is_empty()
                || !contract.execution.is_empty()
                || !contract.escalation.is_empty()
        }) {
            let mut block = String::from("Behavioral Contract:");
            Self::append_titled_bullets(&mut block, "Communication", &contract.communication);
            Self::append_titled_bullets(&mut block, "Execution", &contract.execution);
            Self::append_titled_bullets(&mut block, "Escalation", &contract.escalation);
            blocks.push(block);
        }

        for (title, items) in [
            ("Quality Gates", role_context.quality_gates),
            ("Handoff Expectations", role_context.handoff_expectations),
            ("Definition of Done", role_context.definition_of_done),
            ("Capabilities", role_context.capabilities),
        ] {
            let Some(items) = items.filter(|items| Self::has_non_empty_items(items)) else {
                continue;
            };
            let mut block = format!("{title}:\n");
            Self::append_bullets(&mut block, items);
            blocks.push(block);
        }

        blocks.join("\n\n")
    }

    fn append_role_context_sections(rendered: &mut String, role_context: &RoleContext<'_>) {
        let sections = Self::render_role_sections(role_context);
        if sections.is_empty() {
            return;
        }
        rendered.push_str("\n\n");
        rendered.push_str(&sections);
    }

    fn trimmed(value: Option<&str>) -> Option<&str> {
        value.map(str::trim).filter(|value| !value.is_empty())
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
            RoleContext::default(),
        );

        assert!(rendered.contains("You are \"codex-reviewer\" on team \"architecture-final\"."));
        assert!(rendered.contains(
            "mesh read --unread --mark-read --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains(
            "mesh send {recipient} \"{msg}\" --team architecture-final --name codex-reviewer --summary \"brief\""
        ));
        assert!(rendered.contains("mesh tasks --team architecture-final --name codex-reviewer"));
        assert!(
            rendered.contains("mesh task get <id> --team architecture-final --name codex-reviewer")
        );
        assert!(rendered.contains(
            "Assignment token: use the token from your assignment notice or mesh task get <id>."
        ));
        assert!(rendered.contains(
            "mesh task accept <id> --assignment <token> --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains(
            "mesh task start <id> --assignment <token> --team architecture-final --name codex-reviewer --active-form \"<working>\""
        ));
        assert!(rendered.contains(
            "mesh task progress <id> --summary \"<update>\" --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains(
            "mesh task block <id> --reason \"<blocked>\" --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains(
            "mesh task review <id> --summary \"<handoff>\" --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains(
            "mesh task complete <id> --summary \"<result>\" --team architecture-final --name codex-reviewer"
        ));
        assert!(rendered.contains("If context compaction happens and you have no unread messages"));
        assert!(rendered.contains("Do not assume you are done"));
        assert!(rendered.contains("If blocked, send blocker details to team-lead immediately."));
    }

    #[test]
    fn agy_onboarding_teaches_exit_and_inbox_path() {
        // Regression: commit ac6f006 taught the mesh lifecycle but had no agy
        // variant, leaving Antigravity agents without their stop or inbox path.
        let rendered = DeliveryRenderer::render_for_tool(
            crate::session_scanner::cli_tool::CliTool::Agy,
            "architecture-final",
            "agy-reviewer",
            "team-lead",
            true,
            RoleContext::default(),
        )
        .expect("agy onboarding");

        assert!(rendered.contains("~/.claude/teams/architecture-final/inboxes/agy-reviewer.json"));
        assert!(rendered.contains("Antigravity session:"));
        assert!(rendered.contains("enter /exit"));
    }

    #[test]
    fn grok_onboarding_teaches_quit_the_inbox_path_and_the_queueing_enter() {
        // Regression: commit ac6f006 hard-coded the one harness that needed an
        // exit hint, so grok would have been onboarded with Antigravity's
        // heading, Claude's `/exit`, and nothing about its queueing Enter key.
        let rendered = DeliveryRenderer::render_for_tool(
            crate::session_scanner::cli_tool::CliTool::Grok,
            "architecture-final",
            "grok-developer",
            "team-lead",
            true,
            RoleContext::default(),
        )
        .expect("grok onboarding");

        assert!(rendered.contains("Grok session:"));
        assert!(rendered.contains("~/.claude/teams/architecture-final/inboxes/grok-developer.json"));
        assert!(rendered.contains("enter /quit"));
        assert!(rendered.contains("Ctrl+Enter interjects immediately"));
    }

    #[test]
    fn rendered_onboarding_contains_no_task_list_or_task_update() {
        // Regression: commit 5cebfef taught agents a nonexistent task-list command and a
        // lead-only task-update command, so they could not inspect or complete assigned work
        // (mesh-findings P4).
        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
            RoleContext::default(),
        );

        assert!(!rendered.contains("task list"));
        assert!(!rendered.contains("task update"));
    }

    #[test]
    fn render_onboarding_snapshot_format() {
        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
            RoleContext::default(),
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
            "mesh tasks --team architecture-final --name codex-reviewer\n",
            "mesh task get <id> --team architecture-final --name codex-reviewer\n",
            "Assignment token: use the token from your assignment notice or mesh task get <id>.\n",
            "mesh task accept <id> --assignment <token> --team architecture-final --name codex-reviewer\n",
            "mesh task start <id> --assignment <token> --team architecture-final --name codex-reviewer --active-form \"<working>\"\n",
            "mesh task progress <id> --summary \"<update>\" --team architecture-final --name codex-reviewer\n",
            "mesh task block <id> --reason \"<blocked>\" --team architecture-final --name codex-reviewer\n",
            "mesh task review <id> --summary \"<handoff>\" --team architecture-final --name codex-reviewer\n",
            "mesh task complete <id> --summary \"<result>\" --team architecture-final --name codex-reviewer\n",
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
        let handoff_expectations = vec!["Summarize evidence and residual risk.".to_string()];
        let definition_of_done = vec!["Report the exact shipped behavior.".to_string()];

        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
            RoleContext {
                role_id: Some("codex-reviewer"),
                communication_style: Some("Brief, evidence-backed updates."),
                instructions: Some("Review architecture patches and propose fixes."),
                behavioral_contract: Some(&contract),
                quality_gates: Some(&quality_gates),
                handoff_expectations: Some(&handoff_expectations),
                definition_of_done: Some(&definition_of_done),
                capabilities: Some(&capabilities),
            },
        );

        assert!(rendered.contains("Role: codex-reviewer"));
        assert!(rendered.contains("Communication Style:\nBrief, evidence-backed updates."));
        assert!(rendered.contains("Instructions:\nReview architecture patches and propose fixes."));
        assert!(rendered.contains("Behavioral Contract:"));
        assert!(rendered.contains("Communication:\n- Post concise updates."));
        assert!(rendered.contains("Execution:\n- Ship reviewed patches."));
        assert!(rendered.contains("Escalation:\n- Escalate blockers quickly."));
        assert!(rendered.contains("Quality Gates:\n- Run the scoped test lane."));
        assert!(rendered.contains("Handoff Expectations:\n- Summarize evidence and residual risk."));
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
            RoleContext {
                role_id: Some("claude-developer"),
                communication_style: Some("Crisp internal handoffs."),
                instructions: Some("Implement role-specific changes."),
                behavioral_contract: Some(&contract),
                quality_gates: Some(&quality_gates),
                handoff_expectations: None,
                definition_of_done: Some(&definition_of_done),
                capabilities: Some(&capabilities),
            },
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
