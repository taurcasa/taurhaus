//! Downstream compaction processing from canonical signal records.

use std::path::Path;

use chrono::{DateTime, Utc};

use crate::coordination::compaction_events::{
    emit_compaction_unresolved, CompactionSignalKind as EventSignalKind, CompactionUnresolvedEvent,
    CompactionUnresolvedReason,
};
use crate::coordination::domain::Member;
use crate::coordination::reinjection::{CompactionReinjectionService, OperationalReinjectionCard};
use crate::coordination::roster::get_team_roster_with_attachments;
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::{
    emit_compaction_delivery_event, emit_compaction_detected_event, is_stale_compaction,
    CompactionDeliveryResult, CompactionSignalKind, CompactionSignalRecord, MemberCompactionState,
    MemberCompactionStore, MemberRuntimeStore, MeshInboxMessage, MeshInboxStore,
    OperationalContextSnapshot, OperationalContextSnapshotStore, TeamConfigStore,
};
use crate::provider::path::normalize_project_path;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::CliTool;

const SKIP_REASON_ALREADY_HANDLED: &str = "already_handled";
const SKIP_REASON_MEMBER_NOT_ATTACHED: &str = "member_not_attached";
const SKIP_REASON_NO_RESUMABLE_TASK_CONTEXT: &str = "no_resumable_task_context";

const FAIL_REASON_RECORD_STALE_DELIVERY_FAILED: &str = "record_stale_delivery_failed";
const FAIL_REASON_RECORD_SKIPPED_DELIVERY_FAILED: &str = "record_skipped_delivery_failed";
const FAIL_REASON_RECORD_INJECTED_DELIVERY_FAILED: &str = "record_injected_delivery_failed";
const FAIL_REASON_APPEND_INBOX_FAILED: &str = "append_inbox_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompactionSignalProcessOutcome {
    Injected {
        team_name: String,
        member_name: String,
    },
    Skipped {
        team_name: String,
        member_name: String,
    },
    Stale {
        team_name: String,
        member_name: String,
    },
    Failed {
        team_name: String,
        member_name: String,
        error_message: String,
    },
    Unresolved {
        reason: CompactionUnresolvedReason,
    },
}

#[derive(Debug, Clone)]
struct ResolvedManagedCodexSignal {
    team_name: String,
    member_name: String,
    member: Member,
    snapshot: OperationalContextSnapshot,
}

#[derive(Debug, Default)]
pub struct CompactionSignalProcessor;

impl CompactionSignalProcessor {
    pub fn process_signal(signal: &CompactionSignalRecord) -> CompactionSignalProcessOutcome {
        let runtime = SystemCoordinationRuntime;
        let teams_dir = PlatformPaths::teams_dir();
        Self::process_signal_at(signal, &teams_dir, &runtime, Utc::now())
    }

    pub fn process_signal_at(
        signal: &CompactionSignalRecord,
        teams_dir: &Path,
        runtime: &dyn CoordinationRuntime,
        now: DateTime<Utc>,
    ) -> CompactionSignalProcessOutcome {
        let Some(resolved) = resolve_managed_codex_signal(teams_dir, signal) else {
            emit_compaction_unresolved(CompactionUnresolvedEvent {
                tool: signal.tool,
                session_id: Some(signal.session_id.clone()),
                pane_id: Some(signal.pane_id.clone()),
                project_path: signal.project_path.clone(),
                jsonl_path: Some(signal.jsonl_path.clone()),
                compaction_timestamp: signal.transcript_timestamp,
                signal_kind: Some(event_signal_kind(signal.signal_kind)),
                reason: CompactionUnresolvedReason::ManagedMemberResolutionUnavailable,
            });
            return CompactionSignalProcessOutcome::Unresolved {
                reason: CompactionUnresolvedReason::ManagedMemberResolutionUnavailable,
            };
        };

        emit_compaction_detected_event(
            &resolved.team_name,
            &resolved.member_name,
            signal.tool,
            &signal.session_id,
            signal.transcript_timestamp,
        );

        if is_stale_compaction(signal.transcript_timestamp, now) {
            if let Err(error) = record_delivery_at(
                teams_dir,
                &resolved.team_name,
                &resolved.member_name,
                &signal.session_id,
                signal.transcript_timestamp,
                CompactionDeliveryResult::Stale,
                None,
                None,
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: format!("{FAIL_REASON_RECORD_STALE_DELIVERY_FAILED}: {error}"),
                };
            }
            return CompactionSignalProcessOutcome::Stale {
                team_name: resolved.team_name,
                member_name: resolved.member_name,
            };
        }

