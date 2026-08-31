//! Codex CLI task parser.
//!
//! Codex tracks tasks via `update_plan` function calls in session JSONL files.
//! The plan contains a list of steps, each with a description and status.
//!
//! **Live session**: Use `jsonl_path` from running sessions.
//! **Offline fallback**: Reuse CodexResolver logic — scan `~/.codex/sessions/YYYY/MM/DD/`
//! with 7-day lookback, match by `cwd` in first JSONL line.

use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::RuntimeSession;
use crate::task_scanner::types::{ScanOutcome, TaskStatus, UnifiedTask};
use chrono::{DateTime, Utc};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::claude::TranscriptParser;

pub struct CodexTranscriptParser;

impl TranscriptParser for CodexTranscriptParser {
    fn tool(&self) -> CliTool {
        CliTool::Codex
    }

    fn get_tasks(
        &self,
        project_path: &str,
        sessions: &[&RuntimeSession],
        _claude_index: Option<&crate::task_scanner::claude_index::ClaudeSourceIndex>,
    ) -> ScanOutcome {
        get_tasks(project_path, sessions)
    }

    #[cfg(feature = "mesh-bridged-backend")]
    fn parse_compaction_boundary(
        &self,
        line: &str,
        jsonl_offset: u64,
    ) -> Option<crate::session_scanner::transcript_boundary::ParsedSignalBoundary> {
        use crate::session_scanner::transcript_boundary::CompactionSignalKind;
        use serde_json::Value;

        let parsed: Value = serde_json::from_str(line).ok()?;
        let timestamp = parsed
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))?;
        let signal_kind = match parsed.get("type").and_then(Value::as_str) {
            Some("compacted") => CompactionSignalKind::Compacted,
            Some("event_msg")
                if parsed
                    .get("payload")
                    .and_then(|payload| payload.get("type"))
                    .and_then(Value::as_str)
                    == Some("context_compacted") =>
            {
                CompactionSignalKind::ContextCompacted
            }
            _ => return None,
        };

        Some(
            crate::session_scanner::transcript_boundary::ParsedSignalBoundary {
                timestamp,
                jsonl_offset,
                signal_kind,
            },
        )
    }
}

/// How many bytes from the end of file to read when searching for update_plan.
/// 256KB is generous — plans are small, and we want the last one.
const TAIL_READ_SIZE: u64 = 256 * 1024;

/// How many days back to scan for offline sessions (matches idle.rs).
const CODEX_LOOKBACK_DAYS: i64 = 7;
/// How far back archived session enrichment scans for transcript timestamps.
const CODEX_TIMELINE_LOOKBACK_DAYS: i64 = 30;

#[derive(Debug, Default)]
struct CodexDiagnostics {
    had_errors: bool,
    first_error: Option<String>,
}

impl CodexDiagnostics {
    fn record_error(&mut self, message: String) {
        self.had_errors = true;
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }

    fn unavailable_or_empty(self) -> ScanOutcome {
        if self.had_errors {
            ScanOutcome::Unavailable(
                self.first_error
                    .unwrap_or_else(|| "Codex scan encountered degraded I/O".to_string()),
            )
        } else {
            ScanOutcome::DefinitivelyEmpty
        }
    }
}

#[derive(Debug, Default)]
struct ParseUpdatePlanOutcome {
    tasks: Vec<UnifiedTask>,
    had_errors: bool,
    first_error: Option<String>,
}

impl ParseUpdatePlanOutcome {
    fn record_error(&mut self, message: String) {
        self.had_errors = true;
        if self.first_error.is_none() {
            self.first_error = Some(message);
        }
    }
}

#[derive(Debug, Default)]
struct SessionDiscoveryOutcome {
    session_path: Option<PathBuf>,
    diagnostics: CodexDiagnostics,
}

