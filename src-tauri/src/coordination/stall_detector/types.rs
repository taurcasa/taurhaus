use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coordination::errors::CoordinationError;
use crate::session_scanner::{ActivityConfidence, SessionState};

use super::transitions::is_long_running_command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StallStage {
    Healthy,
    SoftNudged,
    Escalated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StallTriggerStage {
    StageA,
    StageB,
}

impl StallTriggerStage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::StageA => "stage_a",
            Self::StageB => "stage_b",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StallSuppressionReason {
    SuppressionUntil,
    PostMessageGrace,
    PostNudgeCooldown,
    RateLimited,
    PendingNudge,
    EvidenceNotAdvanced,
    LongRunningCommand,
    StrongActivity,
    ExplicitBlockedStatus,
    ExplicitInvestigatingStatus,
    SystemUncertainty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StallSuppressionSnapshot {
    pub suppressed: bool,
    pub reason: Option<StallSuppressionReason>,
    pub suppression_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StallSignalSnapshot {
    pub stage_before: StallStage,
    pub last_strong_signal_at: Option<DateTime<Utc>>,
    pub last_any_signal_at: Option<DateTime<Utc>>,
    pub last_inbound_message_at: Option<DateTime<Utc>>,
    pub idle_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StallTriggerRecord {
    pub trigger_id: String,
    pub team_name: String,
    pub member_name: String,
    pub stage: StallTriggerStage,
    pub triggered_at: DateTime<Utc>,
    pub signal_snapshot: StallSignalSnapshot,
    pub suppression: StallSuppressionSnapshot,
    pub resumed_within_recovery_window_without_intervention: Option<bool>,
    pub resumed_at: Option<DateTime<Utc>>,
    pub lead_confirmed_true_stall: Option<bool>,
    pub lead_annotation_at: Option<DateTime<Utc>>,
    pub lead_intervened_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct StallWeeklyMetrics {
    pub stage_a_alert_count: usize,
    pub stage_b_escalation_count: usize,
    pub stage_a_false_positive_rate: Option<f64>,
    pub stage_b_false_positive_rate: Option<f64>,
    pub mean_time_to_recovery_after_stage_a_secs: Option<f64>,
    pub mean_time_to_lead_intervention_after_stage_b_secs: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct MemberKey {
    pub(super) team_name: String,
    pub(super) member_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NudgeCountWindow {
    pub window_started_at: Option<DateTime<Utc>>,
    pub count: u32,
}

impl NudgeCountWindow {
    pub(crate) fn record(&mut self, now: DateTime<Utc>, nudge_window_secs: u64) {
        match self.window_started_at {
            None => {
                self.window_started_at = Some(now);
                self.count = 1;
            }
            Some(started_at) => {
                let elapsed = now.signed_duration_since(started_at).num_seconds();
                if elapsed < 0 || elapsed >= nudge_window_secs as i64 {
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
    pub uncertainty_defer_active: bool,
}

impl MemberStallState {
    pub(crate) fn new(now: DateTime<Utc>) -> Self {
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
            uncertainty_defer_active: false,
        }
    }

    pub(crate) fn freshest_signal_at(&self) -> Option<DateTime<Utc>> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStrength {
    Strong,
    Medium,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemberSignalContext {
    pub pane_id: Option<String>,
    pub project_path: Option<String>,
    pub coordination_event_at: Option<DateTime<Utc>>,
    pub project_file_write_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSnapshot {
    pub team_name: String,
    pub member_name: String,
    pub observed_at: DateTime<Utc>,
    pub session_state: Option<SessionState>,
    pub session_confidence: Option<ActivityConfidence>,
    pub pane_exists: Option<bool>,
    pub pane_is_dead: Option<bool>,
    pub pane_is_shell: Option<bool>,
    pub pane_current_command: Option<String>,
    pub mesh_last_activity_at: Option<DateTime<Utc>>,
    pub mesh_status: Option<MeshMemberStatus>,
    pub coordination_event_at: Option<DateTime<Utc>>,
    pub project_file_write_at: Option<DateTime<Utc>>,
    pub runtime_last_seen_at: Option<DateTime<Utc>>,
    pub strongest_signal: Option<SignalStrength>,
}

impl SignalSnapshot {
    pub(crate) fn session_is_strong(&self, require_medium_confidence: bool) -> bool {
        matches!(self.session_state, Some(SessionState::Active))
            && self.session_confidence.is_some_and(|confidence| {
                matches!(
                    confidence,
                    ActivityConfidence::High | ActivityConfidence::Medium
                ) || (!require_medium_confidence && confidence == ActivityConfidence::Low)
            })
    }

    pub(crate) fn pane_command_is_medium(&self) -> bool {
        self.pane_current_command
            .as_ref()
            .is_some_and(|cmd| is_long_running_command(cmd))
            && self.pane_is_shell != Some(true)
    }

    pub(crate) fn classify(&self, require_medium_confidence: bool) -> Option<SignalStrength> {
        if self.session_is_strong(require_medium_confidence) || self.pane_is_dead == Some(true) {
            return Some(SignalStrength::Strong);
        }
        if self.pane_command_is_medium() || self.coordination_event_at.is_some() {
            return Some(SignalStrength::Medium);
        }
        if self.project_file_write_at.is_some()
            || self.runtime_last_seen_at.is_some()
            || self.mesh_last_activity_at.is_some()
        {
            return Some(SignalStrength::Weak);
        }
        None
    }

    pub(super) fn selected_session_signal(
        &self,
        require_medium_confidence: bool,
    ) -> Option<SelectedSessionSignal> {
        let state = self.session_state?;
        let is_strong = self.session_is_strong(require_medium_confidence);
        if !matches!(state, SessionState::Active) || !is_strong {
            return None;
        }
        Some(SelectedSessionSignal {
            observed_at: self.observed_at,
            is_strong,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelectedSessionSignal {
    pub(super) observed_at: DateTime<Utc>,
    pub(super) is_strong: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionSignal {
    pub(super) pane_id: Option<String>,
    pub(super) project_path: String,
    pub(super) observed_at: DateTime<Utc>,
    pub(super) state: SessionState,
    pub(super) confidence: ActivityConfidence,
}

impl SessionSignal {
    pub(super) fn confidence_rank(&self) -> u8 {
        match self.confidence {
            ActivityConfidence::High => 3,
            ActivityConfidence::Medium => 2,
            ActivityConfidence::Low => 1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct MeshMemberSignal {
    pub(super) last_activity_at: Option<DateTime<Utc>>,
    pub(super) status: Option<MeshMemberStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TransitionDecision {
    pub(super) transition: StageTransition,
    pub(super) trigger_stage: StallTriggerStage,
    pub(super) signal_snapshot: StallSignalSnapshot,
    pub(super) suppression_snapshot: StallSuppressionSnapshot,
    pub(super) runtime_snapshot: Option<SignalSnapshot>,
    pub(super) pending_nudge_id: Option<String>,
    pub(super) last_nudge_at: Option<DateTime<Utc>>,
}

pub(super) type SessionScannerFn = dyn Fn(DateTime<Utc>) -> Vec<SessionSignal> + Send + Sync;
pub(super) type MeshSignalReaderFn =
    dyn Fn(&str) -> HashMap<String, MeshMemberSignal> + Send + Sync;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StallDetectorConfig {
    pub enabled: bool,
    pub poll_interval_secs: u64,
    pub soft_nudge_after_secs: u64,
    pub hard_escalate_after_secs: u64,
    pub post_message_grace_secs: u64,
    pub post_nudge_cooldown_secs: u64,
    pub nudge_window_secs: u64,
    pub recovery_window_secs: u64,
    pub max_nudges_per_hour: u32,
    pub persist_trigger_history: bool,
    pub require_medium_confidence_for_activity: bool,
}

impl Default for StallDetectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: 30,
            soft_nudge_after_secs: 300,
            hard_escalate_after_secs: 540,
            post_message_grace_secs: 120,
            post_nudge_cooldown_secs: 240,
            nudge_window_secs: 3600,
            recovery_window_secs: 120,
            max_nudges_per_hour: 3,
            persist_trigger_history: true,
            require_medium_confidence_for_activity: true,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct StallDetectorConfigWire {
    enabled: Option<bool>,
    poll_interval_secs: Option<u64>,
    soft_nudge_after_secs: Option<u64>,
    hard_escalate_after_secs: Option<u64>,
    post_message_grace_secs: Option<u64>,
    post_nudge_cooldown_secs: Option<u64>,
    nudge_window_secs: Option<u64>,
    recovery_window_secs: Option<u64>,
    max_nudges_per_hour: Option<u32>,
    persist_trigger_history: Option<bool>,
    require_medium_confidence_for_activity: Option<bool>,
}

impl StallDetectorConfig {
    pub fn poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.poll_interval_secs)
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

        if let Some(v) = wire.enabled {
            config.enabled = v;
        }
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
        if let Some(v) = wire.nudge_window_secs {
            config.nudge_window_secs = v;
        }
        if let Some(v) = wire.recovery_window_secs {
            config.recovery_window_secs = v;
        }
        if let Some(v) = wire.max_nudges_per_hour {
            config.max_nudges_per_hour = v;
        }
        if let Some(v) = wire.persist_trigger_history {
            config.persist_trigger_history = v;
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

    pub(crate) fn validate(&self) -> Result<(), CoordinationError> {
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
        if self.nudge_window_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.nudge_window_secs must be > 0".to_string(),
            ));
        }
        if self.recovery_window_secs == 0 {
            return Err(CoordinationError::Validation(
                "stall_detection.recovery_window_secs must be > 0".to_string(),
            ));
        }
        Ok(())
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
        assert!(config.enabled);
        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.soft_nudge_after_secs, 300);
        assert_eq!(config.hard_escalate_after_secs, 540);
        assert_eq!(config.post_message_grace_secs, 120);
        assert_eq!(config.post_nudge_cooldown_secs, 240);
        assert_eq!(config.nudge_window_secs, 3600);
        assert_eq!(config.recovery_window_secs, 120);
        assert_eq!(config.max_nudges_per_hour, 3);
        assert!(config.persist_trigger_history);
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
            "enabled": false,
            "poll_interval_secs": 15,
            "soft_nudge_after_secs": 120,
            "hard_escalate_after_secs": 300,
            "post_message_grace_secs": 45,
            "post_nudge_cooldown_secs": 90,
            "nudge_window_secs": 1800,
            "recovery_window_secs": 90,
            "max_nudges_per_hour": 5,
            "persist_trigger_history": false,
            "require_medium_confidence_for_activity": false
          }
        }"#;

        let config = StallDetectorConfig::from_team_config_json(raw).expect("config parse");
        assert!(!config.enabled);
        assert_eq!(config.poll_interval_secs, 15);
        assert_eq!(config.soft_nudge_after_secs, 120);
        assert_eq!(config.hard_escalate_after_secs, 300);
        assert_eq!(config.post_message_grace_secs, 45);
        assert_eq!(config.post_nudge_cooldown_secs, 90);
        assert_eq!(config.nudge_window_secs, 1800);
        assert_eq!(config.recovery_window_secs, 90);
        assert_eq!(config.max_nudges_per_hour, 5);
        assert!(!config.persist_trigger_history);
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
    fn nudge_count_window_rolls_after_one_hour() {
        let mut window = NudgeCountWindow::default();
        let start = ts("2026-03-05T12:00:00Z");
        window.record(start, 3600);
        window.record(start + ChronoDuration::minutes(10), 3600);
        assert_eq!(window.count, 2);

        window.record(start + ChronoDuration::minutes(61), 3600);
        assert_eq!(window.count, 1);
        assert_eq!(
            window.window_started_at,
            Some(start + ChronoDuration::minutes(61))
        );
    }
}
