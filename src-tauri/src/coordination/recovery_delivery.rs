//! Claims on existing runtime records, with existing inbox receipts as recovery evidence.

use crate::coordination::errors::CoordinationError;
use crate::coordination::recovery_card::{
    digest, AssignmentFacts, CardKey, CardReceipt, Contract, DeliveryKind, ReceiptStage,
    RecoveryCard, Roots, CARD_SCHEMA, FIRST_ACTION,
};
use crate::coordination::stores::{
    MemberRuntimeStore, MeshInboxStore, OperationalContextSnapshotStore, TeamConfigStore,
    TeamRootRegistry,
};
use std::path::Path;

pub struct PreparedCard {
    pub receipt: CardReceipt,
    pub text: String,
}

pub fn reserve_activation(
    root: &Path,
    team: &str,
    member: &str,
    intent: &str,
) -> Result<(), CoordinationError> {
    crate::coordination::validation::validate_team_name(team)?;
    crate::coordination::validation::validate_member_name(member)?;
    MemberRuntimeStore::update(root, team, member, |record| {
        record.recovery.reserve_activation(intent)
    })?;
    Ok(())
}

fn validate_root(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
) -> Result<(), CoordinationError> {
    crate::coordination::validation::validate_team_name(team)?;
    if !crate::coordination::stores::team_roots::same_teams_root(&registry.resolve(team)?, root) {
        return Err(CoordinationError::Conflict(
            "recovery root authority changed".into(),
        ));
    }
    Ok(())
}

pub fn prepare(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
    member_name: &str,
    path: &str,
) -> Result<Option<PreparedCard>, CoordinationError> {
    validate_root(registry, root, team)?;
    crate::coordination::validation::validate_member_name(member_name)?;
    let guard = crate::coordination::stores::lock::acquire_team_lock(root, team)?;
    validate_root(registry, root, team)?;
    let config = TeamConfigStore::load(root, team)?;
    let member = config
        .members
        .iter()
        .find(|m| m.name == member_name)
        .ok_or_else(|| CoordinationError::NotFound("recovery member not found".into()))?;
    let mut runtime = MemberRuntimeStore::load(root, team, member_name)?;
    let snapshot = OperationalContextSnapshotStore::load(root, team, member_name)?;
    let facts = assignment_facts(root, team, member_name, snapshot.as_ref());
    let normalize = |p: &Path| crate::provider::path::normalize_project_path(&p.to_string_lossy());
    let key = config
        .team_incarnation_id
        .as_ref()
        .zip(runtime.recovery.member_incarnation_id.as_ref())
        .filter(|_| runtime.recovery.activation_generation > 0)
        .map(|(team_id, member_id)| CardKey {
            card_schema: CARD_SCHEMA,
            recipient: (team_id.clone(), member_id.clone()),
            context: runtime.recovery.context(),
            roots: Roots {
                root_authority_revision: registry.revision(team).unwrap_or_default().to_string(),
                resolved_teams_root: normalize(root),
                resolved_mesh_config_root: root.parent().map(normalize).unwrap_or_default(),
                harness_account_root: "unavailable".into(),
                launch_namespace: "unavailable".into(),
                resolved_project_root: normalize(&member.project_path),
            },
            contract: Contract {
                role_id: member.role_id.clone().unwrap_or_default(),
                effective_role_revision: digest(&(
                    &member.instructions,
                    &member.behavioral_contract,
                    &member.runtime_compact_summary,
                    &member.quality_gates,
                    &member.handoff_expectations,
                    &member.definition_of_done,
                )),
                instruction_contract_revision: digest(&FIRST_ACTION),
                packet_revision: facts.packet_revision.clone(),
            },
        });
    let card = RecoveryCard::compile(team, member, snapshot.as_ref(), key.clone(), facts);
    let mut text = card.render();
    let mut receipt = if let Some(key) = key {
        // The durable append is the authority after an interrupted runtime commit.
        if let Some(previous) = runtime.recovery.claim.clone().filter(|r| r.path == "inbox") {
            let appended = MeshInboxStore::load(root, team, member_name)?
                .into_iter()
                .find(|m| m.id.as_deref() == Some(&previous.delivery_id));
            if let Some(message) = appended {
                let observed = message
                    .extra
                    .get("recovery_card")
                    .and_then(|v| serde_json::from_value::<CardReceipt>(v.clone()).ok())
                    .filter(|r| {
                        r.delivery_id == previous.delivery_id && r.card_key == previous.card_key
                    })
                    .unwrap_or(previous);
                runtime
                    .recovery
                    .observe(&observed, ReceiptStage::Accepted, message.text.len());
            } else if previous.stage == ReceiptStage::OutcomeUnknown && path == "inbox" {
                runtime.recovery.observe(&previous, ReceiptStage::Failed, 0);
            }
        }
        let claim = runtime.recovery.claim(&key, &card.content_revision, path);
        MemberRuntimeStore::save_recovery_locked(&guard, root, team, member_name, &runtime)?;
        let Some(receipt) = claim else {
            return Ok(None);
        };
        receipt
    } else {
        // Legacy names/session ids are not enough to authorize suppression.
        CardReceipt {
            delivery_id: uuid::Uuid::new_v4().to_string(),
            obligation_key: ((String::new(), String::new()), (0, 0)),
            card_key: CardKey {
                card_schema: CARD_SCHEMA,
                recipient: (String::new(), String::new()),
                context: (0, 0),
                roots: Roots::default(),
                contract: Contract::default(),
            },
            content_revision: card.content_revision,
            kind: DeliveryKind::Baseline,
            supersedes_revision: None,
            attempt: 1,
            path: path.into(),
            stage: ReceiptStage::OutcomeUnknown,
            generated_bytes: 0,
            accepted_bytes: 0,
        }
    };
    if receipt.kind == DeliveryKind::Correction {
        text = format!("Correction: supersedes_revision={}; authority=taurhaus; scope=roots/role/contract/packet; effective=current recipient/context only. Replaces those instruction groups; assignment and GO authority remain unchanged.\n{text}", receipt.supersedes_revision.as_deref().unwrap_or("unavailable"));
    }
    receipt.generated_bytes = text.len();
    Ok(Some(PreparedCard { receipt, text }))
}