/// Get tasks from Codex session JSONL files.
pub fn get_tasks(project_path: &str, sessions: &[&RuntimeSession]) -> ScanOutcome {
    let mut diagnostics = CodexDiagnostics::default();

    // Try live sessions first — use jsonl_path directly
    for session in sessions {
        if let Some(ref jsonl_path) = session.jsonl_path {
            let path = Path::new(jsonl_path);
            if path.exists() {
                match parse_update_plan_with_diagnostics(path) {
                    Ok(outcome) if !outcome.tasks.is_empty() => {
                        return ScanOutcome::Data(outcome.tasks)
                    }
                    Ok(outcome) => {
                        if outcome.had_errors {
                            diagnostics.record_error(outcome.first_error.unwrap_or_else(|| {
                                format!(
                                    "Failed to fully parse Codex update_plan in {}",
                                    path.display()
                                )
                            }));
                        }
                    }
                    Err(e) => return ScanOutcome::Unavailable(e),
                }
            }
        }
    }

    // Offline fallback
    let offline = get_tasks_offline(project_path);
    match offline {
        ScanOutcome::Data(tasks) => ScanOutcome::Data(tasks),
        ScanOutcome::Unavailable(reason) => ScanOutcome::Unavailable(reason),
        ScanOutcome::DefinitivelyEmpty => diagnostics.unavailable_or_empty(),
    }
}

/// Testable version with injectable sessions directory.
pub fn get_tasks_in(
    project_path: &str,
    sessions: &[&RuntimeSession],
    sessions_dir: &Path,
) -> ScanOutcome {
    let mut diagnostics = CodexDiagnostics::default();

    // Try live sessions first
    for session in sessions {
        if let Some(ref jsonl_path) = session.jsonl_path {
            let path = Path::new(jsonl_path);
            if path.exists() {
                match parse_update_plan_with_diagnostics(path) {
                    Ok(outcome) if !outcome.tasks.is_empty() => {
                        return ScanOutcome::Data(outcome.tasks)
                    }
                    Ok(outcome) => {
                        if outcome.had_errors {
                            diagnostics.record_error(outcome.first_error.unwrap_or_else(|| {
                                format!(
                                    "Failed to fully parse Codex update_plan in {}",
                                    path.display()
                                )
                            }));
                        }
                    }
                    Err(e) => return ScanOutcome::Unavailable(e),
                }
            }
        }
    }

    // Offline fallback with custom dir
    let offline = get_tasks_offline_in(project_path, sessions_dir);
    match offline {
        ScanOutcome::Data(tasks) => ScanOutcome::Data(tasks),
        ScanOutcome::Unavailable(reason) => ScanOutcome::Unavailable(reason),
        ScanOutcome::DefinitivelyEmpty => diagnostics.unavailable_or_empty(),
    }
}

/// Offline fallback: scan recent Codex sessions to find one matching this project.
fn get_tasks_offline(project_path: &str) -> ScanOutcome {
    let sessions_dir = match dirs::home_dir() {
        Some(h) => h.join(".codex").join("sessions"),
        None => return ScanOutcome::Unavailable("Could not resolve home directory".to_string()),
    };
    get_tasks_offline_in(project_path, &sessions_dir)
}

/// Offline fallback with injectable directory.
fn get_tasks_offline_in(project_path: &str, sessions_dir: &Path) -> ScanOutcome {
    if !sessions_dir.exists() {
        return ScanOutcome::DefinitivelyEmpty;
    }
    if !sessions_dir.is_dir() {
        return ScanOutcome::Unavailable(format!(
            "Codex sessions root is not a directory: {}",
            sessions_dir.display()
        ));
    }

    // Reuse the same date-scanning logic as CodexResolver in idle.rs
    let discovery = find_codex_session_for_project_with_diagnostics(project_path, sessions_dir);
    match discovery.session_path {
        Some(path) => match parse_update_plan_with_diagnostics(&path) {
            Ok(outcome) if !outcome.tasks.is_empty() => ScanOutcome::Data(outcome.tasks),
            Ok(outcome) => {
                let mut diagnostics = discovery.diagnostics;
                if outcome.had_errors {
                    diagnostics.record_error(outcome.first_error.unwrap_or_else(|| {
                        format!(
                            "Failed to fully parse Codex update_plan in {}",
                            path.display()
                        )
                    }));
                }
                diagnostics.unavailable_or_empty()
            }
            Err(e) => ScanOutcome::Unavailable(e),
        },
        None => discovery.diagnostics.unavailable_or_empty(),
    }
}

