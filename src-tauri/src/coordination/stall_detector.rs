//! Stall detector core service.
//!
//! Provides per-member in-memory state, configurable thresholds, polling,
//! and stage transitions.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coordination::domain::MemberRole;
use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::requests::{DeliveryRequest, OperatorNoticeDelivery};
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::TeamConfigStore;
use crate::session_scanner::{scan_sessions, ActivityConfidence, SessionState};

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
    fn as_str(self) -> &'static str {
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
struct MemberKey {
    team_name: String,
    member_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NudgeCountWindow {
    pub window_started_at: Option<DateTime<Utc>>,
    pub count: u32,
}

impl NudgeCountWindow {
    fn record(&mut self, now: DateTime<Utc>, nudge_window_secs: u64) {
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
            uncertainty_defer_active: false,
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
    fn session_is_strong(&self, require_medium_confidence: bool) -> bool {
        matches!(self.session_state, Some(SessionState::Active))
            && self.session_confidence.is_some_and(|confidence| {
                matches!(
                    confidence,
                    ActivityConfidence::High | ActivityConfidence::Medium
                ) || (!require_medium_confidence && confidence == ActivityConfidence::Low)
            })
    }

    fn pane_command_is_medium(&self) -> bool {
        self.pane_current_command
            .as_ref()
            .is_some_and(|cmd| is_long_running_command(cmd))
            && self.pane_is_shell != Some(true)
    }

    fn classify(&self, require_medium_confidence: bool) -> Option<SignalStrength> {
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

    fn selected_session_signal(
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

struct SelectedSessionSignal {
    observed_at: DateTime<Utc>,
    is_strong: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSignal {
    pane_id: Option<String>,
    project_path: String,
    observed_at: DateTime<Utc>,
    state: SessionState,
    confidence: ActivityConfidence,
}

impl SessionSignal {
    fn confidence_rank(&self) -> u8 {
        match self.confidence {
            ActivityConfidence::High => 3,
            ActivityConfidence::Medium => 2,
            ActivityConfidence::Low => 1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MeshMemberSignal {
    last_activity_at: Option<DateTime<Utc>>,
    status: Option<MeshMemberStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransitionDecision {
    transition: StageTransition,
    trigger_stage: StallTriggerStage,
    signal_snapshot: StallSignalSnapshot,
    suppression_snapshot: StallSuppressionSnapshot,
    runtime_snapshot: Option<SignalSnapshot>,
    pending_nudge_id: Option<String>,
    last_nudge_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct MemberActivitySnapshot {
    version: u8,
    observed_at: String,
    stall_recent_activity: bool,
    stall_no_output: bool,
    stall_no_active_process: bool,
}

type SessionScannerFn = dyn Fn(DateTime<Utc>) -> Vec<SessionSignal> + Send + Sync;
type MeshSignalReaderFn = dyn Fn(&str) -> HashMap<String, MeshMemberSignal> + Send + Sync;

const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

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

struct PollerHandle {
    stop_tx: Sender<()>,
    join_handle: JoinHandle<()>,
}

pub struct StallDetectorService {
    config: StallDetectorConfig,
    teams_dir: PathBuf,
    member_states: Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    member_signal_contexts: Arc<Mutex<HashMap<MemberKey, MemberSignalContext>>>,
    trigger_history: Arc<Mutex<Vec<StallTriggerRecord>>>,
    trigger_seq: Arc<AtomicU64>,
    polling_ticks: Arc<AtomicU64>,
    runtime: Arc<dyn CoordinationRuntime>,
    session_scanner: Arc<SessionScannerFn>,
    mesh_signal_reader: Arc<MeshSignalReaderFn>,
    poller: Option<PollerHandle>,
}

impl std::fmt::Debug for StallDetectorService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StallDetectorService")
            .field("config", &self.config)
            .field("teams_dir", &self.teams_dir)
            .field(
                "member_states_len",
                &self
                    .member_states
                    .lock()
                    .map(|states| states.len())
                    .unwrap_or_default(),
            )
            .field(
                "member_signal_contexts_len",
                &self
                    .member_signal_contexts
                    .lock()
                    .map(|contexts| contexts.len())
                    .unwrap_or_default(),
            )
            .field(
                "trigger_history_len",
                &self
                    .trigger_history
                    .lock()
                    .map(|history| history.len())
                    .unwrap_or_default(),
            )
            .field("polling_ticks", &self.polling_ticks.load(Ordering::Relaxed))
            .finish()
    }
}

impl StallDetectorService {
    pub fn new(config: StallDetectorConfig) -> Self {
        Self::new_with_dependencies(
            config,
            Arc::new(SystemCoordinationRuntime),
            Arc::new(default_session_scan),
            Arc::new(default_mesh_signal_reader),
        )
    }

    fn new_with_dependencies(
        config: StallDetectorConfig,
        runtime: Arc<dyn CoordinationRuntime>,
        session_scanner: Arc<SessionScannerFn>,
        mesh_signal_reader: Arc<MeshSignalReaderFn>,
    ) -> Self {
        Self::new_with_dependencies_and_teams_dir(
            config,
            runtime,
            session_scanner,
            mesh_signal_reader,
            default_coordination_teams_dir(),
        )
    }

    fn new_with_dependencies_and_teams_dir(
        config: StallDetectorConfig,
        runtime: Arc<dyn CoordinationRuntime>,
        session_scanner: Arc<SessionScannerFn>,
        mesh_signal_reader: Arc<MeshSignalReaderFn>,
        teams_dir: PathBuf,
    ) -> Self {
        Self {
            config,
            teams_dir,
            member_states: Arc::new(Mutex::new(HashMap::new())),
            member_signal_contexts: Arc::new(Mutex::new(HashMap::new())),
            trigger_history: Arc::new(Mutex::new(Vec::new())),
            trigger_seq: Arc::new(AtomicU64::new(0)),
            polling_ticks: Arc::new(AtomicU64::new(0)),
            runtime,
            session_scanner,
            mesh_signal_reader,
            poller: None,
        }
    }

    pub fn config(&self) -> &StallDetectorConfig {
        &self.config
    }

    pub fn trigger_history(&self) -> Vec<StallTriggerRecord> {
        self.trigger_history
            .lock()
            .map(|history| history.clone())
            .unwrap_or_default()
    }

    pub fn trigger_history_json(&self) -> Value {
        let history = self.trigger_history();
        serde_json::to_value(history).unwrap_or(Value::Array(Vec::new()))
    }

    pub fn weekly_metrics(&self, now: DateTime<Utc>) -> StallWeeklyMetrics {
        self.finalize_recovery_windows(now);
        let one_week_ago = now - chrono::Duration::days(7);
        let history = self.trigger_history();
        let recent: Vec<&StallTriggerRecord> = history
            .iter()
            .filter(|record| record.triggered_at >= one_week_ago)
            .collect();

        let stage_a_records: Vec<&StallTriggerRecord> = recent
            .iter()
            .copied()
            .filter(|record| record.stage == StallTriggerStage::StageA)
            .collect();
        let stage_b_records: Vec<&StallTriggerRecord> = recent
            .iter()
            .copied()
            .filter(|record| record.stage == StallTriggerStage::StageB)
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

    pub fn annotate_trigger(
        &self,
        trigger_id: &str,
        confirmed_true_stall: bool,
        annotated_at: DateTime<Utc>,
    ) -> Result<(), CoordinationError> {
        let mut history = self.trigger_history.lock().map_err(|err| {
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

    pub fn reload_config_from_team_config_json(
        &mut self,
        raw: &str,
    ) -> Result<(), CoordinationError> {
        let config = StallDetectorConfig::from_team_config_json(raw)?;
        self.apply_config(config)
    }

    pub fn apply_config(&mut self, config: StallDetectorConfig) -> Result<(), CoordinationError> {
        config.validate()?;
        let was_polling = self.poller.is_some();
        if was_polling {
            self.stop_polling()?;
        }
        self.config = config;
        if was_polling && self.config.enabled {
            self.start_polling()?;
        }
        Ok(())
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

    pub fn upsert_member_signal_context(
        &self,
        team_name: &str,
        member_name: &str,
        context: MemberSignalContext,
    ) {
        self.upsert_member(team_name, member_name, Utc::now());
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut contexts) = self.member_signal_contexts.lock() {
            contexts.insert(key, context);
        }
    }

    pub fn collect_signals(&self) -> Vec<SignalSnapshot> {
        self.collect_signals_at(Utc::now())
    }

    pub fn collect_signals_at(&self, now: DateTime<Utc>) -> Vec<SignalSnapshot> {
        let member_keys = self
            .member_states
            .lock()
            .map(|states| states.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        collect_signals_for_members(
            &member_keys,
            &self.member_signal_contexts,
            self.runtime.as_ref(),
            self.session_scanner.as_ref(),
            self.mesh_signal_reader.as_ref(),
            self.config.require_medium_confidence_for_activity,
            now,
        )
    }

    fn ingest_signal_snapshot(&self, snapshot: &SignalSnapshot) {
        let team_name = snapshot.team_name.as_str();
        let member_name = snapshot.member_name.as_str();

        if let Some(signal) =
            snapshot.selected_session_signal(self.config.require_medium_confidence_for_activity)
        {
            self.ingest_session_signal(
                team_name,
                member_name,
                signal.observed_at,
                signal.is_strong,
            );
        }

        if snapshot.pane_command_is_medium() {
            self.ingest_pane_check(
                team_name,
                member_name,
                snapshot.observed_at,
                true,
                snapshot.pane_is_shell.unwrap_or(false),
            );
        }

        if let Some(at) = snapshot.mesh_last_activity_at {
            self.ingest_mesh_heartbeat(team_name, member_name, at);
        }
        if let Some(status) = snapshot.mesh_status {
            self.ingest_mesh_status(team_name, member_name, snapshot.observed_at, status);
        }

        if snapshot.strongest_signal == Some(SignalStrength::Medium) {
            if let Some(event_at) = snapshot.coordination_event_at {
                self.ingest_pane_check(team_name, member_name, event_at, true, false);
            }
        }
    }

    fn ingest_signal_snapshots(&self, snapshots: &[SignalSnapshot]) {
        for snapshot in snapshots {
            self.ingest_signal_snapshot(snapshot);
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
        self.mark_recovery_if_resumed(team_name, member_name, observed_at);
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
            self.mark_recovery_if_resumed(team_name, member_name, observed_at);
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
        self.mark_recovery_if_resumed(team_name, member_name, observed_at);
    }

    pub fn ingest_mesh_status(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
        status: MeshMemberStatus,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        let mut should_check_recovery = false;
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
                        should_check_recovery = true;
                    }
                    MeshMemberStatus::Blocked
                    | MeshMemberStatus::Idle
                    | MeshMemberStatus::Unknown => {}
                }
            }
        }
        if should_check_recovery {
            self.mark_recovery_if_resumed(team_name, member_name, observed_at);
        }
    }

    pub fn poll_once(&self) -> Vec<StageTransition> {
        self.poll_once_at(Utc::now())
    }

    pub fn poll_once_at(&self, now: DateTime<Utc>) -> Vec<StageTransition> {
        if !self.config.enabled {
            return Vec::new();
        }
        let snapshots = self.collect_signals_at(now);
        let snapshots_by_member = build_signal_snapshot_index(&snapshots);
        self.ingest_signal_snapshots(&snapshots);
        if let Ok(states) = self.member_states.lock() {
            write_activity_snapshots_for_members(
                &self.teams_dir,
                &self.config,
                &states,
                &snapshots_by_member,
                now,
            );
        }
        self.finalize_recovery_windows(now);
        let Ok(mut states) = self.member_states.lock() else {
            return Vec::new();
        };
        let decisions = evaluate_transitions(&self.config, &mut states, &snapshots_by_member, now);
        let transitions: Vec<StageTransition> = decisions
            .iter()
            .map(|decision| decision.transition.clone())
            .collect();
        self.record_trigger_decisions(&decisions);
        transitions
    }

    pub fn poll_once_with_orchestrator(
        &self,
        orchestrator: &mut CoordinationOrchestrator,
    ) -> Vec<StageTransition> {
        self.poll_once_with_orchestrator_at(Utc::now(), orchestrator)
    }

    pub fn poll_once_with_orchestrator_at(
        &self,
        now: DateTime<Utc>,
        orchestrator: &mut CoordinationOrchestrator,
    ) -> Vec<StageTransition> {
        if !self.config.enabled {
            return Vec::new();
        }
        let snapshots = self.collect_signals_at(now);
        let snapshots_by_member = build_signal_snapshot_index(&snapshots);
        self.ingest_signal_snapshots(&snapshots);
        if let Ok(states) = self.member_states.lock() {
            write_activity_snapshots_for_members(
                &self.teams_dir,
                &self.config,
                &states,
                &snapshots_by_member,
                now,
            );
        }
        self.finalize_recovery_windows(now);
        let Ok(mut states) = self.member_states.lock() else {
            return Vec::new();
        };
        let decisions = evaluate_transitions(&self.config, &mut states, &snapshots_by_member, now);
        let transitions: Vec<StageTransition> = decisions
            .iter()
            .map(|decision| decision.transition.clone())
            .collect();
        self.record_trigger_decisions(&decisions);
        self.dispatch_escalations(orchestrator, &decisions);
        transitions
    }

    pub fn start_polling(&mut self) -> Result<(), CoordinationError> {
        if !self.config.enabled {
            return Ok(());
        }
        if self.poller.is_some() {
            return Err(CoordinationError::Conflict(
                "stall detector polling is already active".to_string(),
            ));
        }

        let interval = self.config.poll_interval();
        let config = self.config.clone();
        let member_states = Arc::clone(&self.member_states);
        let member_signal_contexts = Arc::clone(&self.member_signal_contexts);
        let trigger_history = Arc::clone(&self.trigger_history);
        let trigger_seq = Arc::clone(&self.trigger_seq);
        let polling_ticks = Arc::clone(&self.polling_ticks);
        let runtime = Arc::clone(&self.runtime);
        let session_scanner = Arc::clone(&self.session_scanner);
        let mesh_signal_reader = Arc::clone(&self.mesh_signal_reader);
        let teams_dir = self.teams_dir.clone();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let join_handle = thread::spawn(move || loop {
            match stop_rx.recv_timeout(interval) {
                Ok(_) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    let member_keys = member_states
                        .lock()
                        .map(|states| states.keys().cloned().collect::<Vec<_>>())
                        .unwrap_or_default();
                    if member_keys.is_empty() {
                        continue;
                    }

                    polling_ticks.fetch_add(1, Ordering::Relaxed);
                    let now = Utc::now();
                    let snapshots = collect_signals_for_members(
                        &member_keys,
                        &member_signal_contexts,
                        runtime.as_ref(),
                        session_scanner.as_ref(),
                        mesh_signal_reader.as_ref(),
                        config.require_medium_confidence_for_activity,
                        now,
                    );
                    if !snapshots.is_empty() {
                        apply_signal_snapshots_to_member_states(
                            &member_states,
                            &snapshots,
                            config.require_medium_confidence_for_activity,
                        );
                    }
                    let snapshots_by_member = build_signal_snapshot_index(&snapshots);
                    if let Ok(states) = member_states.lock() {
                        write_activity_snapshots_for_members(
                            &teams_dir,
                            &config,
                            &states,
                            &snapshots_by_member,
                            now,
                        );
                    }
                    finalize_recovery_windows_for_history(&config, &trigger_history, now);
                    if let Ok(mut states) = member_states.lock() {
                        let decisions =
                            evaluate_transitions(&config, &mut states, &snapshots_by_member, now);
                        if !decisions.is_empty() {
                            record_trigger_decisions_for_history(
                                &config,
                                &trigger_history,
                                &trigger_seq,
                                &decisions,
                            );
                        }
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

    fn next_trigger_id(&self) -> String {
        let seq = self.trigger_seq.fetch_add(1, Ordering::Relaxed) + 1;
        format!("stall-trigger-{seq}")
    }

    fn mark_recovery_if_resumed(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
    ) {
        let key = MemberKey {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        };
        if let Ok(mut states) = self.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                if state.stage != StallStage::Healthy {
                    state.stage = StallStage::Healthy;
                    state.pending_nudge_id = None;
                    state.uncertainty_defer_active = false;
                    state.suppression_until = Some(
                        observed_at
                            + chrono::Duration::seconds(
                                self.config.post_nudge_cooldown_secs as i64,
                            ),
                    );
                }
            }
        }

        let Ok(mut history) = self.trigger_history.lock() else {
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
            if elapsed <= self.config.recovery_window_secs as i64 {
                record.resumed_within_recovery_window_without_intervention = Some(true);
                record.resumed_at = Some(observed_at);
            }
        }
    }

    fn finalize_recovery_windows(&self, now: DateTime<Utc>) {
        finalize_recovery_windows_for_history(&self.config, &self.trigger_history, now);
    }

    fn record_trigger_decisions(&self, decisions: &[TransitionDecision]) {
        if decisions.is_empty() {
            return;
        }
        let Ok(mut history) = self.trigger_history.lock() else {
            return;
        };
        for decision in decisions {
            let record = StallTriggerRecord {
                trigger_id: self.next_trigger_id(),
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
            if self.config.persist_trigger_history {
                history.push(record);
            }
        }
    }

    fn dispatch_escalations(
        &self,
        orchestrator: &mut CoordinationOrchestrator,
        decisions: &[TransitionDecision],
    ) {
        for decision in decisions {
            let result = match decision.trigger_stage {
                StallTriggerStage::StageA => {
                    let response_window_secs = self
                        .config
                        .hard_escalate_after_secs
                        .saturating_sub(self.config.soft_nudge_after_secs);
                    let response_minutes = std::cmp::max(1, response_window_secs.div_ceil(60));
                    let message = format!(
                        "Are you still working on Task #N? Reply with status (working, blocked, done) within {response_minutes} min."
                    );
                    orchestrator.deliver_message(DeliveryRequest::OperatorNotice(
                        OperatorNoticeDelivery {
                            member_name: decision.transition.member_name.clone(),
                            team_name: decision.transition.team_name.clone(),
                            message,
                            sender_name: Some("stall-detector".to_string()),
                        },
                    ))
                }
                StallTriggerStage::StageB => {
                    let lead_name =
                        resolve_team_lead_name(orchestrator, &decision.transition.team_name);
                    match lead_name {
                        Ok(lead_name) => {
                            let message =
                                render_stage_b_evidence_message(decision, &decision.transition);
                            orchestrator.deliver_message(DeliveryRequest::OperatorNotice(
                                OperatorNoticeDelivery {
                                    member_name: lead_name,
                                    team_name: decision.transition.team_name.clone(),
                                    message,
                                    sender_name: Some("stall-detector".to_string()),
                                },
                            ))
                        }
                        Err(err) => Err(err),
                    }
                }
            };

            if let Err(err) = result {
                tracing::warn!(
                    team_name = %decision.transition.team_name,
                    member_name = %decision.transition.member_name,
                    stage = %decision.trigger_stage.as_str(),
                    error = %err,
                    "stall detector escalation delivery failed"
                );
            }
        }
    }
}

fn collect_signals_for_members(
    member_keys: &[MemberKey],
    member_signal_contexts: &Arc<Mutex<HashMap<MemberKey, MemberSignalContext>>>,
    runtime: &dyn CoordinationRuntime,
    session_scanner: &SessionScannerFn,
    mesh_signal_reader: &MeshSignalReaderFn,
    require_medium_confidence: bool,
    now: DateTime<Utc>,
) -> Vec<SignalSnapshot> {
    if member_keys.is_empty() {
        return Vec::new();
    }

    let probe_tmux_signals = host_supports_tmux_signals();
    let probe_mesh_signals = host_supports_mesh_signals();
    let contexts = member_signal_contexts
        .lock()
        .map(|contexts| contexts.clone())
        .unwrap_or_default();
    let any_session_context = contexts
        .values()
        .any(|context| context.pane_id.is_some() || context.project_path.is_some());
    let sessions = if probe_tmux_signals && any_session_context {
        session_scanner(now)
    } else {
        Vec::new()
    };
    let sessions_by_pane: HashMap<String, SessionSignal> = sessions
        .iter()
        .filter_map(|signal| signal.pane_id.clone().map(|pane| (pane, signal.clone())))
        .collect();
    let sessions_by_project = latest_session_per_project(&sessions);

    let mut mesh_by_team: HashMap<String, HashMap<String, MeshMemberSignal>> = HashMap::new();
    let mut snapshots = Vec::with_capacity(member_keys.len());

    for key in member_keys {
        let context = contexts.get(key).cloned().unwrap_or_default();
        let matched_session =
            matched_session_signal(&context, &sessions_by_pane, &sessions_by_project);
        let mesh_signal = if probe_mesh_signals && !key.team_name.trim().is_empty() {
            let mesh_signals = mesh_by_team
                .entry(key.team_name.clone())
                .or_insert_with(|| mesh_signal_reader(&key.team_name));
            mesh_signals
                .get(&key.member_name)
                .cloned()
                .unwrap_or_default()
        } else {
            MeshMemberSignal::default()
        };

        let (pane_exists, pane_is_dead, pane_is_shell, pane_current_command) = if probe_tmux_signals
        {
            collect_pane_snapshot(runtime, context.pane_id.as_deref())
        } else {
            (None, None, None, None)
        };

        let mut snapshot = SignalSnapshot {
            team_name: key.team_name.clone(),
            member_name: key.member_name.clone(),
            observed_at: matched_session
                .as_ref()
                .map(|signal| signal.observed_at)
                .unwrap_or(now),
            session_state: matched_session.as_ref().map(|signal| signal.state),
            session_confidence: matched_session.as_ref().map(|signal| signal.confidence),
            pane_exists,
            pane_is_dead,
            pane_is_shell,
            pane_current_command,
            mesh_last_activity_at: mesh_signal.last_activity_at,
            mesh_status: mesh_signal.status,
            coordination_event_at: context.coordination_event_at,
            project_file_write_at: context.project_file_write_at,
            runtime_last_seen_at: context.last_seen_at,
            strongest_signal: None,
        };
        snapshot.strongest_signal = snapshot.classify(require_medium_confidence);
        snapshots.push(snapshot);
    }

    snapshots
}

fn collect_pane_snapshot(
    runtime: &dyn CoordinationRuntime,
    pane_id: Option<&str>,
) -> (Option<bool>, Option<bool>, Option<bool>, Option<String>) {
    if !host_supports_tmux_signals() {
        return (None, None, None, None);
    }

    let Some(pane_id) = pane_id else {
        return (None, None, None, None);
    };

    let pane_exists = runtime.pane_exists(pane_id).ok();
    if pane_exists != Some(true) {
        return (pane_exists, None, None, None);
    }

    let pane_is_dead = runtime.pane_is_dead(pane_id).ok();
    if pane_is_dead == Some(true) {
        return (pane_exists, pane_is_dead, None, None);
    }

    let pane_is_shell = runtime.pane_is_shell(pane_id).ok();
    let pane_current_command = runtime.pane_current_command(pane_id).ok().flatten();
    (
        pane_exists,
        pane_is_dead,
        pane_is_shell,
        pane_current_command,
    )
}

fn latest_session_per_project(sessions: &[SessionSignal]) -> HashMap<String, SessionSignal> {
    let mut by_project = HashMap::new();
    for session in sessions {
        by_project
            .entry(session.project_path.clone())
            .and_modify(|current: &mut SessionSignal| {
                if session.observed_at >= current.observed_at
                    || session.confidence_rank() > current.confidence_rank()
                {
                    *current = session.clone();
                }
            })
            .or_insert_with(|| session.clone());
    }
    by_project
}

fn matched_session_signal(
    context: &MemberSignalContext,
    sessions_by_pane: &HashMap<String, SessionSignal>,
    sessions_by_project: &HashMap<String, SessionSignal>,
) -> Option<SessionSignal> {
    if let Some(pane_id) = context.pane_id.as_deref() {
        if let Some(signal) = sessions_by_pane.get(pane_id) {
            return Some(signal.clone());
        }
    }
    context
        .project_path
        .as_ref()
        .and_then(|path| sessions_by_project.get(path))
        .cloned()
}

fn build_signal_snapshot_index(snapshots: &[SignalSnapshot]) -> HashMap<MemberKey, SignalSnapshot> {
    let mut index = HashMap::with_capacity(snapshots.len());
    for snapshot in snapshots {
        index.insert(
            MemberKey {
                team_name: snapshot.team_name.clone(),
                member_name: snapshot.member_name.clone(),
            },
            snapshot.clone(),
        );
    }
    index
}

fn resolve_team_lead_name(
    orchestrator: &CoordinationOrchestrator,
    team_name: &str,
) -> Result<String, CoordinationError> {
    let config = TeamConfigStore::load(&orchestrator.teams_dir, team_name)?;
    config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .map(|member| member.name.clone())
        .ok_or_else(|| {
            CoordinationError::NotFound(format!("lead member not found in team '{team_name}'"))
        })
}

fn default_coordination_teams_dir() -> PathBuf {
    if let Some(path) = std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV) {
        if !path.is_empty() {
            return PathBuf::from(path).join("teams");
        }
    }
    if let Some(path) = mesh_cli::resolve_windows_mesh_teams_dir() {
        return path;
    }
    let base = if let Some(home_dir) = dirs::home_dir() {
        home_dir
    } else {
        let fallback = std::env::temp_dir().join("taurhaus-home");
        tracing::warn!(
            fallback = %fallback.display(),
            "home directory unavailable; falling back to temp directory for stall snapshot path"
        );
        fallback
    };
    base.join(".claude").join("teams")
}

fn write_activity_snapshots_for_members(
    teams_dir: &Path,
    config: &StallDetectorConfig,
    member_states: &HashMap<MemberKey, MemberStallState>,
    snapshots_by_member: &HashMap<MemberKey, SignalSnapshot>,
    now: DateTime<Utc>,
) {
    if member_states.is_empty() {
        return;
    }

    let mut tracked_by_team: HashMap<String, HashSet<String>> = HashMap::new();
    for key in member_states.keys() {
        if key.team_name.trim().is_empty() || key.member_name.trim().is_empty() {
            continue;
        }
        tracked_by_team
            .entry(key.team_name.clone())
            .or_default()
            .insert(key.member_name.clone());
    }
    if tracked_by_team.is_empty() {
        return;
    }

    for (team_name, tracked_members) in tracked_by_team {
        let expected_members = TeamConfigStore::load(teams_dir, &team_name)
            .map(|config| {
                config
                    .members
                    .into_iter()
                    .map(|member| member.name)
                    .filter(|name| !name.trim().is_empty())
                    .collect::<HashSet<String>>()
            })
            .ok()
            .filter(|members| !members.is_empty())
            .unwrap_or(tracked_members);

        for member_name in &expected_members {
            let key = MemberKey {
                team_name: team_name.clone(),
                member_name: member_name.clone(),
            };
            let state = member_states.get(&key);
            let runtime = snapshots_by_member.get(&key);
            let snapshot = build_member_activity_snapshot(config, state, runtime, now);
            write_member_activity_snapshot(teams_dir, &team_name, member_name, &snapshot);
        }

        cleanup_stale_activity_snapshots(teams_dir, &team_name, &expected_members);
    }
}

fn build_member_activity_snapshot(
    config: &StallDetectorConfig,
    state: Option<&MemberStallState>,
    runtime: Option<&SignalSnapshot>,
    now: DateTime<Utc>,
) -> MemberActivitySnapshot {
    let recent_window_secs = config.poll_interval_secs.saturating_mul(2) as i64;
    let stall_recent_activity = state
        .and_then(|state| state.last_any_signal_at)
        .is_some_and(|last_any_signal_at| {
            let elapsed = now.signed_duration_since(last_any_signal_at).num_seconds();
            elapsed >= 0 && elapsed <= recent_window_secs
        });

    let stall_no_output = runtime
        .is_some_and(|snapshot| !matches!(snapshot.session_state, Some(SessionState::Active)));

    let stall_no_active_process = runtime.is_some_and(|snapshot| {
        if snapshot.pane_exists == Some(false) || snapshot.pane_is_dead == Some(true) {
            return true;
        }
        if snapshot.pane_exists != Some(true) {
            return false;
        }
        if snapshot.pane_is_shell == Some(true) {
            return true;
        }
        snapshot
            .pane_current_command
            .as_ref()
            .map(|cmd| cmd.trim().is_empty())
            .unwrap_or(true)
    });

    MemberActivitySnapshot {
        version: 1,
        observed_at: runtime
            .map(|snapshot| snapshot.observed_at)
            .unwrap_or(now)
            .to_rfc3339(),
        stall_recent_activity,
        stall_no_output,
        stall_no_active_process,
    }
}

fn write_member_activity_snapshot(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    snapshot: &MemberActivitySnapshot,
) {
    let dir = activity_snapshot_dir(teams_dir, team_name);
    if let Err(err) = fs::create_dir_all(&dir) {
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            error = %err,
            "failed to create activity snapshot directory"
        );
        return;
    }

    let target_path = activity_snapshot_path(teams_dir, team_name, member_name);
    let tmp_path = activity_snapshot_tmp_path(teams_dir, team_name, member_name);
    let Ok(raw) = serde_json::to_vec_pretty(snapshot) else {
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            "failed to serialize activity snapshot"
        );
        return;
    };

    if let Err(err) = fs::write(&tmp_path, raw) {
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            error = %err,
            "failed to write temporary activity snapshot file"
        );
        return;
    }

    if let Err(rename_err) = fs::rename(&tmp_path, &target_path) {
        #[cfg(target_os = "windows")]
        {
            if target_path.exists() && fs::remove_file(&target_path).is_ok() {
                if fs::rename(&tmp_path, &target_path).is_ok() {
                    return;
                }
            }
        }
        let _ = fs::remove_file(&tmp_path);
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            error = %rename_err,
            "failed to atomically replace activity snapshot file"
        );
    }
}

fn cleanup_stale_activity_snapshots(
    teams_dir: &Path,
    team_name: &str,
    expected_members: &HashSet<String>,
) {
    let dir = activity_snapshot_dir(teams_dir, team_name);
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("tmp") {
            let _ = fs::remove_file(path);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !expected_members.contains(stem) {
            let _ = fs::remove_file(path);
        }
    }
}

fn activity_snapshot_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name).join("state").join("activity")
}

fn activity_snapshot_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    activity_snapshot_dir(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn activity_snapshot_tmp_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
    activity_snapshot_dir(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

fn format_optional_ts(value: Option<DateTime<Utc>>) -> String {
    value
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

fn render_stage_b_evidence_message(
    decision: &TransitionDecision,
    transition: &StageTransition,
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

fn apply_signal_snapshots_to_member_states(
    member_states: &Arc<Mutex<HashMap<MemberKey, MemberStallState>>>,
    snapshots: &[SignalSnapshot],
    require_medium_confidence: bool,
) {
    let Ok(mut states) = member_states.lock() else {
        return;
    };
    for snapshot in snapshots {
        let key = MemberKey {
            team_name: snapshot.team_name.clone(),
            member_name: snapshot.member_name.clone(),
        };
        let Some(state) = states.get_mut(&key) else {
            continue;
        };

        if let Some(signal) = snapshot.selected_session_signal(require_medium_confidence) {
            set_if_newer(&mut state.last_any_signal_at, signal.observed_at);
            if signal.is_strong {
                set_if_newer(&mut state.last_strong_signal_at, signal.observed_at);
            }
        }

        if snapshot.pane_command_is_medium() {
            set_if_newer(&mut state.last_any_signal_at, snapshot.observed_at);
        }
        if snapshot.strongest_signal == Some(SignalStrength::Medium) {
            if let Some(event_at) = snapshot.coordination_event_at {
                set_if_newer(&mut state.last_any_signal_at, event_at);
            }
        }
        if let Some(at) = snapshot.mesh_last_activity_at {
            set_if_newer(&mut state.last_any_signal_at, at);
        }
        if let Some(status) = snapshot.mesh_status {
            match status {
                MeshMemberStatus::Working => {
                    set_if_newer(&mut state.last_any_signal_at, snapshot.observed_at);
                    set_if_newer(&mut state.last_strong_signal_at, snapshot.observed_at);
                }
                MeshMemberStatus::Investigating => {
                    set_if_newer(&mut state.last_any_signal_at, snapshot.observed_at);
                }
                MeshMemberStatus::Blocked | MeshMemberStatus::Idle | MeshMemberStatus::Unknown => {}
            }
        }
    }
}

fn default_session_scan(now: DateTime<Utc>) -> Vec<SessionSignal> {
    if !host_supports_tmux_signals() {
        return Vec::new();
    }

    scan_sessions()
        .into_iter()
        .map(|session| SessionSignal {
            pane_id: session.tmux_pane,
            project_path: session.project_path,
            observed_at: now,
            state: session.state,
            confidence: session.activity_confidence,
        })
        .collect()
}

fn default_mesh_signal_reader(team_name: &str) -> HashMap<String, MeshMemberSignal> {
    if !host_supports_mesh_signals() || team_name.trim().is_empty() {
        return HashMap::new();
    }

    let Some(raw) = fetch_mesh_who_json(team_name) else {
        return HashMap::new();
    };
    parse_mesh_who_json(&raw)
}

const MESH_WHO_TIMEOUT: Duration = Duration::from_secs(2);
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

fn run_command_with_timeout(command: &mut Command, timeout: Duration) -> Option<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    tracing::warn!(
                        timeout_ms = timeout.as_millis() as u64,
                        "stall detector command timed out; terminating process"
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                thread::sleep(COMMAND_POLL_INTERVAL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

fn fetch_mesh_who_json(team_name: &str) -> Option<String> {
    if !host_supports_mesh_signals() || team_name.trim().is_empty() {
        return None;
    }

    let invocation = mesh_cli::mesh_command_invocation(&["who", "--json", "--team", team_name]);
    let output = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args);
        run_command_with_timeout(&mut cmd, MESH_WHO_TIMEOUT)?
    } else {
        let mut cmd = Command::new(&invocation.program);
        cmd.args(&invocation.args);
        run_command_with_timeout(&mut cmd, MESH_WHO_TIMEOUT)?
    };
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_mesh_who_json(raw: &str) -> HashMap<String, MeshMemberSignal> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return HashMap::new();
    };
    let Value::Array(rows) = value else {
        return HashMap::new();
    };
    let mut by_member = HashMap::new();
    for row in rows {
        let Value::Object(map) = row else {
            continue;
        };
        let Some(name) = map.get("name").and_then(Value::as_str) else {
            continue;
        };
        let last_activity_at = map
            .get("lastActivityAt")
            .or_else(|| map.get("last_activity_at"))
            .and_then(parse_mesh_timestamp);
        let status = map
            .get("status")
            .and_then(Value::as_str)
            .and_then(parse_mesh_status);
        by_member.insert(
            name.to_string(),
            MeshMemberSignal {
                last_activity_at,
                status,
            },
        );
    }
    by_member
}

fn parse_mesh_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(raw) = value.as_str() {
        if let Ok(ts) = DateTime::parse_from_rfc3339(raw) {
            return Some(ts.with_timezone(&Utc));
        }
        return None;
    }
    if let Some(epoch) = value.as_i64() {
        if epoch > 10_000_000_000 {
            return DateTime::<Utc>::from_timestamp_millis(epoch);
        }
        return DateTime::<Utc>::from_timestamp(epoch, 0);
    }
    None
}

fn parse_mesh_status(raw: &str) -> Option<MeshMemberStatus> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "working" => Some(MeshMemberStatus::Working),
        "blocked" => Some(MeshMemberStatus::Blocked),
        "investigating" => Some(MeshMemberStatus::Investigating),
        "idle" => Some(MeshMemberStatus::Idle),
        "unknown" => Some(MeshMemberStatus::Unknown),
        _ => None,
    }
}

fn host_supports_tmux_signals() -> bool {
    !cfg!(target_os = "windows")
}

fn host_supports_mesh_signals() -> bool {
    !cfg!(target_os = "windows")
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

fn is_long_running_command(command: &str) -> bool {
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

fn evaluate_transitions(
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
                Some(MeshMemberStatus::Blocked) => {
                    continue;
                }
                Some(MeshMemberStatus::Investigating) => {
                    continue;
                }
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

fn record_trigger_decisions_for_history(
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

fn finalize_recovery_windows_for_history(
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

impl Drop for StallDetectorService {
    fn drop(&mut self) {
        let _ = self.stop_polling();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::TempDir;

    use crate::coordination::backend::fake::FakeBackend;
    use crate::coordination::domain::{Member, MemberRole};
    use crate::coordination::runtime::RecordingCoordinationRuntime;
    use crate::session_scanner::cli_tool::CliTool;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_member(name: &str, role: MemberRole) -> Member {
        Member {
            name: name.to_string(),
            role,
            role_id: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from("/tmp/project"),
            cli_tool: CliTool::Codex,
        }
    }

    fn activity_snapshot_path(teams_dir: &Path, team_name: &str, member_name: &str) -> PathBuf {
        teams_dir
            .join(team_name)
            .join("state")
            .join("activity")
            .join(format!("{member_name}.json"))
    }

    fn test_orchestrator_with_team(
        team_name: &str,
    ) -> (
        CoordinationOrchestrator,
        Arc<FakeBackend>,
        TempDir,
        String,
        String,
    ) {
        let teams_tmp = TempDir::new().expect("temp teams dir");
        let backend = Arc::new(FakeBackend::default());
        let mut orchestrator =
            CoordinationOrchestrator::new(teams_tmp.path().to_path_buf(), backend.clone());
        orchestrator
            .create_team(team_name, None)
            .expect("create team");

        let lead_name = "team-lead".to_string();
        let member_name = "agent-a".to_string();
        orchestrator
            .add_member(team_name, sample_member(&lead_name, MemberRole::Lead))
            .expect("add lead");
        orchestrator
            .add_member(team_name, sample_member(&member_name, MemberRole::Agent))
            .expect("add member");

        (orchestrator, backend, teams_tmp, lead_name, member_name)
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
    fn collect_signals_classifies_active_medium_confidence_session_as_strong() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|now| {
                vec![SessionSignal {
                    pane_id: Some("%11".to_string()),
                    project_path: "/repo".to_string(),
                    observed_at: now,
                    state: SessionState::Active,
                    confidence: ActivityConfidence::Medium,
                }]
            }),
            Arc::new(|_| HashMap::new()),
        );

        let now = ts("2026-03-05T12:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%11".to_string()),
                project_path: Some("/repo".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let snapshots = service.collect_signals_at(now);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strongest_signal, Some(SignalStrength::Strong));
    }

    #[test]
    fn collect_signals_classifies_non_shell_command_as_medium() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%22", true);
        runtime.set_pane_dead("%22", false);
        runtime.set_pane_shell("%22", false);
        runtime.set_pane_current_command("%22", Some("cargo test"));

        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
        );

        let now = ts("2026-03-05T12:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%22".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let snapshots = service.collect_signals_at(now);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strongest_signal, Some(SignalStrength::Medium));
    }

    #[test]
    fn collect_signals_classifies_last_seen_as_weak() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
        );

        let now = ts("2026-03-05T12:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                last_seen_at: Some(now),
                ..MemberSignalContext::default()
            },
        );

        let snapshots = service.collect_signals_at(now);
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].strongest_signal, Some(SignalStrength::Weak));
    }

    #[test]
    fn poll_once_applies_collected_strong_signal_before_threshold_check() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|now| {
                vec![SessionSignal {
                    pane_id: Some("%33".to_string()),
                    project_path: "/repo".to_string(),
                    observed_at: now,
                    state: SessionState::Active,
                    confidence: ActivityConfidence::High,
                }]
            }),
            Arc::new(|_| HashMap::new()),
        );

        let now = ts("2026-03-05T12:10:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(600),
        );
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%33".to_string()),
                project_path: Some("/repo".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());
        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.last_strong_signal_at, Some(now));
        assert_eq!(state.stage, StallStage::Healthy);
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

        let first = service.poll_once_at(now);
        assert!(first.is_empty(), "first hard-window cycle should defer");

        let transitions = service.poll_once_at(now + ChronoDuration::seconds(30));
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].from, StallStage::SoftNudged);
        assert_eq!(transitions[0].to, StallStage::Escalated);

