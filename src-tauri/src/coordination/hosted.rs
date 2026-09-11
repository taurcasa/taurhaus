//! Daemon-owned member hosts. The on-disk attachment is never process ownership.

use super::domain::HealthState;
use super::errors::CoordinationError;
use super::hosted_process::{HostProcess, LaunchError};
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
use taurhaus_lib::daemon::session_activity::{
    hosted_activity::HostedActivityLease, SessionActivityHub,
};
use taurhaus_lib::session_scanner::{launch::HostedLaunch, RuntimeSession, SessionGroupKind};

fn host_connection_closed(error: &str) -> bool {
    matches!(
        error.strip_prefix("outcome_unknown: ").unwrap_or(error),
        "host connection closed"
            | "host connection closed during write"
            | "host WebSocket closed"
            | "host connection unavailable"
    )
}

type SeatKey = (PathBuf, String, String);
type Seat = Arc<Mutex<Option<OwnedSeat>>>;
struct OwnedSeat {
    _activity: HostedActivityLease,
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
    #[cfg(test)]
    activity_hub: Arc<SessionActivityHub>,
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
    fn activity_hub(&self) -> Arc<SessionActivityHub> {
        #[cfg(test)]
        {
            self.activity_hub.clone()
        }
        #[cfg(not(test))]
        {
            SessionActivityHub::shared()
        }
    }

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
        let mut socket = directory.0.join("rpc.sock");
        let mut attempt = 1;
        let mut host = loop {
            match HostProcess::launch(
                launch,
                &definition.project_path,
                &socket,
                before.session_id.as_deref(),
                &guard,
                (team, member),
                attempt,
            ) {
                Ok(host) => break host,
                Err(error @ LaunchError::ExitedBeforeReadiness { .. })
                    if attempt == 1
                        && guard
                            .remaining()
                            .is_ok_and(|left| left > Duration::from_secs(3)) =>
                {
                    // Readiness never arrived: no thread or runtime publication to replay.
                    // A retry needs budget beyond its own back-off; otherwise the exit is
                    // reported as the #162 failure below instead of a bare deadline error.
                    error.log_exit((team, member), 2, true);
                    std::thread::sleep(Duration::from_millis(1500));
                    socket = directory.0.join("retry.sock");
                    attempt = 2;
                }
                Err(error) => {
                    error.log_exit((team, member), attempt, false);
                    return Err(error.to_string());
                }
            }
        };
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
            host: taurhaus_lib::session_scanner::launch::HostedDescriptor::HOST.into(),
            configuration: taurhaus_lib::session_scanner::launch::HostedDescriptor::CONFIGURATION
                .into(),
            configuration_digest: Some(super::recovery_card::digest(&launch.arguments)),
            instruction_sources: host.instruction_sources.clone(),
            trust: taurhaus_lib::session_scanner::launch::HostedDescriptor::TRUST.into(),
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
        let weak = Arc::downgrade(&cell);
        let (refresh_root, refresh_team, refresh_member) =
            (root.clone(), team.to_owned(), member.to_owned());
        #[cfg(test)]
        host.set_activity_hub_for_test(self.activity_hub());
        #[cfg(test)]
        let refresh_hub = Arc::downgrade(&self.activity_hub);
        let activity = self.activity_hub().register_host(
            socket.clone(),
            launch.account_root.clone(),
            RuntimeSession {
                project_path: definition.project_path.to_string_lossy().into_owned(),
                args: record.app_server.as_ref().unwrap().attach_argv.join(" "),
                cli_tool: definition.cli_tool,
                tmux_pane: record.pane_id.clone(),
                session_id: Some(host.thread_id.clone()),
                source: Some("host_unavailable".into()),
                group_kind: SessionGroupKind::MeshTeam,
                group_id: Some(team.into()),
                group_label: Some(team.into()),
                member_name: Some(member.into()),
                ..Default::default()
            },
            Arc::new(move || {
                let Some(cell) = weak.upgrade() else { return };
                let Ok(mut owned) = cell.try_lock() else {
                    return;
                };
                let Some(seat) = owned.as_mut() else { return };
                if !seat.host.activity_retry_due() {
                    return;
                }
                // Reinitializing/resuming needs multiple round trips. Keep the lock
                // non-blocking, but give reconnect its own full five-second budget.
                let guard = if seat.host.needs_reconnect() {
                    HostOperationLock::acquire(
                        &refresh_root,
                        &refresh_team,
                        &refresh_member,
                        Duration::ZERO,
                    )
                } else {
                    HostOperationLock::acquire_for_activity(
                        &refresh_root,
                        &refresh_team,
                        &refresh_member,
                    )
                };
                let Ok(guard) = guard else { return };
                let disconnected = !seat.host.alive() || !seat.attachment.socket_path.exists();
                let closed = !disconnected
                    && seat
                        .host
                        .refresh_activity(&guard)
                        .is_err_and(|error| host_connection_closed(&error));
                if disconnected || closed {
                    #[cfg(test)]
                    let hub = refresh_hub.upgrade().expect("owned activity lease");
                    #[cfg(not(test))]
                    let hub = SessionActivityHub::shared();
                    hub.publish_host_status(
                        &seat.attachment.socket_path,
                        &seat.host.thread_id,
                        &Value::Null,
                    );
                }
            }),
        );
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
            _activity: activity,
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
        self.activity_hub().attach_host_pane(
            &seat.attachment.socket_path,
            &resolution.pane_id,
            live.pane_pid,
        );
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
        // Reads share one acquisition budget; operator mutations are never queued on the cell.
        let read_deadline = matches!(operation, "transcript" | "recover")
            .then(|| std::time::Instant::now() + Duration::from_millis(1500));
        let mut owned = loop {
            match cell.try_lock() {
                Ok(owned) => break owned,
                Err(std::sync::TryLockError::WouldBlock) if read_deadline.is_some() => {
                    let remaining = read_deadline
                        .unwrap()
                        .saturating_duration_since(std::time::Instant::now());
                    if remaining.is_zero() {
                        return Err("host member busy".into());
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(50)));
                }
                Err(_) => return Err("host member busy".into()),
            }
        };
        let seat = owned
            .as_mut()
            .ok_or("failed: host is unavailable in this daemon; controlled resume required")?;
        // The cross-process lock keeps its previous waits: `recover` runs inside the
        // live-presence reconcile under the team orchestrator and may mutate, so it
        // never queues; everything else keeps the 2 s wait it had before the read budget.
        let wait = if operation == "recover" {
            Duration::ZERO
        } else {
            Duration::from_secs(2)
        };
        let guard = HostOperationLock::acquire(&root, team, member, wait).map_err(|error| {
            if read_deadline.is_some()
                && matches!(&error, CoordinationError::Conflict(message) if message == "host operation deferred: lock busy")
            {
                "host member busy".into()
            } else {
                error.to_string()
            }
        })?;
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
        let (mut state, recovery_turn) = if matches!(operation, "transcript" | "recover" | "input")
        {
            let host = &mut seat.host;
            match poll_compaction(host, &root, team, member, &guard, operation == "recover") {
                Ok(polled) => polled,
                Err(error) => {
                    if host_connection_closed(&error) {
                        self.activity_hub().publish_host_status(
                            &seat.attachment.socket_path,
                            &seat.host.thread_id,
                            &Value::Null,
                        );
                    }
                    return Err(error);
                }
            }
        } else {
            (Value::Null, false)
        };
        let record = MemberRuntimeStore::load(&root, team, member).map_err(|e| e.to_string())?;
        let operation = if recovery_turn && operation != "input" {
            "recovery_input"
        } else {
            operation
        };
        let recovery_unknown = record.recovery.claim.as_ref().is_some_and(|claim| {
            claim.card_key.context == record.context()
                && claim.stage == ReceiptStage::OutcomeUnknown
        });
        let result = (|| match operation {
            "transcript" | "recover" => Ok(state.clone()),
            "input" | "recovery_input" => {
                if record.host_input_unknown {
                    return Err("outcome_unknown: previous input requires reconciliation".into());
                }
                let text = if operation == "recovery_input" {
                    ""
                } else {
                    params["text"].as_str().ok_or("missing input text")?
                };
                if recovery_unknown {
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
                if operation == "recovery_input" && card.is_none() {
                    return Ok(state.clone());
                }
                let input = match card.as_ref() {
                    Some(card) if text.is_empty() => card.text.clone(),
                    Some(card) => format!("{}\n\n{}", card.text, text),
                    None => text.to_string(),
                };
                // Persist ambiguity before any possible input bytes, including owner crashes.
                MemberRuntimeStore::update(&root, team, member, |r| {
                    r.host_input_unknown = true;
                })
                .map_err(|e| e.to_string())?;
                let result = seat.host.input_checked(&input, &state, &guard);
                if result.is_ok() || !seat.host.outcome_unknown() {
                    MemberRuntimeStore::update(&root, team, member, |r| {
                        r.host_input_unknown = false;
                    })
                    .map_err(|e| e.to_string())?;
                }
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
                        if let Err(error) = record_host_delivery(&root, team, member) {
                            tracing::warn!(team, member, %error, "confirmed recovery bookkeeping failed");
                        }
                    }
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
        })();
        if result
            .as_ref()
            .is_err_and(|error| host_connection_closed(error))
        {
            self.activity_hub().publish_host_status(
                &seat.attachment.socket_path,
                &seat.host.thread_id,
                &Value::Null,
            );
        }
        if matches!(operation, "transcript" | "recover" | "recovery_input") {
            if result.is_err() {
                tracing::debug!(team, member, "host recovery deferred; read preserved");
            } else if operation == "recovery_input" {
                state = seat.host.transcript(&guard).unwrap_or(state);
            }
            state["outcomeUnknown"] = serde_json::json!(
                seat.host.outcome_unknown() || record.host_input_unknown || recovery_unknown
            );
            return Ok(state);
        }
        result
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
            if let Err(error) =
                self.operation(registry, team, member, generation, "recover", Value::Null)
            {
                tracing::debug!(team, member, %error, "host recovery deferred; continuing liveness");
            }
            return Ok(());
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
    ) -> Result<String, String> {
        self.stop_with_tui(registry, team, member, || Ok(()))
    }

    pub fn stop_with_tui(
        &self,
        registry: &TeamRootRegistry,
        team: &str,
        member: &str,
        stop_tui: impl FnOnce() -> Result<(), String>,
    ) -> Result<String, String> {
        let root = registry.resolve(team).map_err(|e| e.to_string())?;
        let cell = self.seat(&root, team, member)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut owned = loop {
            match cell.try_lock() {
                Ok(owned) => break owned,
                Err(std::sync::TryLockError::WouldBlock)
                    if std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => return Err("host member busy; retry".into()),
            }
        };
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
        // Validate before touching the TUI; keep the seat, but never overlap file locks.
        drop(guard);
        stop_tui()?;
        // Reap under the held seat before a competing hook can delay publication.
        // A failed record update leaves the owned, reaped Child available for a
        // retry; after daemon restart host_alive=false also permits repair.
        let exit_status = match owned.as_mut() {
            Some(seat) => seat.host.stop()?,
            None => "already stopped".into(),
        };
        let guard = HostOperationLock::acquire(&root, team, member, Duration::from_secs(2))
            .map_err(|e| format!("host stopped; record update failed: {e}"))?;
        MemberRuntimeStore::update(&root, team, member, |record| {
            record.attachment_generation = record.attachment_generation.saturating_add(1);
            record.health = HealthState::SessionDead;
            if let Some(host) = &mut record.app_server {
                host.state = "stopped".into();
            }
        })
        .map_err(|e| format!("host stopped; record update failed: {e}"))?;
        // Only a Child held by this daemon can be killed, never a record PID.
        drop(owned.take());
        drop(guard);
        Ok(exit_status)
    }
}

