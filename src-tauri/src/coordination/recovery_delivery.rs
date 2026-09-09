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
        record.reserve_activation(intent)
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
    prepare_inner(registry, root, team, member_name, path, false, false)
}

pub(crate) fn prepare_compaction(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
    member_name: &str,
    path: &str,
) -> Result<Option<PreparedCard>, CoordinationError> {
    prepare_inner(registry, root, team, member_name, path, true, false)
}

/// Recompose at submission without reserving another transport attempt.
pub(crate) fn refresh(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
    member: &str,
) -> Result<Option<PreparedCard>, CoordinationError> {
    prepare_inner(registry, root, team, member, "inbox", false, true)
}

fn prepare_inner(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
    member_name: &str,
    path: &str,
    compaction: bool,
    refresh: bool,
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
    let mut facts = assignment_facts(root, team, member_name, snapshot.as_ref());
    let normalize = |p: &Path| crate::provider::path::normalize_project_path(&p.to_string_lossy());
    let authority_revision = registry.revision(team)?.to_string();
    let key = config
        .team_incarnation_id
        .as_ref()
        .zip(runtime.recovery.member_incarnation_id.as_ref())
        .filter(|_| runtime.attachment_generation > 0)
        .map(|(team_id, member_id)| CardKey {
            card_schema: CARD_SCHEMA,
            recipient: (team_id.clone(), member_id.clone()),
            context: (
                runtime.attachment_generation,
                runtime.context_generation + u64::from(compaction),
            ),
            roots: Roots {
                root_authority_revision: authority_revision.clone(),
                resolved_teams_root: normalize(root),
                resolved_mesh_config_root: root.parent().map(normalize).unwrap_or_default(),
                harness_account_root: runtime
                    .recovery
                    .harness_account_root
                    .clone()
                    .unwrap_or_else(|| "unavailable".into()),
                launch_namespace: runtime
                    .recovery
                    .launch_namespace
                    .clone()
                    .unwrap_or_else(|| "unavailable".into()),
                resolved_project_root: normalize(&member.project_path),
            },
            contract: Contract {
                role_id: member.role_id.clone().unwrap_or_default(),
                effective_role_revision: digest(&(
                    &member.communication_style,
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
    let effective_effort = if runtime.attached_at < runtime.recovery.reserved_attachment {
        runtime.recovery.reserved_effort.clone()
    } else {
        runtime.applied_effort.clone()
    }
    .unwrap_or_default();
    facts.source_revision = digest(&(
        &facts.source_revision,
        &effective_effort,
        &runtime.effort_resume_failure,
    ));
    let mut card = RecoveryCard::compile(team, member, snapshot.as_ref(), key.clone(), facts);
    card.effective_effort = effective_effort;
    card.effort_hold = runtime
        .effort_resume_failure
        .as_ref()
        .map(|_| "assignment held after failed effort relaunch".into())
        .unwrap_or_else(|| "none recorded".into());
    crate::coordination::reinjection::CompactionReinjectionService::append_member_lease_context(
        &mut card.lease_context,
        root,
        team,
        member_name,
    );
    let mut descriptor_snapshot = snapshot.clone().unwrap_or_else(|| {
        crate::coordination::stores::OperationalContextSnapshot {
            recovery_card: None,
            version: 1,
            team_name: team.into(),
            member_name: member_name.into(),
            updated_at: chrono::Utc::now(),
            task: Default::default(),
            assignment_footer: Default::default(),
            ownership: Default::default(),
            working_set: crate::coordination::stores::OperationalWorkingSetSnapshot {
                project_path: card.project_path.clone(),
                focal_files: Vec::new(),
            },
        }
    });
    descriptor_snapshot.recovery_card = Some(crate::coordination::recovery_card::CardDescriptor {
        descriptor_only: snapshot
            .as_ref()
            .is_none_or(|s| s.recovery_card.as_ref().is_some_and(|d| d.descriptor_only)),
        card_schema: CARD_SCHEMA,
        card_key: key.clone(),
        content_revision: card.content_revision.clone(),
        pending: crate::coordination::stores::MemberCompactionStore::load(root, team, member_name)?
            .is_some_and(|s| s.pending),
    });
    OperationalContextSnapshotStore::save_locked(&guard, root, &descriptor_snapshot)?;
    let canonical = crate::coordination::journal::canonical(root, team)?;
    let mut text = card.render();
    let mut receipt = if let Some(key) = key {
        // The durable append is the authority after an interrupted runtime commit.
        if let Some(previous) = runtime
            .recovery
            .claim
            .clone()
            .filter(|r| r.path == "inbox" && !canonical)
        {
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
                runtime.recovery.claim = Some(observed.clone());
                runtime
                    .recovery
                    .observe(&observed, ReceiptStage::Accepted, message.text.len());
            } else if !refresh && previous.stage == ReceiptStage::OutcomeUnknown && path == "inbox"
            {
                runtime.recovery.observe(&previous, ReceiptStage::Failed, 0);
            }
        }
        let mut claim = if refresh {
            runtime
                .recovery
                .claim
                .as_ref()
                .filter(|r| r.card_key == key && r.stage == ReceiptStage::OutcomeUnknown)
                .cloned()
        } else {
            None
        };
        if claim.is_none() {
            claim = runtime.recovery.claim(&key, &card.content_revision, path);
        }
        if refresh {
            if let Some(receipt) = claim.as_mut() {
                receipt.content_revision = card.content_revision.clone();
            }
        }
        if path == "read" && claim.is_none() {
            claim = runtime
                .recovery
                .last_delivered
                .as_ref()
                .filter(|r| r.card_key == key)
                .or_else(|| {
                    runtime
                        .recovery
                        .claim
                        .as_ref()
                        .filter(|r| r.card_key == key)
                })
                .cloned();
        }
        if let Some(receipt) = claim.as_mut().filter(|_| path == "read") {
            receipt.path = "read".into();
            receipt.content_revision = card.content_revision.clone();
        }
        MemberRuntimeStore::save_recovery_locked(&guard, root, team, member_name, &runtime)?;
        let Some(receipt) = claim else {
            return Ok(None);
        };
        receipt
    } else {
        // Legacy names/session ids are not enough to authorize suppression.
        CardReceipt {
            journal: None,
            journal_links: None,
            delivery_id: uuid::Uuid::new_v4().to_string(),
            satisfied_obligations: Vec::new(),
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
            offered_bytes: 0,
            returned_by_read_bytes: 0,
            observations: Vec::new(),
        }
    };
    if receipt.kind == DeliveryKind::Correction {
        text = format!("Correction: supersedes_revision={}; authority=taurhaus; scope=roots/role/contract/packet; effective=current recipient/context only. Replaces those instruction groups; assignment and GO authority remain unchanged.\n{text}", receipt.supersedes_revision.as_deref().unwrap_or("unavailable"));
    }
    // Increment 1 carries skipped obligations on this live baseline/correction/force/read
    // path. Mesh-owned assignment deltas are not wired to operational_context yet.
    if let Some(pending) =
        crate::coordination::stores::MemberCompactionStore::load(root, team, member_name)?
            .and_then(|s| s.pending_obligation)
    {
        if pending.0 == receipt.obligation_key.0
            && !receipt.satisfied_obligations.contains(&pending)
        {
            receipt.satisfied_obligations.push(pending);
        }
    }
    receipt.journal_links =
        (!card.assignment.task_id.is_empty()).then(|| crate::coordination::journal::JournalLinks {
            task: card.assignment.task_id.clone(),
            assignment: card.assignment.assignment_token.clone(),
        });
    receipt.generated_bytes = text.len();
    if !receipt.card_key.recipient.0.is_empty() {
        runtime.recovery.claim = Some(receipt.clone());
        MemberRuntimeStore::save_recovery_locked(&guard, root, team, member_name, &runtime)?;
    }
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
    let durable_receipt = if stage == ReceiptStage::Accepted
        && !crate::coordination::journal::canonical(root, team)?
    {
        MeshInboxStore::load(root, team, member)?
            .into_iter()
            .find(|m| m.id.as_deref() == Some(&receipt.delivery_id))
            .and_then(|m| {
                m.extra
                    .get("recovery_card")
                    .and_then(|v| serde_json::from_value::<CardReceipt>(v.clone()).ok())
            })
            .filter(|r| r.card_key == receipt.card_key)
    } else {
        None
    };
    let receipt = durable_receipt.as_ref().unwrap_or(receipt);
    let current_claim =
        runtime.recovery.claim.as_ref().is_some_and(|r| {
            r.delivery_id == receipt.delivery_id && r.card_key == receipt.card_key
        });
    if current_claim && durable_receipt.is_some() {
        runtime.recovery.claim = Some(receipt.clone());
    }
    runtime
        .recovery
        .observe(receipt, stage, receipt.generated_bytes);
    MemberRuntimeStore::save_recovery_locked(&guard, root, team, member, &runtime)?;
    if stage.satisfies() && current_claim {
        if let Some(mut pending) =
            crate::coordination::stores::MemberCompactionStore::load(root, team, member)?
        {
            if pending
                .pending_obligation
                .as_ref()
                .is_some_and(|key| receipt.satisfied_obligations.contains(key))
            {
                pending.pending = false;
                pending.satisfied_by = Some(receipt.delivery_id.clone());
                if let Some(mut snapshot) =
                    OperationalContextSnapshotStore::load(root, team, member)?
                {
                    if let Some(descriptor) = snapshot.recovery_card.as_mut() {
                        descriptor.pending = false;
                    }
                    OperationalContextSnapshotStore::save_locked(&guard, root, &snapshot)?;
                }
                crate::coordination::stores::MemberCompactionStore::save_locked(
                    &guard, root, team, member, &pending,
                )?;
            }
        }
    }
    let mut observation = receipt.clone();
    observation.record(stage, receipt.generated_bytes);
    taurhaus_lib::logging::emit_global(
        "info",
        "coordination",
        "onboarding.delivery.observed",
        None,
        serde_json::to_value(observation)
            .expect("receipt")
            .as_object()
            .unwrap()
            .clone(),
    );
    Ok(())
}

pub fn read_current(
    registry: &TeamRootRegistry,
    root: &Path,
    team: &str,
    member: &str,
) -> Result<(String, CardReceipt), CoordinationError> {
    let card = prepare(registry, root, team, member, "read")?
        .ok_or_else(|| CoordinationError::Conflict("current recovery view unavailable".into()))?;
    observe(
        registry,
        root,
        team,
        member,
        &card.receipt,
        ReceiptStage::ConsumedByRead,
    )?;
    let mut receipt = card.receipt;
    receipt.record(ReceiptStage::ConsumedByRead, receipt.generated_bytes);
    Ok((card.text, receipt))
}

/// Called only by the daemon backend or native hook that owns delivery receipts.
pub fn observe_inbox_failure(
    root: &Path,
    team: &str,
    member: &str,
    message: &crate::coordination::stores::MeshInboxMessage,
    error: &CoordinationError,
) {
    if crate::coordination::journal::not_submitted(error)
        && observe_pre_submission_failure(root, team, member, message).is_err()
    {
        // Preserve the submission error; persistence failure keeps the claim quarantined.
        tracing::warn!(
            team,
            member,
            "could not persist pre-submission failure receipt"
        );
    }
}

/// A failed preflight/spawn cannot have committed a journal record. Record that
/// fact on the matching claim so RecoveryState's existing two-attempt budget applies.
fn observe_pre_submission_failure(
    root: &Path,
    team: &str,
    member: &str,
    message: &crate::coordination::stores::MeshInboxMessage,
) -> Result<(), CoordinationError> {
    let Some(mut receipt) = message
        .extra
        .get("recovery_card")
        .and_then(|v| serde_json::from_value::<CardReceipt>(v.clone()).ok())
    else {
        return Ok(());
    };
    let mut recorded = false;
    MemberRuntimeStore::update(root, team, member, |runtime| {
        if let Some(claim) = runtime.recovery.claim.as_ref().filter(|claim| {
            claim.delivery_id == receipt.delivery_id
                && claim.attempt == receipt.attempt
                && claim.card_key == receipt.card_key
                && claim.stage == ReceiptStage::OutcomeUnknown
        }) {
            // The inbox envelope is pre-marked accepted for legacy persistence.
            // Only the runtime claim is evidence of what actually happened.
            receipt = claim.clone();
            runtime.recovery.observe(&receipt, ReceiptStage::Failed, 0);
            recorded = true;
        }
    })?;
    if recorded {
        receipt.record(ReceiptStage::Failed, 0);
        taurhaus_lib::logging::emit_global(
            "info",
            "coordination",
            "onboarding.delivery.observed",
            None,
            serde_json::to_value(receipt)
                .expect("receipt")
                .as_object()
                .unwrap()
                .clone(),
        );
    }
    Ok(())
}

pub fn attach_journal_links(
    message: &mut crate::coordination::stores::MeshInboxMessage,
    explicit: Option<&crate::coordination::journal::JournalLinks>,
    context: Option<&crate::coordination::requests::OperationalContextUpdate>,
) {
    if let Some(links) = explicit {
        message.extra.insert(
            "journal_links".into(),
            serde_json::to_value(links).expect("links"),
        );
    } else if let Some(task) = context
        .and_then(|c| c.task.as_ref())
        .filter(|_| !message.extra.contains_key("journal_links"))
    {
        let links = crate::coordination::journal::JournalLinks {
            task: task.id.clone(),
            assignment: String::new(),
        };
        message.extra.insert(
            "journal_links".into(),
            serde_json::to_value(links).expect("links"),
        );
    }
}

pub fn attach_receipt(
    message: &mut crate::coordination::stores::MeshInboxMessage,
    receipt: Option<&CardReceipt>,
) {
    if let Some(receipt) = receipt {
        if let Some(links) = &receipt.journal_links {
            message.extra.insert(
                "journal_links".into(),
                serde_json::to_value(links).expect("links"),
            );
        }
        let mut receipt = receipt.clone();
        receipt.record(ReceiptStage::Accepted, message.text.len());
        message.id = Some(receipt.delivery_id.clone());
        message.extra.insert(
            "recovery_card".into(),
            serde_json::to_value(receipt).expect("receipt"),
        );
    }
}

/// Read only the current owner's typed operational facts. Never copy task prose,
/// peer verdicts, derivative counts, or a private message into the card.
pub fn assignment_facts(
    root: &Path,
    team: &str,
    member: &str,
    snapshot: Option<&crate::coordination::stores::OperationalContextSnapshot>,
) -> AssignmentFacts {
    let mut facts = AssignmentFacts {
        wait: "unavailable".into(),
        source_revision: digest(
            &snapshot
                .map(|s| {
                    (
                        &s.task.id,
                        &s.task.subject,
                        &s.task.status,
                        &s.assignment_footer,
                        &s.ownership,
                        &s.working_set.focal_files,
                    )
                })
                .map(|(id, subject, status, footer, ownership, files)| {
                    (
                        id.clone(),
                        subject.clone(),
                        status.clone(),
                        footer.clone(),
                        ownership.clone(),
                        files.clone(),
                    )
                })
                .unwrap_or_default(),
        ),
        ..Default::default()
    };
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
    let get = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| {
                metadata[*key]
                    .as_str()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
            })
            .unwrap_or_default()
            .to_string()
    };
    facts.assignment_token = get(&["assignment_id", "assignmentId"]);
    facts.state = task["status"].as_str().unwrap_or("unavailable").into();
    // Mesh owns release: its writer clears awaiting_go when an assignment is open.
    facts.wait = if crate::coordination::stores::mesh_task::awaiting_go(Some(metadata)) {
        "awaiting_go"
    } else {
        match facts.state.as_str() {
            "pending" | "in_progress" if !facts.assignment_token.is_empty() => "released",
            "pending" | "in_progress" => "unassigned",
            "blocked" => "blocked",
            "completed" | "cancelled" | "canceled" | "stale" | "failed" => "terminal",
            _ => "unavailable",
        }
    }
    .into();
    facts.objective = task["subject"].as_str().unwrap_or_default().into();
    facts.deliverable = get(&["deliverable"]);
    facts.first_action = get(&["first_step", "firstStep"]);
    facts.completion_signal = get(&["completion_signal", "completionSignal"]);
    // Stage, review route, candidate/rubric and cursor references await the Mesh projection.
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

    fn assigned_snapshot(
        root: &Path,
        metadata: serde_json::Value,
        status: &str,
    ) -> crate::coordination::stores::OperationalContextSnapshot {
        let snapshot = serde_json::from_value(json!({"version":1,"team_name":"team","member_name":"seat","updated_at":Utc::now(),"task":{"id":"1","subject":"Review","status":status},"assignment_footer":{},"ownership":{"override_allowed":false},"working_set":{"project_path":root,"focal_files":[]}})).unwrap();
        let path = crate::coordination::stores::mesh_task::task_path(root, "team", "1").unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            json!({"id":"1","owner":"seat","status":status,"metadata":metadata}).to_string(),
        )
        .unwrap();
        snapshot
    }

    #[cfg(all(unix, feature = "mesh-bridged-backend"))]
    #[test]
    fn canonical_recovery_receipts_links_and_no_projection_retry() {
        use crate::coordination::backend::{
            bridged::MeshBridgedBackend, claude::ClaudeNativeBackend, CoordinationBackend,
        };
        use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
        // Regression: 20b27ac6 sent stale snapshot assignment tokens as delivery preconditions.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        for bridged in [false, true] {
            let mesh = crate::coordination::mesh_cli::FakeMesh::new(
                r#"echo '{"journal_writer":"mesh-journal/2"}'"#,
                r#"for arg in "$@"; do
                    if [ "$arg" = --assignment ]; then echo 'error: assignment token mismatch' >&2; exit 1; fi
                done
                echo '{"status":"accepted","message_id":"m1","sequence":1,"projection":"pending","delivery_targets":[{"recipient":"seat","delivery_id":"d1"}]}'"#,
            );
            let (_temp, root, registry) = fixture();
            let log_path = mesh.dir.path().join("observed.jsonl");
            let sink = taurhaus_lib::logging::LogFileState::new(log_path.clone()).unwrap();
            taurhaus_lib::logging::install_global_sink(&sink);
            let mut config = TeamConfigStore::load(&root, "team").unwrap();
            config.extra.insert("messaging_format".into(), json!(2));
            let mut lead = config.members[0].clone();
            lead.name = "lead".into();
            lead.role = crate::coordination::domain::MemberRole::Lead;
            config.members.push(lead);
            TeamConfigStore::save(&root, "team", &config).unwrap();
            reserve_activation(&root, "team", "seat", "activation").unwrap();
            let snapshot = assigned_snapshot(&root, json!({"assignment_id":"a1"}), "in_progress");
            OperationalContextSnapshotStore::save(&root, &snapshot).unwrap();
            let card = prepare(&registry, &root, "team", "seat", "inbox")
                .unwrap()
                .unwrap();
            let backend: Box<dyn CoordinationBackend> = if bridged {
                Box::new(MeshBridgedBackend::new_with_teams_dir(root.clone()))
            } else {
                Box::new(ClaudeNativeBackend::new(root.clone()))
            };
            let mut orchestrator =
                crate::coordination::orchestrator::CoordinationOrchestrator::new_with_runtime(
                    root.clone(),
                    std::sync::Arc::from(backend),
                    std::sync::Arc::new(
                        crate::coordination::runtime::RecordingCoordinationRuntime::default(),
                    ),
                );
            let result = orchestrator
                .deliver_message(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                    journal_links: None,
                    recovery_card: Some(card.receipt.clone()),
                    team_name: "team".into(),
                    member_name: "seat".into(),
                    message: card.text,
                    sender_name: None,
                    operational_context: Some(
                        crate::coordination::requests::OperationalContextUpdate {
                            task: Some(crate::coordination::requests::OperationalTaskContext {
                                id: "1".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ),
                }))
                .unwrap();
            let wire = serde_json::to_value(&result).unwrap();
            assert_eq!(wire["recoveryCard"]["journal"]["message_id"], "m1");
            assert_eq!(wire["recoveryCard"]["journal"]["delivery_id"], "d1");
            assert!(mesh.argv().contains("--task\n1\n"));
            assert!(!mesh.argv().contains("--assignment\n"));
            assert!(mesh.argv().contains("--name\nlead\n"));
            assert!(prepare(&registry, &root, "team", "seat", "inbox")
                .unwrap()
                .is_none());
            assert!(MeshInboxStore::load(&root, "team", "seat")
                .unwrap()
                .is_empty());
            assert_eq!(mesh.argv().matches("accept\n").count(), 1);
            sink.flush_for_test().unwrap();
            let events = std::fs::read_to_string(log_path).unwrap();
            let event: serde_json::Value = events
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .find(|v| v["event"] == "onboarding.delivery.observed")
                .unwrap();
            assert_eq!(event["journal"]["message_id"], "m1");
            assert_eq!(event["journal"]["delivery_id"], "d1");
            assert_eq!(event["delivery_id"], card.receipt.delivery_id);
        }
    }

    #[cfg(unix)]
    #[test]
    fn canonical_unknown_recovery_submission_is_not_retried_from_array_absence() {
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        // Regression: 3ca169ed reconciled missing legacy rows into a retry, unsafe for canonical acceptance.
        let mesh = crate::coordination::mesh_cli::FakeMesh::new(
            r#"echo '{"journal_writer":"mesh-journal/2"}'"#,
            "exit 23",
        );
        let (_temp, root, registry) = fixture();
        let mut config = TeamConfigStore::load(&root, "team").unwrap();
        config.extra.insert("messaging_format".into(), json!(2));
        TeamConfigStore::save(&root, "team", &config).unwrap();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let card = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        let mut message = MeshInboxMessage::new("lead", card.text, None, Utc::now());
        attach_receipt(&mut message, Some(&card.receipt));
        assert!(MeshInboxStore::append(&root, "team", "seat", &message).is_err());
        assert!(prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .is_none());
        assert_eq!(mesh.argv().matches("accept\n").count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn canonical_pre_submission_failure_allows_only_one_card_retry() {
        // Regression: 755560bb quarantined even failed version probes before any submission.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        for version in [
            "missing",
            "exit 19",
            r#"echo '{"journal_writer":"mesh-journal/1"}'"#,
        ] {
            let mesh = crate::coordination::mesh_cli::FakeMesh::new(version, "exit 99");
            if version == "missing" {
                std::fs::remove_file(mesh.dir.path().join("mesh")).unwrap();
            }
            let (_temp, root, registry) = fixture();
            let mut config = TeamConfigStore::load(&root, "team").unwrap();
            config.extra.insert("messaging_format".into(), json!(2));
            TeamConfigStore::save(&root, "team", &config).unwrap();
            reserve_activation(&root, "team", "seat", "activation").unwrap();
            let mut identity = None;
            for attempt in 1..=2 {
                let card = prepare(&registry, &root, "team", "seat", "inbox")
                    .unwrap()
                    .unwrap();
                assert_eq!(card.receipt.attempt, attempt);
                assert_eq!(
                    identity.get_or_insert(card.receipt.delivery_id.clone()),
                    &card.receipt.delivery_id
                );
                let mut message = MeshInboxMessage::new("lead", card.text, None, Utc::now());
                attach_receipt(&mut message, Some(&card.receipt));
                use crate::coordination::backend::{
                    claude::ClaudeNativeBackend, CoordinationBackend,
                };
                use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
                let backend = ClaudeNativeBackend::new(root.clone());
                assert!(backend
                    .deliver(DeliveryRequest::operator_notice(OperatorNoticeDelivery {
                        team_name: "team".into(),
                        member_name: "seat".into(),
                        message: message.text,
                        sender_name: Some("lead".into()),
                        recovery_card: Some(card.receipt),
                        journal_links: None,
                        operational_context: None,
                    }))
                    .is_err());
                let runtime = MemberRuntimeStore::load(&root, "team", "seat").unwrap();
                let failed = runtime.recovery.claim.unwrap();
                assert_eq!(failed.stage, ReceiptStage::Failed);
                // Regression: 6efb08f5 copied the pre-marked legacy envelope into a failure receipt.
                assert_eq!(failed.accepted_bytes, 0);
                assert!(!failed
                    .observations
                    .iter()
                    .any(|(stage, _, _)| *stage == ReceiptStage::Accepted));
            }
            assert!(prepare(&registry, &root, "team", "seat", "inbox")
                .unwrap()
                .is_none());
            assert!(!mesh.argv().contains("accept\n"));
            assert!(!root.join("team/inboxes").exists());
        }
    }

    #[test]
    fn recovery_mesh_flat_contract_and_aliases_are_projected() {
        // Regression: 2760e88a read an assignment_contract object the owning writer never stores.
        let (_temp, root, _) = fixture();
        for metadata in [
            json!({"assignment_id":"a1","first_step":"Run the review","deliverable":"Review report","completion_signal":"Submit report"}),
            json!({"assignmentId":"a1","firstStep":"Run the review","deliverable":"Review report","completionSignal":"Submit report"}),
        ] {
            let snapshot = assigned_snapshot(&root, metadata, "in_progress");
            let facts = assignment_facts(&root, "team", "seat", Some(&snapshot));
            assert_eq!(facts.assignment_token, "a1");
            assert_eq!(facts.first_action, "Run the review");
            assert_eq!(facts.deliverable, "Review report");
            assert_eq!(facts.completion_signal, "Submit report");
            assert!(facts.stage_id.is_empty() && facts.review_route.is_empty());
            assert!(
                facts.candidate_ref.is_empty()
                    && facts.rubric_ref.is_empty()
                    && facts.restart_cursor_ref.is_empty()
            );
        }
    }

    #[test]
    fn recovery_pending_requires_skipped_boundary_with_known_identity() {
        // Regression: bd853b4f latched pending even for injected or unversioned boundaries.
        use crate::coordination::stores::compaction::{
            record_delivery_at, CompactionDeliveryResult,
        };
        for versioned in [false, true] {
            for result in [
                CompactionDeliveryResult::Skipped,
                CompactionDeliveryResult::Injected,
            ] {
                let (_temp, root, _) = fixture();
                if versioned {
                    reserve_activation(&root, "team", "seat", "activation").unwrap();
                }
                record_delivery_at(
                    &root,
                    "team",
                    "seat",
                    crate::session_scanner::cli_tool::CliTool::Codex,
                    "session-1",
                    Utc::now(),
                    result,
                )
                .unwrap();
                let state =
                    crate::coordination::stores::MemberCompactionStore::load(&root, "team", "seat")
                        .unwrap()
                        .unwrap();
                let expected = versioned && result == CompactionDeliveryResult::Skipped;
                assert_eq!(state.pending, expected);
                assert_eq!(state.pending_obligation.is_some(), expected);
            }
        }
    }

    #[test]
    fn recovery_injected_boundary_keeps_pending_until_its_receipt_is_offered() {
        // Regression: 78c64894 cleared skipped obligations on Injected before stdout succeeded.
        use crate::coordination::stores::compaction::{
            record_delivery_at, CompactionDeliveryResult,
        };
        use crate::coordination::stores::MemberCompactionStore;
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let now = Utc::now();
        record_delivery_at(
            &root,
            "team",
            "seat",
            crate::session_scanner::cli_tool::CliTool::Codex,
            "session-1",
            now,
            CompactionDeliveryResult::Skipped,
        )
        .unwrap();
        let pending = MemberCompactionStore::load(&root, "team", "seat")
            .unwrap()
            .unwrap();
        let card = prepare_compaction(&registry, &root, "team", "seat", "hook_stdout")
            .unwrap()
            .unwrap();
        assert!(card
            .receipt
            .satisfied_obligations
            .contains(pending.pending_obligation.as_ref().unwrap()));
        record_delivery_at(
            &root,
            "team",
            "seat",
            crate::session_scanner::cli_tool::CliTool::Codex,
            "session-1",
            now + chrono::Duration::seconds(1),
            CompactionDeliveryResult::Injected,
        )
        .unwrap();
        observe(
            &registry,
            &root,
            "team",
            "seat",
            &card.receipt,
            ReceiptStage::OutcomeUnknown,
        )
        .unwrap();
        let unknown = MemberCompactionStore::load(&root, "team", "seat")
            .unwrap()
            .unwrap();
        assert!(
            unknown.pending,
            "bookkeeping cannot satisfy an unoffered obligation"
        );
        assert_eq!(unknown.pending_obligation, pending.pending_obligation);
        assert_eq!(unknown.satisfied_by, None);
        observe(
            &registry,
            &root,
            "team",
            "seat",
            &card.receipt,
            ReceiptStage::HookResponseOffered,
        )
        .unwrap();
        let offered = MemberCompactionStore::load(&root, "team", "seat")
            .unwrap()
            .unwrap();
        assert!(!offered.pending);
        assert_eq!(offered.pending_obligation, pending.pending_obligation);
        assert_eq!(
            offered.satisfied_by.as_deref(),
            Some(card.receipt.delivery_id.as_str())
        );
    }

    #[test]
    fn recovery_mesh_wait_marker_is_the_release_authority() {
        // Regression: 2760e88a required an unwritten released_assignment marker, holding every working seat.
        let (_temp, root, _) = fixture();
        for (status, marker, expected) in [
            ("in_progress", json!(null), "released"),
            ("pending", json!(false), "released"),
            ("in_progress", json!(true), "awaiting_go"),
            ("in_progress", json!("a1"), "awaiting_go"),
            ("completed", json!(null), "terminal"),
            ("blocked", json!(null), "blocked"),
        ] {
            let snapshot = assigned_snapshot(
                &root,
                json!({"assignment_id":"a1","awaiting_go":marker}),
                status,
            );
            assert_eq!(
                assignment_facts(&root, "team", "seat", Some(&snapshot)).wait,
                expected
            );
        }
    }

    #[test]
    fn recovery_compaction_continues_owned_work_even_without_versioned_identity() {
        // Regression: 2760e88a gated execution on card_key, although legacy identity only disables suppression.
        let (_temp, root, registry) = fixture();
        let mut config = TeamConfigStore::load(&root, "team").unwrap();
        config.members[0].runtime_compact_summary = Some(serde_json::from_value(json!({"rolePurpose":"Review the assigned change","keepDoing":[],"workflowSequence":[],"avoid":[],"escalateWhen":[]})).unwrap());
        TeamConfigStore::save(&root, "team", &config).unwrap();
        let snapshot = assigned_snapshot(
            &root,
            json!({"assignment_id":"a1","first_step":"Run the review"}),
            "in_progress",
        );
        OperationalContextSnapshotStore::save(&root, &snapshot).unwrap();
        for versioned in [false, true] {
            if versioned {
                reserve_activation(&root, "team", "seat", "activation").unwrap();
            }
            let card = prepare_compaction(&registry, &root, "team", "seat", "hook_stdout")
                .unwrap()
                .unwrap();
            assert_eq!(!card.receipt.card_key.recipient.0.is_empty(), versioned);
            assert!(
                card.text.ends_with("Next action: Run the review"),
                "{}",
                card.text
            );
        }
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
    #[test]
    fn recovery_skipped_hook_persists_pending_and_next_delivery_satisfies_it() {
        use crate::coordination::stores::compaction::{
            record_delivery_at, CompactionDeliveryResult,
        };
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "attachment-1").unwrap();
        record_delivery_at(
            &root,
            "team",
            "seat",
            crate::session_scanner::cli_tool::CliTool::Codex,
            "session-1",
            Utc::now(),
            CompactionDeliveryResult::Skipped,
        )
        .unwrap();
        let path = root.join("team/state/compaction/seat.json");
        let state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(state["pending_obligation"].is_array());
        let card = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        append(&root, &card);
        observe(
            &registry,
            &root,
            "team",
            "seat",
            &card.receipt,
            ReceiptStage::Accepted,
        )
        .unwrap();
        let state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(state["satisfied_by"], card.receipt.delivery_id);
    }

    #[test]
    fn recovery_forced_wire_intent_retains_reason_and_retry_identity() {
        let wire = json!({"team_name":"team","member_name":"seat","force":true,"intent_id":"operator-1","reason":"manual clear"});
        let request: crate::coordination::requests::ReonboardRequest =
            serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(request).unwrap(), wire);
    }
    #[test]
    fn recovery_audience_projection_preserves_go_despite_contradictory_release_metadata() {
        // Regression: 2760e88a let release metadata override an explicit awaiting-GO state.
        let (_temp, root, _registry) = fixture();
        let snapshot: crate::coordination::stores::OperationalContextSnapshot =
            serde_json::from_value(json!({
                "version": 1, "team_name": "team", "member_name": "seat", "updated_at": Utc::now(),
                "task": {"id": "1", "subject": "Review", "status": "in_progress"},
                "assignment_footer": {},
                "ownership": {"override_allowed": false, "active_override_reason": null},
                "working_set": {"project_path": "/scratch", "focal_files": []}
            }))
            .unwrap();
        let path = crate::coordination::stores::mesh_task::task_path(&root, "team", "1").unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            json!({
                "id": "1", "owner": "seat", "status": "in_progress",
                "metadata": {
                    "assignment_id": "a1", "awaiting_go": true, "released_assignment": "a1",
                    "peer_verdict": "PRIVATE-VERDICT", "peer_summary": "PRIVATE-DERIVATIVE"
                }
            })
            .to_string(),
        )
        .unwrap();
        let facts = assignment_facts(&root, "team", "seat", Some(&snapshot));
        assert_eq!(facts.wait, "awaiting_go");
        assert!(!serde_json::to_string(&facts).unwrap().contains("PRIVATE-"));
    }
    #[test]
    fn recovery_explicit_read_returns_pending_baseline_without_inbox_fallback() {
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let (text, receipt) = read_current(&registry, &root, "team", "seat").unwrap();
        crate::coordination::recovery_card::assert_control_golden(&text);
        assert_eq!(receipt.stage, ReceiptStage::ConsumedByRead);
        assert!(MeshInboxStore::load(&root, "team", "seat")
            .unwrap()
            .is_empty());
        assert!(prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .is_none());
    }

    #[test]
    fn recovery_descriptor_is_published_on_the_existing_operational_snapshot() {
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let prepared = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        let snapshot = OperationalContextSnapshotStore::load(&root, "team", "seat")
            .unwrap()
            .expect("descriptor snapshot");
        let wire = serde_json::to_value(snapshot).unwrap();
        assert_eq!(
            wire["recovery_card"]["content_revision"],
            prepared.receipt.content_revision
        );
    }
    #[test]
    fn recovery_read_preserves_acceptance_and_clears_descriptor_pending() {
        // Regression: edf94e2c replaced acceptance with read status and left a stale descriptor.
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let boundary = Utc::now();
        use crate::coordination::stores::compaction::{
            record_delivery_at, CompactionDeliveryResult,
        };
        record_delivery_at(
            &root,
            "team",
            "seat",
            crate::session_scanner::cli_tool::CliTool::Codex,
            "session-1",
            boundary,
            CompactionDeliveryResult::Skipped,
        )
        .unwrap();
        let card = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        append(&root, &card);
        observe(
            &registry,
            &root,
            "team",
            "seat",
            &card.receipt,
            ReceiptStage::Accepted,
        )
        .unwrap();
        let (_, read) = read_current(&registry, &root, "team", "seat").unwrap();
        assert!(read.accepted_bytes > 0);
        assert!(
            serde_json::to_value(&read).unwrap()["returned_by_read_bytes"]
                .as_u64()
                .unwrap_or(0)
                > 0
        );
        assert!(
            !OperationalContextSnapshotStore::load(&root, "team", "seat")
                .unwrap()
                .unwrap()
                .recovery_card
                .unwrap()
                .pending
        );
        record_delivery_at(
            &root,
            "team",
            "seat",
            crate::session_scanner::cli_tool::CliTool::Codex,
            "session-1",
            boundary,
            CompactionDeliveryResult::Skipped,
        )
        .unwrap();
        assert!(
            !crate::coordination::stores::MemberCompactionStore::load(&root, "team", "seat")
                .unwrap()
                .unwrap()
                .pending
        );
    }

    #[test]
    fn recovery_task_delta_preserves_descriptor_and_effective_effort() {
        // Regression: edf94e2c's task snapshot writes erased the app-owned descriptor.
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        MemberRuntimeStore::update(&root, "team", "seat", |r| {
            r.applied_effort = Some("high".into())
        })
        .unwrap();
        let first = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert!(first.text.contains("effective effort: high"));
        crate::coordination::operational_context::apply_delivery_context(
            &root,
            "team",
            "seat",
            &Default::default(),
        )
        .unwrap();
        assert!(OperationalContextSnapshotStore::load(&root, "team", "seat")
            .unwrap()
            .unwrap()
            .recovery_card
            .is_some());
    }
    #[test]
    fn recovery_late_first_append_wins_over_a_reserved_retry() {
        // Regression: 2760e88a could not reconcile attempt 1 after attempt 2 was reserved.
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let first = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        let retry = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert_eq!(retry.receipt.attempt, 2);
        append(&root, &first);
        assert!(prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .is_none());
        let accepted = MemberRuntimeStore::load(&root, "team", "seat")
            .unwrap()
            .recovery
            .last_delivered
            .expect("durable first append wins");
        assert_eq!(accepted.attempt, 1);
        assert_eq!(accepted.content_revision, first.receipt.content_revision);
    }

    #[test]
    fn recovery_view_revision_covers_operational_facts_but_not_timestamps() {
        // Regression: 2760e88a left snapshot-only operative changes outside the view digest.
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        let first = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        let mut snapshot = OperationalContextSnapshotStore::load(&root, "team", "seat")
            .unwrap()
            .unwrap();
        snapshot.assignment_footer.validation_expectation = "cargo check".into();
        OperationalContextSnapshotStore::save(&root, &snapshot).unwrap();
        let retry = prepare(&registry, &root, "team", "seat", "inbox")
            .unwrap()
            .unwrap();
        assert_eq!(first.receipt.delivery_id, retry.receipt.delivery_id);
        assert_ne!(
            first.receipt.content_revision,
            retry.receipt.content_revision
        );
        append(&root, &retry);
        let (_, read) = read_current(&registry, &root, "team", "seat").unwrap();
        snapshot.updated_at = Utc::now();
        OperationalContextSnapshotStore::save(&root, &snapshot).unwrap();
        let (_, again) = read_current(&registry, &root, "team", "seat").unwrap();
        assert_eq!(read.content_revision, again.content_revision);
    }
    #[test]
    fn recovery_relaunch_reports_captured_effort_until_runtime_commit() {
        // Regression: a7ebc2a0 rendered the previous context's effort before activation commit.
        let (_temp, root, registry) = fixture();
        reserve_activation(&root, "team", "seat", "activation").unwrap();
        MemberRuntimeStore::update(&root, "team", "seat", |r| {
            r.applied_effort = Some("low".into());
            r.attached_at = Some(Utc::now() - chrono::Duration::hours(1));
            let mut wire = serde_json::to_value(&r.recovery).unwrap();
            wire["reserved_attachment"] = json!(Utc::now());
            wire["reserved_effort"] = json!("high");
            r.recovery = serde_json::from_value(wire).unwrap();
        })
        .unwrap();
        let (before, first) = read_current(&registry, &root, "team", "seat").unwrap();
        assert!(before.contains("effective effort: high"));
        MemberRuntimeStore::update(&root, "team", "seat", |r| {
            r.attached_at = Some(Utc::now());
            r.applied_effort = Some("medium".into());
        })
        .unwrap();
        let (after, next) = read_current(&registry, &root, "team", "seat").unwrap();
        assert!(after.contains("effective effort: medium"));
        assert_eq!(first.delivery_id, next.delivery_id);
        assert_ne!(first.content_revision, next.content_revision);
    }
}
