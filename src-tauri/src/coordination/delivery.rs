//! Delivery payload renderer for tmux-injectable content.

use crate::coordination::requests::{
    BootstrapDelivery, DeliveryRequest, OperatorNoticeDelivery, RecoveryNudgeDelivery,
};

/// Renders typed delivery payloads into deterministic tmux text.
#[derive(Debug, Default)]
pub struct DeliveryRenderer;

impl DeliveryRenderer {
    /// Render a deterministic onboarding template for non-Claude agents.
    pub fn render_onboarding(team_name: &str, member_name: &str, lead_name: &str) -> String {
        format!(
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
                "Escalation:\n",
                "If blocked, send blocker details to {lead_name} immediately. Do not stall silently."
            ),
            team_name = team_name,
            member_name = member_name,
            lead_name = lead_name
        )
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
        let notice = DeliveryRequest::OperatorNotice(OperatorNoticeDelivery {
            member_name: "member-c".to_string(),
            team_name: "team-c".to_string(),
            message: "notice".to_string(),
            sender_name: None,
        });

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
        assert!(rendered.contains("If blocked, send blocker details to team-lead immediately."));
    }

    #[test]
    fn render_onboarding_snapshot_format() {
        let rendered = DeliveryRenderer::render_onboarding(
            "architecture-final",
            "codex-reviewer",
            "team-lead",
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
            "Escalation:\n",
            "If blocked, send blocker details to team-lead immediately. Do not stall silently."
        );

        assert_eq!(rendered, expected);
    }
}