/// Run from daemon liveness and UI operations, always after attachment validation.
fn poll_compaction(
    host: &mut HostProcess,
    root: &Path,
    team: &str,
    member: &str,
    guard: &HostOperationLock,
    background: bool,
) -> Result<(Value, bool), String> {
    use super::stores::compaction::{emit_host_compaction, record_host_boundary};
    let emit = |boundary: &Value, event, reason| {
        emit_host_compaction(team, member, boundary, event, reason)
    };
    let state = host.transcript_with_retry(guard, !background);
    // Collapse an unread backlog before admission: only the newest context can be recovered.
    let mut newest = None;
    for boundary in host.take_compactions() {
        if boundary["threadId"] != host.thread_id {
            emit(
                &boundary,
                "compaction.codex_host.skipped",
                Some("different_thread"),
            );
        } else {
            newest = Some(boundary);
        }
    }
    if let Some(boundary) = newest {
        let recorded = record_host_boundary(root, team, member, &boundary, "host_notification")
            .map_err(|e| e.to_string())?;
        let (event, reason) = match recorded {
            true => ("compaction.codex_host.received", None),
            false => ("compaction.codex_host.skipped", Some("already_recorded")),
        };
        emit(&boundary, event, reason);
    }
    let pending = super::stores::MemberCompactionStore::load(root, team, member)
        .map_err(|e| e.to_string())?
        .filter(|s| s.pending && s.last_session_id == host.thread_id);
    let idle = state
        .as_ref()
        .is_ok_and(|s| s["thread"]["status"]["type"] == "idle");
    let ready = idle && state.as_ref().is_ok_and(HostProcess::accepts_input);
    if let Some(pending) = &pending {
        if !ready && host.deferred_compaction != Some(pending.last_compaction_timestamp) {
            host.deferred_compaction = Some(pending.last_compaction_timestamp);
            emit(
                &pending.host_boundary.clone().unwrap_or_default(),
                "compaction.codex_host.deferred",
                Some(if idle {
                    "input_not_ready"
                } else {
                    "thread_not_idle"
                }),
            );
        }
    }
    // No busy polling under exclusion; the next reconciliation/input can service the obligation.
    state.map(|s| (s, ready && pending.is_some()))
}

