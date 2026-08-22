//! Team configuration store.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use taurhaus_lib::logging::emit_global;

use super::runtime::MemberRuntimeStore;
use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::runtime::MemberRuntimeRecord;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::launch::ModelSpec;
use crate::templates::types::{BehavioralContract, RuntimeCompactSummary};

const CONFIG_FILENAME: &str = "config.json";
const CONFIG_TMP_FILENAME: &str = "config.json.tmp";
const CONFIG_READBACK_ATTEMPTS: usize = 6;
const CONFIG_READBACK_DELAY: Duration = Duration::from_millis(25);
const SAVE_RETRY_BACKOFFS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(500),
];

fn is_transient_lock_error(err: &std::io::Error) -> bool {
    super::lock::is_transient_file_lock_error(err)
}

fn is_atomic_write_fallback_error(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(1 | 5 | 32))
}

/// Team configuration document persisted at `teams/<team>/config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamConfig {
    pub schema_version: u32,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub members: Vec<Member>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
struct MeshCompatibleTeamConfigWire {
    schema_version: u32,
    name: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
    #[serde(rename = "createdAt")]
    created_at_millis: i64,
    #[serde(rename = "leadAgentId", skip_serializing_if = "Option::is_none")]
    lead_agent_id: Option<String>,
    #[serde(rename = "leadSessionId", skip_serializing_if = "Option::is_none")]
    lead_session_id: Option<String>,
    members: Vec<MeshCompatibleMemberWire>,
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    extra: BTreeMap<String, Value>,
}

/// Mesh-compatible member wire format.
///
/// `project_path` is the canonical internal field. `projectPath` and `cwd` are compatibility
/// aliases that are always derived from `project_path` on write so the three serialized path
/// fields cannot drift.
#[derive(Debug, Serialize)]
struct MeshCompatibleMemberWire {
    name: String,
    role: MemberRole,
    #[serde(rename = "roleId", skip_serializing_if = "Option::is_none")]
    role_id: Option<String>,
    #[serde(rename = "roleName", skip_serializing_if = "Option::is_none")]
    role_name: Option<String>,
    #[serde(rename = "focusArea", skip_serializing_if = "Option::is_none")]
    focus_area: Option<String>,
    #[serde(rename = "contextSummary", skip_serializing_if = "Option::is_none")]
    context_summary: Option<String>,
    #[serde(rename = "behaviorSummary", skip_serializing_if = "Option::is_none")]
    behavior_summary: Option<String>,
    #[serde(rename = "communicationStyle", skip_serializing_if = "Option::is_none")]
    communication_style: Option<String>,
    #[serde(
        rename = "runtimeCompactSummary",
        skip_serializing_if = "Option::is_none"
    )]
    runtime_compact_summary: Option<RuntimeCompactSummary>,
    instructions: Option<String>,
    #[serde(rename = "behavioralContract", skip_serializing_if = "Option::is_none")]
    behavioral_contract: Option<BehavioralContract>,
    #[serde(rename = "qualityGates", skip_serializing_if = "Option::is_none")]
    quality_gates: Option<Vec<String>>,
    #[serde(
        rename = "handoffExpectations",
        skip_serializing_if = "Option::is_none"
    )]
    handoff_expectations: Option<Vec<String>>,
    #[serde(rename = "definitionOfDone", skip_serializing_if = "Option::is_none")]
    definition_of_done: Option<Vec<String>>,
    #[serde(rename = "phaseScope", skip_serializing_if = "Option::is_none")]
    phase_scope: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(rename = "inheritsFrom", skip_serializing_if = "Option::is_none")]
    inherits_from: Option<String>,
    #[serde(rename = "requiredArtifacts", skip_serializing_if = "Option::is_none")]
    required_artifacts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<Vec<String>>,
    project_path: PathBuf,
    cli_tool: CliTool,
    #[serde(rename = "agentId")]
    agent_id: String,
    #[serde(rename = "agentType")]
    agent_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(rename = "reasoningEffort", skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(rename = "joinedAt")]
    joined_at_millis: i64,
    #[serde(rename = "projectPath")]
    project_path_camel: PathBuf,
    cwd: PathBuf,
    #[serde(rename = "tmuxPaneId", skip_serializing_if = "Option::is_none")]
    tmux_pane_id: Option<String>,
    #[serde(rename = "backendType", skip_serializing_if = "Option::is_none")]
    backend_type: Option<String>,
    #[serde(rename = "isActive", skip_serializing_if = "Option::is_none")]
    is_active: Option<bool>,
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    extra: BTreeMap<String, Value>,
}

/// Team discovery entry used for project-anchor restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTeam {
    pub team_name: String,
    pub lead_project_path: Option<PathBuf>,
}

/// Team discovery output with skipped-folder warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDiscovery {
    pub teams: Vec<DiscoveredTeam>,
    pub warnings: Vec<String>,
}

/// Stateless filesystem-backed store for team config documents.
#[derive(Debug, Default)]
pub struct TeamConfigStore;

#[derive(Debug, Deserialize)]
struct MeshTeamConfigWire {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
    #[serde(default)]
    members: Vec<MeshMemberWire>,
    #[serde(flatten, default)]
    extra: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct MeshMemberWire {
    name: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(rename = "type", default)]
    type_name: Option<String>,
    #[serde(rename = "agentType", default)]
    agent_type: Option<String>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default, alias = "roleId")]
    role_id: Option<String>,
    #[serde(default, alias = "roleName")]
    role_name: Option<String>,
    #[serde(default, alias = "focusArea")]
    focus_area: Option<String>,
    #[serde(default, alias = "contextSummary")]
    context_summary: Option<String>,
    #[serde(default, alias = "behaviorSummary")]
    behavior_summary: Option<String>,
    #[serde(default, alias = "communicationStyle")]
    communication_style: Option<String>,
    #[serde(default, alias = "runtimeCompactSummary")]
    runtime_compact_summary: Option<RuntimeCompactSummary>,
    #[serde(default, alias = "behavioralContract")]
    behavioral_contract: Option<BehavioralContract>,
    #[serde(default, alias = "qualityGates")]
    quality_gates: Option<Vec<String>>,
    #[serde(default, alias = "handoffExpectations")]
    handoff_expectations: Option<Vec<String>>,
    #[serde(default, alias = "definitionOfDone")]
    definition_of_done: Option<Vec<String>>,
    #[serde(default, alias = "phaseScope")]
    phase_scope: Option<Vec<String>>,
    #[serde(default, alias = "inheritsFrom")]
    inherits_from: Option<String>,
    #[serde(default, alias = "requiredArtifacts")]
    required_artifacts: Option<Vec<String>>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    capabilities: Option<Vec<String>>,
    #[serde(default)]
    cli_tool: Option<CliTool>,
    #[serde(rename = "projectPath", default)]
    project_path_camel: Option<PathBuf>,
    #[serde(default)]
    project_path: Option<PathBuf>,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "reasoningEffort")]
    reasoning_effort: Option<String>,
    #[serde(flatten, default)]
    extra: BTreeMap<String, Value>,
}

/// Native config member wire format.
///
/// `project_path` is the canonical internal field. `projectPath` and `cwd` are accepted as
/// backward-compatible aliases when reading older config files, but all in-memory consumers should
/// use `Member::project_path`.
#[derive(Debug, Deserialize)]
struct NativeMemberWire {
    name: String,
    role: MemberRole,
    #[serde(default)]
    role_id: Option<String>,
    #[serde(default)]
    role_name: Option<String>,
    #[serde(default)]
    focus_area: Option<String>,
    #[serde(default)]
    context_summary: Option<String>,
    #[serde(default)]
    behavior_summary: Option<String>,
    #[serde(default)]
    communication_style: Option<String>,
    #[serde(default)]
    runtime_compact_summary: Option<RuntimeCompactSummary>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    behavioral_contract: Option<BehavioralContract>,
    #[serde(default)]
    quality_gates: Option<Vec<String>>,
    #[serde(default, alias = "handoffExpectations")]
    handoff_expectations: Option<Vec<String>>,
    #[serde(default)]
    definition_of_done: Option<Vec<String>>,
    #[serde(default)]
    phase_scope: Option<Vec<String>>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    inherits_from: Option<String>,
    #[serde(default)]
    required_artifacts: Option<Vec<String>>,
    #[serde(default)]
    capabilities: Option<Vec<String>>,
    #[serde(default)]
    project_path: Option<PathBuf>,
    #[serde(rename = "projectPath", default)]
    project_path_camel: Option<PathBuf>,
    #[serde(default)]
    cwd: Option<PathBuf>,
    cli_tool: CliTool,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "reasoningEffort")]
    reasoning_effort: Option<String>,
    #[serde(flatten, default)]
    extra: BTreeMap<String, Value>,
}