/// Scan recent date directories to find a Codex session file matching a project.
fn find_codex_session_for_project_with_diagnostics(
    project_path: &str,
    sessions_dir: &Path,
) -> SessionDiscoveryOutcome {
    use chrono::Local;

    let mut outcome = SessionDiscoveryOutcome::default();
    let today = Local::now().date_naive();

    for days_back in 0..CODEX_LOOKBACK_DAYS {
        let date = today - chrono::Duration::days(days_back);
        let date_dir = sessions_dir
            .join(date.format("%Y").to_string())
            .join(date.format("%m").to_string())
            .join(date.format("%d").to_string());

        if !date_dir.is_dir() {
            continue;
        }

        let dir_entries = match fs::read_dir(&date_dir) {
            Ok(entries) => entries,
            Err(e) => {
                outcome.diagnostics.record_error(format!(
                    "Failed to read Codex session dir {}: {e}",
                    date_dir.display()
                ));
                continue;
            }
        };

        let mut entries = Vec::new();
        for entry_result in dir_entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(e) => {
                    outcome
                        .diagnostics
                        .record_error(format!("Failed to read Codex session entry: {e}"));
                    continue;
                }
            };
            if entry.path().extension().is_some_and(|ext| ext == "jsonl") {
                entries.push(entry);
            }
        }

        // Sort by mtime descending
        entries.sort_by(|a, b| {
            let mt_a = match a.metadata().and_then(|m| m.modified()) {
                Ok(ts) => ts,
                Err(e) => {
                    outcome.diagnostics.record_error(format!(
                        "Failed to read mtime for {}: {e}",
                        a.path().display()
                    ));
                    std::time::SystemTime::UNIX_EPOCH
                }
            };
            let mt_b = match b.metadata().and_then(|m| m.modified()) {
                Ok(ts) => ts,
                Err(e) => {
                    outcome.diagnostics.record_error(format!(
                        "Failed to read mtime for {}: {e}",
                        b.path().display()
                    ));
                    std::time::SystemTime::UNIX_EPOCH
                }
            };
            mt_b.cmp(&mt_a)
        });

        for entry in entries {
            match codex_session_matches_project(&entry.path(), project_path) {
                Ok(true) => {
                    outcome.session_path = Some(entry.path());
                    return outcome;
                }
                Ok(false) => {}
                Err(e) => outcome.diagnostics.record_error(e),
            }
        }
    }

    outcome
}

/// Check if a Codex JSONL file's first line session_meta.payload.cwd matches a project path.
fn codex_session_matches_project(jsonl_path: &Path, project_path: &str) -> Result<bool, String> {
    use std::io::{BufRead, BufReader};

    let file = fs::File::open(jsonl_path).map_err(|e| {
        format!(
            "Failed to open Codex session file {}: {e}",
            jsonl_path.display()
        )
    })?;

    let mut reader = BufReader::new(file);
    let mut first_line = String::new();
    if reader.read_line(&mut first_line).is_err() {
        return Err(format!(
            "Failed to read first line from Codex session file {}",
            jsonl_path.display()
        ));
    }
    if first_line.is_empty() {
        return Ok(false);
    }

    let parsed: serde_json::Value = serde_json::from_str(&first_line).map_err(|e| {
        format!(
            "Failed to parse session metadata in {}: {e}",
            jsonl_path.display()
        )
    })?;

    if parsed.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
        return Ok(false);
    }

    let cwd = match parsed
        .get("payload")
        .and_then(|p| p.get("cwd"))
        .and_then(|c| c.as_str())
    {
        Some(cwd) => cwd,
        None => return Ok(false),
    };

    let norm_cwd = crate::provider::path::normalize_project_path(cwd);
    let norm_target = crate::provider::path::normalize_project_path(project_path);
    Ok(norm_cwd == norm_target)
}

/// Parse the last `update_plan` function call from a Codex JSONL file.
///
/// Reads the tail of the file for efficiency (large sessions can be megabytes).
/// Finds the last line with `type: "function_call"` and `name: "update_plan"`,
/// then double-parses: `payload.arguments` is a JSON string containing `{plan: [{step, status}]}`.
pub fn parse_update_plan(jsonl_path: &Path) -> Result<Vec<UnifiedTask>, String> {
    Ok(parse_update_plan_with_diagnostics(jsonl_path)?.tasks)
}

