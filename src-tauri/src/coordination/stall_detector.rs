//! Stall detector core service.
//!
//! Provides per-member in-memory state, configurable thresholds, polling,
//! and stage transitions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::coordination::errors::CoordinationError;

const NUDGE_WINDOW_SECS: i64 = 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StallStage {
    Healthy,
    SoftNudged,
    Escalated,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MemberKey {
    team_name: String,
    member_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeCountWindow {
    pub window_started_at: Option<DateTime<Utc>>,
    pub count: u32,
}

impl Default for NudgeCountWindow {
    fn default() -> Self {
        Self {
            window_started_at: None,
            count: 0,
        }
    }
}

impl NudgeCountWindow {
    fn record(&mut self, now: DateTime<Utc>) {
        match self.window_started_at {
            None => {
                self.window_started_at = Some(now);
                self.count = 1;
            }
            Some(started_at) => {
                let elapsed = now.signed_duration_since(started_at).num_seconds();
                if elapsed < 0 || elapsed >= NUDGE_WINDOW_SECS {
                    self.window_started_at = Some(now);
                    self.count = 1;
                } else {
                    self.count = self.count.saturating_add(1);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberStallState {
    pub last_strong_signal_at: Option<DateTime<Utc>>,
    pub last_any_signal_at: Option<DateTime<Utc>>,
    pub last_inbound_message_at: Option<DateTime<Utc>>,
    pub last_nudge_at: Option<DateTime<Utc>>,
    pub last_escalation_at: Option<DateTime<Utc>>,
    pub pending_nudge_id: Option<String>,
    pub suppression_until: Option<DateTime<Utc>>,
    pub stage: StallStage,
    pub nudge_count_window: NudgeCountWindow,
}

impl MemberStallState {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            last_strong_signal_at: None,
            last_any_signal_at: Some(now),
            last_inbound_message_at: None,
            last_nudge_at: None,
            last_escalation_at: None,
            pending_nudge_id: None,
            suppression_until: None,
            stage: StallStage::Healthy,
            nudge_count_window: NudgeCountWindow::default(),
        }
    }

    fn freshest_signal_at(&self) -> Option<DateTime<Utc>> {
        match (self.last_strong_signal_at, self.last_any_signal_at) {
            (Some(strong), Some(any)) => Some(std::cmp::max(strong, any)),
            (Some(strong), None) => Some(strong),
            (None, Some(any)) => Some(any),
            (None, None) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshMemberStatus {
    Working,
    Blocked,
    Investigating,
    Idle,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTransition {
    pub team_name: String,
    pub member_name: String,
    pub from: StallStage,
    pub to: StallStage,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StallDetectorConfig {
    pub poll_interval_secs: u64,
    pub soft_nudge_after_secs: u64,
    pub hard_escalate_after_secs: u64,
    pub post_message_grace_secs: u64,
    pub post_nudge_cooldown_secs: u64,
    pub max_nudges_per_hour: u32,
    pub require_medium_confidence_for_activity: bool,
}

impl Default for StallDetectorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            soft_nudge_after_secs: 300,
            hard_escalate_after_secs: 540,
            post_message_grace_secs: 120,
            post_nudge_cooldown_secs: 240,
            max_nudges_per_hour: 3,
            require_medium_confidence_for_activity: true,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StallDetectorConfigWire {
    poll_interval_secs: Option<u64>,
    soft_nudge_after_secs: Option<u64>,
    hard_escalate_after_secs: Option<u64>,
    post_message_grace_secs: Option<u64>,
    post_nudge_cooldown_secs: Option<u64>,
    max_nudges_per_hour: Option<u32>,
    require_medium_confidence_for_activity: Option<bool>,
}

impl StallDetectorConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }

    pub fn from_team_config_value(value: &Value) -> Result<Self, CoordinationError> {
        let mut config = Self::default();
        let Some(stall_detection) = value.get("stall_detection") else {
            return Ok(config);
        };

        let wire: StallDetectorConfigWire = serde_json::from_value(stall_detection.clone())
            .map_err(|err| {
                CoordinationError::Validation(format!(
                    "invalid stall_detection config section: {err}"
                ))
            })?;

        if let Some(v) = wire.poll_interval_secs {
            config.poll_interval_secs = v;
        }
        if let Some(v) = wire.soft_nudge_after_secs {
            config.soft_nudge_after_secs = v;
        }
        if let Some(v) = wire.hard_escalate_after_secs {
            config.hard_escalate_after_secs = v;
        }
        if let Some(v) = wire.post_message_grace_secs {
            config.post_message_grace_secs = v;
        }
        if let Some(v) = wire.post_nudge_cooldown_secs {
            config.post_nudge_cooldown_secs = v;
        }
        if let Some(v) = wire.max_nudges_per_hour {
            config.max_nudges_per_hour = v;
        }
        if let Some(v) = wire.require_medium_confidence_for_activity {
            config.require_medium_confidence_for_activity = v;
        }

        config.validate()?;
        Ok(config)
    }

    pub fn from_team_config_json(raw: &str) -> Result<Self, CoordinationError> {
        let value: Value = serde_json::from_str(raw).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to parse team config for stall detector settings: {err}"
            ))
        })?;
        Self::from_team_config_value(&value)
    }

    fn validate(&self) -> Result<(), CoordinationError> {
        if self.poll_interval_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.poll_interval_secs must be > 0".to_string(),
            ));
        }
        if self.soft_nudge_after_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.soft_nudge_after_secs must be > 0".to_string(),
            ));
        }
        if self.hard_escalate_after_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.hard_escalate_after_secs must be > 0".to_string(),
            ));
        }
        if self.post_message_grace_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.post_message_grace_secs must be > 0".to_string(),
            ));
        }
        if self.post_nudge_cooldown_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.post_nudge_cooldown_secs must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

