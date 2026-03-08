//! Downstream compaction processing from canonical signal records.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::coordination::compaction_events::{
    emit_compaction_signal_consumed, emit_compaction_unresolved, signal_event,
    CompactionSignalKind as EventSignalKind, CompactionUnresolvedEvent, CompactionUnresolvedReason,
};
use crate::coordination::domain::Member;
use crate::coordination::reinjection::{CompactionReinjectionService, OperationalReinjectionCard};
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::{
    emit_compaction_delivery_event, emit_compaction_detected_event, is_stale_compaction,
    CompactionDeliveryResult, CompactionSignalKind, CompactionSignalRecord, MemberCompactionState,
    MemberCompactionStore, MemberRuntimeRecord, MemberRuntimeStore, MeshInboxMessage,
    MeshInboxStore, OperationalContextSnapshot, OperationalContextSnapshotStore, TeamConfigStore,
};
use crate::provider::path::normalize_project_path;
use crate::session_scanner::cli_tool::CliTool;

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
        let teams_dir = crate::coordination::stores::operational::default_operational_teams_dir();
        Self::process_signal_at(signal, &teams_dir, &runtime, Utc::now())
    }

    pub fn process_signal_at(
        signal: &CompactionSignalRecord,
        teams_dir: &Path,
        runtime: &dyn CoordinationRuntime,
        now: DateTime<Utc>,
    ) -> CompactionSignalProcessOutcome {
        emit_compaction_signal_consumed(signal_event(
            signal.tool,
            Some(&signal.session_id),
            Some(&signal.pane_id),
            Some(&signal.project_path),
            Some(Path::new(&signal.jsonl_path)),
            Some(signal.transcript_timestamp),
            Some(event_signal_kind(signal.signal_kind)),
        ));

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
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: error.to_string(),
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
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: error.to_string(),
                };
            }
            return CompactionSignalProcessOutcome::Skipped {
                team_name: resolved.team_name,
                member_name: resolved.member_name,
            };
        }

        if !member_is_still_attached(teams_dir, signal, &resolved)
            || !jsonl_prompt_boundary_is_unchanged(signal)
            || !pane_is_live_codex(runtime, &signal.pane_id)
        {
            if let Err(error) = record_delivery_at(
                teams_dir,
                &resolved.team_name,
                &resolved.member_name,
                &signal.session_id,
                signal.transcript_timestamp,
                CompactionDeliveryResult::Skipped,
            ) {
                return CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: error.to_string(),
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
                ) {
                    return CompactionSignalProcessOutcome::Failed {
                        team_name: resolved.team_name,
                        member_name: resolved.member_name,
                        error_message: error.to_string(),
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
                );
                CompactionSignalProcessOutcome::Failed {
                    team_name: resolved.team_name,
                    member_name: resolved.member_name,
                    error_message: error.to_string(),
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
    let rendered_text =
        CompactionReinjectionService::render_codex_inbox_text(card).map_err(|error| {
            crate::coordination::errors::CoordinationError::StoreError(format!(
                "failed to serialize Codex post-compaction card for '{}' in '{}': {error}",
                member_name, team_name
            ))
        })?;
    let inbox_message = MeshInboxMessage::new(
        "taurhaus",
        rendered_text,
        Some("post_compaction_context".to_string()),
        now,
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
        let config = match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team config while resolving compaction signal");
                continue;
            }
        };
        let runtime_by_member = match MemberRuntimeStore::load_all(teams_dir, &team_name) {
            Ok(records) => records.into_iter().collect::<HashMap<_, _>>(),
            Err(error) => {
                tracing::warn!(team_name = team_name, error = %error, "failed to load team runtime while resolving compaction signal");
                continue;
            }
        };

        for member in config.members {
            if member.cli_tool != CliTool::Codex {
                continue;
            }
            if normalize_project_path(&member.project_path.display().to_string())
                != normalized_project
            {
                continue;
            }

            let Some(mut runtime) = runtime_by_member.get(&member.name).cloned() else {
                continue;
            };

            let mut changed = false;
            if runtime.cli_tool.is_none() {
                runtime.cli_tool = Some(member.cli_tool);
                changed = true;
            }
            if runtime.project_path.is_none() {
                runtime.project_path = Some(member.project_path.clone());
                changed = true;
            }
            if runtime.jsonl_path.is_none() {
                runtime.jsonl_path = Some(signal.jsonl_path.clone().into());
                changed = true;
            }
            if changed {
                let _ = MemberRuntimeStore::save(teams_dir, &team_name, &member.name, &runtime);
            }

            let runtime_session = runtime.session_id.as_deref();
            let runtime_pane = runtime.pane_id.as_deref();
            let pane_matches = runtime_pane == Some(signal.pane_id.as_str());
            let session_matches = runtime_session == Some(signal.session_id.as_str());

            let score = match (session_matches, pane_matches) {
                (true, true) => 4,
                (true, false) => 3,
                (false, true) => 2,
                (false, false) => 0,
            };
            if score == 0 {
                continue;
            }

            let snapshot =
                match OperationalContextSnapshotStore::load(teams_dir, &team_name, &member.name) {
                    Ok(Some(snapshot)) => snapshot,
                    Ok(None) => continue,
                    Err(error) => {
                        tracing::warn!(
                            team_name = team_name,
                            member_name = member.name,
                            error = %error,
                            "failed to load operational snapshot while resolving compaction signal"
                        );
                        continue;
                    }
                };

            let resolved = ResolvedManagedCodexSignal {
                team_name: team_name.clone(),
                member_name: member.name.clone(),
                member,
                snapshot,
            };

            let candidate_activity = latest_runtime_activity(&runtime);
            let best_activity = best_match.as_ref().and_then(|current| {
                runtime_by_member
                    .get(&current.member_name)
                    .and_then(latest_runtime_activity)
            });

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

fn latest_runtime_activity(record: &MemberRuntimeRecord) -> Option<DateTime<Utc>> {
    record.last_seen_at.or(record.attached_at)
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
    signal: &CompactionSignalRecord,
    resolved: &ResolvedManagedCodexSignal,
) -> bool {
    let config = match TeamConfigStore::load(teams_dir, &resolved.team_name) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(
                team_name = resolved.team_name,
                member_name = resolved.member_name,
                error = %error,
                "failed to load team config while validating compaction signal delivery"
            );
            return false;
        }
    };

    let Some(member) = config
        .members
        .iter()
        .find(|member| member.name == resolved.member_name)
    else {
        return false;
    };

    if member.cli_tool != CliTool::Codex {
        return false;
    }

    match MemberRuntimeStore::load(teams_dir, &resolved.team_name, &resolved.member_name) {
        Ok(runtime) => {
            runtime.pane_id.as_deref() == Some(signal.pane_id.as_str())
                && runtime.session_id.as_deref() == Some(signal.session_id.as_str())
        }
        Err(error) => {
            tracing::warn!(
                team_name = resolved.team_name,
                member_name = resolved.member_name,
                error = %error,
                "failed to load runtime while validating compaction signal delivery"
            );
            false
        }
    }
}

fn jsonl_prompt_boundary_is_unchanged(signal: &CompactionSignalRecord) -> bool {
    match std::fs::metadata(&signal.jsonl_path) {
        Ok(metadata) => metadata.len() == signal.jsonl_offset,
        Err(error) => {
            tracing::warn!(
                path = signal.jsonl_path,
                session_id = signal.session_id,
                error = %error,
                "failed to stat Codex JSONL while validating signal prompt boundary"
            );
            false
        }
    }
}

fn pane_is_live_codex(runtime: &dyn CoordinationRuntime, pane_id: &str) -> bool {
    if runtime
        .pane_exists(pane_id)
        .ok()
        .filter(|exists| *exists)
        .is_none()
    {
        return false;
    }
    if runtime.pane_is_dead(pane_id).unwrap_or(true) {
        return false;
    }

    runtime
        .pane_current_command(pane_id)
        .ok()
        .flatten()
        .as_deref()
        .is_some_and(foreground_command_matches_codex)
}

fn foreground_command_matches_codex(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let first = normalized.split_whitespace().next().unwrap_or_default();
    let first = first.rsplit('/').next().unwrap_or(first);

    first == "codex" || first.ends_with("codex")
}

fn record_delivery_at(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    session_id: &str,
    compaction_timestamp: DateTime<Utc>,
    result: CompactionDeliveryResult,
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
            instructions: Some("Implement assigned work".to_string()),
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
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
    fn process_signal_injects_for_attached_live_member() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
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
        runtime.set_pane_current_command("%7", Some("codex"));

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
            "/home/mstie/projects/taurhaus",
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
    fn process_signal_skips_when_prompt_boundary_changed() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
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
        runtime.set_pane_current_command("%7", Some("codex"));

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
    fn process_signal_resolves_shared_session_by_matching_pane() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        let project_path = "/home/mstie/projects/taurhaus";
        let pane_match = sample_member("pane-match", project_path);
        let other_match = sample_member("other-match", project_path);
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &pane_match,
            Some("session-1"),
            Some("%7"),
        );
        save_team_fixture(
            &teams_dir,
            "taurhaus-team",
            &other_match,
            Some("session-1"),
            Some("%9"),
        );
        let jsonl_path = tmp.path().join("session.jsonl");
        std::fs::write(&jsonl_path, "{\"line\":1}\n").expect("jsonl");
        let signal = sample_signal(project_path, &jsonl_path, "session-1", "%7");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%7", true);
        runtime.set_pane_dead("%7", false);
        runtime.set_pane_current_command("%7", Some("codex"));

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
        let project_path = "/home/mstie/projects/taurhaus";
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
}
