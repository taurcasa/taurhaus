//! Daemon-owned member hosts. The on-disk attachment is never process ownership.

use super::domain::HealthState;
use super::errors::CoordinationError;
use super::hosted_process::HostProcess;
use super::recovery_card::ReceiptStage;
use super::recovery_delivery;
use super::stores::lock::{acquire_team_lock, HostOperationLock};
use super::stores::runtime::{
    AppServerAttachment, LaunchRoot, MemberRuntimeSnapshot, RuntimeCommitOutcome,
};
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
    launch: HostedLaunch,
    pane_attached: bool,
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
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
pub(crate) struct HostedMembers {
    seats: Mutex<HashMap<SeatKey, Seat>>,
}

fn host_alive(host: &AppServerAttachment) -> bool {
    taurhaus_lib::platform::process_start_ticks(host.process_id)
        .map(|v| v.to_string())
        .as_deref()
        == Some(&host.process_start)
}

/// Close only the recorded operator TUI under the shared terminal exclusion.
pub(super) fn detach_owned_tui(
    root: &Path,
    team: &str,
    member: &str,
    record: &super::stores::MemberRuntimeRecord,
    runtime: &dyn super::runtime::CoordinationRuntime,
) -> Result<bool, CoordinationError> {
    let Some(pane) = record.pane_id.as_deref() else {
        return Ok(false);
    };
    super::stores::lock::terminal_write(root, team, member, "detach_tui", || {
        let Some(live) = runtime.live_pane(pane)? else {
            return Ok(false);
        };
        if super::runtime::pane_belongs_to_member(record, &live)
            != super::runtime::PaneOwnership::Owned
        {
            return Err(CoordinationError::Conflict(
                "attached pane identity changed".into(),
            ));
        }
        runtime.kill_aitx_pane(pane)?;
        Ok(true)
    })
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
        if let Some(seat) = owned.as_mut() {
            let record =
                MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
            if seat.host.alive()
                && record.app_server.as_ref() == Some(&seat.attachment)
                && record.attachment_generation == seat.generation
                && record.launch_root.as_ref() == Some(&authority)
                && seat.launch.account_root == launch.account_root
                && seat.launch.arguments == launch.arguments
                && seat.launch.program == launch.program
            {
                return Ok(()); // Pane restart never resumes/recreates a live owned thread.
            }
            return Err("owned host must be stopped before relaunch".into());
        }
        let guard = HostOperationLock::acquire_for_launch(&root, team, member)
            .map_err(|e| e.to_string())?;
        let before = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        if before.host_input_unknown
            || (before.host_input_abandoned_at != Some(before.attachment_generation)
                && before
                    .recovery
                    .claim
                    .as_ref()
                    .is_some_and(|r| r.stage == ReceiptStage::OutcomeUnknown))
        {
            return Err("outcome_unknown: recovery must be reconciled before relaunch".into());
        }
        if before.app_server.is_none() && (before.pane_id.is_some() || before.session_id.is_some())
        {
            return Err("app_server_switch_requires_5b_recoverable_relaunch_packet".into());
        }
        if let Some(previous) = &before.app_server {
            if previous.account_root != launch.account_root
                || before.session_id.as_deref() != Some(&previous.thread_id)
            {
                return Err("host resume account/thread identity mismatch".into());
            }
            if host_alive(previous) {
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
        launch.prepare_attach_home(&directory.0.join("tui"), &host.attach_config)?;
        let mut record = before.clone();
        record.reserve_activation(&uuid::Uuid::new_v4().to_string());
        record.session_id = Some(host.thread_id.clone());
        record.app_server = Some(AppServerAttachment {
            contract: 1,
            socket_path: socket.clone(),
            thread_id: host.thread_id.clone(),
            member_id: format!("{member}@{team}"),
            account_root: launch.account_root.clone(),
            process_id: host.pid(),
            process_start: host.process_start.clone(),
            host_generation: uuid::Uuid::new_v4().to_string(),
            build: host.build.clone(),
            host: "taurhaus-daemon-owned-thread/1".into(),
            configuration: "strict-config/1".into(),
            configuration_digest: Some(super::recovery_card::digest(&launch.arguments)),
            instruction_sources: host.instruction_sources.clone(),
            trust: "daemon-owned/1".into(),
            transport: taurhaus_lib::session_scanner::launch::HostedDescriptor::codex().transport,
            attach_argv: launch.attach_argv(&socket, &host.thread_id)?,
            state: "recovering".into(),
        });
        record.launch_root = Some(authority);
        record.terminal_contract = 1;
        record.harness = Some(definition.cli_tool);
        record.cli_tool = Some(definition.cli_tool);
        record.project_path = Some(definition.project_path.clone());
        record.health = HealthState::SessionDead;
        // Keep the previous view identity until attach can reuse or retire it
        // under terminal exclusion, including after daemon/host restart.
        record.daemon_pid = None;
        record.applied_effort = launch.applied_effort.clone();
        record.launch_account = launch.account.clone();
        record.attached_at = Some(chrono::Utc::now());
        record.recovery.launch_namespace = Some("native".into());
        record.recovery.harness_account_root =
            Some(launch.account_root.to_string_lossy().into_owned());
        // One compared replacement publishes the generation, socket and thread together.
        let data_guard = acquire_team_lock(&root, team).map_err(|e| e.to_string())?;
        let outcome = MemberRuntimeStore::commit_if_unchanged(
            &data_guard,
            &root,
            team,
            member,
            &MemberRuntimeSnapshot::capture(&before),
            |current| {
                let foreign = std::mem::take(&mut current.extra);
                *current = record.clone();
                current.extra = foreign;
            },
        )
        .map_err(|e| e.to_string())?;
        drop(data_guard);
        if !matches!(outcome, RuntimeCommitOutcome::Committed) {
            return Err("host attachment changed during launch".into());
        }
        let ready = (|| -> Result<(), String> {
            let card = recovery_delivery::prepare(registry, &root, team, member, "app_server")
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
            recovery_delivery::observe(registry, &root, team, member, &card.receipt, stage)
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
            launch: launch.clone(),
            pane_attached: false,
            generation: record.attachment_generation,
            attachment,
            launch_root: record.launch_root.ok_or("host launch root missing")?,
            _socket_directory: directory,
        });
        Ok(())
    }

    /// Attach a view only after bounded RPC validation. Host and terminal locks
    /// never overlap; the seat mutex excludes concurrent daemon stop/relaunch.
    pub fn attach_pane(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
        runtime: &dyn super::runtime::CoordinationRuntime,
        layout: &str,
    ) -> Result<(String, bool), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let mut owned = cell.try_lock().map_err(|_| "host member busy")?;
        let seat = owned
            .as_mut()
            .ok_or("owned thread absent; TUI attach refused")?;
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let before = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let config = TeamConfigStore::load(&root, team).map_err(|e| e.to_string())?;
        let definition = config
            .members
            .iter()
            .find(|m| {
                m.name == member
                    && m.extra.get("adapter_mode").and_then(Value::as_str) == Some("app_server")
            })
            .ok_or("host member missing or opted out")?;
        if before.app_server.as_ref() != Some(&seat.attachment)
            || before.attachment_generation != seat.generation
            || before.launch_root.as_ref() != Some(&seat.launch_root)
            || config.team_incarnation_id != seat.launch_root.team_incarnation_id
            || registry.revision(team).map_err(|e| e.to_string())?
                != seat.launch_root.root_authority_revision
        {
            return Err("host attachment changed before TUI attach".into());
        }
        seat.host.transcript(&guard)?; // Never open a TUI without its exact thread.
        drop(guard);
        let command = seat.launch.attach_command(
            &seat.attachment.attach_argv,
            &seat
                .attachment
                .socket_path
                .parent()
                .ok_or("host socket parent missing")?
                .join("tui"),
        );
        let mut sent = false;
        let (resolution, live, address) =
            super::stores::lock::terminal_write(&root, team, member, "attach_tui", || {
                // A live old TUI still points at the previous socket. Retire it,
                // but only if its recorded PID/start ticks still establish ownership.
                if !seat.pane_attached {
                    if let Some(pane) = before.pane_id.as_deref() {
                        if let Some(live) = runtime.live_pane(pane)? {
                            if super::runtime::pane_belongs_to_member(&before, &live)
                                == super::runtime::PaneOwnership::Owned
                                && !runtime.pane_is_shell(pane)?
                            {
                                runtime.kill_aitx_pane(pane)?;
                            }
                        }
                    }
                }
                let resolution = super::runtime::resolve_or_create_pane_for_member(
                    runtime,
                    definition,
                    Some(&before),
                    layout,
                )?;
                let pane = &resolution.pane_id;
                let attached = (|| {
                    if !resolution.reused_pane || runtime.pane_is_shell(pane)? {
                        sent = true;
                        runtime.send_tmux_keys_with_enter(pane, &command)?;
                    }
                    let live =
                        runtime
                            .live_pane(pane)?
                            .filter(|p| !p.is_dead)
                            .ok_or_else(|| {
                                CoordinationError::Conflict("attached pane disappeared".into())
                            })?;
                    Ok((live, runtime.tmux_address(pane)?))
                })();
                if attached.is_err() && (resolution.created_new_pane || sent) {
                    let _ = runtime.kill_aitx_pane(pane);
                }
                attached.map(|(live, address)| (resolution, live, address))
            })
            .map_err(|e| e.to_string())?;
        // Terminal exclusion is released before any runtime data lock.
        let data_guard = acquire_team_lock(&root, team).map_err(|e| e.to_string())?;
        let outcome = MemberRuntimeStore::commit_if_unchanged(
            &data_guard,
            &root,
            team,
            member,
            &MemberRuntimeSnapshot::capture(&before),
            |record| {
                record.pane_id = Some(resolution.pane_id.clone());
                record.pane_pid = live.pane_pid;
                record.pane_start_time = live.pane_start_time;
                record.tmux_socket = address.as_ref().map(|a| a.0.clone());
                record.tmux_session_id = address.as_ref().map(|a| a.1.clone());
                record.attached_at = Some(chrono::Utc::now());
            },
        )
        .map_err(|e| e.to_string());
        drop(data_guard);
        if !matches!(outcome, Ok(RuntimeCommitOutcome::Committed)) {
            if resolution.created_new_pane || sent {
                let _ = super::stores::lock::terminal_write(
                    &root,
                    team,
                    member,
                    "attach_cleanup",
                    || runtime.kill_aitx_pane(&resolution.pane_id),
                );
            }
            return Err("host changed during TUI attach".into());
        }
        seat.pane_attached = true;
        Ok((resolution.pane_id, resolution.reused_pane))
    }

    /// Taurhaus's rollback half. Live Mesh-owned delivery still needs the paired switch packet.
    pub fn rollback_to_pane(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
        runtime: &dyn super::runtime::CoordinationRuntime,
    ) -> Result<(), String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let owned = cell.try_lock().map_err(|_| "host member busy")?;
        let _guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let config = TeamConfigStore::load(&root, team).map_err(|e| e.to_string())?;
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let attachment = record.app_server.as_ref().ok_or("NOT_HOSTED")?;
        if config.extra.get("delivery_owner").and_then(Value::as_str) == Some("team") {
            return Err("app_server_rollback_on_team_owned_team: stop the seat, remove it, re-add it with delivery tmux".into());
        }
        let opted_out = config.members.iter().any(|m| {
            m.name == member
                && m.extra.get("adapter_mode").and_then(Value::as_str) != Some("app_server")
        });
        let authority = record
            .launch_root
            .as_ref()
            .ok_or("host launch root missing")?;
        if !opted_out
            || owned.is_some()
            || attachment.state != "stopped"
            || record.session_id.as_deref() != Some(&attachment.thread_id)
            || authority.teams_dir != root
            || authority.team_incarnation_id != config.team_incarnation_id
            || authority.root_authority_revision
                != registry.revision(team).map_err(|e| e.to_string())?
            || host_alive(attachment)
        {
            return Err(
                "controlled rollback requires the stopped owned attachment and explicit opt-out"
                    .into(),
            );
        }
        if record.host_input_unknown
            || (record.host_input_abandoned_at != Some(record.attachment_generation)
                && record
                    .recovery
                    .claim
                    .as_ref()
                    .is_some_and(|r| r.stage == ReceiptStage::OutcomeUnknown))
        {
            return Err("outcome_unknown: resolve hosted input before rollback".into());
        }
        // Refuse retained native attempts; never reinterpret missing receipts as unread.
        match std::fs::read_dir(root.join(team).join("state/delivery")) {
            Ok(entries) => {
                for entry in entries {
                    if entry
                        .map_err(|e| e.to_string())?
                        .path()
                        .extension()
                        .is_some_and(|s| s == "native")
                    {
                        return Err("pending: reconcile native attempts before rollback".into());
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        drop(_guard); // Terminal exclusion must never overlap host or data locks.
        match detach_owned_tui(&root, team, member, &record, runtime) {
            Err(CoordinationError::Conflict(message))
                if message == "attached pane identity changed" =>
            {
                tracing::warn!(
                    team,
                    member,
                    "Rollback skipped a foreign attached pane; clearing stale identity"
                );
            }
            result => {
                result.map_err(|e| e.to_string())?;
            }
        }
        let _guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        if registry.resolve(team).map_err(|e| e.to_string())? != root
            || registry.revision(team).map_err(|e| e.to_string())?
                != authority.root_authority_revision
            || TeamConfigStore::load(&root, team).map_err(|e| e.to_string())? != config
        {
            return Err("host rollback authority changed".into());
        }
        let data_guard = acquire_team_lock(&root, team).map_err(|e| e.to_string())?;
        let outcome = MemberRuntimeStore::commit_if_unchanged(
            &data_guard,
            &root,
            team,
            member,
            &MemberRuntimeSnapshot::capture(&record),
            |r| {
                r.attachment_generation = r.attachment_generation.saturating_add(1);
                r.host_rollback = Some(
                    json!({"oldMode":"app_server", "newMode":"tmux", "optIn":false,
                "attachment":attachment, "fence":r.attachment_generation, "unresolvedAttempts":[],
                "abandonedAt":r.host_input_abandoned_at}),
                );
                r.app_server = None;
                // Never type a plain launch into a retained remote TUI. The host is
                // already stopped; rollback must resolve a fresh operator pane.
                r.pane_id = None;
                r.pane_pid = None;
                r.pane_start_time = None;
                r.tmux_socket = None;
                r.tmux_session_id = None;
                r.health = HealthState::SessionDead;
            },
        )
        .map_err(|e| e.to_string())?;
        match outcome {
            RuntimeCommitOutcome::Committed => Ok(()),
            _ => Err("host attachment changed during rollback".into()),
        }
    }

    pub fn abandon_unknown(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
        generation: u64,
    ) -> Result<Value, String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let owned = cell.try_lock().map_err(|_| "host member busy")?;
        let _guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let attachment = record.app_server.as_ref().ok_or("NOT_HOSTED")?;
        if generation != record.attachment_generation
            || owned.is_some()
            || attachment.state != "stopped"
            || host_alive(attachment)
        {
            return Err("Stop the hosted member before resolving its unknown input.".into());
        }
        MemberRuntimeStore::update(&root, team, member, |r| {
            r.host_input_unknown = false;
            r.host_input_abandoned_at = Some(generation);
        })
        .map_err(|e| e.to_string())?;
        tracing::info!(
            team,
            member,
            generation,
            "host input abandoned without replay"
        );
        Ok(json!({"abandoned":true, "attachmentGeneration":generation}))
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
            .ok_or("failed: host is unavailable in this daemon; controlled resume required")?;
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
        let (state, recovery_turn) = if matches!(operation, "transcript" | "recover" | "input") {
            poll_compaction(&mut seat.host, &root, team, member, &guard)?
        } else {
            (Value::Null, false)
        };
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let requested = operation;
        let operation = if recovery_turn && operation != "input" {
            "recovery_input"
        } else {
            operation
        };
        match operation {
            "transcript" | "recover" => Ok(state),
            "input" | "recovery_input" => {
                if record.host_input_unknown {
                    return Err("outcome_unknown: previous input requires reconciliation".into());
                }
                let text = if operation == "recovery_input" {
                    ""
                } else {
                    params["text"].as_str().ok_or("missing input text")?
                };
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
                    if state["thread"]["status"]["type"] != "idle" {
                        return Err("pending: recovery requires the next idle turn".into());
                    }
                    recovery_delivery::prepare(registry, &root, team, member, "app_server")
                        .map_err(|e| e.to_string())?
                } else {
                    None
                };
                let input = card.as_ref().map_or_else(
                    || text.to_string(),
                    |card| {
                        if text.is_empty() {
                            card.text.clone()
                        } else {
                            format!("{}\n\n{}", card.text, text)
                        }
                    },
                );
                // Persist ambiguity before any possible input bytes, including owner crashes.
                MemberRuntimeStore::update(&root, team, member, |r| {
                    r.host_input_unknown = true;
                })
                .map_err(|e| e.to_string())?;
                let result = seat.host.input_checked(&input, &state, &guard);
                if let Some(card) = card {
                    let stage = if result.is_ok() {
                        ReceiptStage::Submitted
                    } else if seat.host.outcome_unknown() {
                        ReceiptStage::OutcomeUnknown
                    } else {
                        ReceiptStage::Failed
                    };
                    recovery_delivery::observe(registry, &root, team, member, &card.receipt, stage)
                        .map_err(|e| e.to_string())?;
                    if stage == ReceiptStage::Submitted {
                        record_host_delivery(&root, team, member)?;
                    }
                }
                if result.is_ok() || !seat.host.outcome_unknown() {
                    MemberRuntimeStore::update(&root, team, member, |r| {
                        r.host_input_unknown = false;
                    })
                    .map_err(|e| e.to_string())?;
                }
                if requested == "transcript" && result.is_ok() {
                    seat.host.transcript(&guard)
                } else {
                    result
                }
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
            Err(CoordinationError::Conflict(message))
                if message == "host operation deferred: lock busy" =>
            {
                tracing::debug!(team, member, "host liveness deferred: lock busy");
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
        };
        if owned.as_mut().is_some_and(|seat| seat.host.alive()) {
            let generation = owned.as_ref().unwrap().generation;
            drop(owned);
            drop(guard);
            return self
                .operation(registry, team, member, generation, "recover", Value::Null)
                .map(|_| ());
        }
        MemberRuntimeStore::update(&root, team, member, |record| {
            if let Some(host) = &mut record.app_server {
                let state = if owned.is_none() && host_alive(host) {
                    "orphaned"
                } else if host.state == "stopped" {
                    "stopped"
                } else {
                    "unavailable"
                };
                if host.state != state {
                    host.state = state.into();
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
            if host_alive(attachment) {
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

/// Run from daemon liveness and UI operations, always after attachment validation.
fn poll_compaction(
    host: &mut HostProcess,
    root: &Path,
    team: &str,
    member: &str,
    guard: &HostOperationLock,
) -> Result<(Value, bool), String> {
    use super::stores::compaction::{emit_host_compaction, record_host_boundary};
    let mut received = None;
    loop {
        let state = host.transcript(guard);
        for boundary in host.take_compactions() {
            if boundary["threadId"] != host.thread_id {
                emit_host_compaction(
                    team,
                    member,
                    &boundary,
                    "compaction.codex_host.skipped",
                    "different_thread",
                );
            } else if record_host_boundary(root, team, member, &boundary, "host_notification")
                .map_err(|e| e.to_string())?
            {
                emit_host_compaction(
                    team,
                    member,
                    &boundary,
                    "compaction.codex_host.received",
                    "",
                );
                received = Some(boundary);
            } else {
                emit_host_compaction(
                    team,
                    member,
                    &boundary,
                    "compaction.codex_host.skipped",
                    "already_recorded",
                );
            }
        }
        let Some(boundary) = &received else {
            return state.map(|s| (s, false));
        };
        if state
            .as_ref()
            .is_ok_and(|s| s["thread"]["status"]["type"] == "idle")
        {
            return state.map(|s| (s, true));
        }
        if state.is_err()
            || guard
                .remaining()
                .map_or(true, |r| r < Duration::from_millis(100))
        {
            emit_host_compaction(
                team,
                member,
                boundary,
                "compaction.codex_host.deferred",
                "thread_not_idle",
            );
            return state.map(|s| (s, false));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn record_host_delivery(root: &Path, team: &str, member: &str) -> Result<(), String> {
    use super::stores::compaction::{emit_host_compaction, CompactionDeliveryResult};
    use super::stores::MemberCompactionStore;
    let guard = acquire_team_lock(root, team).map_err(|e| e.to_string())?;
    if let Some(mut state) =
        MemberCompactionStore::load(root, team, member).map_err(|e| e.to_string())?
    {
        state.last_delivery_result = CompactionDeliveryResult::Injected;
        MemberCompactionStore::save_locked(&guard, root, team, member, &state)
            .map_err(|e| e.to_string())?;
        super::compaction_events::emit_compaction_delivery(
            "compaction.injected",
            super::compaction_events::CompactionDeliveryEvent {
                tool: crate::session_scanner::cli_tool::CliTool::Codex,
                team_name: team.into(),
                member_name: member.into(),
                session_id: state.last_session_id,
                compaction_timestamp: state.last_compaction_timestamp,
                delivery_result: "injected".into(),
                skip_reason: None,
                fail_reason: None,
                delivery: Some("host_turn".into()),
            },
        );
        emit_host_compaction(
            team,
            member,
            &state.host_boundary.unwrap_or_default(),
            "compaction.codex_host.delivered",
            "",
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::compact_hook::{run_compact_hook_cli, tests::write_snapshot_fixture};
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
    pub(crate) fn running(root: &Path) -> (TeamRootRegistry, HostedMembers) {
        let registry = seat(root);
        let hosts = HostedMembers::default();
        hosts
            .launch(&registry, "team", "seat", &fixture(root))
            .unwrap();
        (registry, hosts)
    }
    pub(crate) fn saved(root: &Path) -> MemberRuntimeRecord {
        MemberRuntimeStore::load(root, "team", "seat").unwrap()
    }
    fn transcript(hosts: &HostedMembers, registry: &TeamRootRegistry, generation: u64) -> Value {
        hosts
            .operation(
                registry,
                "team",
                "seat",
                generation,
                "transcript",
                Value::Null,
            )
            .unwrap()
    }
    pub(crate) fn input(
        hosts: &HostedMembers,
        registry: &TeamRootRegistry,
        generation: u64,
        text: &str,
    ) -> Result<Value, String> {
        hosts.operation(
            registry,
            "team",
            "seat",
            generation,
            "input",
            json!({"text":text}),
        )
    }
    #[test]
    fn hosted_paired_identity_is_a_stable_class() {
        let tmp = tempfile::tempdir().unwrap();
        let (_registry, _hosts) = running(tmp.path());
        let wire = serde_json::to_value(saved(tmp.path())).unwrap();
        let host = &wire["appServer"];
        assert_eq!(host["host"], "taurhaus-daemon-owned-thread/1");
        assert_eq!(host["configuration"], "strict-config/1");
        assert_eq!(host["trust"], "daemon-owned/1");
        assert!(host["configurationDigest"]
            .as_str()
            .is_some_and(|v| !v.is_empty()));
        assert_eq!(host["instructionSources"], json!([]));
    }

    #[test]
    fn hosted_relaunch_retains_and_reuses_attached_pane() {
        // Regression: b4a4b2dd cleared the pane identity on host relaunch, orphaning its TUI.
        use super::super::runtime::{RecordingCoordinationRuntime, RuntimeCall};
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let runtime = RecordingCoordinationRuntime::default();
        hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .unwrap();
        let before = saved(tmp.path());
        hosts.stop(&registry, "team", "seat").unwrap();
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        assert_eq!(saved(tmp.path()).pane_id, before.pane_id);
        runtime.set_pane_shell(before.pane_id.as_deref().unwrap(), true);
        let calls = runtime.calls().len();
        hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .unwrap();
        let after = saved(tmp.path());
        assert_eq!(after.pane_id, before.pane_id);
        assert_eq!(after.session_id, before.session_id);
        assert!(runtime.calls()[calls..].iter().any(|call| matches!(call,
            RuntimeCall::SendKeys { keys, .. } if keys.contains(after.app_server.as_ref().unwrap().socket_path.to_str().unwrap()))));
        hosts.stop(&registry, "team", "seat").unwrap();
        runtime.set_pane_shell(before.pane_id.as_deref().unwrap(), false);
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .unwrap();
        assert!(runtime.calls().iter().any(|call| matches!(call,
            RuntimeCall::KillPane { pane_id } if Some(pane_id) == before.pane_id.as_ref())));
        assert_ne!(saved(tmp.path()).pane_id, before.pane_id);
    }

    #[test]
    fn hosted_attach_uses_private_strict_config_with_thread_policy() {
        // Regression: b4a4b2dd attached on the account home, allowing TUI settings to overwrite the thread.
        use super::super::runtime::{RecordingCoordinationRuntime, RuntimeCall};
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "model = \"operator-model\"\n",
        )
        .unwrap();
        let (registry, hosts) = running(tmp.path());
        let runtime = RecordingCoordinationRuntime::default();
        hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .unwrap();
        let record = saved(tmp.path());
        let attachment = record.app_server.unwrap();
        assert!(attachment
            .attach_argv
            .contains(&"--strict-config".to_string()));
        let home = attachment.socket_path.parent().unwrap().join("tui");
        let config: toml::Value =
            toml::from_str(&std::fs::read_to_string(home.join("config.toml")).unwrap()).unwrap();
        assert_eq!(config["model"].as_str(), Some("fake-model"));
        assert_eq!(config["model_reasoning_effort"].as_str(), Some("low"));
        assert_eq!(config["sandbox_mode"].as_str(), Some("read-only"));
        assert_eq!(config["approval_policy"].as_str(), Some("never"));
        assert!(config.get("developer_instructions").is_none());
        assert_eq!(config["project_doc_max_bytes"].as_integer(), Some(0));
        assert_eq!(
            std::fs::read_link(home.join("auth.json")).unwrap(),
            tmp.path().join("auth.json")
        );
        assert!(runtime.calls().iter().any(|call| matches!(call,
            RuntimeCall::SendKeys { keys, .. } if keys.contains(&format!("CODEX_HOME={}", home.display())))));
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("config.toml")).unwrap(),
            "model = \"operator-model\"\n"
        );
    }

    #[test]
    fn hosted_attach_absent_thread_never_opens_a_pane() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let hosts = HostedMembers::default();
        let runtime = super::super::runtime::RecordingCoordinationRuntime::default();
        assert!(hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .is_err());
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        let cell = hosts.seat(tmp.path(), "team", "seat").unwrap();
        cell.lock().unwrap().as_mut().unwrap().host.thread_id = "absent-thread".into();
        assert!(hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .is_err());
        assert!(runtime.calls().is_empty());
    }

    #[test]
    fn hosted_attach_command_survives_private_tmux_pane_restart() {
        // Regression: cadd533e never opened a TUI for the owned thread.
        use super::super::runtime::{RecordingCoordinationRuntime, RuntimeCall};
        use std::process::{Child, Command, Stdio};
        struct ScratchTmux {
            executable: PathBuf,
            root: PathBuf,
            child: Child,
        }
        impl ScratchTmux {
            fn command(&self) -> Command {
                let mut command = Command::new(&self.executable);
                command
                    .env_clear()
                    .env("HOME", &self.root)
                    .env("SHELL", "/bin/sh")
                    .env("PATH", "/usr/bin:/bin")
                    .args([
                        "-S",
                        self.root.join("tmux.sock").to_str().unwrap(),
                        "-f",
                        "/dev/null",
                    ]);
                command
            }
            fn run(&self, args: &[&str]) -> String {
                let output = self.command().args(args).output().unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                String::from_utf8(output.stdout).unwrap().trim().into()
            }
        }
        impl Drop for ScratchTmux {
            fn drop(&mut self) {
                let _ = self.command().arg("kill-server").output();
                let _ = self.child.kill(); // Only the foreground server this test spawned.
                let _ = self.child.wait();
            }
        }
        let Some(executable) = std::env::var_os("PATH").and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("tmux"))
                .find(|path| path.is_file())
        }) else {
            eprintln!("SKIP: private tmux test requires tmux on PATH");
            return;
        };
        let tmp = tempfile::tempdir_in("/tmp").unwrap();
        let (registry, hosts) = running(tmp.path());
        let runtime = RecordingCoordinationRuntime::default();
        hosts
            .attach_pane(&registry, "team", "seat", &runtime, "new_window")
            .unwrap();
        let command = runtime
            .calls()
            .into_iter()
            .find_map(|call| match call {
                RuntimeCall::SendKeys { keys, .. } => Some(keys),
                _ => None,
            })
            .unwrap();
        let socket = tmp.path().join("tmux.sock");
        let child = Command::new(&executable)
            .env_clear()
            .env("HOME", tmp.path())
            .env("SHELL", "/bin/sh")
            .env("PATH", "/usr/bin:/bin")
            .args(["-D", "-S", socket.to_str().unwrap(), "-f", "/dev/null"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let tmux = ScratchTmux {
            executable,
            root: tmp.path().into(),
            child,
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !socket.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let original = saved(tmp.path());
        for count in 1..=2 {
            let pane = tmux.run(&[
                "new-session",
                "-d",
                "-s",
                "operator",
                "-P",
                "-F",
                "#{pane_id}",
                "/bin/sh",
            ]);
            tmux.run(&["send-keys", "-t", &pane, "-l", &command]);
            tmux.run(&["send-keys", "-t", &pane, "Enter"]);
            let path = tmp.path().join("attach-events.jsonl");
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            let events = loop {
                let data = std::fs::read_to_string(&path).unwrap_or_default();
                if data.lines().count() == count {
                    break data;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "fake TUI did not start"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            let event: Value = serde_json::from_str(events.lines().last().unwrap()).unwrap();
            assert_eq!(
                event["argv"],
                json!(original.app_server.as_ref().unwrap().attach_argv)
            );
            assert_eq!(
                event["codexHome"],
                original
                    .app_server
                    .as_ref()
                    .unwrap()
                    .socket_path
                    .parent()
                    .unwrap()
                    .join("tui")
                    .to_str()
                    .unwrap()
            );
            assert!(event["tmux"].is_null());
            tmux.run(&["kill-pane", "-t", &pane]);
            assert_eq!(
                transcript(&hosts, &registry, original.attachment_generation)["thread"]["id"],
                "owned-thread"
            );
            assert_eq!(saved(tmp.path()).app_server, original.app_server);
        }
    }

    #[test]
    fn hosted_definite_rejection_allows_new_input_and_relaunch() {
        // Regression: 9b50346b made definite steer rejections permanently ambiguous.
        for rejected in ["completion race", "wrong turn"] {
            let tmp = tempfile::tempdir().unwrap();
            let (registry, hosts) = running(tmp.path());
            let launch = fixture(tmp.path());
            let generation = saved(tmp.path()).attachment_generation;
            input(&hosts, &registry, generation, "active").unwrap();
            let error = input(&hosts, &registry, generation, rejected).unwrap_err();
            assert!(error.starts_with("failed:"), "{error}");
            input(&hosts, &registry, generation, "next input").unwrap();
            hosts.stop(&registry, "team", "seat").unwrap();
            hosts.launch(&registry, "team", "seat", &launch).unwrap();
            hosts.stop(&registry, "team", "seat").unwrap();
        }
    }
    #[test]
    fn owned_member_publishes_resumes_and_excludes_mesh() {
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let record = saved(tmp.path());
        assert_eq!(
            record.recovery.last_delivered.as_ref().map(|r| r.stage),
            Some(ReceiptStage::Submitted)
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
        assert!(input(&hosts, &registry, generation, "wrong transport").is_err());
        MemberRuntimeStore::update(tmp.path(), "team", "seat", |r| {
            r.app_server = record.app_server.clone()
        })
        .unwrap();

        let holder = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(tmp.path().join("team/state/app-server/seat.lock"))
            .unwrap();
        fs2::FileExt::lock_exclusive(&holder).unwrap();
        // Regression: a9c8109b let shared host-lock contention abort team passes.
        assert!(hosts.reconcile(&registry, "team", "seat").is_ok());
        assert!(input(&hosts, &registry, generation, "blocked")
            .unwrap_err()
            .contains("deferred: lock busy"));
        drop(holder);
        input(&hosts, &registry, generation, "operator").unwrap();
        assert!(transcript(&hosts, &registry, generation)
            .to_string()
            .contains("operator"));
        hosts.stop(&registry, "team", "seat").unwrap();
        assert_eq!(saved(tmp.path()).app_server.unwrap().state, "stopped");
        hosts
            .launch(&registry, "team", "seat", &fixture(tmp.path()))
            .unwrap();
        let resumed = saved(tmp.path());
        assert_eq!(resumed.session_id, record.session_id);
        assert!(resumed.attachment_generation > generation);
        assert_ne!(
            resumed.app_server.unwrap().host_generation,
            attachment.host_generation
        );
        assert!(input(&hosts, &registry, generation, "stale").is_err());
        let before = saved(tmp.path());
        hosts.shutdown().unwrap();
        let after = saved(tmp.path());
        assert!(after.attachment_generation > before.attachment_generation);
        assert_eq!(after.app_server.unwrap().state, "stopped");
        assert!(!host_alive(&before.app_server.unwrap()));
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
        let (registry, hosts) = running(tmp.path());
        write_snapshot_fixture(tmp.path(), "team", "seat");
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"owned-thread","cwd":tmp.path(),"transcript_path":tmp.path().join("rollout-owned-thread.jsonl")});
        let mut output = CheckedOutput {
            path: tmp.path().join("team/state/app-server/seat.lock"),
            bytes: Vec::new(),
        };
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, tmp.path()).unwrap();
        let response: Value = serde_json::from_slice(&output.bytes).unwrap();
        assert!(response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("Current task: #680"));
        let record = saved(tmp.path());
        assert_eq!(record.context_generation, 1);
        hosts.stop(&registry, "team", "seat").unwrap();
    }
    #[test]
    fn hosted_compaction_refuses_an_old_thread_at_the_same_cwd() {
        // Regression: 5c95d585 inherited cwd fallback and admitted a former host's compaction.
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        write_snapshot_fixture(tmp.path(), "team", "seat");
        let before = saved(tmp.path());
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"old-thread","cwd":tmp.path(),"transcript_path":tmp.path().join("rollout-old-thread.jsonl")});
        let mut output = Vec::new();
        assert!(
            run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, tmp.path()).is_err()
        );
        assert!(output.is_empty());
        let after = saved(tmp.path());
        assert_eq!(after.context_generation, before.context_generation);
        assert_eq!(after.recovery, before.recovery);
        hosts.stop(&registry, "team", "seat").unwrap();
    }
    #[test]
    fn hosted_recovery_first_input_uses_existing_pending_compaction_without_new_generation() {
        // Regression: 6f61f611, attempt-8 continuation: busy compaction must persist for first input.
        use super::super::stores::MemberCompactionStore;
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let (registry, hosts) = running(tmp.path());
        write_snapshot_fixture(tmp.path(), "team", "seat");
        let generation = saved(tmp.path()).attachment_generation;
        input(&hosts, &registry, generation, "startup marker").unwrap();
        std::fs::write(tmp.path().join("compact.json"), r#"{"busy":true}"#).unwrap();
        hosts.reconcile(&registry, "team", "seat").unwrap();
        sink.flush_for_test().unwrap();
        assert!(std::fs::read_to_string(tmp.path().join("events.jsonl"))
            .unwrap()
            .contains("compaction.codex_host.deferred"));
        assert!(
            MemberCompactionStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .unwrap()
                .pending
        );
        std::fs::write(tmp.path().join("compact.json"), "{}").unwrap();
        input(&hosts, &registry, generation, "after compact").unwrap();
        let transcript = transcript(&hosts, &registry, generation);
        let message = |index: usize| {
            transcript["thread"]["turns"][index]["items"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
        };
        assert!(message(0).starts_with("[taurhaus] recovery_card"));
        assert_eq!(message(1), "startup marker");
        assert!(
            message(3).starts_with("[taurhaus] recovery_card")
                && message(3).ends_with("after compact")
        );
        assert_eq!(saved(tmp.path()).context_generation, 1);
        assert!(
            !MemberCompactionStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .unwrap()
                .pending
        );
        hosts.stop(&registry, "team", "seat").unwrap();
    }
    #[test]
    fn hosted_notification_compaction_delivers_once_and_deduplicates_hook() {
        // Regression: 6f61f611, attempt-8 continuation step5-boundary-events.json:
        // completed contextCompaction was retained but never admitted or recovered.
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        transcript(&hosts, &registry, generation);
        std::fs::write(tmp.path().join("compact.json"), r#"{"expectCard":true}"#).unwrap();
        hosts.reconcile(&registry, "team", "seat").unwrap();
        let record = saved(tmp.path());
        assert_eq!(record.context_generation, 1);
        assert_eq!(
            serde_json::to_value(&record).unwrap()["contextGeneration"],
            "1"
        );
        let submitted: Value = serde_json::from_slice(
            &std::fs::read(tmp.path().join("boundary-submission.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(submitted["state"]["source"], "host_notification");
        let receipt = record.recovery.last_delivered.unwrap();
        assert_eq!(receipt.stage, ReceiptStage::Submitted);
        let (card, _) =
            recovery_delivery::read_current(&registry, tmp.path(), "team", "seat").unwrap();
        let without_clock = |text: &str| {
            text.lines()
                .filter(|line| !line.starts_with("Generated: "))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(
            without_clock(submitted["input"][0]["text"].as_str().unwrap()),
            without_clock(&card)
        );
        assert_eq!(submitted["turnId"], "2");
        let state = super::super::stores::MemberCompactionStore::load(tmp.path(), "team", "seat")
            .unwrap()
            .unwrap();
        assert!(!state.pending);
        assert_eq!(
            state.last_delivery_result,
            super::super::stores::CompactionDeliveryResult::Injected
        );
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"owned-thread","turn_id":"compact-turn","item_id":"compact-item","cwd":tmp.path(),"transcript_path":tmp.path().join("rollout-owned-thread.jsonl")});
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, tmp.path()).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("recovery_card"));
        assert_eq!(saved(tmp.path()).context_generation, 1);
        let view = transcript(&hosts, &registry, generation);
        assert!(view["thread"]["turns"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["items"][0]["type"] == "contextCompaction"));
        let requests: Vec<Value> = std::fs::read_to_string(tmp.path().join("requests.jsonl"))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["method"] == "turn/start")
                .count(),
            2
        );
        sink.flush_for_test().unwrap();
        let events = std::fs::read_to_string(tmp.path().join("events.jsonl")).unwrap();
        for event in [
            "compaction.codex_host.received",
            "compaction.codex_host.delivered",
            "compaction.codex_host.skipped",
            "already_recorded",
            "host_turn",
        ] {
            assert!(events.contains(event), "missing {event}");
        }
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_notification_compaction_deduplicates_an_earlier_hook() {
        // Regression: 6f61f611, attempt-8 continuation boundary: either observer may arrive first.
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        transcript(&hosts, &registry, generation);
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"owned-thread","turnId":"compact-turn","itemId":"compact-item","cwd":tmp.path(),"transcript_path":tmp.path().join("rollout-owned-thread.jsonl")});
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, tmp.path()).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("recovery_card"));
        std::fs::write(tmp.path().join("compact.json"), "{}").unwrap();
        hosts.reconcile(&registry, "team", "seat").unwrap();
        assert_eq!(saved(tmp.path()).context_generation, 1);
        let requests = std::fs::read_to_string(tmp.path().join("requests.jsonl")).unwrap();
        assert_eq!(requests.matches("turn/start").count(), 1);
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_notification_compaction_ignores_foreign_thread() {
        // Regression: 6f61f611 / attempt-8 continuation: only the owned thread is authority.
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let (registry, hosts) = running(tmp.path());
        transcript(&hosts, &registry, saved(tmp.path()).attachment_generation);
        std::fs::write(tmp.path().join("compact.json"), r#"{"threadId":"foreign"}"#).unwrap();
        hosts.reconcile(&registry, "team", "seat").unwrap();
        assert_eq!(saved(tmp.path()).context_generation, 0);
        assert!(
            super::super::stores::MemberCompactionStore::load(tmp.path(), "team", "seat")
                .unwrap()
                .is_none()
        );
        sink.flush_for_test().unwrap();
        assert!(std::fs::read_to_string(tmp.path().join("events.jsonl"))
            .unwrap()
            .contains("different_thread"));
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
        let record = saved(tmp.path());
        assert_eq!(record.extra.get("foreignClaim"), Some(&json!("concurrent")));
        let path = tmp.path().join("team/config.json");
        let mut config: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        config["team_incarnation_id"] = json!("recreated");
        std::fs::write(path, config.to_string()).unwrap();
        assert!(input(
            &hosts,
            &registry,
            record.attachment_generation,
            "wrong incarnation"
        )
        .is_err());
        hosts.stop(&registry, "team", "seat").unwrap();
    }
    #[test]
    fn hosted_shutdown_preserves_a_replacement_attachment() {
        // Regression: 10fa0eb2 treated an absent appServer as permission to stop a replacement pane.
        for replacement_host in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let (registry, hosts) = running(tmp.path());
            let original = saved(tmp.path());
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
            let replaced = saved(tmp.path());
            assert!(hosts.stop(&registry, "team", "seat").is_err());
            assert_eq!(
                serde_json::to_value(saved(tmp.path())).unwrap(),
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
        assert_eq!(saved(tmp.path()).launch_account.account_applied, Some(true));
        hosts.stop(&registry, "team", "seat").unwrap();
    }
}
