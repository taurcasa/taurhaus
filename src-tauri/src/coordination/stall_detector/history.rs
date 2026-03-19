use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use crate::coordination::errors::CoordinationError;

use super::types::{
    MemberKey, MemberStallState, StallDetectorConfig, StallStage, StallTriggerRecord,
    StallWeeklyMetrics, TransitionDecision,
};

fn format_optional_ts(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn weekly_metrics(
    config: &StallDetectorConfig,
    trigger_history: &Arc<Mutex<Vec<StallTriggerRecord>>>,
    now: DateTime<Utc>,
) -> StallWeeklyMetrics {
    finalize_recovery_windows(config, trigger_history, now);
    let one_week_ago = now - chrono::Duration::days(7);
    let history = trigger_history
        .lock()
        .map(|history| history.clone())
        .unwrap_or_default();
    let recent: Vec<&StallTriggerRecord> = history
        .iter()
        .filter(|record| record.triggered_at >= one_week_ago)
        .collect();

    let stage_a_records: Vec<&StallTriggerRecord> = recent
        .iter()
        .copied()
        .filter(|record| record.stage == super::types::StallTriggerStage::StageA)
        .collect();
    let stage_b_records: Vec<&StallTriggerRecord> = recent
        .iter()
        .copied()
        .filter(|record| record.stage == super::types::StallTriggerStage::StageB)
        .collect();

    let stage_a_false_positive_rate = false_positive_rate(&stage_a_records);
    let stage_b_false_positive_rate = false_positive_rate(&stage_b_records);
    let mean_time_to_recovery_after_stage_a_secs =
        mean_secs(stage_a_records.iter().filter_map(|record| {
            record
                .resumed_at
                .map(|resumed_at| resumed_at.signed_duration_since(record.triggered_at))
        }));
    let mean_time_to_lead_intervention_after_stage_b_secs =
        mean_secs(stage_b_records.iter().filter_map(|record| {
            record
                .lead_intervened_at
                .map(|intervened_at| intervened_at.signed_duration_since(record.triggered_at))
        }));

    StallWeeklyMetrics {
        stage_a_alert_count: stage_a_records.len(),
        stage_b_escalation_count: stage_b_records.len(),
        stage_a_false_positive_rate,
        stage_b_false_positive_rate,
        mean_time_to_recovery_after_stage_a_secs,
        mean_time_to_lead_intervention_after_stage_b_secs,
    }
}

pub(super) fn annotate_trigger(
    trigger_history: &Arc<Mutex<Vec<StallTriggerRecord>>>,
    trigger_id: &str,
    confirmed_true_stall: bool,
    annotated_at: DateTime<Utc>,
) -> Result<(), CoordinationError> {
    let mut history = trigger_history.lock().map_err(|err| {
        CoordinationError::StoreError(format!("failed to lock stall trigger history: {err}"))
    })?;
    let Some(record) = history
        .iter_mut()
        .find(|entry| entry.trigger_id == trigger_id)
    else {
        return Err(CoordinationError::NotFound(format!(
            "stall trigger not found: {trigger_id}"
        )));
    };

    record.lead_confirmed_true_stall = Some(confirmed_true_stall);
    record.lead_annotation_at = Some(annotated_at);
    record.lead_intervened_at = Some(annotated_at);
    if record
        .resumed_within_recovery_window_without_intervention
        .is_none()
    {
        record.resumed_within_recovery_window_without_intervention = Some(false);
    }

    let payload = serde_json::to_string(record).unwrap_or_default();
    tracing::info!(
        event = "stall_trigger_annotation",
        trigger_id = %record.trigger_id,
        stage = %record.stage.as_str(),
        team_name = %record.team_name,
        member_name = %record.member_name,
        trigger_ts = %record.triggered_at.to_rfc3339(),
        annotated_at = %annotated_at.to_rfc3339(),
        lead_confirmed_true_stall = confirmed_true_stall,
        trigger_json = %payload,
    );

    Ok(())
}

pub(super) fn mark_recovery_if_resumed(
    config: &StallDetectorConfig,
    member_states: &Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    trigger_history: &Arc<Mutex<Vec<StallTriggerRecord>>>,
    team_name: &str,
    member_name: &str,
    observed_at: DateTime<Utc>,
) {
    let key = MemberKey {
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
    };
    if let Ok(mut states) = member_states.lock() {
        if let Some(state) = states.get_mut(&key) {
            if state.stage != StallStage::Healthy {
                state.stage = StallStage::Healthy;
                state.pending_nudge_id = None;
                state.uncertainty_defer_active = false;
                state.suppression_until = Some(
                    observed_at + chrono::Duration::seconds(config.post_nudge_cooldown_secs as i64),
                );
            }
        }
    }

    let Ok(mut history) = trigger_history.lock() else {
        return;
    };

    for record in history.iter_mut() {
        if record.team_name != team_name || record.member_name != member_name {
            continue;
        }
        if record
            .resumed_within_recovery_window_without_intervention
            .is_some()
        {
            continue;
        }
        if record.lead_intervened_at.is_some() {
            record.resumed_within_recovery_window_without_intervention = Some(false);
            continue;
        }

        let elapsed = observed_at
            .signed_duration_since(record.triggered_at)
            .num_seconds();
        if elapsed < 0 {
            continue;
        }
        if elapsed <= config.recovery_window_secs as i64 {
            record.resumed_within_recovery_window_without_intervention = Some(true);
            record.resumed_at = Some(observed_at);
        }
    }
}

pub(super) fn finalize_recovery_windows(
    config: &StallDetectorConfig,
    trigger_history: &Arc<Mutex<Vec<StallTriggerRecord>>>,
    now: DateTime<Utc>,
) {
    let Ok(mut history) = trigger_history.lock() else {
        return;
    };

    for record in history.iter_mut() {
        if record
            .resumed_within_recovery_window_without_intervention
            .is_some()
        {
            continue;
        }
        let elapsed = now.signed_duration_since(record.triggered_at).num_seconds();
        if elapsed > config.recovery_window_secs as i64 {
            record.resumed_within_recovery_window_without_intervention = Some(false);
        }
    }
}

pub(super) fn record_trigger_decisions(
    config: &StallDetectorConfig,
    trigger_history: &Arc<Mutex<Vec<StallTriggerRecord>>>,
    trigger_seq: &Arc<AtomicU64>,
    decisions: &[TransitionDecision],
) {
    if decisions.is_empty() {
        return;
    }
    let Ok(mut history) = trigger_history.lock() else {
        return;
    };
    for decision in decisions {
        let seq = trigger_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let record = StallTriggerRecord {
            trigger_id: format!("stall-trigger-{seq}"),
            team_name: decision.transition.team_name.clone(),
            member_name: decision.transition.member_name.clone(),
            stage: decision.trigger_stage,
            triggered_at: decision.transition.at,
            signal_snapshot: decision.signal_snapshot.clone(),
            suppression: decision.suppression_snapshot.clone(),
            resumed_within_recovery_window_without_intervention: None,
            resumed_at: None,
            lead_confirmed_true_stall: None,
            lead_annotation_at: None,
            lead_intervened_at: None,
        };
        emit_trigger_log(&record);
        if config.persist_trigger_history {
            history.push(record);
        }
    }
}

pub(super) fn render_stage_b_evidence_message(
    decision: &TransitionDecision,
    transition: &super::types::StageTransition,
) -> String {
    let signal = &decision.signal_snapshot;
    let runtime = decision.runtime_snapshot.as_ref();
    let strong_signal_age = signal
        .last_strong_signal_at
        .map(|at| (transition.at.signed_duration_since(at).num_seconds()).max(0))
        .unwrap_or(-1);
    let session_state = runtime
        .and_then(|snapshot| snapshot.session_state)
        .map(|state| format!("{state:?}"))
        .unwrap_or_else(|| "unknown".to_string());
    let session_confidence = runtime
        .and_then(|snapshot| snapshot.session_confidence)
        .map(|confidence| format!("{confidence:?}"))
        .unwrap_or_else(|| "unknown".to_string());
    let pane_state = runtime
        .map(|snapshot| {
            format!(
                "exists={:?}, dead={:?}, shell={:?}, cmd={}",
                snapshot.pane_exists,
                snapshot.pane_is_dead,
                snapshot.pane_is_shell,
                snapshot
                    .pane_current_command
                    .clone()
                    .unwrap_or_else(|| "none".to_string())
            )
        })
        .unwrap_or_else(|| "unknown".to_string());
    let strong_age = if strong_signal_age >= 0 {
        format!("{strong_signal_age}s")
    } else {
        "unknown".to_string()
    };

    format!(
        "Stage B stall escalation for {member}.\nEvidence: last strong signal age={strong_age}; session={session_state} ({session_confidence}); pane={pane_state}; nudge_id={nudge_id}; last_nudge_at={last_nudge_at}.\nStage C is manual intervention by team-lead.",
        member = transition.member_name,
        strong_age = strong_age,
        session_state = session_state,
        session_confidence = session_confidence,
        pane_state = pane_state,
        nudge_id = decision
            .pending_nudge_id
            .clone()
            .unwrap_or_else(|| "none".to_string()),
        last_nudge_at = format_optional_ts(decision.last_nudge_at),
    )
}

fn emit_trigger_log(record: &StallTriggerRecord) {
    let payload = serde_json::to_string(record).unwrap_or_default();
    tracing::info!(
        event = "stall_trigger",
        trigger_id = %record.trigger_id,
        stage = %record.stage.as_str(),
        team_name = %record.team_name,
        member_name = %record.member_name,
        trigger_ts = %record.triggered_at.to_rfc3339(),
        signal_snapshot = ?record.signal_snapshot,
        suppression = ?record.suppression,
        resumed_within_recovery_window_without_intervention = ?record.resumed_within_recovery_window_without_intervention,
        lead_confirmed_true_stall = ?record.lead_confirmed_true_stall,
        trigger_json = %payload,
    );
}

fn false_positive_rate(records: &[&StallTriggerRecord]) -> Option<f64> {
    let annotated: Vec<&StallTriggerRecord> = records
        .iter()
        .copied()
        .filter(|record| record.lead_confirmed_true_stall.is_some())
        .collect();
    if annotated.is_empty() {
        return None;
    }
    let false_positives = annotated
        .iter()
        .filter(|record| record.lead_confirmed_true_stall == Some(false))
        .count();
    Some(false_positives as f64 / annotated.len() as f64)
}

fn mean_secs(durations: impl Iterator<Item = chrono::Duration>) -> Option<f64> {
    let values: Vec<i64> = durations.map(|duration| duration.num_seconds()).collect();
    if values.is_empty() {
        return None;
    }
    let total: i64 = values.iter().sum();
    Some(total as f64 / values.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    use super::super::service::StallDetectorService;
    use super::super::types::{StallDetectorConfig, StallStage, StallTriggerStage};

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn poll_records_stage_trigger_history_with_signal_snapshot() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T12:10:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(300),
        );

        let transitions = service.poll_once_at(now);
        assert_eq!(transitions.len(), 1);

        let history = service.trigger_history();
        assert_eq!(history.len(), 1);
        let trigger = &history[0];
        assert_eq!(trigger.stage, StallTriggerStage::StageA);
        assert_eq!(trigger.signal_snapshot.idle_secs, 300);
        assert!(!trigger.suppression.suppressed);
        assert_eq!(
            trigger.resumed_within_recovery_window_without_intervention,
            None
        );
    }

    #[test]
    fn activity_within_recovery_window_marks_trigger_as_recovered() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T12:10:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(300),
        );
        let _ = service.poll_once_at(now);

        let recovery_at = now + ChronoDuration::seconds(60);
        service.ingest_mesh_heartbeat("team-a", "agent-a", recovery_at);

        let history = service.trigger_history();
        assert_eq!(history.len(), 1);
        assert_eq!(
            history[0].resumed_within_recovery_window_without_intervention,
            Some(true)
        );
        assert_eq!(history[0].resumed_at, Some(recovery_at));
    }

    #[test]
    fn recovery_window_expiry_marks_trigger_as_not_recovered() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T12:10:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(300),
        );
        let _ = service.poll_once_at(now);

        let after_window = now + ChronoDuration::seconds(130);
        let _ = service.poll_once_at(after_window);

        let history = service.trigger_history();
        assert_eq!(
            history[0].resumed_within_recovery_window_without_intervention,
            Some(false)
        );
    }

    #[test]
    fn weekly_metrics_compute_false_positive_rates_and_intervention_time() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let stage_a_now = ts("2026-03-05T12:10:00Z");
        service.upsert_member("team-a", "agent-a", stage_a_now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            stage_a_now - ChronoDuration::seconds(300),
        );
        let _ = service.poll_once_at(stage_a_now);
        service.ingest_mesh_heartbeat(
            "team-a",
            "agent-a",
            stage_a_now + ChronoDuration::seconds(60),
        );

        service.set_stage_for_tests("team-a", "agent-a", StallStage::SoftNudged);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            stage_a_now - ChronoDuration::seconds(540),
        );
        let stage_b_now = stage_a_now + ChronoDuration::seconds(400);
        let _ = service.poll_once_at(stage_b_now);
        let _ = service.poll_once_at(stage_b_now + ChronoDuration::seconds(30));

        let history = service.trigger_history();
        assert_eq!(history.len(), 2);
        let stage_a_id = history
            .iter()
            .find(|entry| entry.stage == StallTriggerStage::StageA)
            .map(|entry| entry.trigger_id.clone())
            .expect("stage a trigger id");
        let stage_b_id = history
            .iter()
            .find(|entry| entry.stage == StallTriggerStage::StageB)
            .map(|entry| entry.trigger_id.clone())
            .expect("stage b trigger id");

        service
            .annotate_trigger(
                &stage_a_id,
                false,
                stage_a_now + ChronoDuration::seconds(90),
            )
            .expect("annotate stage a");
        service
            .annotate_trigger(&stage_b_id, true, stage_b_now + ChronoDuration::seconds(60))
            .expect("annotate stage b");

        let metrics = service.weekly_metrics(stage_b_now + ChronoDuration::seconds(120));
        assert_eq!(metrics.stage_a_alert_count, 1);
        assert_eq!(metrics.stage_b_escalation_count, 1);
        assert_eq!(metrics.stage_a_false_positive_rate, Some(1.0));
        assert_eq!(metrics.stage_b_false_positive_rate, Some(0.0));
        assert!(metrics.mean_time_to_recovery_after_stage_a_secs.is_some());
        assert_eq!(
            metrics.mean_time_to_lead_intervention_after_stage_b_secs,
            Some(30.0)
        );
    }
}
