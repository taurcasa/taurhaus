//! Shared argv contract for the native Mesh activation commands.
use super::process::mesh_cli_claude_dir_arg_from_path;
use std::path::Path;

pub(crate) fn create_args(team: &str, lead: &str, teams: &Path, policy: &Path) -> Vec<String> {
    let mut args = vec![
        "team".into(),
        "create".into(),
        "--messaging-canonical".into(),
        "--isolated".into(),
        "--retention-policy".into(),
        mesh_cli_claude_dir_arg_from_path(policy),
    ];
    args.extend(identity_args(team, lead, teams));
    args
}

pub(crate) fn delivery_args(team: &str, lead: &str, teams: &Path) -> Vec<String> {
    let mut args = vec![
        "team".into(),
        "delivery".into(),
        "--owner".into(),
        "team".into(),
    ];
    args.extend(identity_args(team, lead, teams));
    args
}

fn identity_args(team: &str, lead: &str, teams: &Path) -> Vec<String> {
    vec![
        "--claude-dir".into(),
        mesh_cli_claude_dir_arg_from_path(teams.parent().unwrap_or(teams)),
        "--team".into(),
        team.into(),
        "--name".into(),
        lead.into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::stores::TeamConfigStore;
    use serde_json::json;

    #[test]
    #[ignore = "NOT RUN: canonical Mesh contract requires explicit MESH_CONTRACT_BIN; run just test-canonical-mesh-contract"]
    fn canonical_mesh_binary_creates_and_round_trips_authority() {
        let binary = std::env::var_os("MESH_CONTRACT_BIN")
            .map(std::path::PathBuf::from)
            .expect("MESH_CONTRACT_BIN must name the canonical candidate");
        assert!(binary.is_absolute());
        let root = tempfile::tempdir().unwrap();
        let teams = root.path().join("teams");
        std::fs::create_dir(&teams).unwrap();
        let policy_path = teams.join("policy.json");
        let policy = json!({
            "capture_scope": "mesh-producers-only", "synthetic_disposable": true,
            "dm_horizon_days": 7, "task_horizon_days": 30, "retry_horizon_days": 30,
            "archive_owner": "lead", "archive_access": "captured-audience",
            "closure": "manual-disposal-after-evidence-export", "purge_implemented": false,
            "canonical_writers": "mesh-only", "approved_by": "taurhaus-operator",
            "inactive_horizon_days": 14, "review_horizon_days": 30
        });
        std::fs::write(&policy_path, policy.to_string()).unwrap();
        let run = |args: &[String]| {
            let output = std::process::Command::new(&binary)
                .env_clear()
                .env("HOME", root.path())
                .env("PATH", "/usr/bin:/bin")
                .current_dir(root.path())
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "Mesh refused: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        // Help/capability checks also run only here, under the disposable root.
        run(&["--help".into()]);
        run(&["team".into(), "create".into(), "--help".into()]);
        run(&create_args("trial", "lead", &teams, &policy_path));
        std::fs::remove_file(policy_path).unwrap();
        let config_path = teams.join("trial/config.json");
        let before: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&config_path).unwrap()).unwrap();
        assert_eq!(before["messaging_format"], 2);
        assert_eq!(before["minimum_writer"], "mesh-journal/2");
        assert_eq!(before["delivery_owner"], "team");
        assert_eq!(before["messaging_activation"], "disposable-mesh-only");
        assert_eq!(before["messaging_policy"], policy);
        assert!(!before["team_incarnation_id"].as_str().unwrap().is_empty());
        assert_eq!(before["members"].as_array().unwrap().len(), 1);
        assert_eq!(before["members"][0]["name"], "lead");
        assert!(teams.join("trial/state/messaging-v2").is_dir());
        let authority = teams.join("trial/state/messaging-authority.json");
        let authority_before = std::fs::read(&authority).unwrap();
        let mut config = TeamConfigStore::load(&teams, "trial").unwrap();
        config.description = Some("adopted by Taurhaus".into());
        TeamConfigStore::save(&teams, "trial", &config).unwrap();
        let after: serde_json::Value =
            serde_json::from_slice(&std::fs::read(config_path).unwrap()).unwrap();
        for key in [
            "messaging_format",
            "minimum_writer",
            "delivery_owner",
            "messaging_activation",
            "messaging_policy",
            "team_incarnation_id",
        ] {
            assert_eq!(after[key], before[key], "lost Mesh key {key}");
        }
        assert_eq!(std::fs::read(authority).unwrap(), authority_before);
        assert_eq!(
            after["members"][0]["controlAuthTokenHash"],
            before["members"][0]["controlAuthTokenHash"]
        );
        assert_eq!(
            after["members"][0]["agentId"],
            before["members"][0]["agentId"]
        );
    }
}
