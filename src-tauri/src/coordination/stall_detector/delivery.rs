use std::path::Path;

use crate::coordination::domain::MemberRole;
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
use crate::coordination::stores::TeamConfigStore;

use super::history;
use super::types::{StallDetectorConfig, StallTriggerStage, TransitionDecision};

pub(super) fn dispatch_escalations(
    config: &StallDetectorConfig,
    orchestrator: &mut CoordinationOrchestrator,
    decisions: &[TransitionDecision],
) {
    for decision in decisions {
        if team_uses_mesh_owned_idle_delivery(
            &orchestrator.teams_dir,
            &decision.transition.team_name,
        ) {
            tracing::info!(
                team_name = %decision.transition.team_name,
                member_name = %decision.transition.member_name,
                stage = %decision.trigger_stage.as_str(),
                "stall detector delegated idle reminder delivery to mesh"
            );
            continue;
        }

        let result = match decision.trigger_stage {
            StallTriggerStage::StageA => {
                let response_window_secs = config
                    .hard_escalate_after_secs
                    .saturating_sub(config.soft_nudge_after_secs);
                let response_minutes = std::cmp::max(1, response_window_secs.div_ceil(60));
                let message = format!(
                    "Are you still working on Task #N? Reply with status (working, blocked, done) within {response_minutes} min."
                );
                orchestrator.deliver_message(DeliveryRequest::operator_notice(
                    OperatorNoticeDelivery {
                        member_name: decision.transition.member_name.clone(),
                        team_name: decision.transition.team_name.clone(),
                        message,
                        sender_name: Some("stall-detector".to_string()),
                        operational_context: None,
                    },
                ))
            }
            StallTriggerStage::StageB => {
                let lead_name =
                    resolve_team_lead_name(orchestrator, &decision.transition.team_name);
                match lead_name {
                    Ok(lead_name) => {
                        let message = history::render_stage_b_evidence_message(
                            decision,
                            &decision.transition,
                        );
                        orchestrator.deliver_message(DeliveryRequest::operator_notice(
                            OperatorNoticeDelivery {
                                member_name: lead_name,
                                team_name: decision.transition.team_name.clone(),
                                message,
                                sender_name: Some("stall-detector".to_string()),
                                operational_context: None,
                            },
                        ))
                    }
                    Err(err) => Err(err),
                }
            }
        };

        if let Err(err) = result {
            tracing::warn!(
                team_name = %decision.transition.team_name,
                member_name = %decision.transition.member_name,
                stage = %decision.trigger_stage.as_str(),
                error = %err,
                "stall detector escalation delivery failed"
            );
        }
    }
}

fn team_uses_mesh_owned_idle_delivery(teams_dir: &Path, team_name: &str) -> bool {
    TeamConfigStore::load(teams_dir, team_name).is_ok()
}

fn resolve_team_lead_name(
    orchestrator: &CoordinationOrchestrator,
    team_name: &str,
) -> Result<String, CoordinationError> {
    let config = TeamConfigStore::load(&orchestrator.teams_dir, team_name)?;
    config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .map(|member| member.name.clone())
        .ok_or_else(|| {
            CoordinationError::NotFound(format!("lead member not found in team '{team_name}'"))
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use chrono::Duration as ChronoDuration;
    use chrono::{DateTime, Utc};
    use tempfile::TempDir;

    use super::super::service::StallDetectorService;
    use super::super::types::StallStage;
    use super::*;
    use crate::coordination::backend::fake::FakeBackend;
    use crate::coordination::domain::Member;
    use crate::session_scanner::cli_tool::CliTool;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_member(name: &str, role: MemberRole) -> Member {
        Member {
            name: name.to_string(),
            role,
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            runtime_compact_summary: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from("/tmp/project"),
            cli_tool: CliTool::Codex,
        }
    }

    fn test_orchestrator_with_team(
        team_name: &str,
    ) -> (
        CoordinationOrchestrator,
        Arc<FakeBackend>,
        TempDir,
        String,
        String,
    ) {
        let teams_tmp = TempDir::new().expect("temp teams dir");
        let backend = Arc::new(FakeBackend::default());
        let mut orchestrator =
            CoordinationOrchestrator::new(teams_tmp.path().to_path_buf(), backend.clone());
        orchestrator
            .create_team(team_name, None)
            .expect("create team");

        let lead_name = "team-lead".to_string();
        let member_name = "agent-a".to_string();
        orchestrator
            .add_member(team_name, sample_member(&lead_name, MemberRole::Lead))
            .expect("add lead");
        orchestrator
            .add_member(team_name, sample_member(&member_name, MemberRole::Agent))
            .expect("add member");

        (orchestrator, backend, teams_tmp, lead_name, member_name)
    }

    #[test]
    fn stage_a_delivery_is_delegated_to_mesh_for_mesh_managed_team() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let (mut orchestrator, backend, _tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let now = ts("2026-03-05T13:00:00Z");
        service.upsert_member("team-a", &member_name, now);
        service.set_last_any_signal_for_tests(
            "team-a",
            &member_name,
            now - ChronoDuration::seconds(300),
        );

        let transitions = service.poll_once_with_orchestrator_at(now, &mut orchestrator);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].to, StallStage::SoftNudged);

        let delivered = backend.delivered_requests();
        assert!(
            delivered.is_empty(),
            "mesh-managed teams should delegate stage-a idle reminder delivery to mesh"
        );
    }

    #[test]
    fn stage_b_delivery_is_delegated_to_mesh_for_mesh_managed_team() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let (mut orchestrator, backend, _tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let stage_a_now = ts("2026-03-05T13:10:00Z");
        service.upsert_member("team-a", &member_name, stage_a_now);
        service.set_last_any_signal_for_tests(
            "team-a",
            &member_name,
            stage_a_now - ChronoDuration::seconds(300),
        );
        let _ = service.poll_once_with_orchestrator_at(stage_a_now, &mut orchestrator);

        let stage_b_now = stage_a_now + ChronoDuration::seconds(240);
        let first = service.poll_once_with_orchestrator_at(stage_b_now, &mut orchestrator);
        assert!(
            first.is_empty(),
            "first stage-b check should defer for hysteresis"
        );
        let second = service.poll_once_with_orchestrator_at(
            stage_b_now + ChronoDuration::seconds(30),
            &mut orchestrator,
        );
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].to, StallStage::Escalated);

        let delivered = backend.delivered_requests();
        assert!(
            delivered.is_empty(),
            "mesh-managed teams should delegate stage-b idle escalation delivery to mesh"
        );
    }
}
