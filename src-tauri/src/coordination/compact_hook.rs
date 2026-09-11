//! Claude Code, Codex and Grok hook bridge for post-compaction reinjection.

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
    record_delivery_at, CompactionDeliveryResult, MemberCompactionStore, MemberRuntimeStore,
    OperationalContextSnapshotStore, TeamConfigStore, TeamRootRegistry,
};
use crate::provider::path;
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::{spec, CliTool, CompactionDelivery};
use taurhaus_lib::logging::emit_global;

pub mod drain;

const TAURHAUS_COMPACT_HOOK_BASENAME: &str = "taurhaus-session-start-compact";
const CLAUDE_SETTINGS_FILENAME: &str = "settings.json";
const CODEX_HOOKS_FILENAME: &str = "hooks.json";
const GROK_HOOKS_FILENAME: &str = "taurhaus.json";
const SESSION_START_HOOK_EVENT: &str = "SessionStart";
const POST_COMPACT_HOOK_EVENT: &str = "PostCompact";
const GROK_COMPACT_TRIGGERS: &str = "manual|auto";
const COMPACT_SOURCE: &str = "compact";
/// Env names grok's hook runner injects into every hook process. Their presence
/// identifies the harness even when the envelope carries no transcript path.
const GROK_HOOK_EVENT_ENV: &str = "GROK_HOOK_EVENT";
const GROK_SESSION_ID_ENV: &str = "GROK_SESSION_ID";
/// grok also loads `~/.claude/settings.json` hooks, so one compaction can reach
/// this bridge twice. Two invocations for the same session this close together
/// are the same compaction and must yield one reinjection.
const COMPAT_IMPORT_DEDUPE_WINDOW: chrono::TimeDelta = chrono::TimeDelta::seconds(5);
/// Passive grok hooks default to five seconds; the bridge reads team state off
/// disk, so it asks for the same headroom the Codex hook gets.
const GROK_HOOK_TIMEOUT_SECS: u64 = 30;
const HOOK_EXECUTABLE_RECORD_FILENAME: &str = "taurhaus-session-start-compact.executable";
pub(crate) const CODEX_ADDITIONAL_CONTEXT_LIMIT: u64 = 12_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookRuntime {
    Posix,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompactHookInput {
    #[serde(alias = "hookEventName")]
    hook_event_name: String,
    #[serde(alias = "sessionId")]
    session_id: String,
    #[serde(default, alias = "turnId")]
    turn_id: Option<String>,
    #[serde(default, alias = "itemId")]
    item_id: Option<String>,
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
    /// grok's workspace root, which stands in for `cwd` when it is absent.
    #[serde(default, alias = "workspaceRoot")]
    workspace_root: Option<PathBuf>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, alias = "agentType", alias = "subagentType")]
    agent_type: Option<String>,
}

impl CompactHookInput {
    pub fn inferred_tool(&self) -> Option<CliTool> {
        tool_from_hook_environment()
            .or_else(|| infer_tool_from_transcript_path(self.transcript_path.as_deref()))
    }

    /// The directory the session runs in, however the harness spells it.
    fn project_dir(&self) -> Option<&Path> {
        self.cwd.as_deref().or(self.workspace_root.as_deref())
    }
}

/// grok's hook runner injects reserved `GROK_*` names into every hook process
/// (user values are stripped with a warning), which is the only identification
/// available when its envelope carries no transcript path at all.
fn tool_from_hook_environment() -> Option<CliTool> {
    (std::env::var_os(GROK_HOOK_EVENT_ENV).is_some()
        || std::env::var_os(GROK_SESSION_ID_ENV).is_some())
    .then_some(CliTool::Grok)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CompactHookResponse {
    #[serde(skip)]
    drain_receipt: Option<drain::Offer>,
    #[serde(skip)]
    receipt: Option<(
        PathBuf,
        String,
        String,
        crate::coordination::recovery_card::CardReceipt,
    )>,
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
    teams_dir: PathBuf,
    team_name: String,
    member: Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactHookSkipReason {
    PostCompactSignalOnly,
    DuplicateCompatImport,
    ToolInferenceUnavailable,
    NoManagedMemberMatch,
    MultipleManagedMembersMatched,
    MissingOperationalSnapshot,
    NoResumableTaskContext,
}

impl CompactHookSkipReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::PostCompactSignalOnly => "post_compact_signal_only",
            Self::DuplicateCompatImport => "duplicate_compat_import",
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
    DeliverInbox,
    RecordDelivery,
    SerializeResponse,
}

impl CompactHookFailureStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::ReadStdin => "read_stdin",
            Self::ParsePayload => "parse_payload",
            Self::DeliverInbox => "deliver_inbox",
            Self::RecordDelivery => "record_delivery",
            Self::SerializeResponse => "serialize_response",
        }
    }
}

/// Harness-specific hook configuration. Claude, Codex and Grok are the current
/// installer implementations; their stable payload fields share one parser.
pub trait CompactionSignalSource: Send + Sync {
    fn install(&self, config_dir: &Path, taurhaus_exe: &Path) -> Result<bool, CoordinationError>;
    fn remove(&self, config_dir: &Path) -> Result<bool, CoordinationError>;
}

fn parse_compact_hook_input(raw: &str) -> Result<CompactHookInput, serde_json::Error> {
    serde_json::from_str(raw)
}

/// Hooks are *registered* under PascalCase event names everywhere, but the value
/// a harness puts on the wire differs: Claude and Codex echo the PascalCase
/// name, while grok spells it in snake_case (`~/.grok/docs/user-guide/10-hooks.md`
/// documents the stdin envelope as `"hookEventName": "pre_tool_use"`, and injects
/// the same spelling as `GROK_HOOK_EVENT`). One event, either spelling.
fn hook_event_is(raw: &str, canonical: &str) -> bool {
    debug_assert!(!canonical.contains('_'), "canonical names are PascalCase");
    raw.replace('_', "").eq_ignore_ascii_case(canonical)
}

pub struct ClaudeCompactionSignalSource;

impl CompactionSignalSource for ClaudeCompactionSignalSource {
    fn install(&self, config_dir: &Path, taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
        let default_config_dir = PlatformPaths::claude_dir();
        let pinned_config_dir = (!crate::coordination::stores::team_roots::same_teams_root(
            config_dir,
            &default_config_dir,
        ))
        .then_some(config_dir);
        ensure_source_installed(
            config_dir,
            CLAUDE_SETTINGS_FILENAME,
            taurhaus_exe,
            None,
            pinned_config_dir,
        )
    }

    fn remove(&self, config_dir: &Path) -> Result<bool, CoordinationError> {
        remove_source_hook(config_dir, CLAUDE_SETTINGS_FILENAME)
    }
}

pub struct CodexCompactionSignalSource;

impl CompactionSignalSource for CodexCompactionSignalSource {
    fn install(&self, config_dir: &Path, taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
        let runtime = detect_hook_runtime(config_dir);
        let executable = runtime_path_string(taurhaus_exe, runtime)?;
        if !hook_executable_exists(config_dir, &executable) {
            emit_codex_hook_degraded(config_dir, &executable);
            return self.remove(config_dir);
        }
        ensure_source_installed(
            config_dir,
            CODEX_HOOKS_FILENAME,
            taurhaus_exe,
            Some(CODEX_ADDITIONAL_CONTEXT_LIMIT),
            None,
        )
    }

    fn remove(&self, config_dir: &Path) -> Result<bool, CoordinationError> {
        remove_source_hook(config_dir, CODEX_HOOKS_FILENAME)
    }
}

/// grok's personal hook directory (`~/.grok/hooks/*.json`) is always trusted and
/// taurhaus owns its own file there, so installation neither edits the user's
/// config nor needs a folder-trust grant.
pub struct GrokCompactionSignalSource;

impl CompactionSignalSource for GrokCompactionSignalSource {
    fn install(&self, config_dir: &Path, taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
        let runtime = detect_hook_runtime(config_dir);
        let executable = runtime_path_string(taurhaus_exe, runtime)?;
        if !hook_executable_exists(config_dir, &executable) {
            emit_hook_degraded(CliTool::Grok, config_dir, &executable);
            return self.remove(config_dir);
        }

        let hooks_dir = config_dir.join("hooks");
        fs::create_dir_all(&hooks_dir)?;
        let script_path = hooks_dir.join(platform_hook_filename(runtime));
        let script_changed = write_hook_script(&script_path, taurhaus_exe, runtime)?;
        let executable_changed = write_hook_executable_record(&hooks_dir, taurhaus_exe, runtime)?;

        let command = settings_command_for_script(&script_path, runtime)?;
        let desired = grok_hook_document(&command);
        let path = hooks_dir.join(GROK_HOOKS_FILENAME);
        let payload = serde_json::to_vec_pretty(&desired).map_err(|error| {
            CoordinationError::StoreError(format!(
                "failed to serialize grok hooks '{}': {error}",
                path.display()
            ))
        })?;
        let document_changed = fs::read(&path)
            .map(|current| current != payload)
            .unwrap_or(true);
        if document_changed {
            write_atomic_settings_file(&path, &payload)?;
        }
        Ok(script_changed || executable_changed || document_changed)
    }

    fn remove(&self, config_dir: &Path) -> Result<bool, CoordinationError> {
        let hooks_dir = config_dir.join("hooks");
        let mut changed = false;
        for path in [
            hooks_dir.join(GROK_HOOKS_FILENAME),
            hooks_dir.join(platform_hook_filename(detect_hook_runtime(config_dir))),
            hooks_dir.join(HOOK_EXECUTABLE_RECORD_FILENAME),
        ] {
            match fs::remove_file(&path) {
                Ok(()) => changed = true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(CoordinationError::Io(error)),
            }
        }
        Ok(changed)
    }
}

/// `SessionStart(compact)` is the reinjection path; `PostCompact` is registered
/// beside it so a grok compaction is still observed on a build whose start
/// source never reports `compact`.
fn grok_hook_document(command: &str) -> Value {
    json!({
        "hooks": {
            SESSION_START_HOOK_EVENT: [{
                "matcher": COMPACT_SOURCE,
                "hooks": [grok_hook_command(command)],
            }],
            POST_COMPACT_HOOK_EVENT: [{
                "matcher": GROK_COMPACT_TRIGGERS,
                "hooks": [grok_hook_command(command)],
            }],
        }
    })
}

fn grok_hook_command(command: &str) -> Value {
    json!({
        "type": "command",
        "command": command,
        "timeout": GROK_HOOK_TIMEOUT_SECS,
    })
}

pub fn ensure_grok_compact_hook_installed_at(
    grok_home: &Path,
    taurhaus_exe: &Path,
) -> Result<bool, CoordinationError> {
    GrokCompactionSignalSource.install(grok_home, taurhaus_exe)
}

pub fn remove_grok_compact_hook_at(grok_home: &Path) -> Result<bool, CoordinationError> {
    GrokCompactionSignalSource.remove(grok_home)
}

pub fn grok_compact_hook_is_installed_at(grok_home: &Path) -> bool {
    let hooks_dir = grok_home.join("hooks");
    let runtime = detect_hook_runtime(grok_home);
    let Ok(raw) = fs::read_to_string(hooks_dir.join(GROK_HOOKS_FILENAME)) else {
        return false;
    };
    let Ok(document) = serde_json::from_str::<Value>(&raw) else {
        return false;
    };
    let registered = |event: &str| {
        document
            .get("hooks")
            .and_then(|hooks| hooks.get(event))
            .and_then(Value::as_array)
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|hooks| hooks.iter().any(is_taurhaus_compact_hook))
                })
            })
    };
    let script_path = hooks_dir.join(platform_hook_filename(runtime));
    registered(SESSION_START_HOOK_EVENT)
        && registered(POST_COMPACT_HOOK_EVENT)
        && script_path.is_file()
}

fn emit_codex_hook_degraded(codex_home: &Path, executable: &str) {
    emit_hook_degraded(CliTool::Codex, codex_home, executable);
}

