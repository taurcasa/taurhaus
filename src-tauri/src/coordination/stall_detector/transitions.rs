use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::types::{
    MemberKey, MemberStallState, MeshMemberStatus, NudgeCountWindow, SignalSnapshot,
    StageTransition, StallDetectorConfig, StallSignalSnapshot, StallStage,
    StallSuppressionSnapshot, StallTriggerStage, TransitionDecision,
};

pub(super) fn set_if_newer(target: &mut Option<DateTime<Utc>>, observed_at: DateTime<Utc>) {
    match target {
        Some(current) if observed_at <= *current => {}
        _ => *target = Some(observed_at),
    }
}

fn elapsed_secs(now: DateTime<Utc>, then: DateTime<Utc>) -> u64 {
    let elapsed = now.signed_duration_since(then).num_seconds();
    if elapsed <= 0 {
        0
    } else {
        elapsed as u64
    }
}

pub(super) fn is_long_running_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    const ALLOWLIST: &[&str] = &[
        "cargo", "bun", "vitest", "wdio", "pnpm", "npm", "yarn", "pytest", "nextest", "just", "go",
    ];

    let first = normalized.split_whitespace().next().unwrap_or_default();
    let first = first.rsplit('/').next().unwrap_or(first);
    ALLOWLIST.iter().any(|token| {
        first == *token
            || first.ends_with(token)
            || normalized.contains(&format!(" {} ", token))
            || normalized.starts_with(&format!("{token} "))
    })
}

fn can_issue_nudge(
    nudge_window_secs: u64,
    window: &NudgeCountWindow,
    now: DateTime<Utc>,
    max_nudges_per_hour: u32,
) -> bool {
    match window.window_started_at {
        None => true,
        Some(started_at) => {
            let elapsed = now.signed_duration_since(started_at).num_seconds();
            if elapsed < 0 || elapsed >= nudge_window_secs as i64 {
                true
            } else {
                window.count < max_nudges_per_hour
            }
        }
    }
}

