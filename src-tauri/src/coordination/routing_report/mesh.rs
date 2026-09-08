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
    (raw.len() as u64 <= limit).then_some(raw)
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
