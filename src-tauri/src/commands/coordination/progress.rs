use crate::commands::coordination_types::{
    AddAgentReport, InitializeReport, ResumeAgentReport, StepProgress, StepProgressEvent,
    StepStatus,
};
use serde_json::{Map, Value};

pub(super) fn emit_progress_events(
    events: Vec<StepProgressEvent>,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) {
    for event in events {
        emit_progress_log_event(&event);
        if let Some(emit) = emit.as_mut() {
            emit(&event);
        }
    }
}

pub(super) fn initialize_progress_events(report: &InitializeReport) -> Vec<StepProgressEvent> {
    progress_events_for_steps(&report.team_name, "initialize_team", &report.steps)
}

pub(super) fn add_agent_progress_events(report: &AddAgentReport) -> Vec<StepProgressEvent> {
    progress_events_for_steps(&report.team_name, "add_agent", &report.steps)
}

pub(super) fn resume_member_progress_events(report: &ResumeAgentReport) -> Vec<StepProgressEvent> {
    progress_events_for_steps(&report.team_name, "resume_member", &report.steps)
}

fn progress_events_for_steps(
    team_name: &str,
    operation: &str,
    steps: &[StepProgress],
) -> Vec<StepProgressEvent> {
    let mut events = Vec::new();
    for progress in steps {
        events.push(StepProgressEvent {
            team_name: team_name.to_string(),
            operation: operation.to_string(),
            progress: StepProgress {
                step: progress.step.clone(),
                status: StepStatus::Running,
                message: None,
            },
        });
        events.push(StepProgressEvent {
            team_name: team_name.to_string(),
            operation: operation.to_string(),
            progress: progress.clone(),
        });
    }
    events
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

fn step_status_name(status: &StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::Running => "running",
        StepStatus::Succeeded => "succeeded",
        StepStatus::Failed => "failed",
    }
}
