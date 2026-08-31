use chrono::Utc;

use crate::coordination::audit::{
    AuditEvent, DeliveryAttemptedEvent, DeliveryFailedEvent, DeliverySucceededEvent,
    LeaseClaimedEvent, LeaseReclaimedEvent,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::operational_context::apply_delivery_context;
use crate::coordination::requests::{
    DeliveryMethod, DeliveryRequest, DeliveryResult, WakeDisposition,
};
use crate::coordination::runtime::{
    pane_belongs_to_member, quarantine_foreign_member, PaneOwnership,
};
use crate::coordination::stores::MemberRuntimeStore;
use crate::session_scanner::cli_tool::spec;

use super::audit_logging::emit_audit_event_to_structured_log;
use super::helpers::{
    default_method_for_backend, delivery_meta, delivery_operational_context, delivery_type_name,
};
use super::CoordinationOrchestrator;

impl CoordinationOrchestrator {
    /// Drain buffered audit events and clear the in-memory log.
    pub fn drain_audit_log(&mut self) -> Vec<AuditEvent> {
        std::mem::take(&mut self.audit_log)
    }

    /// Flush buffered audit events to tracing and clear the in-memory buffer.
    pub fn flush_audit_to_log(&mut self) {
        for event in self.audit_log.drain(..) {
            let event_type = event.event_type();
            let json = serde_json::to_string(&event)
                .unwrap_or_else(|err| format!("{{\"error\":\"{err}\"}}"));
            tracing::info!(target: "coordination_audit", event_type, "{json}");
            emit_audit_event_to_structured_log(&event);
        }
    }

    /// Route a delivery request through the backend and emit audit events.
    pub fn deliver_message(
        &mut self,
        request: DeliveryRequest,
    ) -> Result<DeliveryResult, CoordinationError> {
        let (team_name, member_name) = delivery_meta(&request);
        let operational_context = delivery_operational_context(&request).cloned();
        let delivery_type = delivery_type_name(&request).to_string();
        let team_name_owned = team_name.to_string();
        let member_name_owned = member_name.to_string();

        crate::coordination::validation::validate_team_name(team_name)?;
        crate::coordination::validation::validate_member_name(member_name)?;

        let config =
            crate::coordination::stores::TeamConfigStore::load(&self.teams_dir, team_name)?;
        if !config
            .members
            .iter()
            .any(|member| member.name == member_name)
        {
            return Err(CoordinationError::NotFound(format!(
                "member '{member_name}' not found in team '{team_name}'"
            )));
        }

        let member_cli_tool = config
            .members
            .iter()
            .find(|member| member.name == member_name)
            .map(|member| member.cli_tool)
            .expect("validated member must exist");
        let effective_backend = match &self.claude_backend {
            Some(claude_be) => {
                if spec(member_cli_tool).capabilities.native_inbox_poller {
                    claude_be.clone()
                } else {
                    self.backend.clone()
                }
            }
            None => self.backend.clone(),
        };
        let attempted_method = default_method_for_backend(effective_backend.kind());

        self.audit_log
            .push(AuditEvent::DeliveryAttempted(DeliveryAttemptedEvent {
                team_name: team_name_owned.clone(),
                member_name: member_name_owned.clone(),
                delivery_type: delivery_type.clone(),
                method: attempted_method,
                attempted_at: Utc::now(),
            }));

        match effective_backend.deliver(request) {
            Ok(mut result) => {
                if !result.delivered {
                    let error = CoordinationError::Backend(format!(
                        "backend reported undelivered {delivery_type} for '{member_name_owned}' in team '{team_name_owned}'"
                    ));
                    self.audit_log
                        .push(AuditEvent::DeliveryFailed(DeliveryFailedEvent {
                            team_name: team_name_owned,
                            member_name: member_name_owned,
                            delivery_type,
                            error: error.to_string(),
                            failed_at: Utc::now(),
                        }));
                    return Err(error);
                }

                let wake = if result.method != DeliveryMethod::InboxFile {
                    WakeDisposition::NotAttempted {
                        reason: "delivery method does not require an inbox wake".to_string(),
                    }
                } else if spec(member_cli_tool).capabilities.native_inbox_poller {
                    WakeDisposition::NotAttempted {
                        reason: "member uses a native inbox poller".to_string(),
                    }
                } else {
                    self.ensure_member_daemon_after_inbox_append_best_effort(
                        &team_name_owned,
                        &member_name_owned,
                    )
                };
                let ensured_daemon_pid = match &wake {
                    WakeDisposition::Spawned { pid } | WakeDisposition::Adopted { pid } => {
                        Some(*pid)
                    }
                    WakeDisposition::AlreadyLive
                    | WakeDisposition::NotAttempted { .. }
                    | WakeDisposition::Failed { .. } => None,
                };
                let mut post_write_warnings = Vec::new();

                self.audit_log
                    .push(AuditEvent::DeliverySucceeded(DeliverySucceededEvent {
                        team_name: team_name_owned.clone(),
                        member_name: member_name_owned.clone(),
                        delivery_type,
                        method: result.method,
                        succeeded_at: Utc::now(),
                    }));

                if let Some(context) = operational_context.as_ref() {
                    if let Err(err) = apply_delivery_context(
                        &self.teams_dir,
                        &team_name_owned,
                        &member_name_owned,
                        context,
                    ) {
                        post_write_warnings.push(err.to_string());
                        tracing::warn!(
                            team_name = %team_name_owned,
                            member_name = %member_name_owned,
                            error = %err,
                            "failed to persist operational snapshot update after successful delivery"
                        );
                    }
                }

                let delivered_at = Utc::now();
                if let Err(err) = MemberRuntimeStore::update(
                    &self.teams_dir,
                    &team_name_owned,
                    &member_name_owned,
                    |runtime| {
                        runtime.last_seen_at = Some(delivered_at);
                        if let Some(daemon_pid) = ensured_daemon_pid {
                            runtime.daemon_pid = Some(daemon_pid);
                        }
                    },
                ) {
                    post_write_warnings.push(err.to_string());
                    tracing::warn!(
                        team_name = %team_name_owned,
                        member_name = %member_name_owned,
                        error = %err,
                        "failed to persist runtime delivery state after successful delivery"
                    );
                }

                result.wake = wake;
                result.post_write_warnings.extend(post_write_warnings);
                Ok(result)
            }
            Err(err) => {
                self.audit_log
                    .push(AuditEvent::DeliveryFailed(DeliveryFailedEvent {
                        team_name: team_name_owned,
                        member_name: member_name_owned,
                        delivery_type,
                        error: err.to_string(),
                        failed_at: Utc::now(),
                    }));
                Err(err)
            }
        }
    }

    fn ensure_member_daemon_after_inbox_append_best_effort(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> WakeDisposition {
        let runtime = match MemberRuntimeStore::load(&self.teams_dir, team_name, member_name) {
            Ok(runtime) => runtime,
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    error = %err,
                    "inbox append succeeded but member runtime was unavailable for daemon wake"
                );
                return WakeDisposition::Failed {
                    reason: format!("member runtime unavailable: {err}"),
                };
            }
        };
        let Some(pane_id) = runtime.pane_id.clone() else {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                "inbox append succeeded but member has no pane for daemon wake"
            );
            return WakeDisposition::NotAttempted {
                reason: "member has no pane".to_string(),
            };
        };

        let live_pane = match self.runtime.live_pane(&pane_id) {
            Ok(Some(live_pane)) if !live_pane.is_dead => live_pane,
            Ok(Some(_)) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pane_id = %pane_id,
                    "inbox append succeeded but member pane was dead during daemon wake"
                );
                return WakeDisposition::NotAttempted {
                    reason: crate::coordination::requests::WAKE_REASON_PANE_DEAD.to_string(),
                };
            }
            Ok(None) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pane_id = %pane_id,
                    "inbox append succeeded but member pane was absent during daemon wake"
                );
                return WakeDisposition::NotAttempted {
                    reason: crate::coordination::requests::WAKE_REASON_PANE_NOT_FOUND.to_string(),
                };
            }
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pane_id = %pane_id,
                    error = %err,
                    "inbox append succeeded but pane ownership could not be verified for daemon wake"
                );
                return WakeDisposition::Failed {
                    reason: format!("pane probe failed: {err}"),
                };
            }
        };
        if let PaneOwnership::Foreign { reason } = pane_belongs_to_member(&runtime, &live_pane) {
            if let Err(err) = quarantine_foreign_member(
                &self.teams_dir,
                self.runtime.as_ref(),
                team_name,
                member_name,
                &runtime,
                &live_pane,
                &reason,
            ) {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pane_id = %pane_id,
                    error = %err,
                    "failed to quarantine foreign pane after inbox append"
                );
            }
            return WakeDisposition::Failed {
                reason: format!("member pane is foreign: {reason}"),
            };
        }

        let daemon_is_live = runtime
            .daemon_pid
            .is_some_and(|pid| self.runtime.is_process_running_by_pid(pid).unwrap_or(false));
        if daemon_is_live {
            return WakeDisposition::AlreadyLive;
        }

        let existing_pid = self
            .runtime
            .find_existing_mesh_daemon_pids(&pane_id, team_name, member_name)
            .ok()
            .and_then(|pids| pids.into_iter().next());
        if let Some(pid) = existing_pid {
            return WakeDisposition::Adopted { pid };
        }

        match self
            .runtime
            .spawn_mesh_daemon(&pane_id, team_name, member_name)
        {
            Ok(pid) => WakeDisposition::Spawned { pid },
            Err(err) => {
                tracing::warn!(
                    team = %team_name,
                    member = %member_name,
                    pane_id = %pane_id,
                    error = %err,
                    "inbox append succeeded but member daemon wake could not be ensured"
                );
                WakeDisposition::Failed {
                    reason: format!("daemon spawn failed: {err}"),
                }
            }
        }
    }

    /// Record a lease-claim audit event.
    pub fn record_lease_claimed(
        &mut self,
        team_name: &str,
        member_name: &str,
        pid: u32,
        instance_uuid: &str,
    ) {
        self.audit_log
            .push(AuditEvent::LeaseClaimed(LeaseClaimedEvent {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                owner_pid: pid,
                instance_uuid: instance_uuid.to_string(),
                claimed_at: Utc::now(),
            }));
    }

    /// Record a lease-reclaim audit event.
    pub fn record_lease_reclaimed(
        &mut self,
        team_name: &str,
        member_name: &str,
        previous_pid: u32,
        new_pid: u32,
    ) {
        self.audit_log
            .push(AuditEvent::LeaseReclaimed(LeaseReclaimedEvent {
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                previous_pid,
                new_pid,
                reclaimed_at: Utc::now(),
            }));
    }
}
