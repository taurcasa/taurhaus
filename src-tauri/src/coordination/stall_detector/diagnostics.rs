use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};

use super::{StallDetectorConfig, StallTriggerRecord, TransitionDecision};

fn format_optional_ts(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn render_stage_b_evidence_message(
    decision: &TransitionDecision,
    transition: &super::StageTransition,
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

pub(super) fn emit_trigger_log(record: &StallTriggerRecord) {
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

pub(super) fn record_trigger_decisions_for_history(
    config: &StallDetectorConfig,
    trigger_history: &Arc<Mutex<Vec<StallTriggerRecord>>>,
    trigger_seq: &Arc<AtomicU64>,
    decisions: &[TransitionDecision],
) {
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

pub(super) fn finalize_recovery_windows_for_history(
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

pub(super) fn false_positive_rate(records: &[&StallTriggerRecord]) -> Option<f64> {
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

pub(super) fn mean_secs(durations: impl Iterator<Item = chrono::Duration>) -> Option<f64> {
    let values: Vec<i64> = durations.map(|duration| duration.num_seconds()).collect();
    if values.is_empty() {
        return None;
    }
    let total: i64 = values.iter().sum();
    Some(total as f64 / values.len() as f64)
}
