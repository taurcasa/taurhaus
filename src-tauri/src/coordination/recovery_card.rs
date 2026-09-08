//! Versioned recovery cards; storage and transports retain their existing owners.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CARD_SCHEMA: u32 = 1;

/// Hash a canonical typed value, never timestamps or rendered whitespace.
pub fn digest(value: &impl Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("card values serialize"))
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roots {
    pub root_authority_revision: String,
    pub resolved_teams_root: String,
    pub resolved_mesh_config_root: String,
    pub harness_account_root: String,
    pub launch_namespace: String,
    pub resolved_project_root: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contract {
    pub role_id: String,
    pub effective_role_revision: String,
    pub instruction_contract_revision: String,
    pub packet_revision: String,
}

pub type ObligationKey = ((String, String), (u64, u64));

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardKey {
    pub card_schema: u32,
    pub recipient: (String, String),
    pub context: (u64, u64),
    pub roots: Roots,
    pub contract: Contract,
}

impl CardKey {
    pub fn obligation_key(&self) -> ObligationKey {
        (self.recipient.clone(), self.context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryKind {
    Baseline,
    Correction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStage {
    Accepted,
    HookResponseOffered,
    Submitted,
    ConsumedByRead,
    OutcomeUnknown,
    Failed,
}

impl ReceiptStage {
    pub fn satisfies(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::HookResponseOffered | Self::Submitted | Self::ConsumedByRead
        )
    }
}

/// An observation, not a claim of comprehension. The inbox also retains this
/// envelope, allowing reconciliation after append but before runtime commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardReceipt {
    pub delivery_id: String,
    pub obligation_key: ObligationKey,
    #[serde(default)]
    pub satisfied_obligations: Vec<ObligationKey>,
    pub card_key: CardKey,
    pub content_revision: String,
    pub kind: DeliveryKind,
    pub supersedes_revision: Option<String>,
    pub attempt: u8,
    pub path: String,
    pub stage: ReceiptStage,
    pub generated_bytes: usize,
    pub accepted_bytes: usize,
    #[serde(default)]
    pub offered_bytes: usize,
    #[serde(default)]
    pub returned_by_read_bytes: usize,
    #[serde(default)]
    pub observations: Vec<(ReceiptStage, chrono::DateTime<chrono::Utc>, String)>,
}

impl CardReceipt {
    pub fn record(&mut self, stage: ReceiptStage, bytes: usize) {
        self.stage = stage;
        self.generated_bytes = bytes;
        match stage {
            ReceiptStage::Accepted => self.accepted_bytes = bytes,
            ReceiptStage::HookResponseOffered | ReceiptStage::Submitted => {
                self.offered_bytes = bytes
            }
            ReceiptStage::ConsumedByRead => self.returned_by_read_bytes = bytes,
            _ => {}
        }
        if !self
            .observations
            .iter()
            .any(|(observed, _, _)| *observed == stage)
        {
            self.observations
                .push((stage, chrono::Utc::now(), self.path.clone()));
        }
    }
}

/// App-authored runtime fields: one baseline binding and one delivered pointer.
/// No per-revision ledger, timer, or coalescing queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryState {
    pub member_incarnation_id: Option<String>,
    #[serde(default)]
    pub harness_account_root: Option<String>,
    #[serde(default)]
    pub launch_namespace: Option<String>,
    pub activation_intent: Option<String>,
    #[serde(default)]
    pub reserved_attachment: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub reserved_effort: Option<String>,
    pub activation_generation: u64,
    pub compaction_generation: u64,
    pub admitted_boundary: Option<String>,
    pub baseline_binding: Option<ObligationKey>,
    pub claim: Option<CardReceipt>,
    pub last_delivered: Option<CardReceipt>,
}

impl RecoveryState {
    pub fn context(&self) -> (u64, u64) {
        (self.activation_generation, self.compaction_generation)
    }

    /// Called by the activation owner only, after capture and before delivery.
    pub fn reserve_activation(&mut self, intent: &str) {
        if self.activation_intent.as_deref() == Some(intent) {
            return;
        }
        self.member_incarnation_id
            .get_or_insert_with(|| uuid::Uuid::new_v4().to_string());
        self.activation_intent = Some(intent.to_string());
        self.activation_generation += 1;
        self.compaction_generation = 0;
        self.admitted_boundary = None;
        self.baseline_binding = None;
        self.claim = None;
    }

    /// Metadata only; the existing hook path decides which boundary is admitted.
    pub fn admit_compaction(&mut self, boundary: &str) {
        if self.admitted_boundary.as_deref() == Some(boundary) {
            return;
        }
        self.admitted_boundary = Some(boundary.to_string());
        self.compaction_generation += 1;
        self.baseline_binding = None;
        if self
            .claim
            .as_ref()
            .is_none_or(|r| r.card_key.context != self.context())
        {
            self.claim = None;
        } else {
            self.baseline_binding = self.claim.as_ref().map(|r| r.obligation_key.clone());
        }
    }

    pub fn claim(&mut self, key: &CardKey, revision: &str, path: &str) -> Option<CardReceipt> {
        let obligation = key.obligation_key();
        if let Some(last) = &self.last_delivered {
            if last.card_key == *key {
                return None;
            }
        }
        let kind = if let Some(previous) = self.claim.as_ref().filter(|r| r.card_key == *key) {
            previous.kind
        } else if self.baseline_binding.as_ref() == Some(&obligation) {
            DeliveryKind::Correction
        } else {
            DeliveryKind::Baseline
        };
        let id = digest(&(key, kind));
        let mut attempt = 1;
        if let Some(previous) = &self.claim {
            if previous.delivery_id == id {
                if previous.stage != ReceiptStage::Failed
                    || previous.attempt >= 2
                    || previous.path != path
                {
                    return None;
                }
                attempt = previous.attempt + 1;
            }
        }
        let receipt = CardReceipt {
            delivery_id: id,
            satisfied_obligations: vec![obligation.clone()],
            obligation_key: obligation,
            card_key: key.clone(),
            content_revision: revision.into(),
            kind,
            supersedes_revision: (kind == DeliveryKind::Correction)
                .then(|| {
                    self.last_delivered
                        .as_ref()
                        .map(|r| r.content_revision.clone())
                })
                .flatten(),
            attempt,
            path: path.into(),
            // Persist before offering: an interrupted offer is ambiguous.
            stage: ReceiptStage::OutcomeUnknown,
            generated_bytes: 0,
            accepted_bytes: 0,
            offered_bytes: 0,
            returned_by_read_bytes: 0,
            observations: Vec::new(),
        };
        self.baseline_binding = Some(receipt.obligation_key.clone());
        self.claim = Some(receipt.clone());
        Some(receipt)
    }

    pub fn observe(&mut self, receipt: &CardReceipt, stage: ReceiptStage, bytes: usize) {
        if self.claim.as_ref().map(|r| (&r.delivery_id, r.attempt))
            != Some((&receipt.delivery_id, receipt.attempt))
        {
            return; // Stale pre-move or previous-context receipt cannot satisfy this claim.
        }
        let mut receipt = receipt.clone();
        receipt.record(stage, bytes);
        if stage.satisfies() {
            self.baseline_binding = Some(receipt.obligation_key.clone());
            self.last_delivered = Some(receipt.clone());
        }
        self.claim = Some(receipt);
    }
}

/// Bounded against the complete bundled-role required-fact fixture.
pub const STEERING_BYTE_CAP: usize = 4_096;
pub const CARD_BYTE_CAP: usize = 8_192;
pub const FIRST_ACTION: &str = "work_contract.first_action: Execute the first action in the delivered current assignment; record a real dependency wait when execution cannot begin. Report through its completion signal; no pure acknowledgment.";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignmentFacts {
    pub task_id: String,
    pub assignment_token: String,
    pub stage_id: String,
    pub source_revision: String,
    pub audience_policy_revision: String,
    pub state: String,
    pub owner: String,
    pub objective: String,
    pub deliverable: String,
    pub first_action: String,
    pub completion_signal: String,
    pub review_route: String,
    pub wait: String,
    pub candidate_ref: String,
    pub rubric_ref: String,
    pub packet_revision: String,
    pub restart_cursor_ref: String,
}

/// App-owned opaque descriptor; Mesh need not parse a new wire vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardDescriptor {
    /// This descriptor created the snapshot before any operational publisher.
    #[serde(default)]
    pub descriptor_only: bool,
    pub card_schema: u32,
    pub card_key: Option<CardKey>,
    pub content_revision: String,
    pub pending: bool,
}

/// One compiler input/output for onboarding, compaction and explicit recovery.
/// Evidence is references only; private peer prose never enters this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCard {
    pub card_key: Option<CardKey>,
    pub content_revision: String,
    pub team_name: String,
    pub member_name: String,
    pub project_path: String,
    pub steering: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub constraints: String,
    pub lease_context: String,
    pub assignment: AssignmentFacts,
    pub requested_effort: String,
    pub effective_effort: String,
    pub effort_hold: String,
    pub focal_files: Vec<String>,
    pub boundaries: Vec<String>,
}

