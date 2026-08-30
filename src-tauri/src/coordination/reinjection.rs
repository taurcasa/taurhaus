//! Post-compaction reinjection payload composition and rendering.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::coordination::domain::Member;
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::operational::OperationalContextSnapshot;
use crate::coordination::stores::{MeshInboxMessage, MeshInboxStore};
use crate::templates::types::RuntimeCompactSummary;

pub const OPERATIONAL_REINJECTION_CARD_VERSION: u32 = 1;
pub const POST_COMPACTION_REASON: &str = "post_compaction";
/// Inbox summary the mesh member sees for a queued post-compaction card.
pub const POST_COMPACTION_INBOX_SUMMARY: &str = "post_compaction_context";

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
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub communication_style: Option<String>,
    pub instructions: Option<String>,
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    pub quality_gates: Vec<String>,
    pub handoff_expectations: Vec<String>,
    pub definition_of_done: Vec<String>,
    pub phase_scope: Vec<String>,
    pub mode: Option<String>,
    pub inherits_from: Option<String>,
    pub required_artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionTask {
    pub id: String,
    pub subject: String,
    pub execution_mode: String,
    pub validation_expectation: String,
    pub response_expectation: String,
    /// Reasoning effort the lead attached to this assignment. Empty when the
    /// assignment carried none. Additive, so a card written before the field
    /// existed still decodes.
    #[serde(default)]
    pub effort: String,
    /// Why the lead chose that level.
    #[serde(default)]
    pub effort_why: String,
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
    pub fn snapshot_has_resumable_task(snapshot: &OperationalContextSnapshot) -> bool {
        let status = snapshot.task.status.trim();
        let has_task_identity =
            !snapshot.task.id.trim().is_empty() && !snapshot.task.subject.trim().is_empty();

        has_task_identity && matches!(status, "pending" | "in_progress")
    }

    pub fn compose(
        member: &Member,
        snapshot: &OperationalContextSnapshot,
    ) -> OperationalReinjectionCard {
        Self::compose_at(member, snapshot, Utc::now())
    }

    /// Queue the card in the member's mesh inbox.
    ///
    /// This is the delivery for every harness that does not read a hook's
    /// stdout — the transcript-signal path Codex uses, and grok, whose passive
    /// `SessionStart` hook discards whatever the bridge prints.
    pub fn deliver_to_inbox(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
        card: &OperationalReinjectionCard,
        now: DateTime<Utc>,
    ) -> Result<(), CoordinationError> {
        let rendered_payload = Self::render_additional_context_text(card).map_err(|error| {
            CoordinationError::StoreError(format!(
                "failed to serialize post-compaction card for '{member_name}' in '{team_name}': {error}"
            ))
        })?;
        let inbox_message = MeshInboxMessage::operator_originated(
            member_name,
            rendered_payload,
            Some(POST_COMPACTION_INBOX_SUMMARY.to_string()),
            now,
            None,
        );
        MeshInboxStore::append(teams_dir, team_name, member_name, &inbox_message)
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
                context_summary: normalize_optional(member.context_summary.as_deref()),
                behavior_summary: normalize_optional(member.behavior_summary.as_deref()),
                communication_style: normalize_optional(member.communication_style.as_deref()),
                instructions: normalize_optional(member.instructions.as_deref()),
                runtime_compact_summary: member.runtime_compact_summary.clone(),
                quality_gates: normalize_list(member.quality_gates.as_deref().unwrap_or(&[])),
                handoff_expectations: normalize_list(
                    member.handoff_expectations.as_deref().unwrap_or(&[]),
                ),
                definition_of_done: normalize_list(
                    member.definition_of_done.as_deref().unwrap_or(&[]),
                ),
                phase_scope: normalize_list(member.phase_scope.as_deref().unwrap_or(&[])),
                mode: normalize_optional(member.mode.as_deref()),
                inherits_from: normalize_optional(member.inherits_from.as_deref()),
                required_artifacts: normalize_list(
                    member.required_artifacts.as_deref().unwrap_or(&[]),
                ),
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
                response_expectation: snapshot
                    .assignment_footer
                    .response_expectation
                    .trim()
                    .to_string(),
                effort: snapshot.assignment_footer.task_effort.trim().to_string(),
                effort_why: snapshot
                    .assignment_footer
                    .task_effort_why
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

    pub fn render_additional_context_text(
        card: &OperationalReinjectionCard,
    ) -> Result<String, serde_json::Error> {
        let mut lines = vec![
            "[taurhaus] restored_working_context_after_compaction".to_string(),
            "Continue the active assignment using the restored context below.".to_string(),
            "Do not stop to summarize or acknowledge this card.".to_string(),
            "Reply only if blocked, the context is still insufficient, or the task is complete."
                .to_string(),
            String::new(),
            format!("Current task: #{} — {}", card.task.id, card.task.subject),
        ];

        if !card.task.effort.is_empty() {
            lines.push(if card.task.effort_why.is_empty() {
                format!("Effort: {}", card.task.effort)
            } else {
                format!("Effort: {} — {}", card.task.effort, card.task.effort_why)
            });
        }
        if !card.task.execution_mode.is_empty() {
            lines.push(format!("Execution mode: {}", card.task.execution_mode));
        }
        if !card.task.validation_expectation.is_empty() {
            lines.push(format!(
                "Validation expectation: {}",
                card.task.validation_expectation
            ));
        }
        if !card.task.response_expectation.is_empty() {
            lines.push(format!(
                "Response expectation: {}",
                card.task.response_expectation
            ));
        }
        if let Some(role_line) = format_role_line(&card.role) {
            lines.push(format!("Role: {role_line}"));
        }
        if let Some(focus_area) = card.role.focus_area.as_deref() {
            lines.push(format!("Focus area: {focus_area}"));
        }
        if let Some(context_summary) = card.role.context_summary.as_deref() {
            lines.push(format!("Context summary: {context_summary}"));
        }
        if let Some(behavior_summary) = card.role.behavior_summary.as_deref() {
            lines.push(format!("Behavior: {behavior_summary}"));
        }
        if let Some(communication_style) = card.role.communication_style.as_deref() {
            lines.push(format!("Communication style: {communication_style}"));
        }
        if let Some(instructions) = card.role.instructions.as_deref() {
            lines.push(String::new());
            lines.push("Full role instructions:".to_string());
            lines.push(instructions.to_string());
            lines.push(String::new());
        }
        if let Some(mode) = card.role.mode.as_deref() {
            lines.push(format!("Mode: {mode}"));
        }
        if let Some(inherits_from) = card.role.inherits_from.as_deref() {
            lines.push(format!("Inherits from: {inherits_from}"));
        }
        append_bullet_section(
            &mut lines,
            "Quality gates",
            &card.role.quality_gates,
            "No explicit quality gates recorded.",
        );
        append_bullet_section(
            &mut lines,
            "Handoff expectations",
            &card.role.handoff_expectations,
            "No explicit handoff expectations recorded.",
        );
        append_bullet_section(
            &mut lines,
            "Definition of done",
            &card.role.definition_of_done,
            "No explicit definition of done recorded.",
        );
        append_bullet_section(
            &mut lines,
            "Phase scope",
            &card.role.phase_scope,
            "No explicit phase scope recorded.",
        );
        append_bullet_section(
            &mut lines,
            "Required artifacts",
            &card.role.required_artifacts,
            "No explicit required artifacts recorded.",
        );
        if let Some(summary) = card.role.runtime_compact_summary.as_ref() {
            lines.push(format!("Role purpose: {}", summary.role_purpose));
            append_bullet_section(
                &mut lines,
                "Keep doing",
                &summary.keep_doing,
                "No keep-doing guidance recorded.",
            );
            append_bullet_section(
                &mut lines,
                "Workflow sequence",
                &summary.workflow_sequence,
                "No workflow sequence recorded.",
            );
            append_bullet_section(
                &mut lines,
                "Avoid",
                &summary.avoid,
                "No avoid guidance recorded.",
            );
            append_bullet_section(
                &mut lines,
                "Escalate when",
                &summary.escalate_when,
                "No escalation guidance recorded.",
            );
        }
        if !card.working_set.project_path.is_empty() {
            lines.push(format!("Project: {}", card.working_set.project_path));
        }

        append_bullet_section(
            &mut lines,
            "Focal files",
            &card.working_set.focal_files,
            "Use the current task context if these are empty.",
        );
        append_bullet_section(
            &mut lines,
            "File ownership boundary",
            &card.boundaries.file_ownership_boundary,
            "No explicit file boundary recorded.",
        );

        if !card.boundaries.adjacent_fix_policy.is_empty() {
            lines.push(format!(
                "Adjacent fix policy: {}",
                card.boundaries.adjacent_fix_policy
            ));
        }
        lines.push(format!(
            "Override allowed: {}",
            if card.boundaries.override_allowed {
                "yes"
            } else {
                "no"
            }
        ));
        if let Some(active_override_reason) = card.boundaries.active_override_reason.as_deref() {
            lines.push(format!("Active override reason: {active_override_reason}"));
        }

        lines.push(String::new());
        lines.push(
            "Next action: continue the current task immediately with this restored context."
                .to_string(),
        );

        Ok(lines.join("\n"))
    }
}

fn format_role_line(role: &OperationalReinjectionRole) -> Option<String> {
    match (role.role_name.as_deref(), role.role_id.as_deref()) {
        (Some(role_name), Some(role_id)) => Some(format!("{role_name} ({role_id})")),
        (Some(role_name), None) => Some(role_name.to_string()),
        (None, Some(role_id)) => Some(role_id.to_string()),
        (None, None) => None,
    }
}

fn append_bullet_section(
    lines: &mut Vec<String>,
    title: &str,
    items: &[String],
    empty_fallback: &str,
) {
    if items.is_empty() {
        lines.push(format!("{title}: {empty_fallback}"));
        return;
    }

    lines.push(format!("{title}:"));
    for item in items {
        lines.push(format!("- {item}"));
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
    use std::fs;
    use std::path::PathBuf;

    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::stores::operational::{
        OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };
    use crate::session_scanner::cli_tool::CliTool;
    use crate::templates::types::{RoleKind, RoleTemplate};

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
            communication_style: Some("Short, evidence-backed progress notes.".to_string()),
            runtime_compact_summary: Some(RuntimeCompactSummary {
                role_purpose:
                    "Preserve cross-layer diagnosis and review-vs-implementation boundaries after compaction."
                        .to_string(),
                keep_doing: vec![
                    "Tie findings to concrete code paths, runtime evidence, and real failure modes."
                        .to_string(),
                    "State clearly whether the current output is review, recommendation, or a narrow fix."
                        .to_string(),
                ],
                workflow_sequence: vec![
                    "Reconnect the active task, owned surface, and failing behavior before changing scope."
                        .to_string(),
                    "Trace the issue across frontend, backend, runtime, and mesh layers until the root cause is explicit."
                        .to_string(),
                    "Deliver findings or a bounded fix with exact evidence, validation, and residual risk."
                        .to_string(),
                ],
                avoid: vec![
                    "Do not drift into generic implementation work or broad refactors during an audit task."
                        .to_string(),
                    "Do not blur review-only, recommend-only, and implement-now modes."
                        .to_string(),
                ],
                escalate_when: vec![
                    "Escalate ownership ambiguity, direction changes, or blocked cross-role boundaries immediately."
                        .to_string(),
                ],
            }),
            instructions: Some("Review architecture edges".to_string()),
            behavioral_contract: None,
            quality_gates: Some(vec![
                "Tie conclusions to concrete repo evidence.".to_string(),
                "Avoid speculative architecture changes.".to_string(),
            ]),
            handoff_expectations: Some(vec![
                "Summarize evidence and residual risk.".to_string(),
            ]),
            definition_of_done: Some(vec![
                "Root cause and impact are explicit.".to_string(),
                "Residual risk is documented.".to_string(),
            ]),
            phase_scope: Some(vec!["investigation".to_string(), "recommendation".to_string()]),
            mode: Some("analysis".to_string()),
            inherits_from: Some("taurhaus-architect-base".to_string()),
            required_artifacts: Some(vec![
                "root-cause summary".to_string(),
                "validation notes".to_string(),
            ]),
            capabilities: None,
            model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: Some("high".to_string()),
            project_path: PathBuf::from("/home/user/projects/taurhaus"),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
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
                ..Default::default()
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "recommend".to_string(),
                file_ownership_boundary: vec![
                    "docs/architecture/post-compaction-reinjection.md".to_string()
                ],
                adjacent_fix_policy: "no".to_string(),
                validation_expectation: "report-only".to_string(),
                response_expectation: "report-on-completion".to_string(),
                task_effort: String::new(),
                task_effort_why: String::new(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: "/home/user/projects/taurhaus".to_string(),
                focal_files: vec!["docs/architecture/post-compaction-reinjection.md".to_string()],
            },
        }
    }

    fn load_role_template(role_id: &str) -> RoleTemplate {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("templates")
            .join("roles")
            .join(format!("{role_id}.yaml"));
        let raw = fs::read_to_string(&path).expect("read role template");
        serde_norway::from_str::<RoleTemplate>(&raw)
            .unwrap_or_else(|err| panic!("parse role template {}: {err}", path.display()))
    }

    fn member_from_role(role: &RoleTemplate) -> Member {
        Member {
            name: role.role_id.clone(),
            role: match role.kind {
                RoleKind::Lead => MemberRole::Lead,
                RoleKind::Agent => MemberRole::Agent,
            },
            role_id: Some(role.role_id.clone()),
            role_name: Some(role.name.clone()),
            focus_area: role.focus_area.clone(),
            context_summary: role.context_summary.clone(),
            behavior_summary: role.behavior_summary.clone(),
            communication_style: role.communication_style.clone(),
            runtime_compact_summary: role.runtime_compact_summary.clone(),
            instructions: Some(role.instructions.clone()),
            behavioral_contract: Some(role.behavioral_contract.clone()),
            quality_gates: role.quality_gates.clone(),
            handoff_expectations: role.handoff_expectations.clone(),
            definition_of_done: role.definition_of_done.clone(),
            phase_scope: role.phase_scope.clone(),
            mode: role.mode.clone(),
            inherits_from: role.inherits_from.clone(),
            required_artifacts: role.required_artifacts.clone(),
            capabilities: Some(role.capabilities.clone()),
            model: Some(role.defaults.model.clone()),
            reasoning_effort: role.defaults.reasoning_effort.clone(),
            project_path: PathBuf::from("/home/user/projects/taurhaus"),
            cli_tool: role.defaults.cli_tool,
            extra: Default::default(),
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
                    context_summary: Some("Keeps architecture context warm.".to_string()),
                    behavior_summary: Some(
                        "Stay concrete, evidence-backed, and escalate ownership ambiguity quickly."
                            .to_string()
                    ),
                    communication_style: Some("Short, evidence-backed progress notes.".to_string()),
                    instructions: Some("Review architecture edges".to_string()),
                    runtime_compact_summary: sample_member().runtime_compact_summary,
                    quality_gates: vec![
                        "Tie conclusions to concrete repo evidence.".to_string(),
                        "Avoid speculative architecture changes.".to_string(),
                    ],
                    handoff_expectations: vec!["Summarize evidence and residual risk.".to_string(),],
                    definition_of_done: vec![
                        "Root cause and impact are explicit.".to_string(),
                        "Residual risk is documented.".to_string(),
                    ],
                    phase_scope: vec!["investigation".to_string(), "recommendation".to_string(),],
                    mode: Some("analysis".to_string()),
                    inherits_from: Some("taurhaus-architect-base".to_string()),
                    required_artifacts: vec![
                        "root-cause summary".to_string(),
                        "validation notes".to_string(),
                    ],
                },
                task: OperationalReinjectionTask {
                    id: "673".to_string(),
                    subject: "Architecture: post-compaction operational re-injection".to_string(),
                    execution_mode: "recommend".to_string(),
                    validation_expectation: "report-only".to_string(),
                    response_expectation: "report-on-completion".to_string(),
                    effort: String::new(),
                    effort_why: String::new(),
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
                    project_path: "/home/user/projects/taurhaus".to_string(),
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
        assert_eq!(
            card.role.context_summary,
            Some("Keeps architecture context warm.".to_string())
        );
        assert_eq!(card.role.behavior_summary, None);
        assert_eq!(
            card.role.communication_style,
            Some("Short, evidence-backed progress notes.".to_string())
        );
        assert!(card.role.runtime_compact_summary.is_some());
        assert_eq!(
            card.role.quality_gates,
            vec![
                "Tie conclusions to concrete repo evidence.".to_string(),
                "Avoid speculative architecture changes.".to_string(),
            ]
        );
        assert_eq!(
            card.role.definition_of_done,
            vec![
                "Root cause and impact are explicit.".to_string(),
                "Residual risk is documented.".to_string(),
            ]
        );
        assert_eq!(
            card.role.phase_scope,
            vec!["investigation".to_string(), "recommendation".to_string()]
        );
        assert_eq!(card.role.mode, Some("analysis".to_string()));
        assert_eq!(
            card.role.inherits_from,
            Some("taurhaus-architect-base".to_string())
        );
        assert_eq!(
            card.role.required_artifacts,
            vec![
                "root-cause summary".to_string(),
                "validation notes".to_string()
            ]
        );
        assert_eq!(card.task.execution_mode, "");
        assert_eq!(card.task.validation_expectation, "");
        assert_eq!(card.task.response_expectation, "report-on-completion");
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
    fn render_additional_context_text_is_imperative_resume_card() {
        let card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        let rendered = CompactionReinjectionService::render_additional_context_text(&card)
            .expect("render text");

        assert!(rendered.contains("[taurhaus] restored_working_context_after_compaction"));
        assert!(
            rendered.contains("Continue the active assignment using the restored context below.")
        );
        assert!(rendered.contains("Do not stop to summarize or acknowledge this card."));
        assert!(rendered.contains("Current task: #673"));
        assert!(rendered.contains("Execution mode: recommend"));
        assert!(rendered.contains("Validation expectation: report-only"));
        assert!(rendered.contains("Response expectation: report-on-completion"));
        assert!(rendered.contains("Role: Taurhaus Architect (taurhaus-architect)"));
        assert!(rendered.contains("Focus area: Cross-layer diagnosis"));
        assert!(rendered.contains("Context summary: Keeps architecture context warm."));
        assert!(rendered.contains(
            "Behavior: Stay concrete, evidence-backed, and escalate ownership ambiguity quickly."
        ));
        assert!(rendered.contains("Communication style: Short, evidence-backed progress notes."));
        assert!(rendered.contains("Full role instructions:"));
        assert!(rendered.contains("Review architecture edges"));
        assert!(rendered.contains("Mode: analysis"));
        assert!(rendered.contains("Inherits from: taurhaus-architect-base"));
        assert!(rendered.contains("Quality gates:"));
        assert!(rendered.contains("- Tie conclusions to concrete repo evidence."));
        assert!(rendered.contains("Definition of done:"));
        assert!(rendered.contains("Phase scope:"));
        assert!(rendered.contains("Required artifacts:"));
        assert!(rendered.contains(
            "Role purpose: Preserve cross-layer diagnosis and review-vs-implementation boundaries after compaction."
        ));
        assert!(rendered.contains("Keep doing:"));
        assert!(rendered.contains("Workflow sequence:"));
        assert!(rendered.contains("Avoid:"));
        assert!(rendered.contains("Escalate when:"));
        assert!(rendered.contains("Project: /home/user/projects/taurhaus"));
        assert!(rendered.contains("Focal files:"));
        assert!(rendered.contains("- docs/architecture/post-compaction-reinjection.md"));
        assert!(rendered.contains("File ownership boundary:"));
        assert!(rendered.contains("Adjacent fix policy: no"));
        assert!(rendered.contains("Override allowed: no"));
        assert!(rendered.contains(
            "Next action: continue the current task immediately with this restored context."
        ));
    }

    #[test]
    fn the_card_restates_the_effort_the_lead_asked_for() {
        // A compaction is exactly where the member loses the `/effort` mesh
        // typed into the pane and the reason that came with it.
        let mut snapshot = sample_snapshot();
        snapshot.assignment_footer.task_effort = "high".to_string();
        snapshot.assignment_footer.task_effort_why = "the migration is irreversible".to_string();

        let card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &snapshot,
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        assert_eq!(card.task.effort, "high");
        assert_eq!(card.task.effort_why, "the migration is irreversible");

        let rendered = CompactionReinjectionService::render_additional_context_text(&card)
            .expect("render text");
        assert!(rendered.contains("Effort: high — the migration is irreversible"));
    }

    #[test]
    fn a_card_without_an_effort_says_nothing_about_one() {
        let card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        let rendered = CompactionReinjectionService::render_additional_context_text(&card)
            .expect("render text");
        assert!(!rendered.contains("Effort:"));
    }

    #[test]
    fn an_effort_without_a_reason_still_states_the_level() {
        let mut snapshot = sample_snapshot();
        snapshot.assignment_footer.task_effort = "medium".to_string();

        let card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &snapshot,
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );

        let rendered = CompactionReinjectionService::render_additional_context_text(&card)
            .expect("render text");
        assert!(rendered.contains("Effort: medium"));
        assert!(!rendered.contains("Effort: medium —"));
    }

    #[test]
    fn render_additional_context_text_handles_sparse_cards() {
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
        card.role.context_summary = None;
        card.role.behavior_summary = None;
        card.role.runtime_compact_summary = None;
        card.task.execution_mode.clear();
        card.task.validation_expectation.clear();
        card.task.response_expectation.clear();
        card.boundaries.file_ownership_boundary.clear();
        card.boundaries.override_allowed = true;
        card.boundaries.active_override_reason = Some("lead-approved adjacent fix".to_string());
        card.working_set.focal_files.clear();

        let rendered = CompactionReinjectionService::render_additional_context_text(&card)
            .expect("render text");

        assert!(!rendered.contains("Role:"));
        assert!(!rendered.contains("Execution mode:"));
        assert!(!rendered.contains("Validation expectation:"));
        assert!(!rendered.contains("Response expectation:"));
        assert!(rendered.contains("Focal files: Use the current task context if these are empty."));
        assert!(rendered.contains("File ownership boundary: No explicit file boundary recorded."));
        assert!(rendered.contains("Override allowed: yes"));
        assert!(rendered.contains("Active override reason: lead-approved adjacent fix"));
    }

    #[test]
    fn render_additional_context_text_preserves_role_id_when_role_name_is_missing() {
        let mut card = CompactionReinjectionService::compose_at(
            &sample_member(),
            &sample_snapshot(),
            DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
                .expect("timestamp")
                .with_timezone(&Utc),
        );
        card.role.role_name = None;

        let rendered = CompactionReinjectionService::render_additional_context_text(&card)
            .expect("render text");

        assert!(rendered.contains("Role: taurhaus-architect"));
    }

    #[test]
    fn representative_taurhaus_roles_render_materially_different_compaction_cards() {
        let snapshot = sample_snapshot();
        let generated_at = DateTime::parse_from_rfc3339("2026-03-08T14:10:05Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        let developer_role = load_role_template("taurhaus-developer");
        let architect_role = load_role_template("taurhaus-architect");
        let lead_role = load_role_template("taurhaus-lead-codex");

        let developer_rendered = CompactionReinjectionService::render_additional_context_text(
            &CompactionReinjectionService::compose_at(
                &member_from_role(&developer_role),
                &snapshot,
                generated_at,
            ),
        )
        .expect("render developer");
        let architect_rendered = CompactionReinjectionService::render_additional_context_text(
            &CompactionReinjectionService::compose_at(
                &member_from_role(&architect_role),
                &snapshot,
                generated_at,
            ),
        )
        .expect("render architect");
        let lead_rendered = CompactionReinjectionService::render_additional_context_text(
            &CompactionReinjectionService::compose_at(
                &member_from_role(&lead_role),
                &snapshot,
                generated_at,
            ),
        )
        .expect("render lead");

        assert_ne!(developer_rendered, architect_rendered);
        assert_ne!(developer_rendered, lead_rendered);
        assert_ne!(architect_rendered, lead_rendered);

        assert!(developer_rendered.contains(
            "Role purpose: Keep scoped implementation and validation discipline intact after compaction."
        ));
        assert!(architect_rendered.contains(
            "Role purpose: Preserve cross-layer diagnosis and boundary clarity after compaction."
        ));
        assert!(lead_rendered.contains(
            "Role purpose: Preserve explicit task protocol and routing discipline after compaction."
        ));
    }

    #[test]
    fn snapshot_has_resumable_task_requires_active_task_status_and_identity() {
        let mut snapshot = sample_snapshot();

        assert!(CompactionReinjectionService::snapshot_has_resumable_task(
            &snapshot
        ));

        snapshot.task.status = "pending".to_string();
        assert!(CompactionReinjectionService::snapshot_has_resumable_task(
            &snapshot
        ));

        snapshot.task.status = "completed".to_string();
        assert!(!CompactionReinjectionService::snapshot_has_resumable_task(
            &snapshot
        ));

        snapshot.task.status = "deleted".to_string();
        assert!(!CompactionReinjectionService::snapshot_has_resumable_task(
            &snapshot
        ));

        snapshot.task.status = "in_progress".to_string();
        snapshot.task.id.clear();
        assert!(!CompactionReinjectionService::snapshot_has_resumable_task(
            &snapshot
        ));
    }
}