fn parse_update_plan_with_diagnostics(jsonl_path: &Path) -> Result<ParseUpdatePlanOutcome, String> {
    let tail = read_file_tail(jsonl_path, TAIL_READ_SIZE)
        .map_err(|e| format!("Failed to read Codex JSONL: {e}"))?;
    let source_key = codex_source_key_from_jsonl(jsonl_path);
    let mut outcome = ParseUpdatePlanOutcome::default();

    // Find the last update_plan line
    let mut last_plan_line: Option<&str> = None;
    for line in tail.lines() {
        if line.is_empty() {
            continue;
        }
        // Quick pre-filter before parsing JSON
        if !line.contains("update_plan") {
            continue;
        }
        match is_update_plan_line(line) {
            Ok(true) => last_plan_line = Some(line),
            Ok(false) => {}
            Err(e) => outcome.record_error(e),
        }
    }

    let plan_line = match last_plan_line {
        Some(line) => line,
        None => return Ok(outcome),
    };

    match parse_plan_from_line(plan_line, &source_key) {
        Ok(tasks) => {
            outcome.tasks = tasks;
            Ok(outcome)
        }
        Err(e) => {
            outcome.record_error(e);
            Ok(outcome)
        }
    }
}

/// Check if a JSONL line is an update_plan function call.
fn is_update_plan_line(line: &str) -> Result<bool, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("Malformed Codex JSONL line: {e}"))?;

    let Some(payload) = parsed.get("payload") else {
        return Ok(false);
    };

    Ok(
        payload.get("type").and_then(|v| v.as_str()) == Some("function_call")
            && payload.get("name").and_then(|v| v.as_str()) == Some("update_plan"),
    )
}

/// Extract tasks from a single update_plan JSONL line.
///
/// Structure: `{"payload":{"type":"function_call","name":"update_plan","arguments":"{\"plan\":[{\"step\":\"...\",\"status\":\"...\"}]}"}}`
/// Note: `arguments` is a JSON-encoded string, so we need to double-parse.
fn parse_plan_from_line(line: &str, source_key: &str) -> Result<Vec<UnifiedTask>, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("Parse error: {e}"))?;

    let arguments_str = parsed
        .get("payload")
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.as_str())
        .ok_or("Missing payload.arguments")?;

    // Double-parse: arguments is a JSON string
    let arguments: serde_json::Value =
        serde_json::from_str(arguments_str).map_err(|e| format!("Arguments parse error: {e}"))?;

    let plan = match arguments.get("plan").and_then(|p| p.as_array()) {
        Some(arr) => arr,
        None => return Ok(vec![]),
    };

    let tasks: Vec<UnifiedTask> = plan
        .iter()
        .enumerate()
        .filter_map(|(idx, step)| {
            let description = step.get("step").and_then(|s| s.as_str())?;
            let status_str = step
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("open");

            let status = match status_str {
                "completed" | "done" => TaskStatus::Completed,
                "in-progress" | "in_progress" | "started" => TaskStatus::InProgress,
                _ => TaskStatus::Pending, // "open", "not-started", etc.
            };

            Some(UnifiedTask {
                id: format!("codex-{idx}"),
                source_key: source_key.to_string(),
                subject: description.to_string(),
                description: None,
                active_form: None,
                status,
                source: CliTool::Codex,
                blocks: vec![],
                blocked_by: vec![],
                owner: None,
                session_id: Some(source_key.to_string()),
                state_changed_at: None,
                updated_at: None,
                archived_at: None,
                last_status: None,
                archived_reason: None,
                effort: None,
                effort_why: None,
                deadline_minutes: None,
            })
        })
        .collect();

    Ok(tasks)
}

/// Derive a stable source key for Codex tasks.
///
/// Primary source: session id in early metadata lines (`payload.id`, `sessionId`,
/// or `payload.sessionId`).
/// Fallback: JSONL filename stem.
fn codex_source_key_from_jsonl(path: &Path) -> String {
    let fallback = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("legacy-codex")
        .to_string();

    codex_session_meta(path)
        .and_then(|meta| meta.session_id())
        .unwrap_or(fallback)
}

/// Resolve Codex transcript time range for a given project/session identity.
pub fn session_time_range(
    project_path: &Path,
    session_id: &str,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let sessions_dir = dirs::home_dir()?.join(".codex").join("sessions");
    session_time_range_in(project_path, session_id, &sessions_dir)
}

fn session_time_range_in(
    project_path: &Path,
    session_id: &str,
    sessions_dir: &Path,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let jsonl_path = find_codex_session_by_id(project_path, session_id, sessions_dir)?;
    codex_time_range_from_file(&jsonl_path)
}

