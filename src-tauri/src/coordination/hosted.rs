//! Daemon-owned member hosts. The on-disk attachment is never process ownership.

use super::domain::HealthState;
use super::hosted_process::HostProcess;
use super::stores::lock::HostOperationLock;
use super::stores::runtime::{AppServerAttachment, LaunchRoot};
use super::stores::{MemberRuntimeStore, TeamConfigStore, TeamRootRegistry};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use taurhaus_lib::session_scanner::launch::HostedLaunch;

type SeatKey = (PathBuf, String, String);
type Seat = Arc<Mutex<Option<OwnedSeat>>>;
struct OwnedSeat {
    host: HostProcess,
    generation: u64,
    attachment: AppServerAttachment,
    launch_root: LaunchRoot,
    _socket_directory: SocketDirectory,
}
struct SocketDirectory(PathBuf);
impl SocketDirectory {
    fn create() -> Result<Self, String> {
        use std::os::unix::fs::DirBuilderExt;
        let path = std::env::temp_dir().join(format!("th-host-{}", uuid::Uuid::new_v4().simple()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}
impl Drop for SocketDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.0);
    }
}
#[derive(Default)]
pub(crate) struct HostedMembers {
    seats: Mutex<HashMap<SeatKey, Seat>>,
}

impl HostedMembers {
    fn seat(&self, root: &Path, team: &str, member: &str) -> Result<Seat, String> {
        let mut seats = self.seats.lock().map_err(|_| "host registry unavailable")?;
        Ok(seats
            .entry((root.into(), team.into(), member.into()))
            .or_default()
            .clone())
    }

