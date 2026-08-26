//! Claude Code and Codex hook bridge for post-compaction operational reinjection.

use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::coordination::domain::Member;
use crate::coordination::errors::CoordinationError;
use crate::coordination::reinjection::CompactionReinjectionService;
use crate::coordination::stores::{
    record_delivery_at, CompactionDeliveryResult, MemberRuntimeStore,
    OperationalContextSnapshotStore, TeamConfigStore,
};
use crate::provider::path;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::CliTool;
use taurhaus_lib::logging::emit_global;

const TAURHAUS_COMPACT_HOOK_BASENAME: &str = "taurhaus-session-start-compact";
const CLAUDE_SETTINGS_FILENAME: &str = "settings.json";
const CODEX_HOOKS_FILENAME: &str = "hooks.json";
const SESSION_START_HOOK_EVENT: &str = "SessionStart";
const COMPACT_SOURCE: &str = "compact";
pub(crate) const CODEX_ADDITIONAL_CONTEXT_LIMIT: u64 = 12_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookRuntime {
    Posix,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct CompactHookInput {
    #[serde(alias = "hookEventName")]
    hook_event_name: String,
    #[serde(alias = "sessionId")]
    session_id: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    trigger: Option<String>,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default, alias = "transcriptPath")]
    transcript_path: Option<PathBuf>,
    #[serde(default, alias = "permissionMode")]
    permission_mode: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    agent_type: Option<String>,
}

impl CompactHookInput {
    fn inferred_tool(&self) -> Option<CliTool> {
        infer_tool_from_transcript_path(self.transcript_path.as_deref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CompactHookResponse {
    #[serde(rename = "hookSpecificOutput", skip_serializing_if = "Option::is_none")]
    hook_specific_output: Option<CompactHookSpecificOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompactHookSpecificOutput {
    #[serde(rename = "hookEventName")]
    hook_event_name: String,
    #[serde(rename = "additionalContext")]
    additional_context: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HookMemberMatch {
    team_name: String,
    member: Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactHookSkipReason {
    NonCompactSessionStart,
    ToolInferenceUnavailable,
    NoManagedMemberMatch,
    MultipleManagedMembersMatched,
    MissingOperationalSnapshot,
    NoResumableTaskContext,
}

impl CompactHookSkipReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::NonCompactSessionStart => "non_compact_session_start",
            Self::ToolInferenceUnavailable => "tool_inference_unavailable",
            Self::NoManagedMemberMatch => "no_managed_member_match",
            Self::MultipleManagedMembersMatched => "multiple_managed_members_matched",
            Self::MissingOperationalSnapshot => "missing_operational_snapshot",
            Self::NoResumableTaskContext => "no_resumable_task_context",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactHookFailureStage {
    ReadStdin,
    ParsePayload,
    RenderAdditionalContext,
    RecordDelivery,
    SerializeResponse,
}

impl CompactHookFailureStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReadStdin => "read_stdin",
            Self::ParsePayload => "parse_payload",
            Self::RenderAdditionalContext => "render_additional_context",
            Self::RecordDelivery => "record_delivery",
            Self::SerializeResponse => "serialize_response",
        }
    }
}

/// Harness-specific hook configuration. Claude and Codex are the two current
/// installer implementations; their stable payload fields share one parser.
pub(crate) trait CompactionSignalSource {
    fn install(&self, taurhaus_exe: &Path) -> Result<bool, CoordinationError>;
    fn remove(&self) -> Result<bool, CoordinationError>;
}

fn parse_compact_hook_input(raw: &str) -> Result<CompactHookInput, serde_json::Error> {
    serde_json::from_str(raw)
}

struct ClaudeCompactionSignalSource<'a> {
    claude_dir: &'a Path,
}

impl CompactionSignalSource for ClaudeCompactionSignalSource<'_> {
    fn install(&self, taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
        ensure_source_installed(
            self.claude_dir,
            CLAUDE_SETTINGS_FILENAME,
            taurhaus_exe,
            None,
        )
    }

    fn remove(&self) -> Result<bool, CoordinationError> {
        remove_source_hook(self.claude_dir, CLAUDE_SETTINGS_FILENAME)
    }
}

struct CodexCompactionSignalSource<'a> {
    codex_home: &'a Path,
}

impl CompactionSignalSource for CodexCompactionSignalSource<'_> {
    fn install(&self, taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
        ensure_source_installed(
            self.codex_home,
            CODEX_HOOKS_FILENAME,
            taurhaus_exe,
            Some(CODEX_ADDITIONAL_CONTEXT_LIMIT),
        )
    }

    fn remove(&self) -> Result<bool, CoordinationError> {
        remove_source_hook(self.codex_home, CODEX_HOOKS_FILENAME)
    }
}

pub fn handle_compact_hook_stdin<R: Read>(
    mut stdin: R,
    teams_dir: &Path,
) -> Result<CompactHookResponse, CoordinationError> {
    let mut raw = String::new();
    stdin.read_to_string(&mut raw).map_err(|error| {
        emit_compact_hook_failed(
            CompactHookFailureStage::ReadStdin,
            None,
            None,
            None,
            None,
            Some(raw.len()),
            &error.to_string(),
        );
        CoordinationError::Io(error)
    })?;
    handle_compact_hook(&raw, teams_dir)
}

pub fn handle_compact_hook(
    raw: &str,
    teams_dir: &Path,
) -> Result<CompactHookResponse, CoordinationError> {
    let payload = parse_compact_hook_input(raw).map_err(|err| {
        emit_compact_hook_parse_payload_debug(raw, &err.to_string());
        emit_compact_hook_failed(
            CompactHookFailureStage::ParsePayload,
            None,
            None,
            None,
            None,
            Some(raw.len()),
            &err.to_string(),
        );
        CoordinationError::Validation(format!("invalid compact hook payload: {err}"))
    })?;

    emit_compact_hook_received(&payload, raw.len());

    if payload.hook_event_name != SESSION_START_HOOK_EVENT
        || payload.source.as_deref() != Some(COMPACT_SOURCE)
    {
        emit_compact_hook_skipped(
            &payload,
            None,
            CompactHookSkipReason::NonCompactSessionStart,
        );
        return Ok(CompactHookResponse::default());
    }

    let Some(tool) = payload.inferred_tool() else {
        emit_compact_hook_skipped(
            &payload,
            None,
            CompactHookSkipReason::ToolInferenceUnavailable,
        );
        return Ok(CompactHookResponse::default());
    };

    let matched = match resolve_member_match(teams_dir, tool, &payload)? {
        Ok(matched) => matched,
        Err(reason) => {
            emit_compact_hook_skipped(&payload, None, reason);
            return Ok(CompactHookResponse::default());
        }
    };

    emit_compact_hook_resolved(&payload, &matched);

    let compaction_timestamp = payload
        .transcript_path
        .as_deref()
        .and_then(crate::session_scanner::compaction_extractor::latest_compaction_timestamp)
        .unwrap_or_else(Utc::now);

    let Some(snapshot) =
        OperationalContextSnapshotStore::load(teams_dir, &matched.team_name, &matched.member.name)?
    else {
        record_delivery_at(
            teams_dir,
            &matched.team_name,
            &matched.member.name,
            tool,
            &payload.session_id,
            compaction_timestamp,
            CompactionDeliveryResult::Skipped,
        )
        .inspect_err(|error| {
            emit_compact_hook_failed(
                CompactHookFailureStage::RecordDelivery,
                Some(&payload),
                Some(&matched),
                None,
                None,
                None,
                &error.to_string(),
            );
        })?;
        emit_compact_hook_skipped(
            &payload,
            Some(&matched),
            CompactHookSkipReason::MissingOperationalSnapshot,
        );
        return Ok(CompactHookResponse::default());
    };

    if !CompactionReinjectionService::snapshot_has_resumable_task(&snapshot) {
        record_delivery_at(
            teams_dir,
            &matched.team_name,
            &matched.member.name,
            tool,
            &payload.session_id,
            compaction_timestamp,
            CompactionDeliveryResult::Skipped,
        )
        .inspect_err(|error| {
            emit_compact_hook_failed(
                CompactHookFailureStage::RecordDelivery,
                Some(&payload),
                Some(&matched),
                None,
                None,
                None,
                &error.to_string(),
            );
        })?;
        emit_compact_hook_skipped(
            &payload,
            Some(&matched),
            CompactHookSkipReason::NoResumableTaskContext,
        );
        return Ok(CompactHookResponse::default());
    }

    let card = CompactionReinjectionService::compose(&matched.member, &snapshot);
    let additional_context = CompactionReinjectionService::render_additional_context_text(&card)
        .map_err(|err| {
            emit_compact_hook_failed(
                CompactHookFailureStage::RenderAdditionalContext,
                Some(&payload),
                Some(&matched),
                None,
                None,
                None,
                &err.to_string(),
            );
            CoordinationError::StoreError(format!(
                "failed to render compact hook additional context for '{}': {err}",
                matched.member.name
            ))
        })?;

    record_delivery_at(
        teams_dir,
        &matched.team_name,
        &matched.member.name,
        tool,
        &payload.session_id,
        compaction_timestamp,
        CompactionDeliveryResult::Injected,
    )
    .inspect_err(|error| {
        emit_compact_hook_failed(
            CompactHookFailureStage::RecordDelivery,
            Some(&payload),
            Some(&matched),
            None,
            None,
            None,
            &error.to_string(),
        );
    })?;

    emit_compact_hook_delivered(&payload, &matched, additional_context.len());

    Ok(CompactHookResponse {
        hook_specific_output: Some(CompactHookSpecificOutput {
            hook_event_name: SESSION_START_HOOK_EVENT.to_string(),
            additional_context,
        }),
    })
}

