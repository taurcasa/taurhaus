mod mesh;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Duration, Utc};

use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::telemetry::{
    read_task_telemetry, EffortSwitchOutcome, NudgeSource, RoutingTelemetryEvent,
};
use crate::coordination::stores::TeamRootRegistry;
use taurhaus_lib::task_scanner::claude::{
    is_budget_raise, is_oversize_failure, parse_task_content, MAX_FILE_SIZE,
};
use taurhaus_lib::task_scanner::claude_index::ClaudeSourceIndex;
use taurhaus_lib::task_scanner::types::TaskStatus;

#[derive(Debug, Default)]
struct ReportStats {
    tasks: BTreeSet<String>,
    accepted: BTreeSet<String>,
    completed_unruled: BTreeSet<String>,
    oversize_diffs: usize,
    budget_raises: usize,
    relaunches: usize,
    effort_switches: usize,
    deadline_nudges: usize,
    monitor_nudges: usize,
    launch_health: usize,
    staled: usize,
    wall_times: BTreeMap<String, i64>,
}

#[derive(Debug)]
struct LedgerVerdict {
    latest_ruling_at: Option<DateTime<Utc>>,
    accepted_eligible: bool,
    has_review_ruling: bool,
    oversize_rulings: Vec<OwnerRuling>,
    budget_rulings: Vec<OwnerRuling>,
}

/// One budget/oversize ruling, carried per ruling (not aggregated) so
/// attribution can select the owner's launch active at the ruling's `at`.
#[derive(Debug)]
struct OwnerRuling {
    owner: String,
    at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
struct LaunchAttribution {
    timestamp: DateTime<Utc>,
    member: String,
    role: String,
    model: String,
}

#[derive(serde::Deserialize)]
struct LedgerTaskFile {
    #[serde(default)]
    metadata: Option<serde_json::Value>,
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
        let workflow = mesh::workflow_records(&teams_dir.join(&team_name));
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
            let monitor = ClaudeSourceIndex::team_tasks_dir(&teams_dir, &team_name)
                .map(|tasks| {
                    mesh::task_records(&tasks.join(format!("{task_id}.json")), task_id, &workflow)
                })
                .unwrap_or_default();
            let ledger = read_ledger_verdict(&teams_dir, &team_name, task_id);
            if !events.iter().any(|event| event_timestamp(event) >= cutoff)
                && !monitor.iter().any(|record| record.timestamp >= cutoff)
                && ledger
                    .as_ref()
                    .and_then(|ledger| ledger.latest_ruling_at)
                    .is_none_or(|at| at < cutoff)
            {
                continue;
            }
            accumulate_task(
                &mut role_rows,
                &mut model_rows,
                &format!("{team_name}/{task_id}"),
                &events,
                ledger.as_ref(),
                &monitor,
            );
        }
    }

    let total_accepted: usize = role_rows.values().map(|stats| stats.accepted.len()).sum();
    let mut output = format!(
        "Routing telemetry: last {days} days\n\
         Wall-time is the Stage 1 cost proxy; tokens are not collected.\n\n"
    );
    if total_accepted == 0 {
        // Accurate condition, printed only while it holds: rulings ARE
        // recordable today (`mesh task ruling`, mesh >= 0.2.28) — none has
        // been recorded in this window yet.
        output.push_str(
            "Accepted counts only tasks whose ledger record carries metadata.rulings \
             (`mesh task ruling`); none are recorded in this window yet.\n\n",
        );
    }
    output.push_str(
        "Role/model\n\
         role | model | tasks_touched | accepted | completed_unruled | oversize_diffs | budget_raises | relaunches | effort_switches | deadline_nudges | monitor_nudges | staled | median_wall_time\n",
    );
    for ((role, model), stats) in &role_rows {
        push_row(&mut output, Some(role), model, stats);
    }
    output.push_str("\nModel rollup\nmodel | tasks_touched | accepted | completed_unruled | oversize_diffs | budget_raises | relaunches | effort_switches | deadline_nudges | monitor_nudges | staled | median_wall_time\n");
    for (model, stats) in &model_rows {
        push_row(&mut output, None, model, stats);
    }
    let health: usize = role_rows.values().map(|stats| stats.launch_health).sum();
    output.push_str(&format!("\nLaunch-health records: {health} (attributed to the affected seat; excluded from monitor_nudges).\n"));
    output.push_str("\nEvents without recipient launch telemetry are omitted from both rollups.\n");
    Ok(output)
}

