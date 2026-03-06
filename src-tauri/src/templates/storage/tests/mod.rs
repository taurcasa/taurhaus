use std::fs;
use std::path::{Path, PathBuf};

use git2::Repository;
use tempfile::TempDir;

use super::*;

mod core;
mod git;
mod presets;
mod roles;

pub(super) fn setup_dirs() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().expect("tempdir");
    let app_data = root.path().join("app-data");
    let builtins = root.path().join("builtins");

    fs::create_dir_all(builtins.join("roles")).expect("create builtins roles");
    fs::create_dir_all(builtins.join("presets")).expect("create builtins presets");

    (root, app_data, builtins)
}

pub(super) fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

pub(super) fn lead_role_yaml(role_id: &str, instructions: &str) -> String {
    format!(
        "schema:\n  kind: role_template\n  version: 1\nrole_id: {role_id}\nname: Lead\nversion: \"1.0.0\"\nkind: lead\ndefaults:\n  cli_tool: claude\n  model: claude-opus-4-6\n  default_name_pattern: lead-{{project}}\ninstructions: \"{instructions}\"\nbehavioral_contract:\n  communication:\n    - sync\n  execution:\n    - plan\n  escalation:\n    - escalate\ncapabilities:\n  - planning\nconstraints:\n  min_instances: 1\n  max_instances: 1\n  allowed_project_binding: lead_project\n"
    )
}

pub(super) fn agent_role_yaml(role_id: &str, instructions: &str) -> String {
    format!(
        "schema:\n  kind: role_template\n  version: 1\nrole_id: {role_id}\nname: Dev\nversion: \"1.0.0\"\nkind: agent\ndefaults:\n  cli_tool: codex\n  model: gpt-5.4-high\n  default_name_pattern: dev-{{n}}\ninstructions: \"{instructions}\"\nbehavioral_contract:\n  communication:\n    - updates\n  execution:\n    - implement\n  escalation:\n    - escalate\ncapabilities:\n  - implementation\nconstraints:\n  min_instances: 0\n  max_instances: 8\n  allowed_project_binding: any\n"
    )
}

pub(super) fn preset_yaml(preset_id: &str) -> String {
    preset_yaml_with_agent(preset_id, "dev")
}

pub(super) fn preset_yaml_with_agent(preset_id: &str, agent_role_id: &str) -> String {
    format!(
        "schema:\n  kind: team_preset\n  version: 1\npreset_id: {preset_id}\nname: Base Team\ndescription: Base preset\nversion: \"1.0.0\"\nlead_role_id: lead\nagent_slots:\n  - role_id: {agent_role_id}\n    count: 1\n    project_binding: lead_project\ndefaults:\n  team_name_pattern: \"{{project}}-team\"\n  tmux_layout: tiled\n"
    )
}

pub(super) fn seed_valid_catalog(builtins_dir: &Path) {
    write(
        &builtins_dir.join("roles").join("lead.yaml"),
        &lead_role_yaml("lead", "lead built-in"),
    );
    write(
        &builtins_dir.join("roles").join("dev.yaml"),
        &agent_role_yaml("dev", "dev built-in"),
    );
    write(
        &builtins_dir.join("presets").join("base.yaml"),
        &preset_yaml("base"),
    );
}

pub(super) fn parse_role(yaml: &str) -> RoleTemplate {
    serde_yml::from_str::<RoleTemplate>(yaml).expect("parse role yaml")
}

pub(super) fn parse_preset(yaml: &str) -> TeamPreset {
    serde_yml::from_str::<TeamPreset>(yaml).expect("parse preset yaml")
}

pub(super) fn age_pending_actions(store: &TemplateStore, seconds: i64) {
    let mut state = store.load_state().expect("load state");
    for action in &mut state.pending_actions {
        action.first_seen_at -= seconds;
        action.last_seen_at -= seconds;
    }
    store.save_state(&state).expect("save state");
}

pub(super) fn latest_commit_message(repo_path: &Path) -> String {
    let repo = Repository::open(repo_path).expect("open repo");
    let head = repo.head().expect("head");
    let commit = head.peel_to_commit().expect("head commit");
    commit.message().unwrap_or("").trim().to_string()
}
