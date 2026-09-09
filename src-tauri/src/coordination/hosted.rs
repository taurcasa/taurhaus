//! Daemon-owned member hosts. The on-disk attachment is never process ownership.

use super::domain::HealthState;
use super::hosted_process::HostProcess;
use super::stores::lock::HostOperationLock;
use super::stores::runtime::{AppServerAttachment, LaunchRoot};
use super::stores::{MemberRuntimeStore, TeamConfigStore, TeamRootRegistry};
use crate::session_scanner::launch::HostedLaunch;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type SeatKey = (PathBuf, String, String);
type Seat = Arc<Mutex<Option<OwnedSeat>>>;
struct OwnedSeat {
    host: HostProcess,
    generation: u64,
    _socket_directory: tempfile::TempDir,
}
#[derive(Default)]
pub(crate) struct HostedMembers { seats: Mutex<HashMap<SeatKey, Seat>> }

impl HostedMembers {
    fn seat(&self, root: &Path, team: &str, member: &str) -> Result<Seat, String> {
        let mut seats = self.seats.lock().map_err(|_| "host registry unavailable")?;
        Ok(seats.entry((root.into(), team.into(), member.into())).or_default().clone())
    }

    pub fn launch(&self, registry: &TeamRootRegistry, team: &str, member: &str, launch: &HostedLaunch) -> Result<(), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let config = TeamConfigStore::load(&root, team).map_err(|e| e.to_string())?;
        let definition = config.members.iter().find(|m| m.name == member).ok_or("host member missing")?;
        if definition.extra.get("adapter_mode").and_then(Value::as_str) != Some("app_server") {
            return Err("owned hosting is not opted in for this member".into());
        }
        let authority = LaunchRoot {
            claude_dir: root.parent().ok_or("invalid teams root")?.into(), teams_dir: root.clone(),
            team_incarnation_id: config.team_incarnation_id.clone(), root_authority_revision: registry.revision(team).map_err(|e| e.to_string())?,
        };
        if authority.team_incarnation_id.is_none() { return Err("host team incarnation missing".into()); }
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        if owned.is_some() { return Err("owned host must be stopped before relaunch".into()); }
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2)).map_err(|e| e.to_string())?;
        let before = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        if before.pane_id.is_some() || (before.app_server.is_none() && before.session_id.is_some()) {
            return Err("app_server_switch_requires_5b_recoverable_relaunch_packet".into());
        }
        if let Some(previous) = &before.app_server {
            if previous.account_root != launch.account_root || before.session_id.as_deref() != Some(&previous.thread_id) {
                return Err("host resume account/thread identity mismatch".into());
            }
            if taurhaus_lib::platform::process_start_ticks(previous.process_id).map(|v| v.to_string()).as_deref() == Some(&previous.process_start) {
                return Err("previous host is still alive; owner restart cannot adopt or replace it".into());
            }
        }
        let directory = tempfile::Builder::new().prefix("th-host-").tempdir().map_err(|e| e.to_string())?;
        let socket = directory.path().join("rpc.sock");
        let host = HostProcess::launch(launch, &definition.project_path, &socket, before.session_id.as_deref(), &guard)?;
        let mut record = before.clone();
        record.reserve_activation(&uuid::Uuid::new_v4().to_string());
        record.session_id = Some(host.thread_id.clone());
        record.app_server = Some(AppServerAttachment {
            contract: 1, socket_path: socket, thread_id: host.thread_id.clone(), member_id: format!("{member}@{team}"),
            account_root: launch.account_root.clone(), process_id: host.pid(), process_start: host.process_start.clone(),
            host_generation: uuid::Uuid::new_v4().to_string(), build: host.build.clone(),
            host: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            configuration: super::recovery_card::digest(&launch.arguments),
            trust: "unverified".into(), transport: "unix_ndjson".into(), state: "ready".into(),
        });
        record.launch_root = Some(authority);
        record.terminal_contract = 1;
        record.harness = Some(definition.cli_tool);
        record.cli_tool = Some(definition.cli_tool);
        record.project_path = Some(definition.project_path.clone());
        record.health = HealthState::Healthy;
        record.pane_id = None; record.pane_pid = None; record.pane_start_time = None;
        record.tmux_socket = None; record.tmux_session_id = None;
        record.daemon_pid = None;
        record.applied_effort = launch.applied_effort.clone();
        record.attached_at = Some(chrono::Utc::now());
        record.recovery.harness_account_root = Some(launch.account_root.to_string_lossy().into_owned());
        // One compared replacement publishes the generation, socket and thread together.
        let data_guard = super::stores::lock::acquire_team_lock(&root, team).map_err(|e| e.to_string())?;
        let outcome = MemberRuntimeStore::commit_if_unchanged(&data_guard, &root, team, member,
            &super::stores::runtime::MemberRuntimeSnapshot::capture(&before), |current| *current = record.clone()).map_err(|e| e.to_string())?;
        drop(data_guard);
        if !matches!(outcome, super::stores::runtime::RuntimeCommitOutcome::Committed) { return Err("host attachment changed during launch".into()); }
        *owned = Some(OwnedSeat { host, generation: record.attachment_generation, _socket_directory: directory });
        Ok(())
    }

    pub fn operation(&self, registry: &TeamRootRegistry, team: &str, member: &str, generation: u64, operation: &str, params: Value) -> Result<Value, String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        let seat = owned.as_mut().ok_or("host is unavailable in this daemon; controlled resume required")?;
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2)).map_err(|e| e.to_string())?;
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let attachment = record.app_server.as_ref().ok_or("member is not hosted")?;
        if generation != seat.generation || generation != record.attachment_generation || attachment.thread_id != seat.host.thread_id
            || attachment.process_id != seat.host.pid() || attachment.process_start != seat.host.process_start || attachment.state != "ready" {
            return Err("host attachment changed; refresh before another operation".into());
        }
        match operation {
            "transcript" => seat.host.transcript(&guard),
            "input" => {
                if record.extra.get("hostInputUnknown") == Some(&Value::Bool(true)) { return Err("outcome_unknown: previous input requires reconciliation".into()); }
                let text = params["text"].as_str().ok_or("missing input text")?;
                // Persist ambiguity before any possible input bytes, including owner crashes.
                MemberRuntimeStore::update(&root, team, member, |r| { r.extra.insert("hostInputUnknown".into(), json!(true)); }).map_err(|e| e.to_string())?;
                let result = seat.host.input(text, &guard);
                if result.is_ok() || !seat.host.outcome_unknown() {
                    MemberRuntimeStore::update(&root, team, member, |r| { r.extra.insert("hostInputUnknown".into(), json!(false)); }).map_err(|e| e.to_string())?;
                }
                result
            }
            "interrupt" => seat.host.interrupt(&guard),
            "approval" => seat.host.approval(&params["requestId"], params["accept"].as_bool().ok_or("missing approval decision")?, &guard),
            _ => Err("UNKNOWN_METHOD".into()),
        }
    }

    pub fn stop(&self, registry: &TeamRootRegistry, team: &str, member: &str) -> Result<(), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2)).map_err(|e| e.to_string())?;
        MemberRuntimeStore::update(&root, team, member, |record| {
            record.attachment_generation = record.attachment_generation.saturating_add(1);
            record.health = HealthState::SessionDead;
            if let Some(host) = &mut record.app_server { host.state = "stopped".into(); }
        }).map_err(|e| e.to_string())?;
        // Only a Child held by this daemon can be killed, never a record PID.
        drop(owned.take());
        drop(guard);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::coordination::hosted_process::tests::fixture;
    use crate::coordination::stores::{MemberRuntimeRecord, TeamConfigStore};
    use serde_json::json;

    pub(crate) fn seat(root: &Path) -> TeamRootRegistry {
        std::fs::create_dir_all(root.join("team")).unwrap();
        let config = json!({"schema_version":3,"name":"team","created_at":"2026-01-01T00:00:00Z","team_incarnation_id":"team-one","members":[{"name":"seat","role":"agent","cli_tool":"codex","project_path":root,"adapter_mode":"app_server"}]});
        std::fs::write(root.join("team/config.json"), config.to_string()).unwrap();
        TeamConfigStore::load(root, "team").unwrap();
        MemberRuntimeStore::save(root, "team", "seat", &MemberRuntimeRecord::default()).unwrap();
        TeamRootRegistry::new(root.into())
    }

    #[test]
    fn owned_member_publishes_resumes_and_excludes_mesh() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let launch = fixture(tmp.path());
        let hosts = HostedMembers::default();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        let attachment = record.app_server.as_ref().unwrap();
        assert_eq!(attachment.thread_id, "owned-thread");
        assert_eq!(record.session_id.as_deref(), Some("owned-thread"));
        assert_eq!(attachment.account_root, tmp.path());
        assert!(attachment.process_id > 0 && !attachment.process_start.is_empty());
        assert!(record.attachment_generation > 0 && record.launch_root.is_some());
        assert!(record.pane_id.is_none());
        let wire: Value = serde_json::from_slice(&std::fs::read(tmp.path().join("team/runtime/seat.json")).unwrap()).unwrap();
        assert_eq!(wire["health"], "active");
        assert!(wire["contextGeneration"].is_string());
        let generation = record.attachment_generation;
        let holder = HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        assert!(hosts.operation(&registry, "team", "seat", generation, "input", json!({"text":"blocked"})).is_err());
        drop(holder);
        hosts.operation(&registry, "team", "seat", generation, "input", json!({"text":"operator"})).unwrap();
        assert!(hosts.operation(&registry, "team", "seat", generation, "transcript", Value::Null).unwrap().to_string().contains("operator"));
        hosts.stop(&registry, "team", "seat").unwrap();
        assert_eq!(MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap().app_server.unwrap().state, "stopped");
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let resumed = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert_eq!(resumed.session_id, record.session_id);
        assert!(resumed.attachment_generation > generation);
        assert_ne!(resumed.app_server.unwrap().host_generation, attachment.host_generation);
        assert!(hosts.operation(&registry, "team", "seat", generation, "input", json!({"text":"stale"})).is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn owned_member_refuses_pane_conversion_and_does_not_adopt_after_owner_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let launch = fixture(tmp.path());
        let hosts = HostedMembers::default();
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| r.pane_id = Some("%42".into())).unwrap();
        assert!(hosts.launch(&registry, "team", "seat", &launch).is_err());
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| r.pane_id = None).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        let restarted_owner = HostedMembers::default();
        assert!(restarted_owner.launch(&registry, "team", "seat", &launch).is_err());
        assert!(restarted_owner.operation(&registry, "team", "seat", record.attachment_generation, "input", json!({"text":"must not adopt"})).is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
    }
}