fn accumulate_task(
    role_rows: &mut BTreeMap<(String, String), ReportStats>,
    model_rows: &mut BTreeMap<String, ReportStats>,
    task_key: &str,
    events: &[RoutingTelemetryEvent],
    ledger: Option<&LedgerVerdict>,
    monitor: &[mesh::MonitorRecord],
) {
    let launches = events
        .iter()
        .filter_map(|event| match event {
            RoutingTelemetryEvent::LaunchRendered {
                timestamp,
                member,
                role,
                model,
                ..
            } => Some(LaunchAttribution {
                timestamp: *timestamp,
                member: member.clone(),
                role: role.clone(),
                model: model.clone().unwrap_or_else(|| "<unknown>".to_string()),
            }),
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
        .map(|launch| (launch.role.clone(), launch.model.clone()))
        .collect::<BTreeSet<_>>();
    let model_keys = launches
        .iter()
        .map(|launch| launch.model.clone())
        .collect::<BTreeSet<_>>();
    for key in &role_keys {
        let stats = role_rows.entry(key.clone()).or_default();
        mark_task(stats, task_key, ledger);
        let launch_times = launches
            .iter()
            .filter(|launch| launch.role == key.0 && launch.model == key.1)
            .map(|launch| launch.timestamp)
            .collect::<Vec<_>>();
        stats.relaunches += launch_times.len().saturating_sub(1);
        record_wall_time(stats, task_key, launch_times.into_iter().min(), completion);
    }
    for model in &model_keys {
        let stats = model_rows.entry(model.clone()).or_default();
        mark_task(stats, task_key, ledger);
        let launch_times = launches
            .iter()
            .filter(|launch| launch.model == *model)
            .map(|launch| launch.timestamp)
            .collect::<Vec<_>>();
        stats.relaunches += launch_times.len().saturating_sub(1);
        record_wall_time(stats, task_key, launch_times.into_iter().min(), completion);
    }
    if let Some(ledger) = ledger {
        for (ruling, budget_raise) in ledger
            .oversize_rulings
            .iter()
            .map(|ruling| (ruling, false))
            .chain(ledger.budget_rulings.iter().map(|ruling| (ruling, true)))
        {
            let owned = launches
                .iter()
                .filter(|launch| launch.member == ruling.owner)
                .collect::<Vec<_>>();
            // Same convention as the event attribution below: the owner's
            // launch active at the ruling's `at`, else the owner's earliest —
            // a mid-task relaunch under another model must not absorb earlier
            // incidents.
            let selected = ruling
                .at
                .and_then(|at| {
                    owned
                        .iter()
                        .filter(|launch| launch.timestamp <= at)
                        .max_by_key(|launch| launch.timestamp)
                        .copied()
                })
                .or_else(|| owned.iter().min_by_key(|launch| launch.timestamp).copied());
            let Some(launch) = selected else {
                continue;
            };
            let role_stats = role_rows
                .entry((launch.role.clone(), launch.model.clone()))
                .or_default();
            let model_stats = model_rows.entry(launch.model.clone()).or_default();
            for stats in [role_stats, model_stats] {
                if budget_raise {
                    stats.budget_raises += 1;
                } else {
                    stats.oversize_diffs += 1;
                }
            }
        }
    }

    for record in monitor {
        let selected = launches
            .iter()
            .filter(|launch| launch.member == record.member && launch.timestamp <= record.timestamp)
            .max_by_key(|launch| launch.timestamp)
            .or_else(|| {
                launches
                    .iter()
                    .filter(|launch| launch.member == record.member)
                    .min_by_key(|launch| launch.timestamp)
            });
        let Some(launch) = selected else {
            continue;
        };
        for stats in [
            role_rows
                .entry((launch.role.clone(), launch.model.clone()))
                .or_default(),
            model_rows.entry(launch.model.clone()).or_default(),
        ] {
            if record.launch_health {
                stats.launch_health += 1;
            } else {
                stats.monitor_nudges += 1;
            }
        }
    }

    for event in events {
        let member = match event {
            RoutingTelemetryEvent::EffortSwitch {
                outcome: EffortSwitchOutcome::Completed,
                member,
                ..
            }
            | RoutingTelemetryEvent::NudgeSent { member, .. }
            | RoutingTelemetryEvent::TaskStaled { member, .. } => member,
            _ => continue,
        };
        let owned = launches
            .iter()
            .filter(|launch| &launch.member == member)
            .collect::<Vec<_>>();
        let timestamp = event_timestamp(event);
        let selected = owned
            .iter()
            .filter(|launch| launch.timestamp <= timestamp)
            .max_by_key(|launch| launch.timestamp)
            .or_else(|| owned.iter().min_by_key(|launch| launch.timestamp));
        let Some(launch) = selected else {
            continue;
        };
        increment_counts(
            role_rows
                .entry((launch.role.clone(), launch.model.clone()))
                .or_default(),
            event,
        );
        increment_counts(model_rows.entry(launch.model.clone()).or_default(), event);
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

fn increment_counts(stats: &mut ReportStats, event: &RoutingTelemetryEvent) {
    match event {
        RoutingTelemetryEvent::EffortSwitch {
            outcome: EffortSwitchOutcome::Completed,
            ..
        } => stats.effort_switches += 1,
        RoutingTelemetryEvent::NudgeSent {
            source: NudgeSource::Deadline,
            ..
        } => stats.deadline_nudges += 1,
        RoutingTelemetryEvent::NudgeSent {
            source: NudgeSource::IdleMonitor,
            ..
        } => stats.monitor_nudges += 1,
        RoutingTelemetryEvent::TaskStaled { .. } => stats.staled += 1,
        _ => {}
    }
}

fn read_ledger_verdict(teams_dir: &Path, team_name: &str, task_id: &str) -> Option<LedgerVerdict> {
    let path =
        ClaudeSourceIndex::team_tasks_dir(teams_dir, team_name)?.join(format!("{task_id}.json"));
    let metadata = fs::metadata(&path).ok()?;
    if metadata.len() > MAX_FILE_SIZE {
        return None;
    }
    let raw = fs::read_to_string(&path).ok()?;
    let task = parse_task_content(&path, &raw, Some(team_name.to_string())).ok()??;
    let ledger = serde_json::from_str::<LedgerTaskFile>(&raw).ok()?;
    let rulings = ledger
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("rulings"))
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let owner = task
        .owner
        .as_deref()
        .map(str::trim)
        .filter(|owner| !owner.is_empty());
    let mut oversize_rulings = Vec::new();
    let mut budget_rulings = Vec::new();
    // The diff's producer is the task owner; `by` names the filing reviewer,
    // so a ruling on an ownerless task is dropped rather than charged to a
    // reviewer's row.
    if let Some(owner) = owner {
        for ruling in rulings
            .iter()
            .filter(|ruling| is_oversize_failure(ruling) || is_budget_raise(ruling))
        {
            let at = ruling
                .get("at")
                .and_then(serde_json::Value::as_str)
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.with_timezone(&Utc));
            let target = if is_budget_raise(ruling) {
                &mut budget_rulings
            } else {
                &mut oversize_rulings
            };
            target.push(OwnerRuling {
                owner: owner.to_string(),
                at,
            });
        }
    }
    Some(LedgerVerdict {
        latest_ruling_at: rulings
            .iter()
            .filter_map(|ruling| {
                DateTime::parse_from_rfc3339(ruling.get("at")?.as_str()?)
                    .ok()
                    .map(|at| at.with_timezone(&Utc))
            })
            .max(),
        accepted_eligible: task.status == TaskStatus::Completed,
        has_review_ruling: task.has_review_ruling,
        oversize_rulings,
        budget_rulings,
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
        "{model} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}\n",
        stats.tasks.len(),
        stats.accepted.len(),
        stats.completed_unruled.len(),
        stats.oversize_diffs,
        stats.budget_raises,
        stats.relaunches,
        stats.effort_switches,
        stats.deadline_nudges,
        stats.monitor_nudges,
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
    #[cfg(target_os = "linux")]
    fn mesh_launch(
        fixture: &crate::coordination::mesh_contract_fixture::MeshFixture,
        launched_at: chrono::DateTime<Utc>,
    ) {
        use crate::coordination::stores::telemetry::{
            append_task_telemetry, RoutingTelemetryEvent,
        };
        append_task_telemetry(
            &fixture.teams(),
            "deadline-team",
            Some(&fixture.task_id),
            &RoutingTelemetryEvent::LaunchRendered {
                timestamp: launched_at,
                task_id: Some(fixture.task_id.clone()),
                member: "builder".into(),
                role: "developer".into(),
                tool: "codex".into(),
                model: Some("fixture-model".into()),
                applied_effort: None,
                capability_tier: None,
                tier_rank: None,
            },
        )
        .unwrap();
    }

    // Regression: 10f294bd added sidecar-only monitor accounting, but Mesh
    // 6789201c writes metadata plus workflow echoes, never that sidecar shape.
    #[cfg(target_os = "linux")]
    #[test]
    fn mesh_binary_monitor_records_are_counted_once_across_workflow_echoes() {
        let fixture = crate::coordination::mesh_contract_fixture::MeshFixture::new("monitor");
        mesh_launch(&fixture, Utc::now() - chrono::Duration::minutes(1));
        let now = Utc::now() + chrono::Duration::hours(1);
        let report = render_routing_report(&fixture.teams(), 30, now).unwrap();
        assert!(
            report
                .contains("developer | fixture-model | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 2 | 0 | -"),
            "{report}"
        );
        assert!(report.contains("Launch-health records: 1"), "{report}");
        // Re-reading cannot multiply emissions. Workflow echoes also remain
        // sufficient when the task-metadata copy is unavailable.
        assert_eq!(
            report,
            render_routing_report(&fixture.teams(), 30, now).unwrap()
        );
        std::fs::remove_file(fixture.task_path()).unwrap();
        assert_eq!(
            report,
            render_routing_report(&fixture.teams(), 30, now).unwrap()
        );
    }

    // Regression: a9fea658 selected tasks by sidecar timestamps alone,
    // omitting a newly recorded ruling on an older launched task.
    #[cfg(target_os = "linux")]
    #[test]
    fn mesh_binary_oversize_and_subsequent_raise_count_even_with_an_old_launch() {
        for launch_age in [0, 31] {
            let fixture = crate::coordination::mesh_contract_fixture::MeshFixture::new("ruling");
            let raw = std::fs::read_to_string(fixture.task_path()).unwrap();
            let task = taurhaus_lib::task_scanner::claude::parse_task_content(
                &fixture.task_path(),
                &raw,
                Some("deadline-team".into()),
            )
            .unwrap()
            .unwrap();
            assert!(!task.has_review_ruling, "budget records are not acceptance");
            assert_eq!(task.owner.as_deref(), Some("builder"));
            let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert!(taurhaus_lib::task_scanner::claude::is_oversize_failure(
                &value["metadata"]["rulings"][0]
            ));
            assert!(taurhaus_lib::task_scanner::claude::is_budget_raise(
                &value["metadata"]["rulings"][1]
            ));
            mesh_launch(&fixture, Utc::now() - chrono::Duration::days(launch_age));
            let report = render_routing_report(&fixture.teams(), 30, Utc::now()).unwrap();
            assert!(
                report.contains(
                    "developer | fixture-model | 1 | 0 | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | -"
                ),
                "launch age {launch_age}: {report}"
            );
            std::fs::remove_file(fixture.teams().join(format!(
                "deadline-team/state/telemetry/{}.jsonl",
                fixture.task_id
            )))
            .unwrap();
            let unattributed = render_routing_report(&fixture.teams(), 30, Utc::now()).unwrap();
            assert!(!unattributed.contains("developer | fixture-model"));
            assert!(unattributed.contains("Events without recipient launch telemetry are omitted"));
        }
    }

    // Regression: c9c6c49b only decoded deadline nudges and attributed actions
    // to the latest launch of any member (Wave-1 F9b; Astra §4).
    #[test]
    fn wave2_monitor_nudges_are_decoded_split_and_attributed_to_the_recipient() {
        let root = tempfile::tempdir().unwrap();
        let teams = root.path().join("teams");
        write_json(
            &teams.join("routing-team/config.json"),
            serde_json::json!({"name":"routing-team","members":[]}),
        );
        let path = teams.join("routing-team/state/telemetry/9.jsonl");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let records = [
            serde_json::json!({"event":"launch_rendered","timestamp":"2026-09-03T19:40:00Z",
                "task_id":"9","member":"judge","role":"judge","tool":"codex","model":"gpt-6-astra"}),
            serde_json::json!({"event":"launch_rendered","timestamp":"2026-09-03T19:50:00Z",
                "task_id":"9","member":"builder","role":"developer","tool":"codex","model":"gpt-5.6-sol"}),
            serde_json::json!({"event":"nudge_sent","timestamp":"2026-09-03T19:57:01Z",
                "task_id":"9","member":"judge","source":"idle_monitor"}),
            // Records written before Wave 2 have no source and remain deadlines.
            serde_json::json!({"event":"nudge_sent","timestamp":"2026-09-03T19:58:00Z",
                "task_id":"9","member":"builder","deadline_minutes":20}),
            serde_json::json!({"event":"nudge_sent","timestamp":"2026-09-03T19:59:00Z",
                "task_id":"9","member":"unknown-seat","source":"idle_monitor"}),
        ];
        std::fs::write(
            &path,
            records
                .iter()
                .map(|record| format!("{record}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let events = crate::coordination::stores::telemetry::read_task_telemetry(&path);
        assert_eq!(events.len(), 5, "monitor events must survive decoding");
        let report = render_routing_report(
            &teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .unwrap();
        // Regression: 6a6c14a9 dropped unknown-seat nudges without disclosing
        // that missing recipient launch telemetry suppresses report counts.
        assert!(
            report.contains(
                "Events without recipient launch telemetry are omitted from both rollups."
            ),
            "{report}"
        );
        assert!(
            report.contains("deadline_nudges | monitor_nudges"),
            "{report}"
        );
        assert!(
            report.contains("judge | gpt-6-astra | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | -"),
            "{report}"
        );
        assert!(
            report.contains("developer | gpt-5.6-sol | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | -"),
            "{report}"
        );
    }

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
                "\"observed_at\":\"2026-09-04T11:00:00Z\",",
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

    // Regression: 5ebf28b9 counted oversize rulings but hid F5 budget raises
    // and let a budget-only ruling manufacture review acceptance.
    #[test]
    fn wave2_budget_raises_are_owner_attributed_and_not_reviews_or_oversize() {
        let root = tempfile::tempdir().unwrap();
        let teams = root.path().join("teams");
        write_json(
            &teams.join("routing-team/config.json"),
            serde_json::json!({"name":"routing-team", "members":[]}),
        );
        for (id, owner, raises) in [("1", Some("builder"), "1"), ("2", None, "0")] {
            write_sidecar(
                &teams,
                "routing-team",
                id,
                "heavy",
                "gpt-6-astra",
                "2026-09-03T10:10:00Z",
                false,
            );
            write_json(
                &root.path().join(format!("tasks/routing-team/{id}.json")),
                serde_json::json!({"id":id,"subject":"Raise budget","status":"completed",
                    "owner":owner,"metadata":{"rulings":[{"kind":"ruling",
                    "field":"budget_raised","value":"400→450","note":"scope clarified",
                    "by":"reviewer","at":"2026-09-03T10:09:00Z"}]}}),
            );
            let verdict = super::read_ledger_verdict(&teams, "routing-team", id).unwrap();
            assert!(!verdict.has_review_ruling, "a raise is not a review");
            let mut rows = std::collections::BTreeMap::new();
            let mut models = std::collections::BTreeMap::new();
            let events = crate::coordination::stores::telemetry::read_task_telemetry(
                &teams.join(format!("routing-team/state/telemetry/{id}.jsonl")),
            );
            super::accumulate_task(&mut rows, &mut models, id, &events, Some(&verdict), &[]);
            let mut row = String::new();
            super::push_row(
                &mut row,
                Some("heavy"),
                "gpt-6-astra",
                rows.values().next().unwrap(),
            );
            assert_eq!(
                row,
                format!(
                    "heavy | gpt-6-astra | 1 | 0 | 1 | 0 | {raises} | 0 | 0 | 0 | 0 | 0 | 10m 00s\n"
                )
            );
        }
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
                "metadata": {"rulings": [
                    {"seq": 1, "kind": "verdict", "value": "accepted", "by": "reviewer", "at": "2026-09-03T10:08:00Z"},
                    {"seq": 2, "kind": "ruling", "field": "oversize_diff", "value": "failed", "by": "reviewer", "at": "2026-09-03T10:09:00Z", "note": "budget 20 lines; actual 30"},
                    {"seq": 3, "kind": "ruling", "field": "oversize_diff", "value": "failed", "by": "reviewer", "at": "2026-09-03T10:09:30Z", "note": "budget 20 lines; actual 31"}
                ]}
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
        // The zero-accepted hint must NEVER print beside a non-zero accepted
        // column (this fixture records a ruling).
        assert!(!report.contains("none are recorded in this window yet"));
        assert!(report.contains(
            "role | model | tasks_touched | accepted | completed_unruled | oversize_diffs | budget_raises | relaunches"
        ));
        assert!(report.contains(
            "rust-developer | gpt-5.6-sol | 1 | 1 | 0 | 2 | 0 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
        assert!(report.contains(
            "test-developer | gpt-5.6-luna | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 5m 00s"
        ));
        assert!(report.contains("gpt-5.6-sol | 1 | 1 | 0 | 2"));
        assert!(report.contains("gpt-5.6-luna | 1 | 0 | 1 | 0"));
    }

    // Regression: c9c6c49b could not attribute a reused member launch that
    // lived under an earlier task, so the report dropped the later task's
    // acceptance and nudge when its sidecar contained no launch of its own.
    #[test]
    fn zero_accepted_windows_print_the_rulings_hint() {
        let teams = tempfile::TempDir::new().expect("teams root");
        let report = render_routing_report(
            teams.path(),
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");
        assert!(report.contains("none are recorded in this window yet"));
    }

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
                source: crate::coordination::stores::telemetry::NudgeSource::Deadline,
                deadline_minutes: Some(20),
            },
            crate::coordination::stores::telemetry::RoutingTelemetryEvent::CompletionObserved {
                timestamp: now,
                observed_at: Some(now),
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

        assert!(report.contains("rust-developer | gpt-5.6-sol | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 1 | 0"));
    }

    // Regression: 24854270 credited an oversize ruling to its filing reviewer
    // (`by`) instead of the task owner whose produced diff the leash measures.
    #[test]
    fn oversize_diff_rulings_are_attributed_to_the_task_owner() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        let launched_at = Utc.with_ymd_and_hms(2026, 9, 3, 10, 0, 0).unwrap();
        for (offset, member, role, model) in [
            (0, "builder", "astra-heavy-implementer", "gpt-6-astra"),
            (
                1,
                "reviewer",
                "adversarial-reviewer-claude",
                "claude-opus-4-6",
            ),
            (2, "lead", "v3-lead-claude", "fable"),
        ] {
            crate::coordination::stores::telemetry::append_task_telemetry(
                &default_teams,
                "routing-team",
                Some("44"),
                &crate::coordination::stores::telemetry::RoutingTelemetryEvent::LaunchRendered {
                    timestamp: launched_at + chrono::Duration::minutes(offset),
                    task_id: Some("44".to_string()),
                    member: member.to_string(),
                    role: role.to_string(),
                    tool: "codex".to_string(),
                    model: Some(model.to_string()),
                    applied_effort: Some("high".to_string()),
                    capability_tier: Some("frontier".to_string()),
                    tier_rank: Some(0),
                },
            )
            .expect("record launch");
        }
        crate::coordination::stores::telemetry::append_task_telemetry(
            &default_teams,
            "routing-team",
            Some("44"),
            &crate::coordination::stores::telemetry::RoutingTelemetryEvent::CompletionObserved {
                timestamp: launched_at + chrono::Duration::minutes(10),
                observed_at: Some(launched_at + chrono::Duration::minutes(10)),
                task_id: "44".to_string(),
                status: "completed".to_string(),
                has_review_ruling: true,
            },
        )
        .expect("record completion");
        write_json(
            &root.path().join("personal/tasks/routing-team/44.json"),
            serde_json::json!({
                "id": "44",
                "subject": "Reviewed heavy implementation",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [
                    {"seq": 1, "kind": "verdict", "value": "accepted", "by": "reviewer", "at": "2026-09-03T10:08:00Z"},
                    {"seq": 2, "kind": "ruling", "field": "oversize_diff", "value": "failed", "by": "reviewer", "at": "2026-09-03T10:09:00Z", "note": "budget 200 lines; actual 260"}
                ]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "astra-heavy-implementer | gpt-6-astra | 1 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
        assert!(report.contains(
            "adversarial-reviewer-claude | claude-opus-4-6 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 9m 00s"
        ));
        assert!(report
            .contains("v3-lead-claude | fable | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 8m 00s"));
        assert!(report.contains("gpt-6-astra | 1 | 1 | 0 | 1"));
        assert!(report.contains("claude-opus-4-6 | 1 | 1 | 0 | 0"));
        assert!(report.contains("fable | 1 | 1 | 0 | 0"));
    }

    // Regression: 13111833 treated every generic `ruling` entry as review
    // acceptance, so the corrected oversize failure shape could accept a task.
    #[test]
    fn oversize_failure_alone_is_completed_unruled_not_accepted() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        write_sidecar(
            &default_teams,
            "routing-team",
            "45",
            "astra-heavy-implementer",
            "gpt-6-astra",
            "2026-09-03T10:10:00Z",
            true,
        );
        write_json(
            &root.path().join("personal/tasks/routing-team/45.json"),
            serde_json::json!({
                "id": "45",
                "subject": "Oversized heavy implementation",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [{
                    "seq": 1,
                    "kind": "ruling",
                    "field": "oversize_diff",
                    "value": "failed",
                    "by": "reviewer",
                    "at": "2026-09-03T10:09:00Z",
                    "note": "budget 200 lines; actual 260"
                }]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "astra-heavy-implementer | gpt-6-astra | 1 | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
    }

    // Regression: 52505603 fell back to the ruling's `by` when the ledger
    // record carried no owner, charging the incident to the filing reviewer —
    // the same inverted contract the owner fix rejected, narrowed to
    // unassigned tasks.
    #[test]
    fn ownerless_oversize_rulings_are_dropped_not_charged_to_the_reviewer() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        write_sidecar(
            &default_teams,
            "routing-team",
            "47",
            "astra-heavy-implementer",
            "gpt-6-astra",
            "2026-09-03T10:10:00Z",
            false,
        );
        crate::coordination::stores::telemetry::append_task_telemetry(
            &default_teams,
            "routing-team",
            Some("47"),
            &crate::coordination::stores::telemetry::RoutingTelemetryEvent::LaunchRendered {
                timestamp: Utc.with_ymd_and_hms(2026, 9, 3, 10, 1, 0).unwrap(),
                task_id: Some("47".to_string()),
                member: "reviewer".to_string(),
                role: "adversarial-reviewer-claude".to_string(),
                tool: "claude".to_string(),
                model: Some("claude-opus-4-6".to_string()),
                applied_effort: None,
                capability_tier: Some("strong".to_string()),
                tier_rank: Some(1),
            },
        )
        .expect("record reviewer launch");
        write_json(
            &root.path().join("personal/tasks/routing-team/47.json"),
            serde_json::json!({
                "id": "47",
                "subject": "Never assigned, still ruled oversize",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "metadata": {"rulings": [{
                    "seq": 1,
                    "kind": "ruling",
                    "field": "oversize_diff",
                    "value": "failed",
                    "by": "reviewer",
                    "at": "2026-09-03T10:09:00Z",
                    "note": "budget 200 lines; actual 260"
                }]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "astra-heavy-implementer | gpt-6-astra | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
        assert!(report.contains(
            "adversarial-reviewer-claude | claude-opus-4-6 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 9m 00s"
        ));
    }

    // Regression: 52505603 charged every oversize ruling to the owner's
    // LATEST launch, so a mid-task relaunch under another model absorbed the
    // earlier model's incidents — the comparison the column exists to inform.
    #[test]
    fn oversize_rulings_attribute_to_the_launch_active_at_the_ruling_time() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        for (minute, model) in [(0, "gpt-5.6-sol"), (6, "gpt-6-astra")] {
            crate::coordination::stores::telemetry::append_task_telemetry(
                &default_teams,
                "routing-team",
                Some("48"),
                &crate::coordination::stores::telemetry::RoutingTelemetryEvent::LaunchRendered {
                    timestamp: Utc.with_ymd_and_hms(2026, 9, 3, 10, minute, 0).unwrap(),
                    task_id: Some("48".to_string()),
                    member: "builder".to_string(),
                    role: "astra-heavy-implementer".to_string(),
                    tool: "codex".to_string(),
                    model: Some(model.to_string()),
                    applied_effort: Some("high".to_string()),
                    capability_tier: Some("frontier".to_string()),
                    tier_rank: Some(0),
                },
            )
            .expect("record launch");
        }
        crate::coordination::stores::telemetry::append_task_telemetry(
            &default_teams,
            "routing-team",
            Some("48"),
            &crate::coordination::stores::telemetry::RoutingTelemetryEvent::CompletionObserved {
                timestamp: Utc.with_ymd_and_hms(2026, 9, 3, 10, 10, 0).unwrap(),
                observed_at: Some(Utc.with_ymd_and_hms(2026, 9, 3, 10, 10, 0).unwrap()),
                task_id: "48".to_string(),
                status: "completed".to_string(),
                has_review_ruling: true,
            },
        )
        .expect("record completion");
        write_json(
            &root.path().join("personal/tasks/routing-team/48.json"),
            serde_json::json!({
                "id": "48",
                "subject": "Relaunched under another model mid-task",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [
                    {"seq": 1, "kind": "ruling", "field": "oversize_diff", "value": "failed", "by": "reviewer", "at": "2026-09-03T10:03:00Z", "note": "budget 200 lines; actual 260"},
                    {"seq": 2, "kind": "verdict", "value": "accepted", "by": "reviewer", "at": "2026-09-03T10:08:00Z"},
                    {"seq": 3, "kind": "ruling", "field": "oversize_diff", "value": "failed", "by": "reviewer", "at": "2026-09-03T10:09:00Z", "note": "budget 200 lines; actual 240"}
                ]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "astra-heavy-implementer | gpt-5.6-sol | 1 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
        assert!(report.contains(
            "astra-heavy-implementer | gpt-6-astra | 1 | 1 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 4m 00s"
        ));
        assert!(report.contains("gpt-5.6-sol | 1 | 1 | 0 | 1"));
        assert!(report.contains("gpt-6-astra | 1 | 1 | 0 | 1"));
    }

    // Regression: 52505603 charged every oversize ruling to the owner's
    // LATEST launch, so a mid-task relaunch under another model absorbed the
    // earlier model's incidents — the comparison the column exists to inform.
    #[test]
    fn wave2_budget_rulings_attribute_to_the_launch_active_at_the_ruling_time() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        for (minute, model) in [(0, "gpt-5.6-sol"), (6, "gpt-6-astra")] {
            crate::coordination::stores::telemetry::append_task_telemetry(
                &default_teams,
                "routing-team",
                Some("48"),
                &crate::coordination::stores::telemetry::RoutingTelemetryEvent::LaunchRendered {
                    timestamp: Utc.with_ymd_and_hms(2026, 9, 3, 10, minute, 0).unwrap(),
                    task_id: Some("48".to_string()),
                    member: "builder".to_string(),
                    role: "astra-heavy-implementer".to_string(),
                    tool: "codex".to_string(),
                    model: Some(model.to_string()),
                    applied_effort: Some("high".to_string()),
                    capability_tier: Some("frontier".to_string()),
                    tier_rank: Some(0),
                },
            )
            .expect("record launch");
        }
        crate::coordination::stores::telemetry::append_task_telemetry(
            &default_teams,
            "routing-team",
            Some("48"),
            &crate::coordination::stores::telemetry::RoutingTelemetryEvent::CompletionObserved {
                timestamp: Utc.with_ymd_and_hms(2026, 9, 3, 10, 10, 0).unwrap(),
                observed_at: Some(Utc.with_ymd_and_hms(2026, 9, 3, 10, 10, 0).unwrap()),
                task_id: "48".to_string(),
                status: "completed".to_string(),
                has_review_ruling: true,
            },
        )
        .expect("record completion");
        write_json(
            &root.path().join("personal/tasks/routing-team/48.json"),
            serde_json::json!({
                "id": "48",
                "subject": "Relaunched under another model mid-task",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [
                    {"seq": 1, "kind": "ruling", "field": "budget_raised", "value": "200→260", "by": "reviewer", "at": "2026-09-03T10:03:00Z", "note": "budget 200 lines; actual 260"},
                    {"seq": 2, "kind": "verdict", "value": "accepted", "by": "reviewer", "at": "2026-09-03T10:08:00Z"},
                    {"seq": 3, "kind": "ruling", "field": "budget_raised", "value": "200→260", "by": "reviewer", "at": "2026-09-03T10:09:00Z", "note": "budget 200 lines; actual 240"}
                ]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "astra-heavy-implementer | gpt-5.6-sol | 1 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
        assert!(report.contains(
            "astra-heavy-implementer | gpt-6-astra | 1 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 | 0 | 4m 00s"
        ));
        assert!(report.contains("gpt-5.6-sol | 1 | 1 | 0 | 0 | 1"));
        assert!(report.contains("gpt-6-astra | 1 | 1 | 0 | 0 | 1"));
    }

    // Regression: 52505603 counted any `field: oversize_diff` ruling as an
    // incident while the scanner only excluded `value: failed` from review
    // acceptance — the two layers disagreed about a waived ruling. Both now
    // share task_scanner::claude::is_oversize_failure.
    #[test]
    fn waived_oversize_rulings_neither_count_as_incidents_nor_block_acceptance() {
        let root = tempfile::tempdir().expect("tempdir");
        let default_teams = root.path().join("personal/teams");
        write_json(
            &default_teams.join("routing-team/config.json"),
            serde_json::json!({"name": "routing-team", "members": []}),
        );
        write_sidecar(
            &default_teams,
            "routing-team",
            "46",
            "astra-heavy-implementer",
            "gpt-6-astra",
            "2026-09-03T10:10:00Z",
            true,
        );
        write_json(
            &root.path().join("personal/tasks/routing-team/46.json"),
            serde_json::json!({
                "id": "46",
                "subject": "Oversize waived by the lead",
                "description": null,
                "activeForm": null,
                "status": "completed",
                "blocks": [],
                "blockedBy": [],
                "owner": "builder",
                "metadata": {"rulings": [{
                    "seq": 1,
                    "kind": "ruling",
                    "field": "oversize_diff",
                    "value": "waived",
                    "by": "reviewer",
                    "at": "2026-09-03T10:09:00Z",
                    "note": "budget 200 lines; actual 210 — lead waived"
                }]}
            }),
        );

        let report = render_routing_report(
            &default_teams,
            30,
            Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap(),
        )
        .expect("render report");

        assert!(report.contains(
            "astra-heavy-implementer | gpt-6-astra | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 10m 00s"
        ));
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
            "rust-developer | gpt-5.6-sol | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 10m 00s"
        ));
    }
}
