//! Delivery payload renderer for tmux-injectable content.

use crate::coordination::requests::{
    BootstrapDelivery, DeliveryRequest, OperatorNoticeDelivery, RecoveryNudgeDelivery,
};

/// Renders typed delivery payloads into deterministic tmux text.
#[derive(Debug, Default)]
pub struct DeliveryRenderer;

impl DeliveryRenderer {
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
}

