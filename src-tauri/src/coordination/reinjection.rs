//! Post-compaction reinjection payload composition and rendering.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::domain::Member;
use crate::coordination::stores::operational::OperationalContextSnapshot;

pub const OPERATIONAL_REINJECTION_CARD_VERSION: u32 = 1;
pub const POST_COMPACTION_REASON: &str = "post_compaction";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionCard {
    pub version: u32,
    pub reason: String,
    pub generated_at: DateTime<Utc>,
    pub team_name: String,
    pub member_name: String,
    pub role: OperationalReinjectionRole,
    pub task: OperationalReinjectionTask,
    pub boundaries: OperationalReinjectionBoundaries,
    pub working_set: OperationalReinjectionWorkingSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionRole {
    pub role_id: Option<String>,
    pub role_name: Option<String>,
    pub focus_area: Option<String>,
    pub behavior_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionTask {
    pub id: String,
    pub subject: String,
    pub execution_mode: String,
    pub validation_expectation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionBoundaries {
    pub file_ownership_boundary: Vec<String>,
    pub adjacent_fix_policy: String,
    pub override_allowed: bool,
    pub active_override_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionWorkingSet {
    pub project_path: String,
    pub focal_files: Vec<String>,
}

#[derive(Debug, Default)]
pub struct CompactionReinjectionService;

impl CompactionReinjectionService {
    pub fn compose(
        member: &Member,
        snapshot: &OperationalContextSnapshot,
    ) -> OperationalReinjectionCard {
        Self::compose_at(member, snapshot, Utc::now())
    }

    pub fn compose_at(
        member: &Member,
        snapshot: &OperationalContextSnapshot,
        generated_at: DateTime<Utc>,
    ) -> OperationalReinjectionCard {
        OperationalReinjectionCard {
            version: OPERATIONAL_REINJECTION_CARD_VERSION,
            reason: POST_COMPACTION_REASON.to_string(),
            generated_at,
            team_name: snapshot.team_name.clone(),
            member_name: snapshot.member_name.clone(),
            role: OperationalReinjectionRole {
                role_id: normalize_optional(member.role_id.as_deref()),
                role_name: normalize_optional(member.role_name.as_deref()),
                focus_area: normalize_optional(member.focus_area.as_deref()),
                behavior_summary: normalize_optional(member.behavior_summary.as_deref()),
            },
            task: OperationalReinjectionTask {
                id: snapshot.task.id.trim().to_string(),
                subject: snapshot.task.subject.trim().to_string(),
                execution_mode: snapshot.assignment_footer.execution_mode.trim().to_string(),
                validation_expectation: snapshot
                    .assignment_footer
                    .validation_expectation
                    .trim()
                    .to_string(),
            },
            boundaries: OperationalReinjectionBoundaries {
                file_ownership_boundary: normalize_list(
                    &snapshot.assignment_footer.file_ownership_boundary,
                ),
                adjacent_fix_policy: snapshot
                    .assignment_footer
                    .adjacent_fix_policy
                    .trim()
                    .to_string(),
                override_allowed: snapshot.ownership.override_allowed,
                active_override_reason: normalize_optional(
                    snapshot.ownership.active_override_reason.as_deref(),
                ),
            },
            working_set: OperationalReinjectionWorkingSet {
                project_path: snapshot.working_set.project_path.trim().to_string(),
                focal_files: normalize_list(&snapshot.working_set.focal_files),
            },
        }
    }

    pub fn render_claude_additional_context(
        card: &OperationalReinjectionCard,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(card)
    }

    pub fn render_codex_inbox_payload(
        card: &OperationalReinjectionCard,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(card)
    }
}

fn normalize_optional(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::stores::operational::{
        OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };
    use crate::session_scanner::cli_tool::CliTool;

    fn sample_member() -> Member {
        Member {
            name: "architect".to_string(),
            role: MemberRole::Agent,
            role_id: Some("taurhaus-architect".to_string()),
            role_name: Some("Taurhaus Architect".to_string()),
            focus_area: Some("Cross-layer diagnosis".to_string()),
            context_summary: Some("Keeps architecture context warm.".to_string()),
            behavior_summary: Some(
                "Stay concrete, evidence-backed, and escalate ownership ambiguity quickly."
                    .to_string(),
            ),
            instructions: Some("Review architecture edges".to_string()),
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from("/home/mstie/projects/taurhaus"),
            cli_tool: CliTool::Codex,
        }
    }

    fn sample_snapshot() -> OperationalContextSnapshot {
        OperationalContextSnapshot {
            version: 1,
            team_name: "taurhaus-team".to_string(),
            member_name: "architect".to_string(),
            updated_at: DateTime::parse_from_rfc3339("2026-03-08T14:10:00Z")
                .expect("timestamp")
                .with_timezone(&Utc),
            task: OperationalTaskSnapshot {
                id: "673".to_string(),
                subject: "Architecture: post-compaction operational re-injection".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "recommend".to_string(),
                file_ownership_boundary: vec![
                    "docs/architecture/post-compaction-reinjection.md".to_string()
                ],
                adjacent_fix_policy: "no".to_string(),
                validation_expectation: "report-only".to_string(),
                response_expectation: "report-on-completion".to_string(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: "/home/mstie/projects/taurhaus".to_string(),
                focal_files: vec!["docs/architecture/post-compaction-reinjection.md".to_string()],
            },
        }
    }

    #[test]
    fn compose_card_uses_member_role_and_operational_snapshot() {
        let member = sample_member();
        let snapshot = sample_snapshot();
        let generated_at = DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        let card = CompactionReinjectionService::compose_at(&member, &snapshot, generated_at);

        assert_eq!(
            card,
            OperationalReinjectionCard {
                version: 1,
                reason: "post_compaction".to_string(),
                generated_at,
                team_name: "taurhaus-team".to_string(),
                member_name: "architect".to_string(),
                role: OperationalReinjectionRole {
                    role_id: Some("taurhaus-architect".to_string()),
                    role_name: Some("Taurhaus Architect".to_string()),
                    focus_area: Some("Cross-layer diagnosis".to_string()),
                    behavior_summary: Some(
                        "Stay concrete, evidence-backed, and escalate ownership ambiguity quickly."
                            .to_string()
                    ),
                },
                task: OperationalReinjectionTask {
                    id: "673".to_string(),
                    subject: "Architecture: post-compaction operational re-injection".to_string(),
                    execution_mode: "recommend".to_string(),
                    validation_expectation: "report-only".to_string(),
                },
                boundaries: OperationalReinjectionBoundaries {
                    file_ownership_boundary: vec![
                        "docs/architecture/post-compaction-reinjection.md".to_string(),
                    ],
                    adjacent_fix_policy: "no".to_string(),
                    override_allowed: false,
                    active_override_reason: None,
                },
                working_set: OperationalReinjectionWorkingSet {
                    project_path: "/home/mstie/projects/taurhaus".to_string(),
                    focal_files: vec![
                        "docs/architecture/post-compaction-reinjection.md".to_string(),
                    ],
                },
            }
        );
    }

    #[test]
    fn compose_card_normalizes_missing_optional_fields() {
        let mut member = sample_member();
        member.role_id = Some("  ".to_string());
        member.role_name = None;
        member.focus_area = Some(String::new());
        member.behavior_summary = Some(" ".to_string());

        let mut snapshot = sample_snapshot();
        snapshot.assignment_footer.execution_mode = " ".to_string();
        snapshot.assignment_footer.file_ownership_boundary = vec![
            String::new(),
            " src/lib/components/MeshTab.svelte ".to_string(),
            " ".to_string(),
        ];
        snapshot.assignment_footer.validation_expectation = String::new();
        snapshot.assignment_footer.adjacent_fix_policy = "  ".to_string();
        snapshot.ownership.override_allowed = true;
        snapshot.ownership.active_override_reason = Some(" ".to_string());
        snapshot.working_set.project_path = " ".to_string();
        snapshot.working_set.focal_files = vec![" ".to_string(), "Cargo.toml".to_string()];

        let card = CompactionReinjectionService::compose_at(
            &member,
            &snapshot,
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        assert_eq!(card.role.role_id, None);
        assert_eq!(card.role.role_name, None);
        assert_eq!(card.role.focus_area, None);
        assert_eq!(card.role.behavior_summary, None);
        assert_eq!(card.task.execution_mode, "");
        assert_eq!(card.task.validation_expectation, "");
        assert_eq!(
            card.boundaries.file_ownership_boundary,
            vec!["src/lib/components/MeshTab.svelte".to_string()]
        );
        assert_eq!(card.boundaries.adjacent_fix_policy, "");
        assert_eq!(card.boundaries.active_override_reason, None);
        assert_eq!(card.working_set.project_path, "");
        assert_eq!(card.working_set.focal_files, vec!["Cargo.toml".to_string()]);
    }

    #[test]
    fn render_claude_additional_context_matches_expected_json_snapshot() {
        let card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        let rendered =
            CompactionReinjectionService::render_claude_additional_context(&card).expect("json");

        let expected = r#"{
  "version": 1,
  "reason": "post_compaction",
  "generated_at": "2026-03-08T14:10:05Z",
  "team_name": "taurhaus-team",
  "member_name": "architect",
  "role": {
    "role_id": "taurhaus-architect",
    "role_name": "Taurhaus Architect",
    "focus_area": "Cross-layer diagnosis",
    "behavior_summary": "Stay concrete, evidence-backed, and escalate ownership ambiguity quickly."
  },
  "task": {
    "id": "673",
    "subject": "Architecture: post-compaction operational re-injection",
    "execution_mode": "recommend",
    "validation_expectation": "report-only"
  },
  "boundaries": {
    "file_ownership_boundary": [
      "docs/architecture/post-compaction-reinjection.md"
    ],
    "adjacent_fix_policy": "no",
    "override_allowed": false,
    "active_override_reason": null
  },
  "working_set": {
    "project_path": "/home/mstie/projects/taurhaus",
    "focal_files": [
      "docs/architecture/post-compaction-reinjection.md"
    ]
  }
}"#;

        assert_eq!(rendered, expected);
    }

    #[test]
    fn render_codex_inbox_payload_matches_expected_snapshot() {
        let card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        let rendered = CompactionReinjectionService::render_codex_inbox_payload(&card)
            .expect("render codex inbox text");
        let parsed: OperationalReinjectionCard =
            serde_json::from_str(&rendered).expect("parse rendered inbox payload");

        assert_eq!(parsed, card);
    }

    #[test]
    fn render_codex_inbox_payload_preserves_missing_optionals_and_override_status() {
        let mut card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );
        card.role.role_name = None;
        card.role.role_id = None;
        card.role.focus_area = None;
        card.role.behavior_summary = None;
        card.task.execution_mode.clear();
        card.task.validation_expectation.clear();
        card.boundaries.file_ownership_boundary.clear();
        card.boundaries.override_allowed = true;
        card.boundaries.active_override_reason = Some("lead-approved adjacent fix".to_string());

        let rendered = CompactionReinjectionService::render_codex_inbox_payload(&card)
            .expect("render codex inbox text");
        let parsed: OperationalReinjectionCard =
            serde_json::from_str(&rendered).expect("parse rendered inbox payload");

        assert_eq!(parsed, card);
    }

    #[test]
    fn render_codex_inbox_payload_preserves_role_id_when_role_name_is_missing() {
        let mut card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );
        card.role.role_name = None;

        let rendered = CompactionReinjectionService::render_codex_inbox_payload(&card)
            .expect("render codex inbox text");
        let parsed: OperationalReinjectionCard =
            serde_json::from_str(&rendered).expect("parse rendered inbox payload");

        assert_eq!(parsed.role.role_name, None);
        assert_eq!(parsed.role.role_id.as_deref(), Some("taurhaus-architect"));
    }
}