pub fn render_card_steering(member: &crate::coordination::domain::Member) -> String {
    let mut summary = member.runtime_compact_summary.clone();
    if summary.is_none() {
        let mut authorized = Vec::new();
        for text in [&member.instructions, &member.communication_style]
            .into_iter()
            .flatten()
        {
            if !text.trim().is_empty() {
                authorized.push(text.clone());
            }
        }
        if let Some(contract) = &member.behavioral_contract {
            authorized.extend(
                contract
                    .communication
                    .iter()
                    .chain(&contract.execution)
                    .chain(&contract.escalation)
                    .filter(|s| !s.trim().is_empty())
                    .cloned(),
            );
        }
        if !authorized.is_empty() {
            summary = Some(crate::templates::types::RuntimeCompactSummary {
                role_purpose: authorized.remove(0),
                keep_doing: authorized,
                workflow_sequence: Vec::new(),
                avoid: Vec::new(),
                escalate_when: Vec::new(),
            });
        }
    }
    steering(
        &member.role_id,
        &summary,
        &member.quality_gates,
        &member.handoff_expectations,
        &member.definition_of_done,
    )
}

fn steering(
    role_id: &Option<String>,
    summary: &Option<crate::templates::types::RuntimeCompactSummary>,
    gates: &Option<Vec<String>>,
    handoff: &Option<Vec<String>>,
    done: &Option<Vec<String>>,
) -> String {
    let mut lines = vec![format!(
        "Role: {}",
        role_id.as_deref().unwrap_or("unavailable")
    )];
    if let Some(summary) = summary {
        lines.push(format!("Purpose: {}", summary.role_purpose));
        for (title, facts) in [
            ("Boundary", &summary.keep_doing),
            ("Sequence", &summary.workflow_sequence),
            ("Constraint", &summary.avoid),
            ("Escalation", &summary.escalate_when),
        ] {
            lines.extend(facts.iter().map(|fact| format!("{title}: {fact}")));
        }
    } else {
        lines.push("HOLD: minimal role steering unavailable; owner: team lead.".into());
    }
    for (title, facts) in [("Gate", gates), ("Handoff", handoff), ("Completion", done)] {
        lines.extend(
            facts
                .iter()
                .flatten()
                .map(|fact| format!("{title}: {fact}")),
        );
    }
    let text = lines.join("\n");
    if text.len() > STEERING_BYTE_CAP {
        "HOLD: role steering exceeds the byte budget; owner: team lead; provide a bounded authorized role revision.".into()
    } else {
        text
    }
}