        if already_handled(
            teams_dir,
            &resolved.team_name,
            &resolved.member_name,
            &signal.session_id,
            signal.transcript_timestamp,
        ) {
            if let Err(error) = record_delivery_at(
                teams_dir,
                &resolved.team_name,
                &resolved.member_name,
                &signal.session_id,
                signal.transcript_timestamp,
                CompactionDeliveryResult::Skipped,
                Some(SKIP_REASON_ALREADY_HANDLED),
                None,
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: format!("{FAIL_REASON_RECORD_SKIPPED_DELIVERY_FAILED}: {error}"),
                };
            }
            return CompactionSignalProcessOutcome::Skipped {
                team_name: resolved.team_name,
                member_name: resolved.member_name,
            };
        }

        if let Some(skip_reason) =
            delivery_skip_reason(teams_dir, &resolved, runtime, &signal.pane_id)
        {
            if let Err(error) = record_delivery_at(
                teams_dir,
                &resolved.team_name,
                &resolved.member_name,
                &signal.session_id,
                signal.transcript_timestamp,
                CompactionDeliveryResult::Skipped,
                Some(skip_reason),
                None,
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: format!("{FAIL_REASON_RECORD_SKIPPED_DELIVERY_FAILED}: {error}"),
                };
            }
            return CompactionSignalProcessOutcome::Skipped {
                team_name: resolved.team_name,
                member_name: resolved.member_name,
            };
        }

        if !CompactionReinjectionService::snapshot_has_resumable_task(&resolved.snapshot) {
            if let Err(error) = record_delivery_at(
                teams_dir,
                &resolved.team_name,
                &resolved.member_name,
                &signal.session_id,
                signal.transcript_timestamp,
                CompactionDeliveryResult::Skipped,
                Some(SKIP_REASON_NO_RESUMABLE_TASK_CONTEXT),
                None,
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: format!("{FAIL_REASON_RECORD_SKIPPED_DELIVERY_FAILED}: {error}"),
                };
            }
            return CompactionSignalProcessOutcome::Skipped {
                team_name: resolved.team_name,
                member_name: resolved.member_name,
            };
        }

        let card = CompactionReinjectionService::compose(&resolved.member, &resolved.snapshot);
        match append_codex_inbox_message(
            teams_dir,
            &resolved.team_name,
            &resolved.member_name,
            &card,
            now,
        ) {
            Ok(()) => {
                if let Err(error) = record_delivery_at(
                    teams_dir,
                    &resolved.team_name,
                    &resolved.member_name,
                    &signal.session_id,
                    signal.transcript_timestamp,
                    CompactionDeliveryResult::Injected,
                    None,
                    None,
                ) {
                    return CompactionSignalProcessOutcome::Failed {
                        team_name: resolved.team_name,
                        member_name: resolved.member_name,
                        error_message: format!(
                            "{FAIL_REASON_RECORD_INJECTED_DELIVERY_FAILED}: {error}"
                        ),
                    };
                }
                CompactionSignalProcessOutcome::Injected {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                }
            }
            Err(error) => {
                let _ = record_delivery_at(
                    teams_dir,
                    &resolved.team_name,
                    &resolved.member_name,
                    &signal.session_id,
                    signal.transcript_timestamp,
                    CompactionDeliveryResult::Failed,
                    None,
                    Some(FAIL_REASON_APPEND_INBOX_FAILED),
                );
                CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: format!("{FAIL_REASON_APPEND_INBOX_FAILED}: {error}"),
                }
            }
        }
    }
}

