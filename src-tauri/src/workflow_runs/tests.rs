use std::fs;
use std::path::Path;

use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;

use super::*;

const RUN_ID: &str = "wf_live-123";

struct Fixture {
    _temp: TempDir,
}

fn command_fixture() -> Fixture {
    let temp = TempDir::new().expect("tempdir");
    let session_dir = temp.path().join("projects/project/session-123");
    fs::create_dir_all(session_dir.join("subagents/workflows").join(RUN_ID)).expect("run dir");
    let summary_path = session_dir.join("workflows").join(format!("{RUN_ID}.json"));
    fs::create_dir_all(summary_path.parent().expect("summary parent")).expect("summary dir");
    fs::write(
        summary_path,
        json!({
            "runId": RUN_ID,
            "status": "completed",
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
            "agentCount": 0,
            "durationMs": 1,
            "totalTokens": 0,
            "totalToolCalls": 0,
            "workflowName": "feature-pr",
            "startTime": 1_700_000_000_000_i64,
            "timestamp": "2023-11-14T22:13:20.001Z",
            "workflowProgress": []
        })
        .to_string(),
    )
    .expect("summary");
    Fixture { _temp: temp }
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

#[cfg_attr(target_os = "windows", ignore = "in-process scanner is Unix-only")]
#[test]
fn ipc_command_implementations_resolve_only_scratch_claude_session_dirs() {
    let _env_guard = crate::test_support::acquire_env_test_guard();
    let fixture = command_fixture();
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
    let provider = crate::ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };

    let summaries = list_workflow_runs_impl(&provider, "session-123").expect("list runs");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].run_id, RUN_ID);
    let summary_json = serde_json::to_value(&summaries[0]).expect("summary json");
    assert!(summary_json.get("agents").is_none());
    assert!(summary_json.get("result").is_none());

    let run = get_workflow_run_impl(&provider, "session-123", RUN_ID).expect("get run");
    assert!(run.agents.is_empty());
    assert!(run.result.is_some());
    assert_eq!(
        workflow_ledger_row_impl(&provider, "session-123", RUN_ID)
            .expect("ledger command")
            .as_deref(),
        Some("| W2a \\| scanner | Codex | Opus conformance, Opus operational | 2 | 1 | tbd |")
    );

    let error =
        get_workflow_run_impl(&provider, "missing-session", RUN_ID).expect_err("unknown session");
    assert!(error.contains("Session not found"), "{error}");
}
