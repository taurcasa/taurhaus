//! Claude Code hook bridge for post-compaction operational reinjection.

use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::coordination::domain::Member;
use crate::coordination::errors::CoordinationError;
use crate::coordination::reinjection::CompactionReinjectionService;
use crate::coordination::stores::{
    record_delivery, CompactionDeliveryResult, MemberRuntimeStore, OperationalContextSnapshotStore,
    TeamConfigStore,
};
use crate::session_scanner::cli_tool::CliTool;

const TAURHAUS_COMPACT_HOOK_BASENAME: &str = "taurhaus-session-start-compact";
const CLAUDE_SETTINGS_FILENAME: &str = "settings.json";
const SESSION_START_HOOK_EVENT: &str = "SessionStart";
const COMPACT_SOURCE: &str = "compact";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeSessionStartHookInput {
    hook_event_name: String,
    session_id: String,
    source: String,
    #[serde(default)]
    cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ClaudeHookResponse {
    #[serde(rename = "hookSpecificOutput", skip_serializing_if = "Option::is_none")]
    hook_specific_output: Option<ClaudeSessionStartHookSpecificOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaudeSessionStartHookSpecificOutput {
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

pub fn handle_session_start_hook_stdin<R: Read>(
    mut stdin: R,
    teams_dir: &Path,
) -> Result<ClaudeHookResponse, CoordinationError> {
    let mut raw = String::new();
    stdin.read_to_string(&mut raw)?;
    handle_session_start_hook(&raw, teams_dir)
}

pub fn handle_session_start_hook(
    raw: &str,
    teams_dir: &Path,
) -> Result<ClaudeHookResponse, CoordinationError> {
    let payload: ClaudeSessionStartHookInput = serde_json::from_str(raw).map_err(|err| {
        CoordinationError::Validation(format!("invalid Claude SessionStart hook payload: {err}"))
    })?;

    if payload.hook_event_name != SESSION_START_HOOK_EVENT || payload.source != COMPACT_SOURCE {
        return Ok(ClaudeHookResponse::default());
    }

    let Some(matched) = resolve_member_match(teams_dir, &payload)? else {
        return Ok(ClaudeHookResponse::default());
    };

    let Some(snapshot) =
        OperationalContextSnapshotStore::load(teams_dir, &matched.team_name, &matched.member.name)?
    else {
        let _ = record_delivery(
            &matched.team_name,
            &matched.member.name,
            CliTool::Claude,
            &payload.session_id,
            Utc::now(),
            CompactionDeliveryResult::Skipped,
        );
        return Ok(ClaudeHookResponse::default());
    };

    let card = CompactionReinjectionService::compose(&matched.member, &snapshot);
    let additional_context = CompactionReinjectionService::render_claude_additional_context(&card)
        .map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to render Claude additional context for '{}': {err}",
                matched.member.name
            ))
        })?;

    if additional_context.trim().is_empty() {
        let _ = record_delivery(
            &matched.team_name,
            &matched.member.name,
            CliTool::Claude,
            &payload.session_id,
            Utc::now(),
            CompactionDeliveryResult::Skipped,
        );
        return Ok(ClaudeHookResponse::default());
    }

    record_delivery(
        &matched.team_name,
        &matched.member.name,
        CliTool::Claude,
        &payload.session_id,
        Utc::now(),
        CompactionDeliveryResult::Injected,
    )?;

    Ok(ClaudeHookResponse {
        hook_specific_output: Some(ClaudeSessionStartHookSpecificOutput {
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

    let hooks_dir = claude_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    let script_path = hooks_dir.join(platform_hook_filename());
    write_hook_script(&script_path, taurhaus_exe)?;
    ensure_settings_hook_entry(&claude_dir.join(CLAUDE_SETTINGS_FILENAME), &script_path)
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

fn resolve_member_match(
    teams_dir: &Path,
    payload: &ClaudeSessionStartHookInput,
) -> Result<Option<HookMemberMatch>, CoordinationError> {
    let mut runtime_matches = Vec::new();

    for team_name in TeamConfigStore::list(teams_dir)? {
        let config = match TeamConfigStore::load(teams_dir, &team_name) {
            Ok(config) => config,
            Err(err) => {
                tracing::warn!(
                    team_name = team_name,
                    error = %err,
                    "failed to load team config during Claude compaction hook resolution"
                );
                continue;
            }
        };

        for member in config.members {
            if member.cli_tool != CliTool::Claude {
                continue;
            }
            let runtime = match MemberRuntimeStore::load(teams_dir, &team_name, &member.name) {
                Ok(runtime) => runtime,
                Err(_) => continue,
            };
            if runtime.session_id.as_deref() != Some(payload.session_id.as_str()) {
                continue;
            }
            if !cwd_matches_member(payload.cwd.as_deref(), &member.project_path) {
                continue;
            }
            runtime_matches.push(HookMemberMatch {
                team_name: team_name.clone(),
                member,
            });
        }
    }

    if runtime_matches.len() == 1 {
        return Ok(runtime_matches.into_iter().next());
    }
    if runtime_matches.len() > 1 {
        tracing::warn!(
            session_id = %payload.session_id,
            "multiple Claude members matched hook payload by runtime session; skipping reinjection"
        );
        return Ok(None);
    }
    Ok(None)
}

fn cwd_matches_member(cwd: Option<&Path>, member_project_path: &Path) -> bool {
    let Some(cwd) = cwd else {
        return true;
    };

    normalize_path(cwd) == normalize_path(member_project_path)
}

fn normalize_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn write_hook_script(script_path: &Path, taurhaus_exe: &Path) -> Result<(), CoordinationError> {
    let script_body = render_hook_script(taurhaus_exe);
    fs::write(script_path, script_body.as_bytes())?;
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(script_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(script_path, perms)?;
    }
    Ok(())
}

fn render_hook_script(taurhaus_exe: &Path) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "@echo off\r\n\"{}\" --claude-compact-hook\r\n",
            taurhaus_exe.display()
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nexec {} --claude-compact-hook\n",
            shell_quote_path(taurhaus_exe)
        )
    }
}

fn ensure_settings_hook_entry(
    settings_path: &Path,
    script_path: &Path,
) -> Result<bool, CoordinationError> {
    let mut settings = load_settings_json(settings_path)?;
    let original_settings = settings.clone();
    let command = settings_command_for_script(script_path);

    let root = settings.as_object_mut().ok_or_else(|| {
        CoordinationError::StoreError(format!(
            "Claude settings at '{}' are not a JSON object",
            settings_path.display()
        ))
    })?;

    let hooks = root
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    let hooks_obj = hooks.as_object_mut().ok_or_else(|| {
        CoordinationError::StoreError(format!(
            "Claude settings 'hooks' in '{}' are not a JSON object",
            settings_path.display()
        ))
    })?;

    let session_start = hooks_obj
        .entry(SESSION_START_HOOK_EVENT.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let entries = session_start.as_array_mut().ok_or_else(|| {
        CoordinationError::StoreError(format!(
            "Claude settings 'hooks.{SESSION_START_HOOK_EVENT}' in '{}' are not an array",
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
                "Claude settings compact SessionStart hooks in '{}' are not an array",
                settings_path.display()
            ))
        })?;
        hook_array.push(command_hook_value(&command));
        inserted = true;
        break;
    }

    if !inserted {
        entries.push(json!({
            "matcher": COMPACT_SOURCE,
            "hooks": [command_hook_value(&command)],
        }));
    }

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let changed = settings != original_settings;
    if changed {
        let payload = serde_json::to_vec_pretty(&settings).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize Claude settings '{}': {err}",
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
            "Claude settings path '{}' has no parent directory",
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

fn command_hook_value(command: &str) -> Value {
    json!({
        "type": "command",
        "command": command,
    })
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
            "failed to parse Claude settings '{}': {err}",
            settings_path.display()
        ))
    })
}