pub fn ensure_compact_hook_installed(
    teams_dir: &Path,
    taurhaus_exe: &Path,
) -> Result<bool, CoordinationError> {
    let Some(claude_dir) = teams_dir.parent() else {
        return Err(CoordinationError::Validation(format!(
            "team directory '{}' has no parent Claude dir",
            teams_dir.display()
        )));
    };

    ClaudeCompactionSignalSource { claude_dir }.install(taurhaus_exe)
}

pub fn ensure_codex_compact_hook_installed(taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
    ensure_codex_compact_hook_installed_at(&PlatformPaths::codex_dir(), taurhaus_exe)
}

pub fn ensure_codex_compact_hook_installed_at(
    codex_home: &Path,
    taurhaus_exe: &Path,
) -> Result<bool, CoordinationError> {
    CodexCompactionSignalSource { codex_home }.install(taurhaus_exe)
}

pub fn remove_codex_compact_hook() -> Result<bool, CoordinationError> {
    remove_codex_compact_hook_at(&PlatformPaths::codex_dir())
}

pub fn remove_codex_compact_hook_at(codex_home: &Path) -> Result<bool, CoordinationError> {
    CodexCompactionSignalSource { codex_home }.remove()
}

pub fn codex_compact_hook_is_installed() -> bool {
    source_hook_is_installed(&PlatformPaths::codex_dir(), CODEX_HOOKS_FILENAME)
}

pub fn team_has_managed_claude_member(
    teams_dir: &Path,
    team_name: &str,
) -> Result<bool, CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    Ok(config
        .members
        .iter()
        .any(|member| member.cli_tool == CliTool::Claude))
}

pub fn team_has_managed_codex_member(
    teams_dir: &Path,
    team_name: &str,
) -> Result<bool, CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    Ok(config
        .members
        .iter()
        .any(|member| member.cli_tool == CliTool::Codex))
}

pub fn any_managed_codex_member(teams_dir: &Path) -> Result<bool, CoordinationError> {
    for team_name in TeamConfigStore::list(teams_dir)? {
        match team_has_managed_codex_member(teams_dir, &team_name) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(
                    team_name,
                    error = %error,
                    "skipping invalid team config during managed Codex discovery"
                );
            }
        }
    }
    Ok(false)
}

fn resolve_member_match(
    teams_dir: &Path,
    tool: CliTool,
    payload: &CompactHookInput,
) -> Result<Result<HookMemberMatch, CompactHookSkipReason>, CoordinationError> {
    let mut candidates = Vec::new();

    for team_name in TeamConfigStore::list(teams_dir)? {
        let config = match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    team_name = team_name,
                    error = %err,
                    "failed to load team config during compact hook resolution"
                );
                continue;
            }
        };

        for member in config.members {
            if member.cli_tool != tool {
                continue;
            }
            let runtime_session_id = MemberRuntimeStore::load(teams_dir, &team_name, &member.name)
                .ok()
                .and_then(|runtime| runtime.session_id);
            candidates.push((
                HookMemberMatch {
                    team_name: team_name.clone(),
                    member,
                },
                runtime_session_id,
            ));
        }
    }

    let session_matches = candidates
        .iter()
        .filter(|(_, runtime_session_id)| {
            runtime_session_id.as_deref() == Some(payload.session_id.as_str())
        })
        .map(|(matched, _)| matched.clone())
        .collect::<Vec<_>>();
    if session_matches.len() == 1 {
        return Ok(Ok(session_matches
            .into_iter()
            .next()
            .expect("single match")));
    }
    if session_matches.len() > 1 {
        tracing::warn!(
            session_id = %payload.session_id,
            tool = %tool,
            "multiple members matched compact hook payload by runtime session; skipping reinjection"
        );
        return Ok(Err(CompactHookSkipReason::MultipleManagedMembersMatched));
    }

    let cwd_matches = candidates
        .into_iter()
        .filter(|(matched, _)| {
            cwd_matches_member(payload.cwd.as_deref(), &matched.member.project_path)
        })
        .map(|(matched, _)| matched)
        .collect::<Vec<_>>();
    match cwd_matches.len() {
        1 => Ok(Ok(cwd_matches.into_iter().next().expect("single match"))),
        0 => Ok(Err(CompactHookSkipReason::NoManagedMemberMatch)),
        _ => Ok(Err(CompactHookSkipReason::MultipleManagedMembersMatched)),
    }
}

fn infer_tool_from_transcript_path(transcript_path: Option<&Path>) -> Option<CliTool> {
    let transcript_path = transcript_path?;
    let normalized = transcript_path.to_string_lossy().replace('\\', "/");
    let normalized = normalized.to_ascii_lowercase();
    let file_name = transcript_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if normalized.contains("/.codex/") || file_name.starts_with("rollout-") {
        return Some(CliTool::Codex);
    }
    if normalized.contains("/.claude") || normalized.contains("/projects/") {
        return Some(CliTool::Claude);
    }
    None
}

pub fn handle_session_start_hook(
    raw: &str,
    teams_dir: &Path,
) -> Result<CompactHookResponse, CoordinationError> {
    handle_compact_hook(raw, teams_dir)
}

