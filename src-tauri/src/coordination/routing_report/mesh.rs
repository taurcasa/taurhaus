//! Read Mesh 0.2.29's monitor authority and its workflow echo without writing
//! synthetic routing sidecars or replaying any lifecycle action.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Clone)]
pub(super) struct MonitorRecord {
    identity: String,
    pub task_id: String,
    pub member: String,
    pub timestamp: DateTime<Utc>,
    pub launch_health: bool,
}

fn decode(record: &Value, workflow: bool) -> Option<MonitorRecord> {
    if workflow {
        if record["eventType"] != "message_sent" || record["sender"] != "mesh-idle-monitor" {
            return None;
        }
    } else if record["source"] != "idle_monitor" {
        return None;
    }
    let launch_health = match record["kind"].as_str()? {
        "nudge" => false,
        "launch_health" => true,
        _ => return None,
    };
    let identity = record["message_id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(|id| format!("message:{id}"))
        .or_else(|| {
            // A leadless health record has no message/echo. Mesh identifies
            // that episode by the ignored originating nudge IDs.
            let ids = record["ignored_nudges"].as_array()?;
            (launch_health
                && !workflow
                && !ids.is_empty()
                && ids
                    .iter()
                    .all(|id| id.as_str().is_some_and(|id| !id.is_empty())))
            .then(|| format!("health:{}:{ids:?}", record["seat"]))
        })?;
    let member = if workflow {
        if launch_health {
            record["task_owner"].as_str()?
        } else {
            record["recipient"].as_str()?
        }
    } else {
        record["seat"].as_str()?
    };
    Some(MonitorRecord {
        identity,
        task_id: record["task_id"].as_str()?.to_owned(),
        member: member.to_owned(),
        timestamp: DateTime::parse_from_rfc3339(record["timestamp"].as_str()?)
            .ok()?
            .with_timezone(&Utc),
        launch_health,
    })
}

fn bounded_read(path: &Path, limit: u64) -> Option<String> {
    let mut raw = String::new();
    File::open(path)
        .ok()?
        .take(limit + 1)
        .read_to_string(&mut raw)
        .ok()?;
    if raw.len() as u64 > limit {
        tracing::debug!(
            event = "routing.input.skipped",
            path = %path.display(),
            limit,
            reason = "size_limit",
            "Routing input exceeds the read budget"
        );
        return None;
    }
    Some(raw)
}

pub(super) fn workflow_records(team_dir: &Path) -> Vec<MonitorRecord> {
    bounded_read(&team_dir.join("state/workflow_events.jsonl"), 8 * 1_048_576)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|record| decode(&record, true))
        .collect()
}

pub(super) fn task_records(
    task_path: &Path,
    task_id: &str,
    workflow: &[MonitorRecord],
) -> Vec<MonitorRecord> {
    let task = bounded_read(task_path, taurhaus_lib::task_scanner::claude::MAX_FILE_SIZE)
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
    let mut records = BTreeMap::new();
    for record in task
        .as_ref()
        .and_then(|task| task["metadata"]["idle_monitor_records"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|record| decode(record, false))
        .chain(workflow.iter().cloned())
        .filter(|record| record.task_id == task_id)
    {
        records.entry(record.identity.clone()).or_insert(record);
    }
    records.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression: 22d0fc03 silently discarded an entire workflow journal
    // above 8 MiB. Oversize input must leave a diagnostic naming its path.
    #[test]
    fn oversize_workflow_journal_emits_one_debug_diagnostic() {
        let root = tempfile::tempdir().unwrap();
        let team = root.path().join("teams/oversize-team");
        std::fs::create_dir_all(team.join("state")).unwrap();
        let journal = team.join("state/workflow_events.jsonl");
        File::create(&journal)
            .unwrap()
            .set_len(8 * 1_048_576 + 1)
            .unwrap();
        let log = root.path().join("capture.log");
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(std::sync::Mutex::new(File::create(&log).unwrap()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            assert!(workflow_records(&team).is_empty());
        });
        let output = std::fs::read_to_string(log).unwrap();
        assert_eq!(output.lines().count(), 1, "{output}");
        assert!(output.contains("DEBUG"), "{output}");
        assert!(output.contains("routing.input.skipped"), "{output}");
        assert!(
            output.contains("oversize-team/state/workflow_events.jsonl"),
            "{output}"
        );
        assert!(output.contains("size_limit"), "{output}");
    }
}
