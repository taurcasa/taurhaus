//! Taurhaus's generated-message producer adapter. Mesh alone owns format-2 projections.
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::errors::CoordinationError;
use super::stores::MeshInboxMessage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalReceipt {
    pub message_id: String,
    pub delivery_id: String,
    pub sequence: u64,
    pub projection: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalLinks {
    pub task: String,
    pub assignment: String,
}

/// Missing config retains the historical store-only append contract. An unreadable,
/// malformed or future format is never permission to mutate compatibility arrays.
pub fn canonical(root: &Path, team: &str) -> Result<bool, CoordinationError> {
    let path = root.join(team).join("config.json");
    let raw = match super::stores::lock::read_to_string_with_retry(&path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match super::stores::lock::read_to_string_with_retry(
                &super::stores::lock::displaced_path(&path),
            ) {
                Ok(raw) => raw,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(e) => return Err(e.into()),
            }
        }
        Err(e) => return Err(e.into()),
    };
    let config: Value =
        serde_json::from_str(&raw).map_err(|_| failure("invalid team messaging config"))?;
    if !config.is_object() {
        return Err(failure("invalid team messaging config"));
    }
    match config.get("messaging_format") {
        None => Ok(false),
        Some(v) if v.as_u64() == Some(1) => Ok(false),
        Some(v) if v.as_u64() == Some(2) => Ok(true),
        _ => Err(failure("unsupported messaging_format")),
    }
}

fn failure(reason: &str) -> CoordinationError {
    CoordinationError::Backend(format!("journal acceptance failed: {reason}"))
}

pub fn report_failure(team: &str, member: &str, id: Option<&str>, error: &CoordinationError) {
    taurhaus_lib::logging::emit_global(
        "warn", "coordination", "coordination.journal.accept_failed", None,
        json!({"team":team,"recipient":member,"idempotency_key":id.map(|id| format!("taurhaus-daemon:{id}")),"reason":error.to_string()})
            .as_object().unwrap().clone(),
    );
}

pub fn accept(
    root: &Path,
    team: &str,
    member: &str,
    message: &MeshInboxMessage,
) -> Result<JournalReceipt, CoordinationError> {
    let id = message
        .id
        .as_deref()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| failure("missing delivery identity"))?;
    let claude_dir = root
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| failure("missing resolved Mesh config root"))?;
    let version = run(root, team, member, &["version", "--json"])?;
    if version["journal_writer"] != "mesh-journal/2" {
        return Err(failure("mesh-journal/2 required"));
    }
    let claude_dir = super::runtime::mesh_cli_claude_dir_arg_from_path(claude_dir);
    let key = format!("taurhaus-daemon:{id}");
    // Authenticate the managed recipient through the same control credential seam
    // as the daemon backend. Mesh marks this service producer as generated/verified.
    let mut args = vec![
        "journal",
        "accept",
        "--producer",
        "taurhaus-daemon",
        "--recipient",
        member,
        "--text",
        &message.text,
        "--idempotency-key",
        &key,
        "--claude-dir",
        &claude_dir,
        "--team",
        team,
        "--name",
        member,
    ];
    let links = message
        .extra
        .get("journal_links")
        .and_then(|value| serde_json::from_value::<JournalLinks>(value.clone()).ok());
    if let Some(links) = &links {
        if !links.task.is_empty() {
            args.extend(["--task", &links.task]);
            if !links.assignment.is_empty() {
                args.extend(["--assignment", &links.assignment]);
            }
        }
    }
    let value = run(root, team, member, &args)?;
    let nonempty = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let target = value["delivery_targets"]
        .as_array()
        .filter(|targets| targets.len() == 1)
        .and_then(|targets| targets.first())
        .filter(|target| target["recipient"] == member)
        .ok_or_else(|| failure("invalid acceptance recipient"))?;
    if value["status"] != "accepted" {
        return Err(failure("missing canonical acceptance"));
    }
    Ok(JournalReceipt {
        message_id: nonempty(&value["message_id"]).ok_or_else(|| failure("missing message_id"))?,
        delivery_id: nonempty(&target["delivery_id"])
            .ok_or_else(|| failure("missing delivery_id"))?,
        sequence: value["sequence"]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or_else(|| failure("invalid sequence"))?,
        projection: nonempty(&value["projection"])
            .ok_or_else(|| failure("missing projection state"))?,
    })
}

fn run(root: &Path, team: &str, member: &str, args: &[&str]) -> Result<Value, CoordinationError> {
    #[cfg(test)]
    if !super::mesh_cli::test_mesh_installed() {
        return Err(failure("test requires a fake mesh executable"));
    }
    let invocation =
        super::runtime::mesh_command_invocation_for_member_at(args, team, member, root);
    let mut command = Command::new(&invocation.program);
    super::runtime::apply_background_command_settings(&mut command);
    command.args(&invocation.args);
    let timeout = if cfg!(test) {
        Duration::from_millis(150)
    } else {
        Duration::from_secs(5)
    };
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        timeout,
        "mesh journal producer",
    )
    .map_err(|e| failure(&e.to_string()))?;
    if !output.status.success() {
        // Do not copy CLI stderr or argv into telemetry: they can contain body or credentials.
        return Err(failure(&format!(
            "mesh {} exited {}",
            args[0], output.status
        )));
    }
    serde_json::from_slice(&output.stdout).map_err(|_| failure("invalid mesh JSON response"))
}