pub fn run_compact_hook_cli<R: Read, W: Write>(
    stdin: R,
    mut stdout: W,
    teams_dir: &Path,
) -> Result<(), CoordinationError> {
    let response = handle_compact_hook_stdin(stdin, teams_dir)?;
    serde_json::to_writer(&mut stdout, &response).map_err(|error| {
        CoordinationError::StoreError(format!(
            "failed to serialize compact hook response: {error}"
        ))
    })?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

pub fn emit_compact_hook_cli_failed(error_message: &str) {
    emit_compact_hook_failed(
        CompactHookFailureStage::SerializeResponse,
        None,
        None,
        None,
        None,
        None,
        error_message,
    );
}

fn emit_compact_hook_received(payload: &CompactHookInput, raw_bytes: usize) {
    let mut fields = base_compact_hook_fields(Some(payload), None);
    fields.insert("raw_bytes".to_string(), Value::from(raw_bytes as u64));
    let event = hook_event_name(payload.inferred_tool(), "received");
    emit_global(
        "info",
        "coordination",
        &event,
        Some("Compact hook payload received".to_string()),
        fields,
    );
}

fn emit_compact_hook_resolved(payload: &CompactHookInput, matched: &HookMemberMatch) {
    let event = hook_event_name(Some(matched.member.cli_tool), "resolved");
    emit_global(
        "info",
        "coordination",
        &event,
        Some("Compact hook matched managed member".to_string()),
        base_compact_hook_fields(Some(payload), Some(matched)),
    );
}

fn emit_compact_hook_delivered(
    payload: &CompactHookInput,
    matched: &HookMemberMatch,
    additional_context_bytes: usize,
) {
    let mut fields = base_compact_hook_fields(Some(payload), Some(matched));
    fields.insert(
        "additional_context_bytes".to_string(),
        Value::from(additional_context_bytes as u64),
    );
    let event = hook_event_name(Some(matched.member.cli_tool), "delivered");
    emit_global(
        "info",
        "coordination",
        &event,
        Some("Compact hook returned additional context".to_string()),
        fields,
    );
}

fn emit_compact_hook_skipped(
    payload: &CompactHookInput,
    matched: Option<&HookMemberMatch>,
    reason: CompactHookSkipReason,
) {
    let mut fields = base_compact_hook_fields(Some(payload), matched);
    fields.insert(
        "skip_reason".to_string(),
        Value::String(reason.as_str().to_string()),
    );
    let event = hook_event_name(payload.inferred_tool(), "skipped");
    emit_global(
        "info",
        "coordination",
        &event,
        Some("Compact hook did not return additional context".to_string()),
        fields,
    );
}

fn emit_compact_hook_failed(
    stage: CompactHookFailureStage,
    payload: Option<&CompactHookInput>,
    matched: Option<&HookMemberMatch>,
    session_id: Option<&str>,
    cwd: Option<&Path>,
    raw_bytes: Option<usize>,
    error_message: &str,
) {
    let mut fields = base_compact_hook_fields(payload, matched);
    if payload.is_none() {
        insert_optional_string(&mut fields, "session_id", session_id.map(ToOwned::to_owned));
        insert_optional_string(&mut fields, "cwd", cwd.map(path_display));
    }
    if let Some(raw_bytes) = raw_bytes {
        fields.insert("raw_bytes".to_string(), Value::from(raw_bytes as u64));
    }
    fields.insert(
        "failure_stage".to_string(),
        Value::String(stage.as_str().to_string()),
    );
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    let tool = matched
        .map(|matched| matched.member.cli_tool)
        .or_else(|| payload.and_then(CompactHookInput::inferred_tool));
    let event = hook_event_name(tool, "failed");
    emit_global(
        "warn",
        "coordination",
        &event,
        Some("Compact hook bridge failed".to_string()),
        fields,
    );
}

fn emit_compact_hook_parse_payload_debug(raw: &str, error_message: &str) {
    let mut fields = Map::new();
    fields.insert(
        "error.message".to_string(),
        Value::String(error_message.to_string()),
    );
    fields.insert("raw_payload".to_string(), Value::String(raw.to_string()));
    fields.insert("raw_bytes".to_string(), Value::from(raw.len() as u64));
    emit_global(
        "debug",
        "coordination",
        "compaction.compact_hook.parse_payload_debug",
        Some("Compact hook payload parse failed".to_string()),
        fields,
    );
}

fn base_compact_hook_fields(
    payload: Option<&CompactHookInput>,
    matched: Option<&HookMemberMatch>,
) -> Map<String, Value> {
    let mut fields = Map::new();
    let tool = matched
        .map(|matched| matched.member.cli_tool)
        .or_else(|| payload.and_then(CompactHookInput::inferred_tool));
    if let Some(tool) = tool {
        fields.insert("tool".to_string(), Value::String(tool.to_string()));
    }
    if let Some(payload) = payload {
        fields.insert(
            "hook_event_name".to_string(),
            Value::String(payload.hook_event_name.clone()),
        );
        insert_optional_string(&mut fields, "source", payload.source.clone());
        insert_optional_string(&mut fields, "trigger", payload.trigger.clone());
        insert_optional_string(&mut fields, "session_id", Some(payload.session_id.clone()));
        insert_optional_string(&mut fields, "cwd", payload.cwd.as_deref().map(path_display));
        insert_optional_string(
            &mut fields,
            "transcript_path",
            payload.transcript_path.as_deref().map(path_display),
        );
        insert_optional_string(
            &mut fields,
            "permission_mode",
            payload.permission_mode.clone(),
        );
        insert_optional_string(&mut fields, "model", payload.model.clone());
        insert_optional_string(&mut fields, "agent_type", payload.agent_type.clone());
    }
    if let Some(matched) = matched {
        insert_optional_string(&mut fields, "team_name", Some(matched.team_name.clone()));
        insert_optional_string(
            &mut fields,
            "member_name",
            Some(matched.member.name.clone()),
        );
        insert_optional_string(
            &mut fields,
            "project_path",
            Some(matched.member.project_path.display().to_string()),
        );
    }
    fields
}

fn hook_event_name(tool: Option<CliTool>, action: &str) -> String {
    let source = tool
        .map(|tool| tool.to_string())
        .unwrap_or_else(|| "compact".to_string());
    format!("compaction.{source}_hook.{action}")
}

fn insert_optional_string(fields: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        fields.insert(key.to_string(), Value::String(value));
    }
}

fn path_display(path: &Path) -> String {
    path.display().to_string()
}

fn cwd_matches_member(cwd: Option<&Path>, member_project_path: &Path) -> bool {
    let Some(cwd) = cwd else {
        return true;
    };

    if path::normalize_project_path(&cwd.to_string_lossy())
        == path::normalize_project_path(&member_project_path.to_string_lossy())
    {
        return true;
    }

    match (fs::canonicalize(cwd), fs::canonicalize(member_project_path)) {
        (Ok(canonical_cwd), Ok(canonical_member)) => canonical_cwd == canonical_member,
        _ => false,
    }
}

fn ensure_source_installed(
    config_dir: &Path,
    settings_filename: &str,
    taurhaus_exe: &Path,
    additional_context_limit: Option<u64>,
) -> Result<bool, CoordinationError> {
    let hooks_dir = config_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let runtime = detect_hook_runtime(config_dir);
    let script_path = hooks_dir.join(platform_hook_filename(runtime));
    let script_changed = write_hook_script(&script_path, taurhaus_exe, runtime)?;
    let settings_changed = ensure_settings_hook_entry(
        &config_dir.join(settings_filename),
        &script_path,
        runtime,
        additional_context_limit,
    )?;
    Ok(script_changed || settings_changed)
}

fn remove_source_hook(
    config_dir: &Path,
    settings_filename: &str,
) -> Result<bool, CoordinationError> {
    let settings_path = config_dir.join(settings_filename);
    let mut settings = load_settings_json(&settings_path)?;
    let original = settings.clone();

    if let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) {
        if let Some(entries) = hooks
            .get_mut(SESSION_START_HOOK_EVENT)
            .and_then(Value::as_array_mut)
        {
            for entry in entries.iter_mut() {
                if let Some(hook_array) = entry.get_mut("hooks").and_then(Value::as_array_mut) {
                    hook_array.retain(|hook| !is_taurhaus_compact_hook(hook));
                }
            }
            entries.retain(|entry| {
                entry
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_none_or(|hook_array| !hook_array.is_empty())
            });
            if entries.is_empty() {
                hooks.remove(SESSION_START_HOOK_EVENT);
            }
        }
        if hooks.is_empty() {
            settings
                .as_object_mut()
                .expect("settings root remains an object")
                .remove("hooks");
        }
    }

    let settings_changed = settings != original;
    if settings_changed {
        let payload = serde_json::to_vec_pretty(&settings).map_err(|error| {
            CoordinationError::StoreError(format!(
                "failed to serialize hook settings '{}': {error}",
                settings_path.display()
            ))
        })?;
        write_atomic_settings_file(&settings_path, &payload)?;
    }

    let runtime = detect_hook_runtime(config_dir);
    let script_path = config_dir
        .join("hooks")
        .join(platform_hook_filename(runtime));
    let script_changed = match fs::remove_file(&script_path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(CoordinationError::Io(error)),
    };
    Ok(settings_changed || script_changed)
}

