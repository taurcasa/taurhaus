use std::process::Command;

const OBSERVED_CODEX_0_149_PAYLOAD: &str =
    include_str!("../src/session_scanner/idle/fixtures/codex-agent-turn-complete-0.149.0.json");

// Regression: 791f6be had no daemon subcommand for Codex's appended notify
// payload, so managed launches could not persist a native idle edge.
#[test]
fn codex_notify_subcommand_appends_to_the_configured_app_data_root() {
    let data_dir = tempfile::tempdir().expect("data dir");
    let output = Command::new(env!("CARGO_BIN_EXE_taurhaus-daemon"))
        .args(["codex-notify", OBSERVED_CODEX_0_149_PAYLOAD])
        .env("TAURHAUS_DATA_DIR", data_dir.path())
        .output()
        .expect("run codex-notify");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "notify must not write stdout");
    let sink =
        std::fs::read_to_string(data_dir.path().join("codex-notify.jsonl")).expect("notify sink");
    let record: serde_json::Value = serde_json::from_str(sink.trim()).expect("notify record");
    assert_eq!(record["session_id"], "01a03e54-7a7a-7fb3-85f5-24dfa739a2e1");
    assert_eq!(record["event"], "agent-turn-complete");
    assert_eq!(record["turn_id"], "01a03e54-7bbf-74b2-ac52-2cfc3b0688cc");
    assert!(!sink.contains("input-messages"));
    assert!(!sink.contains("last-assistant-message"));
}