fn find_codex_session_by_id(
    project_path: &Path,
    session_id: &str,
    sessions_dir: &Path,
) -> Option<PathBuf> {
    use chrono::Local;

    let today = Local::now().date_naive();
    let normalized_project =
        crate::provider::path::normalize_project_path(&project_path.to_string_lossy());

    for days_back in 0..CODEX_TIMELINE_LOOKBACK_DAYS {
        let date = today - chrono::Duration::days(days_back);
        let date_dir = sessions_dir
            .join(date.format("%Y").to_string())
            .join(date.format("%m").to_string())
            .join(date.format("%d").to_string());

        if !date_dir.is_dir() {
            continue;
        }

        let mut entries: Vec<_> = fs::read_dir(&date_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            .collect();

        entries.sort_by(|a, b| {
            let mt_a = a
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let mt_b = b
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            mt_b.cmp(&mt_a)
        });

        for entry in entries {
            let path = entry.path();
            let Some(meta) = codex_session_meta(&path) else {
                continue;
            };

            let project_matches = meta
                .cwd
                .as_deref()
                .map(|cwd| crate::provider::path::normalize_project_path(cwd) == normalized_project)
                .unwrap_or(false);
            if !project_matches {
                continue;
            }

            let stem_matches = path.file_stem().and_then(|s| s.to_str()) == Some(session_id);
            let id_matches = meta
                .session_id()
                .map(|id| id == session_id)
                .unwrap_or(false);
            if stem_matches || id_matches {
                return Some(path);
            }
        }
    }

    None
}

fn codex_time_range_from_file(jsonl_path: &Path) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let file = fs::File::open(jsonl_path).ok()?;
    let reader = BufReader::new(file);

    let mut start: Option<DateTime<Utc>> = None;
    let mut end: Option<DateTime<Utc>> = None;

    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(ts_str) = value.get("timestamp").and_then(|v| v.as_str()) else {
            continue;
        };
        let Ok(ts) = ts_str.parse::<DateTime<Utc>>() else {
            continue;
        };
        if start.is_none() {
            start = Some(ts);
        }
        end = Some(ts);
    }

    let start = start?;
    let mut end = end.unwrap_or(start);
    if end < start {
        end = start;
    }
    Some((start, end))
}

#[derive(Debug, Clone, Default)]
struct CodexSessionMeta {
    cwd: Option<String>,
    payload_id: Option<String>,
    session_id: Option<String>,
}

impl CodexSessionMeta {
    fn session_id(self) -> Option<String> {
        self.payload_id
            .or(self.session_id)
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
    }
}