fn source_hook_is_installed(config_dir: &Path, settings_filename: &str) -> bool {
    let runtime = detect_hook_runtime(config_dir);
    let script_path = config_dir
        .join("hooks")
        .join(platform_hook_filename(runtime));
    let script_is_current =
        fs::read_to_string(&script_path).is_ok_and(|script| script.contains("--compact-hook"));
    let settings_contains_hook = load_settings_json(&config_dir.join(settings_filename))
        .ok()
        .and_then(|settings| {
            settings
                .get("hooks")?
                .get(SESSION_START_HOOK_EVENT)?
                .as_array()
                .map(|entries| {
                    entries.iter().any(|entry| {
                        entry
                            .get("hooks")
                            .and_then(Value::as_array)
                            .is_some_and(|hooks| hooks.iter().any(is_taurhaus_compact_hook))
                    })
                })
        })
        .unwrap_or(false);
    script_is_current && settings_contains_hook
}

fn write_hook_script(
    script_path: &Path,
    taurhaus_exe: &Path,
    runtime: HookRuntime,
) -> Result<bool, CoordinationError> {
    let script_body = render_hook_script(taurhaus_exe, runtime)?;
    let changed = fs::read(script_path)
        .map(|current| current != script_body.as_bytes())
        .unwrap_or(true);
    if changed {
        fs::write(script_path, script_body.as_bytes())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(script_path, perms)?;
    }
    Ok(changed)
}

fn render_hook_script(
    taurhaus_exe: &Path,
    runtime: HookRuntime,
) -> Result<String, CoordinationError> {
    let executable = runtime_path_string(taurhaus_exe, runtime)?;
    Ok(match runtime {
        HookRuntime::Windows => {
            format!("@echo off\r\n\"{}\" --compact-hook\r\n", executable)
        }
        HookRuntime::Posix => format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nexec {} --compact-hook\n",
            shell_quote_string(&executable)
        ),
    })
}

fn ensure_settings_hook_entry(
    settings_path: &Path,
    script_path: &Path,
    runtime: HookRuntime,
    additional_context_limit: Option<u64>,
) -> Result<bool, CoordinationError> {
    let mut settings = load_settings_json(settings_path)?;
    let original_settings = settings.clone();
    let command = settings_command_for_script(script_path, runtime)?;

    let root = settings.as_object_mut().ok_or_else(|| {
        CoordinationError::StoreError(format!(
            "Hook settings at '{}' are not a JSON object",
            settings_path.display()
        ))
    })?;

    let hooks = root
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let hooks_obj = hooks.as_object_mut().ok_or_else(|| {
        CoordinationError::StoreError(format!(
            "Hook settings 'hooks' in '{}' are not a JSON object",
            settings_path.display()
        ))
    })?;

    let session_start = hooks_obj
        .entry(SESSION_START_HOOK_EVENT.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let entries = session_start.as_array_mut().ok_or_else(|| {
        CoordinationError::StoreError(format!(
            "Hook settings 'hooks.{SESSION_START_HOOK_EVENT}' in '{}' are not an array",
            settings_path.display()
        ))
    })?;

    remove_existing_taurhaus_compact_hooks(entries);

    let mut inserted = false;
    for entry in entries.iter_mut() {
        let Some(entry_obj) = entry.as_object_mut() else {
            continue;
        };
        if entry_obj.get("matcher").and_then(Value::as_str) != Some(COMPACT_SOURCE) {
            continue;
        }

        let hooks_value = entry_obj
            .entry("hooks".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        let hook_array = hooks_value.as_array_mut().ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "Hook settings compact SessionStart hooks in '{}' are not an array",
                settings_path.display()
            ))
        })?;
        hook_array.push(command_hook_value(&command, additional_context_limit));
        inserted = true;
        break;
    }

    if !inserted {
        entries.push(json!({
            "matcher": COMPACT_SOURCE,
            "hooks": [command_hook_value(&command, additional_context_limit)],
        }));
    }

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let changed = settings != original_settings;
    if changed {
        let payload = serde_json::to_vec_pretty(&settings).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize hook settings '{}': {err}",
                settings_path.display()
            ))
        })?;
        write_atomic_settings_file(settings_path, &payload)?;
    }
    Ok(changed)
}

fn write_atomic_settings_file(
    settings_path: &Path,
    payload: &[u8],
) -> Result<(), CoordinationError> {
    let Some(parent) = settings_path.parent() else {
        return Err(CoordinationError::Validation(format!(
            "Hook settings path '{}' has no parent directory",
            settings_path.display()
        )));
    };

    fs::create_dir_all(parent)?;
    let tmp_path = temp_path_for(settings_path);
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp_path)?;
    file.write_all(payload)?;
    file.sync_all()?;
    drop(file);

    if let Err(err) = fs::rename(&tmp_path, settings_path) {
        if is_windows_unsupported_rename_error(&err) {
            fs::write(settings_path, payload)?;
            let _ = fs::remove_file(&tmp_path);
            return Ok(());
        }

        let _ = fs::remove_file(&tmp_path);
        return Err(CoordinationError::Io(err));
    }

    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let random_suffix = format!("{:016x}", rand::thread_rng().next_u64());
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings");
    path.with_file_name(format!("{file_name}.tmp.{random_suffix}"))
}

fn is_windows_unsupported_rename_error(err: &std::io::Error) -> bool {
    cfg!(target_os = "windows") && err.raw_os_error() == Some(1)
}

fn remove_existing_taurhaus_compact_hooks(entries: &mut [Value]) {
    for entry in entries.iter_mut() {
        let Some(entry_obj) = entry.as_object_mut() else {
            continue;
        };
        let Some(hooks) = entry_obj.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        hooks.retain(|hook| !is_taurhaus_compact_hook(hook));
    }
}

fn is_taurhaus_compact_hook(hook: &Value) -> bool {
    let Some(hook_obj) = hook.as_object() else {
        return false;
    };
    if hook_obj.get("type").and_then(Value::as_str) != Some("command") {
        return false;
    }
    let command = hook_obj
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    command.contains(TAURHAUS_COMPACT_HOOK_BASENAME)
}

fn command_hook_value(command: &str, additional_context_limit: Option<u64>) -> Value {
    let mut hook = json!({
        "type": "command",
        "command": command,
    });
    if let Some(limit) = additional_context_limit {
        hook["additionalContextLimit"] = Value::from(limit);
    }
    hook
}

fn load_settings_json(settings_path: &Path) -> Result<Value, CoordinationError> {
    let raw = match fs::read_to_string(settings_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(err) => return Err(CoordinationError::Io(err)),
    };
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to parse hook settings '{}': {err}",
            settings_path.display()
        ))
    })
}

fn platform_hook_filename(runtime: HookRuntime) -> String {
    match runtime {
        HookRuntime::Windows => format!("{TAURHAUS_COMPACT_HOOK_BASENAME}.cmd"),
        HookRuntime::Posix => format!("{TAURHAUS_COMPACT_HOOK_BASENAME}.sh"),
    }
}

fn settings_command_for_script(
    script_path: &Path,
    runtime: HookRuntime,
) -> Result<String, CoordinationError> {
    let script = runtime_path_string(script_path, runtime)?;
    Ok(match runtime {
        HookRuntime::Windows => format!("\"{}\"", script),
        HookRuntime::Posix => format!("bash {}", shell_quote_string(&script)),
    })
}

