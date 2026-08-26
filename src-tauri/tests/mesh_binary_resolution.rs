#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path).expect("read permissions").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("set executable permissions");
}

// Regression: 03c6c3f reused any executable workspace binary, so a source
// update surfaced only as a downstream mesh lock mismatch instead of rebuilding.
#[test]
fn resolver_rebuilds_workspace_binary_when_git_commit_differs_from_lock() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("mesh-workspace");
    let release_dir = workspace.join("target/release");
    let fake_bin_dir = temp.path().join("bin");
    fs::create_dir_all(&release_dir).expect("create release dir");
    fs::create_dir_all(&fake_bin_dir).expect("create fake bin dir");

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let lock_path = repo_root.join("src-tauri/resources/mesh.lock.json");
    let lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(lock_path).expect("read mesh lock"))
            .expect("parse mesh lock");
    let mut stale = lock.clone();
    stale["git_commit"] = serde_json::json!("stale");

    let stale_binary = release_dir.join("mesh");
    write_executable(
        &stale_binary,
        &format!("#!/usr/bin/env bash\nprintf '%s\\n' '{}'\n", stale),
    );

    let rebuilt_binary = temp.path().join("rebuilt-mesh");
    write_executable(
        &rebuilt_binary,
        &format!("#!/usr/bin/env bash\nprintf '%s\\n' '{}'\n", lock),
    );

    let build_marker = temp.path().join("cargo-build-ran");
    let fake_cargo = fake_bin_dir.join("cargo");
    write_executable(
        &fake_cargo,
        "#!/usr/bin/env bash\ncp \"$REBUILT_MESH\" target/release/mesh\nchmod +x target/release/mesh\nprintf built > \"$BUILD_MARKER\"\n",
    );

    let script = repo_root.join("scripts/resolve-mesh-binary.sh");
    let path = format!(
        "{}:{}",
        fake_bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("bash")
        .arg(script)
        .env("MESH_PROJECT", &workspace)
        .env("REBUILT_MESH", &rebuilt_binary)
        .env("BUILD_MARKER", &build_marker)
        .env("PATH", path)
        .env_remove("MESH_BIN")
        .output()
        .expect("run resolver");

    assert!(
        output.status.success(),
        "resolver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(build_marker.exists(), "stale workspace binary was reused");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        stale_binary.display().to_string()
    );
}

// Regression: 4a5a62e added the commit comparison without a negative guard,
// so an unconditional rebuild would still pass the original resolver test.
#[test]
fn resolver_reuses_workspace_binary_when_git_commit_matches_lock() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("mesh-workspace");
    let release_dir = workspace.join("target/release");
    let fake_bin_dir = temp.path().join("bin");
    fs::create_dir_all(&release_dir).expect("create release dir");
    fs::create_dir_all(&fake_bin_dir).expect("create fake bin dir");

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let lock_path = repo_root.join("src-tauri/resources/mesh.lock.json");
    let lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(lock_path).expect("read mesh lock"))
            .expect("parse mesh lock");

    let matching_binary = release_dir.join("mesh");
    write_executable(
        &matching_binary,
        &format!("#!/usr/bin/env bash\nprintf '%s\\n' '{}'\n", lock),
    );

    let build_marker = temp.path().join("cargo-build-ran");
    let fake_cargo = fake_bin_dir.join("cargo");
    write_executable(
        &fake_cargo,
        "#!/usr/bin/env bash\nprintf built > \"$BUILD_MARKER\"\nexit 99\n",
    );

    let script = repo_root.join("scripts/resolve-mesh-binary.sh");
    let path = format!(
        "{}:{}",
        fake_bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("bash")
        .arg(script)
        .env("MESH_PROJECT", &workspace)
        .env("BUILD_MARKER", &build_marker)
        .env("PATH", path)
        .env_remove("MESH_BIN")
        .output()
        .expect("run resolver");

    assert!(
        output.status.success(),
        "resolver failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!build_marker.exists(), "matching binary was rebuilt");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        matching_binary.display().to_string()
    );
}

// Regression: 4a5a62e treated any Python failure as "no rebuild needed",
// silently reusing stale mesh binaries when the commit check could not run.
#[test]
fn resolver_fails_when_git_commit_comparison_cannot_run() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = temp.path().join("mesh-workspace");
    let release_dir = workspace.join("target/release");
    let fake_bin_dir = temp.path().join("bin");
    fs::create_dir_all(&release_dir).expect("create release dir");
    fs::create_dir_all(&fake_bin_dir).expect("create fake bin dir");

    let stale_binary = release_dir.join("mesh");
    write_executable(
        &stale_binary,
        "#!/usr/bin/env bash\nprintf '%s\\n' '{\"git_commit\":\"stale\"}'\n",
    );
    write_executable(&fake_bin_dir.join("python3"), "#!/bin/sh\nexit 127\n");

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let script = repo_root.join("scripts/resolve-mesh-binary.sh");
    let path = format!(
        "{}:{}",
        fake_bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("bash")
        .arg(script)
        .env("MESH_PROJECT", &workspace)
        .env("PATH", path)
        .env_remove("MESH_BIN")
        .output()
        .expect("run resolver");

    assert!(
        !output.status.success(),
        "Python failure reused stale binary"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("could not compare mesh git commit"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