    pub fn launch(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
        launch: &HostedLaunch,
    ) -> Result<(), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let config = TeamConfigStore::load(&root, team).map_err(|e| e.to_string())?;
        let definition = config
            .members
            .iter()
            .find(|m| m.name == member)
            .ok_or("host member missing")?;
        if !HostedLaunch::supports(definition.cli_tool)
            || definition.extra.get("adapter_mode").and_then(Value::as_str) != Some("app_server")
        {
            return Err("owned hosting is not opted in for this member".into());
        }
        let authority = LaunchRoot {
            claude_dir: root.parent().ok_or("invalid teams root")?.into(),
            teams_dir: root.clone(),
            team_incarnation_id: config.team_incarnation_id.clone(),
            root_authority_revision: registry.revision(team).map_err(|e| e.to_string())?,
        };
        if authority.team_incarnation_id.is_none() {
            return Err("host team incarnation missing".into());
        }
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        if owned.is_some() {
            return Err("owned host must be stopped before relaunch".into());
        }
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let before = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        if before.host_input_unknown
            || (before.host_input_abandoned_at != Some(before.attachment_generation) && before
                .recovery
                .claim
                .as_ref()
                .is_some_and(|r| r.stage == super::recovery_card::ReceiptStage::OutcomeUnknown))
        {
            return Err("outcome_unknown: recovery must be reconciled before relaunch".into());
        }
        if before.pane_id.is_some() || (before.app_server.is_none() && before.session_id.is_some())
        {
            return Err("app_server_switch_requires_5b_recoverable_relaunch_packet".into());
        }
        if let Some(previous) = &before.app_server {
            if previous.account_root != launch.account_root
                || before.session_id.as_deref() != Some(&previous.thread_id)
            {
                return Err("host resume account/thread identity mismatch".into());
            }
            if taurhaus_lib::platform::process_start_ticks(previous.process_id)
                .map(|v| v.to_string())
                .as_deref()
                == Some(&previous.process_start)
            {
                return Err(
                    "previous host is still alive; owner restart cannot adopt or replace it".into(),
                );
            }
        }
        let directory = SocketDirectory::create()?;
        let socket = directory.0.join("rpc.sock");
        let mut host = HostProcess::launch(
            launch,
            &definition.project_path,
            &socket,
            before.session_id.as_deref(),
            &guard,
        )?;
        if registry.resolve(team).map_err(|e| e.to_string())? != root
            || registry.revision(team).map_err(|e| e.to_string())?
                != authority.root_authority_revision
            || TeamConfigStore::load(&root, team).map_err(|e| e.to_string())? != config
        {
            return Err("host launch authority changed".into());
        }
        let mut record = before.clone();
        record.reserve_activation(&uuid::Uuid::new_v4().to_string());
        record.session_id = Some(host.thread_id.clone());
        record.app_server = Some(AppServerAttachment {
            contract: 1,
            socket_path: socket,
            thread_id: host.thread_id.clone(),
            member_id: format!("{member}@{team}"),
            account_root: launch.account_root.clone(),
            process_id: host.pid(),
            process_start: host.process_start.clone(),
            host_generation: uuid::Uuid::new_v4().to_string(),
            build: host.build.clone(),
            host: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            configuration: super::recovery_card::digest(&launch.arguments),
            trust: "unverified".into(),
            transport: "unix_ndjson".into(),
            state: "recovering".into(),
        });
        record.launch_root = Some(authority);
        record.terminal_contract = 1;
        record.harness = Some(definition.cli_tool);
        record.cli_tool = Some(definition.cli_tool);
        record.project_path = Some(definition.project_path.clone());
        record.health = HealthState::SessionDead;
        record.pane_id = None;
        record.pane_pid = None;
        record.pane_start_time = None;
        record.tmux_socket = None;
        record.tmux_session_id = None;
        record.daemon_pid = None;
        record.applied_effort = launch.applied_effort.clone();
        record.launch_account = launch.account.clone();
        record.attached_at = Some(chrono::Utc::now());
        record.recovery.launch_namespace = Some("native".into());
        record.recovery.harness_account_root =
            Some(launch.account_root.to_string_lossy().into_owned());
        // One compared replacement publishes the generation, socket and thread together.
        let data_guard =
            super::stores::lock::acquire_team_lock(&root, team).map_err(|e| e.to_string())?;
        let outcome = MemberRuntimeStore::commit_if_unchanged(
            &data_guard,
            &root,
            team,
            member,
            &super::stores::runtime::MemberRuntimeSnapshot::capture(&before),
            |current| {
                let foreign = std::mem::take(&mut current.extra);
                *current = record.clone();
                current.extra = foreign;
            },
        )
        .map_err(|e| e.to_string())?;
        drop(data_guard);
        if !matches!(
            outcome,
            super::stores::runtime::RuntimeCommitOutcome::Committed
        ) {
            return Err("host attachment changed during launch".into());
        }
        let ready = (|| -> Result<(), String> {
            use super::recovery_card::ReceiptStage;
            let card =
                super::recovery_delivery::prepare(registry, &root, team, member, "app_server")
                    .map_err(|e| e.to_string())?
                    .ok_or("startup recovery is not ready")?;
            MemberRuntimeStore::update(&root, team, member, |r| {
                r.host_input_unknown = true;
            })
            .map_err(|e| e.to_string())?;
            let result = host.input(&card.text, &guard);
            let stage = if result.is_ok() {
                ReceiptStage::Submitted
            } else if host.outcome_unknown() {
                ReceiptStage::OutcomeUnknown
            } else {
                ReceiptStage::Failed
            };
            super::recovery_delivery::observe(registry, &root, team, member, &card.receipt, stage)
                .map_err(|e| e.to_string())?;
            MemberRuntimeStore::update(&root, team, member, |r| {
                r.host_input_unknown = host.outcome_unknown();
            })
            .map_err(|e| e.to_string())?;
            result.map(|_| ())
        })();
        MemberRuntimeStore::update(&root, team, member, |record| {
            record.health = if ready.is_ok() {
                HealthState::Healthy
            } else {
                HealthState::SessionDead
            };
            if let Some(attachment) = &mut record.app_server {
                attachment.state = if ready.is_ok() {
                    "ready"
                } else {
                    "unavailable"
                }
                .into();
            }
        })
        .map_err(|e| e.to_string())?;
        ready?;
        let mut attachment = record.app_server.clone().ok_or("host attachment missing")?;
        attachment.state = "ready".into();
        *owned = Some(OwnedSeat {
            host,
            generation: record.attachment_generation,
            attachment,
            launch_root: record.launch_root.ok_or("host launch root missing")?,
            _socket_directory: directory,
        });
        Ok(())
    }

    pub fn abandon_unknown(
        &self, registry: &TeamRootRegistry, team: &str, member: &str, generation: u64,
    ) -> Result<Value, String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let owned = cell.try_lock().map_err(|_| "host member busy")?;
        let _guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let attachment = record.app_server.as_ref().ok_or("NOT_HOSTED")?;
        if generation != record.attachment_generation || owned.is_some()
            || attachment.state != "stopped"
            || taurhaus_lib::platform::process_start_ticks(attachment.process_id)
                .map(|v| v.to_string()).as_deref() == Some(&attachment.process_start) {
            return Err("Stop the hosted member before resolving its unknown input.".into());
        }
        MemberRuntimeStore::update(&root, team, member, |r| {
            r.host_input_unknown = false;
            r.host_input_abandoned_at = Some(generation);
        }).map_err(|e| e.to_string())?;
        tracing::info!(team, member, generation, "host input abandoned without replay");
        Ok(json!({"abandoned":true}))
    }

    pub fn operation(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
        generation: u64,
        operation: &str,
        params: Value,
    ) -> Result<Value, String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        let seat = owned
            .as_mut()
            .ok_or("host is unavailable in this daemon; controlled resume required")?;
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let attachment = record.app_server.as_ref().ok_or("member is not hosted")?;
        if generation != seat.generation
            || generation != record.attachment_generation
            || attachment != &seat.attachment
            || record.launch_root.as_ref() != Some(&seat.launch_root)
        {
            return Err("host attachment changed; refresh before another operation".into());
        }
        let config = TeamConfigStore::load(&root, team).map_err(|e| e.to_string())?;
        let authority = record
            .launch_root
            .as_ref()
            .ok_or("host launch root missing")?;
        if registry.resolve(team).map_err(|e| e.to_string())? != authority.teams_dir
            || registry.revision(team).map_err(|e| e.to_string())?
                != authority.root_authority_revision
            || config.team_incarnation_id != authority.team_incarnation_id
            || !config.members.iter().any(|m| {
                m.name == member
                    && m.extra.get("adapter_mode").and_then(Value::as_str) == Some("app_server")
            })
        {
            return Err("host team/member authority changed".into());
        }
        match operation {
            "transcript" => seat.host.transcript(&guard),
            "input" => {
                if record.host_input_unknown {
                    return Err("outcome_unknown: previous input requires reconciliation".into());
                }
                let text = params["text"].as_str().ok_or("missing input text")?;
                use super::recovery_card::ReceiptStage;
                if record.recovery.claim.as_ref().is_some_and(|claim| {
                    claim.card_key.context == record.context()
                        && claim.stage == ReceiptStage::OutcomeUnknown
                }) {
                    return Err("outcome_unknown: recovery offer requires reconciliation".into());
                }
                let needs_recovery = record
                    .recovery
                    .last_delivered
                    .as_ref()
                    .is_none_or(|receipt| receipt.card_key.context != record.context());
                let card = if needs_recovery {
                    // A fallback card belongs to the first next turn, never an active-turn steer.
                    if seat.host.transcript(&guard)?["thread"]["status"]["type"] != "idle" {
                        return Err("pending: recovery requires the next idle turn".into());
                    }
                    super::recovery_delivery::prepare(registry, &root, team, member, "app_server")
                        .map_err(|e| e.to_string())?
                } else {
                    None
                };
                let input = card.as_ref().map_or_else(
                    || text.to_string(),
                    |card| format!("{}\n\n{}", card.text, text),
                );
                // Persist ambiguity before any possible input bytes, including owner crashes.
                MemberRuntimeStore::update(&root, team, member, |r| {
                    r.host_input_unknown = true;
                })
                .map_err(|e| e.to_string())?;
                let result = seat.host.input(&input, &guard);
                if let Some(card) = card {
                    let stage = if result.is_ok() {
                        ReceiptStage::Submitted
                    } else if seat.host.outcome_unknown() {
                        ReceiptStage::OutcomeUnknown
                    } else {
                        ReceiptStage::Failed
                    };
                    super::recovery_delivery::observe(
                        registry,
                        &root,
                        team,
                        member,
                        &card.receipt,
                        stage,
                    )
                    .map_err(|e| e.to_string())?;
                }
                if result.is_ok() || !seat.host.outcome_unknown() {
                    MemberRuntimeStore::update(&root, team, member, |r| {
                        r.host_input_unknown = false;
                    })
                    .map_err(|e| e.to_string())?;
                }
                result
            }
            "interrupt" => seat.host.interrupt(&guard),
            "approval" => seat.host.approval(
                &params["requestId"],
                params["accept"]
                    .as_bool()
                    .ok_or("missing approval decision")?,
                &guard,
            ),
            _ => Err("UNKNOWN_METHOD".into()),
        }
    }

    pub fn reconcile(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
    ) -> Result<(), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let Ok(mut owned) = cell.try_lock() else {
            return Ok(());
        };
        let guard = match HostOperationLock::acquire(&root, team, member, Duration::ZERO) {
            Ok(guard) => guard,
            Err(super::errors::CoordinationError::Conflict(message))
                if message == "host operation deferred: lock busy" => {
                    tracing::debug!(team, member, "host liveness deferred: lock busy");
                    return Ok(());
                }
            Err(error) => return Err(error.to_string()),
        };
        if owned.as_mut().is_some_and(|seat| seat.host.alive()) {
            return Ok(());
        }
        MemberRuntimeStore::update(&root, team, member, |record| {
            if let Some(host) = &mut record.app_server {
                if host.state == "ready" {
                    host.state = "unavailable".into();
                    record.attachment_generation = record.attachment_generation.saturating_add(1);
                    record.health = HealthState::SessionDead;
                }
            }
        })
        .map_err(|e| e.to_string())?;
        drop(owned.take());
        drop(guard);
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), String> {
        let seats: Vec<_> = self
            .seats
            .lock()
            .map_err(|_| "host registry unavailable")?
            .iter()
            .map(|(key, cell)| (key.clone(), cell.clone()))
            .collect();
        let mut failure = None;
        for ((root, team, member), cell) in seats {
            let active = match cell.try_lock() {
                Ok(seat) => seat.is_some(),
                Err(_) => {
                    failure = Some("host member busy during shutdown".to_string());
                    continue;
                }
            };
            if active {
                if let Err(error) = self.stop(&TeamRootRegistry::new(root), &team, &member) {
                    failure = Some(error);
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    pub fn stop(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
    ) -> Result<(), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        if let Some(seat) = owned.as_ref() {
            if record.app_server.as_ref() != Some(&seat.attachment)
                || record.attachment_generation != seat.generation
                || record.launch_root.as_ref() != Some(&seat.launch_root)
            {
                drop(owned.take());
                return Err("host attachment changed before shutdown".into());
            }
        } else if let Some(attachment) = &record.app_server {
            if taurhaus_lib::platform::process_start_ticks(attachment.process_id)
                .map(|v| v.to_string())
                .as_deref()
                == Some(&attachment.process_start)
            {
                return Err("live host belongs to a previous daemon; cannot claim shutdown".into());
            }
        } else {
            return Err("member is not hosted".into());
        }
        MemberRuntimeStore::update(&root, team, member, |record| {
            record.attachment_generation = record.attachment_generation.saturating_add(1);
            record.health = HealthState::SessionDead;
            if let Some(host) = &mut record.app_server {
                host.state = "stopped".into();
            }
        })
        .map_err(|e| e.to_string())?;
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
    fn hosted_definite_rejection_allows_new_input_and_relaunch() {
        // Regression: 7921d720 made definite steer rejections permanently ambiguous.
        for rejected in ["completion race", "wrong turn"] {
            let tmp = tempfile::tempdir().unwrap();
            let registry = seat(tmp.path());
            let launch = fixture(tmp.path());
            let hosts = HostedMembers::default();
            hosts.launch(&registry, "team", "seat", &launch).unwrap();
            let generation = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap().attachment_generation;
            hosts.operation(&registry, "team", "seat", generation, "input", json!({"text":"active"})).unwrap();
            let error = hosts.operation(&registry, "team", "seat", generation, "input", json!({"text":rejected})).unwrap_err();
            assert!(error.starts_with("failed:"), "{error}");
            hosts.operation(&registry, "team", "seat", generation, "input", json!({"text":"next input"})).unwrap();
            hosts.stop(&registry, "team", "seat").unwrap();
            hosts.launch(&registry, "team", "seat", &launch).unwrap();
            hosts.stop(&registry, "team", "seat").unwrap();
        }
    }

    #[test]
    fn hosted_liveness_defers_a_mesh_lock_holder() {
        // Regression: fa18910c let shared host-lock contention abort team passes.
        use fs2::FileExt;
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let launch = fixture(tmp.path());
        let hosts = HostedMembers::default();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let holder = std::fs::OpenOptions::new().read(true).write(true)
            .open(tmp.path().join("team/state/app-server/seat.lock")).unwrap();
        holder.lock_exclusive().unwrap();
        assert!(hosts.reconcile(&registry, "team", "seat").is_ok());
        drop(holder);
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn owned_member_publishes_resumes_and_excludes_mesh() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let launch = fixture(tmp.path());
        let hosts = HostedMembers::default();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert_eq!(
            record.recovery.last_delivered.as_ref().map(|r| r.stage),
            Some(super::super::recovery_card::ReceiptStage::Submitted)
        );
        let attachment = record.app_server.as_ref().unwrap();
        assert_eq!(attachment.thread_id, "owned-thread");
        assert_eq!(record.session_id.as_deref(), Some("owned-thread"));
        assert_eq!(attachment.account_root, tmp.path());
        assert!(attachment.process_id > 0 && !attachment.process_start.is_empty());
        assert!(record.attachment_generation > 0 && record.launch_root.is_some());
        assert!(record.pane_id.is_none());
        let wire: Value = serde_json::from_slice(
            &std::fs::read(tmp.path().join("team/runtime/seat.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(wire["health"], "active");
        assert!(wire["contextGeneration"].is_string());
        let generation = record.attachment_generation;
        // Regression: 83077dad checked PID/thread but ignored changed native transport facts.
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| {
            r.app_server.as_mut().unwrap().transport = "unsupported".into()
        })
        .unwrap();
        assert!(hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"wrong transport"})
            )
            .is_err());
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| {
            r.app_server = record.app_server.clone()
        })
        .unwrap();

        let holder =
            HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
        assert!(hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"blocked"})
            )
            .is_err());
        drop(holder);
        hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"operator"}),
            )
            .unwrap();
        assert!(hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "transcript",
                Value::Null
            )
            .unwrap()
            .to_string()
            .contains("operator"));
        hosts.stop(&registry, "team", "seat").unwrap();
        assert_eq!(
            MemberRuntimeStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .app_server
                .unwrap()
                .state,
            "stopped"
        );
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let resumed = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert_eq!(resumed.session_id, record.session_id);
        assert!(resumed.attachment_generation > generation);
        assert_ne!(
            resumed.app_server.unwrap().host_generation,
            attachment.host_generation
        );
        assert!(hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"stale"})
            )
            .is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn owned_member_refuses_pane_conversion_and_does_not_adopt_after_owner_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let launch = fixture(tmp.path());
        let hosts = HostedMembers::default();
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| {
            r.pane_id = Some("%42".into())
        })
        .unwrap();
        assert!(hosts.launch(&registry, "team", "seat", &launch).is_err());
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| r.pane_id = None).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        let restarted_owner = HostedMembers::default();
        // Regression: 83077dad reported a live unowned child as stopped on owner restart.
        assert!(restarted_owner.stop(&registry, "team", "seat").is_err());
        assert!(restarted_owner
            .launch(&registry, "team", "seat", &launch)
            .is_err());
        assert!(restarted_owner
            .operation(
                &registry,
                "team",
                "seat",
                record.attachment_generation,
                "input",
                json!({"text":"must not adopt"})
            )
            .is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_recovery_hook_holds_exclusion_through_stdout() {
        struct CheckedOutput {
            path: PathBuf,
            bytes: Vec<u8>,
        }
        impl std::io::Write for CheckedOutput {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                use fs2::FileExt;
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.path)?;
                assert!(
                    file.try_lock_exclusive().is_err(),
                    "hook stdout must exclude mesh"
                );
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let hosts = HostedMembers::default();
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        super::super::compact_hook::tests::write_snapshot_fixture(tmp.path(), "team", "seat");
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"owned-thread","cwd":tmp.path(),"transcript_path":tmp.path().join("rollout-owned-thread.jsonl")});
        let mut output = CheckedOutput {
            path: tmp.path().join("team/state/app-server/seat.lock"),
            bytes: Vec::new(),
        };
        super::super::compact_hook::run_compact_hook_cli(
            payload.to_string().as_bytes(),
            &mut output,
            tmp.path(),
        )
        .unwrap();
        let response: Value = serde_json::from_slice(&output.bytes).unwrap();
        assert!(response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("Current task: #680"));
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert_eq!(record.context_generation, 1);
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_compaction_refuses_an_old_thread_at_the_same_cwd() {
        // Regression: 5c95d585 inherited cwd fallback and admitted a former host's compaction.
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let hosts = HostedMembers::default();
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        super::super::compact_hook::tests::write_snapshot_fixture(tmp.path(), "team", "seat");
        let before = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"old-thread","cwd":tmp.path(),"transcript_path":tmp.path().join("rollout-old-thread.jsonl")});
        let mut output = Vec::new();
        assert!(super::super::compact_hook::run_compact_hook_cli(
            payload.to_string().as_bytes(),
            &mut output,
            tmp.path()
        )
        .is_err());
        assert!(output.is_empty());
        let after = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert_eq!(after.context_generation, before.context_generation);
        assert_eq!(after.recovery, before.recovery);
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_recovery_first_input_uses_existing_pending_compaction_without_new_generation() {
        use super::super::stores::{
            record_delivery_at, CompactionDeliveryResult, MemberCompactionStore,
        };
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let hosts = HostedMembers::default();
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        super::super::compact_hook::tests::write_snapshot_fixture(tmp.path(), "team", "seat");
        let generation = MemberRuntimeStore::load(tmp.path(), "team", "seat")
            .unwrap()
            .attachment_generation;
        hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"startup marker"}),
            )
            .unwrap();
        {
            let _guard =
                HostOperationLock::acquire(tmp.path(), "team", "seat", Duration::ZERO).unwrap();
            record_delivery_at(
                tmp.path(),
                "team",
                "seat",
                crate::session_scanner::cli_tool::CliTool::Codex,
                "owned-thread",
                chrono::Utc::now(),
                CompactionDeliveryResult::Skipped,
            )
            .unwrap();
        }
        assert!(
            MemberCompactionStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .unwrap()
                .pending
        );
        hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "input",
                json!({"text":"after compact"}),
            )
            .unwrap();
        let transcript = hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "transcript",
                Value::Null,
            )
            .unwrap();
        assert!(
            transcript["thread"]["turns"][0]["items"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .starts_with("[taurhaus] recovery_card")
        );
        assert_eq!(
            transcript["thread"]["turns"][1]["items"][0]["content"][0]["text"],
            "startup marker"
        );
        let text = transcript["thread"]["turns"][2]["items"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(text.starts_with("[taurhaus] recovery_card") && text.ends_with("after compact"));
        assert_eq!(
            MemberRuntimeStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .context_generation,
            1
        );
        assert!(
            !MemberCompactionStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .unwrap()
                .pending
        );
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_publication_preserves_concurrent_foreign_fields_and_checks_incarnation() {
        // Regression: 83077dad replaced the compared current record with a stale clone,
        // losing Mesh extensions that are intentionally outside the attachment comparison.
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let mut launch = fixture(tmp.path());
        launch.environment.insert(
            "FAKE_RUNTIME".into(),
            tmp.path()
                .join("team/runtime/seat.json")
                .to_string_lossy()
                .into_owned(),
        );
        let hosts = HostedMembers::default();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let record = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert_eq!(record.extra.get("foreignClaim"), Some(&json!("concurrent")));
        let path = tmp.path().join("team/config.json");
        let mut config: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        config["team_incarnation_id"] = json!("recreated");
        std::fs::write(path, config.to_string()).unwrap();
        assert!(hosts
            .operation(
                &registry,
                "team",
                "seat",
                record.attachment_generation,
                "input",
                json!({"text":"wrong incarnation"})
            )
            .is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_daemon_shutdown_publishes_before_owned_children_exit() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let hosts = HostedMembers::default();
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        let before = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        hosts.shutdown().unwrap();
        let after = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
        assert!(after.attachment_generation > before.attachment_generation);
        assert_eq!(after.app_server.unwrap().state, "stopped");
        assert!(
            taurhaus_lib::platform::process_start_ticks(before.app_server.unwrap().process_id)
                .is_none()
        );
    }

    #[test]
    fn hosted_shutdown_preserves_a_replacement_attachment() {
        // Regression: 10fa0eb2 treated an absent appServer as permission to stop a replacement pane.
        for replacement_host in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let registry = seat(tmp.path());
            let hosts = HostedMembers::default();
            hosts
                .launch(&registry, "team", "seat", &fixture(tmp.path()))
                .unwrap();
            let original = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
            MemberRuntimeStore::update(tmp.path(), "team", "seat", |record| {
                record.attachment_generation += 1;
                if replacement_host {
                    record.app_server.as_mut().unwrap().host_generation = "replacement".into();
                } else {
                    record.app_server = None;
                    record.pane_id = Some("%replacement".into());
                }
            })
            .unwrap();
            let replaced = MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap();
            assert!(hosts.stop(&registry, "team", "seat").is_err());
            assert_eq!(
                serde_json::to_value(MemberRuntimeStore::load(tmp.path(), "team", "seat").unwrap())
                    .unwrap(),
                serde_json::to_value(replaced).unwrap()
            );
            assert!(taurhaus_lib::platform::process_start_ticks(
                original.app_server.unwrap().process_id
            )
            .is_none());
        }
    }

    #[test]
    fn hosted_launch_requires_codex_capability_and_records_account_selection() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let mut config = TeamConfigStore::load(tmp.path(), "team").unwrap();
        config.members[0].cli_tool = crate::session_scanner::cli_tool::CliTool::Grok;
        TeamConfigStore::save(tmp.path(), "team", &config).unwrap();
        let hosts = HostedMembers::default();
        let launch = fixture(tmp.path());
        // Regression: 83077dad allowed a Codex child to be published as another harness.
        assert!(hosts.launch(&registry, "team", "seat", &launch).is_err());
        config.members[0].cli_tool = crate::session_scanner::cli_tool::CliTool::Codex;
        TeamConfigStore::save(tmp.path(), "team", &config).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        assert_eq!(
            MemberRuntimeStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .launch_account
                .account_applied,
            Some(true)
        );
        hosts.stop(&registry, "team", "seat").unwrap();
    }
}