fn shell_quote_string(value: &str) -> String {
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn detect_hook_runtime(config_dir: &Path) -> HookRuntime {
    let value = config_dir.display().to_string();
    if value.starts_with('/') || path::is_wsl_path(&value) {
        return HookRuntime::Posix;
    }
    if path::is_windows_drive_path(&value) {
        return HookRuntime::Windows;
    }
    HookRuntime::Posix
}

fn runtime_path_string(
    path_value: &Path,
    runtime: HookRuntime,
) -> Result<String, CoordinationError> {
    let value = path_value.display().to_string();
    match runtime {
        HookRuntime::Posix => {
            if value.starts_with('/') {
                return Ok(value);
            }
            path::to_linux(&value).ok_or_else(|| {
                CoordinationError::Validation(format!(
                    "path '{}' is not executable from a POSIX hook runtime",
                    path_value.display()
                ))
            })
        }
        HookRuntime::Windows => {
            if path::is_windows_drive_path(&value) {
                return Ok(value);
            }
            path::linux_mount_to_windows(&value).ok_or_else(|| {
                CoordinationError::Validation(format!(
                    "path '{}' is not executable from a Windows hook runtime",
                    path_value.display()
                ))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::DateTime;
    use fs2::FileExt;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    use crate::coordination::domain::{HealthState, MemberRole};
    use crate::coordination::stores::{
        MemberCompactionStore, MemberRuntimeRecord, OperationalAssignmentFooterSnapshot,
        OperationalContextSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfig,
    };
    use taurhaus_lib::logging::{install_global_sink, LogFileState};

    const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    struct EnvTestGuard {
        _in_process: MutexGuard<'static, ()>,
        lock_file: std::fs::File,
        previous_override: Option<OsString>,
    }

    impl EnvTestGuard {
        fn set_override(&self, value: impl AsRef<std::ffi::OsStr>) {
            std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, value);
        }
    }

    impl Drop for EnvTestGuard {
        fn drop(&mut self) {
            match self.previous_override.as_ref() {
                Some(previous) => std::env::set_var(CLAUDE_DIR_OVERRIDE_ENV, previous),
                None => std::env::remove_var(CLAUDE_DIR_OVERRIDE_ENV),
            }
            let _ = self.lock_file.unlock();
        }
    }

    fn acquire_env_test_guard() -> EnvTestGuard {
        let in_process = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let lock_path = std::env::temp_dir().join("taurhaus-claude-hooks-env-tests.lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap_or_else(|e| panic!("failed to open env test lock at {:?}: {e}", lock_path));
        lock_file
            .lock_exclusive()
            .unwrap_or_else(|e| panic!("failed to lock env test lock at {:?}: {e}", lock_path));
        EnvTestGuard {
            _in_process: in_process,
            lock_file,
            previous_override: std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV),
        }
    }

    fn sample_member(project_path: &Path) -> Member {
        Member {
            name: "architect".to_string(),
            role: MemberRole::Agent,
            role_id: Some("taurhaus-architect".to_string()),
            role_name: Some("Taurhaus Architect".to_string()),
            focus_area: Some("Cross-layer diagnosis".to_string()),
            context_summary: Some("Keeps context warm.".to_string()),
            behavior_summary: Some("Stay concrete.".to_string()),
            communication_style: None,
            runtime_compact_summary: None,
            instructions: Some("Inspect architecture".to_string()),
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
            project_path: project_path.to_path_buf(),
            cli_tool: CliTool::Claude,
            extra: Default::default(),
        }
    }

    fn write_team_fixture(teams_dir: &Path, team_name: &str, member: &Member, session_id: &str) {
        TeamConfigStore::save(
            teams_dir,
            team_name,
            &TeamConfig {
                schema_version: 1,
                name: team_name.to_string(),
                description: None,
                created_at: DateTime::parse_from_rfc3339("2026-03-08T15:00:00Z")
                    .expect("timestamp")
                    .with_timezone(&Utc),
                members: vec![member.clone()],
                extra: Default::default(),
            },
        )
        .expect("save team");

        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            &member.name,
            &MemberRuntimeRecord {
                schema_version: 3,
                member_name: member.name.clone(),
                cli_tool: Some(member.cli_tool),
                project_path: Some(member.project_path.clone()),
                pane_id: Some("%217".to_string()),
                pane_pid: None,
                pane_start_time: None,
                session_id: Some(session_id.to_string()),
                jsonl_path: None,
                daemon_pid: None,
                health: HealthState::Healthy,
                delivery_lease: None,
                attached_at: None,
                last_seen_at: None,
            },
        )
        .expect("save runtime");
    }

    fn write_snapshot_fixture(teams_dir: &Path, team_name: &str, member_name: &str) {
        OperationalContextSnapshotStore::save(
            teams_dir,
            &OperationalContextSnapshot {
                version: 1,
                team_name: team_name.to_string(),
                member_name: member_name.to_string(),
                updated_at: DateTime::parse_from_rfc3339("2026-03-08T15:01:00Z")
                    .expect("timestamp")
                    .with_timezone(&Utc),
                task: OperationalTaskSnapshot {
                    id: "680".to_string(),
                    subject: "Implement Claude SessionStart(source=compact) hook bridge"
                        .to_string(),
                    status: "in_progress".to_string(),
                },
                assignment_footer: OperationalAssignmentFooterSnapshot {
                    execution_mode: "implement".to_string(),
                    file_ownership_boundary: vec![
                        "src-tauri/src/coordination/compact_hook.rs".to_string()
                    ],
                    adjacent_fix_policy: "no".to_string(),
                    validation_expectation: "cargo check --tests".to_string(),
                    response_expectation: "report-on-completion".to_string(),
                },
                ownership: OperationalOwnershipSnapshot {
                    override_allowed: false,
                    active_override_reason: None,
                },
                working_set: OperationalWorkingSetSnapshot {
                    project_path: "/home/user/projects/taurhaus".to_string(),
                    focal_files: vec!["src-tauri/src/coordination/compact_hook.rs".to_string()],
                },
            },
        )
        .expect("save snapshot");
    }

    #[test]
    fn compact_hook_returns_additional_context_for_legacy_camel_case_payload() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");
        write_snapshot_fixture(tmp.path(), "taurhaus-team", &member.name);

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        let output = response
            .hook_specific_output
            .expect("hook should inject additional context");
        assert_eq!(output.hook_event_name, "SessionStart");
        assert!(output
            .additional_context
            .contains("[taurhaus] restored_working_context_after_compaction"));
        assert!(output.additional_context.contains("Current task: #680"));
    }

    #[test]
    fn compact_hook_returns_additional_context_for_current_snake_case_payload() {
        // Regression: Claude Code now sends snake_case SessionStart hook input with
        // transcript_path / permission_mode / model fields, so the bridge must not
        // require the old camelCase hookEventName/sessionId shape.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let transcript_path = project
            .join(".claude")
            .join("transcripts")
            .join("sess-123.jsonl");
        fs::create_dir_all(transcript_path.parent().expect("transcript dir")).expect("mkdirs");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");
        write_snapshot_fixture(tmp.path(), "taurhaus-team", &member.name);

        let response = handle_session_start_hook(
            &json!({
                "hook_event_name": "SessionStart",
                "session_id": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcript_path": transcript_path,
                "permission_mode": "default",
                "model": "claude-opus-4-1",
            })
            .to_string(),
            tmp.path(),
        )
        .expect("current payload should succeed");

        let output = response
            .hook_specific_output
            .expect("hook should inject additional context");
        assert_eq!(output.hook_event_name, "SessionStart");
        assert!(output.additional_context.contains("Current task: #680"));
    }

    #[test]
    fn compact_hook_emits_received_resolved_and_delivered_events() {
        let guard = acquire_env_test_guard();
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let claude_dir = tmp.path().join("claude");
        guard.set_override(&claude_dir);
        let log_path = tmp.path().join("claude-hook.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");
        write_snapshot_fixture(tmp.path(), "taurhaus-team", &member.name);

        let payload = json!({
            "hookEventName": "SessionStart",
            "sessionId": "sess-123",
            "source": "compact",
            "cwd": &project,
            "transcriptPath": project.join(".claude/projects/session.jsonl"),
        })
        .to_string();

        let response =
            handle_session_start_hook(&payload, tmp.path()).expect("hook should succeed");
        assert!(response.hook_specific_output.is_some());

        // Regression: 0b87699 emitted only Claude hook lifecycle events, leaving
        // the Codex bridge without acceptance telemetry.
        let mut codex_member = sample_member(&project);
        codex_member.name = "codex-architect".to_string();
        codex_member.cli_tool = CliTool::Codex;
        write_team_fixture(tmp.path(), "codex-team", &codex_member, "codex-session");
        write_snapshot_fixture(tmp.path(), "codex-team", &codex_member.name);
        let codex_payload = json!({
            "hook_event_name": "SessionStart",
            "session_id": "codex-session",
            "source": "compact",
            "cwd": &project,
            "transcript_path": project.join(
                ".codex/sessions/2026/08/26/rollout-2026-08-26T10-00-00-codex-session.jsonl"
            ),
        })
        .to_string();
        let response =
            handle_compact_hook(&codex_payload, tmp.path()).expect("Codex hook should succeed");
        assert!(response.hook_specific_output.is_some());

        let contents = read_log_after_flush(
            &log_state,
            &log_path,
            "\"event\":\"compaction.codex_hook.delivered\"",
        );
        assert!(contents.contains("\"event\":\"compaction.claude_hook.received\""));
        assert!(contents.contains("\"event\":\"compaction.claude_hook.resolved\""));
        assert!(contents.contains("\"event\":\"compaction.claude_hook.delivered\""));
        assert!(contents.contains("\"event\":\"compaction.codex_hook.received\""));
        assert!(contents.contains("\"event\":\"compaction.codex_hook.resolved\""));
        assert!(contents.contains("\"event\":\"compaction.codex_hook.delivered\""));
        assert!(contents.contains("\"session_id\":\"sess-123\""));
        assert!(contents.contains("\"team_name\":\"taurhaus-team\""));
        assert!(contents.contains("\"member_name\":\"architect\""));
    }

    #[test]
    fn compact_hook_falls_back_to_cwd_when_runtime_session_is_not_yet_captured() {
        // Regression: 0b87699 required a captured runtime session id; the shared
        // PR 9 resolver must fall back to normalized cwd for a newly compacted session.
        let guard = acquire_env_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let claude_dir = tmp.path().join("claude");
        let team_name = "taurhaus-team-forged";
        guard.set_override(&claude_dir);

        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let mut member = sample_member(&project);
        member.name = "architect-forged".to_string();
        write_team_fixture(tmp.path(), team_name, &member, "sess-123");
        write_snapshot_fixture(tmp.path(), team_name, &member.name);

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "forged-session",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert!(response.hook_specific_output.is_some());
        assert!(
            MemberCompactionStore::load(tmp.path(), team_name, &member.name)
                .expect("load compaction state")
                .is_some()
        );
    }

    #[test]
    fn compact_hook_additional_context_is_well_formed_and_contains_expected_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");
        write_snapshot_fixture(tmp.path(), "taurhaus-team", &member.name);

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        let output = response
            .hook_specific_output
            .expect("hook should inject additional context");
        assert!(output.additional_context.contains("Current task: #680"));
        assert!(output
            .additional_context
            .contains("Role: Taurhaus Architect"));
        assert!(output
            .additional_context
            .contains("Validation expectation: cargo check --tests"));
        assert!(output
            .additional_context
            .contains("src-tauri/src/coordination/compact_hook.rs"));
    }

    #[test]
    fn compact_hook_skips_non_compact_session_start() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "startup",
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, CompactHookResponse::default());
    }

    #[test]
    fn compact_hook_skips_when_snapshot_missing() {
        // Regression: 0b87699b asserted against a process-global async sink by
        // polling for one second, which raced under full parallel test load.
        let guard = acquire_env_test_guard();
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let claude_dir = tmp.path().join("claude");
        guard.set_override(&claude_dir);
        let log_path = tmp.path().join("claude-hook.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, CompactHookResponse::default());
        let contents = read_log_after_flush(
            &log_state,
            &log_path,
            "\"event\":\"compaction.claude_hook.skipped\"",
        );
        assert!(contents.contains("\"skip_reason\":\"missing_operational_snapshot\""));
    }

    #[test]
    fn compact_hook_records_delivery_under_passed_teams_dir() {
        // Regression: 0b87699b made record_delivery resolve ~/.claude/teams again,
        // ignoring the teams_dir already passed to the Claude hook bridge.
        let guard = acquire_env_test_guard();
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let passed_teams_dir = tmp.path().join("passed-teams");
        let unrelated_claude_dir = tmp.path().join("unrelated-claude");
        guard.set_override(&unrelated_claude_dir);
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(&passed_teams_dir, "taurhaus-team", &member, "sess-123");

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            &passed_teams_dir,
        )
        .expect("hook should skip cleanly without a snapshot");

        assert_eq!(response, CompactHookResponse::default());
        assert!(
            MemberCompactionStore::load(&passed_teams_dir, "taurhaus-team", &member.name,)
                .expect("load passed-root state")
                .is_some()
        );
        assert!(MemberCompactionStore::load(
            &unrelated_claude_dir.join("teams"),
            "taurhaus-team",
            &member.name,
        )
        .expect("load unrelated-root state")
        .is_none());
    }

    #[test]
    fn cwd_match_normalizes_wsl_unc_and_linux_project_paths() {
        // Regression: 0b87699b used filesystem canonicalization for hook matching,
        // which cannot equate the app's WSL UNC path with Claude's Linux cwd.
        assert!(cwd_matches_member(
            Some(Path::new("/home/user/projects/taurhaus")),
            Path::new(r"\\wsl.localhost\Ubuntu\home\user\projects\taurhaus"),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn cwd_match_falls_back_to_canonical_paths_for_symlinked_roots() {
        // Regression: a89ea4c replaced canonicalization with string-only normalization,
        // so Claude hooks stopped matching projects reached through a symlink.
        let tmp = tempfile::tempdir().expect("tempdir");
        let real_project = tmp.path().join("real-project");
        let linked_project = tmp.path().join("linked-project");
        fs::create_dir_all(&real_project).expect("real project");
        std::os::unix::fs::symlink(&real_project, &linked_project).expect("project symlink");

        assert!(cwd_matches_member(Some(&linked_project), &real_project));
    }

    #[test]
    fn compact_hook_skips_when_snapshot_task_is_completed() {
        let guard = acquire_env_test_guard();
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let claude_dir = tmp.path().join("claude");
        guard.set_override(&claude_dir);
        let log_path = tmp.path().join("claude-hook.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");
        write_snapshot_fixture(tmp.path(), "taurhaus-team", &member.name);

        let mut snapshot =
            OperationalContextSnapshotStore::load(tmp.path(), "taurhaus-team", &member.name)
                .expect("load snapshot")
                .expect("snapshot exists");
        snapshot.task.status = "completed".to_string();
        OperationalContextSnapshotStore::save(tmp.path(), &snapshot).expect("save snapshot");

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, CompactHookResponse::default());

        let contents = read_log_after_flush(
            &log_state,
            &log_path,
            "\"event\":\"compaction.claude_hook.skipped\"",
        );
        assert!(contents.contains("\"skip_reason\":\"no_resumable_task_context\""));
    }

    #[test]
    fn compact_hook_logs_parse_failures() {
        let guard = acquire_env_test_guard();
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let claude_dir = tmp.path().join("claude");
        guard.set_override(&claude_dir);
        let log_path = tmp.path().join("claude-hook.log.jsonl");
        let log_state = LogFileState::new(log_path.clone()).expect("log state");
        install_global_sink(&log_state);

        let error = handle_session_start_hook("{", tmp.path()).expect_err("parse should fail");
        assert!(error.to_string().contains("invalid compact hook payload"));

        let contents = read_log_after_flush(
            &log_state,
            &log_path,
            "\"event\":\"compaction.compact_hook.failed\"",
        );
        assert!(contents.contains("\"failure_stage\":\"parse_payload\""));
        assert!(contents.contains("\"event\":\"compaction.compact_hook.parse_payload_debug\""));
        assert!(contents.contains("\"raw_payload\":\"{\""));
    }

    #[test]
    fn compact_hook_skips_when_multiple_members_match_same_runtime_session() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let architect = sample_member(&project);
        let mut reviewer = sample_member(&project);
        reviewer.name = "reviewer".to_string();
        reviewer.role_id = Some("taurhaus-reviewer".to_string());
        reviewer.role_name = Some("Taurhaus Reviewer".to_string());

        TeamConfigStore::save(
            tmp.path(),
            "taurhaus-team",
            &TeamConfig {
                schema_version: 1,
                name: "taurhaus-team".to_string(),
                description: None,
                created_at: DateTime::parse_from_rfc3339("2026-03-08T15:00:00Z")
                    .expect("timestamp")
                    .with_timezone(&Utc),
                members: vec![architect.clone(), reviewer.clone()],
                extra: Default::default(),
            },
        )
        .expect("save team");

        for member in [&architect, &reviewer] {
            MemberRuntimeStore::save(
                tmp.path(),
                "taurhaus-team",
                &member.name,
                &MemberRuntimeRecord {
                    schema_version: 3,
                    member_name: member.name.clone(),
                    cli_tool: Some(member.cli_tool),
                    project_path: Some(member.project_path.clone()),
                    pane_id: Some("%217".to_string()),
                    pane_pid: None,
                    pane_start_time: None,
                    session_id: Some("sess-123".to_string()),
                    jsonl_path: None,
                    daemon_pid: None,
                    health: HealthState::Healthy,
                    delivery_lease: None,
                    attached_at: None,
                    last_seen_at: None,
                },
            )
            .expect("save runtime");
            write_snapshot_fixture(tmp.path(), "taurhaus-team", &member.name);
        }

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": project.join(".claude/projects/session.jsonl"),
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, CompactHookResponse::default());
    }

    #[test]
    fn ensure_compact_hook_installed_writes_script_and_settings_entry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        fs::create_dir_all(&teams_dir).expect("teams dir");
        let exe_path = tmp.path().join("taurhaus");
        fs::write(&exe_path, b"binary").expect("exe path");

        let installed = ensure_compact_hook_installed(&teams_dir, &exe_path).expect("install hook");
        assert!(installed);

        let script_path = tmp
            .path()
            .join("hooks")
            .join(platform_hook_filename(HookRuntime::Posix));
        assert!(script_path.exists());
        let settings_raw =
            fs::read_to_string(tmp.path().join("settings.json")).expect("settings exists");
        let settings: Value = serde_json::from_str(&settings_raw).expect("settings parses");

        let hooks = settings["hooks"]["SessionStart"]
            .as_array()
            .expect("session start hooks array");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["matcher"], "compact");
        assert_eq!(hooks[0]["hooks"][0]["type"], "command");
        assert_eq!(
            hooks[0]["hooks"][0]["command"]
                .as_str()
                .expect("command str"),
            format!(
                "bash {}",
                shell_quote_string(&script_path.display().to_string())
            )
        );
    }

    #[test]
    fn ensure_compact_hook_installed_is_idempotent_for_existing_settings() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let teams_dir = tmp.path().join("teams");
        fs::create_dir_all(&teams_dir).expect("teams dir");
        let exe_path = tmp.path().join("taurhaus");
        fs::write(&exe_path, b"binary").expect("exe path");
        fs::write(
            tmp.path().join("settings.json"),
            serde_json::to_string_pretty(&json!({
                "hooks": {
                    "SessionStart": [{
                        "matcher": "compact",
                        "hooks": [{
                            "type": "command",
                            "command": format!("/tmp/{TAURHAUS_COMPACT_HOOK_BASENAME}.sh"),
                        }, {
                            "type": "command",
                            "command": "echo untouched",
                        }]
                    }]
                }
            }))
            .expect("settings json"),
        )
        .expect("write settings");

        let installed = ensure_compact_hook_installed(&teams_dir, &exe_path).expect("install hook");
        assert!(installed);

        let settings: Value = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("settings.json")).expect("settings exists"),
        )
        .expect("settings parse");
        let hooks = settings["hooks"]["SessionStart"][0]["hooks"]
            .as_array()
            .expect("hooks array");
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0]["command"], "echo untouched");
        assert_eq!(
            hooks[1]["command"].as_str().expect("command"),
            format!(
                "bash {}",
                shell_quote_string(
                    &tmp.path()
                        .join("hooks")
                        .join(platform_hook_filename(HookRuntime::Posix))
                        .display()
                        .to_string()
                )
            )
        );
    }

    #[test]
    fn ensure_settings_hook_entry_preserves_valid_json_after_atomic_update() {
        // Regression: before task #712, ensure_settings_hook_entry rewrote settings.json via
        // direct fs::write, which risked truncation or clobbering unrelated content mid-update.
        let tmp = tempfile::tempdir().expect("tempdir");
        let settings_path = tmp.path().join("settings.json");
        let script_path = tmp
            .path()
            .join("hooks")
            .join(platform_hook_filename(HookRuntime::Posix));
        fs::create_dir_all(script_path.parent().expect("hooks dir")).expect("hooks dir");
        fs::write(
            &settings_path,
            serde_json::to_string_pretty(&json!({
                "theme": "dark",
                "hooks": {
                    "Stop": [{
                        "matcher": "*",
                        "hooks": [{
                            "type": "command",
                            "command": "echo stop"
                        }]
                    }]
                }
            }))
            .expect("settings json"),
        )
        .expect("write settings");

        let changed =
            ensure_settings_hook_entry(&settings_path, &script_path, HookRuntime::Posix, None)
                .expect("update settings");
        assert!(changed);

        let updated: Value = serde_json::from_str(
            &fs::read_to_string(&settings_path).expect("updated settings exists"),
        )
        .expect("updated settings remains valid json");

        assert_eq!(updated["theme"], "dark");
        assert_eq!(
            updated["hooks"]["Stop"][0]["hooks"][0]["command"],
            "echo stop"
        );
        assert_eq!(updated["hooks"]["SessionStart"][0]["matcher"], "compact");
        assert_eq!(
            updated["hooks"]["SessionStart"][0]["hooks"][0]["command"]
                .as_str()
                .expect("command"),
            format!(
                "bash {}",
                shell_quote_string(&script_path.display().to_string())
            )
        );
    }

    #[test]
    fn detect_hook_runtime_treats_wsl_unc_paths_as_posix() {
        assert_eq!(
            detect_hook_runtime(Path::new(r"\\wsl.localhost\Ubuntu\home\user\.claude")),
            HookRuntime::Posix
        );
    }

    #[test]
    fn runtime_path_string_converts_windows_exe_for_posix_runtime() {
        let converted = runtime_path_string(
            Path::new(r"C:\Users\user\AppData\Local\taurhaus\taurhaus.exe"),
            HookRuntime::Posix,
        )
        .expect("convert to linux path");
        assert_eq!(
            converted,
            "/mnt/c/Users/user/AppData/Local/taurhaus/taurhaus.exe"
        );
    }

    #[test]
    fn settings_command_for_wsl_claude_uses_bash_and_linux_path() {
        let command = settings_command_for_script(
            Path::new(
                r"\\wsl.localhost\Ubuntu\home\user\.claude\hooks\taurhaus-session-start-compact.sh",
            ),
            HookRuntime::Posix,
        )
        .expect("settings command");
        assert_eq!(
            command,
            "bash '/home/user/.claude/hooks/taurhaus-session-start-compact.sh'"
        );
    }

    #[test]
    fn render_hook_script_for_posix_runtime_execs_linux_mapped_windows_exe() {
        let script = render_hook_script(
            Path::new(r"C:\Users\user\AppData\Local\taurhaus\taurhaus.exe"),
            HookRuntime::Posix,
        )
        .expect("render script");
        assert!(script.contains(
            "exec '/mnt/c/Users/user/AppData/Local/taurhaus/taurhaus.exe' --compact-hook"
        ));
    }

    // Regression: 0b87699 introduced a Claude-only compact hook, so managed
    // Codex sessions lost their operational context after compaction.
    #[test]
    fn compact_hook_accepts_claude_and_codex_payload_fixtures() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");

        for (tool, transcript_path) in [
            (
                CliTool::Claude,
                tmp.path()
                    .join(".claude/projects/project/session-claude.jsonl"),
            ),
            (
                CliTool::Codex,
                tmp.path().join(
                    ".codex/sessions/2026/08/26/rollout-2026-08-26T10-00-00-session-codex.jsonl",
                ),
            ),
        ] {
            let team_name = format!("{tool}-team");
            let session_id = format!("session-{tool}");
            let mut member = sample_member(&project);
            member.name = format!("{tool}-member");
            member.cli_tool = tool;
            write_team_fixture(tmp.path(), &team_name, &member, &session_id);
            write_snapshot_fixture(tmp.path(), &team_name, &member.name);

            let payload = json!({
                "hook_event_name": "SessionStart",
                "session_id": session_id,
                "source": "compact",
                "cwd": &project,
                "transcript_path": transcript_path,
            })
            .to_string();
            let parsed = parse_compact_hook_input(&payload).expect("source parses fixture");
            assert_eq!(parsed.inferred_tool(), Some(tool));

            let response =
                handle_compact_hook(&payload, tmp.path()).expect("compact SessionStart fixture");
            let response = serde_json::to_value(response).expect("serialize response");
            let context = response["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .expect("additional context");
            assert!(context.contains("[taurhaus] restored_working_context_after_compaction"));

            let post_compact = handle_compact_hook(
                &json!({
                    "hook_event_name": "PostCompact",
                    "session_id": format!("session-{tool}"),
                    "trigger": "manual",
                    "cwd": &project,
                    "transcript_path": transcript_path,
                })
                .to_string(),
                tmp.path(),
            )
            .expect("PostCompact fixture");
            assert_eq!(
                serde_json::to_value(post_compact).expect("serialize response"),
                json!({})
            );
        }
    }

    // Regression: 0b87699 installed only Claude settings and persisted no
    // repairable Codex hook executable path.
    #[test]
    fn codex_installer_is_idempotent_repairs_exe_path_and_removes_cleanly() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let codex_home = tmp.path().join("isolated-codex-home");

        let first_exe = tmp.path().join("taurhaus-daemon-v1");
        let second_exe = tmp.path().join("taurhaus-daemon-v2");
        fs::write(&first_exe, b"v1").expect("first exe");
        fs::write(&second_exe, b"v2").expect("second exe");

        assert!(
            ensure_codex_compact_hook_installed_at(&codex_home, &first_exe).expect("first install")
        );
        assert!(
            !ensure_codex_compact_hook_installed_at(&codex_home, &first_exe)
                .expect("idempotent install")
        );

        let hooks: Value = serde_json::from_str(
            &fs::read_to_string(codex_home.join("hooks.json")).expect("hooks.json"),
        )
        .expect("hooks json");
        assert_eq!(
            hooks["hooks"]["SessionStart"][0]["hooks"][0]["additionalContextLimit"],
            CODEX_ADDITIONAL_CONTEXT_LIMIT
        );

        assert!(
            ensure_codex_compact_hook_installed_at(&codex_home, &second_exe).expect("repair exe")
        );
        let script = fs::read_to_string(
            codex_home
                .join("hooks")
                .join(platform_hook_filename(HookRuntime::Posix)),
        )
        .expect("hook script");
        assert!(script.contains(&second_exe.display().to_string()));
        assert!(!script.contains(&first_exe.display().to_string()));

        assert!(remove_codex_compact_hook_at(&codex_home).expect("remove hook"));
        assert!(!remove_codex_compact_hook_at(&codex_home).expect("idempotent remove"));
        let hooks_after: Value = serde_json::from_str(
            &fs::read_to_string(codex_home.join("hooks.json")).expect("hooks.json after"),
        )
        .expect("hooks json after");
        assert!(!hooks_after
            .to_string()
            .contains(TAURHAUS_COMPACT_HOOK_BASENAME));
    }

    #[test]
    fn codex_installer_regression_does_not_mutate_home_or_codex_home() {
        // Regression: 6fe0aa3 made an installer test mutate process-wide HOME and
        // CODEX_HOME while unrelated coordination tests resolved those variables.
        let source = include_str!("compact_hook.rs");
        let installer_test = source
            .split("fn codex_installer_is_idempotent_repairs_exe_path_and_removes_cleanly")
            .nth(1)
            .expect("installer regression test")
            .split("fn codex_installer_regression_does_not_mutate_home_or_codex_home")
            .next()
            .expect("installer regression test body");
        assert!(!installer_test.contains("guard.set_home"));
        assert!(!installer_test.contains("guard.set_codex_home"));
    }

    #[test]
    fn managed_codex_discovery_skips_orphan_team_directories() {
        // Regression: 6fe0aa3 made one non-team directory under teams/ abort Codex
        // hook reconciliation before a later valid managed Codex team was checked.
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("00-orphan")).expect("orphan team dir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let mut member = sample_member(&project);
        member.cli_tool = CliTool::Codex;
        write_team_fixture(tmp.path(), "zz-codex-team", &member, "codex-session");

        assert!(any_managed_codex_member(tmp.path()).expect("scan valid teams"));
    }

    #[test]
    fn compact_hook_records_the_transcript_compaction_timestamp() {
        // Regression: 6fe0aa3 recorded Utc::now() for hook delivery while the
        // transcript fallback recorded the compacted event timestamp, defeating dedupe.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let mut member = sample_member(&project);
        member.cli_tool = CliTool::Codex;
        write_team_fixture(tmp.path(), "codex-team", &member, "session-codex");
        write_snapshot_fixture(tmp.path(), "codex-team", &member.name);
        let transcript_path = tmp
            .path()
            .join(".codex/sessions/rollout-session-codex.jsonl");
        fs::create_dir_all(transcript_path.parent().expect("transcript parent"))
            .expect("create transcript parent");
        fs::write(
            &transcript_path,
            concat!(
                "{\"timestamp\":\"2026-08-26T05:59:59.000Z\",\"type\":\"session_meta\",\"payload\":{}}\n",
                "{\"timestamp\":\"2026-08-26T06:00:00.123Z\",\"type\":\"compacted\",\"payload\":{}}\n"
            ),
        )
        .expect("write transcript");

        handle_compact_hook(
            &json!({
                "hook_event_name": "SessionStart",
                "session_id": "session-codex",
                "source": "compact",
                "cwd": &project,
                "transcript_path": &transcript_path,
            })
            .to_string(),
            tmp.path(),
        )
        .expect("handle Codex compact hook");

        let state = MemberCompactionStore::load(tmp.path(), "codex-team", &member.name)
            .expect("load compaction state")
            .expect("compaction state");
        assert_eq!(
            state.last_compaction_timestamp,
            DateTime::parse_from_rfc3339("2026-08-26T06:00:00.123Z")
                .expect("timestamp")
                .with_timezone(&Utc)
        );
    }

    // Regression: 0b87699 wired hook stdin/stdout only through the desktop
    // binary, leaving the WSL daemon binary unable to host the same bridge.
    #[test]
    fn compact_hook_cli_reads_stdin_and_writes_stdout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let mut member = sample_member(&project);
        member.cli_tool = CliTool::Codex;
        write_team_fixture(tmp.path(), "codex-team", &member, "session-codex");
        write_snapshot_fixture(tmp.path(), "codex-team", &member.name);
        let payload = json!({
            "hook_event_name": "SessionStart",
            "session_id": "session-codex",
            "source": "compact",
            "cwd": &project,
            "transcript_path": tmp.path().join(
                ".codex/sessions/2026/08/26/rollout-2026-08-26T10-00-00-session-codex.jsonl"
            ),
        })
        .to_string();
        let mut stdout = Vec::new();

        run_compact_hook_cli(payload.as_bytes(), &mut stdout, tmp.path())
            .expect("CLI bridge succeeds");

        let response: Value = serde_json::from_slice(&stdout).expect("stdout JSON");
        assert!(response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .expect("additional context")
            .contains("Current task:"));
    }

    #[test]
    fn team_has_managed_claude_member_detects_claude_presence() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");

        assert!(team_has_managed_claude_member(tmp.path(), "taurhaus-team").expect("team loads"));
    }

    fn read_log_after_flush(log_state: &LogFileState, path: &Path, needle: &str) -> String {
        log_state
            .flush_for_test()
            .expect("flush structured log sink");
        let contents = fs::read_to_string(path).expect("read structured log");
        assert!(
            contents.contains(needle),
            "expected structured log to contain {needle}: {contents}"
        );
        contents
    }
}
