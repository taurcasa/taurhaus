use crate::commands::coordination_types::{
    AddAgentReport, InitializeReport, ResumeAgentReport, StepProgress, StepProgressEvent,
    StepStatus,
};

pub(super) fn emit_progress_events(
    events: Vec<StepProgressEvent>,
    mut emit: Option<&mut dyn FnMut(&StepProgressEvent)>,
) {
    let Some(emit) = emit.as_mut() else {
        return;
    };
    for event in events {
        emit(&event);
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