impl RecoveryCard {
    pub fn compile(
        team_name: &str,
        member: &crate::coordination::domain::Member,
        snapshot: Option<&crate::coordination::stores::OperationalContextSnapshot>,
        card_key: Option<CardKey>,
        mut assignment: AssignmentFacts,
    ) -> Self {
        if assignment.state.is_empty() {
            assignment.state = snapshot
                .map(|s| s.task.status.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unassigned".into());
        }
        if let Some(snapshot) = snapshot {
            if assignment.task_id.is_empty() {
                assignment.task_id = snapshot.task.id.clone();
            }
            if assignment.objective.is_empty() {
                assignment.objective = snapshot.task.subject.clone();
            }
        }
        // The operative digest excludes updated_at and deadline/presence markers.
        let content_revision = digest(&(
            &card_key,
            &assignment.assignment_token,
            &assignment.stage_id,
            &assignment.source_revision,
            &assignment.audience_policy_revision,
        ));
        Self {
            card_key,
            content_revision,
            team_name: team_name.into(),
            member_name: member.name.clone(),
            project_path: member.project_path.to_string_lossy().into_owned(),
            steering: render_card_steering(member),
            generated_at: chrono::Utc::now(), lease_context: String::new(),
            constraints: snapshot.map(|s| format!("Execution mode: {}; Validation expectation: {}; Response expectation: {}; Adjacent fix policy: {}; Override allowed: {}; Override reason: {}",s.assignment_footer.execution_mode,s.assignment_footer.validation_expectation,s.assignment_footer.response_expectation,s.assignment_footer.adjacent_fix_policy,s.ownership.override_allowed,s.ownership.active_override_reason.as_deref().unwrap_or("none"))).unwrap_or_else(|| "Operational constraints: unavailable".into()),
            assignment,
            effective_effort: String::new(), effort_hold: String::new(), focal_files: snapshot.map(|s| s.working_set.focal_files.clone()).unwrap_or_default(),
            requested_effort: snapshot
                .map(|s| s.assignment_footer.task_effort.clone())
                .unwrap_or_default(),
            boundaries: snapshot
                .map(|s| s.assignment_footer.file_ownership_boundary.clone())
                .unwrap_or_default(),
        }
    }

    pub fn from_reinjection(
        card: &crate::coordination::reinjection::OperationalReinjectionCard,
    ) -> Self {
        Self {
            card_key: None, content_revision: digest(&(&card.task,&card.boundaries,&card.working_set)),
            team_name: card.team_name.clone(), member_name: card.member_name.clone(), project_path: card.working_set.project_path.clone(),
            generated_at: card.generated_at,
            steering: steering(&card.role.role_id,&card.role.runtime_compact_summary,&Some(card.role.quality_gates.clone()),&Some(card.role.handoff_expectations.clone()),&Some(card.role.definition_of_done.clone())),
            assignment: AssignmentFacts { task_id: card.task.id.clone(), objective: card.task.subject.clone(), state: card.task.status.clone(), wait: "release_unavailable".into(), ..Default::default() },
            effective_effort: String::new(), effort_hold: String::new(), focal_files: card.working_set.focal_files.clone(),
            requested_effort: card.task.effort.clone(), boundaries: card.boundaries.file_ownership_boundary.clone(),
            constraints: format!("Execution mode: {}; Validation expectation: {}; Response expectation: {}; Adjacent fix policy: {}; Override allowed: {}; Active override reason: {}; Effort rationale: {}", card.task.execution_mode,card.task.validation_expectation,card.task.response_expectation,card.boundaries.adjacent_fix_policy,card.boundaries.override_allowed,card.boundaries.active_override_reason.as_deref().unwrap_or("none"),card.task.effort_why),
            lease_context: crate::coordination::reinjection::render_lease_context_line(&card.leases).unwrap_or_default(),
        }
    }

    pub fn render(&self) -> String {
        let unavailable =
            |text: &str| if text.is_empty() { "unavailable" } else { text }.to_string();
        let a = &self.assignment;
        let key = self
            .card_key
            .as_ref()
            .map(|k| serde_json::to_string(k).expect("card key"))
            .unwrap_or_else(|| "generation_unknown".into());
        let mut lines = vec![
            "[taurhaus] recovery_card".into(),
            format!(
                "Identity: {} on {}; key={key}",
                self.member_name, self.team_name
            ),
            format!(
                "Coverage: {}; source={}; audience={}",
                self.content_revision,
                unavailable(&a.source_revision),
                unavailable(&a.audience_policy_revision)
            ),
            format!("Project cwd: {}", self.project_path),
            format!(
                "Assignment: task={} token={} stage={} owner={} state={}",
                unavailable(&a.task_id),
                unavailable(&a.assignment_token),
                unavailable(&a.stage_id),
                unavailable(&a.owner),
                a.state
            ),
        ];
        for (title, fact) in [
            ("Objective", &a.objective),
            ("Deliverable", &a.deliverable),
            ("First action", &a.first_action),
            ("Completion signal", &a.completion_signal),
            ("Review route", &a.review_route),
            ("Wait/release", &a.wait),
            ("Candidate", &a.candidate_ref),
            ("Rubric", &a.rubric_ref),
            ("Restart cursor", &a.restart_cursor_ref),
        ] {
            lines.push(format!("{title}: {}", unavailable(fact)));
        }
        lines.push(format!(
            "Requested effort: {}; effective effort: {}; hold: {}",
            unavailable(&self.requested_effort),
            unavailable(&self.effective_effort),
            unavailable(&self.effort_hold)
        ));
        lines.push(format!(
            "File boundary: {}",
            unavailable(&self.boundaries.join(", "))
        ));
        lines.push("Evidence/handoff retention: unavailable; references do not prove archival preservation.".into());
        lines.push(format!("Generated: {}", self.generated_at));
        lines.push(format!("Current task: #{} — {}", a.task_id, a.objective));
        lines.push(self.constraints.clone());
        lines.push(format!(
            "Focal files: {}",
            unavailable(&self.focal_files.join(", "))
        ));
        if !self.lease_context.is_empty() {
            lines.push(self.lease_context.clone());
        }
        lines.push(self.steering.clone());
        lines.push(FIRST_ACTION.into());
        lines.push("Corrections replace only named instructions; reminders cannot release GO. Ordinary assignments require no card fetch.".into());
        let executable = matches!(a.state.as_str(), "pending" | "in_progress")
            && !a.assignment_token.is_empty()
            && a.wait == "released"
            && !a.first_action.is_empty()
            && !self.steering.contains("HOLD:");
        lines.push(if executable { format!("Next action: {}", a.first_action) } else { "Next action: preserve the stated wait or terminal/unassigned state; ask the team lead for any missing identity, release, or required context.".into() });
        // Focal links and lease context are optional; reserve all operative facts first.
        if lines.join("\n").len() > CARD_BYTE_CAP {
            lines.retain(|line| !line.starts_with("Focal files:") && line != &self.lease_context);
            lines.insert(
                1,
                "Optional focal files/leases omitted to fit the card budget.".into(),
            );
        }
        if lines.join("\n").len() > CARD_BYTE_CAP {
            *lines.last_mut().expect("next action") = "Next action: preserve the stated wait; ask the team lead for the oversized field group.".into();
        }
        while lines.join("\n").len() > CARD_BYTE_CAP {
            // Retain identity, assignment and wait before other required groups. A
            // group that cannot fit becomes an explicit unavailable-field hold.
            // There are fewer than 32 groups, so overflow always has a >256-byte group.
            let (index, line) = lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.len() > 256)
                .max_by_key(|(_, line)| {
                    let reserved = ["Identity:", "Assignment:", "Wait/release:", "Project cwd:"]
                        .iter()
                        .any(|prefix| line.starts_with(prefix));
                    (!reserved, line.len())
                })
                .expect("oversized card has an oversized field group");
            let group = if line == &self.steering {
                "Role steering"
            } else if line == &self.constraints {
                "Operational constraints"
            } else {
                line.split_once(':')
                    .map(|(title, _)| title)
                    .unwrap_or("Required context")
            };
            lines[index] = format!("HOLD: {group} unavailable because it exceeds the remaining byte budget; owner: team lead; request a bounded authorized replacement.");
        }
        lines.join("\n")
    }
}

