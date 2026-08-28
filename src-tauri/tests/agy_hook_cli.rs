use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn agy_hook_invalid_payload_is_fail_open() {
    // Regression: commit 4e9e2c5 made malformed hook input exit non-zero even
    // though Antigravity runs PreInvocation synchronously as an agent-loop gate.
    let data_dir = tempfile::tempdir().expect("data dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_taurhaus-daemon"))
        .args(["agy-hook", "busy"])
        .env("TAURHAUS_DATA_DIR", data_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run agy-hook");
    child
        .stdin
        .take()
        .expect("hook stdin")
        .write_all(b"not-json")
        .expect("write malformed hook payload");
    let output = child.wait_with_output().expect("wait for agy-hook");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "{}");
    assert!(!data_dir.path().join("agy-hooks.jsonl").exists());
}