impl TeamConfigStore {
    /// Load a single team configuration from `<teams_dir>/<team_name>/config.json`.
    pub fn load(teams_dir: &Path, team_name: &str) -> Result<TeamConfig, CoordinationError> {
        let config_path = config_path(teams_dir, team_name);
        let raw = super::lock::read_to_string_with_retry(&config_path).map_err(|err| match err
            .kind()
        {
            std::io::ErrorKind::NotFound => CoordinationError::NotFound(format!(
                "team config not found for '{team_name}' at {}",
                config_path.display()
            )),
            _ => CoordinationError::Io(err),
        })?;

        if raw.trim().is_empty() {
            return Err(CoordinationError::NotFound(format!(
                "team config is empty for '{team_name}' at {}",
                config_path.display()
            )));
        }

        parse_team_config(&raw, team_name)
    }

    /// Save a team configuration atomically via advisory lock + `config.json.tmp` + rename.
    pub fn save(
        teams_dir: &Path,
        team_name: &str,
        config: &TeamConfig,
    ) -> Result<(), CoordinationError> {
        let team_dir = team_dir(teams_dir, team_name);
        fs::create_dir_all(&team_dir).map_err(|err| {
            let coordination_err = CoordinationError::Io(err);
            log_config_store_error("create_dir", &team_dir, &coordination_err, None);
            coordination_err
        })?;

        let mut normalized = config.clone();
        normalized.schema_version = 1;
        normalized.name = team_name.to_string();

        let tmp_path = team_dir.join(CONFIG_TMP_FILENAME);
        let runtime_by_member = match MemberRuntimeStore::load_all(teams_dir, team_name) {
            Ok(records) => records.into_iter().collect::<HashMap<_, _>>(),
            Err(err) => {
                tracing::warn!(
                    team_name = team_name,
                    error = %err,
                    "failed to load runtime records while serializing team config; continuing with defaults"
                );
                HashMap::new()
            }
        };
        let target_path = config_path(teams_dir, team_name);
        let target_lock = super::lock::TargetFileLock::acquire_or_create(&target_path)
            .inspect_err(|err| log_config_store_error("lock", &target_path, err, None))?;
        match target_lock.read_contents() {
            Ok(current_raw) => {
                if let Err(err) =
                    merge_current_extension_fields(&mut normalized, &current_raw, team_name)
                {
                    log_config_store_error("merge_extensions", &target_path, &err, None);
                }
            }
            Err(err)
                if matches!(
                    &err,
                    CoordinationError::Io(io_err)
                        if io_err.kind() == std::io::ErrorKind::InvalidData
                ) =>
            {
                log_config_store_error("merge_extensions", &target_path, &err, None);
            }
            Err(err) => return Err(err),
        }
        let payload =
            serde_json::to_string_pretty(&mesh_compatible_wire(&normalized, &runtime_by_member))
                .map_err(|err| {
                    CoordinationError::StoreError(format!(
                        "failed to serialize team config for '{team_name}': {err}"
                    ))
                })?;

        persist_config_payload(
            teams_dir,
            team_name,
            &target_path,
            &tmp_path,
            &payload,
            &target_lock,
        )
    }

    /// Clear one member's derived tmux binding while holding the config lock
    /// across the read-modify-write cycle.
    pub fn clear_member_pane_binding(
        teams_dir: &Path,
        team_name: &str,
        member_name: &str,
    ) -> Result<bool, CoordinationError> {
        let team_dir = team_dir(teams_dir, team_name);
        let target_path = config_path(teams_dir, team_name);
        let tmp_path = team_dir.join(CONFIG_TMP_FILENAME);
        let target_lock = super::lock::TargetFileLock::acquire_or_create(&target_path)
            .inspect_err(|err| log_config_store_error("lock", &target_path, err, None))?;
        let current_raw = target_lock.read_contents()?;
        if current_raw.trim().is_empty() {
            return Err(CoordinationError::NotFound(format!(
                "team config not found for '{team_name}' at {}",
                target_path.display()
            )));
        }

        let mut current: Value = serde_json::from_str(&current_raw).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to parse config.json for '{team_name}': {err}"
            ))
        })?;
        let members = current
            .get_mut("members")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| {
                CoordinationError::StoreError(format!(
                    "team config for '{team_name}' does not contain a members array"
                ))
            })?;
        let Some(member) = members
            .iter_mut()
            .find(|member| member.get("name").and_then(Value::as_str) == Some(member_name))
        else {
            return Ok(false);
        };
        let member = member.as_object_mut().ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "member '{member_name}' in team '{team_name}' is not an object"
            ))
        })?;
        let changed = member.remove("tmuxPaneId").is_some();
        if !changed {
            return Ok(false);
        }

        let payload = serde_json::to_string_pretty(&current).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize team config for '{team_name}': {err}"
            ))
        })?;
        persist_config_payload(
            teams_dir,
            team_name,
            &target_path,
            &tmp_path,
            &payload,
            &target_lock,
        )?;
        Ok(true)
    }

    /// List team names by scanning direct child directories under `teams_dir`.
    pub fn list(teams_dir: &Path) -> Result<Vec<String>, CoordinationError> {
        if !teams_dir.exists() {
            return Ok(Vec::new());
        }

        let mut teams = Vec::new();
        for entry in fs::read_dir(teams_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let file_name = entry.file_name();
                if let Some(name) = file_name.to_str() {
                    teams.push(name.to_string());
                }
            }
        }

        teams.sort();
        Ok(teams)
    }

    /// Discover valid teams and resolve each lead project anchor.
    ///
    /// Corrupt or unreadable team folders are skipped with warning strings.
    pub fn discover(teams_dir: &Path) -> Result<TeamDiscovery, CoordinationError> {
        if !teams_dir.exists() {
            return Ok(TeamDiscovery {
                teams: Vec::new(),
                warnings: Vec::new(),
            });
        }

        let mut teams = Vec::new();
        let mut warnings = Vec::new();
        let team_dirs = Self::list(teams_dir)?;
        for team_name in team_dirs {
            match Self::load(teams_dir, &team_name) {
                Ok(config) => {
                    let lead_project_path = config
                        .members
                        .iter()
                        .find(|member| member.role == MemberRole::Lead)
                        .or_else(|| config.members.first())
                        .map(|member| member.project_path.clone());
                    teams.push(DiscoveredTeam {
                        team_name: config.name,
                        lead_project_path,
                    });
                }
                Err(CoordinationError::NotFound(_)) => {}
                Err(CoordinationError::StoreError(_)) => {
                    warnings.push(format!(
                        "skipped team folder '{team_name}' because config is missing or invalid"
                    ));
                }
                Err(CoordinationError::Io(err)) => {
                    warnings.push(format!(
                        "skipped team folder '{team_name}' due to IO error: {err}"
                    ));
                }
                Err(other) => {
                    warnings.push(format!(
                        "skipped team folder '{team_name}' due to discovery error: {other}"
                    ));
                }
            }
        }

        teams.sort_by(|a, b| a.team_name.cmp(&b.team_name));
        warnings.sort();
        Ok(TeamDiscovery { teams, warnings })
    }

    /// Remove `<teams_dir>/<team_name>` recursively.
    pub fn delete(teams_dir: &Path, team_name: &str) -> Result<(), CoordinationError> {
        let path = team_dir(teams_dir, team_name);
        match fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(CoordinationError::Io(err)),
        }
    }
}