struct PollerHandle {
    stop_tx: Sender<()>,
    join_handle: JoinHandle<()>,
}

pub struct StallDetectorService {
    config: StallDetectorConfig,
    member_states: Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    polling_ticks: Arc<AtomicU64>,
    poller: Option<PollerHandle>,
}

impl std::fmt::Debug for StallDetectorService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StallDetectorService")
            .field("config", &self.config)
            .field(
                "member_states_len",
                &self
                    .member_states
                    .lock()
                    .map(|states| states.len())
                    .unwrap_or_default(),
            )
            .field("polling_ticks", &self.polling_ticks.load(Ordering::Relaxed))
            .finish()
    }
}

impl StallDetectorService {
    pub fn new(config: StallDetectorConfig) -> Self {
        Self {
            config,
            member_states: Arc::new(Mutex::new(HashMap::new())),
            polling_ticks: Arc::new(AtomicU64::new(0)),
            poller: None,
        }
    }

    pub fn config(&self) -> &StallDetectorConfig {
        &self.config
    }

    pub fn polling_tick_count(&self) -> u64 {
        self.polling_ticks.load(Ordering::Relaxed)
    }

    pub fn upsert_member(&self, team_name: &str, member_name: &str, now: DateTime<Utc>) {
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            states
                .entry(key)
                .or_insert_with(|| MemberStallState::new(now));
        }
    }

    pub fn member_state(&self, team_name: &str, member_name: &str) -> Option<MemberStallState> {
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        self.member_states
            .lock()
            .ok()
            .and_then(|states| states.get(&key).cloned())
    }

    pub fn set_last_any_signal_for_tests(
        &self,
        team_name: &str,
        member_name: &str,
        at: DateTime<Utc>,
    ) {
        self.upsert_member(team_name, member_name, at);
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                state.last_any_signal_at = Some(at);
            }
        }
    }

    pub fn set_stage_for_tests(&self, team_name: &str, member_name: &str, stage: StallStage) {
        self.upsert_member(team_name, member_name, Utc::now());
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                state.stage = stage;
            }
        }
    }

    pub fn ingest_session_signal(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
        is_strong: bool,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                set_if_newer(&mut state.last_any_signal_at, observed_at);
                if is_strong {
                    set_if_newer(&mut state.last_strong_signal_at, observed_at);
                }
            }
        }
    }

    pub fn ingest_pane_check(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
        pane_alive: bool,
        _pane_is_shell: bool,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        if pane_alive {
            let key = MemberKey {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
            };
            if let Ok(mut states) = self.member_states.lock() {
                if let Some(state) = states.get_mut(&key) {
                    set_if_newer(&mut state.last_any_signal_at, observed_at);
                }
            }
        }
    }

    pub fn ingest_mesh_heartbeat(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                set_if_newer(&mut state.last_any_signal_at, observed_at);
            }
        }
    }

    pub fn ingest_mesh_status(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
        status: MeshMemberStatus,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                match status {
                    MeshMemberStatus::Working | MeshMemberStatus::Investigating => {
                        set_if_newer(&mut state.last_any_signal_at, observed_at);
                        if matches!(status, MeshMemberStatus::Working) {
                            set_if_newer(&mut state.last_strong_signal_at, observed_at);
                        }
                    }
                    MeshMemberStatus::Blocked
                    | MeshMemberStatus::Idle
                    | MeshMemberStatus::Unknown => {}
                }
            }
        }
    }

    pub fn poll_once(&self) -> Vec<StageTransition> {
        self.poll_once_at(Utc::now())
    }

    pub fn poll_once_at(&self, now: DateTime<Utc>) -> Vec<StageTransition> {
        let Ok(mut states) = self.member_states.lock() else {
            return Vec::new();
        };
        evaluate_transitions(&self.config, &mut states, now)
    }

    pub fn start_polling(&mut self) -> Result<(), CoordinationError> {
        if self.poller.is_some() {
            return Err(CoordinationError::Conflict(
                "stall detector polling is already active".to_string(),
            ));
        }

        let interval = self.config.poll_interval();
        let config = self.config.clone();
        let member_states = Arc::clone(&self.member_states);
        let polling_ticks = Arc::clone(&self.polling_ticks);
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let join_handle = thread::spawn(move || loop {
            match stop_rx.recv_timeout(interval) {
                Ok(_) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    polling_ticks.fetch_add(1, Ordering::Relaxed);
                    if let Ok(mut states) = member_states.lock() {
                        let _ = evaluate_transitions(&config, &mut states, Utc::now());
                    }
                }
            }
        });

        self.poller = Some(PollerHandle {
            stop_tx,
            join_handle,
        });

        Ok(())
    }

    pub fn stop_polling(&mut self) -> Result<(), CoordinationError> {
        let Some(poller) = self.poller.take() else {
            return Ok(());
        };

        let _ = poller.stop_tx.send(());
        poller.join_handle.join().map_err(|_| {
            CoordinationError::Backend("stall detector polling thread panicked".to_string())
        })?;
        Ok(())
    }
}

