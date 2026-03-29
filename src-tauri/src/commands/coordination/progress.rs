use crate::commands::coordination_types::{
    AddAgentReport, InitializeReport, MemberActivationStage, ResumeTeamProgressEvent, StepProgress,
    StepProgressEvent, StepStatus,
};
use crate::coordination::requests::canonical_member_activation_stages;
use serde_json::{Map, Value};

pub(super) fn emit_progress_event(
    event: StepProgressEvent,
    emit: &mut Option<&mut dyn FnMut(&StepProgressEvent)>,
) {
    emit_progress_log_event(&event);
    if let Some(emit) = emit.as_deref_mut() {
        emit(&event);
    }
}

pub(super) fn emit_progress_events(
    events: Vec<StepProgressEvent>,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) {
    for event in events {
        emit_progress_event(event, &mut emit);
    }
}

pub(super) fn initialize_progress_events(report: &InitializeReport) -> Vec<StepProgressEvent> {
    progress_events_for_steps(&report.team_name, "initialize_team", &report.steps)
}

pub(super) fn add_agent_progress_events(report: &AddAgentReport) -> Vec<StepProgressEvent> {
    progress_events_for_steps(&report.team_name, "add_agent", &report.steps)
}

pub(super) fn resume_member_progress_event_for_stage(
    team_name: &str,
    stage: MemberActivationStage,
    status: StepStatus,
    message: Option<String>,
) -> StepProgressEvent {
    StepProgressEvent {
        team_name: team_name.to_string(),
        operation: "resume_member".to_string(),
        progress: StepProgress {
            step: resume_member_stream_step_name(stage).to_string(),
            status,
            message,
        },
        canonical_stages: vec![stage],
    }
}

pub(super) fn resume_team_progress_event_for_stage(
    team_name: &str,
    member_name: &str,
    member_index: usize,
    member_count: usize,
    stage: MemberActivationStage,
    status: StepStatus,
    message: Option<String>,
) -> ResumeTeamProgressEvent {
    ResumeTeamProgressEvent {
        operation: "resume_team".to_string(),
        team_name: team_name.to_string(),
        member_name: member_name.to_string(),
        member_index,
        member_count,
        stage,
        status,
        message,
    }
}

pub(super) fn initialize_step_for_member_stage(
    stage: MemberActivationStage,
) -> Option<&'static str> {
    match stage {
        MemberActivationStage::PrepareMember => Some("validate_configuration"),
        MemberActivationStage::AcquirePane | MemberActivationStage::LaunchSession => {
            Some("create_panes")
        }
        MemberActivationStage::CaptureSessionIdentity => Some("launch_sessions"),
        MemberActivationStage::JoinMesh => Some("join_mesh"),
        MemberActivationStage::StartMemberDaemon => Some("start_daemons"),
        MemberActivationStage::CommitRuntime => None,
        MemberActivationStage::DeliverOnboarding => Some("send_onboarding"),
    }
}

fn progress_events_for_steps(
    team_name: &str,
    operation: &str,
    steps: &[StepProgress],
) -> Vec<StepProgressEvent> {
    let mut events = Vec::new();
    for progress in steps {
        let canonical_stages = canonical_stages_for_operation_step(operation, &progress.step);
        debug_assert!(
            operation != "initialize_team"
                || canonical_stages
                    .iter()
                    .all(|stage| initialize_step_for_member_stage(*stage)
                        == Some(progress.step.as_str())),
            "initialize step mapping drifted for '{}'",
            progress.step
        );
        events.push(StepProgressEvent {
            team_name: team_name.to_string(),
            operation: operation.to_string(),
            progress: StepProgress {
                step: progress.step.clone(),
                status: StepStatus::Running,
                message: None,
            },
            canonical_stages: canonical_stages.clone(),
        });
        events.push(StepProgressEvent {
            team_name: team_name.to_string(),
            operation: operation.to_string(),
            progress: progress.clone(),
            canonical_stages,
        });
    }
    events
}

fn canonical_stages_for_operation_step(
    operation: &str,
    legacy_step: &str,
) -> Vec<crate::coordination::requests::MemberActivationStage> {
    let wrapper = match operation {
        "initialize_team" => "initialize",
        "add_agent" => "add_agent",
        "resume_member" => "resume",
        _ => return Vec::new(),
    };
    canonical_member_activation_stages(wrapper, legacy_step).to_vec()
}

fn resume_member_stream_step_name(stage: MemberActivationStage) -> &'static str {
    stage.as_str()
}

fn emit_progress_log_event(event: &StepProgressEvent) {
    let (level, event_name) = match event.progress.status {
        StepStatus::Pending => ("debug", "coordination.step.pending"),
        StepStatus::Running => ("info", "coordination.step.started"),
        StepStatus::Succeeded => ("info", "coordination.step.completed"),
        StepStatus::Failed => ("warn", "coordination.step.failed"),
    };
    let mut fields = Map::new();
    fields.insert(
        "team_name".to_string(),
        Value::String(event.team_name.clone()),
    );
    fields.insert(
        "operation".to_string(),
        Value::String(event.operation.clone()),
    );
    fields.insert(
        "step".to_string(),
        Value::String(event.progress.step.clone()),
    );
    fields.insert(
        "status".to_string(),
        Value::String(step_status_name(&event.progress.status).to_string()),
    );
    if let Some(message) = event.progress.message.as_ref() {
        fields.insert("message".to_string(), Value::String(message.clone()));
    }
    taurhaus_lib::logging::emit_global(
        level,
        "backend",
        event_name,
        Some("Coordination step lifecycle event".to_string()),
        fields,
    );
}

pub(super) fn emit_resume_team_progress_log_event(event: &ResumeTeamProgressEvent) {
    let (level, event_name) = match event.status {
        StepStatus::Pending => ("debug", "coordination.resume_team.member.pending"),
        StepStatus::Running => ("info", "coordination.resume_team.member.started"),
        StepStatus::Succeeded => ("info", "coordination.resume_team.member.completed"),
        StepStatus::Failed => ("warn", "coordination.resume_team.member.failed"),
    };
    let mut fields = Map::new();
    fields.insert(
        "team_name".to_string(),
        Value::String(event.team_name.clone()),
    );
    fields.insert(
        "operation".to_string(),
        Value::String(event.operation.clone()),
    );
    fields.insert(
        "member_name".to_string(),
        Value::String(event.member_name.clone()),
    );
    fields.insert(
        "member_index".to_string(),
        Value::Number(event.member_index.into()),
    );
    fields.insert(
        "member_count".to_string(),
        Value::Number(event.member_count.into()),
    );
    fields.insert("stage".to_string(), Value::String(event.stage.to_string()));
    fields.insert(
        "status".to_string(),
        Value::String(step_status_name(&event.status).to_string()),
    );
    if let Some(message) = event.message.as_ref() {
        fields.insert("message".to_string(), Value::String(message.clone()));
    }
    taurhaus_lib::logging::emit_global(
        level,
        "backend",
        event_name,
        Some("Resume-team member stage lifecycle event".to_string()),
        fields,
    );
}

fn step_status_name(status: &StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::Running => "running",
        StepStatus::Succeeded => "succeeded",
        StepStatus::Failed => "failed",
    }
}
