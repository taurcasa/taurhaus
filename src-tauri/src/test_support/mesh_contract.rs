//! Binary-produced Mesh fixtures; no live roots, harnesses, or tmux clients.
//! Linux contract prerequisites: Python 3, cc, and the lock-matching mesh in
//! ~/.local/bin (or MESH_CONTRACT_BIN). Missing prerequisites fail explicitly.
use std::path::{Path, PathBuf};

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