#[cfg(test)]
pub(crate) fn assert_control_golden(text: &str) {
    let control = text
        .lines()
        .filter(|line| {
            *line == "[taurhaus] recovery_card"
                || line.starts_with("work_contract.first_action:")
                || line.starts_with("Corrections replace")
                || line.starts_with("Next action:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        control,
        "[taurhaus] recovery_card\nwork_contract.first_action: Execute the first action in the delivered current assignment; record a real dependency wait when execution cannot begin. Report through its completion signal; no pure acknowledgment.\nCorrections replace only named instructions; reminders cannot release GO. Ordinary assignments require no card fetch.\nNext action: preserve the stated wait or terminal/unassigned state; ask the team lead for any missing identity, release, or required context."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_custom_role_keeps_authorized_steering_without_compact_summary() {
        // Regression: 2760e88a discarded custom role instructions without a compact summary.
        let member = serde_json::from_value(serde_json::json!({
            "name":"seat","role":"agent","project_path":"/scratch","cli_tool":"codex",
            "instructions":"Inspect the assigned diff.","communication_style":"Report evidence.",
            "behavioral_contract":{"communication":["Name the commit."],"execution":["Run the tests."],"escalation":["Ask the lead about missing requirements."]}
        })).unwrap();
        let text = render_card_steering(&member);
        for required in [
            "Inspect the assigned diff.",
            "Report evidence.",
            "Name the commit.",
            "Run the tests.",
            "Ask the lead about missing requirements.",
        ] {
            assert!(text.contains(required), "missing {required}: {text}");
        }
        assert!(!text.contains("HOLD:"));
        assert!(text.len() <= STEERING_BYTE_CAP);
    }

    #[test]
    fn recovery_optional_overflow_preserves_identity_assignment_and_next_action() {
        // Regression: 2760e88a discarded the entire reserved block on optional-list overflow.
        let member = serde_json::from_value(serde_json::json!({"name":"seat","role":"agent","project_path":"/scratch","cli_tool":"codex"})).unwrap();
        let facts = AssignmentFacts {
            task_id: "1".into(),
            owner: "seat".into(),
            assignment_token: "a1".into(),
            state: "in_progress".into(),
            wait: "released".into(),
            first_action: "Run the review".into(),
            ..Default::default()
        };
        let mut card = RecoveryCard::compile(
            "team",
            &member,
            None,
            Some(key(&RecoveryState::default())),
            facts,
        );
        card.steering = "s".repeat(STEERING_BYTE_CAP);
        card.focal_files = (0..120)
            .map(|i| format!("src/components/recovery/long_component_name_{i}.svelte"))
            .collect();
        card.lease_context = "lease ".repeat(1500);
        card.boundaries = (0..12)
            .map(|i| format!("src/coordination/recovery/module_{i}.rs"))
            .collect();
        let text = card.render();
        assert!(text.len() <= CARD_BYTE_CAP);
        assert!(text.contains("Identity: seat on team; key="));
        assert!(text.contains("Assignment: task=1 token=a1"));
        assert!(text.contains("Wait/release: released"));
        assert!(text.contains("Corrections replace only named instructions"));
        assert!(text.ends_with("Next action: Run the review"));
        assert!(text.contains("File boundary: src/coordination/recovery/module_0.rs"));

        // Regression: 2760e88a lost the machine identity even when only the boundary was oversized.
        card.boundaries = vec!["large-boundary/".repeat(CARD_BYTE_CAP)];
        let text = card.render();
        assert!(text.len() <= CARD_BYTE_CAP);
        assert!(text.contains("Identity: seat on team; key="));
        assert!(text.contains("Assignment: task=1 token=a1"));
        assert!(text.contains("Wait/release: released"));
        assert!(text.contains("HOLD: File boundary"));
        assert!(!text.ends_with("Next action: Run the review"));
    }

    fn key(state: &RecoveryState) -> CardKey {
        CardKey {
            card_schema: 1,
            recipient: ("team-incarnation".into(), "member-incarnation".into()),
            context: state.context(),
            roots: Roots::default(),
            contract: Contract::default(),
        }
    }

    #[test]
    fn recovery_baseline_once_across_activation_entrypoints_and_worker_restart() {
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment-1");
        for entrypoint in ["init", "add", "resume", "reonboard", "worker-restart"] {
            let k = key(&state);
            let claim = state.claim(&k, "view-1", "inbox");
            if entrypoint == "init" {
                let claim = claim.unwrap();
                state.observe(&claim, ReceiptStage::Accepted, 123);
            } else {
                assert!(claim.is_none(), "{entrypoint} replayed a baseline");
            }
            state = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        }
    }

    #[test]
    fn recovery_forced_intent_and_preserved_session_relaunch_reserve_once() {
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment-1");
        let first = key(&state);
        state.reserve_activation("force:operator-intent-1");
        let forced = key(&state);
        assert_ne!(first, forced);
        state.reserve_activation("force:operator-intent-1");
        assert_eq!(forced, key(&state));
        state.reserve_activation("attachment-2-same-session");
        assert_ne!(forced, key(&state));
    }

    #[test]
    fn recovery_skipped_compaction_pending_is_satisfied_by_next_receipt() {
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment-1");
        state.admit_compaction("boundary-1");
        let pending = key(&state);
        state.admit_compaction("boundary-1");
        assert_eq!(pending, key(&state));
        let claim = state.claim(&pending, "assignment-view", "inbox").unwrap();
        state.observe(&claim, ReceiptStage::Accepted, 123);
        assert!(state.claim(&pending, "assignment-view", "inbox").is_none());
        assert_eq!(state.last_delivered.unwrap().stage, ReceiptStage::Accepted);
    }

    #[test]
    fn recovery_view_recomposition_keeps_delivery_id_and_retry_budget() {
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment-1");
        let k = key(&state);
        let first = state.claim(&k, "assignment-a", "inbox").unwrap();
        state.observe(&first, ReceiptStage::Failed, 0);
        let second = state.claim(&k, "assignment-b", "inbox").unwrap();
        assert_eq!(first.delivery_id, second.delivery_id);
        assert_ne!(first.content_revision, second.content_revision);
        assert_eq!(second.attempt, 2);
        state.observe(&second, ReceiptStage::Failed, 0);
        assert!(state.claim(&k, "assignment-c", "inbox").is_none());
    }

    #[test]
    fn recovery_unknown_hook_outcome_never_falls_back_to_inbox() {
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment-1");
        let k = key(&state);
        let claim = state.claim(&k, "view", "hook_stdout").unwrap();
        state.observe(&claim, ReceiptStage::OutcomeUnknown, 123);
        assert!(state.claim(&k, "view", "inbox").is_none());
        assert!(state.last_delivered.is_none());
    }

    #[test]
    fn recovery_latest_root_correction_names_single_delivered_predecessor() {
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment-1");
        let mut k = key(&state);
        let baseline = state.claim(&k, "view", "inbox").unwrap();
        state.observe(&baseline, ReceiptStage::Accepted, 100);
        k.roots.resolved_teams_root = "/new-root".into();
        k.contract.effective_role_revision = "latest-role".into();
        let correction = state.claim(&k, "latest-view", "inbox").unwrap();
        assert_eq!(correction.kind, DeliveryKind::Correction);
        assert_eq!(
            correction.supersedes_revision,
            Some(baseline.content_revision)
        );
        assert_ne!(correction.delivery_id, baseline.delivery_id);
        assert_eq!(correction.obligation_key, baseline.obligation_key);
    }
    #[test]
    fn recovery_renderer_holds_unassigned_waiting_and_terminal_contexts() {
        // Regression: 9b857060a ended every recovered context with unconditional continuation.
        for status in ["unassigned", "awaiting_go", "completed", "blocked"] {
            let member: crate::coordination::domain::Member =
                serde_json::from_value(serde_json::json!({
                    "name":"seat", "role":"agent", "project_path":"/scratch", "cli_tool":"codex"
                }))
                .unwrap();
            let facts = AssignmentFacts {
                state: status.into(),
                ..Default::default()
            };
            let card = RecoveryCard::compile("team", &member, None, None, facts);
            let text = card.render();
            assert!(text.contains(status));
            assert!(!text.contains("continue immediately"));
            assert!(text.contains("Next action:"));
        }
    }

    #[test]
    fn recovery_steering_golden_and_bundled_required_fact_coverage() {
        use crate::coordination::domain::Member;
        let mut member: Member = serde_json::from_value(serde_json::json!({
            "name":"seat", "role":"agent", "project_path":"/scratch", "cli_tool":"codex",
            "role_id":"reviewer", "focus_area":"Independent review",
            "runtime_compact_summary": {"rolePurpose":"Judge the candidate", "keepDoing":["Review only"],
                "workflowSequence":[],"avoid":["Peer verdicts"],"escalateWhen":["Missing candidate"]},
            "definition_of_done":["Return findings"]
        })).unwrap();
        assert_eq!(render_card_steering(&member), "Role: reviewer\nPurpose: Judge the candidate\nBoundary: Review only\nConstraint: Peer verdicts\nEscalation: Missing candidate\nCompletion: Return findings");
        let mut count = 0;
        let mut maximum = 0;
        for entry in std::fs::read_dir(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/templates/roles"
        ))
        .unwrap()
        {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let role: crate::templates::types::RoleTemplate =
                serde_norway::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            member.role_id = Some(role.role_id);
            member.runtime_compact_summary = role.runtime_compact_summary;
            member.definition_of_done = role.definition_of_done;
            member.quality_gates = role.quality_gates;
            member.handoff_expectations = role.handoff_expectations;
            let text = render_card_steering(&member);
            assert!(
                !text.contains("HOLD"),
                "{}",
                member.role_id.as_deref().unwrap()
            );
            if let Some(summary) = &member.runtime_compact_summary {
                assert!(text.contains(&summary.role_purpose));
                for fact in summary
                    .keep_doing
                    .iter()
                    .chain(&summary.workflow_sequence)
                    .chain(&summary.avoid)
                    .chain(&summary.escalate_when)
                {
                    assert!(text.contains(fact), "missing required fact: {fact}");
                }
            }
            maximum = maximum.max(text.len());
            count += 1;
        }
        assert!(count > 20);
        eprintln!("bundled steering: {count} roles, maximum {maximum} UTF-8 bytes");
        assert!(maximum <= STEERING_BYTE_CAP);
    }
    #[test]
    fn recovery_root_change_after_claim_cannot_open_a_second_baseline() {
        // Regression: ff287130 opened another baseline when roots changed before acceptance.
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment");
        let mut k = key(&state);
        state.claim(&k, "first", "inbox").unwrap();
        k.roots.resolved_teams_root = "/moved".into();
        assert_eq!(
            state.claim(&k, "moved", "inbox").unwrap().kind,
            DeliveryKind::Correction
        );
    }
    #[test]
    fn recovery_missing_required_steering_holds_even_a_released_assignment() {
        // Regression: 2760e88a allowed executable next action despite missing required steering.
        let member=serde_json::from_value(serde_json::json!({"name":"seat","role":"agent","cli_tool":"codex","project_path":"/scratch"})).unwrap();
        let facts = AssignmentFacts {
            state: "in_progress".into(),
            assignment_token: "a1".into(),
            wait: "released".into(),
            first_action: "edit now".into(),
            ..Default::default()
        };
        let card = RecoveryCard::compile("team", &member, None, None, facts);
        assert!(card.render().contains("HOLD"));
        assert!(!card.render().contains("Next action: edit now"));
    }
    #[test]
    fn recovery_hook_admission_retains_claimed_baseline_binding() {
        // Regression: f0a5bad7 reset the preview claim's binding while admitting its boundary.
        let mut state = RecoveryState::default();
        state.reserve_activation("attachment");
        let mut preview = key(&state);
        preview.context.1 += 1;
        state.claim(&preview, "hook-view", "hook_stdout").unwrap();
        state.admit_compaction("boundary");
        preview.contract.packet_revision = "changed".into();
        assert_eq!(
            state
                .claim(&preview, "replacement", "hook_stdout")
                .unwrap()
                .kind,
            DeliveryKind::Correction
        );
    }
}