fn append_codex_inbox_message(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    card: &OperationalReinjectionCard,
    now: DateTime<Utc>,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    let rendered_payload =
        CompactionReinjectionService::render_codex_inbox_text(card).map_err(|error| {
            crate::coordination::errors::CoordinationError::StoreError(format!(
                "failed to serialize Codex post-compaction card for '{}' in '{}': {error}",
                member_name, team_name
            ))
        })?;
    let inbox_message = MeshInboxMessage::operator_originated(
        member_name,
        rendered_payload,
        Some("post_compaction_context".to_string()),
        now,
        None,
    );
    MeshInboxStore::append(teams_dir, team_name, member_name, &inbox_message)
}

fn resolve_managed_codex_signal(
    teams_dir: &Path,
    signal: &CompactionSignalRecord,
) -> Option<ResolvedManagedCodexSignal> {
    let normalized_project = normalize_project_path(&signal.project_path);

    let team_names = TeamConfigStore::list(teams_dir).ok()?;
    let mut best_match: Option<ResolvedManagedCodexSignal> = None;
    let mut best_score = 0u8;
    let mut ambiguous = false;

    for team_name in team_names {
        let roster = match get_team_roster_with_attachments(teams_dir, &team_name) {
            Ok(roster) => roster,
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team roster while resolving compaction signal");
                continue;
            }
        };

        for member in &roster {
            if member.configured_cli_tool != CliTool::Codex {
                continue;
            }
            if normalize_project_path(&member.configured_project_path.display().to_string())
                != normalized_project
            {
                continue;
            }

            let pane_matches = member.pane_id.as_deref() == Some(signal.pane_id.as_str());
            let session_matches = member.session_id.as_deref() == Some(signal.session_id.as_str());
            let score = match (session_matches, pane_matches) {
                (true, true) => 4,
                (true, false) => 3,
                (false, true) => 2,
                (false, false) => 0,
            };
            if score == 0 {
                continue;
            }

            let Some(mut runtime) = member.runtime_record() else {
                continue;
            };

            let mut changed = false;
            if runtime.cli_tool.is_none() {
                runtime.cli_tool = Some(member.configured_cli_tool);
                changed = true;
            }
            if runtime.project_path.is_none() {
                runtime.project_path = Some(member.configured_project_path.clone());
                changed = true;
            }
            if runtime.jsonl_path.is_none() {
                runtime.jsonl_path = Some(signal.jsonl_path.clone().into());
                changed = true;
            }
            if changed {
                let _ =
                    MemberRuntimeStore::save(teams_dir, &team_name, &member.member_name, &runtime);
            }

            let snapshot = match OperationalContextSnapshotStore::load(
                teams_dir,
                &team_name,
                &member.member_name,
            ) {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(
                        team_name = team_name,
                        member_name = member.member_name,
                        error = %error,
                        "failed to load operational snapshot while resolving compaction signal"
                    );
                    continue;
                }
            };

            let resolved = ResolvedManagedCodexSignal {
                team_name: team_name.clone(),
                member_name: member.member_name.clone(),
                member: member.configured_member(),
                snapshot,
            };

            let candidate_activity = member.latest_runtime_activity();
            let best_activity = best_match
                .as_ref()
                .and_then(|current| {
                    roster
                        .iter()
                        .find(|view| view.member_name == current.member_name)
                })
                .and_then(|view| view.latest_runtime_activity());

            if score > best_score {
                best_score = score;
                best_match = Some(resolved);
                ambiguous = false;
            } else if score == best_score {
                match (candidate_activity, best_activity) {
                    (Some(candidate), Some(current)) if candidate > current => {
                        best_match = Some(resolved);
                        ambiguous = false;
                    }
                    (Some(candidate), Some(current)) if candidate < current => {}
                    _ => ambiguous = true,
                }
            }
        }
    }

    if ambiguous {
        None
    } else {
        best_match
    }
}