fn persist_config_payload(
    teams_dir: &Path,
    team_name: &str,
    target_path: &Path,
    tmp_path: &Path,
    payload: &str,
    _target_lock: &super::lock::TargetFileLock,
) -> Result<(), CoordinationError> {
    retry_file_operation(
        "write",
        tmp_path,
        None,
        &SAVE_RETRY_BACKOFFS,
        || write_file_synced(tmp_path, payload),
        |err| log_config_store_io_error("write", tmp_path, err, None),
    )
    .map_err(CoordinationError::Io)?;

    if let Err(err) = retry_file_operation(
        "rename",
        target_path,
        Some(tmp_path),
        &SAVE_RETRY_BACKOFFS,
        || fs::rename(tmp_path, target_path),
        |rename_err| log_config_store_io_error("rename", target_path, rename_err, Some(tmp_path)),
    ) {
        if is_atomic_write_fallback_error(&err) {
            tracing::warn!(
                team_name = team_name,
                target = %target_path.display(),
                raw_os_error = ?err.raw_os_error(),
                "atomic rename failed for team config save; falling back to direct write"
            );
            retry_file_operation(
                "write",
                target_path,
                None,
                &SAVE_RETRY_BACKOFFS,
                || write_file_synced(target_path, payload),
                |write_err| log_config_store_io_error("write", target_path, write_err, None),
            )
            .map_err(CoordinationError::Io)?;
            let _ = fs::remove_file(tmp_path);
            return ensure_saved_config_visible(teams_dir, team_name, target_path, payload);
        }
        // Best-effort cleanup for failed atomic swap.
        let _ = fs::remove_file(tmp_path);
        return Err(CoordinationError::Io(err));
    }

    ensure_saved_config_visible(teams_dir, team_name, target_path, payload)
}

fn mesh_agent_id(team_name: &str, member_name: &str) -> String {
    format!("{member_name}@{team_name}")
}

fn config_store_error_fields(
    operation: &str,
    path: &Path,
    err: &CoordinationError,
    from_path: Option<&Path>,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert(
        "operation".to_string(),
        Value::String(operation.to_string()),
    );
    fields.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );
    fields.insert("error".to_string(), Value::String(err.to_string()));
    fields.insert(
        "raw_os_error".to_string(),
        err.raw_os_error()
            .map(|code| Value::Number(code.into()))
            .unwrap_or(Value::Null),
    );
    if let Some(from_path) = from_path {
        fields.insert(
            "from_path".to_string(),
            Value::String(from_path.display().to_string()),
        );
    }
    fields
}

fn log_config_store_error(
    operation: &str,
    path: &Path,
    err: &CoordinationError,
    from_path: Option<&Path>,
) {
    let fields = config_store_error_fields(operation, path, err, from_path);
    let from_path_display = from_path.map(|value| value.display().to_string());
    emit_global(
        "warn",
        "coordination",
        "coordination.config_store.io_failed",
        Some("Team config store file operation failed".to_string()),
        fields,
    );
    tracing::warn!(
        operation,
        path = %path.display(),
        from_path = from_path_display.as_deref(),
        error = %err,
        raw_os_error = ?err.raw_os_error(),
        "team config store file operation failed"
    );
}

fn clone_io_error(err: &std::io::Error) -> std::io::Error {
    err.raw_os_error()
        .map(std::io::Error::from_raw_os_error)
        .unwrap_or_else(|| std::io::Error::new(err.kind(), err.to_string()))
}

fn log_config_store_io_error(
    operation: &str,
    path: &Path,
    err: &std::io::Error,
    from_path: Option<&Path>,
) {
    let coordination_err = CoordinationError::Io(clone_io_error(err));
    log_config_store_error(operation, path, &coordination_err, from_path);
}

fn retry_file_operation<F, Log>(
    operation: &str,
    path: &Path,
    from_path: Option<&Path>,
    backoffs: &[Duration],
    mut work: F,
    mut log_failure: Log,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
    Log: FnMut(&std::io::Error),
{
    retry_file_operation_with_sleep(
        operation,
        path,
        from_path,
        backoffs,
        &mut work,
        &mut log_failure,
        thread::sleep,
    )
}

fn retry_file_operation_with_sleep<F, Log, Sleep>(
    operation: &str,
    path: &Path,
    from_path: Option<&Path>,
    backoffs: &[Duration],
    work: &mut F,
    log_failure: &mut Log,
    mut sleep: Sleep,
) -> std::io::Result<()>
where
    F: FnMut() -> std::io::Result<()>,
    Log: FnMut(&std::io::Error),
    Sleep: FnMut(Duration),
{
    let total_attempts = backoffs.len() + 1;
    let from_path_display = from_path.map(|value| value.display().to_string());

    for attempt in 0..total_attempts {
        match work() {
            Ok(()) => return Ok(()),
            Err(err) => {
                log_failure(&err);

                if is_transient_lock_error(&err) && attempt < backoffs.len() {
                    let delay = backoffs[attempt];
                    tracing::warn!(
                        operation,
                        path = %path.display(),
                        from_path = from_path_display.as_deref(),
                        attempt = attempt + 1,
                        max_attempts = total_attempts,
                        retry_in_ms = delay.as_millis() as u64,
                        raw_os_error = ?err.raw_os_error(),
                        "transient team config file lock detected; retrying save operation"
                    );
                    sleep(delay);
                    continue;
                }

                return Err(err);
            }
        }
    }

    Ok(())
}

fn write_file_synced(path: &Path, payload: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(payload.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

fn ensure_saved_config_visible(
    teams_dir: &Path,
    team_name: &str,
    target_path: &Path,
    payload: &str,
) -> Result<(), CoordinationError> {
    ensure_config_visible_with_retry(
        CONFIG_READBACK_ATTEMPTS,
        CONFIG_READBACK_DELAY,
        || TeamConfigStore::load(teams_dir, team_name).map(|_| ()),
        || write_file_synced(target_path, payload).map_err(CoordinationError::Io),
    )
}

fn ensure_config_visible_with_retry<ReadBack, Rewrite>(
    attempts: usize,
    delay: Duration,
    mut read_back: ReadBack,
    mut rewrite_target: Rewrite,
) -> Result<(), CoordinationError>
where
    ReadBack: FnMut() -> Result<(), CoordinationError>,
    Rewrite: FnMut() -> Result<(), CoordinationError>,
{
    let total_attempts = attempts.max(1);
    let mut rewrote_target = false;
    let mut last_error = None;

    for attempt in 0..total_attempts {
        match read_back() {
            Ok(()) => return Ok(()),
            Err(CoordinationError::NotFound(message)) => {
                last_error = Some(CoordinationError::NotFound(message));
                if !rewrote_target {
                    rewrite_target()?;
                    rewrote_target = true;
                }
                if attempt + 1 < total_attempts && !delay.is_zero() {
                    thread::sleep(delay);
                }
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        CoordinationError::StoreError("team config did not become visible after save".to_string())
    }))
}

fn mesh_compatible_wire(
    config: &TeamConfig,
    runtime_by_member: &HashMap<String, MemberRuntimeRecord>,
) -> MeshCompatibleTeamConfigWire {
    let created_at_millis = config.created_at.timestamp_millis();
    let lead_member = config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| config.members.first());
    let lead_agent_id = lead_member.map(|member| mesh_agent_id(&config.name, &member.name));
    let lead_session_id = lead_member.map(|member| {
        runtime_by_member
            .get(&member.name)
            .and_then(|runtime| runtime.session_id.clone())
            .unwrap_or_else(|| format!("{}-session", member.name))
    });

    let members = config
        .members
        .iter()
        .map(|member| {
            let project_path = member.project_path.clone();
            let runtime = runtime_by_member.get(&member.name);
            let backend_type = if member.role == MemberRole::Lead {
                None
            } else {
                Some("external".to_string())
            };
            let is_active = if member.role == MemberRole::Lead {
                None
            } else {
                Some(true)
            };
            let mut extra = member.extra.clone();
            for key in ["agentId", "joinedAt", "tmuxPaneId"] {
                extra.remove(key);
            }
            if backend_type.is_some() {
                extra.remove("backendType");
            }
            if is_active.is_some() {
                extra.remove("isActive");
            }
            MeshCompatibleMemberWire {
                name: member.name.clone(),
                role: member.role,
                role_id: member.role_id.clone(),
                role_name: member.role_name.clone(),
                focus_area: member.focus_area.clone(),
                context_summary: member.context_summary.clone(),
                behavior_summary: member.behavior_summary.clone(),
                communication_style: member.communication_style.clone(),
                runtime_compact_summary: member.runtime_compact_summary.clone(),
                instructions: member.instructions.clone(),
                behavioral_contract: member.behavioral_contract.clone(),
                quality_gates: member.quality_gates.clone(),
                handoff_expectations: member.handoff_expectations.clone(),
                definition_of_done: member.definition_of_done.clone(),
                phase_scope: member.phase_scope.clone(),
                mode: member.mode.clone(),
                inherits_from: member.inherits_from.clone(),
                required_artifacts: member.required_artifacts.clone(),
                capabilities: member.capabilities.clone(),
                project_path: project_path.clone(),
                cli_tool: member.cli_tool,
                agent_id: mesh_agent_id(&config.name, &member.name),
                agent_type: if member.role == MemberRole::Lead {
                    "orchestrator".to_string()
                } else {
                    "general-purpose".to_string()
                },
                model: member.model.clone(),
                reasoning_effort: member.reasoning_effort.clone(),
                joined_at_millis: created_at_millis,
                project_path_camel: project_path.clone(),
                cwd: project_path,
                tmux_pane_id: runtime.and_then(|state| state.pane_id.clone()),
                backend_type,
                is_active,
                extra,
            }
        })
        .collect();

    let mut extra = config.extra.clone();
    for key in [
        "schema_version",
        "name",
        "description",
        "created_at",
        "createdAt",
        "leadAgentId",
        "leadSessionId",
        "members",
    ] {
        extra.remove(key);
    }

    MeshCompatibleTeamConfigWire {
        schema_version: config.schema_version,
        name: config.name.clone(),
        description: config.description.clone(),
        created_at: config.created_at,
        created_at_millis,
        lead_agent_id,
        lead_session_id,
        members,
        extra,
    }
}

fn merge_current_extension_fields(
    config: &mut TeamConfig,
    current_raw: &str,
    team_name: &str,
) -> Result<(), CoordinationError> {
    if current_raw.trim().is_empty() {
        return Ok(());
    }

    let current = parse_team_config(current_raw, team_name)?;
    config.extra.extend(current.extra);
    let current_members = current
        .members
        .into_iter()
        .map(|member| (member.name.clone(), member.extra))
        .collect::<HashMap<_, _>>();
    for member in &mut config.members {
        if let Some(current_extra) = current_members.get(&member.name) {
            member.extra.extend(current_extra.clone());
        }
    }
    Ok(())
}

fn parse_team_config(raw: &str, team_name: &str) -> Result<TeamConfig, CoordinationError> {
    let value: Value = serde_json::from_str(raw).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to parse config.json for '{team_name}': {err}"
        ))
    })?;

    // Mesh currently uses camelCase keys (createdAt, leadAgentId, etc).
    if value.get("createdAt").is_some() || value.get("leadAgentId").is_some() {
        parse_mesh_config(value, team_name)
    } else {
        parse_native_config(value, team_name)
    }
}

