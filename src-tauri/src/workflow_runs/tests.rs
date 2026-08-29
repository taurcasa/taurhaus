use std::fs::{self, File, FileTimes};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use super::*;

const LIVE_RUN_ID: &str = "wf_live-123";

struct Fixture {
    _temp: TempDir,
    session_dir: PathBuf,
    first_transcript: PathBuf,
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, contents).expect("write fixture");
}

fn set_modified(path: &Path, modified: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .expect("open fixture")
        .set_times(FileTimes::new().set_modified(modified))
        .expect("set fixture mtime");
}

fn live_fixture() -> Fixture {
    let temp = TempDir::new().expect("tempdir");
    let session_dir = temp.path().join("projects/project/session-123");
    let run_dir = session_dir.join("subagents/workflows").join(LIVE_RUN_ID);

    write(
        &session_dir
            .join("workflows/scripts")
            .join(format!("feature-pr-{LIVE_RUN_ID}.js")),
        r#"export const meta = {
  name: 'feature-pr',
  description: 'Implement, review, and gate a feature',
  phases: [
    { title: 'Implement', detail: 'red then green' },
    { title: 'Review' },
    { title: 'Gate' },
  ],
}
return { ok: true }
"#,
    );
    write(
        &run_dir.join("journal.jsonl"),
        &format!(
            "{}\nnot-json\n{}\n{}\n",
            json!({"type":"started","key":"v2:first","agentId":"agent-one"}),
            json!({"type":"started","key":"v2:second","agentId":"agent-two"}),
            json!({"type":"result","key":"v2:first","agentId":"agent-one","result":{"status":"ok"}}),
        ),
    );

    let first_transcript = run_dir.join("agent-agent-one.jsonl");
    let mut first = File::create(&first_transcript).expect("first transcript");
    writeln!(
        first,
        "{}",
        json!({"type":"user","message":{"role":"user","content":"Implement the scanner without reading real config dirs"}})
    )
    .expect("first user");
    writeln!(
        first,
        "{}",
        json!({
            "type":"assistant",
            "message":{
                "id":"message-one",
                "role":"assistant",
                "model":"claude-opus-5",
                "usage":{
                    "input_tokens":10,
                    "output_tokens":5,
                    "cache_read_input_tokens":20,
                    "cache_creation_input_tokens":2
                },
                "content":[
                    {"type":"text","text":"Working"},
                    {"type":"tool_use","id":"tool-one","name":"Read","input":{}}
                ]
            }
        })
    )
    .expect("first assistant");
    write!(first, "{{\"type\":\"assistant\"").expect("mid-write line");
    first.flush().expect("flush first transcript");

    write(
        &run_dir.join("agent-agent-two.jsonl"),
        &format!(
            "{}\n{}\n",
            json!({"type":"user","message":{"role":"user","content":[{"type":"text","text":"Review the implementation carefully"}]}}),
            json!({
                "type":"assistant",
                "message":{
                    "id":"message-two",
                    "role":"assistant",
                    "model":"claude-fable-5",
                    "usage":{"input_tokens":4,"output_tokens":3},
                    "content":[{"type":"tool_use","id":"tool-two","name":"Grep","input":{}}]
                }
            }),
        ),
    );

    Fixture {
        _temp: temp,
        session_dir,
        first_transcript,
    }
}

fn completed_summary(run_id: &str, status: &str) -> serde_json::Value {
    json!({
        "runId": run_id,
        "status": status,
        "result": {
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
        },
        "agentCount": 2,
        "durationMs": 2400,
        "totalTokens": 91,
        "totalToolCalls": 4,
        "defaultModel": "claude-opus-5",
        "workflowName": "feature-pr",
        "phases": [{"title":"Implement"},{"title":"Review"},{"title":"Gate"}],
        "scriptPath": "/scratch/feature-pr.js",
        "startTime": 1_787_949_435_335_i64,
        "timestamp": "2026-08-28T20:37:17.672Z",
        "workflowProgress": [
            {"type":"workflow_phase","index":1,"title":"Implement"},
            {
                "type":"workflow_agent",
                "index":1,
                "label":"implementer",
                "phaseIndex":1,
                "phaseTitle":"Implement",
                "agentId":"agent-one",
                "model":"claude-opus-5",
                "state":"done",
                "startedAt":1_787_949_435_347_i64,
                "queuedAt":1_787_949_435_346_i64,
                "lastToolName":"Bash",
                "lastProgressAt":1_787_949_436_814_i64,
                "tokens":60,
                "toolCalls":3,
                "durationMs":1465,
                "promptPreview":"Implement the scanner",
                "resultPreview":"done"
            },
            {
                "type":"workflow_agent",
                "index":2,
                "label":"reviewer",
                "phaseIndex":2,
                "phaseTitle":"Review",
                "agentId":"agent-two",
                "model":"claude-fable-5",
                "state":"failed",
                "startedAt":1_787_949_435_348_i64,
                "queuedAt":1_787_949_435_346_i64,
                "lastToolName":null,
                "lastProgressAt":1_787_949_437_672_i64,
                "tokens":31,
                "toolCalls":1,
                "durationMs":1513,
                "promptPreview":"Review the scanner",
                "resultPreview":{"error":"review unavailable"}
            }
        ]
    })
}

