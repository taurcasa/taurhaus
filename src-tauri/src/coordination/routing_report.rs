use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};

use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::telemetry::{
    read_task_telemetry, EffortSwitchOutcome, RoutingTelemetryEvent,
};
use crate::coordination::stores::TeamRootRegistry;

#[derive(Debug, Default)]
struct ReportStats {
    tasks: BTreeSet<String>,
    accepted: BTreeSet<String>,
    completed_unruled: BTreeSet<String>,
    relaunches: usize,
    effort_switches: usize,
    nudges: usize,
    staled: usize,
    wall_times: BTreeMap<String, i64>,
}

#[derive(Debug)]
struct LedgerVerdict {
    accepted_eligible: bool,
    has_review_ruling: bool,
}

pub fn render_routing_report(
    default_teams_dir: &Path,
    days: u32,
    now: DateTime<Utc>,
) -> Result<String, CoordinationError> {
    let cutoff = now - Duration::days(i64::from(days));
    let registry = TeamRootRegistry::new(default_teams_dir.to_path_buf());
    let mut role_rows = BTreeMap::<(String, String), ReportStats>::new();
    let mut model_rows = BTreeMap::<String, ReportStats>::new();

    for (teams_dir, team_name) in registry.team_locations()? {
        let telemetry_dir = teams_dir.join(&team_name).join("state/telemetry");
        let Ok(entries) = fs::read_dir(&telemetry_dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(task_id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .filter(|stem| *stem != "_unattributed")
            else {
                continue;
            };
            let events = read_task_telemetry(&path);
            if !events.iter().any(|event| event_timestamp(event) >= cutoff) {
                continue;
            }
            let ledger = read_ledger_verdict(&teams_dir, &team_name, task_id);
            accumulate_task(
                &mut role_rows,
                &mut model_rows,
                &format!("{team_name}/{task_id}"),
                &events,
                ledger.as_ref(),
            );
        }
    }

    let mut output = format!(
        "Routing telemetry: last {days} days\n\
         Wall-time is the Stage 1 cost proxy; tokens are not collected.\n\n\
         Role/model\n\
         role | model | tasks_touched | accepted | completed_unruled | relaunches | effort_switches | nudges | staled | median_wall_time\n"
    );
    for ((role, model), stats) in &role_rows {
        push_row(&mut output, Some(role), model, stats);
    }
    output.push_str("\nModel rollup\nmodel | tasks_touched | accepted | completed_unruled | relaunches | effort_switches | nudges | staled | median_wall_time\n");
    for (model, stats) in &model_rows {
        push_row(&mut output, None, model, stats);
    }
    Ok(output)
}

fn accumulate_task(
    role_rows: &mut BTreeMap<(String, String), ReportStats>,
    model_rows: &mut BTreeMap<String, ReportStats>,
    task_key: &str,
    events: &[RoutingTelemetryEvent],
    ledger: Option<&LedgerVerdict>,
) {
    let launches = events
        .iter()
        .filter_map(|event| match event {
            RoutingTelemetryEvent::LaunchRendered {
                timestamp,
                role,
                model,
                ..
            } => Some((
                *timestamp,
                role.clone(),
                model.clone().unwrap_or_else(|| "<unknown>".to_string()),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if launches.is_empty() {
        return;
    }
    let completion = events
        .iter()
        .filter_map(|event| match event {
            RoutingTelemetryEvent::CompletionObserved { timestamp, .. } => Some(*timestamp),
            _ => None,
        })
        .max();

    let role_keys = launches
        .iter()
        .map(|(_, role, model)| (role.clone(), model.clone()))
        .collect::<BTreeSet<_>>();
    let model_keys = launches
        .iter()
        .map(|(_, _, model)| model.clone())
        .collect::<BTreeSet<_>>();
    for key in &role_keys {
        let stats = role_rows.entry(key.clone()).or_default();
        mark_task(stats, task_key, ledger);
        let launch_times = launches
            .iter()
            .filter(|(_, role, model)| role == &key.0 && model == &key.1)
            .map(|(timestamp, _, _)| *timestamp)
            .collect::<Vec<_>>();
        stats.relaunches += launch_times.len().saturating_sub(1);
        record_wall_time(stats, task_key, launch_times.into_iter().min(), completion);
    }
    for model in &model_keys {
        let stats = model_rows.entry(model.clone()).or_default();
        mark_task(stats, task_key, ledger);
        let launch_times = launches
            .iter()
            .filter(|(_, _, launched_model)| launched_model == model)
            .map(|(timestamp, _, _)| *timestamp)
            .collect::<Vec<_>>();
        stats.relaunches += launch_times.len().saturating_sub(1);
        record_wall_time(stats, task_key, launch_times.into_iter().min(), completion);
    }

    for event in events {
        let (counts_effort, counts_nudge, counts_stale) = match event {
            RoutingTelemetryEvent::EffortSwitch {
                outcome: EffortSwitchOutcome::Completed,
                ..
            } => (true, false, false),
            RoutingTelemetryEvent::NudgeSent { .. } => (false, true, false),
            RoutingTelemetryEvent::TaskStaled { .. } => (false, false, true),
            _ => continue,
        };
        let timestamp = event_timestamp(event);
        let selected = launches
            .iter()
            .filter(|(launched_at, _, _)| *launched_at <= timestamp)
            .max_by_key(|(launched_at, _, _)| *launched_at)
            .or_else(|| launches.first());
        let Some((_, role, model)) = selected else {
            continue;
        };
        increment_counts(
            role_rows.entry((role.clone(), model.clone())).or_default(),
            counts_effort,
            counts_nudge,
            counts_stale,
        );
        increment_counts(
            model_rows.entry(model.clone()).or_default(),
            counts_effort,
            counts_nudge,
            counts_stale,
        );
    }
}

fn mark_task(stats: &mut ReportStats, task_key: &str, ledger: Option<&LedgerVerdict>) {
    stats.tasks.insert(task_key.to_string());
    match ledger {
        Some(ledger) if ledger.accepted_eligible && ledger.has_review_ruling => {
            stats.accepted.insert(task_key.to_string());
        }
        Some(ledger) if ledger.accepted_eligible => {
            stats.completed_unruled.insert(task_key.to_string());
        }
        _ => {}
    }
}

fn record_wall_time(
    stats: &mut ReportStats,
    task_key: &str,
    started: Option<DateTime<Utc>>,
    completed: Option<DateTime<Utc>>,
) {
    if let Some(seconds) = started
        .zip(completed)
        .map(|(started, completed)| (completed - started).num_seconds())
        .filter(|seconds| *seconds >= 0)
    {
        stats.wall_times.insert(task_key.to_string(), seconds);
    }
}

fn increment_counts(stats: &mut ReportStats, effort: bool, nudge: bool, stale: bool) {
    stats.effort_switches += usize::from(effort);
    stats.nudges += usize::from(nudge);
    stats.staled += usize::from(stale);
}

fn read_ledger_verdict(teams_dir: &Path, team_name: &str, task_id: &str) -> Option<LedgerVerdict> {
    let path = teams_dir
        .parent()?
        .join("tasks")
        .join(team_name)
        .join(format!("{task_id}.json"));
    let metadata = fs::metadata(&path).ok()?;
    if metadata.len() > 1_048_576 {
        return None;
    }
    let task =
        taurhaus_lib::task_scanner::claude::parse_task_file(&path, Some(team_name.to_string()))
            .ok()??;
    let terminal = crate::coordination::operational_context::is_terminal_task_status(
        &task.status.to_string(),
    );
    Some(LedgerVerdict {
        accepted_eligible: terminal
            && task.status == taurhaus_lib::task_scanner::TaskStatus::Completed,
        has_review_ruling: task.has_review_ruling,
    })
}

fn event_timestamp(event: &RoutingTelemetryEvent) -> DateTime<Utc> {
    match event {
        RoutingTelemetryEvent::LaunchRendered { timestamp, .. }
        | RoutingTelemetryEvent::EffortSwitch { timestamp, .. }
        | RoutingTelemetryEvent::NudgeSent { timestamp, .. }
        | RoutingTelemetryEvent::TaskStaled { timestamp, .. }
        | RoutingTelemetryEvent::CompletionObserved { timestamp, .. } => *timestamp,
    }
}

fn push_row(output: &mut String, role: Option<&str>, model: &str, stats: &ReportStats) {
    if let Some(role) = role {
        output.push_str(&format!("{role} | "));
    }
    output.push_str(&format!(
        "{model} | {} | {} | {} | {} | {} | {} | {} | {}\n",
        stats.tasks.len(),
        stats.accepted.len(),
        stats.completed_unruled.len(),
        stats.relaunches,
        stats.effort_switches,
        stats.nudges,
        stats.staled,
        median_wall_time(&stats.wall_times)
    ));
}

fn median_wall_time(values: &BTreeMap<String, i64>) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    let mut seconds = values.values().copied().collect::<Vec<_>>();
    seconds.sort_unstable();
    let middle = seconds.len() / 2;
    let median = if seconds.len() % 2 == 0 {
        (seconds[middle - 1] + seconds[middle]) / 2
    } else {
        seconds[middle]
    };
    format!("{}m {:02}s", median / 60, median % 60)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::render_routing_report;

    fn write_json(path: &std::path::Path, value: serde_json::Value) {
        std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture");
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&value).expect("serialize fixture"),
        )
        .expect("write fixture");
    }

    fn write_sidecar(
        teams_dir: &std::path::Path,
        team: &str,
        task_id: &str,
        role: &str,
        model: &str,
        completed_at: &str,
        ruled: bool,
    ) {
        let path = teams_dir
            .join(team)
            .join("state/telemetry")
            .join(format!("{task_id}.jsonl"));
        std::fs::create_dir_all(path.parent().expect("sidecar parent"))
            .expect("create sidecar dir");
        let lines = format!(
            concat!(
                "{{\"event\":\"launch_rendered\",\"timestamp\":\"2026-09-03T10:00:00Z\",",
                "\"task_id\":\"{task_id}\",\"member\":\"builder\",\"role\":\"{role}\",",
                "\"tool\":\"codex\",\"model\":\"{model}\",\"applied_effort\":\"high\",",
                "\"capability_tier\":\"strong\",\"tier_rank\":0}}\n",
                "not-json\n",
                "{{\"event\":\"completion_observed\",\"timestamp\":\"{completed_at}\",",
                "\"task_id\":\"{task_id}\",\"status\":\"completed\",",
                "\"has_review_ruling\":{ruled}}}\n"
            ),
            task_id = task_id,
            role = role,
            model = model,
            completed_at = completed_at,
            ruled = ruled,
        );
        std::fs::write(path, lines).expect("write sidecar");
    }

    #[test]
    fn smoke_report_reads_all_registered_roots_and_splits_accepted_from_unruled() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        let work_teams = root.path().join("work/teams");
        for (teams_dir, team) in [
            (&default_teams, "accepted-team"),
            (&work_teams, "unruled-team"),
        ] {
            write_json(
                &teams_dir.join(team).join("config.json"),
                serde_json::json!({"name": team, "members": []}),
            );
        }
        crate::coordination::stores::TeamRootRegistry::new(default_teams.clone())
            .set("unruled-team", &work_teams)
            .expect("register work root");

        write_sidecar(
            &default_teams,
            "accepted-team",
            "41",
            "rust-developer",
            "gpt-5.6-sol",
            "2026-09-03T10:10:00Z",
            true,
        );
        write_sidecar(
            &work_teams,
            "unruled-team",
            "42",
            "test-developer",
            "gpt-5.6-luna",
            "2026-09-03T10:05:00Z",
            false,
        );
        write_json(
            &root.path().join("personal/tasks/accepted-team/41.json"),
            serde_json::json!({
                "id": "41",
                "subject": "Accepted task",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [{"kind": "verdict", "value": "accepted"}]}
            }),
        );
        write_json(
            &root.path().join("work/tasks/unruled-team/42.json"),
            serde_json::json!({
                "id": "42",
                "subject": "Unruled task",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains("Wall-time is the Stage 1 cost proxy; tokens are not collected."));
        assert!(report.contains("rust-developer | gpt-5.6-sol | 1 | 1 | 0"));
        assert!(report.contains("test-developer | gpt-5.6-luna | 1 | 0 | 1"));
        assert!(report.contains("gpt-5.6-sol | 1 | 1 | 0"));
        assert!(report.contains("gpt-5.6-luna | 1 | 0 | 1"));
    }

    // Regression: c9c6c49b could not attribute a reused member launch that
    // lived under an earlier task, so the report dropped the later task's
    // acceptance and nudge when its sidecar contained no launch of its own.
    #[test]
    fn report_includes_a_later_task_attributed_from_an_earlier_task_launch() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        crate::coordination::stores::telemetry::append_task_telemetry(
            &default_teams,
            "routing-team",
            Some("task-a"),
            &crate::coordination::stores::telemetry::RoutingTelemetryEvent::LaunchRendered {
                timestamp: Utc.with_ymd_and_hms(2026, 7, 1, 10, 0, 0).unwrap(),
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
        crate::coordination::stores::telemetry::attribute_latest_launch_to_task(
            &default_teams,
            "routing-team",
            "task-b",
            "builder",
        );
        let now = Utc::now();
        for event in [
            crate::coordination::stores::telemetry::RoutingTelemetryEvent::NudgeSent {
                timestamp: now,
                task_id: "task-b".to_string(),
                member: "builder".to_string(),
                deadline_minutes: 20,
            },
            crate::coordination::stores::telemetry::RoutingTelemetryEvent::CompletionObserved {
                timestamp: now,
                task_id: "task-b".to_string(),
                status: "completed".to_string(),
                has_review_ruling: true,
            },
        ] {
            crate::coordination::stores::telemetry::append_task_telemetry(
                &default_teams,
                "routing-team",
                Some("task-b"),
                &event,
            )
            .expect("record task B event");
        }
        write_json(
            &root.path().join("personal/tasks/routing-team/task-b.json"),
            serde_json::json!({
                "id": "task-b",
                "subject": "Later accepted task",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [{"kind": "verdict", "value": "accepted"}]}
            }),
        );

        let report = render_routing_report(&default_teams, 30, now + chrono::Duration::minutes(1))
            .expect("render report");

        assert!(report.contains("rust-developer | gpt-5.6-sol | 1 | 1 | 0 | 0 | 0 | 1 | 0"));
    }

    // Regression: c9c6c49b treated the scanner's `stale` terminal state as
    // accepted-eligible, so a timed-out task with a ruling inflated accepted.
    #[test]
    fn stale_ledger_tasks_count_only_as_staled_even_with_a_ruling() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        write_sidecar(
            &default_teams,
            "routing-team",
            "43",
            "rust-developer",
            "gpt-5.6-sol",
            "2026-09-03T10:10:00Z",
            false,
        );
        let sidecar = default_teams.join("routing-team/state/telemetry/43.jsonl");
        use std::io::Write;
        writeln!(
            std::fs::OpenOptions::new()
                .append(true)
                .open(&sidecar)
                .expect("open sidecar"),
            "{{\"event\":\"task_staled\",\"timestamp\":\"2026-09-03T10:09:00Z\",\"task_id\":\"43\",\"member\":\"builder\",\"deadline_minutes\":20}}"
        )
        .expect("append stale observation");
        write_json(
            &root.path().join("personal/tasks/routing-team/43.json"),
            serde_json::json!({
                "id": "43",
                "subject": "Timed out after review",
                "description": null,
                "activeForm": null,
                "status": "stale",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [{"kind": "score", "value": 8}]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "rust-developer | gpt-5.6-sol | 1 | 0 | 0 | 0 | 0 | 0 | 1 | 10m 00s"
        ));
    }
}
