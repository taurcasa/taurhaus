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
