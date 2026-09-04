//! Append-only routing telemetry sidecars.
//!
//! These records are observations only. Callers use the fail-soft wrapper so
//! storage trouble cannot change a launch, effort, deadline, or task outcome.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Error, ErrorKind, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::models::{CapabilityTier, ModelCatalog};
use crate::session_scanner::cli_tool::CliTool;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const UNATTRIBUTED_SIDECAR: &str = "_unattributed";
const MAX_SIDECAR_BYTES: u64 = 8 * 1_048_576;
static WRITE_FAILURE_REPORTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffortSwitchOutcome {
    Started,
    Completed,
    Failed,
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RoutingTelemetryEvent {
    LaunchRendered {
        timestamp: DateTime<Utc>,
        task_id: Option<String>,
        member: String,
        role: String,
        tool: String,
        model: Option<String>,
        applied_effort: Option<String>,
        capability_tier: Option<String>,
        tier_rank: Option<u32>,
    },
    EffortSwitch {
        timestamp: DateTime<Utc>,
        task_id: String,
        member: String,
        attempt: u32,
        from_effort: Option<String>,
        to_effort: String,
        outcome: EffortSwitchOutcome,
    },
    NudgeSent {
        timestamp: DateTime<Utc>,
        task_id: String,
        member: String,
        deadline_minutes: u32,
    },
    TaskStaled {
        timestamp: DateTime<Utc>,
        task_id: String,
        member: String,
        deadline_minutes: u32,
    },
    CompletionObserved {
        timestamp: DateTime<Utc>,
        task_id: String,
        status: String,
        has_review_ruling: bool,
    },
}

pub fn append_task_telemetry(
    teams_dir: &Path,
    team_name: &str,
    task_id: Option<&str>,
    event: &RoutingTelemetryEvent,
) -> std::io::Result<()> {
    let path = task_telemetry_path(teams_dir, team_name, task_id)?;
    let mut file = open_sidecar(&path)?;
    file.lock_exclusive()?;
    append_locked(&mut file, event)?;
    FileExt::unlock(&file)
}

/// Observe a terminal task once per `(status, ruling-presence)` state. A later
/// ruling on an already-completed task is a new observation because it changes
/// whether Amendment 4 permits acceptance.
pub fn record_completion_observed(
    teams_dir: &Path,
    team_name: &str,
    task_id: &str,
    status: &str,
    has_review_ruling: bool,
) {
    let result = (|| -> std::io::Result<()> {
        let path = task_telemetry_path(teams_dir, team_name, Some(task_id))?;
        let Some(mut file) = open_existing_sidecar(&path)? else {
            return Ok(());
        };
        file.lock_exclusive()?;
        let Some(events) = read_locked_task_telemetry(&mut file)? else {
            FileExt::unlock(&file)?;
            return Ok(());
        };
        let duplicate = events.iter().any(|event| {
            matches!(
                event,
                RoutingTelemetryEvent::CompletionObserved {
                    status: observed_status,
                    has_review_ruling: observed_ruling,
                    ..
                } if observed_status == status && *observed_ruling == has_review_ruling
            )
        });
        if !duplicate {
            append_locked(
                &mut file,
                &RoutingTelemetryEvent::CompletionObserved {
                    timestamp: Utc::now(),
                    task_id: task_id.to_string(),
                    status: status.to_string(),
                    has_review_ruling,
                },
            )?;
        }
        FileExt::unlock(&file)
    })();
    if let Err(error) = result {
        report_write_failure(team_name, Some(task_id), &error);
    }
}

/// Attribute the latest rendered launch observed for `member` when a
/// daemon-owned operational snapshot first names a task. The render fields
/// remain authoritative; the new timestamp marks when ownership was observed.
pub fn attribute_latest_launch_to_task(
    teams_dir: &Path,
    team_name: &str,
    task_id: &str,
    member: &str,
) {
    let result = (|| -> std::io::Result<()> {
        let task_path = task_telemetry_path(teams_dir, team_name, Some(task_id))?;
        if read_task_telemetry(&task_path).iter().any(|event| {
            matches!(
                event,
                RoutingTelemetryEvent::LaunchRendered {
                    member: launched_member,
                    ..
                } if launched_member == member
            )
        }) {
            return Ok(());
        }

        let telemetry_dir = task_path
            .parent()
            .expect("task telemetry path has a parent");
        let entries = match fs::read_dir(telemetry_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        let latest = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
            .flat_map(|path| read_task_telemetry(&path))
            .filter_map(|event| match event {
                RoutingTelemetryEvent::LaunchRendered {
                    timestamp,
                    member: launched_member,
                    role,
                    tool,
                    model,
                    applied_effort,
                    capability_tier,
                    tier_rank,
                    ..
                } if launched_member == member => Some((
                    timestamp,
                    role,
                    tool,
                    model,
                    applied_effort,
                    capability_tier,
                    tier_rank,
                )),
                _ => None,
            })
            .max_by_key(|(timestamp, ..)| *timestamp);
        let Some((_, role, tool, model, applied_effort, capability_tier, tier_rank)) = latest
        else {
            return Ok(());
        };
        append_task_telemetry(
            teams_dir,
            team_name,
            Some(task_id),
            &RoutingTelemetryEvent::LaunchRendered {
                timestamp: Utc::now(),
                task_id: Some(task_id.to_string()),
                member: member.to_string(),
                role,
                tool,
                model,
                applied_effort,
                capability_tier,
                tier_rank,
            },
        )
    })();
    if let Err(error) = result {
        report_write_failure(team_name, Some(task_id), &error);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn record_launch_rendered(
    teams_dir: &Path,
    team_name: &str,
    task_id: Option<&str>,
    member: &str,
    role: &str,
    tool: CliTool,
    model: Option<&str>,
    applied_effort: Option<&str>,
) {
    let catalog = model.and_then(|model| ModelCatalog::entry_for(tool, model));
    let event = RoutingTelemetryEvent::LaunchRendered {
        timestamp: Utc::now(),
        task_id: task_id.map(ToString::to_string),
        member: member.to_string(),
        role: role.to_string(),
        tool: tool.to_string(),
        model: model.map(ToString::to_string),
        applied_effort: applied_effort.map(ToString::to_string),
        capability_tier: catalog
            .and_then(|entry| entry.capability_tier)
            .map(capability_tier_name)
            .map(ToString::to_string),
        tier_rank: catalog.and_then(|entry| entry.tier_rank),
    };
    append_task_telemetry_fail_soft(teams_dir, team_name, task_id, &event);
}

fn capability_tier_name(tier: CapabilityTier) -> &'static str {
    match tier {
        CapabilityTier::Frontier => "frontier",
        CapabilityTier::Strong => "strong",
        CapabilityTier::Efficient => "efficient",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn record_effort_switch(
    teams_dir: &Path,
    team_name: &str,
    task_id: &str,
    member: &str,
    attempt: u32,
    from_effort: Option<&str>,
    to_effort: &str,
    outcome: EffortSwitchOutcome,
) {
    append_task_telemetry_fail_soft(
        teams_dir,
        team_name,
        Some(task_id),
        &RoutingTelemetryEvent::EffortSwitch {
            timestamp: Utc::now(),
            task_id: task_id.to_string(),
            member: member.to_string(),
            attempt,
            from_effort: from_effort.map(ToString::to_string),
            to_effort: to_effort.to_string(),
            outcome,
        },
    );
}

pub fn record_deadline_action(
    teams_dir: &Path,
    team_name: &str,
    task_id: &str,
    member: &str,
    deadline_minutes: u32,
    staled: bool,
) {
    let event = if staled {
        RoutingTelemetryEvent::TaskStaled {
            timestamp: Utc::now(),
            task_id: task_id.to_string(),
            member: member.to_string(),
            deadline_minutes,
        }
    } else {
        RoutingTelemetryEvent::NudgeSent {
            timestamp: Utc::now(),
            task_id: task_id.to_string(),
            member: member.to_string(),
            deadline_minutes,
        }
    };
    append_task_telemetry_fail_soft(teams_dir, team_name, Some(task_id), &event);
}

/// Record an observation without ever changing the wrapped operation's result.
pub fn append_task_telemetry_fail_soft(
    teams_dir: &Path,
    team_name: &str,
    task_id: Option<&str>,
    event: &RoutingTelemetryEvent,
) {
    if let Err(error) = append_task_telemetry(teams_dir, team_name, task_id, event) {
        report_write_failure(team_name, task_id, &error);
    }
}

fn open_sidecar(path: &Path) -> std::io::Result<File> {
    fs::create_dir_all(path.parent().expect("telemetry path has a parent"))?;
    let mut options = OpenOptions::new();
    options.create(true).append(true).read(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

fn open_existing_sidecar(path: &Path) -> std::io::Result<Option<File>> {
    match OpenOptions::new().append(true).read(true).open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_locked_task_telemetry(
    file: &mut File,
) -> std::io::Result<Option<Vec<RoutingTelemetryEvent>>> {
    if file.metadata()?.len() > MAX_SIDECAR_BYTES {
        return Ok(None);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(Some(
        BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect(),
    ))
}

fn append_locked(file: &mut File, event: &RoutingTelemetryEvent) -> std::io::Result<()> {
    let mut payload = serde_json::to_vec(event).map_err(Error::other)?;
    payload.push(b'\n');
    file.write_all(&payload)?;
    file.sync_data()
}

fn report_write_failure(team_name: &str, task_id: Option<&str>, error: &std::io::Error) {
    if !WRITE_FAILURE_REPORTED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            team = team_name,
            task_id = task_id.unwrap_or(""),
            error = %error,
            "routing telemetry write failed; wrapped operation continues"
        );
    }
}

/// Read every valid JSONL record, skipping corrupt or partially written lines.
pub fn read_task_telemetry(path: &Path) -> Vec<RoutingTelemetryEvent> {
    let Ok(metadata) = fs::metadata(path) else {
        return Vec::new();
    };
    if metadata.len() > MAX_SIDECAR_BYTES {
        return Vec::new();
    }
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

pub fn task_telemetry_path(
    teams_dir: &Path,
    team_name: &str,
    task_id: Option<&str>,
) -> std::io::Result<PathBuf> {
    if !safe_component(team_name) {
        return Err(Error::new(ErrorKind::InvalidInput, "invalid team name"));
    }
    let sidecar = task_id.unwrap_or(UNATTRIBUTED_SIDECAR);
    if !safe_component(sidecar) {
        return Err(Error::new(ErrorKind::InvalidInput, "invalid task id"));
    }
    Ok(teams_dir
        .join(team_name)
        .join("state")
        .join("telemetry")
        .join(format!("{sidecar}.jsonl")))
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{
        append_task_telemetry, attribute_latest_launch_to_task, read_task_telemetry,
        record_completion_observed, record_deadline_action, record_effort_switch,
        record_launch_rendered, EffortSwitchOutcome, RoutingTelemetryEvent,
    };

    #[test]
    fn task_sidecar_appends_events_and_skips_corrupt_lines() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");
        let event = RoutingTelemetryEvent::LaunchRendered {
            timestamp: Utc.with_ymd_and_hms(2026, 9, 4, 10, 0, 0).unwrap(),
            task_id: Some("42".to_string()),
            member: "builder".to_string(),
            role: "rust-developer".to_string(),
            tool: "codex".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            applied_effort: Some("high".to_string()),
            capability_tier: Some("strong".to_string()),
            tier_rank: Some(0),
        };

        append_task_telemetry(&teams_dir, "routing-team", Some("42"), &event)
            .expect("append event");
        let path = teams_dir.join("routing-team/state/telemetry/42.jsonl");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut file| std::io::Write::write_all(&mut file, b"not-json\n"))
            .expect("append corrupt line");
        append_task_telemetry(&teams_dir, "routing-team", Some("42"), &event)
            .expect("append second event");

        assert_eq!(read_task_telemetry(&path), vec![event.clone(), event]);
    }

    #[test]
    fn unattributed_launches_use_a_team_sidecar_with_a_null_task_id() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");
        let event = RoutingTelemetryEvent::LaunchRendered {
            timestamp: Utc.with_ymd_and_hms(2026, 9, 4, 10, 0, 0).unwrap(),
            task_id: None,
            member: "builder".to_string(),
            role: "rust-developer".to_string(),
            tool: "codex".to_string(),
            model: Some("gpt-5.6-sol".to_string()),
            applied_effort: None,
            capability_tier: Some("strong".to_string()),
            tier_rank: Some(0),
        };

        append_task_telemetry(&teams_dir, "routing-team", None, &event)
            .expect("append unattributed event");

        let path = teams_dir.join("routing-team/state/telemetry/_unattributed.jsonl");
        assert_eq!(read_task_telemetry(&path), vec![event]);
    }

    // Regression: c9c6c49b searched only `_unattributed.jsonl`, so a member's
    // later task vanished when its reusable launch had first named another task.
    #[test]
    fn attribution_finds_a_members_launch_recorded_under_an_earlier_task() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");
        let launched_at = Utc.with_ymd_and_hms(2026, 9, 4, 10, 0, 0).unwrap();
        append_task_telemetry(
            &teams_dir,
            "routing-team",
            Some("task-a"),
            &RoutingTelemetryEvent::LaunchRendered {
                timestamp: launched_at,
                task_id: Some("task-a".to_string()),
                member: "builder".to_string(),
                role: "rust-developer".to_string(),
                tool: "codex".to_string(),
                model: Some("gpt-5.6-sol".to_string()),
                applied_effort: Some("high".to_string()),
                capability_tier: Some("strong".to_string()),
                tier_rank: Some(0),
            },
        )
        .expect("record task A launch");

        attribute_latest_launch_to_task(
            &teams_dir,
            "routing-team",
            "task-b",
            "builder",
        );

        let attributed = read_task_telemetry(
            &teams_dir.join("routing-team/state/telemetry/task-b.jsonl"),
        );
        assert!(matches!(
            attributed.as_slice(),
            [RoutingTelemetryEvent::LaunchRendered {
                task_id: Some(task_id),
                member,
                model: Some(model),
                ..
            }] if task_id == "task-b" && member == "builder" && model == "gpt-5.6-sol"
        ));
    }

    // Regression: c9c6c49b limited attribution to one sidecar, allowing an
    // older render to win after the member relaunched on a different model.
    #[test]
    fn attribution_uses_the_newest_render_across_task_sidecars() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");
        for (task_id, hour, model) in [
            ("task-a", 9, "gpt-5.6-luna"),
            ("task-c", 10, "gpt-5.6-sol"),
        ] {
            append_task_telemetry(
                &teams_dir,
                "routing-team",
                Some(task_id),
                &RoutingTelemetryEvent::LaunchRendered {
                    timestamp: Utc.with_ymd_and_hms(2026, 9, 4, hour, 0, 0).unwrap(),
                    task_id: Some(task_id.to_string()),
                    member: "builder".to_string(),
                    role: "rust-developer".to_string(),
                    tool: "codex".to_string(),
                    model: Some(model.to_string()),
                    applied_effort: Some("high".to_string()),
                    capability_tier: Some("strong".to_string()),
                    tier_rank: Some(0),
                },
            )
            .expect("record rendered launch");
        }

        attribute_latest_launch_to_task(
            &teams_dir,
            "routing-team",
            "task-b",
            "builder",
        );

        assert!(matches!(
            read_task_telemetry(
                &teams_dir.join("routing-team/state/telemetry/task-b.jsonl")
            )
            .as_slice(),
            [RoutingTelemetryEvent::LaunchRendered {
                model: Some(model),
                ..
            }] if model == "gpt-5.6-sol"
        ));
    }

    #[test]
    fn completion_observation_deduplicates_repeated_scans_but_records_a_later_ruling() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");

        record_launch_rendered(
            &teams_dir,
            "routing-team",
            Some("42"),
            "builder",
            "rust-developer",
            crate::session_scanner::cli_tool::CliTool::Codex,
            Some("gpt-5.6-sol"),
            Some("high"),
        );

        record_completion_observed(&teams_dir, "routing-team", "42", "completed", false);
        record_completion_observed(&teams_dir, "routing-team", "42", "completed", false);
        record_completion_observed(&teams_dir, "routing-team", "42", "completed", true);

        let path = teams_dir.join("routing-team/state/telemetry/42.jsonl");
        let events = read_task_telemetry(&path);
        assert_eq!(events.len(), 3);
        assert!(matches!(
            events.last(),
            Some(RoutingTelemetryEvent::CompletionObserved {
                has_review_ruling: true,
                ..
            })
        ));
    }

    // Regression: 13111833 created a sidecar for every historical terminal
    // task and treated an oversized sidecar as empty, making repeated scans
    // append forever after the reader's safety cap was crossed.
    #[test]
    fn completion_observation_requires_existing_bounded_telemetry() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");
        let absent = teams_dir.join("routing-team/state/telemetry/old-task.jsonl");

        record_completion_observed(&teams_dir, "routing-team", "old-task", "completed", true);
        assert!(!absent.exists(), "historical tasks must not gain sidecars");

        let oversized = teams_dir.join("routing-team/state/telemetry/large-task.jsonl");
        std::fs::create_dir_all(oversized.parent().expect("telemetry dir"))
            .expect("create telemetry dir");
        let file = std::fs::File::create(&oversized).expect("create oversized sidecar");
        file.set_len(super::MAX_SIDECAR_BYTES + 1)
            .expect("extend oversized sidecar");
        let before = file.metadata().expect("oversized metadata").len();

        record_completion_observed(&teams_dir, "routing-team", "large-task", "completed", true);

        assert_eq!(
            std::fs::metadata(&oversized)
                .expect("oversized metadata after observation")
                .len(),
            before,
            "oversized telemetry must not be appended"
        );
    }

    #[test]
    fn launch_observation_uses_actual_rendered_model_for_catalog_attribution() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");

        record_launch_rendered(
            &teams_dir,
            "routing-team",
            Some("42"),
            "builder",
            "rust-developer",
            crate::session_scanner::cli_tool::CliTool::Codex,
            Some("gpt-5.6-sol"),
            Some("high"),
        );

        let path = teams_dir.join("routing-team/state/telemetry/42.jsonl");
        assert!(matches!(
            read_task_telemetry(&path).as_slice(),
            [RoutingTelemetryEvent::LaunchRendered {
                model: Some(model),
                capability_tier: Some(tier),
                tier_rank: Some(0),
                ..
            }] if model == "gpt-5.6-sol" && tier == "strong"
        ));
    }

    #[test]
    fn effort_and_deadline_observations_keep_the_existing_outcome_fields() {
        let root = tempfile::tempdir().expect("tempdir");
        let teams_dir = root.path().join("teams");

        record_effort_switch(
            &teams_dir,
            "routing-team",
            "42",
            "builder",
            2,
            Some("medium"),
            "high",
            EffortSwitchOutcome::Completed,
        );
        record_deadline_action(&teams_dir, "routing-team", "42", "builder", 20, false);

        let path = teams_dir.join("routing-team/state/telemetry/42.jsonl");
        let events = read_task_telemetry(&path);
        assert!(matches!(
            &events[0],
            RoutingTelemetryEvent::EffortSwitch {
                attempt: 2,
                outcome: EffortSwitchOutcome::Completed,
                ..
            }
        ));
        assert!(matches!(
            &events[1],
            RoutingTelemetryEvent::NudgeSent {
                deadline_minutes: 20,
                ..
            }
        ));
    }
}