pub(super) fn evaluate_transitions(
    config: &StallDetectorConfig,
    member_states: &mut HashMap<MemberKey, MemberStallState>,
    snapshots_by_member: &HashMap<MemberKey, SignalSnapshot>,
    now: DateTime<Utc>,
) -> Vec<TransitionDecision> {
    let mut transitions = Vec::new();

    for (key, state) in member_states.iter_mut() {
        let suppression_snapshot = StallSuppressionSnapshot {
            suppressed: false,
            reason: None,
            suppression_until: state.suppression_until,
        };
        let snapshot = snapshots_by_member.get(key).cloned();

        if state.pending_nudge_id.is_some() && state.stage == StallStage::Healthy {
            continue;
        }

        if let Some(suppression_until) = state.suppression_until {
            if suppression_until > now {
                continue;
            }
        }

        if let Some(inbound_at) = state.last_inbound_message_at {
            if elapsed_secs(now, inbound_at) < config.post_message_grace_secs {
                continue;
            }
        }

        if state.stage == StallStage::Healthy
            && state.last_nudge_at.is_some_and(|nudge_at| {
                elapsed_secs(now, nudge_at) < config.post_nudge_cooldown_secs
            })
        {
            continue;
        }

        let Some(last_signal_at) = state.freshest_signal_at() else {
            continue;
        };

        if state.stage == StallStage::Healthy
            && state
                .last_nudge_at
                .is_some_and(|nudge_at| last_signal_at <= nudge_at)
        {
            continue;
        }

        let idle_secs = elapsed_secs(now, last_signal_at);
        if let Some(snapshot) = snapshot.as_ref() {
            if snapshot.session_is_strong(config.require_medium_confidence_for_activity) {
                continue;
            }

            if snapshot
                .pane_current_command
                .as_ref()
                .is_some_and(|command| is_long_running_command(command))
                && snapshot.pane_is_shell != Some(true)
            {
                continue;
            }

            match snapshot.mesh_status {
                Some(MeshMemberStatus::Blocked) => continue,
                Some(MeshMemberStatus::Investigating) => continue,
                Some(MeshMemberStatus::Working)
                | Some(MeshMemberStatus::Idle)
                | Some(MeshMemberStatus::Unknown)
                | None => {}
            }

            if snapshot.session_state.is_some() {
                state.uncertainty_defer_active = false;
            }
        }

        match state.stage {
            StallStage::Healthy => {
                if idle_secs < config.soft_nudge_after_secs {
                    continue;
                }
                if !can_issue_nudge(
                    config.nudge_window_secs,
                    &state.nudge_count_window,
                    now,
                    config.max_nudges_per_hour,
                ) {
                    continue;
                }

                let signal_snapshot = StallSignalSnapshot {
                    stage_before: StallStage::Healthy,
                    last_strong_signal_at: state.last_strong_signal_at,
                    last_any_signal_at: state.last_any_signal_at,
                    last_inbound_message_at: state.last_inbound_message_at,
                    idle_secs,
                };
                state.stage = StallStage::SoftNudged;
                state.last_nudge_at = Some(now);
                state.pending_nudge_id = Some(format!("nudge-{}", now.timestamp_millis()));
                state
                    .nudge_count_window
                    .record(now, config.nudge_window_secs);
                transitions.push(TransitionDecision {
                    transition: StageTransition {
                        team_name: key.team_name.clone(),
                        member_name: key.member_name.clone(),
                        from: StallStage::Healthy,
                        to: StallStage::SoftNudged,
                        at: now,
                    },
                    trigger_stage: StallTriggerStage::StageA,
                    signal_snapshot,
                    suppression_snapshot,
                    runtime_snapshot: snapshot.clone(),
                    pending_nudge_id: state.pending_nudge_id.clone(),
                    last_nudge_at: state.last_nudge_at,
                });
            }
            StallStage::SoftNudged => {
                if idle_secs < config.hard_escalate_after_secs {
                    continue;
                }
                let uncertain = snapshot
                    .as_ref()
                    .is_none_or(|signal| signal.session_state.is_none());
                if uncertain && !state.uncertainty_defer_active {
                    state.uncertainty_defer_active = true;
                    continue;
                }
                let signal_snapshot = StallSignalSnapshot {
                    stage_before: StallStage::SoftNudged,
                    last_strong_signal_at: state.last_strong_signal_at,
                    last_any_signal_at: state.last_any_signal_at,
                    last_inbound_message_at: state.last_inbound_message_at,
                    idle_secs,
                };
                state.stage = StallStage::Escalated;
                state.last_escalation_at = Some(now);
                state.uncertainty_defer_active = false;
                transitions.push(TransitionDecision {
                    transition: StageTransition {
                        team_name: key.team_name.clone(),
                        member_name: key.member_name.clone(),
                        from: StallStage::SoftNudged,
                        to: StallStage::Escalated,
                        at: now,
                    },
                    trigger_stage: StallTriggerStage::StageB,
                    signal_snapshot,
                    suppression_snapshot,
                    runtime_snapshot: snapshot.clone(),
                    pending_nudge_id: state.pending_nudge_id.clone(),
                    last_nudge_at: state.last_nudge_at,
                });
            }
            StallStage::Escalated => {}
        }
    }

    transitions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::{ActivityConfidence, SessionState};

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn member_key() -> MemberKey {
        MemberKey {
            team_name: "team-a".to_string(),
            member_name: "agent-a".to_string(),
        }
    }

    fn state_with_last_signal(now: DateTime<Utc>, seconds_ago: i64) -> MemberStallState {
        let mut state = MemberStallState::new(now);
        state.last_any_signal_at = Some(now - chrono::Duration::seconds(seconds_ago));
        state
    }

    fn snapshot(now: DateTime<Utc>) -> SignalSnapshot {
        SignalSnapshot {
            team_name: "team-a".to_string(),
            member_name: "agent-a".to_string(),
            observed_at: now,
            session_state: None,
            session_confidence: None,
            pane_exists: None,
            pane_is_dead: None,
            pane_is_shell: None,
            pane_current_command: None,
            mesh_last_activity_at: None,
            mesh_status: None,
            coordination_event_at: None,
            project_file_write_at: None,
            runtime_last_seen_at: None,
            strongest_signal: None,
        }
    }

    #[test]
    fn healthy_members_transition_to_stage_a_at_soft_threshold() {
        let now = ts("2026-03-05T12:10:00Z");
        let mut member_states = HashMap::from([(member_key(), state_with_last_signal(now, 300))]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now,
        );

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].transition.from, StallStage::Healthy);
        assert_eq!(transitions[0].transition.to, StallStage::SoftNudged);
        assert_eq!(
            member_states.get(&member_key()).expect("state").stage,
            StallStage::SoftNudged
        );
    }

    #[test]
    fn soft_nudged_members_escalate_after_second_uncertain_check() {
        let now = ts("2026-03-05T12:20:00Z");
        let mut state = state_with_last_signal(now, 540);
        state.stage = StallStage::SoftNudged;
        let mut member_states = HashMap::from([(member_key(), state)]);

        let first = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now,
        );
        assert!(first.is_empty());

        let second = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now + chrono::Duration::seconds(30),
        );
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].transition.to, StallStage::Escalated);
    }

    #[test]
    fn members_do_not_transition_before_soft_threshold() {
        let now = ts("2026-03-05T12:05:00Z");
        let mut member_states = HashMap::from([(member_key(), state_with_last_signal(now, 299))]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now,
        );

        assert!(transitions.is_empty());
        assert_eq!(
            member_states.get(&member_key()).expect("state").stage,
            StallStage::Healthy
        );
    }

    #[test]
    fn blocked_mesh_status_suppresses_stage_a() {
        let now = ts("2026-03-05T14:00:00Z");
        let mut member_states = HashMap::from([(member_key(), state_with_last_signal(now, 400))]);
        let mut runtime_snapshot = snapshot(now);
        runtime_snapshot.mesh_status = Some(MeshMemberStatus::Blocked);
        let snapshots = HashMap::from([(member_key(), runtime_snapshot)]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &snapshots,
            now,
        );

        assert!(transitions.is_empty());
    }

    #[test]
    fn non_allowlisted_short_command_does_not_suppress_stage_a() {
        let now = ts("2026-03-05T14:10:00Z");
        let mut member_states = HashMap::from([(member_key(), state_with_last_signal(now, 301))]);
        let mut runtime_snapshot = snapshot(now);
        runtime_snapshot.pane_current_command = Some("ls -la".to_string());
        runtime_snapshot.pane_is_shell = Some(false);
        let snapshots = HashMap::from([(member_key(), runtime_snapshot)]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &snapshots,
            now,
        );

        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].transition.to, StallStage::SoftNudged);
    }

    #[test]
    fn pending_nudge_blocks_second_stage_a() {
        let now = ts("2026-03-05T14:20:00Z");
        let mut state = state_with_last_signal(now, 500);
        state.pending_nudge_id = Some("nudge-1".to_string());
        let mut member_states = HashMap::from([(member_key(), state)]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now,
        );

        assert!(transitions.is_empty());
    }

    #[test]
    fn evidence_must_advance_before_re_nudge() {
        let now = ts("2026-03-05T14:30:00Z");
        let last_signal = now - chrono::Duration::seconds(600);
        let mut state = MemberStallState::new(now);
        state.last_any_signal_at = Some(last_signal);
        state.last_nudge_at = Some(last_signal + chrono::Duration::seconds(60));
        let mut member_states = HashMap::from([(member_key(), state)]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now,
        );

        assert!(transitions.is_empty());
    }

    #[test]
    fn max_nudges_per_hour_is_enforced() {
        let now = ts("2026-03-05T14:40:00Z");
        let mut state = state_with_last_signal(now, 600);
        state.nudge_count_window.window_started_at = Some(now - chrono::Duration::minutes(10));
        state.nudge_count_window.count = 3;
        let mut member_states = HashMap::from([(member_key(), state)]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &HashMap::new(),
            now,
        );

        assert!(transitions.is_empty());
    }

    #[test]
    fn strong_session_suppresses_transitions() {
        let now = ts("2026-03-05T14:50:00Z");
        let mut member_states = HashMap::from([(member_key(), state_with_last_signal(now, 600))]);
        let mut runtime_snapshot = snapshot(now);
        runtime_snapshot.session_state = Some(SessionState::Active);
        runtime_snapshot.session_confidence = Some(ActivityConfidence::High);
        let snapshots = HashMap::from([(member_key(), runtime_snapshot)]);

        let transitions = evaluate_transitions(
            &StallDetectorConfig::default(),
            &mut member_states,
            &snapshots,
            now,
        );

        assert!(transitions.is_empty());
    }
}