fn parse_native_config(value: Value, team_name: &str) -> Result<TeamConfig, CoordinationError> {
    #[derive(Debug, Deserialize)]
    struct NativeTeamConfigWire {
        #[serde(default = "schema_version_one")]
        schema_version: u32,
        name: String,
        #[serde(default)]
        description: Option<String>,
        created_at: DateTime<Utc>,
        #[serde(default)]
        members: Vec<NativeMemberWire>,
        #[serde(flatten, default)]
        extra: BTreeMap<String, Value>,
    }

    let wire: NativeTeamConfigWire = serde_json::from_value(value).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to deserialize native config format for '{team_name}': {err}"
        ))
    })?;

    let members = wire
        .members
        .into_iter()
        .map(native_member_to_domain)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TeamConfig {
        schema_version: wire.schema_version,
        name: wire.name,
        description: wire.description,
        created_at: wire.created_at,
        members,
        extra: extension_fields_only(wire.extra, TEAM_AUTHORED_KEYS),
    })
}

fn parse_mesh_config(value: Value, team_name: &str) -> Result<TeamConfig, CoordinationError> {
    let wire: MeshTeamConfigWire = serde_json::from_value(value).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to deserialize mesh config format for '{team_name}': {err}"
        ))
    })?;

    let created_at = wire
        .created_at
        .and_then(|millis| Utc.timestamp_millis_opt(millis).single())
        .unwrap_or_else(Utc::now);

    let members = wire
        .members
        .into_iter()
        .map(mesh_member_to_domain)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TeamConfig {
        schema_version: 1,
        name: wire.name.unwrap_or_else(|| team_name.to_string()),
        description: wire.description,
        created_at,
        members,
        extra: extension_fields_only(wire.extra, TEAM_AUTHORED_KEYS),
    })
}

fn mesh_member_to_domain(member: MeshMemberWire) -> Result<Member, CoordinationError> {
    let role_hint = member
        .role
        .as_deref()
        .or(member.type_name.as_deref())
        .or(member.agent_type.as_deref())
        .unwrap_or("agent");
    let role = parse_role(role_hint);
    let (model, reasoning_effort) =
        normalize_persisted_model_fields(member.model, member.reasoning_effort);
    let cli_tool = member
        .cli_tool
        .or_else(|| model.as_deref().map(cli_tool_from_model))
        .unwrap_or(CliTool::Codex);
    let project_path = member
        .project_path
        .or(member.project_path_camel)
        .or(member.cwd)
        .ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "mesh member '{}' missing project path field (project_path/projectPath/cwd)",
                member.name
            ))
        })?;

    Ok(Member {
        name: member.name,
        role,
        role_id: member.role_id,
        role_name: member.role_name,
        focus_area: member.focus_area,
        context_summary: member.context_summary,
        behavior_summary: member.behavior_summary,
        communication_style: member.communication_style,
        runtime_compact_summary: member.runtime_compact_summary,
        instructions: member.instructions,
        behavioral_contract: member.behavioral_contract,
        quality_gates: member.quality_gates,
        handoff_expectations: member.handoff_expectations,
        definition_of_done: member.definition_of_done,
        phase_scope: member.phase_scope,
        mode: member.mode,
        inherits_from: member.inherits_from,
        required_artifacts: member.required_artifacts,
        capabilities: member.capabilities,
        model,
        reasoning_effort,
        project_path,
        cli_tool,
        extra: extension_fields_only(member.extra, MEMBER_AUTHORED_KEYS),
    })
}

fn native_member_to_domain(member: NativeMemberWire) -> Result<Member, CoordinationError> {
    let (model, reasoning_effort) =
        normalize_persisted_model_fields(member.model, member.reasoning_effort);
    let project_path = member
        .project_path
        .or(member.project_path_camel)
        .or(member.cwd)
        .ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "native member '{}' missing project path field (project_path/projectPath/cwd)",
                member.name
            ))
        })?;

    Ok(Member {
        name: member.name,
        role: member.role,
        role_id: member.role_id,
        role_name: member.role_name,
        focus_area: member.focus_area,
        context_summary: member.context_summary,
        behavior_summary: member.behavior_summary,
        communication_style: member.communication_style,
        runtime_compact_summary: member.runtime_compact_summary,
        instructions: member.instructions,
        behavioral_contract: member.behavioral_contract,
        quality_gates: member.quality_gates,
        handoff_expectations: member.handoff_expectations,
        definition_of_done: member.definition_of_done,
        phase_scope: member.phase_scope,
        mode: member.mode,
        inherits_from: member.inherits_from,
        required_artifacts: member.required_artifacts,
        capabilities: member.capabilities,
        model,
        reasoning_effort,
        project_path,
        cli_tool: member.cli_tool,
        extra: extension_fields_only(member.extra, MEMBER_AUTHORED_KEYS),
    })
}

