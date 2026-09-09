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