pub fn observe(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
    member: &str,
    receipt: &CardReceipt,
    stage: ReceiptStage,
) -> Result<(), CoordinationError> {
    validate_root(registry, root, team)?;
    let guard = crate::coordination::stores::lock::acquire_team_lock(root, team)?;
    validate_root(registry, root, team)?;
    if !receipt.card_key.recipient.0.is_empty()
        && receipt.card_key.roots.root_authority_revision != registry.revision(team)?.to_string()
    {
        return Err(CoordinationError::Conflict(
            "stale recovery root receipt".into(),
        ));
    }
    let mut runtime = MemberRuntimeStore::load(root, team, member)?;
    runtime
        .recovery
        .observe(receipt, stage, receipt.generated_bytes);
    MemberRuntimeStore::save_recovery_locked(&guard, root, team, member, &runtime)
}

/// Read only the current owner's typed operational facts. Never copy task prose,
/// peer verdicts, derivative counts, or a private message into the card.
pub fn assignment_facts(
    root: &Path,
    team: &str,
    member: &str,
    snapshot: Option<&crate::coordination::stores::OperationalContextSnapshot>,
) -> AssignmentFacts {
    let mut facts = AssignmentFacts::default();
    let Some(snapshot) = snapshot else {
        return facts;
    };
    let Ok(path) = crate::coordination::stores::mesh_task::task_path(root, team, &snapshot.task.id)
    else {
        return facts;
    };
    if std::fs::metadata(&path)
        .map(|m| m.len() > 1_048_576)
        .unwrap_or(true)
    {
        return facts;
    }
    let Some(task) = std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
    else {
        return facts;
    };
    if task["id"].as_str() != Some(&snapshot.task.id) || task["owner"].as_str() != Some(member) {
        return facts;
    }
    facts.task_id = snapshot.task.id.clone();
    facts.owner = member.into();
    let metadata = &task["metadata"];
    let get = |key: &str| metadata[key].as_str().unwrap_or_default().to_string();
    facts.assignment_token = get("assignment_id");
    facts.stage_id = get("stage_id");
    facts.state = task["status"].as_str().unwrap_or("unavailable").into();
    facts.wait = if crate::coordination::stores::mesh_task::awaiting_go(Some(metadata)) {
        "awaiting_go"
    } else {
        "release_unavailable"
    }
    .into();
    // No wait marker is not proof of a token-bound GO.
    if metadata["released_assignment"].as_str() == Some(facts.assignment_token.as_str())
        && !facts.assignment_token.is_empty()
    {
        facts.wait = "released".into();
    }
    let contract = &metadata["assignment_contract"];
    for (field, target) in [
        ("objective", &mut facts.objective),
        ("deliverable", &mut facts.deliverable),
        ("first_action", &mut facts.first_action),
        ("completion_signal", &mut facts.completion_signal),
        ("review_route", &mut facts.review_route),
    ] {
        *target = contract[field].as_str().unwrap_or_default().into();
    }
    facts.candidate_ref = get("candidate_ref");
    facts.rubric_ref = get("rubric_ref");
    facts.packet_revision = get("packet_revision");
    facts.restart_cursor_ref = get("restart_cursor_ref");
    facts.audience_policy_revision = "recipient-only:no-peer-evidence:v1".into();
    facts.source_revision = digest(&facts);
    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::recovery_card::ReceiptStage;
    use crate::coordination::stores::{MeshInboxMessage, MeshInboxStore, TeamRootRegistry};
    use chrono::Utc;
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, TeamRootRegistry) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("account/teams");
        std::fs::create_dir_all(root.join("team/runtime")).unwrap();
        std::fs::write(root.join("team/config.json"), json!({
            "schema_version":1,"name":"team","created_at":Utc::now(),
            "team_incarnation_id":"team-1",
            "members":[{"name":"seat","role":"agent","cli_tool":"codex","project_path":temp.path()}]
        }).to_string()).unwrap();
        std::fs::write(
            root.join("team/runtime/seat.json"),
            json!({
                "member_name":"seat", "pane_id":"%1", "session_id":"session-1",
                "foreign_mesh_field":{"keep":true}
            })
            .to_string(),
        )
        .unwrap();
        let registry = TeamRootRegistry::new(root.clone());
        (temp, root, registry)
    }

    fn append(root: &std::path::Path, prepared: &PreparedCard) {
        let mut message = MeshInboxMessage::new("lead", prepared.text.clone(), None, Utc::now());
        message.id = Some(prepared.receipt.delivery_id.clone());
        message.extra.insert(
            "recovery_card".into(),
            serde_json::to_value(&prepared.receipt).unwrap(),
        );
        MeshInboxStore::append(root, "team", "seat", &message).unwrap();
    }

    #[test]
    fn recovery_append_before_bookkeeping_reconciles_original_receipt_without_replay() {
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "attachment-1").unwrap();
        let prepared = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        append(&root, &prepared); // Simulated crash: no runtime receipt commit.
        assert!(prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .is_none());
        let runtime =
            crate::coordination::stores::MemberRuntimeStore::load(&root, "team", "seat").unwrap();
        assert_eq!(
            runtime.recovery.last_delivered.unwrap().stage,
            ReceiptStage::Accepted
        );
        assert_eq!(runtime.extra["foreign_mesh_field"], json!({"keep":true}));
        assert_eq!(
            MeshInboxStore::load(&root, "team", "seat").unwrap().len(),
            1
        );
    }

    #[test]
    fn recovery_reserved_but_unappended_claim_retries_same_id_only_once() {
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "attachment-1").unwrap();
        let first = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        let retry = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert_eq!(first.receipt.delivery_id, retry.receipt.delivery_id);
        assert_eq!(retry.receipt.attempt, 2);
        assert!(prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .is_none());
    }

    #[test]
    fn recovery_legacy_identity_never_suppresses_on_guessed_names() {
        let (_temp, root, registry) = fixture();
        let first = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert!(first.text.contains("generation_unknown"));
        append(&root, &first);
        assert!(prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .is_some());
    }
}