const TEAM_AUTHORED_KEYS: &[&str] = &[
    "schema_version",
    "schemaVersion",
    "name",
    "description",
    "created_at",
    "createdAt",
    "leadAgentId",
    "leadSessionId",
    "members",
];

const MEMBER_AUTHORED_KEYS: &[&str] = &[
    "name",
    "role",
    "type",
    "agentType",
    "role_id",
    "roleId",
    "role_name",
    "roleName",
    "focus_area",
    "focusArea",
    "context_summary",
    "contextSummary",
    "behavior_summary",
    "behaviorSummary",
    "communication_style",
    "communicationStyle",
    "runtime_compact_summary",
    "runtimeCompactSummary",
    "instructions",
    "behavioral_contract",
    "behavioralContract",
    "quality_gates",
    "qualityGates",
    "handoff_expectations",
    "handoffExpectations",
    "definition_of_done",
    "definitionOfDone",
    "phase_scope",
    "phaseScope",
    "mode",
    "inherits_from",
    "inheritsFrom",
    "required_artifacts",
    "requiredArtifacts",
    "capabilities",
    "project_path",
    "projectPath",
    "cwd",
    "cli_tool",
    "model",
    "reasoning_effort",
    "reasoningEffort",
    "agentId",
    "joinedAt",
    "tmuxPaneId",
];

fn extension_fields_only(
    mut fields: BTreeMap<String, Value>,
    authored_keys: &[&str],
) -> BTreeMap<String, Value> {
    for key in authored_keys {
        fields.remove(*key);
    }
    fields
}

fn normalize_persisted_model_fields(
    model: Option<String>,
    reasoning_effort: Option<String>,
) -> (Option<String>, Option<String>) {
    let Some(model) = model.as_deref() else {
        return (None, reasoning_effort);
    };
    if model.trim().eq_ignore_ascii_case("external") {
        return (None, reasoning_effort);
    }

    let mut parsed = ModelSpec::parse_legacy(model);
    if reasoning_effort.is_some() {
        parsed.reasoning_effort = reasoning_effort;
    }
    (parsed.model, parsed.reasoning_effort)
}

fn parse_role(value: &str) -> MemberRole {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "lead" || normalized == "team-lead" || normalized == "orchestrator" {
        MemberRole::Lead
    } else {
        MemberRole::Agent
    }
}

fn cli_tool_from_model(model: &str) -> CliTool {
    let lower = model.to_ascii_lowercase();
    if lower.contains("claude") {
        CliTool::Claude
    } else if lower.contains("gemini") {
        CliTool::Gemini
    } else {
        CliTool::Codex
    }
}

fn team_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name)
}

fn config_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    team_dir(teams_dir, team_name).join(CONFIG_FILENAME)
}

