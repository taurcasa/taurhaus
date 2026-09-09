//! Taurhaus's generated-message producer adapter. Mesh alone owns format-2 projections.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
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
        json!({"team":team,"recipient":member,"idempotency_key":id.map(idempotency_key),"reason":error.to_string()})
            .as_object().unwrap().clone(),
    );
}

fn idempotency_key(id: &str) -> String {
    format!("taurhaus-daemon:{id}")
}

// Successful probes only: repairing an unavailable/old binary must allow the
// existing bounded pre-submission retry. Fake executables have unique temp paths.
type CapabilityKey = (String, Vec<String>, PathBuf);
fn check_capability(root: &Path, team: &str, actor: &str) -> Result<(), CoordinationError> {
    #[cfg(test)]
    if !super::mesh_cli::test_mesh_installed() {
        return Err(failure("test requires a fake mesh executable"));
    }
    static SUPPORTED: OnceLock<Mutex<HashSet<CapabilityKey>>> = OnceLock::new();
    let invocation = super::mesh_cli::mesh_command_invocation(&["version", "--json"]);
    let key = (invocation.program, invocation.args, root.to_path_buf());
    let cache = SUPPORTED.get_or_init(Default::default);
    let mut supported = cache.lock().unwrap_or_else(|e| e.into_inner());
    if supported.contains(&key) {
        return Ok(());
    }
    let version = run(root, team, actor, &["version", "--json"]).map_err(|e| e.error)?;
    if version["journal_writer"] != "mesh-journal/2" {
        return Err(failure("mesh-journal/2 required"));
    }
    supported.insert(key);
    Ok(())
}

pub fn accept(
    root: &Path,
    team: &str,
    member: &str,
    message: &MeshInboxMessage,
) -> Result<JournalReceipt, CoordinationError> {
    // Mesh authenticates --name and uses it as claimed_sender. A service notice
    // uses the lead; explicit senders retain their identity, never the recipient.
    let prepared = (|| {
        let id = message
            .id
            .as_deref()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| failure("missing delivery identity"))?;
        let claude_dir = root
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| failure("missing resolved Mesh config root"))?;
        let actor = if message.from == super::stores::inbox::OPERATOR_SENDER_NAME {
            super::stores::TeamConfigStore::load(root, team)?
                .members
                .into_iter()
                .find(|m| m.role == super::domain::MemberRole::Lead)
                .map(|m| m.name)
                .ok_or_else(|| failure("missing journal producer lead"))?
        } else {
            message.from.clone()
        };
        check_capability(root, team, &actor)?;
        Ok((id, claude_dir, actor))
    })();
    let (id, claude_dir, actor) = prepared.map_err(pre_submission_failure)?;
    let claude_dir = super::runtime::mesh_cli_claude_dir_arg_from_path(claude_dir);
    let key = idempotency_key(id);
    // Resolve the actor's control token through the existing daemon CLI seam.
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
        &actor,
    ];
    let links = message
        .extra
        .get("journal_links")
        .and_then(|value| serde_json::from_value::<JournalLinks>(value.clone()).ok());
    if let Some(links) = &links {
        if !links.task.is_empty() {
            args.extend(["--task", &links.task]);
            // Snapshot tokens can lag reassignment. Mesh derives the current
            // assignment from the task; informational notices must not assert it.
        }
    }
    let value = run(root, team, &actor, &args).map_err(|error| {
        if error.not_submitted {
            pre_submission_failure(error.error)
        } else {
            error.error
        }
    })?;
    if value["status"] != "accepted" {
        return Err(failure(
            safe_error_code(&value).unwrap_or("missing canonical acceptance"),
        ));
    }
    let nonempty = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let target = value["delivery_targets"]
        .as_array()
        .filter(|targets| targets.len() == 1)
        .and_then(|targets| targets.first())
        .filter(|target| nonempty(&target["recipient"]).is_some())
        .ok_or_else(|| failure("invalid acceptance recipient"))?;
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

// Preserve a typed preflight/spawn classification through the existing error
// surface. The backend/hook owner, not this CLI adapter, records recovery state.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct PreSubmissionFailure(CoordinationError);

fn pre_submission_failure(error: CoordinationError) -> CoordinationError {
    CoordinationError::Io(std::io::Error::other(PreSubmissionFailure(error)))
}

pub fn not_submitted(error: &CoordinationError) -> bool {
    matches!(error, CoordinationError::Io(io)
        if io.get_ref().is_some_and(|source| source.is::<PreSubmissionFailure>()))
}

struct CommandFailure {
    error: CoordinationError,
    not_submitted: bool,
}

// An allowlist prevents untrusted CLI output from copying prose or credentials
// into telemetry. This candidate emits plain journal codes; also accept JSON codes.
fn safe_error_code(value: &Value) -> Option<&'static str> {
    known_error_code(value.get("code").or_else(|| value.get("error"))?.as_str()?)
}

fn known_error_code(code: &str) -> Option<&'static str> {
    [
        "canonical_service_contract_required",
        "invalid_idempotency_key",
        "idempotency_conflict",
        "capture_disabled",
        "assignment_mismatch",
        "assignment_requires_one_task",
        "conflicting_orchestration_links",
        "unauthorized",
        "task_not_found",
        "canonical_required",
        "body_budget",
    ]
    .into_iter()
    .find(|known| *known == code)
}

fn refusal_code(bytes: &[u8]) -> Option<&'static str> {
    String::from_utf8_lossy(bytes).lines().find_map(|line| {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            return safe_error_code(&value);
        }
        if let Some(code) = line.strip_prefix("error: IO error: journal: ") {
            return known_error_code(code);
        }
        if line.starts_with("error: unauthorized: ") {
            return Some("unauthorized");
        }
        if line.starts_with("error: assignment token mismatch: ") {
            return Some("assignment_mismatch");
        }
        None
    })
}

fn run(root: &Path, team: &str, member: &str, args: &[&str]) -> Result<Value, CommandFailure> {
    #[cfg(test)]
    if !super::mesh_cli::test_mesh_installed() {
        return Err(CommandFailure {
            error: failure("test requires a fake mesh executable"),
            not_submitted: true,
        });
    }
    let invocation =
        super::runtime::mesh_command_invocation_for_member_at(args, team, member, root);
    let mut command = Command::new(&invocation.program);
    super::runtime::apply_background_command_settings(&mut command);
    command.args(&invocation.args);
    let timeout = if cfg!(test) {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(5)
    };
    let output = crate::process_utils::run_command_with_timeout(
        &mut command,
        timeout,
        "mesh journal producer",
    )
    .map_err(|e| CommandFailure {
        not_submitted: matches!(
            e.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
        ),
        error: failure(&e.to_string()),
    })?;
    if !output.status.success() {
        let code = refusal_code(&output.stdout).or_else(|| refusal_code(&output.stderr));
        return Err(CommandFailure {
            error: failure(&format!(
                "mesh {} exited {}{}",
                args[0],
                output.status,
                code.map(|c| format!(": {c}")).unwrap_or_default()
            )),
            // A cached binary may have been replaced by an older CLI. Clap's
            // explicit subcommand rejection precedes journal submission. Other
            // failures, including coded refusals, remain quarantined.
            not_submitted: output.status.code() == Some(2)
                && code.is_none()
                && String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .any(|line| line.starts_with("error: unrecognized subcommand ")),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|_| CommandFailure {
        error: failure("invalid mesh JSON response"),
        not_submitted: false,
    })
}
