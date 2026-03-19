use crate::coordination::backend::BackendKind;
use crate::coordination::domain::{HealthState, Member, MemberRole};
use crate::coordination::requests::{DeliveryMethod, DeliveryRequest};
use crate::coordination::roster::TeamMemberView;
use crate::coordination::stores::{DiscoveredTeam, MemberRuntimeRecord};
use crate::session_scanner::cli_tool::CliTool;

use super::DiscoveredTeamStatus;

pub(super) fn infer_backend_kind(tool: CliTool) -> BackendKind {
    match tool {
        CliTool::Claude => BackendKind::ClaudeNative,
        CliTool::Codex | CliTool::Gemini => BackendKind::MeshBridged,
    }
}

pub(super) fn delivery_meta(req: &DeliveryRequest) -> (&str, &str) {
    match req {
        DeliveryRequest::Bootstrap(payload) => (&payload.team_name, &payload.member_name),
        DeliveryRequest::RecoveryNudge(payload) => (&payload.team_name, &payload.member_name),
        DeliveryRequest::OperatorNotice(payload) => (&payload.team_name, &payload.member_name),
    }
}

pub(super) fn delivery_type_name(req: &DeliveryRequest) -> &'static str {
    match req {
        DeliveryRequest::Bootstrap(_) => "bootstrap",
        DeliveryRequest::RecoveryNudge(_) => "recovery_nudge",
        DeliveryRequest::OperatorNotice(_) => "operator_notice",
    }
}

pub(super) fn delivery_operational_context(
    req: &DeliveryRequest,
) -> Option<&crate::coordination::requests::OperationalContextUpdate> {
    match req {
        DeliveryRequest::OperatorNotice(payload) => payload.operational_context.as_ref(),
        _ => None,
    }
}

pub(super) fn default_method_for_backend(kind: BackendKind) -> DeliveryMethod {
    match kind {
        BackendKind::ClaudeNative => DeliveryMethod::NativeMessageApi,
        BackendKind::MeshBridged => DeliveryMethod::TmuxInjection,
    }
}

pub(super) fn discovered_team_to_status(team: DiscoveredTeam) -> DiscoveredTeamStatus {
    DiscoveredTeamStatus {
        team_name: team.team_name,
        lead_project_path: team.lead_project_path,
    }
}

pub(super) fn should_teardown_member_on_team_cleanup(member: &TeamMemberView) -> bool {
    if member.role != MemberRole::Lead {
        return true;
    }

    if member.configured_cli_tool != CliTool::Claude {
        return true;
    }

    member.runtime_record().is_some_and(|record| {
        record.daemon_pid.is_some() || (record.pane_id.is_some() && record.attached_at.is_some())
    })
}

pub(super) fn team_is_self_heal_candidate(
    runtime_records: &[(String, MemberRuntimeRecord)],
) -> bool {
    runtime_records.iter().any(|(_, record)| {
        record.health != HealthState::SessionDead
            || record.daemon_pid.is_some()
            || record.pane_id.is_some()
            || record.session_id.is_some()
            || record.attached_at.is_some()
    })
}

pub(super) fn team_should_ensure_daemon(runtime_records: &[(String, MemberRuntimeRecord)]) -> bool {
    runtime_records.iter().any(|(_, record)| {
        record.health != HealthState::SessionDead
            || record.daemon_pid.is_some()
            || record.session_id.is_some()
    })
}

pub(super) fn ordered_members_for_team_resume(members: &[Member]) -> Vec<Member> {
    let Some(lead) = members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
    else {
        return members.to_vec();
    };

    let mut ordered = vec![lead.clone()];
    ordered.extend(
        members
            .iter()
            .filter(|member| member.name != lead.name && member.project_path == lead.project_path)
            .cloned(),
    );
    ordered.extend(
        members
            .iter()
            .filter(|member| member.name != lead.name && member.project_path != lead.project_path)
            .cloned(),
    );
    ordered
}