        let state = service
            .member_state("team-a", "agent-a")
            .expect("member state");
        assert_eq!(state.stage, StallStage::Escalated);
        assert_eq!(
            state.last_escalation_at,
            Some(now + ChronoDuration::seconds(30))
        );
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

    #[test]
    fn parse_mesh_who_json_parses_optional_activity_and_status_fields() {
        let raw = r#"[
          {
            "name": "agent-a",
            "lastActivityAt": 1772711785867,
            "status": "working"
          },
          {
            "name": "agent-b",
            "last_activity_at": "2026-03-05T12:00:00Z",
            "status": "investigating"
          }
        ]"#;

        let parsed = parse_mesh_who_json(raw);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed.get("agent-a").and_then(|entry| entry.status),
            Some(MeshMemberStatus::Working)
        );
        assert_eq!(
            parsed
                .get("agent-b")
                .and_then(|entry| entry.last_activity_at),
            Some(ts("2026-03-05T12:00:00Z"))
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_command_with_timeout_returns_output_before_deadline() {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "printf '{\"ok\":true}'"]);
        let output =
            run_command_with_timeout(&mut cmd, Duration::from_millis(500)).expect("command output");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "{\"ok\":true}");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_command_with_timeout_terminates_hanging_process() {
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "sleep 2"]);
        let started_at = Instant::now();
        let output = run_command_with_timeout(&mut cmd, Duration::from_millis(100));
        assert!(output.is_none());
        assert!(
            started_at.elapsed() < Duration::from_secs(2),
            "timeout helper should return before command naturally exits"
        );
    }

    #[test]
    fn reload_config_from_team_config_json_updates_runtime_config() {
        let mut service = StallDetectorService::new(StallDetectorConfig::default());
        let raw = r#"{
          "name": "architecture-final",
          "created_at": "2026-03-05T12:00:00Z",
          "members": [],
          "stall_detection": {
            "poll_interval_secs": 20,
            "soft_nudge_after_secs": 400,
            "hard_escalate_after_secs": 700
          }
        }"#;

        service
            .reload_config_from_team_config_json(raw)
            .expect("reload should succeed");

        assert_eq!(service.config().poll_interval_secs, 20);
        assert_eq!(service.config().soft_nudge_after_secs, 400);
        assert_eq!(service.config().hard_escalate_after_secs, 700);
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

    #[test]
    fn polling_loop_without_members_does_not_tick() {
        let mut service = StallDetectorService::new(StallDetectorConfig {
            poll_interval_secs: 1,
            ..StallDetectorConfig::default()
        });

        service.start_polling().expect("start polling");
        std::thread::sleep(Duration::from_millis(1200));
        service.stop_polling().expect("stop polling");

        assert_eq!(service.polling_tick_count(), 0);
    }

    #[test]
    fn collect_signals_skips_session_scan_without_context() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let scanner_calls = Arc::new(AtomicUsize::new(0));
        let scanner_calls_ref = scanner_calls.clone();
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(move |_| {
                scanner_calls_ref.fetch_add(1, Ordering::Relaxed);
                Vec::new()
            }),
            Arc::new(|_| HashMap::new()),
        );

        let now = ts("2026-03-05T13:40:00Z");
        service.upsert_member("team-a", "agent-a", now);
        let snapshots = service.collect_signals_at(now);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(scanner_calls.load(Ordering::Relaxed), 0);
        assert!(snapshots[0].session_state.is_none());
    }

    #[test]
    fn blank_team_name_skips_mesh_signal_reader() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let mesh_calls = Arc::new(AtomicUsize::new(0));
        let mesh_calls_ref = mesh_calls.clone();
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(move |_| {
                mesh_calls_ref.fetch_add(1, Ordering::Relaxed);
                HashMap::new()
            }),
        );

        let now = ts("2026-03-05T13:50:00Z");
        service.upsert_member("", "agent-a", now);
        let snapshots = service.collect_signals_at(now);

        assert_eq!(snapshots.len(), 1);
        assert_eq!(mesh_calls.load(Ordering::Relaxed), 0);
        assert!(snapshots[0].mesh_status.is_none());
        assert!(snapshots[0].mesh_last_activity_at.is_none());
    }

    #[test]
    fn stage_a_delivery_uses_operator_notice_path() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let (mut orchestrator, backend, _tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let now = ts("2026-03-05T13:00:00Z");
        service.upsert_member("team-a", &member_name, now);
        service.set_last_any_signal_for_tests(
            "team-a",
            &member_name,
            now - ChronoDuration::seconds(300),
        );

        let transitions = service.poll_once_with_orchestrator_at(now, &mut orchestrator);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].to, StallStage::SoftNudged);

        let delivered = backend.delivered_requests();
        assert_eq!(delivered.len(), 1);
        let DeliveryRequest::OperatorNotice(payload) = &delivered[0] else {
            panic!("expected operator notice");
        };
        assert_eq!(payload.team_name, "team-a");
        assert_eq!(payload.member_name, member_name);
        assert!(payload
            .message
            .contains("Are you still working on Task #N?"));
    }

    #[test]
    fn stage_b_delivery_alerts_team_lead_with_evidence() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let (mut orchestrator, backend, _tmp, lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let stage_a_now = ts("2026-03-05T13:10:00Z");
        service.upsert_member("team-a", &member_name, stage_a_now);
        service.set_last_any_signal_for_tests(
            "team-a",
            &member_name,
            stage_a_now - ChronoDuration::seconds(300),
        );
        let _ = service.poll_once_with_orchestrator_at(stage_a_now, &mut orchestrator);

        let stage_b_now = stage_a_now + ChronoDuration::seconds(240);
        let first = service.poll_once_with_orchestrator_at(stage_b_now, &mut orchestrator);
        assert!(
            first.is_empty(),
            "first stage-b check should defer for hysteresis"
        );
        let second = service.poll_once_with_orchestrator_at(
            stage_b_now + ChronoDuration::seconds(30),
            &mut orchestrator,
        );
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].to, StallStage::Escalated);

        let delivered = backend.delivered_requests();
        assert_eq!(delivered.len(), 2);
        let DeliveryRequest::OperatorNotice(payload) = &delivered[1] else {
            panic!("expected operator notice");
        };
        assert_eq!(payload.member_name, lead_name);
        assert!(payload.message.contains("Stage B stall escalation"));
        assert!(payload.message.contains("nudge_id="));
    }

    #[test]
    fn blocked_mesh_status_suppresses_stage_a() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_| {
                let mut map = HashMap::new();
                map.insert(
                    "agent-a".to_string(),
                    MeshMemberSignal {
                        last_activity_at: None,
                        status: Some(MeshMemberStatus::Blocked),
                    },
                );
                map
            }),
        );

        let now = ts("2026-03-05T14:00:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(400),
        );

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());
        assert_eq!(
            service
                .member_state("team-a", "agent-a")
                .expect("member state")
                .stage,
            StallStage::Healthy
        );
    }

    #[test]
    fn non_allowlisted_short_command_does_not_suppress_stage_a() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%44", true);
        runtime.set_pane_dead("%44", false);
        runtime.set_pane_shell("%44", false);
        runtime.set_pane_current_command("%44", Some("ls -la"));

        let service = StallDetectorService::new_with_dependencies(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
        );
        let now = ts("2026-03-05T14:10:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(301),
        );
        service.upsert_member_signal_context(
            "team-a",
            "agent-a",
            MemberSignalContext {
                pane_id: Some("%44".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let transitions = service.poll_once_at(now);
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].to, StallStage::SoftNudged);
    }

    #[test]
    fn pending_nudge_blocks_second_stage_a() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T14:20:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(500),
        );

        let key = MemberKey {
            team_name: "team-a".to_string(),
            member_name: "agent-a".to_string(),
        };
        if let Ok(mut states) = service.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                state.pending_nudge_id = Some("nudge-1".to_string());
            }
        }

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());
    }

    #[test]
    fn evidence_must_advance_before_re_nudge() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T14:30:00Z");
        let last_signal = now - ChronoDuration::seconds(600);
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests("team-a", "agent-a", last_signal);

        let key = MemberKey {
            team_name: "team-a".to_string(),
            member_name: "agent-a".to_string(),
        };
        if let Ok(mut states) = service.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                state.last_nudge_at = Some(last_signal + ChronoDuration::seconds(60));
            }
        }

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());
    }

    #[test]
    fn max_nudges_per_hour_enforced() {
        let service = StallDetectorService::new(StallDetectorConfig::default());
        let now = ts("2026-03-05T14:40:00Z");
        service.upsert_member("team-a", "agent-a", now);
        service.set_last_any_signal_for_tests(
            "team-a",
            "agent-a",
            now - ChronoDuration::seconds(600),
        );

        let key = MemberKey {
            team_name: "team-a".to_string(),
            member_name: "agent-a".to_string(),
        };
        if let Ok(mut states) = service.member_states.lock() {
            if let Some(state) = states.get_mut(&key) {
                state.nudge_count_window.window_started_at =
                    Some(now - ChronoDuration::minutes(10));
                state.nudge_count_window.count = 3;
            }
        }

        let transitions = service.poll_once_at(now);
        assert!(transitions.is_empty());
    }

    #[test]
    fn poll_once_writes_activity_snapshot_file_with_v1_schema() {
        let runtime = Arc::new(RecordingCoordinationRuntime::default());
        runtime.set_pane_exists("%55", true);
        runtime.set_pane_dead("%55", false);
        runtime.set_pane_shell("%55", false);
        runtime.set_pane_current_command("%55", Some("cargo test"));

        let (_orchestrator, _backend, teams_tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let service = StallDetectorService::new_with_dependencies_and_teams_dir(
            StallDetectorConfig::default(),
            runtime,
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
            teams_tmp.path().to_path_buf(),
        );
        let now = ts("2026-03-05T15:00:00Z");
        service.upsert_member("team-a", &member_name, now);
        service.upsert_member_signal_context(
            "team-a",
            &member_name,
            MemberSignalContext {
                pane_id: Some("%55".to_string()),
                ..MemberSignalContext::default()
            },
        );

        let _ = service.poll_once_at(now);

        let snapshot_path = activity_snapshot_path(teams_tmp.path(), "team-a", &member_name);
        let raw = fs::read_to_string(&snapshot_path).expect("snapshot should be written");
        let parsed: Value = serde_json::from_str(&raw).expect("valid snapshot json");
        assert_eq!(parsed.get("version").and_then(Value::as_u64), Some(1));
        assert!(parsed.get("observed_at").and_then(Value::as_str).is_some());
        assert!(parsed
            .get("stall_recent_activity")
            .and_then(Value::as_bool)
            .is_some());
        assert!(parsed
            .get("stall_no_output")
            .and_then(Value::as_bool)
            .is_some());
        assert!(parsed
            .get("stall_no_active_process")
            .and_then(Value::as_bool)
            .is_some());
    }

    #[test]
    fn poll_once_writes_activity_snapshot_atomically_with_tmp_rename() {
        let (_orchestrator, _backend, teams_tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let service = StallDetectorService::new_with_dependencies_and_teams_dir(
            StallDetectorConfig::default(),
            Arc::new(RecordingCoordinationRuntime::default()),
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
            teams_tmp.path().to_path_buf(),
        );
        let now = ts("2026-03-05T15:10:00Z");
        service.upsert_member("team-a", &member_name, now);

        let _ = service.poll_once_at(now);

        let snapshot_dir = teams_tmp
            .path()
            .join("team-a")
            .join("state")
            .join("activity");
        let snapshot_path = activity_snapshot_path(teams_tmp.path(), "team-a", &member_name);
        assert!(snapshot_path.exists(), "final snapshot file must exist");
        let tmp_path = snapshot_dir.join(format!("{member_name}.json.tmp"));
        assert!(!tmp_path.exists(), "tmp file must be renamed away");
    }

    #[test]
    fn poll_once_snapshot_observed_at_matches_poll_time() {
        let (_orchestrator, _backend, teams_tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let service = StallDetectorService::new_with_dependencies_and_teams_dir(
            StallDetectorConfig::default(),
            Arc::new(RecordingCoordinationRuntime::default()),
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
            teams_tmp.path().to_path_buf(),
        );
        let now = ts("2026-03-05T15:20:00Z");
        service.upsert_member("team-a", &member_name, now);

        let _ = service.poll_once_at(now);

        let snapshot_path = activity_snapshot_path(teams_tmp.path(), "team-a", &member_name);
        let parsed: Value = serde_json::from_str(
            &fs::read_to_string(snapshot_path).expect("snapshot should be readable"),
        )
        .expect("snapshot json should parse");
        assert_eq!(
            parsed.get("observed_at").and_then(Value::as_str),
            Some("2026-03-05T15:20:00+00:00")
        );
    }

    #[test]
    fn poll_once_cleans_up_stale_snapshot_when_member_removed() {
        let (mut orchestrator, _backend, teams_tmp, _lead_name, member_name) =
            test_orchestrator_with_team("team-a");
        let service = StallDetectorService::new_with_dependencies_and_teams_dir(
            StallDetectorConfig::default(),
            Arc::new(RecordingCoordinationRuntime::default()),
            Arc::new(|_| Vec::new()),
            Arc::new(|_| HashMap::new()),
            teams_tmp.path().to_path_buf(),
        );
        let now = ts("2026-03-05T15:30:00Z");
        service.upsert_member("team-a", &member_name, now);
        let _ = service.poll_once_at(now);

        let snapshot_path = activity_snapshot_path(teams_tmp.path(), "team-a", &member_name);
        assert!(snapshot_path.exists(), "snapshot must exist before removal");

        orchestrator
            .remove_member("team-a", &member_name, None)
            .expect("remove team member");

        let _ = service.poll_once_at(now + ChronoDuration::seconds(30));
        assert!(
            !snapshot_path.exists(),
            "removed member snapshot should be cleaned up"
        );
    }
}
