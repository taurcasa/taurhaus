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
    pub card_key: CardKey,
    pub content_revision: String,
    pub kind: DeliveryKind,
    pub supersedes_revision: Option<String>,
    pub attempt: u8,
    pub path: String,
    pub stage: ReceiptStage,
    pub generated_bytes: usize,
    pub accepted_bytes: usize,
}

/// App-authored runtime fields: one baseline binding and one delivered pointer.
/// No per-revision ledger, timer, or coalescing queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryState {
    pub member_incarnation_id: Option<String>,
    pub activation_intent: Option<String>,
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
        self.claim = None;
    }

    pub fn claim(&mut self, key: &CardKey, revision: &str, path: &str) -> Option<CardReceipt> {
        let obligation = key.obligation_key();
        if let Some(last) = &self.last_delivered {
            if last.card_key == *key {
                return None;
            }
        }
        let kind = if self.baseline_binding.as_ref() == Some(&obligation) {
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
        };
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
        receipt.stage = stage;
        receipt.generated_bytes = bytes;
        receipt.accepted_bytes = if stage == ReceiptStage::Accepted {
            bytes
        } else {
            0
        };
        if stage.satisfies() {
            self.baseline_binding = Some(receipt.obligation_key.clone());
            self.last_delivered = Some(receipt.clone());
        }
        self.claim = Some(receipt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
