//! Binary-produced Mesh fixtures; no live roots, harnesses, or tmux clients.
//! Linux contract prerequisites: Python 3, cc, and the lock-matching mesh in
//! ~/.local/bin (or MESH_CONTRACT_BIN). Missing prerequisites fail explicitly.
use std::path::{Path, PathBuf};

// Regression: 22d0fc03 used a fixed 250 ms delay and could cut a monitor
// cycle short. The script's fake-clock tests never start Mesh or a harness.
#[test]
fn mesh_fixture_monitor_wait_regressions() {
    let root = tempfile::tempdir().unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/mesh-contract-fixture.py");
    let output = std::process::Command::new("/usr/bin/python3")
        .env_clear()
        .env("HOME", root.path())
        .arg(script)
        .arg("--self-test")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

// Regression: 22d0fc03 (then 38ba6672 and 845aa31c) put binary fixtures in
// the mandatory unit lane even though CI does not provision locked Mesh.
#[test]
fn mesh_contract_lane_is_explicit_and_unit_lane_reports_its_exclusion() {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let dry_run = |recipe: &str| {
        let output = std::process::Command::new("just")
            .current_dir(checkout)
            .args(["--dry-run", recipe])
            .output()
            .unwrap();
        assert!(output.status.success(), "{recipe}: {output:?}");
        String::from_utf8(output.stderr).unwrap()
    };
    let unit = dry_run("test-rust-unit");
    assert!(unit.contains("--skip mesh_binary_"), "{unit}");
    assert!(unit.contains("NOT RUN: Mesh binary contracts"), "{unit}");
    let contracts = dry_run("test-mesh-contracts");
    assert!(
        contracts.contains("--lib mesh_binary_ -- --test-threads=1"),
        "{contracts}"
    );
    assert!(!contracts.contains("--skip mesh_binary_"), "{contracts}");
}

pub(crate) struct MeshFixture {
    pub root: tempfile::TempDir,
    pub task_id: String,
}

impl MeshFixture {
    pub fn new(scenario: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let binary = std::env::var_os("MESH_CONTRACT_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs::home_dir().unwrap().join(".local/bin/mesh"));
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/mesh-contract-fixture.py");
        let output = std::process::Command::new("/usr/bin/python3")
            .env_clear()
            .env("HOME", root.path())
            .arg(script)
            .arg(binary)
            .arg(root.path())
            .arg(scenario)
            .output()
            .expect("run isolated Mesh fixture generator");
        assert!(
            output.status.success(),
            "Mesh fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let task_id = std::fs::read_to_string(root.path().join("task-id")).unwrap();
        Self { root, task_id }
    }

    pub fn teams(&self) -> PathBuf {
        self.root.path().join("teams")
    }
    pub fn task_path(&self) -> PathBuf {
        self.root
            .path()
            .join("tasks/deadline-team")
            .join(format!("{}.json", self.task_id))
    }
}