fn emit_hook_degraded(tool: CliTool, config_dir: &Path, executable: &str) {
    let mut fields = Map::new();
    fields.insert("tool".to_string(), Value::String(tool.to_string()));
    fields.insert(
        "reason".to_string(),
        Value::String("hook_executable_missing".to_string()),
    );
    fields.insert(
        "config_dir".to_string(),
        Value::String(config_dir.display().to_string()),
    );
    if tool == CliTool::Codex {
        // Shipped field name kept for the existing Codex log consumers.
        fields.insert(
            "codex_home".to_string(),
            Value::String(config_dir.display().to_string()),
        );
    }
    fields.insert(
        "executable".to_string(),
        Value::String(executable.to_string()),
    );
    emit_global(
        "warn",
        "coordination",
        &hook_event_name(Some(tool), "degraded"),
        Some(format!(
            "Skipped {} compaction hook because its executable is unavailable",
            spec(tool).label
        )),
        fields,
    );
}

fn read_hosted_hook<R: Read>(
    stdin: R,
    teams_dir: &Path,
    host_guard: &mut Option<crate::coordination::stores::lock::HostOperationLock>,
) -> Result<CompactHookResponse, CoordinationError> {
    let mut raw = Vec::new();
    stdin
        .take(64 * 1024 + 1)
        .read_to_end(&mut raw)
        .map_err(|error| {
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
    if raw.len() > 64 * 1024 {
        emit_global(
            "debug",
            "coordination",
            "delivery.hook.skipped",
            None,
            json!({"reason":"hook_input_budget"})
                .as_object()
                .unwrap()
                .clone(),
        );
        return Ok(CompactHookResponse::default());
    }
    let raw = String::from_utf8(raw)
        .map_err(|_| CoordinationError::Validation("invalid hook UTF-8".into()))?;
    handle_compact_hook_with_guard(&raw, teams_dir, host_guard)
}

pub fn handle_compact_hook(
    raw: &str,
    teams_dir: &Path,
) -> Result<CompactHookResponse, CoordinationError> {
    handle_compact_hook_with_guard(raw, teams_dir, &mut None)
}

fn handle_compact_hook_with_guard(
    raw: &str,
    teams_dir: &Path,
    host_guard: &mut Option<crate::coordination::stores::lock::HostOperationLock>,
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

    if hook_event_is(&payload.hook_event_name, POST_COMPACT_HOOK_EVENT)
        || (hook_event_is(&payload.hook_event_name, SESSION_START_HOOK_EVENT)
            && payload.source.as_deref() == Some(COMPACT_SOURCE))
    {
        emit_compact_hook_received(&payload, raw.len());
    } else {
        emit_global(
            "debug",
            "coordination",
            "delivery.hook.received",
            None,
            base_compact_hook_fields(Some(&payload), None),
        );
    }
    let mut response = handle_compaction_decision(&payload, teams_dir, host_guard)?;
    // Hosted compaction keeps exclusion through stdout and its local recovery receipt.
    // Mesh drain subprocess round-trips must not consume that bounded lock budget.
    if host_guard.is_none() {
        drain::append(teams_dir, &payload, &mut response);
    }
    Ok(response)
}

fn handle_compaction_decision(
    payload: &CompactHookInput,
    teams_dir: &Path,
    host_guard: &mut Option<crate::coordination::stores::lock::HostOperationLock>,
) -> Result<CompactHookResponse, CoordinationError> {
    let is_post_compact = hook_event_is(&payload.hook_event_name, POST_COMPACT_HOOK_EVENT);
    if !is_post_compact
        && (!hook_event_is(&payload.hook_event_name, SESSION_START_HOOK_EVENT)
            || payload.source.as_deref() != Some(COMPACT_SOURCE))
    {
        return Ok(CompactHookResponse::default());
    }

    let Some(tool) = payload.inferred_tool() else {
        emit_compact_hook_skipped(
            payload,
            None,
            CompactHookSkipReason::ToolInferenceUnavailable,
        );
        return Ok(CompactHookResponse::default());
    };

    let delivery = spec(tool).capabilities.compaction_delivery;
    // `PostCompact` carries the reinjection wherever the session-start source
    // never reports `compact` — grok's matcher tests the start source
    // (`startup`, `resume`, …) and compaction restarts nothing. It has no
    // documented stdout contract though, so a harness that is answered on
    // stdout can only treat it as a signal.
    if is_post_compact && delivery != CompactionDelivery::MeshInbox {
        emit_compact_hook_skipped(payload, None, CompactHookSkipReason::PostCompactSignalOnly);
        return Ok(CompactHookResponse::default());
    }

    let matched = match resolve_member_match(teams_dir, tool, payload)? {
        Ok(matched) => matched,
        Err(reason) => {
            emit_compact_hook_skipped(payload, None, reason);
            return Ok(CompactHookResponse::default());
        }
    };

    let before =
        MemberRuntimeStore::load(&matched.teams_dir, &matched.team_name, &matched.member.name)?;
    if before.app_server.is_some() {
        *host_guard = Some(
            crate::coordination::stores::lock::HostOperationLock::acquire(
                &matched.teams_dir,
                &matched.team_name,
                &matched.member.name,
                std::time::Duration::from_secs(2),
            )?,
        );
        let current =
            MemberRuntimeStore::load(&matched.teams_dir, &matched.team_name, &matched.member.name)?;
        let attachment = current.app_server.as_ref().ok_or_else(|| {
            CoordinationError::Conflict("hosted compaction has no attachment".into())
        })?;
        if current.app_server != before.app_server
            || current.attachment_generation != before.attachment_generation
            || current.launch_root != before.launch_root
            || current.session_id.as_deref() != Some(payload.session_id.as_str())
            || attachment.thread_id != payload.session_id
            || attachment.contract != 1
            || attachment.transport
                != taurhaus_lib::session_scanner::launch::HostedDescriptor::codex().transport
            || attachment.state != "ready"
        {
            return Err(CoordinationError::Conflict(
                "hosted compaction attachment changed or is unavailable".into(),
            ));
        }
        use crate::coordination::stores::compaction::{emit_host_compaction, record_host_boundary};
        let timestamp = payload
            .transcript_path
            .as_deref()
            .and_then(crate::session_scanner::transcript_boundary::latest_compaction_timestamp);
        let boundary = json!({"threadId":payload.session_id,"turnId":payload.turn_id,
            "itemId":payload.item_id,"completedAtMs":timestamp.map(|t| t.timestamp_millis())});
        if !record_host_boundary(
            &matched.teams_dir,
            &matched.team_name,
            &matched.member.name,
            &boundary,
            "hook",
        )? {
            emit_host_compaction(
                &matched.team_name,
                &matched.member.name,
                &boundary,
                "compaction.codex_host.skipped",
                Some("already_recorded"),
            );
            return Ok(CompactHookResponse::default());
        }
        let registry = crate::coordination::stores::TeamRootRegistry::new(teams_dir.into());
        let card = crate::coordination::recovery_delivery::prepare(
            &registry,
            &matched.teams_dir,
            &matched.team_name,
            &matched.member.name,
            "hook_stdout",
        )?;
        if card.is_some() {
            let state = crate::coordination::stores::MemberCompactionStore::load(
                &matched.teams_dir,
                &matched.team_name,
                &matched.member.name,
            )?
            .ok_or_else(|| CoordinationError::Conflict("host compaction state missing".into()))?;
            record_delivery_at(
                &matched.teams_dir,
                &matched.team_name,
                &matched.member.name,
                tool,
                &payload.session_id,
                state.last_compaction_timestamp,
                CompactionDeliveryResult::Injected,
            )?;
        }
        return Ok(
            card.map_or_else(CompactHookResponse::default, |card| CompactHookResponse {
                drain_receipt: None,
                receipt: Some((
                    matched.teams_dir.clone(),
                    matched.team_name.clone(),
                    matched.member.name.clone(),
                    card.receipt,
                )),
                hook_specific_output: Some(CompactHookSpecificOutput {
                    hook_event_name: SESSION_START_HOOK_EVENT.into(),
                    additional_context: card.text,
                }),
            }),
        );
    }
    emit_compact_hook_resolved(payload, &matched);

    if crate::session_scanner::cli_tool::spec(tool)
        .capabilities
        .compaction_hook_compat_import
    {
        emit_compaction_hook_compat_import(tool);
        if compat_import_duplicate(
            &matched.teams_dir,
            &matched,
            &payload.session_id,
            Utc::now(),
        )? {
            emit_compact_hook_skipped(
                payload,
                Some(&matched),
                CompactHookSkipReason::DuplicateCompatImport,
            );
            return Ok(CompactHookResponse::default());
        }
    }

    let compaction_timestamp = if tool == CliTool::Codex {
        payload
            .transcript_path
            .as_deref()
            .and_then(crate::session_scanner::transcript_boundary::latest_compaction_timestamp)
            .unwrap_or_else(Utc::now)
    } else {
        Utc::now()
    };

    let Some(snapshot) = OperationalContextSnapshotStore::load(
        &matched.teams_dir,
        &matched.team_name,
        &matched.member.name,
    )?
    else {
        record_delivery_at(
            &matched.teams_dir,
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
                Some(payload),
                Some(&matched),
                None,
                None,
                None,
                &error.to_string(),
            );
        })?;
        emit_compact_hook_skipped(
            payload,
            Some(&matched),
            CompactHookSkipReason::MissingOperationalSnapshot,
        );
        return Ok(CompactHookResponse::default());
    };

    if !CompactionReinjectionService::snapshot_has_resumable_task(&snapshot) {
        record_delivery_at(
            &matched.teams_dir,
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
                Some(payload),
                Some(&matched),
                None,
                None,
                None,
                &error.to_string(),
            );
        })?;
        emit_compact_hook_skipped(
            payload,
            Some(&matched),
            CompactHookSkipReason::NoResumableTaskContext,
        );
        return Ok(CompactHookResponse::default());
    }

    let registry = crate::coordination::stores::TeamRootRegistry::new(teams_dir.to_path_buf());
    let path = if delivery == CompactionDelivery::MeshInbox {
        "inbox"
    } else {
        "hook_stdout"
    };
    let Some(mut card) = crate::coordination::recovery_delivery::prepare_compaction(
        &registry,
        &matched.teams_dir,
        &matched.team_name,
        &matched.member.name,
        path,
    )?
    else {
        return Ok(CompactHookResponse::default());
    };
    let additional_context = card.text.clone();

    // A harness that ignores passive-hook stdout has to be handed the card
    // before the delivery is recorded — the hook answer would go nowhere.
    if delivery == CompactionDelivery::MeshInbox {
        let mut message = crate::coordination::stores::MeshInboxMessage::operator_originated(
            &matched.member.name,
            card.text.clone(),
            Some(crate::coordination::reinjection::POST_COMPACTION_INBOX_SUMMARY.into()),
            Utc::now(),
            None,
        );
        crate::coordination::recovery_delivery::attach_receipt(&mut message, Some(&card.receipt));
        match CompactionReinjectionService::deliver_to_inbox(
            &matched.teams_dir,
            &matched.team_name,
            &matched.member.name,
            &message,
        ) {
            Ok(journal) => card.receipt.journal = journal,
            Err(error) => {
                crate::coordination::recovery_delivery::observe_inbox_failure(
                    &matched.teams_dir,
                    &matched.team_name,
                    &matched.member.name,
                    &message,
                    &error,
                );
                let _ = record_delivery_at(
                    &matched.teams_dir,
                    &matched.team_name,
                    &matched.member.name,
                    tool,
                    &payload.session_id,
                    compaction_timestamp,
                    CompactionDeliveryResult::Failed,
                );
                emit_compact_hook_failed(
                    CompactHookFailureStage::DeliverInbox,
                    Some(payload),
                    Some(&matched),
                    None,
                    None,
                    None,
                    &error.to_string(),
                );
                return Err(error);
            }
        }
    }

    crate::coordination::stores::compaction::record_delivery_with_journal_at(
        &matched.teams_dir,
        &matched.team_name,
        &matched.member.name,
        tool,
        &payload.session_id,
        compaction_timestamp,
        CompactionDeliveryResult::Injected,
        card.receipt.journal.clone(),
    )
    .inspect_err(|error| {
        emit_compact_hook_failed(
            CompactHookFailureStage::RecordDelivery,
            Some(payload),
            Some(&matched),
            None,
            None,
            None,
            &error.to_string(),
        );
    })?;

    if delivery == CompactionDelivery::MeshInbox {
        crate::coordination::recovery_delivery::observe(
            &registry,
            &matched.teams_dir,
            &matched.team_name,
            &matched.member.name,
            &card.receipt,
            crate::coordination::recovery_card::ReceiptStage::Accepted,
        )?;
    }
    emit_compact_hook_delivered(payload, &matched, additional_context.len());

    Ok(match delivery {
        CompactionDelivery::HookStdout => CompactHookResponse {
            drain_receipt: None,
            receipt: Some((
                matched.teams_dir.clone(),
                matched.team_name.clone(),
                matched.member.name.clone(),
                card.receipt,
            )),
            hook_specific_output: Some(CompactHookSpecificOutput {
                hook_event_name: SESSION_START_HOOK_EVENT.to_string(),
                additional_context,
            }),
        },
        CompactionDelivery::MeshInbox => CompactHookResponse::default(),
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

    let compact_changed = ClaudeCompactionSignalSource.install(claude_dir, taurhaus_exe)?;
    if !drain::enabled() {
        return Ok(compact_changed);
    }
    let bindings = TeamConfigStore::list(teams_dir)?
        .into_iter()
        .filter_map(|team| {
            TeamConfigStore::load(teams_dir, &team)
                .ok()
                .map(|config| (team, config))
        })
        .flat_map(|(team, config)| {
            config
                .members
                .into_iter()
                .filter(|m| m.cli_tool == CliTool::Claude)
                .map(move |member| (teams_dir.to_path_buf(), team.clone(), member.name))
        })
        .collect::<Vec<_>>();
    let drain_changed = reconcile_claude_drain_hooks(claude_dir, &bindings, taurhaus_exe)?;
    Ok(compact_changed || drain_changed)
}

fn reconcile_claude_drain_hooks(
    home: &Path,
    bindings: &[drain::Binding],
    exe: &Path,
) -> Result<bool, CoordinationError> {
    drain::reconcile_home(home, CliTool::Claude, bindings, exe)
}

pub fn remove_compact_hook(teams_dir: &Path) -> Result<bool, CoordinationError> {
    let Some(claude_dir) = teams_dir.parent() else {
        return Err(CoordinationError::Validation(format!(
            "team directory '{}' has no parent Claude dir",
            teams_dir.display()
        )));
    };
    let exe = std::env::current_exe()?;
    let compact_changed = ClaudeCompactionSignalSource.remove(claude_dir)?;
    let drain_changed = reconcile_claude_drain_hooks(claude_dir, &[], &exe)?;
    Ok(compact_changed || drain_changed)
}

pub fn ensure_codex_compact_hook_installed(taurhaus_exe: &Path) -> Result<bool, CoordinationError> {
    match crate::models::CliVersions::current().codex_compaction_hooks_support() {
        Some(true) => {
            ensure_codex_compact_hook_installed_at(&PlatformPaths::codex_dir(), taurhaus_exe)
        }
        Some(false) | None => Ok(false),
    }
}

pub fn ensure_codex_compact_hook_installed_at(
    codex_home: &Path,
    taurhaus_exe: &Path,
) -> Result<bool, CoordinationError> {
    CodexCompactionSignalSource.install(codex_home, taurhaus_exe)
}

pub fn remove_codex_compact_hook() -> Result<bool, CoordinationError> {
    remove_codex_compact_hook_at(&PlatformPaths::codex_dir())
}

pub fn remove_codex_compact_hook_at(codex_home: &Path) -> Result<bool, CoordinationError> {
    CodexCompactionSignalSource.remove(codex_home)
}

pub fn codex_compact_hook_is_installed() -> bool {
    codex_compact_hook_is_installed_at(&PlatformPaths::codex_dir())
}

pub fn codex_compact_hook_is_installed_at(codex_home: &Path) -> bool {
    source_hook_is_installed(codex_home, CODEX_HOOKS_FILENAME)
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
    team_has_managed_member(teams_dir, team_name, CliTool::Codex)
}

fn team_has_managed_member(
    teams_dir: &Path,
    team_name: &str,
    tool: CliTool,
) -> Result<bool, CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    Ok(config.members.iter().any(|member| member.cli_tool == tool))
}

