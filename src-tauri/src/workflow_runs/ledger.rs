use serde_json::Value;

use super::WorkflowRun;

pub fn ledger_row(run: &WorkflowRun) -> Option<String> {
    let result = run.result.as_ref()?.as_object()?;
    if !result.get("commits")?.is_array() || result.get("gate").is_none() {
        return None;
    }
    let ledger = result.get("ledger")?.as_object()?;
    for key in [
        "title",
        "size",
        "implementer",
        "reviewers",
        "rounds",
        "majors",
        "findings",
        "remaining",
    ] {
        if !ledger.contains_key(key) {
            return None;
        }
    }

    let title = scalar(ledger.get("title")?)?;
    let implementer = scalar(ledger.get("implementer")?)?;
    let reviewers = ledger
        .get("reviewers")?
        .as_array()?
        .iter()
        .map(scalar)
        .collect::<Option<Vec<_>>>()?
        .join(", ");
    let rounds = scalar(ledger.get("rounds")?)?;
    let majors = scalar(ledger.get("majors")?)?;

    Some(format!(
        "| {} | {} | {} | {} | {} | tbd |",
        markdown_cell(&title),
        markdown_cell(&implementer),
        markdown_cell(&reviewers),
        markdown_cell(&rounds),
        markdown_cell(&majors),
    ))
}

fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn markdown_cell(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;
    use crate::workflow_runs::{WorkflowRunStatus, WorkflowRunTotals};

    fn run_with_result(result: Value) -> WorkflowRun {
        WorkflowRun {
            run_id: "wf_live-123".to_string(),
            name: "feature-pr".to_string(),
            description: String::new(),
            phases: Vec::new(),
            status: WorkflowRunStatus::Completed,
            started_at: 0,
            finished_at: Some(1),
            agents: Vec::new(),
            totals: WorkflowRunTotals {
                agents: 0,
                done: 0,
                tokens: Some(0),
                tool_calls: Some(0),
                duration_ms: Some(1),
            },
            result: Some(result),
            script_path: PathBuf::new(),
        }
    }

    #[test]
    fn ledger_row_renders_only_the_procedure_return_shape() {
        let mut run = run_with_result(json!({
            "ledger": {
                "title": "W2a | scanner",
                "size": "feature",
                "implementer": "Codex",
                "reviewers": ["Opus conformance", "Opus operational"],
                "rounds": 2,
                "majors": 1,
                "findings": [],
                "remaining": []
            },
            "commits": ["abc123"],
            "gate": {"status":"pass"}
        }));

        assert_eq!(
            ledger_row(&run).as_deref(),
            Some("| W2a \\| scanner | Codex | Opus conformance, Opus operational | 2 | 1 | tbd |")
        );

        run.result = Some(json!("plain workflow result"));
        assert_eq!(ledger_row(&run), None);
    }
}