fn codex_session_meta(path: &Path) -> Option<CodexSessionMeta> {
    let file = fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    let mut meta = CodexSessionMeta::default();

    for line in reader.lines().map_while(Result::ok).take(20) {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if meta.cwd.is_none() {
            meta.cwd = parsed
                .get("payload")
                .and_then(|p| p.get("cwd"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
        }
        if meta.payload_id.is_none() {
            meta.payload_id = parsed
                .get("payload")
                .and_then(|p| p.get("id"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
        }
        if meta.session_id.is_none() {
            meta.session_id = parsed
                .get("sessionId")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    parsed
                        .get("payload")
                        .and_then(|p| p.get("sessionId"))
                        .and_then(|v| v.as_str())
                })
                .map(ToString::to_string);
        }
        if meta.cwd.is_some() && (meta.payload_id.is_some() || meta.session_id.is_some()) {
            return Some(meta);
        }
    }

    Some(meta)
}

/// Read the last N bytes of a file as a UTF-8 string.
///
/// If the file is smaller than `max_bytes`, reads the entire file.
/// Trims the first partial line (from the seek position) to avoid broken JSON.
fn read_file_tail(path: &Path, max_bytes: u64) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    let start_pos = file_size.saturating_sub(max_bytes);

    file.seek(SeekFrom::Start(start_pos))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;

    // If we seeked into the middle of the file, skip the first partial line
    if start_pos > 0 {
        if let Some(newline_pos) = buf.find('\n') {
            buf = buf[newline_pos + 1..].to_string();
        }
    }

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_codex_session(dir: &Path, filename: &str, cwd: &str, lines: &[&str]) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(filename);
        let mut f = File::create(&path).unwrap();
        // Write session_meta first line
        writeln!(
            f,
            r#"{{"timestamp":"2026-02-21T16:00:00Z","type":"session_meta","payload":{{"cwd":"{cwd}","id":"test-id"}}}}"#
        )
        .unwrap();
        for line in lines {
            writeln!(f, "{line}").unwrap();
        }
        f.sync_all().unwrap();
        path
    }

    fn make_update_plan_line(steps: &[(&str, &str)]) -> String {
        let plan_items: Vec<String> = steps
            .iter()
            .map(|(step, status)| format!(r#"{{"step":"{step}","status":"{status}"}}"#))
            .collect();
        let plan_json = format!("[{}]", plan_items.join(","));
        // arguments is a JSON-encoded string
        let arguments = format!(r#"{{"plan":{plan_json}}}"#);
        let escaped_arguments = arguments.replace('"', r#"\""#);
        format!(
            r#"{{"timestamp":"2026-02-21T16:00:00Z","payload":{{"type":"function_call","name":"update_plan","arguments":"{escaped_arguments}"}}}}"#
        )
    }

    #[test]
    fn parse_single_update_plan() {
        let tmp = TempDir::new().unwrap();
        let plan_line = make_update_plan_line(&[
            ("Set up project structure", "completed"),
            ("Implement core logic", "in-progress"),
            ("Write tests", "open"),
        ]);

        let path = write_codex_session(
            tmp.path(),
            "session.jsonl",
            "/home/user/project",
            &[&plan_line],
        );

        let tasks = parse_update_plan(&path).unwrap();
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].id, "codex-0");
        assert_eq!(tasks[0].subject, "Set up project structure");
        assert_eq!(tasks[0].status, TaskStatus::Completed);
        assert_eq!(tasks[0].source, CliTool::Codex);
        assert_eq!(tasks[1].id, "codex-1");
        assert_eq!(tasks[1].status, TaskStatus::InProgress);
        assert_eq!(tasks[2].id, "codex-2");
        assert_eq!(tasks[2].status, TaskStatus::Pending);
        assert_eq!(tasks[0].source_key, "test-id");
        assert_eq!(tasks[0].session_id.as_deref(), Some("test-id"));
    }

    #[test]
    fn multiple_update_plans_uses_last() {
        let tmp = TempDir::new().unwrap();

        let plan1 = make_update_plan_line(&[("Old task", "open")]);
        let plan2 =
            make_update_plan_line(&[("Old task", "completed"), ("New task", "in-progress")]);

        let path = write_codex_session(
            tmp.path(),
            "session.jsonl",
            "/home/user/project",
            &[&plan1, r#"{"payload":{"type":"response_item"}}"#, &plan2],
        );

        let tasks = parse_update_plan(&path).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].subject, "Old task");
        assert_eq!(tasks[0].status, TaskStatus::Completed);
        assert_eq!(tasks[1].subject, "New task");
        assert_eq!(tasks[1].status, TaskStatus::InProgress);
    }

    #[test]
    fn no_update_plan_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = write_codex_session(
            tmp.path(),
            "session.jsonl",
            "/home/user/project",
            &[
                r#"{"payload":{"type":"response_item"}}"#,
                r#"{"payload":{"type":"function_call","name":"other_fn","arguments":"{}"}}"#,
            ],
        );

        let tasks = parse_update_plan(&path).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn malformed_jsonl_lines_skipped() {
        let tmp = TempDir::new().unwrap();
        let plan_line = make_update_plan_line(&[("Valid task", "open")]);

        let path = write_codex_session(
            tmp.path(),
            "session.jsonl",
            "/home/user/project",
            &["not valid json", "", &plan_line],
        );

        let tasks = parse_update_plan(&path).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Valid task");
    }

    #[test]
    fn malformed_update_plan_without_survivors_is_unavailable() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");

        let today = chrono::Local::now().date_naive();
        let date_dir = sessions_dir
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        write_codex_session(
            &date_dir,
            "broken-plan.jsonl",
            "/home/user/projects/myapp",
            &[r#"{"payload":{"type":"function_call","name":"update_plan","arguments":"{"}}"#],
        );

        let outcome = get_tasks_in("/home/user/projects/myapp", &[], &sessions_dir);
        assert!(matches!(outcome, ScanOutcome::Unavailable(_)));
    }

    #[test]
    fn malformed_update_plan_with_survivors_returns_data() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");

        let today = chrono::Local::now().date_naive();
        let date_dir = sessions_dir
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let valid_plan = make_update_plan_line(&[("Recovered task", "open")]);
        write_codex_session(
            &date_dir,
            "partial-plan.jsonl",
            "/home/user/projects/myapp",
            &["not valid json but contains update_plan token", &valid_plan],
        );

        let tasks = match get_tasks_in("/home/user/projects/myapp", &[], &sessions_dir) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected partial task data, got {other:?}"),
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Recovered task");
    }

    #[test]
    fn status_mapping_variants() {
        let tmp = TempDir::new().unwrap();
        let plan_line = make_update_plan_line(&[
            ("Done task", "done"),
            ("Started task", "started"),
            ("In progress task", "in-progress"),
            ("Not started", "not-started"),
            ("Completed task", "completed"),
        ]);

        let path = write_codex_session(
            tmp.path(),
            "session.jsonl",
            "/home/user/project",
            &[&plan_line],
        );

        let tasks = parse_update_plan(&path).unwrap();
        assert_eq!(tasks[0].status, TaskStatus::Completed); // done
        assert_eq!(tasks[1].status, TaskStatus::InProgress); // started
        assert_eq!(tasks[2].status, TaskStatus::InProgress); // in-progress
        assert_eq!(tasks[3].status, TaskStatus::Pending); // not-started
        assert_eq!(tasks[4].status, TaskStatus::Completed); // completed
    }

    #[test]
    fn offline_fallback_finds_session() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");

        let today = chrono::Local::now().date_naive();
        let date_dir = sessions_dir
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let plan_line = make_update_plan_line(&[("Offline task", "open")]);
        write_codex_session(
            &date_dir,
            "rollout-2026-02-21T10-00-00-test-uuid.jsonl",
            "/home/user/projects/myapp",
            &[&plan_line],
        );

        let tasks = match get_tasks_in("/home/user/projects/myapp", &[], &sessions_dir) {
            ScanOutcome::Data(tasks) => tasks,
            other => panic!("expected task data, got {other:?}"),
        };
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].subject, "Offline task");
    }

    #[test]
    fn offline_no_match_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions");

        let today = chrono::Local::now().date_naive();
        let date_dir = sessions_dir
            .join(today.format("%Y").to_string())
            .join(today.format("%m").to_string())
            .join(today.format("%d").to_string());

        let plan_line = make_update_plan_line(&[("Other task", "open")]);
        write_codex_session(
            &date_dir,
            "rollout-2026-02-21T10-00-00-test-uuid.jsonl",
            "/home/user/projects/other",
            &[&plan_line],
        );

        let outcome = get_tasks_in("/home/user/projects/myapp", &[], &sessions_dir);
        assert_eq!(outcome, ScanOutcome::DefinitivelyEmpty);
    }

    #[test]
    fn read_file_tail_small_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("small.txt");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "line 1").unwrap();
        writeln!(f, "line 2").unwrap();
        f.sync_all().unwrap();

        let content = read_file_tail(&path, TAIL_READ_SIZE).unwrap();
        assert!(content.contains("line 1"));
        assert!(content.contains("line 2"));
    }

    #[test]
    fn read_file_tail_large_file_skips_partial_first_line() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("large.txt");
        let mut f = File::create(&path).unwrap();
        // Write enough data to exceed our small test tail size
        for i in 0..100 {
            writeln!(f, "line {i}: {}", "x".repeat(100)).unwrap();
        }
        f.sync_all().unwrap();

        // Read only last 500 bytes
        let content = read_file_tail(&path, 500).unwrap();
        // Should not start with a partial line
        let first_line = content.lines().next().unwrap_or("");
        assert!(
            first_line.starts_with("line "),
            "First line should be complete, got: {first_line}"
        );
    }

    #[test]
    fn parse_update_plan_source_key_falls_back_to_session_id_field() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session-fallback.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"sessionId":"fallback-123","payload":{{"cwd":"/home/user/project"}}}}"#
        )
        .unwrap();
        let plan = make_update_plan_line(&[("Task", "open")]);
        writeln!(f, "{plan}").unwrap();
        f.sync_all().unwrap();

        let tasks = parse_update_plan(&path).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_key, "fallback-123");
        assert_eq!(tasks[0].session_id.as_deref(), Some("fallback-123"));
    }

    #[test]
    fn parse_update_plan_source_key_falls_back_to_filename_stem() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("stem-session.jsonl");
        let mut f = File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{"type":"session_meta","payload":{{"cwd":"/home/user/project"}}}}"#
        )
        .unwrap();
        let plan = make_update_plan_line(&[("Task", "open")]);
        writeln!(f, "{plan}").unwrap();
        f.sync_all().unwrap();

        let tasks = parse_update_plan(&path).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].source_key, "stem-session");
        assert_eq!(tasks[0].session_id.as_deref(), Some("stem-session"));
    }
}