fn set_if_newer(target: &mut Option<DateTime<Utc>>, observed_at: DateTime<Utc>) {
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

fn can_issue_nudge(
    window: &NudgeCountWindow,
    now: DateTime<Utc>,
    max_nudges_per_hour: u32,
) -> bool {
    match window.window_started_at {
        None => true,
        Some(started_at) => {
            let elapsed = now.signed_duration_since(started_at).num_seconds();
            if elapsed < 0 || elapsed >= NUDGE_WINDOW_SECS {
                true
            } else {
                window.count < max_nudges_per_hour
            }
        }
    }
}

fn evaluate_transitions(
    config: &StallDetectorConfig,
    member_states: &mut HashMap<MemberKey, MemberStallState>,
    now: DateTime<Utc>,
) -> Vec<StageTransition> {
    let mut transitions = Vec::new();

    for (key, state) in member_states.iter_mut() {
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
        let idle_secs = elapsed_secs(now, last_signal_at);

        match state.stage {
            StallStage::Healthy => {
                if idle_secs < config.soft_nudge_after_secs {
                    continue;
                }
                if !can_issue_nudge(&state.nudge_count_window, now, config.max_nudges_per_hour) {
                    continue;
                }

                state.stage = StallStage::SoftNudged;
                state.last_nudge_at = Some(now);
                state.pending_nudge_id = Some(format!("nudge-{}", now.timestamp_millis()));
                state.nudge_count_window.record(now);
                transitions.push(StageTransition {
                    team_name: key.team_name.clone(),
                    member_name: key.member_name.clone(),
                    from: StallStage::Healthy,
                    to: StallStage::SoftNudged,
                    at: now,
                });
            }
            StallStage::SoftNudged => {
                if idle_secs < config.hard_escalate_after_secs {
                    continue;
                }
                state.stage = StallStage::Escalated;
                state.last_escalation_at = Some(now);
                transitions.push(StageTransition {
                    team_name: key.team_name.clone(),
                    member_name: key.member_name.clone(),
                    from: StallStage::SoftNudged,
                    to: StallStage::Escalated,
                    at: now,
                });
            }
            StallStage::Escalated => {}
        }
    }

    transitions
}

impl Drop for StallDetectorService {
    fn drop(&mut self) {
        let _ = self.stop_polling();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn config_defaults_match_design() {
        let config = StallDetectorConfig::default();
        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.soft_nudge_after_secs, 300);
        assert_eq!(config.hard_escalate_after_secs, 540);
        assert_eq!(config.post_message_grace_secs, 120);
        assert_eq!(config.post_nudge_cooldown_secs, 240);
        assert_eq!(config.max_nudges_per_hour, 3);
        assert!(config.require_medium_confidence_for_activity);
    }

    #[test]
    fn config_deserializes_from_team_config_json_with_defaults_when_missing() {
        let raw = r#"{
          "name": "architecture-final",
          "created_at": "2026-03-05T12:00:00Z",
          "members": []
        }"#;
        let config = StallDetectorConfig::from_team_config_json(raw).expect("config parse");
        assert_eq!(config, StallDetectorConfig::default());
    }

    #[test]
    fn config_deserializes_overrides_from_team_config_json() {
        let raw = r#"{
          "name": "architecture-final",
          "created_at": "2026-03-05T12:00:00Z",
          "members": [],
          "stall_detection": {
            "poll_interval_secs": 15,
            "soft_nudge_after_secs": 120,
            "hard_escalate_after_secs": 300,
            "post_message_grace_secs": 45,
            "post_nudge_cooldown_secs": 90,
            "max_nudges_per_hour": 5,
            "require_medium_confidence_for_activity": false
          }
        }"#;

        let config = StallDetectorConfig::from_team_config_json(raw).expect("config parse");
        assert_eq!(config.poll_interval_secs, 15);
        assert_eq!(config.soft_nudge_after_secs, 120);
        assert_eq!(config.hard_escalate_after_secs, 300);
        assert_eq!(config.post_message_grace_secs, 45);
        assert_eq!(config.post_nudge_cooldown_secs, 90);
        assert_eq!(config.max_nudges_per_hour, 5);
        assert!(!config.require_medium_confidence_for_activity);
    }

    #[test]
    fn config_deserialization_rejects_zero_intervals() {
        let raw = r#"{
          "name": "architecture-final",
          "created_at": "2026-03-05T12:00:00Z",
          "members": [],
          "stall_detection": {
            "poll_interval_secs": 0
          }
        }"#;

        let err = StallDetectorConfig::from_team_config_json(raw).expect_err("should fail");
        match err {
            CoordinationError::Validation(message) => {
                assert!(message.contains("poll_interval_secs"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn ingest_session_signal_updates_strong_and_any_timestamps() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let at = ts("2026-03-05T12:00:00Z");
        service.ingest_session_signal("team-a", "agent-a", at, true);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.last_strong_signal_at, Some(at));
        assert_eq!(state.last_any_signal_at, Some(at));
    }

    #[test]
    fn ingest_pane_check_updates_any_timestamp_only() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let at = ts("2026-03-05T12:00:00Z");
        service.ingest_pane_check("team-a", "agent-a", at, true, false);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.last_strong_signal_at, None);
        assert_eq!(state.last_any_signal_at, Some(at));
    }

    #[test]
    fn poll_transitions_healthy_to_soft_nudged_at_soft_threshold() {
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
        assert_eq!(transitions[0].from, StallStage::Healthy);
        assert_eq!(transitions[0].to, StallStage::SoftNudged);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.stage, StallStage::SoftNudged);
        assert_eq!(state.last_nudge_at, Some(now));
    }

    #[test]
    fn poll_transitions_soft_nudged_to_escalated_at_hard_threshold() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T12:20:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_stage_for_tests("team-a", "agent-a", StallStage::SoftNudged);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(540),
        );

        let transitions = service.poll_once_at(now);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].from, StallStage::SoftNudged);
        assert_eq!(transitions[0].to, StallStage::Escalated);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.stage, StallStage::Escalated);
        assert_eq!(state.last_escalation_at, Some(now));
    }

    #[test]
    fn poll_does_not_transition_before_soft_threshold() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T12:05:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(299),
        );

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.stage, StallStage::Healthy);
    }

    #[test]
    fn nudge_count_window_rolls_after_one_hour() {
        let mut window = NudgeCountWindow::default();
        let start = ts("2026-03-05T12:00:00Z");
        window.record(start);
        window.record(start + ChronoDuration::minutes(10));
        assert_eq!(window.count, 2);

        window.record(start + ChronoDuration::minutes(61));
        assert_eq!(window.count, 1);
        assert_eq!(
            window.window_started_at,
            Some(start + ChronoDuration::minutes(61))
        );
    }

    #[test]
    fn polling_loop_uses_configurable_interval() {
        let mut service = StallDetectorService::new(StallDetectorConfig {
            poll_interval_secs: 1,
            ..StallDetectorConfig::default()
        });
        let now = Utc::now();
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(600),
        );

        service.start_polling().expect("start polling");
        std::thread::sleep(Duration::from_millis(1200));
        service.stop_polling().expect("stop polling");

        assert!(service.polling_tick_count() >= 1);
        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.stage, StallStage::SoftNudged);
    }
}