fn already_handled(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
) -> bool {
    match MemberCompactionStore::load(teams_dir, team_name, member_name) {
        Ok(Some(state)) => {
            state.last_session_id == session_id
                && state.last_compaction_timestamp == compaction_timestamp
        }
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(
                team_name = team_name,
                member_name = member_name,
                error = %error,
                "failed to load compaction state while resolving idempotency"
            );
            false
        }
    }
}

fn member_is_still_attached(
    teams_dir: &Path,
    pane_id: &str,
    resolved: &ResolvedManagedCodexSignal,
) -> bool {
    let roster = match get_team_roster_with_attachments(teams_dir, &resolved.team_name) {
        Ok(roster) => roster,
        Err(error) => {
            tracing::warn!(
                team_name = resolved.team_name,
                member_name = resolved.member_name,
                error = %error,
                "failed to load team roster while validating compaction signal delivery"
            );
            return false;
        }
    };

    let Some(member) = roster
        .iter()
        .find(|member| member.member_name == resolved.member_name)
    else {
        return false;
    };

    if member.configured_cli_tool != CliTool::Codex {
        return false;
    }

    member.pane_id.as_deref() == Some(pane_id)
}

fn delivery_skip_reason(
    teams_dir: &Path,
    resolved: &ResolvedManagedCodexSignal,
    runtime: &dyn CoordinationRuntime,
    pane_id: &str,
) -> Option<&'static str> {
    if !member_is_still_attached(teams_dir, pane_id, resolved) {
        return Some(SKIP_REASON_MEMBER_NOT_ATTACHED);
    }

    if runtime
        .pane_exists(pane_id)
        .ok()
        .filter(|exists| *exists)
        .is_none()
    {
        return Some(SKIP_REASON_MEMBER_NOT_ATTACHED);
    }
    if runtime.pane_is_dead(pane_id).unwrap_or(true) {
        return Some(SKIP_REASON_MEMBER_NOT_ATTACHED);
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn record_delivery_at(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
    skip_reason: Option<&str>,
    fail_reason: Option<&str>,
) -> Result<(), crate::coordination::errors::CoordinationError> {
    if !should_persist_delivery_state(teams_dir, team_name, member_name)? {
        tracing::debug!(
            team_name = team_name,
            member_name = member_name,
            session_id = session_id,
            result = ?result,
            "skipping compaction bookkeeping because team/member no longer exists"
        );
        return Ok(());
    }

    MemberCompactionStore::save(
        teams_dir,
        team_name,
        member_name,
        &MemberCompactionState {
            version: 1,
            member_name: member_name.to_string(),
            last_session_id: session_id.to_string(),
            last_compaction_timestamp: compaction_timestamp,
            last_delivery_result: result,
        },
    )?;
    emit_compaction_delivery_event(
        team_name,
        member_name,
        CliTool::Codex,
        session_id,
        compaction_timestamp,
        result,
        skip_reason,
        fail_reason,
    );
    Ok(())
}

fn should_persist_delivery_state(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> Result<bool, crate::coordination::errors::CoordinationError> {
    let config = match TeamConfigStore::load(teams_dir, team_name) {
        Ok(config) => config,
        Err(crate::coordination::errors::CoordinationError::NotFound(_)) => return Ok(false),
        Err(error) => return Err(error),
    };

    Ok(config
        .members
        .iter()
        .any(|member| member.name == member_name && member.cli_tool == CliTool::Codex))
}

fn event_signal_kind(kind: CompactionSignalKind) -> EventSignalKind {
    match kind {
        CompactionSignalKind::Compacted => EventSignalKind::Compacted,
        CompactionSignalKind::ContextCompacted => EventSignalKind::ContextCompacted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::TempDir;

    use crate::coordination::domain::{HealthState, MemberRole};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::coordination::stores::{
        CompactionSignalKind, MemberRuntimeRecord, OperationalAssignmentFooterSnapshot,
        OperationalOwnershipSnapshot, OperationalWorkingSetSnapshot, TeamConfig,
    };

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_member(name: &str, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: Some(format!("{name}-role")),
            role_name: Some(format!("{name} role")),
            focus_area: Some("Keep task execution aligned".to_string()),
            context_summary: Some("Maintains project context".to_string()),
            behavior_summary: Some("Stay concrete and report blockers".to_string()),
            communication_style: None,
            runtime_compact_summary: None,
            instructions: Some("Implement assigned work".to_string()),
            behavioral_contract: None,
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: None,
            model: None,
            reasoning_effort: None,
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
        }
    }

    fn sample_snapshot(
        team_name: &str,
        member_name: &str,
        project_path: &str,
    ) -> OperationalContextSnapshot {
        OperationalContextSnapshot {
            version: 1,
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            updated_at: timestamp("2026-03-08T14:10:00Z"),
            task: crate::coordination::stores::OperationalTaskSnapshot {
                id: "678".to_string(),
                subject: "Implement Codex compaction watcher".to_string(),
                status: "in_progress".to_string(),
            },
            assignment_footer: OperationalAssignmentFooterSnapshot {
                execution_mode: "implement".to_string(),
                file_ownership_boundary: vec![
                    "src-tauri/src/session_scanner/compaction.rs".to_string()
                ],
                adjacent_fix_policy: "local validation only".to_string(),
                validation_expectation: "cargo check --tests".to_string(),
                response_expectation: "report-on-completion".to_string(),
            },
            ownership: OperationalOwnershipSnapshot {
                override_allowed: false,
                active_override_reason: None,
            },
            working_set: OperationalWorkingSetSnapshot {
                project_path: project_path.to_string(),
                focal_files: vec!["src-tauri/src/session_scanner/compaction.rs".to_string()],
            },
        }
    }

    fn save_team_fixture(
        teams_dir: &Path,
        team_name: &str,
        member: &Member,
        runtime_session_id: Option<&str>,
        runtime_pane_id: Option<&str>,
    ) {
        let config = TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: timestamp("2026-03-08T14:00:00Z"),
            members: vec![member.clone()],
            extra: Default::default(),
        };
        TeamConfigStore::save(teams_dir, team_name, &config).expect("save team config");

        let runtime = MemberRuntimeRecord {
            schema_version: 3,
            member_name: member.name.clone(),
            cli_tool: Some(member.cli_tool),
            project_path: Some(member.project_path.clone()),
            pane_id: runtime_pane_id.map(ToOwned::to_owned),
            session_id: runtime_session_id.map(ToOwned::to_owned),
            jsonl_path: None,
            daemon_pid: Some(42),
            health: HealthState::Healthy,
            delivery_lease: None,
            attached_at: Some(timestamp("2026-03-08T14:01:00Z")),
            last_seen_at: Some(timestamp("2026-03-08T14:02:00Z")),
        };
        MemberRuntimeStore::save(teams_dir, team_name, &member.name, &runtime)
            .expect("save runtime");
        OperationalContextSnapshotStore::save(
            teams_dir,
            &sample_snapshot(
                team_name,
                &member.name,
                &member.project_path.display().to_string(),
            ),
        )
        .expect("save snapshot");
    }

    fn sample_signal(
        project_path: &str,
        jsonl_path: &Path,
        session_id: &str,
        pane_id: &str,
    ) -> CompactionSignalRecord {
        CompactionSignalRecord {
            version: 1,
            signal_id: "signal-1".to_string(),
            emitted_at: timestamp("2026-03-08T13:46:41.100Z"),
            tool: CliTool::Codex,
            session_id: session_id.to_string(),
            pane_id: pane_id.to_string(),
            project_path: project_path.to_string(),
            jsonl_path: jsonl_path.display().to_string(),
            jsonl_offset: std::fs::metadata(jsonl_path).expect("jsonl metadata").len(),
            transcript_timestamp: timestamp("2026-03-08T13:46:41.037Z"),
            signal_kind: CompactionSignalKind::Compacted,
        }
    }

    #[test]
    fn compaction_card_and_operator_notice_share_the_same_inbox_record_contract() {
        // Regression: mesh-findings P1/H2; compaction and operator delivery had
        // independent inbox writers and could drift in sender or wire fields.
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "wire-parity";
        let member_name = "developer2";
        let now = timestamp("2026-03-08T14:10:05Z");
        let member = sample_member(member_name, "/tmp/project");
        let snapshot = sample_snapshot(team_name, member_name, "/tmp/project");
        let card = CompactionReinjectionService::compose_at(&member, &snapshot, now);
        let rendered =
            CompactionReinjectionService::render_codex_inbox_text(&card).expect("render card");

        append_codex_inbox_message(teams_dir, team_name, member_name, &card, now)
            .expect("append compaction card");
        let compaction = MeshInboxStore::load(teams_dir, team_name, member_name)
            .expect("load compaction")
            .remove(0);
        let mut operator = MeshInboxMessage::operator_originated(
            member_name,
            rendered,
            Some("operator_notice".to_string()),
            now,
            None,
        );
        operator.id.clone_from(&compaction.id);
        MeshInboxStore::append(teams_dir, team_name, member_name, &operator)
            .expect("append operator notice");

        let records = MeshInboxStore::load(teams_dir, team_name, member_name).expect("load inbox");
        let mut compaction_value = serde_json::to_value(&records[0]).expect("serialize compaction");
        let mut operator_value = serde_json::to_value(&records[1]).expect("serialize operator");
        compaction_value
            .as_object_mut()
            .expect("compaction object")
            .remove("summary");
        operator_value
            .as_object_mut()
            .expect("operator object")
            .remove("summary");
        assert_eq!(compaction_value, operator_value);
        assert_eq!(
            records[0].from,
            crate::coordination::stores::OPERATOR_SENDER_NAME
        );
        assert_eq!(
            records[1].from,
            crate::coordination::stores::OPERATOR_SENDER_NAME
        );
    }

    #[test]
    fn process_signal_injects_for_attached_live_member() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/user/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Injected {
                team_name: "taurhaus-team".to_string(),
                member_name: "developer2".to_string(),
            }
        );

        let inbox =
            MeshInboxStore::load(&teams_dir, "taurhaus-team", "developer2").expect("load inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].summary.as_deref(), Some("post_compaction_context"));

        let runtime =
            MemberRuntimeStore::load(&teams_dir, "taurhaus-team", "developer2").expect("runtime");
        assert_eq!(runtime.jsonl_path.as_deref(), Some(jsonl_path.as_path()));
    }

    #[test]
    fn process_signal_returns_unresolved_when_no_managed_member_matches() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(
            "/home/user/projects/taurhaus",
            &jsonl_path,
            "session-1",
            "%7",
        );

        let runtime = RecordingCoordinationRuntime::default();
        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Unresolved {
                reason: CompactionUnresolvedReason::ManagedMemberResolutionUnavailable,
            }
        );
    }

    #[test]
    fn process_signal_injects_even_when_jsonl_grows_after_compaction() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/user/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let mut signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");
        signal.jsonl_offset = 1;

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Injected {
                team_name: "taurhaus-team".to_string(),
                member_name: "developer2".to_string(),
            }
        );
    }

    #[test]
    fn process_signal_resolves_shared_session_by_matching_pane() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/user/projects/taurhaus";
        let pane_match = sample_member("pane-match", project_path);
        let other_match = sample_member("other-match", project_path);
        TeamConfigStore::save(
            &teams_dir,
            "taurhaus-team",
            &TeamConfig {
                schema_version: 1,
                name: "taurhaus-team".to_string(),
                description: None,
                created_at: timestamp("2026-03-08T14:00:00Z"),
                members: vec![pane_match.clone(), other_match.clone()],
                extra: Default::default(),
            },
        )
        .expect("save team config");
        for (member, pane_id) in [(&pane_match, "%7"), (&other_match, "%9")] {
            let runtime = MemberRuntimeRecord {
                schema_version: 3,
                member_name: member.name.clone(),
                cli_tool: Some(member.cli_tool),
                project_path: Some(member.project_path.clone()),
                pane_id: Some(pane_id.to_string()),
                session_id: Some("session-1".to_string()),
                jsonl_path: None,
                daemon_pid: Some(42),
                health: HealthState::Healthy,
                delivery_lease: None,
                attached_at: Some(timestamp("2026-03-08T14:01:00Z")),
                last_seen_at: Some(timestamp("2026-03-08T14:02:00Z")),
            };
            MemberRuntimeStore::save(&teams_dir, "taurhaus-team", &member.name, &runtime)
                .expect("save runtime");
            OperationalContextSnapshotStore::save(
                &teams_dir,
                &sample_snapshot("taurhaus-team", &member.name, project_path),
            )
            .expect("save snapshot");
        }
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Injected {
                team_name: "taurhaus-team".to_string(),
                member_name: "pane-match".to_string(),
            }
        );
    }

    #[test]
    fn process_signal_marks_duplicate_as_skipped() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/user/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");
        MemberCompactionStore::save(
            &teams_dir,
            "taurhaus-team",
            "developer2",
            &MemberCompactionState {
                version: 1,
                member_name: "developer2".to_string(),
                last_session_id: "session-1".to_string(),
                last_compaction_timestamp: signal.transcript_timestamp,
                last_delivery_result: CompactionDeliveryResult::Injected,
            },
        )
        .expect("save state");

        let runtime = RecordingCoordinationRuntime::default();
        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Skipped {
                team_name: "taurhaus-team".to_string(),
                member_name: "developer2".to_string(),
            }
        );
    }

    #[test]
    fn process_signal_injects_when_pane_is_alive_but_foreground_command_is_not_codex() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/user/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("cargo test"));

        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Injected {
                team_name: "taurhaus-team".to_string(),
                member_name: "developer2".to_string(),
            }
        );
    }

    #[test]
    fn process_signal_skips_when_snapshot_task_is_completed() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/user/projects/taurhaus";
        let member = sample_member("developer2", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &member,
            Some("session-1"),
            Some("%7"),
        );
        let mut snapshot = sample_snapshot("taurhaus-team", "developer2", project_path);
        snapshot.task.status = "completed".to_string();
        OperationalContextSnapshotStore::save(&teams_dir, &snapshot).expect("save snapshot");

        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);

        let outcome = CompactionSignalProcessor::process_signal_at(
            &signal,
            &teams_dir,
            &runtime,
            timestamp("2026-03-08T13:46:42Z"),
        );

        assert_eq!(
            outcome,
            CompactionSignalProcessOutcome::Skipped {
                team_name: "taurhaus-team".to_string(),
                member_name: "developer2".to_string(),
            }
        );

        let inbox =
            MeshInboxStore::load(&teams_dir, "taurhaus-team", "developer2").expect("load inbox");
        assert!(inbox.is_empty());

        let state = MemberCompactionStore::load(&teams_dir, "taurhaus-team", "developer2")
            .expect("load compaction state")
            .expect("state exists");
        assert_eq!(
            state.last_delivery_result,
            CompactionDeliveryResult::Skipped
        );
    }
}
