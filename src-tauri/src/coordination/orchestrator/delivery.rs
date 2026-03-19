use chrono::Utc;

use crate::coordination::audit::{
    AuditEvent, DeliveryAttemptedEvent, DeliveryFailedEvent, DeliverySucceededEvent,
    LeaseClaimedEvent, LeaseReclaimedEvent,
};
use crate::coordination::errors::CoordinationError;
use crate::coordination::operational_context::apply_delivery_context;
use crate::coordination::requests::{DeliveryRequest, DeliveryResult};
use crate::coordination::stores::MemberRuntimeStore;
use crate::session_scanner::cli_tool::CliTool;

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

        let effective_backend = match &self.claude_backend {
            Some(claude_be) => {
                let member_cli_tool = config
                    .members
                    .iter()
                    .find(|m| m.name == member_name)
                    .map(|m| m.cli_tool);
                if member_cli_tool == Some(CliTool::Claude) {
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
            Ok(result) => {
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
                        tracing::warn!(
                            team_name = %team_name_owned,
                            member_name = %member_name_owned,
                            error = %err,
                            "failed to persist operational snapshot update after successful delivery"
                        );
                    }
                }

                if let Ok(mut runtime) =
                    MemberRuntimeStore::load(&self.teams_dir, &team_name_owned, &member_name_owned)
                {
                    runtime.last_seen_at = Some(Utc::now());
                    if let Err(err) = MemberRuntimeStore::save(
                        &self.teams_dir,
                        &team_name_owned,
                        &member_name_owned,
                        &runtime,
                    ) {
                        tracing::warn!(
                            team_name = %team_name_owned,
                            member_name = %member_name_owned,
                            error = %err,
                            "failed to persist runtime last_seen after successful delivery"
                        );
                    }
                }

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