#[test]
fn live_run_is_reconstructed_from_script_journal_and_bounded_transcript_reads() {
    let fixture = live_fixture();

    let runs = scan_session_runs(&fixture.session_dir);

    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.run_id, LIVE_RUN_ID);
    assert_eq!(run.name, "feature-pr");
    assert_eq!(run.description, "Implement, review, and gate a feature");
    assert_eq!(run.phases, ["Implement", "Review", "Gate"]);
    assert_eq!(run.status, WorkflowRunStatus::Live);
    assert_eq!(run.finished_at, None);
    assert_eq!(run.result, None);
    assert!(run
        .script_path
        .ends_with(format!("feature-pr-{LIVE_RUN_ID}.js")));
    assert_eq!(run.totals.agents, 2);
    assert_eq!(run.totals.done, 1);
    assert_eq!(run.totals.tokens, Some(44));
    assert_eq!(run.totals.tool_calls, Some(2));
    assert_eq!(run.totals.duration_ms, None);

    assert_eq!(run.agents.len(), 2);
    let first = &run.agents[0];
    assert_eq!(first.agent_id, "agent-one");
    assert_eq!(first.label, None, "live labels are not guessed");
    assert_eq!(first.phase, None, "live phases are not guessed");
    assert_eq!(first.model.as_deref(), Some("claude-opus-5"));
    assert_eq!(first.state, WorkflowAgentState::Done);
    assert_eq!(
        first.prompt_preview,
        "Implement the scanner without reading real config dirs"
    );
    assert_eq!(first.last_tool.as_deref(), Some("Read"));
    assert_eq!(first.tokens, Some(37));
    assert_eq!(first.tool_calls, Some(1));
    assert_eq!(first.result_preview, Some(json!({"status":"ok"})));

    let second = &run.agents[1];
    assert_eq!(second.agent_id, "agent-two");
    assert_eq!(second.state, WorkflowAgentState::Running);
    assert_eq!(second.prompt_preview, "Review the implementation carefully");
    assert_eq!(second.last_tool.as_deref(), Some("Grep"));
    assert_eq!(second.tokens, Some(7));
}

#[test]
fn completed_summary_is_authoritative_for_agents_totals_and_result() {
    let fixture = live_fixture();
    write(
        &fixture
            .session_dir
            .join("workflows")
            .join(format!("{LIVE_RUN_ID}.json")),
        &completed_summary(LIVE_RUN_ID, "completed").to_string(),
    );

    let run = read_run(&fixture.session_dir, LIVE_RUN_ID).expect("completed run");

    assert_eq!(run.status, WorkflowRunStatus::Completed);
    assert_eq!(run.started_at, 1_787_949_435_335);
    assert_eq!(run.finished_at, Some(1_787_949_437_672));
    assert_eq!(run.totals.agents, 2);
    assert_eq!(run.totals.done, 2);
    assert_eq!(run.totals.tokens, Some(91));
    assert_eq!(run.totals.tool_calls, Some(4));
    assert_eq!(run.totals.duration_ms, Some(2400));
    assert!(run
        .result
        .as_ref()
        .is_some_and(|result| result["ledger"].is_object()));
    assert_eq!(run.agents[0].label.as_deref(), Some("implementer"));
    assert_eq!(run.agents[0].phase.as_deref(), Some("Implement"));
    assert_eq!(run.agents[0].last_tool.as_deref(), Some("Bash"));
    assert_eq!(run.agents[1].state, WorkflowAgentState::Failed);
    assert_eq!(
        run.agents[1].result_preview,
        Some(json!({"error":"review unavailable"}))
    );
}

#[test]
fn failed_and_unknown_summary_statuses_are_preserved() {
    for (raw, expected) in [
        ("failed", WorkflowRunStatus::Failed),
        ("cancelled", WorkflowRunStatus::Unknown),
    ] {
        let fixture = live_fixture();
        write(
            &fixture
                .session_dir
                .join("workflows")
                .join(format!("{LIVE_RUN_ID}.json")),
            &completed_summary(LIVE_RUN_ID, raw).to_string(),
        );

        assert_eq!(
            read_run(&fixture.session_dir, LIVE_RUN_ID)
                .expect("summary run")
                .status,
            expected
        );
    }
}

