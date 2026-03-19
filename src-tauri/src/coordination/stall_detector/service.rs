use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::delivery;
use super::history;
use super::paths::default_coordination_teams_dir;
use super::signal_sources::{
    apply_signal_snapshots_to_member_states, default_mesh_signal_reader, default_session_scan,
};
use super::signals;
use super::transitions::evaluate_transitions;
use super::types::{
    MemberKey, MemberSignalContext, MemberStallState, MeshSignalReaderFn, SessionScannerFn,
    SignalSnapshot, StageTransition, StallDetectorConfig, StallStage, StallTriggerRecord,
    StallWeeklyMetrics, TransitionDecision,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::orchestrator::CoordinationOrchestrator;
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};

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

    pub(super) fn new_with_dependencies(
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

    pub(super) fn new_with_dependencies_and_teams_dir(
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
        history::weekly_metrics(&self.config, &self.trigger_history, now)
    }

    pub fn annotate_trigger(
        &self,
        trigger_id: &str,
        confirmed_true_stall: bool,
        annotated_at: DateTime<Utc>,
    ) -> Result<(), CoordinationError> {
        history::annotate_trigger(
            &self.trigger_history,
            trigger_id,
            confirmed_true_stall,
            annotated_at,
        )
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
        signals::collect_signals_for_members(
            &member_keys,
            &self.member_signal_contexts,
            self.runtime.as_ref(),
            self.session_scanner.as_ref(),
            self.mesh_signal_reader.as_ref(),
            self.config.require_medium_confidence_for_activity,
            now,
        )
    }

    pub fn ingest_session_signal(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
        is_strong: bool,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        signals::ingest_session_signal(
            &self.member_states,
            team_name,
            member_name,
            observed_at,
            is_strong,
        );
        history::mark_recovery_if_resumed(
            &self.config,
            &self.member_states,
            &self.trigger_history,
            team_name,
            member_name,
            observed_at,
        );
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
        if signals::ingest_pane_check(
            &self.member_states,
            team_name,
            member_name,
            observed_at,
            pane_alive,
        ) {
            history::mark_recovery_if_resumed(
                &self.config,
                &self.member_states,
                &self.trigger_history,
                team_name,
                member_name,
                observed_at,
            );
        }
    }

    pub fn ingest_mesh_heartbeat(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        signals::ingest_mesh_heartbeat(&self.member_states, team_name, member_name, observed_at);
        history::mark_recovery_if_resumed(
            &self.config,
            &self.member_states,
            &self.trigger_history,
            team_name,
            member_name,
            observed_at,
        );
    }

    pub fn ingest_mesh_status(
        &self,
        team_name: &str,
        member_name: &str,
        observed_at: DateTime<Utc>,
        status: super::types::MeshMemberStatus,
    ) {
        self.upsert_member(team_name, member_name, observed_at);
        if signals::ingest_mesh_status(
            &self.member_states,
            team_name,
            member_name,
            observed_at,
            status,
        ) {
            history::mark_recovery_if_resumed(
                &self.config,
                &self.member_states,
                &self.trigger_history,
                team_name,
                member_name,
                observed_at,
            );
        }
    }

    pub fn poll_once(&self) -> Vec<StageTransition> {
        self.poll_once_at(Utc::now())
    }

    pub fn poll_once_at(&self, now: DateTime<Utc>) -> Vec<StageTransition> {
        if !self.config.enabled {
            return Vec::new();
        }
        let (decisions, transitions) = self.evaluate_poll_cycle(now);
        history::record_trigger_decisions(
            &self.config,
            &self.trigger_history,
            &self.trigger_seq,
            &decisions,
        );
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
        let (decisions, transitions) = self.evaluate_poll_cycle(now);
        history::record_trigger_decisions(
            &self.config,
            &self.trigger_history,
            &self.trigger_seq,
            &decisions,
        );
        delivery::dispatch_escalations(&self.config, orchestrator, &decisions);
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
                    let snapshots = signals::collect_signals_for_members(
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
                    let snapshots_by_member = signals::build_signal_snapshot_index(&snapshots);
                    history::finalize_recovery_windows(&config, &trigger_history, now);
                    if let Ok(mut states) = member_states.lock() {
                        let decisions =
                            evaluate_transitions(&config, &mut states, &snapshots_by_member, now);
                        history::record_trigger_decisions(
                            &config,
                            &trigger_history,
                            &trigger_seq,
                            &decisions,
                        );
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

    fn evaluate_poll_cycle(
        &self,
        now: DateTime<Utc>,
    ) -> (Vec<TransitionDecision>, Vec<StageTransition>) {
        let snapshots = self.collect_signals_at(now);
        if !snapshots.is_empty() {
            apply_signal_snapshots_to_member_states(
                &self.member_states,
                &snapshots,
                self.config.require_medium_confidence_for_activity,
            );
        }
        let snapshots_by_member = signals::build_signal_snapshot_index(&snapshots);
        history::finalize_recovery_windows(&self.config, &self.trigger_history, now);
        let Ok(mut states) = self.member_states.lock() else {
            return (Vec::new(), Vec::new());
        };
        let decisions = evaluate_transitions(&self.config, &mut states, &snapshots_by_member, now);
        let transitions = decisions
            .iter()
            .map(|decision| decision.transition.clone())
            .collect();
        (decisions, transitions)
    }
}

impl Drop for StallDetectorService {
    fn drop(&mut self) {
        let _ = self.stop_polling();
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration as ChronoDuration;

    use super::*;

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
        std::thread::sleep(std::time::Duration::from_millis(1200));
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
        std::thread::sleep(std::time::Duration::from_millis(1200));
        service.stop_polling().expect("stop polling");

        assert_eq!(service.polling_tick_count(), 0);
    }
}
