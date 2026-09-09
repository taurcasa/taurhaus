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
    let mut args = Vec::new();
    // Match spawn_team_daemon_at_root: never reinterpret the teams root as its parent.
    if let Some(root) = teams.parent() {
        args.extend([
            "--claude-dir".into(),
            mesh_cli_claude_dir_arg_from_path(root),
        ]);
    }
    args.extend(["--team".into(), team.into(), "--name".into(), lead.into()]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::stores::TeamConfigStore;

    // Regression: 7ffdba14 unbalanced string delimiters confused the source boundary scanner.
    fn frontend_canonical_policy() -> serde_json::Value {
        // The shipping literal is valid JSON so this lane validates exactly the UI policy.
        let source = include_str!("../../../../src/lib/components/meshTabUtils.js");
        let literal = source
            .split_once("export const DEFAULT_CANONICAL_POLICY = Object.freeze({")
            .unwrap()
            .1
            .split_once("\n})")
            .unwrap()
            .0;
        serde_json::from_str(&format!("{{{literal}\n}}"))
            .expect("keep the shared policy literal valid JSON")
    }

    // Regression: 796bba0e duplicated the UI policy in the candidate contract.
    #[test]
    fn canonical_review_contract_reads_shipping_policy() {
        assert!(frontend_canonical_policy().is_object());
    }

    // Regression: b643834d retargeted parentless roots into an unintended nested teams directory.
    #[test]
    fn canonical_review_parentless_root_does_not_invent_claude_dir() {
        assert!(!identity_args("trial", "lead", Path::new("/"))
            .iter()
            .any(|arg| arg == "--claude-dir"));
    }

    // Regression: 796bba0e gave the candidate a name matching the locked Mesh lane filter.
    #[test]
    fn canonical_candidate_is_excluded_from_locked_mesh_contract_lane() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--list", "--ignored", "mesh_binary_"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let selected = String::from_utf8(output.stdout).unwrap();
        assert!(
            !selected.contains("team_activation::"),
            "candidate leaked into locked lane: {selected}"
        );
    }

    #[test]
    #[ignore = "NOT RUN: canonical Mesh contract requires explicit MESH_CONTRACT_BIN; run just test-canonical-mesh-contract"]
    fn canonical_activation_candidate_creates_and_round_trips_authority() {
        let binary = std::env::var_os("MESH_CONTRACT_BIN")
            .map(std::path::PathBuf::from)
            .expect("MESH_CONTRACT_BIN must name the canonical candidate");
        assert!(binary.is_absolute());
        let root = tempfile::tempdir().unwrap();
        let teams = root.path().join("teams");
        std::fs::create_dir(&teams).unwrap();
        let policy_path = teams.join("policy.json");
        let policy = frontend_canonical_policy();
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
        config.members[0].role = crate::coordination::domain::MemberRole::Lead;
        config.members[0].cli_tool = crate::session_scanner::cli_tool::CliTool::Codex;
        config.members[0].model = Some("gpt-6-astra".into());
        config.members[0].project_path = root.path().into();
        let mut builder = config.members[0].clone();
        builder.name = "builder".into();
        builder.role = crate::coordination::domain::MemberRole::Agent;
        builder.extra.clear();
        config.members.push(builder);
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
        // Mesh itself must still authenticate the adopted lead after Taurhaus saves.
        // No seats are launched here: the expected refusal is the missing runtime.
        let credential: serde_json::Value = serde_json::from_slice(
            &std::fs::read(teams.join("trial/state/control_auth/lead.json")).unwrap(),
        )
        .unwrap();
        let output = std::process::Command::new(&binary)
            .env_clear()
            .env("HOME", root.path())
            .env("PATH", "/usr/bin:/bin")
            .env("MESH_CONTROL_TOKEN", credential["token"].as_str().unwrap())
            .current_dir(root.path())
            .args(delivery_args("trial", "lead", &teams))
            .output()
            .unwrap();
        let refusal = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(
            refusal.contains("pending: runtime lead:")
                && refusal.contains("No such file or directory"),
            "unexpected refusal: {}",
            refusal.replace(credential["token"].as_str().unwrap(), "[redacted]")
        );
        assert!(!refusal.contains("is not authorized for team-wide control"));
    }
}