fn record_host_delivery(root: &Path, team: &str, member: &str) -> Result<(), String> {
    use super::stores::compaction::{
        emit_host_compaction, record_delivery_with_transport_at, CompactionDeliveryResult,
    };
    let runtime = MemberRuntimeStore::load(root, team, member).map_err(|e| e.to_string())?;
    let state = super::stores::MemberCompactionStore::load(root, team, member)
        .map_err(|e| e.to_string())?;
    if let Some(state) = state.filter(|s| {
        s.last_delivery_result == CompactionDeliveryResult::Skipped
            && s.pending_obligation.as_ref().map(|(_, context)| *context) == Some(runtime.context())
    }) {
        record_delivery_with_transport_at(
            root,
            team,
            member,
            runtime.cli_tool.ok_or("host harness identity missing")?,
            &state.last_session_id,
            state.last_compaction_timestamp,
            CompactionDeliveryResult::Injected,
            None,
            Some("host_turn"),
        )
        .map_err(|e| e.to_string())?;
        emit_host_compaction(
            team,
            member,
            &state.host_boundary.unwrap_or_default(),
            "compaction.codex_host.delivered",
            None,
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) fn hold_stop_seat(
        hosts: &super::HostedMembers,
        root: &std::path::Path,
        duration: std::time::Duration,
    ) -> std::thread::JoinHandle<()> {
        let cell = hosts.seat(root, "team", "seat").unwrap();
        let (ready, held) = std::sync::mpsc::channel();
        let holder = std::thread::spawn(move || {
            let _seat = cell.lock().unwrap();
            ready.send(()).unwrap();
            std::thread::sleep(duration);
        });
        held.recv().unwrap();
        holder
    }

    use super::super::compact_hook::{run_compact_hook_cli, tests::write_snapshot_fixture};
    use super::*;
    use crate::coordination::hosted_process::tests::fixture;
    use crate::coordination::recovery_delivery::{observe, prepare, read_current};
    use crate::coordination::stores::compaction::{record_host_boundary, MemberCompactionState};
    use crate::coordination::stores::CompactionDeliveryResult::Injected;
    use crate::coordination::stores::{MemberCompactionStore, MemberRuntimeRecord};
    use serde_json::json;
    use taurhaus_lib::logging::{install_global_sink, LogFileState};

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

    #[test]
    fn hosted_refresh_does_not_poll_another_fixture() {
        // Regression: 1b19edd2 registered all test seats in the shared hub,
        // so refreshing one fixture consumed another fixture's queued RPCs.
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let (_, hosts) = running(first.path());
        let (_, other) = running(second.path());
        let before = fixture_text(first.path(), "requests.jsonl");
        let untouched = fixture_text(second.path(), "requests.jsonl");
        let hub = hosts.activity_hub();
        hub.refresh_hosts();
        assert_ne!(fixture_text(first.path(), "requests.jsonl"), before);
        assert_eq!(fixture_text(second.path(), "requests.jsonl"), untouched);
        drop(hosts);
        assert!(hub.runtime_snapshot().runtime_sessions.is_empty());
        assert_eq!(
            other
                .activity_hub()
                .runtime_snapshot()
                .runtime_sessions
                .len(),
            1
        );
    }

    #[test]
    fn hosted_stop_reaps_before_contended_record_update() {
        // Regression: 73a42755 reacquired the host lock after destroying the TUI,
        // so a concurrent hook could prevent the owned child from being reaped.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (registry, hosts) = running(root);
        let before = saved(root);
        let attachment = before.app_server.as_ref().unwrap();
        let result = std::thread::scope(|scope| {
            let (acquire, requested) = std::sync::mpsc::channel();
            let (ready, held) = std::sync::mpsc::channel();
            let (release, finished) = std::sync::mpsc::channel::<()>();
            scope.spawn(move || {
                requested.recv().unwrap();
                let _guard =
                    HostOperationLock::acquire(root, "team", "seat", Duration::from_secs(2))
                        .unwrap();
                ready.send(()).unwrap();
                // Channel closure also releases the holder if the caller panics.
                let _ = finished.recv();
            });
            let result = hosts.stop_with_tui(&registry, "team", "seat", || {
                acquire.send(()).unwrap();
                held.recv().unwrap();
                Ok(())
            });
            drop(release);
            result
        });
        assert!(!host_alive(attachment), "owned host survived TUI stop");
        let error = result.unwrap_err();
        assert!(
            error.starts_with("host stopped; record update failed:"),
            "{error}"
        );
        assert_eq!(saved(root), before, "failed publication remains retryable");
        hosts.stop(&registry, "team", "seat").unwrap();
        let after = saved(root);
        assert_eq!(after.app_server.unwrap().state, "stopped");
        assert_eq!(
            after.attachment_generation,
            before.attachment_generation + 1
        );
    }

    #[test]
    fn hosted_read_busy_waits_for_background_activity() {
        // Regression: 3000bc3e (#161) background refresh exposed operation()'s immediate busy refusal.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (registry, hosts) = running(root);
        let generation = saved(root).attachment_generation;
        transcript(&hosts, &registry, generation);
        for operation in ["transcript", "recover"] {
            let cell = hosts.seat(root, "team", "seat").unwrap();
            let (held, ready) = std::sync::mpsc::channel();
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    let mut owned = cell.lock().unwrap();
                    let guard =
                        HostOperationLock::acquire_for_activity(root, "team", "seat").unwrap();
                    held.send(()).unwrap();
                    std::thread::sleep(Duration::from_millis(100));
                    owned
                        .as_mut()
                        .unwrap()
                        .host
                        .refresh_activity(&guard)
                        .unwrap();
                });
                ready.recv_timeout(Duration::from_secs(1)).unwrap();
                let result = hosts.operation(
                    &registry,
                    "team",
                    "seat",
                    generation,
                    operation,
                    Value::Null,
                );
                assert!(result.is_ok(), "{operation}: {result:?}");
                // Success proves the contended read waited; scheduler delay is not failure.
            });
        }
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_read_busy_bounds_contention_without_queueing_mutations() {
        // Regression: 3000bc3e (#161) refused cell reads instantly but allowed a 2s file-lock wait.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let registry = seat(root);
        let hosts = HostedMembers::default();
        let launch = fixture(root);
        let script = std::fs::read_to_string(&launch.program).unwrap().replace(
            "                elif method == 'turn/interrupt':",
            "                elif method == 'turn/interrupt':\n                    open(os.path.join(root, 'interrupt-held'), 'w').close(); time.sleep(4)",
        );
        std::fs::write(&launch.program, script).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let generation = saved(root).attachment_generation;
        input(&hosts, &registry, generation, "active").unwrap();
        transcript(&hosts, &registry, generation);
        let run = |operation| {
            hosts.operation(
                &registry,
                "team",
                "seat",
                generation,
                operation,
                Value::Null,
            )
        };
        for hold_cell in [true, false] {
            let (held, ready) = std::sync::mpsc::channel();
            let (release, released) = std::sync::mpsc::channel();
            std::thread::scope(|scope| {
                scope.spawn(move || {
                    if hold_cell {
                        run("interrupt").unwrap();
                    } else {
                        let _guard =
                            HostOperationLock::acquire(root, "team", "seat", Duration::ZERO)
                                .unwrap();
                        held.send(()).unwrap();
                        let _ = released.recv_timeout(Duration::from_secs(10));
                    }
                });
                if hold_cell {
                    let deadline = std::time::Instant::now() + Duration::from_secs(1);
                    while !root.join("interrupt-held").exists() {
                        assert!(std::time::Instant::now() < deadline);
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    for operation in ["input", "interrupt", "approval"] {
                        let started = std::time::Instant::now();
                        assert_eq!(run(operation).unwrap_err(), "host member busy");
                        assert!(started.elapsed() < Duration::from_millis(100));
                    }
                } else {
                    ready.recv_timeout(Duration::from_secs(1)).unwrap();
                }
                for operation in ["transcript", "recover"] {
                    let started = std::time::Instant::now();
                    assert_eq!(run(operation).unwrap_err(), "host member busy");
                    let elapsed = started.elapsed();
                    if !hold_cell && operation == "recover" {
                        // `recover` runs under the team orchestrator from the live-presence
                        // reconcile and never queues on the cross-process lock.
                        assert!(
                            elapsed < Duration::from_millis(100),
                            "{operation}: {elapsed:?}"
                        );
                    } else {
                        // The read budget (cell) or the 2 s file-lock wait must have elapsed;
                        // no upper bound — scheduler jitter under parallel lanes is not a defect.
                        assert!(
                            elapsed >= Duration::from_millis(1450),
                            "{operation}: {elapsed:?}"
                        );
                    }
                }
                if !hold_cell {
                    release.send(()).unwrap();
                }
            });
        }
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_launch_retry_succeeds_once() {
        launch_retry_case("once");
    }

    #[test]
    fn hosted_launch_retry_stops_after_two_exits() {
        launch_retry_case("twice");
    }

    #[test]
    fn hosted_launch_retry_excludes_readiness_timeout() {
        launch_retry_case("timeout");
    }

    fn launch_retry_case(mode: &str) {
        // Regression: cadd533e failed fresh-account initialization races without a bounded retry.
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let registry = seat(root);
        let before: Value =
            serde_json::from_str(&fixture_text(root, "team/runtime/seat.json")).unwrap();
        let hosts = HostedMembers::default();
        let launch = fixture(root);
        let sink = LogFileState::new(root.join("events.jsonl")).unwrap();
        install_global_sink(&sink);
        std::fs::write(root.join("mode"), mode).unwrap();
        let script = std::fs::read_to_string(&launch.program).unwrap().replace(
            "root = os.environ['CODEX_HOME']",
            r#"root = os.environ['CODEX_HOME']
mode = open(os.path.join(root, 'mode')).read()
starts = os.path.join(root, 'starts.jsonl')
previous = [json.loads(line) for line in open(starts)] if os.path.exists(starts) else []
address = sys.argv[sys.argv.index('--listen')+1].removeprefix('unix://')
with open(starts, 'a') as out: out.write(json.dumps({'pid':os.getpid(), 'socket':address, 'time':time.monotonic(), 'argv':sys.argv, 'runtime':json.load(open(os.path.join(root, 'team/runtime/seat.json')))})+'\n')
if previous:
    assert not os.path.exists('/proc/'+str(previous[0]['pid'])), 'first child must be reaped'
    assert previous[0]['socket'] != address and not os.path.exists(previous[0]['socket'])
    assert time.monotonic() - previous[0]['time'] >= 1.5
    assert previous[0]['argv'][:-1] == sys.argv[:-1], 'strict config must be identical'
if mode == 'timeout':
    sys.stderr.write('waiting\n'); sys.stderr.flush()
    time.sleep(60)
if mode == 'twice' or not previous:
    with socket.socket(socket.AF_UNIX) as stale: stale.bind(address)
    sys.stderr.write('\x1b[31mfailed sqlite initialization\x1b[0m sk-fake-secret\n'); sys.stderr.flush()
    sys.exit(1)"#,
        );
        std::fs::write(&launch.program, script).unwrap();
        // A live child is judged by the launch deadline; shorten it so the timeout case
        // takes seconds, not the production 30 s budget (the child itself sleeps 60 s).
        if mode == "timeout" {
            crate::coordination::stores::lock::LAUNCH_DEADLINE_OVERRIDE
                .with(|d| d.set(Some(Duration::from_millis(2500))));
        }
        let result = hosts.launch(&registry, "team", "seat", &launch);
        crate::coordination::stores::lock::LAUNCH_DEADLINE_OVERRIDE.with(|d| d.set(None));
        let starts: Vec<Value> = std::fs::read_to_string(root.join("starts.jsonl"))
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(starts.len(), if mode == "timeout" { 1 } else { 2 });
        if mode == "once" {
            result.unwrap();
            assert_eq!(saved(root).session_id.as_deref(), Some("owned-thread"));
        } else {
            let error = result.unwrap_err();
            assert!(
                error.contains(if mode == "twice" {
                    "(exit 1): failed sqlite initialization [redacted]"
                } else {
                    "deadline expired"
                }),
                "{error}"
            );
        }
        let record = saved(root);
        assert_eq!(record.attachment_generation, u64::from(mode == "once"));
        assert_eq!(record.recovery.claim.is_some(), mode == "once");
        assert_eq!(
            MemberRuntimeStore::list(root, "team").unwrap(),
            vec!["seat"]
        );
        let requests = std::fs::read_to_string(root.join("requests.jsonl")).unwrap_or_default();
        assert_eq!(
            requests.matches("\"method\": \"thread/start\"").count(),
            usize::from(mode == "once")
        );
        drop(hosts);
        for start in &starts {
            assert_eq!(
                start["runtime"], before,
                "failed attempt must not publish runtime or receipt"
            );
            assert!(taurhaus_lib::platform::process_start_ticks(
                start["pid"].as_u64().unwrap() as u32
            )
            .is_none());
        }
        sink.flush_for_test().unwrap();
        let logs = std::fs::read_to_string(root.join("events.jsonl")).unwrap();
        assert!(!logs.contains("sk-fake-secret"));
        let events: Vec<Value> = logs
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        let count = |name| events.iter().filter(|e| e["event"] == name).count();
        assert_eq!(
            count("hosted.launch.retried"),
            usize::from(mode != "timeout")
        );
        for event in events
            .iter()
            .filter(|e| e["event"].as_str().unwrap().starts_with("hosted.launch."))
        {
            assert_eq!(event["level"], "WARN");
            assert_eq!(event["team"], "team");
            assert_eq!(event["member"], "seat");
            if mode != "timeout" {
                assert_eq!(event["attempt"], 2);
                assert_eq!(event["exit_status"], "1");
                assert_eq!(
                    event["stderr_tail"],
                    "failed sqlite initialization [redacted]"
                );
            }
        }
        assert_eq!(count("hosted.launch.failed"), usize::from(mode == "twice"));
        assert_eq!(
            count("hosted.launch.timed_out"),
            usize::from(mode == "timeout")
        );
    }

    #[test]
    fn hosted_activity_regression_recovers_after_mid_frame_deadline() {
        // Regression: 011ca88c imposed 250ms reads but reused a partially consumed frame.
        let tmp = tempfile::tempdir().unwrap();
        let registry = seat(tmp.path());
        let hosts = HostedMembers::default();
        let launch = fixture(tmp.path());
        let script = std::fs::read_to_string(&launch.program).unwrap().replace(
            "                emit(reply)",
            "                marker = os.path.join(root, 'split-frame')\n                if method == 'thread/read' and os.path.exists(marker):\n                    os.unlink(marker)\n                    payload = json.dumps(reply).encode()\n                    stream.write(b'\\x81\\x7e'+struct.pack('!H', len(payload))+payload[:1]); stream.flush()\n                    import time; time.sleep(0.4)\n                    stream.write(payload[1:]); stream.flush()\n                    continue\n                emit(reply)",
        );
        let script = script.replace(
            "                elif method in ('thread/resume', 'thread/read'):",
            "                elif method in ('thread/resume', 'thread/read'):\n                    if method == 'thread/resume' and os.path.exists(os.path.join(root, 'slow-resume')):\n                        import time; time.sleep(0.4)",
        );
        std::fs::write(&launch.program, script).unwrap();
        hosts.launch(&registry, "team", "seat", &launch).unwrap();
        let generation = saved(tmp.path()).attachment_generation;
        let hub = hosts.activity_hub();
        let source = || {
            hub.runtime_snapshot()
                .runtime_sessions
                .into_iter()
                .find(|s| s.project_path == tmp.path().to_str().unwrap())
                .unwrap()
                .source
        };
        input(&hosts, &registry, generation, "active").unwrap();
        transcript(&hosts, &registry, generation);
        std::fs::write(tmp.path().join("split-frame"), "").unwrap();
        hub.refresh_hosts();
        let after_timeout = source();
        // Regression: 4ad65497 retried background reconnect under the failed 250ms budget.
        std::fs::write(tmp.path().join("slow-resume"), "").unwrap();
        hub.refresh_hosts();
        assert_eq!(after_timeout.as_deref(), Some("host_unavailable"));
        assert_eq!(source().as_deref(), Some("host"));
        hosts
            .operation(
                &registry,
                "team",
                "seat",
                generation,
                "interrupt",
                Value::Null,
            )
            .unwrap();
        input(&hosts, &registry, generation, "after timeout").unwrap();
        assert!(transcript(&hosts, &registry, generation)
            .to_string()
            .contains("after timeout"));
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_activity_reconnect_failures_back_off() {
        // Regression: 4ad65497 resumed a failed connection on every background tick.
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        assert!(input(&hosts, &registry, generation, "disconnect").is_err());
        // The fake host uses this marker to refuse initialize on new connections.
        std::fs::write(tmp.path().join("fail-reconnect"), "").unwrap();
        let hub = hosts.activity_hub();
        let attempts = || {
            std::fs::read_to_string(tmp.path().join("requests.jsonl"))
                .unwrap()
                .lines()
                .filter(|line| line.contains("initialize"))
                .count()
        };
        let before = attempts();
        // Regression: 4ad65497's sleep-based retry count depended on scheduler
        // delays. Drive the one- then two-second backoffs with fixture time.
        let now = std::time::Instant::now();
        let tick = |millis| {
            hosts
                .seat(tmp.path(), "team", "seat")
                .unwrap()
                .lock()
                .unwrap()
                .as_mut()
                .unwrap()
                .host
                .activity_clock_for_test = Some(now + Duration::from_millis(millis));
            hub.refresh_hosts();
        };
        tick(0);
        assert_eq!(attempts(), before + 1);
        for millis in [100, 500, 999] {
            tick(millis);
            assert_eq!(attempts(), before + 1);
        }
        tick(1000);
        assert_eq!(attempts(), before + 2);
        tick(2999);
        assert_eq!(attempts(), before + 2);
        tick(3000);
        assert_eq!(attempts(), before + 3);
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_activity_reconnect_flags_lost_approval() {
        // Regression: 4ad65497 silently discarded connection-scoped approval IDs.
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        input(&hosts, &registry, generation, "approval").unwrap();
        let pending = transcript(&hosts, &registry, generation);
        assert!(!pending["requests"].as_array().unwrap().is_empty());
        // Drop the owned client transport without killing the host or its pending turn.
        hosts
            .seats
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .host
            .disconnect_for_test();
        hosts.activity_hub().refresh_hosts();
        let recovered = transcript(&hosts, &registry, generation);
        assert!(
            recovered["outcomeUnknown"] == true
                || !recovered["requests"].as_array().unwrap().is_empty(),
            "lost approval must remain answerable or explicitly uncertain"
        );
        hosts.stop(&registry, "team", "seat").unwrap();
    }

    #[test]
    fn hosted_probe_preserves_authority_while_thread_read_is_pending() {
        // Regression: 1b19edd2 blocked the global scanner for 5s and called pending a disconnect.
        let tmp = tempfile::tempdir().unwrap();
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        input(&hosts, &registry, generation, "active").unwrap();
        std::fs::write(tmp.path().join("pending-read"), "").unwrap();
        let started = std::time::Instant::now();
        hosts.activity_hub().refresh_hosts();
        let elapsed = started.elapsed();
        let row = hosts
            .activity_hub()
            .runtime_snapshot()
            .runtime_sessions
            .into_iter()
            .find(|s| s.project_path == tmp.path().to_str().unwrap())
            .unwrap();
        assert_eq!(row.source.as_deref(), Some("host"));
        assert!(elapsed < Duration::from_secs(1), "probe took {elapsed:?}");
    }

    #[test]
    fn hosted_activity_tracks_turns_waits_disconnect_and_teardown() {
        // Regression: 6f61f611 kept owned thread activity private to the host client.
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let sink =
            taurhaus_lib::logging::LogFileState::new(tmp.path().join("events.jsonl")).unwrap();
        taurhaus_lib::logging::install_global_sink(&sink);
        let (registry, hosts) = running(tmp.path());
        let generation = saved(tmp.path()).attachment_generation;
        let hub = hosts.activity_hub();
        let op =
            |method, params| hosts.operation(&registry, "team", "seat", generation, method, params);
        let snapshot = || {
            let rows = hub.runtime_snapshot().runtime_sessions;
            serde_json::to_value(
                rows.iter()
                    .find(|s| s.project_path == tmp.path().to_str().unwrap())
                    .expect("host must publish its session identity"),
            )
            .unwrap()
        };
        transcript(&hosts, &registry, generation);
        assert_eq!(snapshot()["source"], "host");
        assert_eq!(snapshot()["session_id"], "owned-thread");
        assert_eq!(snapshot()["group_label"], "team");
        assert_eq!(snapshot()["member_name"], "seat");
        let socket = saved(tmp.path()).app_server.unwrap().socket_path;
        let hidden_socket = socket.with_extension("hidden");
        std::fs::rename(&socket, &hidden_socket).unwrap();
        hub.refresh_hosts();
        assert_eq!(snapshot()["source"], "host_unavailable");
        std::fs::rename(&hidden_socket, &socket).unwrap();
        hub.refresh_hosts();
        assert_eq!(snapshot()["source"], "host");
        let version = hub.snapshot().version;
        input(&hosts, &registry, generation, "active").unwrap();
        assert!(hub.wait_for_update(version, 0, Duration::ZERO).changed);
        let sessions = tmp.path().join("sessions").join("2020/01/01");
        std::fs::create_dir_all(&sessions).unwrap();
        let transcript_path = sessions.join("rollout-fixture-owned-thread.jsonl");
        std::fs::write(
            &transcript_path,
            json!({"type":"session_meta","payload":{"id":"owned-thread","cwd":tmp.path()}})
                .to_string(),
        )
        .unwrap();
        let process = taurhaus_lib::session_scanner::process::ProcessInfo {
            pid: 941_091,
            project_path: tmp.path().to_string_lossy().into_owned(),
            tty: "/dev/pts/fake".into(),
            args: saved(tmp.path()).app_server.unwrap().attach_argv.join(" "),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Codex,
        };
        let _hub_scope = SessionActivityHub::scoped_for_test(hub.clone());
        let resolved = crate::session_scanner::cli_tool::spec(process.cli_tool)
            .session_source()
            .process_session(&process, Some("%fixture"))
            .unwrap();
        assert_eq!(resolved.session_id.as_deref(), Some("owned-thread"));
        assert_eq!(resolved.jsonl_path.as_deref(), transcript_path.to_str());
        assert_eq!(resolved.tmux_pane.as_deref(), Some("%fixture"));
        assert_eq!(resolved.pid, process.pid);
        let roster = super::super::roster::get_team_roster_with_runtime_sessions(
            tmp.path(),
            "team",
            &[resolved],
        )
        .unwrap();
        assert_eq!(roster[0].host_activity.as_ref().unwrap().state, "working");
        assert_eq!(snapshot()["state"], "active");
        assert_eq!(snapshot()["activity_attribution"], "attributed");
        assert_eq!(snapshot()["activity_confidence"], "high");
        op("interrupt", Value::Null).unwrap();
        assert_eq!(snapshot()["state"], "idle");
        input(&hosts, &registry, generation, "approval").unwrap();
        hub.refresh_hosts();
        assert_eq!(snapshot()["state"], "active");
        assert_eq!(snapshot()["activity_attribution"], "none");
        assert_eq!(snapshot()["activity_confidence"], "high");
        op(
            "approval",
            json!({"requestId":"permission-1","accept":false}),
        )
        .unwrap();
        op("interrupt", Value::Null).unwrap();
        assert!(input(&hosts, &registry, generation, "disconnect").is_err());
        // Disconnect invalidates immediately; the next probe may reconnect (covered above).
        assert_eq!(snapshot()["source"], "host_unavailable");
        assert_eq!(snapshot()["activity_confidence"], "low");
        hosts.stop(&registry, "team", "seat").unwrap();
        assert!(!hub
            .runtime_snapshot()
            .runtime_sessions
            .iter()
            .any(|s| s.project_path == tmp.path().to_str().unwrap()));
        sink.flush_for_test().unwrap();
        let edges: Vec<Value> = std::fs::read_to_string(tmp.path().join("events.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["event"] == "activity.state.changed" && row["pid"] == process.pid)
            .map(|row| json!([row["from"], row["to"], row["source"]]))
            .collect();
        assert_eq!(
            json!(edges),
            json!([
                ["working", "idle", "host"],
                ["idle", "working", "host"],
                ["working", "active", "host"],
                ["active", "working", "host"],
                ["working", "idle", "host"],
                ["idle", "uncertain", "host_unavailable"]
            ])
        );
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
    fn fixture_text(root: &Path, file: &str) -> String {
        std::fs::read_to_string(root.join(file)).unwrap()
    }
    fn starts(root: &Path) -> usize {
        let requests = fixture_text(root, "requests.jsonl");
        requests.matches("turn/start").count()
    }
    fn has_compaction(view: &Value) -> bool {
        let turns = view["thread"]["turns"].as_array().unwrap();
        turns
            .iter()
            .any(|turn| turn["items"][0]["type"] == "contextCompaction")
    }
    fn compaction(root: &Path) -> MemberCompactionState {
        let state = MemberCompactionStore::load(root, "team", "seat").unwrap();
        state.unwrap()
    }
    #[test]
    fn hosted_compaction_reads_survive_unknown_and_exhausted_recovery() {
        // Regression: 8fab0c8d, attempt-8 continuation: recovery errors hid the transcript.
        for failure in [
            "input_unknown",
            "claim_unknown",
            "exhausted",
            "blocked",
            "requests",
            "bookkeeping",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path();
            let (reg, hosts) = running(root);
            let gen = saved(root).attachment_generation;
            transcript(&hosts, &reg, gen);
            // Regression: 8fab0c8d propagated transient recovery errors into liveness.
            if failure == "input_unknown" {
                let cell = hosts.seat(root, "team", "seat").unwrap();
                let held = cell.lock().unwrap();
                assert!(hosts.reconcile(&reg, "team", "seat").is_ok());
                drop(held);
                let held =
                    std::fs::File::open(root.join("team/state/app-server/seat.lock")).unwrap();
                fs2::FileExt::lock_exclusive(&held).unwrap();
                assert!(hosts.reconcile(&reg, "team", "seat").is_ok());
                drop(held);
                std::fs::write(root.join("pending-read"), "").unwrap();
                let started = std::time::Instant::now();
                assert!(hosts.reconcile(&reg, "team", "seat").is_ok());
                std::fs::remove_file(root.join("pending-read")).unwrap();
                assert!(started.elapsed() < Duration::from_secs(1));
            }
            std::fs::write(root.join("compact.json"), r#"{"busy":true}"#).unwrap();
            hosts.reconcile(&reg, "team", "seat").unwrap();
            if failure == "input_unknown" {
                MemberRuntimeStore::update(root, "team", "seat", |r| r.host_input_unknown = true)
                    .unwrap();
            } else if matches!(failure, "claim_unknown" | "exhausted") {
                for _ in 0..2 {
                    let card = prepare(&reg, root, "team", "seat", "app_server")
                        .unwrap()
                        .unwrap();
                    if failure == "claim_unknown" {
                        break;
                    }
                    let stage = ReceiptStage::Failed;
                    observe(&reg, root, "team", "seat", &card.receipt, stage).unwrap();
                }
            }
            // Regression: d7aba13a / attempt-8 continuation: unattended pre-send refusals spent both attempts.
            let readiness = matches!(failure, "blocked" | "requests");
            if readiness {
                std::fs::write(root.join("refuse-input"), failure).unwrap();
            }
            let claim = saved(root).recovery.claim;
            std::fs::write(root.join("compact.json"), "{}").unwrap();
            let before = starts(root);
            if failure == "bookkeeping" {
                // Regression: 8fab0c8d left confirmed recovery ambiguous after bookkeeping failed.
                MemberRuntimeStore::update(root, "team", "seat", |r| r.cli_tool = None).unwrap();
                assert!(input(&hosts, &reg, gen, "confirmed").is_ok());
                assert_eq!(transcript(&hosts, &reg, gen)["outcomeUnknown"], false);
                assert!(input(&hosts, &reg, gen, "next input").is_ok());
                continue;
            }
            for operation in ["transcript", "recover", "transcript"] {
                let result = hosts.operation(&reg, "team", "seat", gen, operation, Value::Null);
                let view = result.unwrap_or_else(|error| panic!("{failure}/{operation}: {error}"));
                assert_eq!(view["outcomeUnknown"], failure.ends_with("unknown"));
                assert!(has_compaction(&view));
            }
            assert_eq!(before, starts(root));
            assert!(compaction(root).pending);
            if readiness {
                assert_eq!(
                    saved(root).recovery.claim,
                    claim,
                    "{failure} spent an attempt"
                );
                std::fs::write(root.join("refuse-input"), "").unwrap();
                hosts
                    .operation(
                        &reg,
                        "team",
                        "seat",
                        gen,
                        "approval",
                        json!({"requestId":"lingering","accept":true}),
                    )
                    .ok();
                transcript(&hosts, &reg, gen);
                assert_eq!(starts(root), before + 1, "{failure}");
                assert_eq!(saved(root).recovery.last_delivered.unwrap().attempt, 1);
                assert!(!compaction(root).pending);
            }
            let mut config = TeamConfigStore::load(root, "team").unwrap();
            config.members[0].extra.remove("adapter_mode");
            TeamConfigStore::save(root, "team", &config).unwrap();
            assert!(hosts.reconcile(&reg, "team", "seat").is_ok());
        }
    }
    #[test]
    fn hosted_recovery_first_input_uses_existing_pending_compaction_without_new_generation() {
        // Regression: 6f61f611, attempt-8 continuation: busy compaction must persist for first input.
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let sink = LogFileState::new(root.join("events.jsonl")).unwrap();
        install_global_sink(&sink);
        let (registry, hosts) = running(tmp.path());
        write_snapshot_fixture(tmp.path(), "team", "seat");
        let generation = saved(tmp.path()).attachment_generation;
        input(&hosts, &registry, generation, "startup marker").unwrap();
        let boundary = r#"{"busy":true,"turnId":"deferred-test"}"#;
        std::fs::write(root.join("compact.json"), boundary).unwrap();
        // Regression: 8fab0c8d held host exclusion for five seconds after a busy boundary.
        let started = std::time::Instant::now();
        hosts.reconcile(&registry, "team", "seat").unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        // Regression: 8fab0c8d logged the attempt-8 busy boundary on every panel poll.
        for _ in 0..3 {
            hosts.reconcile(&registry, "team", "seat").unwrap();
        }
        sink.flush_for_test().unwrap();
        let events = fixture_text(root, "events.jsonl");
        let deferred = events
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|e| {
                e["event"] == "compaction.codex_host.deferred" && e["turn_id"] == "deferred-test"
            })
            .count();
        assert_eq!(deferred, 1);
        assert!(compaction(root).pending);
        std::fs::write(root.join("compact.json"), r#"{"turnId":"deferred-test"}"#).unwrap();
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
        assert!(!compaction(root).pending);
        hosts.stop(&registry, "team", "seat").unwrap();
    }
    #[test]
    fn hosted_notification_compaction_delivers_once_and_deduplicates_hook() {
        // Regression: 6f61f611 / attempt-8 continuation step5-boundary-events.json: owned-only recovery.
        let _logs = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let sink = LogFileState::new(root.join("events.jsonl")).unwrap();
        install_global_sink(&sink);
        let (reg, hosts) = running(root);
        let gen = saved(root).attachment_generation;
        transcript(&hosts, &reg, gen);
        std::fs::write(root.join("compact.json"), r#"{"threadId":"foreign"}"#).unwrap();
        hosts.reconcile(&reg, "team", "seat").unwrap();
        assert_eq!(saved(root).context_generation, 0);
        assert!(MemberCompactionStore::load(root, "team", "seat")
            .unwrap()
            .is_none());
        // Regression: 06031992 used a missing rollout's wall clock for the hook,
        // leaving only one second of the two-second observer correlation window.
        // Replay an old boundary so scheduler speed cannot make this test pass.
        let boundary = r#"{"expectCard":true,"backlog":65,"completedAtMs":1767225600000}"#;
        std::fs::write(root.join("compact.json"), boundary).unwrap();
        hosts.reconcile(&reg, "team", "seat").unwrap();
        let record = saved(root);
        assert_eq!(record.context_generation, 1);
        let submitted = fixture_text(root, "boundary-submission.json");
        let submitted: Value = serde_json::from_str(&submitted).unwrap();
        assert_eq!(submitted["state"]["source"], "host_notification");
        let receipt = record.recovery.last_delivered.unwrap();
        assert_eq!(receipt.stage, ReceiptStage::Submitted);
        let (card, _) = read_current(&reg, root, "team", "seat").unwrap();
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
        let state = compaction(root);
        assert!(!state.pending);
        assert_eq!(state.last_delivery_result, Injected);
        // Regression: 8fab0c8d fabricated hook IDs absent in attempt-8's documented envelope.
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"owned-thread","cwd":root,"transcript_path":root.join("rollout-owned-thread.jsonl")});
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, root).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("recovery_card"));
        assert_eq!(saved(root).context_generation, 1);
        let view = transcript(&hosts, &reg, gen);
        assert!(has_compaction(&view));
        assert_eq!(starts(root), 2);
        // Regression: 8fab0c8d re-counted stale deliveries; assert locally, not on the global sink.
        record_host_delivery(root, "team", "seat").unwrap();
        assert_eq!(compaction(root), state);
        sink.flush_for_test().unwrap();
        let events = fixture_text(root, "events.jsonl");
        assert!(!events.contains("\"reason\":\"\""));
        for event in [
            "compaction.codex_host.received",
            "compaction.codex_host.delivered",
            "compaction.codex_host.skipped",
            "already_recorded",
            "different_thread",
            "host_turn",
        ] {
            assert!(events.contains(event), "missing {event}");
        }
    }
    #[test]
    fn hosted_notification_compaction_deduplicates_an_earlier_hook() {
        // Regression: 6f61f611, attempt-8 continuation boundary: either observer may arrive first.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let (reg, hosts) = running(root);
        let gen = saved(root).attachment_generation;
        transcript(&hosts, &reg, gen);
        // Regression: 8fab0c8d fabricated hook IDs absent in attempt-8's documented envelope.
        let payload = json!({"hook_event_name":"SessionStart","source":"compact","session_id":"owned-thread","cwd":root,"transcript_path":root.join("rollout-owned-thread.jsonl")});
        let compacted = json!({"type":"compacted","timestamp":chrono::Utc::now(),"payload":{}});
        std::fs::write(
            root.join("rollout-owned-thread.jsonl"),
            compacted.to_string(),
        )
        .unwrap();
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, root).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("recovery_card"));
        // Both observers describe this same persisted event, regardless of scheduling.
        let boundary =
            json!({"completedAtMs":compaction(root).last_compaction_timestamp.timestamp_millis()});
        std::fs::write(root.join("compact.json"), boundary.to_string()).unwrap();
        hosts.reconcile(&reg, "team", "seat").unwrap();
        assert_eq!(saved(root).context_generation, 1);
        assert_eq!(starts(root), 1);
        // Regression: 8fab0c8d's timestamp fallback could merge distinct known boundaries.
        let state = compaction(root);
        let guard = HostOperationLock::acquire(root, "team", "seat", Duration::ZERO).unwrap();
        let next = json!({"threadId":"owned-thread","turnId":"next","itemId":"next",
            "completedAtMs":state.last_compaction_timestamp.timestamp_millis()});
        assert!(record_host_boundary(root, "team", "seat", &next, "host_notification").unwrap());
        assert_eq!(saved(root).context_generation, 2);
        // Regression: 06b1510c merged later ID-less boundaries and legacy records for 30 seconds.
        for (source, delay) in [(Some("host_notification"), 5000), (None, 1000)] {
            let mut previous = compaction(root);
            previous.source = source.map(str::to_owned);
            MemberCompactionStore::save(root, "team", "seat", &previous).unwrap();
            let later = json!({"threadId":"owned-thread", "completedAtMs":previous.last_compaction_timestamp.timestamp_millis()+delay});
            assert!(record_host_boundary(root, "team", "seat", &later, "hook").unwrap());
        }
        assert_eq!(saved(root).context_generation, 4);
        drop(guard);
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
