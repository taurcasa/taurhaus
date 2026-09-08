//! Post-compaction task admission and current lease context.

use crate::coordination::stores::operational::OperationalContextSnapshot;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const MAX_LEASE_RECORD_BYTES: u64 = 1_048_576;
/// Inbox summary the mesh member sees for a queued post-compaction card.
pub const POST_COMPACTION_INBOX_SUMMARY: &str = "post_compaction_context";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionLeases {
    pub held: Vec<String>,
    pub waiting: Vec<OperationalReinjectionLeaseWait>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OperationalReinjectionLeaseWait {
    pub name: String,
    pub position: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MeshLeaseRecord {
    state: String,
    holder: Option<String>,
    #[serde(default)]
    waiters: Vec<MeshLeaseWaiter>,
}

#[derive(Debug, Deserialize)]
struct MeshLeaseWaiter {
    name: String,
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

    pub fn append_member_lease_context(
        rendered: &mut String,
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) {
        let leases = load_member_leases(teams_dir, team_name, member_name);
        if let Some(lease_line) = render_lease_context_line(&leases) {
            rendered.push_str("\n\n");
            rendered.push_str(&lease_line);
        }
    }
}

pub(crate) fn render_lease_context_line(leases: &OperationalReinjectionLeases) -> Option<String> {
    let held = (!leases.held.is_empty()).then(|| format!("held {}", leases.held.join(", ")));
    let waiting = (!leases.waiting.is_empty()).then(|| {
        format!(
            "waiting {}",
            leases
                .waiting
                .iter()
                .map(|wait| format!("#{} for {}", wait.position, wait.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    });

    match (held, waiting) {
        (Some(held), Some(waiting)) => Some(format!("Leases: {held}; {waiting}.")),
        (Some(held), None) => Some(format!("Leases: {held}.")),
        (None, Some(waiting)) => Some(format!("Leases: {waiting}.")),
        (None, None) => None,
    }
}

fn load_member_leases(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> OperationalReinjectionLeases {
    let leases_dir = teams_dir.join(team_name).join("state").join("leases");
    let entries = match fs::read_dir(&leases_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return OperationalReinjectionLeases::default();
        }
        Err(error) => {
            tracing::warn!(
                path = %leases_dir.display(),
                error = %error,
                "skipping unavailable mesh leases directory while composing member context"
            );
            return OperationalReinjectionLeases::default();
        }
    };

    let mut leases = OperationalReinjectionLeases::default();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(
                    path = %leases_dir.display(),
                    error = %error,
                    "skipping unreadable mesh lease directory entry while composing member context"
                );
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        match fs::metadata(&path) {
            Ok(metadata) if metadata.len() > MAX_LEASE_RECORD_BYTES => {
                tracing::warn!(
                    path = %path.display(),
                    bytes = metadata.len(),
                    max_bytes = MAX_LEASE_RECORD_BYTES,
                    "skipping oversized mesh lease record while composing member context"
                );
                continue;
            }
            // A zero-byte file is mesh's own first-acquire transient
            // (acquire_or_create touches the file before the first atomic
            // write); mesh's reader skips it silently and so do we.
            Ok(metadata) if metadata.len() == 0 => continue,
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "skipping unreadable mesh lease record while composing member context"
                );
                continue;
            }
        }
        let record = match fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|raw| {
                serde_json::from_slice::<MeshLeaseRecord>(&raw).map_err(|error| error.to_string())
            }) {
            Ok(record) => record,
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "skipping unreadable mesh lease record while composing member context"
                );
                continue;
            }
        };

        let Some(lease_name) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            tracing::warn!(
                path = %path.display(),
                "skipping mesh lease record with an empty name while composing member context"
            );
            continue;
        };
        if record.state == "held" && record.holder.as_deref() == Some(member_name) {
            leases.held.push(lease_name.to_string());
        }
        if let Some(position) = record
            .waiters
            .iter()
            .position(|waiter| waiter.name == member_name)
        {
            leases.waiting.push(OperationalReinjectionLeaseWait {
                name: lease_name.to_string(),
                position: position + 1,
            });
        }
    }

    leases.held.sort();
    leases.waiting.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.position.cmp(&right.position))
    });
    leases
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::stores::operational::{
        OperationalAssignmentFooterSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot,
    };
    use chrono::{DateTime, Utc};

    fn sample_snapshot() -> OperationalContextSnapshot {
        OperationalContextSnapshot {
            recovery_card: None,
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

    #[test]
    fn lease_context_lists_held_leases_and_waiting_positions() {
        // Regression: commit 4c08860d trusted the JSON body's name instead of
        // the filename key and read arbitrarily large mesh lease records.
        let tmp = tempfile::tempdir().expect("temp teams dir");
        let leases_dir = tmp
            .path()
            .join("taurhaus-team")
            .join("state")
            .join("leases");
        fs::create_dir_all(&leases_dir).expect("create leases dir");
        fs::write(
            leases_dir.join("delivery-renderer.json"),
            r#"{
                "name": "delivery-renderer",
                "state": "held",
                "holder": "architect",
                "waiters": [],
                "taskId": "673",
                "scope": "onboarding",
                "futureField": true
            }"#,
        )
        .expect("write held lease");
        fs::write(
            leases_dir.join("shared-card.json"),
            r#"{
                "name": "shared-card",
                "state": "held",
                "holder": "another-member",
                "waiters": [
                    {"name": "first-waiter", "since": "2026-09-02T10:00:00Z"},
                    {"name": "architect", "since": "2026-09-02T10:01:00Z"}
                ]
            }"#,
        )
        .expect("write waiting lease");
        fs::write(
            leases_dir.join("canonical-seam.json"),
            r#"{
                "name": "misleading-body-name",
                "state": "held",
                "holder": "architect",
                "waiters": []
            }"#,
        )
        .expect("write filename-keyed lease");
        let oversized_record = format!(
            r#"{{
                "name": "oversized",
                "state": "held",
                "holder": "architect",
                "waiters": [],
                "padding": "{}"
            }}"#,
            "x".repeat(1_048_576)
        );
        fs::write(leases_dir.join("oversized.json"), oversized_record)
            .expect("write oversized lease");
        fs::write(leases_dir.join("unreadable.json"), "not json").expect("write malformed lease");

        let leases = load_member_leases(tmp.path(), "taurhaus-team", "architect");

        assert_eq!(leases.held, vec!["canonical-seam", "delivery-renderer"]);
        assert_eq!(
            leases.waiting,
            vec![OperationalReinjectionLeaseWait {
                name: "shared-card".to_string(),
                position: 2,
            }]
        );
        let mut rendered = String::new();
        CompactionReinjectionService::append_member_lease_context(
            &mut rendered,
            tmp.path(),
            "taurhaus-team",
            "architect",
        );
        assert!(rendered.contains(
            "Leases: held canonical-seam, delivery-renderer; waiting #2 for shared-card."
        ));
    }

    #[test]
    fn absent_or_unreadable_leases_dir_leaves_card_unchanged() {
        let tmp = tempfile::tempdir().expect("temp teams dir");
        let leases_path = tmp.path().join("taurhaus-team/state/leases");
        for unreadable in [false, true] {
            if unreadable {
                fs::create_dir_all(leases_path.parent().unwrap()).unwrap();
                fs::write(&leases_path, "not a directory").unwrap();
            }
            let mut rendered = "Current recovery card".to_string();
            CompactionReinjectionService::append_member_lease_context(
                &mut rendered,
                tmp.path(),
                "taurhaus-team",
                "architect",
            );
            assert_eq!(rendered, "Current recovery card");
        }
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