const fn schema_version_one() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-03-01T21:00:00Z")
            .expect("valid RFC3339 timestamp")
            .with_timezone(&Utc)
    }

    fn sample_config(team_name: &str) -> TeamConfig {
        TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: Some("Architecture team".to_string()),
            created_at: test_timestamp(),
            members: vec![Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("Own orchestration".to_string()),
                behavioral_contract: None,
                quality_gates: None,
                handoff_expectations: Some(vec!["Report verification evidence".to_string()]),
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                model: Some("claude-sonnet-4-5".to_string()),
                reasoning_effort: Some("high".to_string()),
                project_path: PathBuf::from("/tmp/taurhaus"),
                cli_tool: CliTool::Claude,
                extra: Default::default(),
            }],
            extra: Default::default(),
        }
    }

    fn sample_config_with_role_metadata(team_name: &str) -> TeamConfig {
        TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: Some("Architecture team".to_string()),
            created_at: test_timestamp(),
            members: vec![Member {
                name: "codex-dev".to_string(),
                role: MemberRole::Agent,
                role_id: Some("codex-developer".to_string()),
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: Some("Implement safely".to_string()),
                behavioral_contract: Some(BehavioralContract {
                    communication: vec!["share updates".to_string()],
                    execution: vec!["ship patches".to_string()],
                    escalation: vec!["raise blockers".to_string()],
                }),
                quality_gates: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: Some(vec!["implementation".to_string(), "testing".to_string()]),
                model: None,
                reasoning_effort: None,
                project_path: PathBuf::from("/tmp/taurhaus"),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            }],
            extra: Default::default(),
        }
    }

    #[test]
    fn atomic_write_fallback_error_detection_includes_unc_locking_codes() {
        assert!(is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(1)
        ));
        assert!(is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(5)
        ));
        assert!(is_atomic_write_fallback_error(
            &std::io::Error::from_raw_os_error(32)
        ));
    }

    #[test]
    fn non_fallback_rename_error_is_rejected() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(!is_atomic_write_fallback_error(&err));
    }

    #[test]
    fn retry_file_operation_retries_transient_lock_errors_until_success() {
        let mut attempts = 0;
        let mut slept = Vec::new();

        let result = retry_file_operation_with_sleep(
            "rename",
            Path::new("/tmp/config.json"),
            Some(Path::new("/tmp/config.json.tmp")),
            &SAVE_RETRY_BACKOFFS,
            &mut || {
                attempts += 1;
                if attempts < 4 {
                    Err(std::io::Error::from_raw_os_error(32))
                } else {
                    Ok(())
                }
            },
            &mut |_| {},
            |delay| slept.push(delay),
        );

        assert!(result.is_ok());
        assert_eq!(attempts, 4);
        assert_eq!(slept, SAVE_RETRY_BACKOFFS);
    }

    #[test]
    fn retry_file_operation_retries_windows_lock_violation() {
        // Regression: 694b130 locked config.json itself while unlocked Windows
        // readers treated ERROR_LOCK_VIOLATION (33) as a permanent I/O failure.
        let mut attempts = 0;
        let result = retry_file_operation_with_sleep(
            "read",
            Path::new("C:/tmp/config.json"),
            None,
            &[Duration::ZERO],
            &mut || {
                attempts += 1;
                if attempts == 1 {
                    Err(std::io::Error::from_raw_os_error(33))
                } else {
                    Ok(())
                }
            },
            &mut |_| {},
            |_| {},
        );

        assert!(result.is_ok());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn retry_file_operation_stops_after_retry_budget_is_exhausted() {
        let mut attempts = 0;
        let mut slept = Vec::new();

        let err = retry_file_operation_with_sleep(
            "rename",
            Path::new("/tmp/config.json"),
            Some(Path::new("/tmp/config.json.tmp")),
            &SAVE_RETRY_BACKOFFS,
            &mut || {
                attempts += 1;
                Err(std::io::Error::from_raw_os_error(5))
            },
            &mut |_| {},
            |delay| slept.push(delay),
        )
        .expect_err("retry budget should eventually surface the error");

        assert_eq!(err.raw_os_error(), Some(5));
        assert_eq!(attempts, SAVE_RETRY_BACKOFFS.len() + 1);
        assert_eq!(slept, SAVE_RETRY_BACKOFFS);
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let loaded = TeamConfigStore::load(teams_dir, team_name).expect("load should succeed");

        assert_eq!(loaded, config);
    }

    #[test]
    fn clear_member_pane_binding_preserves_current_config_fields() {
        // Regression: 39eeb33 cleared quarantine metadata through an unlocked
        // typed load+save, so a concurrent roster or metadata edit could be reverted.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "pane-quarantine";
        TeamConfigStore::save(tmp.path(), team_name, &sample_config(team_name)).expect("save");

        let path = config_path(tmp.path(), team_name);
        let mut current: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read config"))
                .expect("parse config");
        current["freshTeamField"] = serde_json::json!({ "keep": true });
        current["members"][0]["instructions"] = Value::String("new instructions".to_string());
        current["members"][0]["tmuxPaneId"] = Value::String("%9".to_string());
        fs::write(
            &path,
            serde_json::to_string_pretty(&current).expect("serialize current config"),
        )
        .expect("write current config");

        let mut expected = current;
        expected["members"][0]
            .as_object_mut()
            .expect("member object")
            .remove("tmuxPaneId");

        assert!(
            TeamConfigStore::clear_member_pane_binding(tmp.path(), team_name, "team-lead")
                .expect("clear binding")
        );
        let saved: Value = serde_json::from_str(
            &fs::read_to_string(&path).expect("read config after targeted update"),
        )
        .expect("parse targeted update");
        assert_eq!(saved, expected);
        assert!(
            !TeamConfigStore::clear_member_pane_binding(tmp.path(), team_name, "team-lead")
                .expect("repeat clear is idempotent")
        );
    }

    #[test]
    fn save_writes_declared_model_and_reasoning_effort() {
        let tmp = TempDir::new().expect("tempdir");
        let config = sample_config("architecture-final");

        TeamConfigStore::save(tmp.path(), "architecture-final", &config).expect("save");
        let value: Value = serde_json::from_str(
            &fs::read_to_string(config_path(tmp.path(), "architecture-final"))
                .expect("read config"),
        )
        .expect("parse config");

        assert_eq!(value["members"][0]["model"], "claude-sonnet-4-5");
        assert_eq!(value["members"][0]["reasoningEffort"], "high");
        assert_eq!(
            value["members"][0]["handoffExpectations"][0],
            "Report verification evidence"
        );
    }

    #[test]
    fn save_after_injected_mesh_fields_preserves_all_unknown_fields() {
        // Regression: mesh-findings P1; live espn-fantasy-desktop-team lost every
        // controlAuthTokenHash except the resumed member's on a taurhaus save.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "mesh-round-trip";
        TeamConfigStore::save(tmp.path(), team_name, &sample_config(team_name)).expect("save");

        let path = config_path(tmp.path(), team_name);
        let mut value: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read config"))
                .expect("parse config");
        value["claudeCodeExtension"] = serde_json::json!({ "preserve": true });
        let member = value["members"][0].as_object_mut().expect("member object");
        member.insert(
            "controlAuthTokenHash".to_string(),
            Value::String("sha256:lead-token".to_string()),
        );
        member.insert(
            "lastActivityAt".to_string(),
            serde_json::json!(1772399807000_u64),
        );
        member.insert(
            "lastActivityReason".to_string(),
            Value::String("tool_call".to_string()),
        );
        member.insert(
            "statusState".to_string(),
            Value::String("working".to_string()),
        );
        member.insert(
            "statusReason".to_string(),
            Value::String("implementing".to_string()),
        );
        member.insert(
            "statusSetAt".to_string(),
            serde_json::json!(1772399807100_u64),
        );
        member.insert("isActive".to_string(), Value::Bool(false));
        member.insert(
            "futureMeshField".to_string(),
            serde_json::json!({ "nested": [1, 2, 3] }),
        );
        fs::write(
            &path,
            serde_json::to_string_pretty(&value).expect("serialize injected config"),
        )
        .expect("write injected config");

        let loaded = TeamConfigStore::load(tmp.path(), team_name).expect("load injected config");
        TeamConfigStore::save(tmp.path(), team_name, &loaded).expect("resave injected config");

        let saved: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read round-tripped config"))
                .expect("parse round-tripped config");
        assert_eq!(
            saved["claudeCodeExtension"],
            serde_json::json!({ "preserve": true })
        );
        assert_eq!(
            saved["members"][0]["controlAuthTokenHash"],
            "sha256:lead-token"
        );
        assert_eq!(saved["members"][0]["lastActivityAt"], 1772399807000_u64);
        assert_eq!(saved["members"][0]["lastActivityReason"], "tool_call");
        assert_eq!(saved["members"][0]["statusState"], "working");
        assert_eq!(saved["members"][0]["statusReason"], "implementing");
        assert_eq!(saved["members"][0]["statusSetAt"], 1772399807100_u64);
        assert_eq!(saved["members"][0]["isActive"], false);
        assert_eq!(
            saved["members"][0]["futureMeshField"],
            serde_json::json!({ "nested": [1, 2, 3] })
        );
    }

    #[test]
    fn save_merges_unknown_fields_written_by_a_second_writer_after_load() {
        // Regression: mesh-findings P1; taurhaus serialized a stale domain copy
        // over mesh's credential write because the two writers locked different files.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "two-writer-round-trip";
        TeamConfigStore::save(tmp.path(), team_name, &sample_config(team_name)).expect("save");
        let stale = TeamConfigStore::load(tmp.path(), team_name).expect("stale load");

        let path = config_path(tmp.path(), team_name);
        let mut mesh_value: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read config"))
                .expect("parse config");
        mesh_value["members"][0]["controlAuthTokenHash"] =
            Value::String("sha256:concurrent-token".to_string());
        mesh_value["members"][0]["lastActivityAt"] = serde_json::json!(1772399808000_u64);
        fs::write(
            &path,
            serde_json::to_string_pretty(&mesh_value).expect("serialize mesh write"),
        )
        .expect("write mesh update");

        TeamConfigStore::save(tmp.path(), team_name, &stale).expect("save stale taurhaus copy");

        let saved: Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read final config"))
                .expect("parse final config");
        assert_eq!(
            saved["members"][0]["controlAuthTokenHash"],
            "sha256:concurrent-token"
        );
        assert_eq!(saved["members"][0]["lastActivityAt"], 1772399808000_u64);
    }

    #[test]
    fn pre_model_persistence_config_loads_with_absent_model_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "legacy-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");
        fs::write(
            dir.join(CONFIG_FILENAME),
            r#"{
  "name": "legacy-team",
  "createdAt": 1772399806546,
  "members": [{
    "name": "builder",
    "agentType": "general-purpose",
    "cli_tool": "codex",
    "cwd": "/tmp/project"
  }]
}"#,
        )
        .expect("write legacy config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load legacy config");
        assert_eq!(config.members[0].model, None);
        assert_eq!(config.members[0].reasoning_effort, None);
    }

    #[test]
    fn config_store_has_no_legacy_model_default_helper() {
        let removed_helper = ["default_model", "for_cli"].join("_");
        assert!(!include_str!("config.rs").contains(&removed_helper));
    }

    #[test]
    fn save_then_load_round_trip_keeps_all_project_path_aliases_consistent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");

        let raw =
            fs::read_to_string(config_path(teams_dir, team_name)).expect("read serialized config");
        let value: Value = serde_json::from_str(&raw).expect("parse serialized config");
        let member = value["members"][0].as_object().expect("member object");
        let canonical = member
            .get("project_path")
            .and_then(Value::as_str)
            .expect("project_path present");
        assert_eq!(
            member.get("projectPath").and_then(Value::as_str),
            Some(canonical),
            "projectPath alias should mirror canonical project_path"
        );
        assert_eq!(
            member.get("cwd").and_then(Value::as_str),
            Some(canonical),
            "cwd alias should mirror canonical project_path"
        );

        let loaded = TeamConfigStore::load(teams_dir, team_name).expect("load should succeed");
        assert_eq!(loaded.members[0].project_path, PathBuf::from(canonical));
        assert_eq!(loaded, config);
    }

    #[test]
    fn save_writes_mesh_compatibility_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let raw =
            fs::read_to_string(config_path(teams_dir, team_name)).expect("read serialized config");
        let value: Value = serde_json::from_str(&raw).expect("parse serialized config");

        assert_eq!(
            value.get("leadAgentId").and_then(Value::as_str),
            Some("team-lead@architecture-final")
        );
        assert_eq!(
            value.get("leadSessionId").and_then(Value::as_str),
            Some("team-lead-session")
        );
        let members = value
            .get("members")
            .and_then(Value::as_array)
            .expect("members array");
        assert_eq!(
            members[0].get("agentId").and_then(Value::as_str),
            Some("team-lead@architecture-final")
        );
        assert_eq!(
            members[0].get("agentType").and_then(Value::as_str),
            Some("orchestrator")
        );
        assert!(members[0].get("joinedAt").is_some());
        assert!(members[0].get("model").is_some());
        assert!(
            members[0].get("project_path").is_some(),
            "project_path canonical field should be present"
        );
        assert!(
            members[0].get("projectPath").is_some(),
            "projectPath compatibility field should be present"
        );
        assert!(
            members[0].get("cwd").is_some(),
            "cwd compatibility field should be present"
        );
    }

    #[test]
    fn save_serializes_role_template_context_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "templated-team";
        let config = sample_config_with_role_metadata(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let raw =
            fs::read_to_string(config_path(teams_dir, team_name)).expect("read serialized config");
        let value: Value = serde_json::from_str(&raw).expect("parse serialized config");

        let members = value
            .get("members")
            .and_then(Value::as_array)
            .expect("members array");
        let member = &members[0];

        assert_eq!(
            member.get("roleId").and_then(Value::as_str),
            Some("codex-developer")
        );
        assert_eq!(
            member.get("instructions").and_then(Value::as_str),
            Some("Implement safely")
        );
        assert_eq!(
            member
                .get("behavioralContract")
                .and_then(|contract| contract.get("execution"))
                .and_then(Value::as_array)
                .and_then(|steps| steps.first())
                .and_then(Value::as_str),
            Some("ship patches")
        );
        assert_eq!(
            member
                .get("capabilities")
                .and_then(Value::as_array)
                .and_then(|capabilities| capabilities.first())
                .and_then(Value::as_str),
            Some("implementation")
        );
    }

    #[test]
    fn list_returns_team_directories_sorted() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();

        fs::create_dir_all(teams_dir.join("zeta")).expect("create zeta");
        fs::create_dir_all(teams_dir.join("alpha")).expect("create alpha");
        fs::write(teams_dir.join("README.txt"), "not a team dir").expect("write file");

        let teams = TeamConfigStore::list(teams_dir).expect("list should succeed");
        assert_eq!(teams, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    #[test]
    fn corrupt_json_returns_store_error() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "broken-team";
        let team_dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&team_dir).expect("create team dir");
        fs::write(team_dir.join(CONFIG_FILENAME), "{ this is invalid json").expect("write garbage");

        let err = TeamConfigStore::load(tmp.path(), team_name).expect_err("expected parse failure");
        match err {
            CoordinationError::StoreError(message) => {
                assert!(message.contains("failed to parse config.json"));
            }
            other => panic!("expected store error, got {other:?}"),
        }
    }

    #[test]
    fn save_repairs_an_unparseable_existing_config() {
        // Regression: 694b130 made extension-field preservation fatal, wedging
        // every later save when config.json was partial, foreign, or corrupt.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "repair-corrupt-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");
        fs::write(dir.join(CONFIG_FILENAME), "{ truncated").expect("write corrupt config");
        let replacement = sample_config(team_name);

        TeamConfigStore::save(tmp.path(), team_name, &replacement)
            .expect("a valid taurhaus save should repair corrupt config.json");

        assert_eq!(
            TeamConfigStore::load(tmp.path(), team_name).expect("load repaired config"),
            replacement
        );

        fs::write(dir.join(CONFIG_FILENAME), [0xff, 0xfe]).expect("write non-UTF-8 config");
        TeamConfigStore::save(tmp.path(), team_name, &replacement)
            .expect("a valid taurhaus save should repair non-UTF-8 config.json");
        assert_eq!(
            TeamConfigStore::load(tmp.path(), team_name).expect("load UTF-8 repaired config"),
            replacement
        );
    }

    #[test]
    fn empty_config_is_treated_as_not_found() {
        // Regression: 694b130 could create config.json before a failed save,
        // leaving an empty file that blocked later team creation as corruption.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "empty-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");
        fs::write(dir.join(CONFIG_FILENAME), "").expect("write empty config");

        assert!(matches!(
            TeamConfigStore::load(tmp.path(), team_name),
            Err(CoordinationError::NotFound(_))
        ));
    }

    #[test]
    fn save_does_not_leave_tmp_file_on_success() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        TeamConfigStore::save(teams_dir, team_name, &sample_config(team_name))
            .expect("save should succeed");

        let tmp_path = team_dir(teams_dir, team_name).join(CONFIG_TMP_FILENAME);
        assert!(
            !tmp_path.exists(),
            "tmp file should not linger after successful save"
        );
    }

    #[test]
    fn duplicate_save_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("first save");
        TeamConfigStore::save(teams_dir, team_name, &config).expect("second save should succeed");

        let loaded = TeamConfigStore::load(teams_dir, team_name).expect("load after double save");
        assert_eq!(loaded, config);
    }

    #[test]
    fn config_visibility_retry_recovers_after_initial_not_found() {
        let mut attempts = 0;
        let mut rewrites = 0;

        let result = ensure_config_visible_with_retry(
            3,
            Duration::ZERO,
            || {
                attempts += 1;
                if attempts < 3 {
                    Err(CoordinationError::NotFound("not visible yet".to_string()))
                } else {
                    Ok(())
                }
            },
            || {
                rewrites += 1;
                Ok(())
            },
        );

        assert!(result.is_ok());
        assert_eq!(rewrites, 1);
        assert_eq!(attempts, 3);
    }

    #[test]
    fn config_visibility_retry_returns_not_found_when_visibility_never_recovers() {
        let mut rewrites = 0;

        let result = ensure_config_visible_with_retry(
            2,
            Duration::ZERO,
            || Err(CoordinationError::NotFound("still missing".to_string())),
            || {
                rewrites += 1;
                Ok(())
            },
        );

        match result {
            Err(CoordinationError::NotFound(message)) => {
                assert!(message.contains("still missing"));
            }
            other => panic!("expected not found, got {other:?}"),
        }
        assert_eq!(rewrites, 1);
    }

    #[test]
    fn duplicate_delete_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        TeamConfigStore::save(teams_dir, team_name, &sample_config(team_name)).expect("save");
        TeamConfigStore::delete(teams_dir, team_name).expect("first delete");
        TeamConfigStore::delete(teams_dir, team_name).expect("second delete should not error");
    }

    #[test]
    fn concurrent_saves_do_not_corrupt() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = Arc::new(tmp.path().to_path_buf());
        let team_name = "concurrent-team";
        let barrier = Arc::new(Barrier::new(8));

        // Pre-create team dir so locks work.
        TeamConfigStore::save(&teams_dir, team_name, &sample_config(team_name))
            .expect("initial save");

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let dir = Arc::clone(&teams_dir);
                let bar = Arc::clone(&barrier);
                let name = team_name.to_string();
                thread::spawn(move || {
                    bar.wait();
                    let mut config = sample_config(&name);
                    config.description = Some(format!("thread-{i}"));
                    TeamConfigStore::save(&dir, &name, &config).expect("concurrent save");
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }

        // The final state should be a valid config from one of the threads.
        let loaded =
            TeamConfigStore::load(&teams_dir, team_name).expect("load after concurrent saves");
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.name, team_name);
        assert!(
            loaded.description.as_ref().unwrap().starts_with("thread-"),
            "description should be from one of the concurrent writers"
        );
    }

    #[test]
    fn load_mesh_format_config() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "architecture-final";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");

        let mesh_json = r#"{
  "name": "architecture-final",
  "description": "Final architecture discussion",
  "createdAt": 1772399806546,
  "leadAgentId": "team-lead@architecture-final",
  "members": [
    {
      "name": "team-lead",
      "agentType": "team-lead",
      "model": "claude-opus-4-6",
      "cwd": "/home/user/projects/taurhaus"
    },
    {
      "name": "codex-reviewer",
      "agentType": "general-purpose",
      "model": "gpt-5.2-codex",
      "cwd": "/home/user/projects/taurhaus"
    }
  ]
}"#;
        fs::write(dir.join(CONFIG_FILENAME), mesh_json).expect("write mesh config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load should succeed");
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.name, "architecture-final");
        assert_eq!(config.members.len(), 2);
        assert_eq!(config.members[0].role, MemberRole::Lead);
        assert_eq!(config.members[0].cli_tool, CliTool::Claude);
        assert_eq!(config.members[0].role_id, None);
        assert_eq!(config.members[0].instructions, None);
        assert_eq!(config.members[0].behavioral_contract, None);
        assert_eq!(config.members[0].capabilities, None);
        assert_eq!(config.members[1].role, MemberRole::Agent);
        assert_eq!(config.members[1].cli_tool, CliTool::Codex);
        assert_eq!(config.members[1].role_id, None);
        assert_eq!(config.members[1].instructions, None);
        assert_eq!(config.members[1].behavioral_contract, None);
        assert_eq!(config.members[1].capabilities, None);
    }

    #[test]
    fn load_mesh_format_without_name_uses_folder_name() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "legacy-mesh-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");

        let mesh_json = r#"{
  "createdAt": 1773711699720,
  "description": "legacy mesh config without top-level name",
  "leadAgentId": "evaluator@legacy-mesh-team",
  "members": [
    {
      "name": "evaluator",
      "agentType": "orchestrator",
      "model": "claude-opus-4-6",
      "cwd": "/home/user/projects/taureval"
    },
    {
      "name": "agent-under-test",
      "agentType": "general-purpose",
      "model": "claude-opus-4-6",
      "cwd": "/home/user/projects/taureval"
    }
  ]
}"#;
        fs::write(dir.join(CONFIG_FILENAME), mesh_json).expect("write mesh config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load should succeed");
        assert_eq!(config.name, team_name);
        assert_eq!(config.members.len(), 2);
        assert_eq!(config.members[0].role, MemberRole::Lead);
        assert_eq!(
            config.members[0].project_path,
            PathBuf::from("/home/user/projects/taureval")
        );
        assert_eq!(config.members[1].cli_tool, CliTool::Claude);
    }

    #[test]
    fn load_mesh_format_with_orchestrator_agent_type_marks_lead() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "orchestrator-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");

        let mesh_json = r#"{
  "name": "orchestrator-team",
  "createdAt": 1772399806546,
  "leadAgentId": "lead@orchestrator-team",
  "leadSessionId": "session-1",
  "members": [
    {
      "name": "lead",
      "agentId": "lead@orchestrator-team",
      "agentType": "orchestrator",
      "model": "claude-opus-4-6",
      "cwd": "/home/user/projects/taurhaus"
    }
  ]
}"#;
        fs::write(dir.join(CONFIG_FILENAME), mesh_json).expect("write mesh config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load should succeed");
        assert_eq!(config.members.len(), 1);
        assert_eq!(config.members[0].role, MemberRole::Lead);
    }

    #[test]
    fn load_mesh_format_preserves_role_template_context_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "templated-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");

        let mesh_json = r#"{
  "name": "templated-team",
  "createdAt": 1772399806546,
  "members": [
    {
      "name": "codex-dev",
      "agentType": "general-purpose",
      "model": "gpt-5.4 high",
      "cwd": "/home/user/projects/taurhaus",
      "roleId": "codex-developer",
      "instructions": "Implement safely",
      "behavioralContract": {
        "communication": ["updates"],
        "execution": ["test"],
        "escalation": ["blockers"]
      },
      "capabilities": ["implementation", "testing"]
    }
  ]
}"#;
        fs::write(dir.join(CONFIG_FILENAME), mesh_json).expect("write mesh config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load should succeed");
        assert_eq!(config.members.len(), 1);
        let member = &config.members[0];
        assert_eq!(member.role_id.as_deref(), Some("codex-developer"));
        assert_eq!(member.instructions.as_deref(), Some("Implement safely"));
        assert_eq!(
            member
                .behavioral_contract
                .as_ref()
                .map(|contract| contract.communication.clone())
                .unwrap_or_default(),
            vec!["updates".to_string()]
        );
        assert_eq!(
            member.capabilities.as_ref().cloned().unwrap_or_default(),
            vec!["implementation".to_string(), "testing".to_string()]
        );
    }

    #[test]
    fn load_native_format_falls_back_to_project_path_aliases_in_order() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "native-alias-team";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");

        let native_json = r#"{
  "schema_version": 1,
  "name": "native-alias-team",
  "created_at": "2026-03-01T21:00:00Z",
  "members": [
    {
      "name": "canonical-first",
      "role": "lead",
      "project_path": "/tmp/canonical",
      "projectPath": "/tmp/ignored-camel",
      "cwd": "/tmp/ignored-cwd",
      "cli_tool": "claude"
    },
    {
      "name": "camel-fallback",
      "role": "agent",
      "projectPath": "/tmp/camel-only",
      "cli_tool": "codex"
    },
    {
      "name": "cwd-fallback",
      "role": "agent",
      "cwd": "/tmp/cwd-only",
      "cli_tool": "gemini"
    }
  ]
}"#;
        fs::write(dir.join(CONFIG_FILENAME), native_json).expect("write native config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load should succeed");
        assert_eq!(
            config.members[0].project_path,
            PathBuf::from("/tmp/canonical")
        );
        assert_eq!(
            config.members[1].project_path,
            PathBuf::from("/tmp/camel-only")
        );
        assert_eq!(
            config.members[2].project_path,
            PathBuf::from("/tmp/cwd-only")
        );
    }

    #[test]
    fn load_missing_config_returns_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let err = TeamConfigStore::load(tmp.path(), "missing-team").expect_err("missing team");
        match err {
            CoordinationError::NotFound(message) => assert!(message.contains("missing-team")),
            other => panic!("expected not found, got {other:?}"),
        }
    }

    #[test]
    fn list_returns_io_error_when_teams_dir_is_not_a_directory() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("not-a-dir");
        fs::write(&file_path, "x").expect("write file");

        let err = TeamConfigStore::list(&file_path).expect_err("file path should error");
        match err {
            CoordinationError::Io(io) => {
                assert!(
                    io.kind() == std::io::ErrorKind::NotADirectory
                        || io.kind() == std::io::ErrorKind::Other
                );
            }
            other => panic!("expected io error, got {other:?}"),
        }
    }

    #[test]
    fn discover_resolves_lead_project_anchor() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let discovery = TeamConfigStore::discover(teams_dir).expect("discover should succeed");

        assert_eq!(discovery.warnings.len(), 0);
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, team_name);
        assert_eq!(
            discovery.teams[0].lead_project_path.as_deref(),
            Some(Path::new("/tmp/taurhaus"))
        );
    }

    #[test]
    fn discover_skips_corrupt_folder_with_warning() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let valid_team = "alpha";
        TeamConfigStore::save(teams_dir, valid_team, &sample_config(valid_team))
            .expect("save valid team");

        let corrupt_team = "broken-team";
        let corrupt_dir = team_dir(teams_dir, corrupt_team);
        fs::create_dir_all(&corrupt_dir).expect("create corrupt dir");
        fs::write(corrupt_dir.join(CONFIG_FILENAME), "{ invalid json").expect("write corrupt");

        let discovery = TeamConfigStore::discover(teams_dir).expect("discover should succeed");
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, valid_team);
        assert_eq!(discovery.warnings.len(), 1);
        assert!(discovery.warnings[0].contains(corrupt_team));
    }

    #[test]
    fn discover_skips_missing_config_folder_without_warning() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let valid_team = "alpha";
        TeamConfigStore::save(teams_dir, valid_team, &sample_config(valid_team))
            .expect("save valid team");

        let empty_team = "empty-team";
        fs::create_dir_all(team_dir(teams_dir, empty_team)).expect("create empty team dir");

        let discovery = TeamConfigStore::discover(teams_dir).expect("discover should succeed");
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, valid_team);
        assert!(
            discovery.warnings.is_empty(),
            "missing config directories should be silently skipped"
        );
    }

    #[test]
    fn discover_uses_first_member_when_no_lead_role_present() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "agent-only-team";
        let config = TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: test_timestamp(),
            members: vec![Member {
                name: "member-a".to_string(),
                role: MemberRole::Agent,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                model: None,
                reasoning_effort: None,
                project_path: PathBuf::from("/tmp/agent-a"),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            }],
            extra: Default::default(),
        };

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save");
        let discovery = TeamConfigStore::discover(teams_dir).expect("discover");
        assert_eq!(
            discovery.teams[0].lead_project_path.as_deref(),
            Some(Path::new("/tmp/agent-a"))
        );
    }
}