#[test]
fn empty_workflow_directory_and_invalid_run_ids_are_fail_soft() {
    let temp = TempDir::new().expect("tempdir");
    fs::create_dir_all(temp.path().join("subagents/workflows")).expect("workflow root");

    assert!(scan_session_runs(temp.path()).is_empty());
    assert_eq!(read_run(temp.path(), "../escape"), None);
}

#[test]
fn workflow_activity_requires_a_live_transcript_write_within_sixty_seconds() {
    let fixture = live_fixture();
    let written_at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    set_modified(&fixture.first_transcript, written_at);

    let recent = workflow_activity(&fixture.session_dir, written_at + Duration::from_secs(60))
        .expect("sixty seconds is inside the activity window");
    assert_eq!(recent.live_runs, 1);
    assert_eq!(recent.last_write_at, 1_800_000_000_000);

    assert_eq!(
        workflow_activity(&fixture.session_dir, written_at + Duration::from_secs(61)),
        None
    );

    write(
        &fixture
            .session_dir
            .join("workflows")
            .join(format!("{LIVE_RUN_ID}.json")),
        &completed_summary(LIVE_RUN_ID, "completed").to_string(),
    );
    assert_eq!(
        workflow_activity(&fixture.session_dir, written_at + Duration::from_secs(1)),
        None,
        "completed runs do not make the parent look active"
    );
}

#[test]
fn ledger_row_renders_only_the_procedure_return_shape() {
    let fixture = live_fixture();
    let mut run = scan_session_runs(&fixture.session_dir)
        .into_iter()
        .next()
        .expect("live run");
    run.result = completed_summary(LIVE_RUN_ID, "completed")["result"]
        .clone()
        .into();

    assert_eq!(
        ledger_row(&run).as_deref(),
        Some("| W2a \\| scanner | Codex | Opus conformance, Opus operational | 2 | 1 | tbd |")
    );

    run.result = Some(json!("plain workflow result"));
    assert_eq!(ledger_row(&run), None);
}

struct EnvRestore {
    values: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvRestore {
    fn set(paths: &[(&'static str, &Path)]) -> Self {
        let values = paths
            .iter()
            .map(|(key, path)| {
                let previous = std::env::var_os(key);
                std::env::set_var(key, path);
                (*key, previous)
            })
            .collect();
        Self { values }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (key, value) in self.values.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[test]
fn ipc_command_implementations_resolve_only_scratch_claude_session_dirs() {
    let _env_guard = crate::test_support::acquire_env_test_guard();
    let fixture = live_fixture();
    let data_dir = fixture._temp.path().join("taurhaus-data");
    let config_dir = fixture._temp.path().to_path_buf();
    fs::create_dir_all(&data_dir).expect("data dir");
    let _env = EnvRestore::set(&[
        ("TAURHAUS_DATA_DIR", &data_dir),
        ("TAURHAUS_CLAUDE_DIR", &config_dir),
        ("CLAUDE_CONFIG_DIR", &config_dir),
    ]);
    let workflow_tool = crate::session_scanner::cli_tool::all()
        .iter()
        .find(|entry| entry.capabilities.workflow_runs)
        .expect("workflow tool")
        .tool;
    let _accounts = crate::session_scanner::accounts::install_detection_override(
        workflow_tool,
        crate::session_scanner::accounts::AccountScan {
            config_dirs: vec![config_dir],
            accounts: Vec::new(),
        },
    );
    write(
        &fixture
            .session_dir
            .join("workflows")
            .join(format!("{LIVE_RUN_ID}.json")),
        &completed_summary(LIVE_RUN_ID, "completed").to_string(),
    );
    let provider = crate::ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let summaries = list_workflow_runs_impl(&provider, "session-123").expect("list runs");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].run_id, LIVE_RUN_ID);
    let summary_json = serde_json::to_value(&summaries[0]).expect("summary json");
    assert!(summary_json.get("agents").is_none());
    assert!(summary_json.get("result").is_none());

    let run = get_workflow_run_impl(&provider, "session-123", LIVE_RUN_ID).expect("get run");
    assert_eq!(run.agents.len(), 2);
    assert!(run.result.is_some());
    assert_eq!(
        workflow_ledger_row_impl(&provider, "session-123", LIVE_RUN_ID)
            .expect("ledger command")
            .as_deref(),
        Some("| W2a \\| scanner | Codex | Opus conformance, Opus operational | 2 | 1 | tbd |")
    );

    let error = get_workflow_run_impl(&provider, "missing-session", LIVE_RUN_ID)
        .expect_err("unknown session");
    assert!(error.contains("Session not found"), "{error}");
}