#[cfg(target_os = "windows")]
fn platform_hook_filename() -> String {
    format!("{TAURHAUS_COMPACT_HOOK_BASENAME}.cmd")
}

#[cfg(not(target_os = "windows"))]
fn platform_hook_filename() -> String {
    format!("{TAURHAUS_COMPACT_HOOK_BASENAME}.sh")
}

#[cfg(target_os = "windows")]
fn settings_command_for_script(script_path: &Path) -> String {
    format!("\"{}\"", script_path.display())
}

#[cfg(not(target_os = "windows"))]
fn settings_command_for_script(script_path: &Path) -> String {
    shell_quote_path(script_path)
}

#[cfg(not(target_os = "windows"))]
fn shell_quote_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\'', "'\"'\"'");
    format!("'{value}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::DateTime;
    use fs2::FileExt;
    use std::ffi::OsString;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    use crate::coordination::domain::{HealthState, MemberRole};
    use crate::coordination::stores::{
        MemberCompactionStore, MemberRuntimeRecord, OperationalAssignmentFooterSnapshot,
        OperationalContextSnapshot, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
        OperationalWorkingSetSnapshot, TeamConfig,
    };

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
            instructions: Some("Inspect architecture".to_string()),
            behavioral_contract: None,
            capabilities: None,
            project_path: project_path.to_path_buf(),
            cli_tool: CliTool::Claude,
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
            },
        )
        .expect("save team");

        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            &member.name,
            &MemberRuntimeRecord {
                schema_version: 1,
                member_name: member.name.clone(),
                pane_id: Some("%217".to_string()),
                session_id: Some(session_id.to_string()),
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
                        "src-tauri/src/coordination/claude_hooks.rs".to_string()
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
                    project_path: "/home/mstie/projects/taurhaus".to_string(),
                    focal_files: vec!["src-tauri/src/coordination/claude_hooks.rs".to_string()],
                },
            },
        )
        .expect("save snapshot");
    }

    #[test]
    fn compact_hook_returns_additional_context_for_matching_member() {
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
                "cwd": project,
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
            .contains("\"reason\": \"post_compaction\""));
        assert!(output
            .additional_context
            .contains("\"member_name\": \"architect\""));
    }

    #[test]
    fn compact_hook_skips_forged_session_id_even_when_cwd_matches_managed_project() {
        // Regression: commit 34e7b9d allowed cwd-only fallback, so a forged compact hook with a
        // managed project path could receive reinjection context without owning the live session.
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
                "cwd": project,
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, ClaudeHookResponse::default());
        assert!(
            MemberCompactionStore::load(&claude_dir.join("teams"), team_name, &member.name,)
                .expect("load compaction state")
                .is_none(),
            "forged payload must not record delivery state"
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
                "cwd": project,
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        let output = response
            .hook_specific_output
            .expect("hook should inject additional context");
        let parsed: Value =
            serde_json::from_str(&output.additional_context).expect("additional context json");

        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["reason"], "post_compaction");
        assert_eq!(parsed["team_name"], "taurhaus-team");
        assert_eq!(parsed["member_name"], "architect");
        assert_eq!(parsed["role"]["role_id"], "taurhaus-architect");
        assert_eq!(parsed["role"]["role_name"], "Taurhaus Architect");
        assert_eq!(parsed["role"]["focus_area"], "Cross-layer diagnosis");
        assert_eq!(parsed["task"]["id"], "680");
        assert_eq!(
            parsed["task"]["subject"],
            "Implement Claude SessionStart(source=compact) hook bridge"
        );
        assert_eq!(parsed["task"]["execution_mode"], "implement");
        assert_eq!(
            parsed["task"]["validation_expectation"],
            "cargo check --tests"
        );
        assert_eq!(
            parsed["boundaries"]["file_ownership_boundary"][0],
            "src-tauri/src/coordination/claude_hooks.rs"
        );
        assert_eq!(
            parsed["working_set"]["project_path"],
            "/home/mstie/projects/taurhaus"
        );
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

        assert_eq!(response, ClaudeHookResponse::default());
    }

    #[test]
    fn compact_hook_skips_when_snapshot_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).expect("project dir");
        let member = sample_member(&project);
        write_team_fixture(tmp.path(), "taurhaus-team", &member, "sess-123");

        let response = handle_session_start_hook(
            &json!({
                "hookEventName": "SessionStart",
                "sessionId": "sess-123",
                "source": "compact",
                "cwd": project,
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, ClaudeHookResponse::default());
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
            },
        )
        .expect("save team");

        for member in [&architect, &reviewer] {
            MemberRuntimeStore::save(
                tmp.path(),
                "taurhaus-team",
                &member.name,
                &MemberRuntimeRecord {
                    schema_version: 1,
                    member_name: member.name.clone(),
                    pane_id: Some("%217".to_string()),
                    session_id: Some("sess-123".to_string()),
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
                "cwd": project,
            })
            .to_string(),
            tmp.path(),
        )
        .expect("hook should succeed");

        assert_eq!(response, ClaudeHookResponse::default());
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

        let script_path = tmp.path().join("hooks").join(platform_hook_filename());
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
        assert!(hooks[0]["hooks"][0]["command"]
            .as_str()
            .expect("command str")
            .contains(TAURHAUS_COMPACT_HOOK_BASENAME));
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
        assert!(hooks[1]["command"]
            .as_str()
            .expect("command")
            .contains(TAURHAUS_COMPACT_HOOK_BASENAME));
    }

    #[test]
    fn ensure_settings_hook_entry_preserves_valid_json_after_atomic_update() {
        // Regression: before task #712, ensure_settings_hook_entry rewrote settings.json via
        // direct fs::write, which risked truncation or clobbering unrelated content mid-update.
        let tmp = tempfile::tempdir().expect("tempdir");
        let settings_path = tmp.path().join("settings.json");
        let script_path = tmp.path().join("hooks").join(platform_hook_filename());
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
            ensure_settings_hook_entry(&settings_path, &script_path).expect("update settings");
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
        assert!(updated["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .expect("command")
            .contains(TAURHAUS_COMPACT_HOOK_BASENAME));
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
}