pub fn any_managed_codex_member(teams_dir: &Path) -> Result<bool, CoordinationError> {
    any_managed_member(teams_dir, CliTool::Codex)
}

pub fn any_managed_claude_member(teams_dir: &Path) -> Result<bool, CoordinationError> {
    any_managed_member(teams_dir, CliTool::Claude)
}

pub fn any_managed_grok_member(teams_dir: &Path) -> Result<bool, CoordinationError> {
    any_managed_member(teams_dir, CliTool::Grok)
}

/// Whether any team on this host runs a member on `tool`.
///
/// A directory under `teams/` that holds no team config is not a team and is
/// skipped. Every other read failure is reported: the caller uninstalls a
/// global hook on `false`, and "this roster cannot be read" is not proof the
/// last member on `tool` is gone.
fn any_managed_member(teams_dir: &Path, tool: CliTool) -> Result<bool, CoordinationError> {
    for team_name in TeamConfigStore::list(teams_dir)? {
        match team_has_managed_member(teams_dir, &team_name, tool) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(CoordinationError::NotFound(reason)) => {
                tracing::debug!(
                    team_name,
                    tool = %tool,
                    reason,
                    "skipping a directory without a team config during managed member discovery"
                );
            }
            Err(error) => return Err(error),
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

    for (team_teams_dir, team_name) in
        TeamRootRegistry::new(teams_dir.to_path_buf()).team_locations()?
    {
        let config = match TeamConfigStore::load(&team_teams_dir, &team_name) {
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
            let runtime_session_id =
                MemberRuntimeStore::load(&team_teams_dir, &team_name, &member.name)
                    .ok()
                    .and_then(|runtime| runtime.session_id);
            candidates.push((
                HookMemberMatch {
                    teams_dir: team_teams_dir.clone(),
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
            cwd_matches_member(payload.project_dir(), &matched.member.project_path)
        })
        .map(|(matched, _)| matched)
        .collect::<Vec<_>>();
    match cwd_matches.len() {
        1 => Ok(Ok(cwd_matches.into_iter().next().expect("single match"))),
        0 => Ok(Err(CompactHookSkipReason::NoManagedMemberMatch)),
        _ => Ok(Err(CompactHookSkipReason::MultipleManagedMembersMatched)),
    }
}

/// One compaction can invoke this bridge through both taurhaus's own grok hook
/// and the Claude registration grok imports. The delivery already recorded for
/// this session is the dedupe key; the second invocation is a duplicate.
fn compat_import_duplicate(
    teams_dir: &Path,
    matched: &HookMemberMatch,
    session_id: &str,
    now: chrono::DateTime<Utc>,
) -> Result<bool, CoordinationError> {
    let Some(state) =
        MemberCompactionStore::load(teams_dir, &matched.team_name, &matched.member.name)?
    else {
        return Ok(false);
    };
    Ok(state.last_session_id == session_id
        && (state.last_delivery_result == CompactionDeliveryResult::Injected
            || crate::coordination::journal::canonical(teams_dir, &matched.team_name)?)
        && now.signed_duration_since(state.last_compaction_timestamp) < COMPAT_IMPORT_DEDUPE_WINDOW)
}

fn emit_compaction_hook_compat_import(tool: CliTool) {
    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        let mut fields = Map::new();
        fields.insert("tool".to_string(), Value::String(tool.to_string()));
        emit_global(
            "info",
            "coordination",
            "compaction.hook.compat_import",
            Some(
                "Harness imports another vendor's hook registrations; duplicate \
                 compaction invocations are deduplicated"
                    .to_string(),
            ),
            fields,
        );
    });
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
    if normalized.contains("/.grok/") {
        return Some(CliTool::Grok);
    }
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

/// Native hook mode is a short-lived process: take sole ownership of fd 1 so
/// dropping the output executor sends EOF before Mesh can close its offer.
#[cfg(unix)]
#[allow(unsafe_code)] // std::io::Stdout is shared; this standalone executor must close fd 1.
pub fn hook_stdout() -> fs::File {
    use std::os::fd::{FromRawFd, OwnedFd};
    static TAKEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    assert!(
        !TAKEN.swap(true, std::sync::atomic::Ordering::SeqCst),
        "hook stdout already owned"
    );
    // SAFETY: called once, only by the standalone hook CLI before any stdout use.
    fs::File::from(unsafe { OwnedFd::from_raw_fd(1) })
}
#[cfg(not(unix))]
pub fn hook_stdout() -> std::io::Stdout {
    std::io::stdout()
}

pub fn run_compact_hook_cli<R: Read, W: Write>(
    stdin: R,
    mut stdout: W,
    teams_dir: &Path,
) -> Result<(), CoordinationError> {
    let mut host_guard = None;
    let response = match read_hosted_hook(stdin, teams_dir, &mut host_guard) {
        Ok(response) => response,
        // Preserve the hosted boundary's silent refusal: this attachment has
        // not authorized any output, and no drain offer was requested.
        Err(error) if host_guard.is_some() => return Err(error),
        Err(error) => {
            stdout.write_all(b"{}\n")?;
            stdout.flush()?;
            return Err(error);
        }
    };
    let output_result = (|| -> Result<(), CoordinationError> {
        let bytes = serde_json::to_vec(&response)
            .map_err(|error| CoordinationError::StoreError(error.to_string()))?;
        stdout.write_all(&bytes)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
        Ok(())
    })();
    // Release the final output executor before any receipt; this offer emits no more bytes.
    // Native fd 1 is dead from here on and may be reused by child sockets. Never use stdout again.
    drop(stdout);
    if let Some(offer) = response.drain_receipt {
        offer.finish(output_result.is_ok());
    }
    output_result?;
    if let Some((root, team, member, receipt)) = response.receipt {
        if let Err(error) = crate::coordination::recovery_delivery::observe(
            &crate::coordination::stores::TeamRootRegistry::new(teams_dir.to_path_buf()),
            &root,
            &team,
            &member,
            &receipt,
            crate::coordination::recovery_card::ReceiptStage::HookResponseOffered,
        ) {
            // Stdout is already offered. Keep the receipt unknown without failing the hook.
            tracing::warn!(team, member, %error, "hook response offered; recovery receipt commit failed");
        }
    }
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
    // Hook payloads may contain prompts/tool arguments. Only their size is diagnostic.
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
        insert_optional_string(&mut fields, "cwd", payload.project_dir().map(path_display));
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
    taurhaus_claude_dir: Option<&Path>,
) -> Result<bool, CoordinationError> {
    let hooks_dir = config_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;
    let runtime = detect_hook_runtime(config_dir);
    let script_path = hooks_dir.join(platform_hook_filename(runtime));
    let script_changed = write_hook_script_with_claude_dir(
        &script_path,
        taurhaus_exe,
        runtime,
        taurhaus_claude_dir,
    )?;
    let executable_changed = write_hook_executable_record(&hooks_dir, taurhaus_exe, runtime)?;
    let settings_changed = ensure_settings_hook_entry(
        &config_dir.join(settings_filename),
        &script_path,
        runtime,
        additional_context_limit,
    )?;
    Ok(script_changed || executable_changed || settings_changed)
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
    let executable_record_changed = match fs::remove_file(
        config_dir
            .join("hooks")
            .join(HOOK_EXECUTABLE_RECORD_FILENAME),
    ) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(CoordinationError::Io(error)),
    };
    Ok(settings_changed || script_changed || executable_record_changed)
}

fn source_hook_is_installed(config_dir: &Path, settings_filename: &str) -> bool {
    let runtime = detect_hook_runtime(config_dir);
    let hooks_dir = config_dir.join("hooks");
    let script_path = hooks_dir.join(platform_hook_filename(runtime));
    let executable = fs::read_to_string(hooks_dir.join(HOOK_EXECUTABLE_RECORD_FILENAME))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let script_is_current = executable.as_deref().is_some_and(|executable| {
        render_hook_script(Path::new(executable), runtime)
            .ok()
            .is_some_and(|expected| {
                fs::read_to_string(&script_path).is_ok_and(|script| script == expected)
            })
    });
    let executable_exists = executable
        .as_deref()
        .is_some_and(|executable| hook_executable_exists(config_dir, executable));
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
    script_is_current && executable_exists && settings_contains_hook
}

fn write_hook_executable_record(
    hooks_dir: &Path,
    taurhaus_exe: &Path,
    runtime: HookRuntime,
) -> Result<bool, CoordinationError> {
    let executable = runtime_path_string(taurhaus_exe, runtime)?;
    let record_path = hooks_dir.join(HOOK_EXECUTABLE_RECORD_FILENAME);
    let changed = fs::read_to_string(&record_path)
        .map(|current| current.trim() != executable)
        .unwrap_or(true);
    if changed {
        fs::write(record_path, executable.as_bytes())?;
    }
    Ok(changed)
}

fn hook_executable_exists(config_dir: &Path, executable: &str) -> bool {
    let config_dir = config_dir.to_string_lossy();
    let visible_path = if executable.starts_with('/') {
        path::wsl_distro_from_path(&config_dir)
            .map(|distro| PathBuf::from(path::linux_to_wsl_unc(executable, &distro)))
            .unwrap_or_else(|| PathBuf::from(executable))
    } else {
        PathBuf::from(executable)
    };
    visible_path.is_file()
}

fn write_hook_script(
    script_path: &Path,
    taurhaus_exe: &Path,
    runtime: HookRuntime,
) -> Result<bool, CoordinationError> {
    write_hook_script_with_claude_dir(script_path, taurhaus_exe, runtime, None)
}

fn write_hook_script_with_claude_dir(
    script_path: &Path,
    taurhaus_exe: &Path,
    runtime: HookRuntime,
    taurhaus_claude_dir: Option<&Path>,
) -> Result<bool, CoordinationError> {
    let script_body =
        render_hook_script_with_claude_dir(taurhaus_exe, runtime, taurhaus_claude_dir)?;
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
    render_hook_script_with_claude_dir(taurhaus_exe, runtime, None)
}

fn render_hook_script_with_claude_dir(
    taurhaus_exe: &Path,
    runtime: HookRuntime,
    taurhaus_claude_dir: Option<&Path>,
) -> Result<String, CoordinationError> {
    let executable = runtime_path_string(taurhaus_exe, runtime)?;
    let claude_dir = taurhaus_claude_dir
        .map(|path| runtime_path_string(path, runtime))
        .transpose()?;
    Ok(match runtime {
        HookRuntime::Windows => claude_dir.map_or_else(
            || format!("@echo off\r\n\"{}\" --compact-hook\r\n", executable),
            |claude_dir| {
                format!(
                    "@echo off\r\nset \"TAURHAUS_CLAUDE_DIR={}\"\r\n\"{}\" --compact-hook\r\n",
                    claude_dir, executable
                )
            },
        ),
        HookRuntime::Posix => format!(
            "#!/usr/bin/env bash\nset -euo pipefail\n{}exec {} --compact-hook\n",
            claude_dir
                .map(|claude_dir| format!(
                    "TAURHAUS_CLAUDE_DIR={} ",
                    shell_quote_string(&claude_dir)
                ))
                .unwrap_or_default(),
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
            // These files are read continuously by external tools (Claude
            // Code, Codex, Grok) with zero torn-read tolerance: never
            // truncate them in place. The move-aside publish keeps the path
            // holding complete content on both sides of the swap.
            crate::coordination::stores::lock::report_atomic_write_degraded(
                settings_path,
                "settings_hooks",
                err.raw_os_error(),
            );
            if let Err(publish_err) =
                crate::coordination::stores::lock::replace_via_move_aside(&tmp_path, settings_path)
            {
                let _ = fs::remove_file(&tmp_path);
                return Err(CoordinationError::Io(publish_err));
            }
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

use crate::coordination::stores::lock::is_windows_unsupported_rename_error;

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
pub(crate) mod tests {
    use super::*;

    use chrono::DateTime;
    use fs2::FileExt;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    use crate::coordination::domain::{HealthState, MemberRole};
    use crate::coordination::stores::{
        MemberCompactionStore, MemberRuntimeRecord, MeshInboxStore,
        OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
        OperationalOwnershipSnapshot, OperationalTaskSnapshot, OperationalWorkingSetSnapshot,
        TeamConfig,
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
            account_id: None,
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
                team_incarnation_id: None,
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
                recovery: Default::default(),
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
                applied_effort: None,
                effort_resume_failure: None,
                launch_account: Default::default(),
                extra: Default::default(),
                ..Default::default()
            },
        )
        .expect("save runtime");
    }

    pub(crate) fn write_snapshot_fixture(teams_dir: &Path, team_name: &str, member_name: &str) {
        OperationalContextSnapshotStore::save(
            teams_dir,
            &OperationalContextSnapshot {
                recovery_card: None,
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
                    ..Default::default()
                },
                assignment_footer: OperationalAssignmentFooterSnapshot {
                    execution_mode: "implement".to_string(),
                    file_ownership_boundary: vec![
                        "src-tauri/src/coordination/compact_hook.rs".to_string()
                    ],
                    adjacent_fix_policy: "no".to_string(),
                    validation_expectation: "cargo check --tests".to_string(),
                    response_expectation: "report-on-completion".to_string(),
                    task_effort: String::new(),
                    task_effort_why: String::new(),
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

    fn grok_member(project_path: &Path) -> Member {
        let mut member = sample_member(project_path);
        member.cli_tool = CliTool::Grok;
        member
    }

    /// grok's own envelope: camelCase keys whose `hookEventName` *value* is
    /// snake_case, a workspace root instead of a cwd, and a transcript path
    /// under its home rather than Claude's. Spelling per
    /// `~/.grok/docs/user-guide/10-hooks.md` ("Input": `"hookEventName":
    /// "pre_tool_use"`), which is also what the imported Claude registration
    /// receives — grok sends its own envelope to every hook it runs.
    fn grok_payload(project: &Path, session_id: &str) -> String {
        json!({
            "hookEventName": "session_start",
            "sessionId": session_id,
            "source": "compact",
            "workspaceRoot": project,
            "transcriptPath": project.join(".grok/sessions/%2Fp/session/updates.jsonl"),
            "permissionMode": "bypassPermissions",
            "timestamp": "2026-08-28T12:00:00Z",
        })
        .to_string()
    }

    /// grok's own compaction event on the wire: `post_compact`, with a
    /// compaction trigger where `session_start` would carry a start source, and
    /// the common fields every grok event carries.
    fn grok_post_compact_payload(project: &Path, session_id: &str) -> String {
        json!({
            "hookEventName": "post_compact",
            "sessionId": session_id,
            "trigger": "auto",
            "cwd": project,
            "workspaceRoot": project,
            "transcriptPath": project.join(".grok/sessions/%2Fp/session/updates.jsonl"),
            "permissionMode": "default",
            "timestamp": "2026-08-28T12:00:00Z",
        })
        .to_string()
    }

    #[test]
    fn grok_hook_installer_owns_one_always_trusted_file_and_is_removable() {
        // Regression: commit 358a7c9 registered grok without a compaction slice,
        // so its always-trusted `~/.grok/hooks/*.json` directory went unused and
        // the only reinjection path left was the Claude import it also loads.
        let temp = tempfile::tempdir().expect("grok hook root");
        let home = temp.path().join(".grok");
        let executable = temp.path().join("taurhaus");
        fs::write(&executable, b"fixture").expect("fixture executable");
        let source = spec(CliTool::Grok)
            .compaction_signal_source()
            .expect("declared grok compaction source");

        assert!(source.install(&home, &executable).expect("first install"));
        assert!(!source.install(&home, &executable).expect("idempotent"));
        assert!(grok_compact_hook_is_installed_at(&home));

        let document: Value = serde_json::from_slice(
            &fs::read(home.join("hooks").join(GROK_HOOKS_FILENAME)).expect("hook document"),
        )
        .expect("hook document parses");
        assert_eq!(
            document["hooks"]["SessionStart"][0]["matcher"],
            COMPACT_SOURCE
        );
        assert_eq!(
            document["hooks"]["PostCompact"][0]["matcher"],
            "manual|auto"
        );
        assert_eq!(
            document["hooks"]["PostCompact"][0]["hooks"][0]["timeout"],
            30
        );

        assert!(source.remove(&home).expect("first removal"));
        assert!(!source.remove(&home).expect("idempotent removal"));
        assert!(!grok_compact_hook_is_installed_at(&home));
    }

    #[test]
    fn grok_hook_installer_self_repairs_a_missing_executable() {
        // Regression: commit 358a7c9 had no grok installer; a hook pointing at a
        // deleted binary must be removed rather than left to fail on every turn.
        let temp = tempfile::tempdir().expect("grok hook root");
        let home = temp.path().join(".grok");
        let executable = temp.path().join("taurhaus");
        fs::write(&executable, b"fixture").expect("fixture executable");
        ensure_grok_compact_hook_installed_at(&home, &executable).expect("install");
        fs::remove_file(&executable).expect("remove executable");

        assert!(ensure_grok_compact_hook_installed_at(&home, &executable).expect("self repair"));
        assert!(!grok_compact_hook_is_installed_at(&home));
    }

    #[test]
    fn grok_camel_case_envelope_is_reinjected_through_its_workspace_root() {
        // Regression: commit 358a7c9 left the bridge inferring the harness from
        // a Claude- or Codex-shaped transcript path only, so grok's camelCase
        // envelope — which carries `workspaceRoot`, not `cwd` — never matched a
        // managed member.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = grok_member(&project);
        write_team_fixture(tmp.path(), "grok-team", &member, "01a04585-2d53-7123");
        write_snapshot_fixture(tmp.path(), "grok-team", &member.name);

        handle_compact_hook(&grok_payload(&project, "01a04585-2d53-7123"), tmp.path())
            .expect("hook should succeed");

        let inbox =
            MeshInboxStore::load(tmp.path(), "grok-team", &member.name).expect("grok inbox");
        assert_eq!(inbox.len(), 1);
        assert!(inbox[0].text.contains("Current task: #680"));
    }

    #[test]
    fn a_grok_compaction_is_delivered_through_the_mesh_inbox() {
        // Regression: commit c1005ec answered a grok compaction with
        // `hookSpecificOutput.additionalContext` on stdout and recorded the card
        // as Injected, but grok 1.0.5 documents passive-hook stdout as ignored
        // ("Passive Hooks", `~/.grok/docs/user-guide/10-hooks.md`), so the
        // restored context was never handed to the model.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = grok_member(&project);
        write_team_fixture(tmp.path(), "grok-team", &member, "01a04585-2d53-7123");
        write_snapshot_fixture(tmp.path(), "grok-team", &member.name);

        let response =
            handle_compact_hook(&grok_payload(&project, "01a04585-2d53-7123"), tmp.path())
                .expect("hook should succeed");

        assert_eq!(
            response,
            CompactHookResponse::default(),
            "grok ignores passive-hook stdout, so the bridge must not answer with one"
        );
        let inbox =
            MeshInboxStore::load(tmp.path(), "grok-team", &member.name).expect("grok inbox");
        assert_eq!(inbox.len(), 1, "the card is queued where grok reads it");
        assert!(inbox[0].text.contains("Current task: #680"));
        assert_eq!(inbox[0].summary.as_deref(), Some("post_compaction_context"));
        let state = MemberCompactionStore::load(tmp.path(), "grok-team", &member.name)
            .expect("compaction state")
            .expect("recorded delivery");
        assert_eq!(
            state.last_delivery_result,
            CompactionDeliveryResult::Injected
        );
    }

    #[cfg(unix)]
    #[test]
    fn canonical_grok_compaction_retains_acceptance_and_never_resends_failure() {
        let _log_guard = taurhaus_lib::test_support::acquire_global_log_test_guard();
        for refused in [false, true] {
            let mesh = crate::coordination::mesh_cli::FakeMesh::new(
                r#"echo '{"journal_writer":"mesh-journal/2"}'"#,
                if refused {
                    "exit 23"
                } else {
                    r#"echo '{"status":"accepted","message_id":"m1","sequence":1,"projection":"pending","delivery_targets":[{"recipient":"architect","delivery_id":"d1"}]}'"#
                },
            );
            let root = mesh.dir.path().join("teams");
            let project = mesh.dir.path().join("project");
            fs::create_dir_all(&project).unwrap();
            let member = grok_member(&project);
            assert_eq!(member.name, "architect");
            write_team_fixture(&root, "grok-team", &member, "grok-session");
            write_snapshot_fixture(&root, "grok-team", &member.name);
            let mut config = TeamConfigStore::load(&root, "grok-team").unwrap();
            config.team_incarnation_id = Some("grok-incarnation".into());
            let mut lead = member.clone();
            lead.name = "lead".into();
            lead.role = crate::coordination::domain::MemberRole::Lead;
            config.members.push(lead);
            config
                .extra
                .insert("messaging_format".into(), serde_json::json!(2));
            TeamConfigStore::save(&root, "grok-team", &config).unwrap();
            crate::coordination::recovery_delivery::reserve_activation(
                &root,
                "grok-team",
                &member.name,
                "activation",
            )
            .unwrap();
            let result = handle_compact_hook(&grok_payload(&project, "grok-session"), &root);
            assert_eq!(result.is_err(), refused);
            let state = MemberCompactionStore::load(&root, "grok-team", &member.name)
                .unwrap()
                .unwrap();
            let wire = serde_json::to_value(&state).unwrap();
            if !refused {
                assert_eq!(wire["journal"]["message_id"], "m1");
                assert_eq!(wire["journal"]["delivery_id"], "d1");
                assert!(mesh.argv().contains("--task\n680\n"));
            }
            handle_compact_hook(&grok_payload(&project, "grok-session"), &root).unwrap();
            assert_eq!(mesh.argv().matches("accept\n").count(), 1);
            assert!(MeshInboxStore::load(&root, "grok-team", &member.name)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn a_failed_grok_inbox_append_is_never_recorded_as_injected() {
        // Regression: commit c1005ec recorded Injected before any delivery
        // happened, so a card that never reached the member still suppressed the
        // next compaction through the compat-import dedupe window.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = grok_member(&project);
        write_team_fixture(tmp.path(), "grok-team", &member, "01a04585-2d53-7123");
        write_snapshot_fixture(tmp.path(), "grok-team", &member.name);
        // A file where the inbox directory belongs makes the append fail.
        fs::write(tmp.path().join("grok-team").join("inboxes"), b"not a dir")
            .expect("block the inbox directory");

        handle_compact_hook(&grok_payload(&project, "01a04585-2d53-7123"), tmp.path())
            .expect_err("a blocked inbox is a delivery failure");

        let state = MemberCompactionStore::load(tmp.path(), "grok-team", &member.name)
            .expect("compaction state")
            .expect("recorded delivery");
        assert_eq!(state.last_delivery_result, CompactionDeliveryResult::Failed);
    }

    #[test]
    fn one_grok_compaction_yields_one_reinjection_despite_the_claude_import() {
        // Regression: commit 358a7c9 registered taurhaus's own grok hook while
        // grok also runs the Claude registration it imports from
        // `~/.claude/settings.json`, so a single compaction called this bridge
        // twice and the member received the card twice.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = grok_member(&project);
        write_team_fixture(tmp.path(), "grok-team", &member, "01a04585-2d53-7123");
        write_snapshot_fixture(tmp.path(), "grok-team", &member.name);

        handle_compact_hook(
            &grok_post_compact_payload(&project, "01a04585-2d53-7123"),
            tmp.path(),
        )
        .expect("native PostCompact");
        handle_compact_hook(&grok_payload(&project, "01a04585-2d53-7123"), tmp.path())
            .expect("imported Claude SessionStart");

        assert_eq!(
            MeshInboxStore::load(tmp.path(), "grok-team", &member.name)
                .expect("grok inbox")
                .len(),
            1,
            "the imported Claude registration must not reinject the same compaction"
        );
    }

    #[test]
    fn a_native_grok_post_compact_delivers_one_inbox_card() {
        // Regression: commit c1005ec answered grok's authoritative `PostCompact`
        // event with a signal-only skip that returned before member resolution,
        // so a grok home without the imported Claude registration got no
        // restored context at all. grok's `SessionStart` matcher tests the start
        // source (`startup`, `resume`, …) and never reports `compact`
        // (`~/.grok/docs/user-guide/10-hooks.md`), so `PostCompact` is the only
        // compaction event grok itself fires.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = grok_member(&project);
        write_team_fixture(tmp.path(), "grok-team", &member, "01a04585-2d53-7123");
        write_snapshot_fixture(tmp.path(), "grok-team", &member.name);

        let response = handle_compact_hook(
            &grok_post_compact_payload(&project, "01a04585-2d53-7123"),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(
            response,
            CompactHookResponse::default(),
            "grok ignores passive-hook stdout, so PostCompact must answer with none"
        );
        let inbox =
            MeshInboxStore::load(tmp.path(), "grok-team", &member.name).expect("grok inbox");
        assert_eq!(inbox.len(), 1, "the card is queued where grok reads it");
        assert!(inbox[0].text.contains("Current task: #680"));
        let state = MemberCompactionStore::load(tmp.path(), "grok-team", &member.name)
            .expect("compaction state")
            .expect("recorded delivery");
        assert_eq!(
            state.last_delivery_result,
            CompactionDeliveryResult::Injected
        );
    }

    #[test]
    fn a_hook_event_is_recognised_in_either_harness_spelling() {
        // Regression: commit c1005ec compared `hookEventName` byte-for-byte
        // against the PascalCase names hooks are *registered* under, but grok
        // puts a snake_case value on the wire
        // (`~/.grok/docs/user-guide/10-hooks.md` documents `"hookEventName":
        // "pre_tool_use"`). Every native `post_compact` therefore took the
        // non-compaction early return before any member was resolved, so a grok
        // member got no restored context at all.
        for (raw, canonical, expected) in [
            ("PostCompact", POST_COMPACT_HOOK_EVENT, true),
            ("post_compact", POST_COMPACT_HOOK_EVENT, true),
            ("SessionStart", SESSION_START_HOOK_EVENT, true),
            ("session_start", SESSION_START_HOOK_EVENT, true),
            // A neighbouring event must never be read as the compaction one.
            ("PreCompact", POST_COMPACT_HOOK_EVENT, false),
            ("pre_compact", POST_COMPACT_HOOK_EVENT, false),
            ("SessionEnd", SESSION_START_HOOK_EVENT, false),
            ("session_end", SESSION_START_HOOK_EVENT, false),
        ] {
            assert_eq!(
                hook_event_is(raw, canonical),
                expected,
                "'{raw}' against '{canonical}'"
            );
        }
    }

    #[test]
    fn claude_and_codex_transcript_inference_is_unchanged_by_the_grok_arm() {
        // Regression: commit 358a7c9 must not let the new `/.grok/` marker or
        // the camelCase aliases reclassify the two harnesses already bridged.
        for (path, expected) in [
            (
                "/home/user/.claude/projects/-home-user-p/session.jsonl",
                Some(CliTool::Claude),
            ),
            (
                "/home/user/.codex/sessions/2026/08/28/rollout-session.jsonl",
                Some(CliTool::Codex),
            ),
            (
                "/home/user/.grok/sessions/%2Fhome%2Fuser/01a04585/updates.jsonl",
                Some(CliTool::Grok),
            ),
            ("/home/user/notes.txt", None),
        ] {
            assert_eq!(
                infer_tool_from_transcript_path(Some(Path::new(path))),
                expected,
                "{path}"
            );
        }
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
            .contains("[taurhaus] recovery_card"));
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
            .contains("Role: taurhaus-architect"));
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
    fn codex_compact_hook_resolves_member_under_registered_team_root() {
        // Regression: 18810949 introduced selected Claude team roots without
        // teaching the global Codex hook to resolve members outside the default root.
        let guard = acquire_env_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let default_claude_dir = tmp.path().join("default-claude");
        let default_teams_dir = default_claude_dir.join("teams");
        let work_teams_dir = tmp.path().join("work-claude").join("teams");
        guard.set_override(&default_claude_dir);

        crate::coordination::stores::TeamRootRegistry::new(default_teams_dir.clone())
            .set("work-team", &work_teams_dir)
            .expect("register work team root");

        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let mut member = sample_member(&project);
        member.name = "codex-architect".to_string();
        member.cli_tool = CliTool::Codex;
        write_team_fixture(&work_teams_dir, "work-team", &member, "codex-session");
        write_snapshot_fixture(&work_teams_dir, "work-team", &member.name);

        let response = handle_compact_hook(
            &json!({
                "hook_event_name": "SessionStart",
                "session_id": "codex-session",
                "source": "compact",
                "cwd": &project,
                "transcript_path": project.join(
                    ".codex/sessions/2026/09/03/rollout-2026-09-03T10-00-00-codex-session.jsonl"
                ),
            })
            .to_string(),
            &default_teams_dir,
        )
        .expect("Codex hook should resolve the registered-root member");

        assert!(response.hook_specific_output.is_some());
        assert!(
            MemberCompactionStore::load(&work_teams_dir, "work-team", &member.name)
                .expect("load work-root delivery state")
                .is_some()
        );
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
        assert!(!contents.contains("raw_payload"));
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
                team_incarnation_id: None,
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
                    recovery: Default::default(),
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
                    applied_effort: None,
                    effort_resume_failure: None,
                    launch_account: Default::default(),
                    extra: Default::default(),
                    ..Default::default()
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
        let script = fs::read_to_string(&script_path).expect("script exists");
        assert!(script.contains(&format!(
            "TAURHAUS_CLAUDE_DIR={}",
            shell_quote_string(&tmp.path().display().to_string())
        )));
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
    fn default_root_hook_script_keeps_the_unpinned_historical_bytes() {
        // Regression: a4fd2cf2 pinned every Claude hook script, rewriting the
        // default-root script even though its resolution behavior was unchanged.
        let guard = acquire_env_test_guard();
        let tmp = tempfile::tempdir().expect("tempdir");
        let claude_dir = tmp.path().join("claude");
        let teams_dir = claude_dir.join("teams");
        guard.set_override(&claude_dir);
        fs::create_dir_all(&teams_dir).expect("teams dir");
        let exe_path = tmp.path().join("taurhaus");
        fs::write(&exe_path, b"binary").expect("exe path");

        ensure_compact_hook_installed(&teams_dir, &exe_path).expect("install default hook");

        let script_path = claude_dir
            .join("hooks")
            .join(platform_hook_filename(HookRuntime::Posix));
        assert_eq!(
            fs::read_to_string(script_path).expect("script"),
            render_hook_script(&exe_path, HookRuntime::Posix).expect("historical script")
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
            assert!(context.contains("[taurhaus] recovery_card"));

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
    fn codex_hook_is_not_active_when_its_installed_executable_is_missing() {
        // Regression: 6fe0aa3 disabled the transcript owner from hook-file
        // presence alone even when the daemon referenced by the hook was absent.
        let tmp = tempfile::tempdir().expect("tempdir");
        let codex_home = tmp.path().join("isolated-codex-home");
        let executable = tmp.path().join("taurhaus-daemon");
        fs::write(&executable, b"daemon").expect("executable fixture");

        ensure_codex_compact_hook_installed_at(&codex_home, &executable).expect("install hook");
        assert!(codex_compact_hook_is_installed_at(&codex_home));

        fs::remove_file(&executable).expect("remove executable fixture");
        assert!(!codex_compact_hook_is_installed_at(&codex_home));
    }

    #[test]
    fn codex_installer_does_not_register_a_missing_executable() {
        // Regression: 80ee59e made the installed-state check require the hook
        // executable but still registered a missing executable in hooks.json.
        let tmp = tempfile::tempdir().expect("tempdir");
        let codex_home = tmp.path().join("isolated-codex-home");
        let missing_executable = tmp.path().join("missing-taurhaus-daemon");

        ensure_codex_compact_hook_installed_at(&codex_home, &missing_executable)
            .expect("skip unavailable hook executable");

        let hooks = fs::read_to_string(codex_home.join("hooks.json")).unwrap_or_default();
        assert!(!hooks.contains(TAURHAUS_COMPACT_HOOK_BASENAME));
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
    fn compact_hook_preserves_opted_in_pane_recovery_and_timestamp() {
        // Regression: 6fe0aa3 recorded Utc::now() for hook delivery while the
        // transcript fallback recorded the compacted event timestamp, defeating dedupe.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let mut member = sample_member(&project);
        member.cli_tool = CliTool::Codex;
        // Regression: 4cae348e treated opt-in as a published host, dropping pane recovery.
        member
            .extra
            .insert("adapter_mode".into(), json!("app_server"));
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

        let response = handle_compact_hook(
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
        assert!(response
            .hook_specific_output
            .unwrap()
            .additional_context
            .contains("Current task: #680"));
        let record = MemberRuntimeStore::load(tmp.path(), "codex-team", &member.name).unwrap();
        assert!(record.pane_id.is_some() && record.app_server.is_none());

        let state = MemberCompactionStore::load(tmp.path(), "codex-team", &member.name)
            .expect("load compaction state")
            .expect("compaction state");
        assert_eq!(
            state.last_delivery_result,
            CompactionDeliveryResult::Injected
        );
        assert_eq!(state.last_session_id, "session-codex");
        assert_eq!(
            state.last_compaction_timestamp,
            DateTime::parse_from_rfc3339("2026-08-26T06:00:00.123Z")
                .expect("timestamp")
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn claude_compact_hook_does_not_scan_codex_boundaries_from_its_transcript() {
        // Regression: 6fe0aa3 scanned and JSON-parsed the entire transcript for
        // both tools even though Codex is the only supported boundary schema.
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "claude-team", &member, "session-claude");
        write_snapshot_fixture(tmp.path(), "claude-team", &member.name);
        let transcript_path = project.join(".claude/projects/session-claude.jsonl");
        fs::create_dir_all(transcript_path.parent().expect("transcript parent"))
            .expect("create transcript parent");
        fs::write(
            &transcript_path,
            "{\"timestamp\":\"2020-01-01T00:00:00.000Z\",\"type\":\"compacted\",\"payload\":{}}\n",
        )
        .expect("write Claude transcript fixture");

        let invoked_at = Utc::now();
        handle_compact_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "session-claude",
                "source": "compact",
                "cwd": &project,
                "transcriptPath": &transcript_path,
            })
            .to_string(),
            tmp.path(),
        )
        .expect("handle Claude compact hook");

        let state = MemberCompactionStore::load(tmp.path(), "claude-team", &member.name)
            .expect("load compaction state")
            .expect("compaction state");
        assert!(state.last_compaction_timestamp >= invoked_at);
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
    #[test]
    fn recovery_hook_receipt_failure_after_flush_keeps_successful_output() {
        // Regression: f0a5bad7 propagated receipt errors after offering valid hook stdout.
        struct MoveRootAfterFlush<'a> {
            root: &'a Path,
            bytes: Vec<u8>,
        }
        impl Write for MoveRootAfterFlush<'_> {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                crate::coordination::stores::TeamRootRegistry::new(self.root.to_path_buf())
                    .set("team", &self.root.join("moved/teams"))
                    .unwrap();
                Ok(())
            }
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("claude/teams");
        // Regression: 0e5986bf used a teams root whose registry escaped the fixture.
        assert!(
            crate::coordination::stores::TeamRootRegistry::new(root.clone())
                .path()
                .starts_with(tmp.path()),
            "team registry must stay inside its fixture"
        );
        let project = root.join("project");
        fs::create_dir_all(&project).unwrap();
        let member = sample_member(&project);
        write_team_fixture(&root, "team", &member, "session");
        write_snapshot_fixture(&root, "team", &member.name);
        let mut config = TeamConfigStore::load(&root, "team").unwrap();
        config.team_incarnation_id = Some("team-1".into());
        TeamConfigStore::save(&root, "team", &config).unwrap();
        crate::coordination::recovery_delivery::reserve_activation(
            &root,
            "team",
            &member.name,
            "activation",
        )
        .unwrap();
        let payload = json!({"hook_event_name":"SessionStart","session_id":"session","source":"compact","cwd":project,"transcript_path":root.join(".claude/projects/transcript.jsonl")}).to_string();
        let mut out = MoveRootAfterFlush {
            root: &root,
            bytes: Vec::new(),
        };
        assert!(run_compact_hook_cli(payload.as_bytes(), &mut out, &root).is_ok());
        let response: serde_json::Value = serde_json::from_slice(&out.bytes).unwrap();
        assert!(response["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("[taurhaus] recovery_card"));
        let runtime = MemberRuntimeStore::load(&root, "team", &member.name).unwrap();
        assert_eq!(
            runtime.recovery.claim.unwrap().stage,
            crate::coordination::recovery_card::ReceiptStage::OutcomeUnknown
        );
    }

    #[test]
    fn recovery_hook_output_failure_is_unknown_and_success_is_only_offered() {
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        for broken in [true, false] {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path().join("claude/teams");
            // Regression: 0e5986bf used a teams root whose registry escaped the fixture.
            assert!(
                crate::coordination::stores::TeamRootRegistry::new(root.clone())
                    .path()
                    .starts_with(tmp.path()),
                "team registry must stay inside its fixture"
            );
            let project = root.join("project");
            fs::create_dir_all(&project).unwrap();
            let member = sample_member(&project);
            write_team_fixture(&root, "team", &member, "session");
            write_snapshot_fixture(&root, "team", &member.name);
            let mut config = TeamConfigStore::load(&root, "team").unwrap();
            config.team_incarnation_id = Some("team-1".into());
            TeamConfigStore::save(&root, "team", &config).unwrap();
            crate::coordination::recovery_delivery::reserve_activation(
                &root,
                "team",
                &member.name,
                "activation",
            )
            .unwrap();
            let payload = json!({"hook_event_name":"SessionStart","session_id":"session","source":"compact","cwd":project,"transcript_path":root.join(".claude/projects/transcript.jsonl")}).to_string();
            if broken {
                assert!(run_compact_hook_cli(payload.as_bytes(), Broken, &root).is_err());
            } else {
                let mut out = Vec::new();
                run_compact_hook_cli(payload.as_bytes(), &mut out, &root).unwrap();
                let output: serde_json::Value = serde_json::from_slice(&out).unwrap();
                crate::coordination::recovery_card::assert_control_golden(
                    output["hookSpecificOutput"]["additionalContext"]
                        .as_str()
                        .unwrap(),
                );
            }
            let runtime = MemberRuntimeStore::load(&root, "team", &member.name).unwrap();
            let receipt = runtime.recovery.claim.expect("persisted hook receipt");
            assert_eq!(
                receipt.stage,
                if broken {
                    crate::coordination::recovery_card::ReceiptStage::OutcomeUnknown
                } else {
                    crate::coordination::recovery_card::ReceiptStage::HookResponseOffered
                }
            );
            assert_eq!(receipt.accepted_bytes, 0);
            assert!(MeshInboxStore::load(&root, "team", &member.name)
                .unwrap()
                .is_empty());
        }
    }
    #[cfg(unix)]
    fn hook_drain_fixture() -> (crate::coordination::mesh_cli::FakeMesh, Value, PathBuf) {
        use crate::coordination::mesh_cli::FakeMesh;
        let fake = FakeMesh::new("exit 99", "exit 99");
        let root = fake.dir.path();
        fs::write(
            root.join("mesh.drain-enabled"),
            "fixture enabled descriptors",
        )
        .unwrap();
        let teams = root.join("teams");
        let mut member = sample_member(root);
        member.cli_tool = CliTool::Codex;
        write_team_fixture(&teams, "drain-team", &member, "verified-session");
        let config_path = teams.join("drain-team/config.json");
        let mut config: Value = serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
        config["delivery_owner"] = json!("team");
        config["messaging_format"] = json!(2);
        config["team_incarnation_id"] = json!("incarnation");
        fs::write(config_path, config.to_string()).unwrap();
        let mut record = MemberRuntimeStore::load(&teams, "drain-team", &member.name).unwrap();
        record.terminal_contract = 1;
        record.attachment_generation = 7;
        record.context_generation = 3;
        record.tmux_socket = Some(root.join("private.sock"));
        record.tmux_session_id = Some("$9".into());
        record.pane_pid = Some(123);
        record.pane_start_time = Some(456);
        record.harness = Some(CliTool::Codex);
        record.launch_root = Some(crate::coordination::stores::runtime::LaunchRoot {
            claude_dir: root.into(),
            teams_dir: teams.clone(),
            team_incarnation_id: Some("incarnation".into()),
            root_authority_revision: 0,
        });
        MemberRuntimeStore::save(&teams, "drain-team", &member.name, &record).unwrap();
        let state = teams.join("drain-team/state/delivery");
        fs::create_dir_all(&state).unwrap();
        fs::write(
            state.join("adapter-architect.json"),
            r#"{"mode":"hook","revision":1,"boundary":{"owner_fence":2}}"#,
        )
        .unwrap();
        let script = r#"#!/usr/bin/python3
import sys,json,pathlib,os,time
root=pathlib.Path(__file__).parent
value=json.load(sys.stdin)
with (root/'calls').open('a') as f: f.write(json.dumps({'argv':sys.argv[1:],'stdin':value,'env_keys':list(os.environ)})+'\n')
protocol='mesh-hook-drain/1'
mode=(root/'mode').read_text() if (root/'mode').exists() else ''
if sys.argv[-1]=='capabilities':
 print(json.dumps({'protocol':protocol,'descriptors':[{'id':'codex/0.153.4/PostToolUse/1','harness':'codex','build':'0.153.4','host':'fixture','event':'PostToolUse','source':'ordinary','matcher':'','config_trust':'fixture','envelope':'hookSpecificOutput.additionalContext; event-specific validation required','context_entry':'fixture','drop_rules':'fixture','max_bytes':8192,'max_chars':8000,'continuation_budget':0,'enabled':True}]}))
elif sys.argv[-1]=='drain':
 if mode=='invalid': print('not json'); sys.exit(0)
 if mode=='oversized': print('x'*70000); sys.exit(0)
 if mode=='stderr': sys.stderr.write('x'*100000)
 print(json.dumps({'protocol':protocol,'status':'offered','stage':'bridge_rendered','text':'[mesh message data] fixture marker\n','deliveries':[{'message_id':'m1','delivery_id':'d1','sequence':1,'coverage':'full_body','body_bytes':14,'body_chars':14}],'attempt_ids':['a1'],'offer_id':'offer-1','owner_fence':2,'selection_revision':1,'continue':False}),flush=True)
 if mode=='nonzero': sys.exit(7)
 if mode=='timeout': time.sleep(10)
elif mode=='receipt-failure': sys.exit(7)
else: print(json.dumps({'protocol':protocol,'status':'recorded','text':'','deliveries':[],'offer_id':None,'continue':False}))
"#;
        fs::write(root.join("mesh"), script).unwrap();
        let payload = json!({"hook_event_name":"PostToolUse","session_id":"verified-session","cwd":root,"transcript_path":root.join(".codex/rollout-test.jsonl")});
        (fake, payload, teams)
    }

    #[cfg(unix)]
    #[test]
    fn hosted_compaction_delivers_recovery_without_mesh_drain_subprocesses() {
        // Regression: 7b686594 ran drain capabilities, delivery and receipt subprocesses
        // under HostOperationLock, exceeding its budget and blocking operator controls.
        let (fake, mut payload, teams) = hook_drain_fixture();
        let root = fake.dir.path();
        let script = fs::read_to_string(root.join("mesh"))
            .unwrap()
            .replace("PostToolUse", "SessionStart")
            .replace("'source':'ordinary'", "'source':'compact'");
        fs::write(root.join("mesh"), script).unwrap();
        write_snapshot_fixture(&teams, "drain-team", "architect");
        payload["hook_event_name"] = json!("SessionStart");
        payload["source"] = json!("compact");
        let mut runtime = MemberRuntimeStore::load(&teams, "drain-team", "architect").unwrap();
        runtime.app_server = Some(crate::coordination::stores::runtime::AppServerAttachment {
            contract: 1,
            socket_path: root.join("host.sock"),
            thread_id: "verified-session".into(),
            member_id: "architect".into(),
            account_root: root.into(),
            process_id: 123,
            process_start: "456".into(),
            host_generation: "host-1".into(),
            build: "fixture".into(),
            host: "fixture".into(),
            configuration: "fixture".into(),
            configuration_digest: None,
            instruction_sources: Vec::new(),
            trust: "fixture".into(),
            transport: "unix-websocket".into(),
            attach_argv: Vec::new(),
            state: "ready".into(),
        });
        MemberRuntimeStore::save(&teams, "drain-team", "architect", &runtime).unwrap();

        crate::coordination::recovery_delivery::reserve_activation(
            &teams,
            "drain-team",
            "architect",
            "activation",
        )
        .unwrap();
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, &teams).unwrap();
        let output: Value = serde_json::from_slice(&output).unwrap();
        assert!(output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("Current task: #680"));
        let runtime = MemberRuntimeStore::load(&teams, "drain-team", "architect").unwrap();
        assert_eq!(
            runtime.recovery.claim.unwrap().stage,
            crate::coordination::recovery_card::ReceiptStage::HookResponseOffered
        );
        assert!(
            hook_drain_calls(&fake).is_empty(),
            "hosted compaction must not spawn Mesh while excluding operator controls"
        );
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_membership_ignores_activity_but_honors_removal() {
        // Regression: 026627bb, L1 run 3: Claude activity was mistaken for membership.
        for removed in [false, true] {
            let (fake, payload, teams) = hook_drain_fixture();
            let root = fake.dir.path();
            let path = teams.join("drain-team/config.json");
            let mut config: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            config["members"][0]["isActive"] = json!(removed);
            config["members"][0]["lastActivityReason"] = json!("message_sent");
            if removed {
                config["members"][0]["removedAt"] = json!("2026-09-10T12:44:00Z");
            }
            fs::write(path, config.to_string()).unwrap();
            let mut output = Vec::new();
            run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, &teams).unwrap();
            let response: Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(
                response["hookSpecificOutput"]["additionalContext"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("fixture marker"),
                !removed
            );
            assert_eq!(
                hook_drain_calls(&fake)
                    .iter()
                    .any(|c| c["argv"][7] == "drain"),
                !removed
            );
            let bindings = vec![(teams, "drain-team".into(), "architect".into())];
            drain::reconcile_home(root, CliTool::Codex, &bindings, &root.join("mesh")).unwrap();
            let settings = fs::read_to_string(root.join("hooks.json")).unwrap_or_default();
            assert_eq!(settings.contains("taurhaus-delivery-drain"), !removed);
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_managed_boundary_round_trip() {
        let (fake, payload, teams) = hook_drain_fixture();
        let root = fake.dir.path();
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, &teams).unwrap();
        let output: Value = serde_json::from_slice(&output).unwrap();
        assert!(output["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or_default()
            .contains("fixture marker"));
        let calls: Vec<Value> = fs::read_to_string(root.join("calls"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(calls.len(), 3);
        for call in &calls {
            assert!(call["env_keys"]
                .as_array()
                .unwrap()
                .iter()
                .all(|key| matches!(key.as_str(), Some("LANG" | "LC_CTYPE"))));
        }
        assert_eq!(
            calls[1]["argv"],
            json!([
                "--claude-dir",
                root,
                "--team",
                "drain-team",
                "--name",
                "architect",
                "delivery",
                "drain"
            ])
        );
        let request = &calls[1]["stdin"]["request"];
        assert_eq!(request["runtime"]["context_generation"], "3");
        assert_eq!(request["runtime"]["attachment_generation"], 7);
        assert_eq!(request["runtime"]["pane_start"], "456");
        assert_eq!(request["runtime"]["tmux_session"], "$9");
        assert_eq!(request["session"], "verified-session");
        assert_eq!(calls[2]["stdin"]["request"], *request);
        assert_eq!(calls[2]["stdin"]["stage"], "hook_response_offered");
        assert_eq!(calls[2]["stdin"]["offer_id"], "offer-1");
    }

    #[cfg(unix)]
    #[test]
    fn disabled_drain_descriptors_skip_installer_and_account_home_probes() {
        // Regression: 260cf1e3 gated teardown as well as installation, orphaning owned hooks.
        let (fake, _, teams) = hook_drain_fixture();
        let root = fake.dir.path();
        for tool in [CliTool::Codex, CliTool::Claude] {
            let filename = if tool == CliTool::Codex {
                "hooks.json"
            } else {
                "settings.json"
            };
            let script = root
                .join("hooks")
                .join(format!("taurhaus-delivery-drain-{}.sh", "a".repeat(64)));
            fs::write(root.join(filename), json!({"hooks":{"PostToolUse":[{"hooks":[
                {"type":"command", "command": settings_command_for_script(&script, HookRuntime::Posix).unwrap()},
                {"type":"command", "command":"foreign-hook"}
            ]}]}}).to_string()).unwrap();
        }
        fs::remove_file(root.join("mesh.drain-enabled")).unwrap();
        drain::reconcile_home(root, CliTool::Codex, &[], &root.join("mesh")).unwrap();
        let hooks = fs::read_to_string(root.join("hooks.json")).unwrap();
        assert!(!hooks.contains("taurhaus-delivery-drain"));
        assert!(hooks.contains("foreign-hook"));
        let mesh = root.join("mesh");
        fs::write(
            &mesh,
            fs::read_to_string(&mesh)
                .unwrap()
                .replace("codex", "claude"),
        )
        .unwrap();
        let config = teams.join("drain-team/config.json");
        fs::write(
            &config,
            fs::read_to_string(&config)
                .unwrap()
                .replace("codex", "claude"),
        )
        .unwrap();
        for _ in 0..2 {
            ensure_compact_hook_installed(&teams, &mesh).unwrap();
            drain::reconcile_home(
                root,
                CliTool::Claude,
                &[(teams.clone(), "drain-team".into(), "architect".into())],
                &mesh,
            )
            .unwrap();
        }
        assert!(
            hook_drain_calls(&fake).is_empty(),
            "disabled descriptors must never spawn capability probes"
        );
        assert!(!fs::read_to_string(root.join("settings.json"))
            .unwrap()
            .contains("taurhaus-delivery-drain"));
    }

    #[test]
    fn hook_drain_claude_installer_is_gated_and_idempotent() {
        let (fake, _, teams) = hook_drain_fixture();
        let root = fake.dir.path();
        let script = fs::read_to_string(root.join("mesh"))
            .unwrap()
            .replace("codex", "claude");
        fs::write(root.join("mesh"), script).unwrap();
        let config_path = teams.join("drain-team/config.json");
        let config = fs::read_to_string(&config_path)
            .unwrap()
            .replace("codex", "claude");
        fs::write(config_path, config).unwrap();
        let settings_path = root.join("settings.json");
        fs::write(&settings_path, r#"{"trust":"unchanged","hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"foreign"}]}]}}"#).unwrap();
        assert!(ensure_compact_hook_installed(&teams, &root.join("mesh")).unwrap());
        let settings: Value = serde_json::from_slice(&fs::read(&settings_path).unwrap()).unwrap();
        assert!(settings["hooks"]["PostToolUse"]
            .to_string()
            .contains("taurhaus-delivery-drain"));
        // Regression: 15f222bd copied Codex's config-only limit into Claude registrations.
        assert!(!settings["hooks"]["PostToolUse"]
            .to_string()
            .contains("additionalContextLimit"));
        assert_eq!(settings["trust"], "unchanged");
        assert!(settings["hooks"]["PostToolUse"]
            .to_string()
            .contains("foreign"));
        assert!(!ensure_compact_hook_installed(&teams, &root.join("mesh")).unwrap());
        let script = fs::read_to_string(root.join("mesh"))
            .unwrap()
            .replace("'enabled':True", "'enabled':False");
        fs::write(root.join("mesh"), script).unwrap();
        assert!(ensure_compact_hook_installed(&teams, &root.join("mesh")).unwrap());
        let settings: Value = serde_json::from_slice(&fs::read(&settings_path).unwrap()).unwrap();
        assert!(!settings.to_string().contains("taurhaus-delivery-drain"));
        assert!(settings
            .to_string()
            .contains("taurhaus-session-start-compact"));
    }

    #[cfg(unix)]
    fn hook_drain_calls(fake: &crate::coordination::mesh_cli::FakeMesh) -> Vec<Value> {
        fs::read_to_string(fake.dir.path().join("calls"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_ineligible_boundaries_never_call_drain() {
        // Regression: c408b68a treated absent published hook facts as verified serde defaults.
        for case in [
            "tmux",
            "non-owner",
            "legacy",
            "future",
            "disabled",
            "wrong-session",
            "missing-hook-session",
            "missing-context",
            "wrong-root",
            "dead",
            "wrong-event",
            "grok",
            "agy",
            "stop",
        ] {
            let (fake, mut payload, teams) = hook_drain_fixture();
            let root = fake.dir.path();
            let config_path = teams.join("drain-team/config.json");
            let mut config: Value =
                serde_json::from_slice(&fs::read(&config_path).unwrap()).unwrap();
            match case {
                "tmux" => fs::write(
                    teams.join("drain-team/state/delivery/adapter-architect.json"),
                    r#"{"mode":"tmux","revision":2}"#,
                )
                .unwrap(),
                "non-owner" => config["delivery_owner"] = json!("member"),
                "legacy" => config["messaging_format"] = json!(1),
                "future" => config["messaging_format"] = json!(99),
                "disabled" => {
                    let script = fs::read_to_string(root.join("mesh"))
                        .unwrap()
                        .replace("'enabled':True", "'enabled':False");
                    fs::write(root.join("mesh"), script).unwrap();
                }
                "missing-hook-session" | "missing-context" => {
                    let path = teams.join("drain-team/runtime/architect.json");
                    let mut record: Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    record
                        .as_object_mut()
                        .unwrap()
                        .remove(if case == "missing-context" {
                            "contextGeneration"
                        } else {
                            "hookSessionId"
                        });
                    fs::write(path, record.to_string()).unwrap();
                }
                "wrong-session" => payload["session_id"] = json!("same-cwd-is-not-identity"),
                "wrong-event" => payload["hook_event_name"] = json!("Cancelled"),
                "grok" => payload["transcript_path"] = json!(root.join(".grok/transcript.jsonl")),
                "agy" => payload["transcript_path"] = json!(root.join(".gemini/transcript.jsonl")),
                "stop" => payload["hook_event_name"] = json!("Stop"),
                _ => {
                    let mut runtime =
                        MemberRuntimeStore::load(&teams, "drain-team", "architect").unwrap();
                    if case == "dead" {
                        runtime.health = HealthState::SessionDead;
                    } else {
                        runtime
                            .launch_root
                            .as_mut()
                            .unwrap()
                            .root_authority_revision = 99;
                    }
                    MemberRuntimeStore::save(&teams, "drain-team", "architect", &runtime).unwrap();
                }
            }
            fs::write(config_path, config.to_string()).unwrap();
            let mut out = Vec::new();
            run_compact_hook_cli(payload.to_string().as_bytes(), &mut out, &teams).unwrap();
            assert!(
                !hook_drain_calls(&fake)
                    .iter()
                    .any(|call| call["argv"][7] == "drain"),
                "{case}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_compact_card_survives_faults_without_retries() {
        for mode in [
            "ok",
            "nonzero",
            "timeout",
            "invalid",
            "oversized",
            "receipt-failure",
            "stderr",
        ] {
            let (fake, mut payload, teams) = hook_drain_fixture();
            let root = fake.dir.path();
            let script = fs::read_to_string(root.join("mesh"))
                .unwrap()
                .replace("PostToolUse", "SessionStart")
                .replace("'source':'ordinary'", "'source':'compact'");
            fs::write(root.join("mesh"), script).unwrap();
            fs::write(root.join("mode"), mode).unwrap();
            write_snapshot_fixture(&teams, "drain-team", "architect");
            payload["hook_event_name"] = json!("SessionStart");
            payload["source"] = json!("compact");
            let start = std::time::Instant::now();
            let mut out = Vec::new();
            run_compact_hook_cli(payload.to_string().as_bytes(), &mut out, &teams).unwrap();
            assert!(
                start.elapsed() < std::time::Duration::from_secs(4),
                "{mode}"
            );
            let out: Value = serde_json::from_slice(&out).unwrap();
            let context = out["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .expect(mode);
            assert!(
                context.contains("Inspect architecture"),
                "{mode}: {context}"
            );
            let calls = hook_drain_calls(&fake);
            assert_eq!(
                calls.iter().filter(|c| c["argv"][7] == "drain").count(),
                1,
                "{mode}"
            );
            // Regression: c408b68a sent null offer IDs, which Mesh always refuses.
            let has_id = !matches!(mode, "invalid" | "oversized");
            assert_eq!(
                calls.iter().filter(|c| c["argv"][7] == "receipt").count(),
                usize::from(has_id),
                "{mode}"
            );
            if !has_id {
                assert!(!context.contains("fixture marker"));
                continue;
            }
            let receipt = &calls.last().unwrap()["stdin"];
            let offered = matches!(mode, "ok" | "receipt-failure" | "stderr");
            assert_eq!(
                receipt["stage"],
                if offered {
                    "hook_response_offered"
                } else {
                    "outcome_unknown"
                },
                "{mode}"
            );
            assert_eq!(context.contains("fixture marker"), offered, "{mode}");
            if offered {
                assert!(
                    context.find("Inspect architecture").unwrap()
                        < context
                            .find("## Mesh pending messages (attributed data)")
                            .unwrap()
                );
            }
            assert_eq!(calls[1]["stdin"]["request"]["compose_compaction"], true);
            assert!(
                calls[1]["stdin"]["request"]["reserved_bytes"]
                    .as_u64()
                    .unwrap()
                    > 100
            );
            assert_eq!(receipt["request"], calls[1]["stdin"]["request"]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_partial_write_closes_executor_before_unknown_receipt() {
        struct BrokenOutput(PathBuf);
        impl Write for BrokenOutput {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                unreachable!()
            }
        }
        impl Drop for BrokenOutput {
            fn drop(&mut self) {
                fs::write(&self.0, "closed").unwrap();
            }
        }
        let (fake, payload, teams) = hook_drain_fixture();
        let root = fake.dir.path();
        let script = fs::read_to_string(root.join("mesh")).unwrap().replace("else: print(json.dumps({'protocol':protocol,'status':'recorded'", "else:\n assert (root/'executor-closed').exists()\n print(json.dumps({'protocol':protocol,'status':'recorded'");
        fs::write(root.join("mesh"), script).unwrap();
        assert!(run_compact_hook_cli(
            payload.to_string().as_bytes(),
            BrokenOutput(root.join("executor-closed")),
            &teams
        )
        .is_err());
        let calls = hook_drain_calls(&fake);
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[2]["stdin"]["stage"], "outcome_unknown");
        assert_eq!(calls[2]["stdin"]["offer_id"], "offer-1");
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_foreign_hook_shapes_remain_tolerated() {
        // Regression: 15f222bd made valid foreign hook entries fatal after compaction install.
        let (fake, _, teams) = hook_drain_fixture();
        let settings = fake.dir.path().join("settings.json");
        let foreign = json!({"PreToolUse":[{"matcher":"Bash"}],"PostToolUse":"foreign"});
        fs::write(&settings, json!({"hooks":foreign}).to_string()).unwrap();
        ensure_compact_hook_installed(&teams, &fake.dir.path().join("mesh")).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(settings).unwrap()).unwrap();
        for event in ["PreToolUse", "PostToolUse"] {
            assert_eq!(value["hooks"][event], foreign[event]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_old_root_removal_cleans_ordinary_registrations() {
        // Regression: 15f222bd installed drain entries but root-switch removal only removed compaction.
        let (fake, _, teams) = hook_drain_fixture();
        let root = fake.dir.path();
        let mesh = root.join("mesh");
        fs::write(
            &mesh,
            fs::read_to_string(&mesh)
                .unwrap()
                .replace("codex", "claude"),
        )
        .unwrap();
        let bindings = vec![(teams.clone(), "drain-team".into(), "architect".into())];
        ensure_compact_hook_installed(&teams, &mesh).unwrap();
        drain::reconcile_home(root, CliTool::Claude, &bindings, &mesh).unwrap();
        assert!(remove_compact_hook(&teams).unwrap());
        let settings = fs::read_to_string(root.join("settings.json")).unwrap();
        assert!(!settings.contains("taurhaus-delivery-drain"));
        assert!(!settings.contains("taurhaus-session-start-compact"));
        assert!(!remove_compact_hook(&teams).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_sequence_zero_is_valid_but_missing_sequence_is_not() {
        // Regression: c408b68a conflated a valid sequence zero with missing sequence.
        for (sequence, offered) in [("0", true), ("None", false)] {
            let (fake, payload, teams) = hook_drain_fixture();
            let mesh = fake.dir.path().join("mesh");
            fs::write(
                &mesh,
                fs::read_to_string(&mesh)
                    .unwrap()
                    .replace("'sequence':1", &format!("'sequence':{sequence}")),
            )
            .unwrap();
            let response = handle_compact_hook(&payload.to_string(), &teams).unwrap();
            assert_eq!(response.hook_specific_output.is_some(), offered);
        }
    }

    #[test]
    fn hook_drain_oversized_input_skips_without_hook_failure() {
        // Regression: 734e93ed made ordinary prompt/tool payloads over 64 KiB fail the hook.
        let root = tempfile::tempdir().unwrap();
        let payload = json!({"hook_event_name":"PreToolUse","session_id":"session","tool_input":"界".repeat(65 * 1024)});
        let mut output = Vec::new();
        run_compact_hook_cli(payload.to_string().as_bytes(), &mut output, root.path()).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&output).unwrap(), json!({}));
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_discovery_failure_preserves_installed_home() {
        // Regression: 15f222bd treated unavailable capabilities as disabled pins,
        // deleting daemon-owned registrations during Windows/app reconciliation.
        for failure in ["missing", "nonzero", "invalid", "malformed-descriptor"] {
            let (fake, _, teams) = hook_drain_fixture();
            let home = fake.dir.path().join("selected-codex");
            let mesh = fake.dir.path().join("mesh");
            let exe = fake.dir.path().join("taurhaus");
            fs::write(&exe, "fixture executable").unwrap();
            let bindings = vec![(teams, "drain-team".into(), "architect".into())];
            assert!(drain::reconcile_home(&home, CliTool::Codex, &bindings, &exe).unwrap());
            let settings = home.join("hooks.json");
            let installed = fs::read(&settings).unwrap();
            let scripts = fs::read_dir(home.join("hooks"))
                .unwrap()
                .map(|entry| {
                    let path = entry.unwrap().path();
                    let bytes = fs::read(&path).unwrap();
                    (path, bytes)
                })
                .collect::<Vec<_>>();
            match failure {
                "missing" => fs::remove_file(&mesh).unwrap(),
                "nonzero" => fs::write(&mesh, "#!/bin/sh\nexit 7\n").unwrap(),
                "invalid" => fs::write(&mesh, "#!/bin/sh\nprintf invalid\n").unwrap(),
                _ => {
                    let script = fs::read_to_string(&mesh)
                        .unwrap()
                        .replace("'max_bytes':8192", "'max_bytes':'invalid'");
                    fs::write(&mesh, script).unwrap();
                }
            }
            assert!(
                !drain::reconcile_home(&home, CliTool::Codex, &bindings, &exe).unwrap(),
                "{failure}"
            );
            assert_eq!(fs::read(&settings).unwrap(), installed, "{failure}");
            for (path, bytes) in scripts {
                assert_eq!(fs::read(path).unwrap(), bytes);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_codex_home_reconciliation_preserves_foreign_and_malformed_settings() {
        let (fake, _, teams) = hook_drain_fixture();
        let home = fake.dir.path().join("selected-codex");
        let exe = fake.dir.path().join("mesh");
        fs::create_dir_all(&home).unwrap();
        let bindings = vec![(teams, "drain-team".into(), "architect".into())];
        ensure_codex_compact_hook_installed_at(&home, &exe).unwrap();
        let settings = home.join("hooks.json");
        let compaction = fs::read_to_string(&settings).unwrap();
        assert!(drain::reconcile_home(&home, CliTool::Codex, &bindings, &exe).unwrap());
        assert!(!drain::reconcile_home(&home, CliTool::Codex, &bindings, &exe).unwrap());
        let installed = fs::read_to_string(&settings).unwrap();
        assert!(installed.contains("PostToolUse"));
        assert!(installed.contains("taurhaus-session-start-compact"));
        assert!(drain::reconcile_home(&home, CliTool::Codex, &[], &exe).unwrap());
        assert_eq!(
            serde_json::from_str::<Value>(&fs::read_to_string(&settings).unwrap()).unwrap(),
            serde_json::from_str::<Value>(&compaction).unwrap()
        );
        fs::write(&settings, "{").unwrap();
        assert!(drain::reconcile_home(&home, CliTool::Codex, &bindings, &exe).is_err());
        assert_eq!(fs::read_to_string(settings).unwrap(), "{");
    }
    #[cfg(unix)]
    #[test]
    fn hook_drain_rejects_wrong_contract_and_combined_unicode_overflow() {
        for (from, to) in [
            ("mesh-hook-drain/1", "mesh-hook-drain/99"),
            ("'selection_revision':1", "'selection_revision':9"),
            ("'continue':False", "'continue':True"),
            ("'coverage':'full_body'", "'coverage':'partial'"),
            ("'[mesh message data] fixture marker\\n'", "'界'*4000"),
        ] {
            let (fake, payload, teams) = hook_drain_fixture();
            let script = fs::read_to_string(fake.dir.path().join("mesh")).unwrap();
            assert!(script.contains(from), "{from}");
            fs::write(fake.dir.path().join("mesh"), script.replace(from, to)).unwrap();
            let mut out = Vec::new();
            run_compact_hook_cli(payload.to_string().as_bytes(), &mut out, &teams).unwrap();
            let output: Value = serde_json::from_slice(&out).unwrap();
            assert!(output.get("hookSpecificOutput").is_none(), "{from}");
            let calls = hook_drain_calls(&fake);
            if calls.len() > 1 {
                assert_eq!(calls.last().unwrap()["stdin"]["stage"], "outcome_unknown");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_no_card_and_duplicate_card_remain_independent() {
        let (fake, mut payload, teams) = hook_drain_fixture();
        let script = fs::read_to_string(fake.dir.path().join("mesh"))
            .unwrap()
            .replace("PostToolUse", "SessionStart")
            .replace("'source':'ordinary'", "'source':'compact'");
        fs::write(fake.dir.path().join("mesh"), script).unwrap();
        payload["hook_event_name"] = json!("SessionStart");
        payload["source"] = json!("compact");
        for _ in 0..2 {
            let mut out = Vec::new();
            run_compact_hook_cli(payload.to_string().as_bytes(), &mut out, &teams).unwrap();
            let output: Value = serde_json::from_slice(&out).unwrap();
            assert!(output["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
                .contains("fixture marker"));
        }
        // Suppression belongs to Mesh; absence of a card cannot suppress a drain.
        assert_eq!(
            hook_drain_calls(&fake)
                .iter()
                .filter(|c| c["argv"][7] == "drain")
                .count(),
            2
        );
    }
    #[cfg(unix)]
    #[test]
    fn hook_drain_claude_codex_event_envelopes_and_ordinary_context_are_pinned() {
        for tool in [CliTool::Claude, CliTool::Codex] {
            for event in [
                "SessionStart",
                "UserPromptSubmit",
                "PreToolUse",
                "PostToolUse",
                "PermissionRequest",
            ] {
                let (fake, mut payload, teams) = hook_drain_fixture();
                let root = fake.dir.path();
                let script = fs::read_to_string(root.join("mesh"))
                    .unwrap()
                    .replace("codex", &tool.to_string())
                    .replace("PostToolUse", event);
                fs::write(root.join("mesh"), script).unwrap();
                let config_path = teams.join("drain-team/config.json");
                let config = fs::read_to_string(&config_path)
                    .unwrap()
                    .replace("codex", &tool.to_string());
                fs::write(config_path, config).unwrap();
                let mut record =
                    MemberRuntimeStore::load(&teams, "drain-team", "architect").unwrap();
                record.cli_tool = Some(tool);
                record.harness = Some(tool);
                MemberRuntimeStore::save(&teams, "drain-team", "architect", &record).unwrap();
                payload["transcript_path"] =
                    json!(root.join(format!(".{tool}/projects/transcript.jsonl")));
                payload["hook_event_name"] = json!(event);
                let mut out = Vec::new();
                run_compact_hook_cli(payload.to_string().as_bytes(), &mut out, &teams).unwrap();
                let out: Value = serde_json::from_slice(&out).unwrap();
                assert_eq!(out["hookSpecificOutput"]["hookEventName"], event, "{tool}");
                assert!(out["hookSpecificOutput"]["additionalContext"]
                    .as_str()
                    .unwrap()
                    .contains("fixture marker"));
                assert_eq!(
                    MemberRuntimeStore::load(&teams, "drain-team", "architect")
                        .unwrap()
                        .context_generation,
                    3
                );
                assert!(
                    MemberCompactionStore::load(&teams, "drain-team", "architect")
                        .unwrap()
                        .is_none()
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn hook_drain_preserves_foreign_mentions_of_managed_scripts() {
        // Regression: 15f222bd identified owned hooks by a substring in any command.
        let (fake, _, _) = hook_drain_fixture();
        let root = fake.dir.path();
        let path = root.join("hooks.json");
        let foreign = json!({"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":format!("echo taurhaus-delivery-drain-{}.sh", "a".repeat(64))}]}]}});
        fs::write(&path, foreign.to_string()).unwrap();
        drain::reconcile_home(root, CliTool::Codex, &[], &root.join("mesh")).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
            foreign
        );
    }
}
