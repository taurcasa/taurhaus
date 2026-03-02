//! Team configuration store.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;

const CONFIG_FILENAME: &str = "config.json";
const CONFIG_TMP_FILENAME: &str = "config.json.tmp";

/// Team configuration document persisted at `teams/<team>/config.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamConfig {
    pub schema_version: u32,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub members: Vec<Member>,
}

#[derive(Debug, Serialize)]
struct MeshCompatibleTeamConfigWire {
    schema_version: u32,
    name: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
    #[serde(rename = "createdAt")]
    created_at_millis: i64,
    #[serde(rename = "leadAgentId", skip_serializing_if = "Option::is_none")]
    lead_agent_id: Option<String>,
    #[serde(rename = "leadSessionId", skip_serializing_if = "Option::is_none")]
    lead_session_id: Option<String>,
    members: Vec<MeshCompatibleMemberWire>,
}

#[derive(Debug, Serialize)]
struct MeshCompatibleMemberWire {
    name: String,
    role: MemberRole,
    instructions: Option<String>,
    project_path: PathBuf,
    cli_tool: CliTool,
    #[serde(rename = "agentId")]
    agent_id: String,
    #[serde(rename = "agentType")]
    agent_type: String,
    model: String,
    #[serde(rename = "joinedAt")]
    joined_at_millis: i64,
    #[serde(rename = "projectPath")]
    project_path_camel: PathBuf,
    cwd: PathBuf,
    #[serde(rename = "tmuxPaneId", skip_serializing_if = "Option::is_none")]
    tmux_pane_id: Option<String>,
    #[serde(rename = "backendType", skip_serializing_if = "Option::is_none")]
    backend_type: Option<String>,
    #[serde(rename = "isActive", skip_serializing_if = "Option::is_none")]
    is_active: Option<bool>,
}

/// Team discovery entry used for project-anchor restoration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTeam {
    pub team_name: String,
    pub lead_project_path: Option<PathBuf>,
}

/// Team discovery output with skipped-folder warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamDiscovery {
    pub teams: Vec<DiscoveredTeam>,
    pub warnings: Vec<String>,
}

/// Stateless filesystem-backed store for team config documents.
#[derive(Debug, Default)]
pub struct TeamConfigStore;

#[derive(Debug, Deserialize)]
struct MeshTeamConfigWire {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
    #[serde(default)]
    members: Vec<MeshMemberWire>,
}

#[derive(Debug, Deserialize)]
struct MeshMemberWire {
    name: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(rename = "type", default)]
    type_name: Option<String>,
    #[serde(rename = "agentType", default)]
    agent_type: Option<String>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    cli_tool: Option<CliTool>,
    #[serde(rename = "projectPath", default)]
    project_path_camel: Option<PathBuf>,
    #[serde(default)]
    project_path: Option<PathBuf>,
    #[serde(default)]
    cwd: Option<PathBuf>,
    #[serde(default)]
    model: Option<String>,
}

impl TeamConfigStore {
    /// Load a single team configuration from `<teams_dir>/<team_name>/config.json`.
    pub fn load(teams_dir: &Path, team_name: &str) -> Result<TeamConfig, CoordinationError> {
        let config_path = config_path(teams_dir, team_name);
        let raw = fs::read_to_string(&config_path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => CoordinationError::NotFound(format!(
                "team config not found for '{team_name}' at {}",
                config_path.display()
            )),
            _ => CoordinationError::Io(err),
        })?;

        parse_team_config(&raw, team_name)
    }

    /// Save a team configuration atomically via advisory lock + `config.json.tmp` + rename.
    pub fn save(
        teams_dir: &Path,
        team_name: &str,
        config: &TeamConfig,
    ) -> Result<(), CoordinationError> {
        let _lock = super::lock::acquire_team_lock(teams_dir, team_name)?;

        let mut normalized = config.clone();
        normalized.schema_version = 1;
        normalized.name = team_name.to_string();

        let team_dir = team_dir(teams_dir, team_name);
        fs::create_dir_all(&team_dir)?;

        let target_path = config_path(teams_dir, team_name);
        let tmp_path = team_dir.join(CONFIG_TMP_FILENAME);
        let payload = serde_json::to_string_pretty(&mesh_compatible_wire(&normalized)).map_err(|err| {
            CoordinationError::StoreError(format!(
                "failed to serialize team config for '{team_name}': {err}"
            ))
        })?;

        if let Err(err) = fs::write(&tmp_path, payload) {
            return Err(CoordinationError::Io(err));
        }

        if let Err(err) = fs::rename(&tmp_path, &target_path) {
            // Best-effort cleanup for failed atomic swap.
            let _ = fs::remove_file(&tmp_path);
            return Err(CoordinationError::Io(err));
        }

        Ok(())
    }

    /// List team names by scanning direct child directories under `teams_dir`.
    pub fn list(teams_dir: &Path) -> Result<Vec<String>, CoordinationError> {
        if !teams_dir.exists() {
            return Ok(Vec::new());
        }

        let mut teams = Vec::new();
        for entry in fs::read_dir(teams_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let file_name = entry.file_name();
                if let Some(name) = file_name.to_str() {
                    teams.push(name.to_string());
                }
            }
        }

        teams.sort();
        Ok(teams)
    }

    /// Discover valid teams and resolve each lead project anchor.
    ///
    /// Corrupt or unreadable team folders are skipped with warning strings.
    pub fn discover(teams_dir: &Path) -> Result<TeamDiscovery, CoordinationError> {
        if !teams_dir.exists() {
            return Ok(TeamDiscovery {
                teams: Vec::new(),
                warnings: Vec::new(),
            });
        }

        let mut teams = Vec::new();
        let mut warnings = Vec::new();
        let team_dirs = Self::list(teams_dir)?;
        for team_name in team_dirs {
            match Self::load(teams_dir, &team_name) {
                Ok(config) => {
                    let lead_project_path = config
                        .members
                        .iter()
                        .find(|member| member.role == MemberRole::Lead)
                        .or_else(|| config.members.first())
                        .map(|member| member.project_path.clone());
                    teams.push(DiscoveredTeam {
                        team_name: config.name,
                        lead_project_path,
                    });
                }
                Err(CoordinationError::NotFound(_)) | Err(CoordinationError::StoreError(_)) => {
                    warnings.push(format!(
                        "skipped team folder '{team_name}' because config is missing or invalid"
                    ));
                }
                Err(CoordinationError::Io(err)) => {
                    warnings.push(format!(
                        "skipped team folder '{team_name}' due to IO error: {err}"
                    ));
                }
                Err(other) => {
                    warnings.push(format!(
                        "skipped team folder '{team_name}' due to discovery error: {other}"
                    ));
                }
            }
        }

        teams.sort_by(|a, b| a.team_name.cmp(&b.team_name));
        warnings.sort();
        Ok(TeamDiscovery { teams, warnings })
    }

    /// Remove `<teams_dir>/<team_name>` recursively.
    pub fn delete(teams_dir: &Path, team_name: &str) -> Result<(), CoordinationError> {
        let path = team_dir(teams_dir, team_name);
        match fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(CoordinationError::Io(err)),
        }
    }
}

fn mesh_agent_id(team_name: &str, member_name: &str) -> String {
    format!("{member_name}@{team_name}")
}

fn mesh_compatible_wire(config: &TeamConfig) -> MeshCompatibleTeamConfigWire {
    let created_at_millis = config.created_at.timestamp_millis();
    let lead_member = config
        .members
        .iter()
        .find(|member| member.role == MemberRole::Lead)
        .or_else(|| config.members.first());
    let lead_agent_id = lead_member.map(|member| mesh_agent_id(&config.name, &member.name));
    let lead_session_id = lead_member.map(|member| format!("{}-session", member.name));

    let members = config
        .members
        .iter()
        .map(|member| {
            let project_path = member.project_path.clone();
            MeshCompatibleMemberWire {
                name: member.name.clone(),
                role: member.role,
                instructions: member.instructions.clone(),
                project_path: project_path.clone(),
                cli_tool: member.cli_tool,
                agent_id: mesh_agent_id(&config.name, &member.name),
                agent_type: if member.role == MemberRole::Lead {
                    "team-lead".to_string()
                } else {
                    "general-purpose".to_string()
                },
                model: default_model_for_cli(member.cli_tool).to_string(),
                joined_at_millis: created_at_millis,
                project_path_camel: project_path.clone(),
                cwd: project_path,
                tmux_pane_id: if member.role == MemberRole::Lead {
                    Some(String::new())
                } else {
                    None
                },
                backend_type: if member.role == MemberRole::Lead {
                    None
                } else {
                    Some("external".to_string())
                },
                is_active: if member.role == MemberRole::Lead {
                    None
                } else {
                    Some(true)
                },
            }
        })
        .collect();

    MeshCompatibleTeamConfigWire {
        schema_version: config.schema_version,
        name: config.name.clone(),
        description: config.description.clone(),
        created_at: config.created_at,
        created_at_millis,
        lead_agent_id,
        lead_session_id,
        members,
    }
}

fn default_model_for_cli(cli_tool: CliTool) -> &'static str {
    match cli_tool {
        CliTool::Claude => "claude-opus-4-6",
        CliTool::Codex => "gpt-5.3-codex",
        CliTool::Gemini => "gemini-2.5-pro",
    }
}

fn parse_team_config(raw: &str, team_name: &str) -> Result<TeamConfig, CoordinationError> {
    let value: Value = serde_json::from_str(raw).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to parse config.json for '{team_name}': {err}"
        ))
    })?;

    // Mesh currently uses camelCase keys (createdAt, leadAgentId, etc).
    if value.get("createdAt").is_some() || value.get("leadAgentId").is_some() {
        parse_mesh_config(value, team_name)
    } else {
        parse_native_config(value, team_name)
    }
}

fn parse_native_config(value: Value, team_name: &str) -> Result<TeamConfig, CoordinationError> {
    #[derive(Debug, Deserialize)]
    struct NativeTeamConfigWire {
        #[serde(default = "schema_version_one")]
        schema_version: u32,
        name: String,
        #[serde(default)]
        description: Option<String>,
        created_at: DateTime<Utc>,
        #[serde(default)]
        members: Vec<Member>,
    }

    let wire: NativeTeamConfigWire = serde_json::from_value(value).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to deserialize native config format for '{team_name}': {err}"
        ))
    })?;

    Ok(TeamConfig {
        schema_version: wire.schema_version,
        name: wire.name,
        description: wire.description,
        created_at: wire.created_at,
        members: wire.members,
    })
}

fn parse_mesh_config(value: Value, team_name: &str) -> Result<TeamConfig, CoordinationError> {
    let wire: MeshTeamConfigWire = serde_json::from_value(value).map_err(|err| {
        CoordinationError::StoreError(format!(
            "failed to deserialize mesh config format for '{team_name}': {err}"
        ))
    })?;

    let created_at = wire
        .created_at
        .and_then(|millis| Utc.timestamp_millis_opt(millis).single())
        .unwrap_or_else(Utc::now);

    let members = wire
        .members
        .into_iter()
        .map(mesh_member_to_domain)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(TeamConfig {
        schema_version: 1,
        name: wire.name,
        description: wire.description,
        created_at,
        members,
    })
}

fn mesh_member_to_domain(member: MeshMemberWire) -> Result<Member, CoordinationError> {
    let role_hint = member
        .role
        .as_deref()
        .or(member.type_name.as_deref())
        .or(member.agent_type.as_deref())
        .unwrap_or("agent");
    let role = parse_role(role_hint);
    let cli_tool = member
        .cli_tool
        .or_else(|| member.model.as_deref().map(cli_tool_from_model))
        .unwrap_or(CliTool::Codex);
    let project_path = member
        .project_path
        .or(member.project_path_camel)
        .or(member.cwd)
        .ok_or_else(|| {
            CoordinationError::StoreError(format!(
                "mesh member '{}' missing project path field (project_path/projectPath/cwd)",
                member.name
            ))
        })?;

    Ok(Member {
        name: member.name,
        role,
        instructions: member.instructions,
        project_path,
        cli_tool,
    })
}

fn parse_role(value: &str) -> MemberRole {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "lead" || normalized == "team-lead" {
        MemberRole::Lead
    } else {
        MemberRole::Agent
    }
}

fn cli_tool_from_model(model: &str) -> CliTool {
    let lower = model.to_ascii_lowercase();
    if lower.contains("claude") {
        CliTool::Claude
    } else if lower.contains("gemini") {
        CliTool::Gemini
    } else {
        CliTool::Codex
    }
}

fn team_dir(teams_dir: &Path, team_name: &str) -> PathBuf {
    teams_dir.join(team_name)
}

fn config_path(teams_dir: &Path, team_name: &str) -> PathBuf {
    team_dir(teams_dir, team_name).join(CONFIG_FILENAME)
}

const fn schema_version_one() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-03-01T21:00:00Z")
            .expect("valid RFC3339 timestamp")
            .with_timezone(&Utc)
    }

    fn sample_config(team_name: &str) -> TeamConfig {
        TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: Some("Architecture team".to_string()),
            created_at: test_timestamp(),
            members: vec![Member {
                name: "team-lead".to_string(),
                role: MemberRole::Lead,
                instructions: Some("Own orchestration".to_string()),
                project_path: PathBuf::from("/tmp/taurhaus"),
                cli_tool: CliTool::Claude,
            }],
        }
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let loaded = TeamConfigStore::load(teams_dir, team_name).expect("load should succeed");

        assert_eq!(loaded, config);
    }

    #[test]
    fn save_writes_mesh_compatibility_fields() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let raw =
            fs::read_to_string(config_path(teams_dir, team_name)).expect("read serialized config");
        let value: Value = serde_json::from_str(&raw).expect("parse serialized config");

        assert_eq!(
            value.get("leadAgentId").and_then(Value::as_str),
            Some("team-lead@architecture-final")
        );
        assert_eq!(
            value.get("leadSessionId").and_then(Value::as_str),
            Some("team-lead-session")
        );
        let members = value
            .get("members")
            .and_then(Value::as_array)
            .expect("members array");
        assert_eq!(
            members[0].get("agentId").and_then(Value::as_str),
            Some("team-lead@architecture-final")
        );
        assert!(members[0].get("joinedAt").is_some());
        assert!(members[0].get("model").is_some());
        assert!(
            members[0].get("projectPath").is_some(),
            "projectPath compatibility field should be present"
        );
    }

    #[test]
    fn list_returns_team_directories_sorted() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();

        fs::create_dir_all(teams_dir.join("zeta")).expect("create zeta");
        fs::create_dir_all(teams_dir.join("alpha")).expect("create alpha");
        fs::write(teams_dir.join("README.txt"), "not a team dir").expect("write file");

        let teams = TeamConfigStore::list(teams_dir).expect("list should succeed");
        assert_eq!(teams, vec!["alpha".to_string(), "zeta".to_string()]);
    }

    #[test]
    fn corrupt_json_returns_store_error() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "broken-team";
        let team_dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&team_dir).expect("create team dir");
        fs::write(team_dir.join(CONFIG_FILENAME), "{ this is invalid json").expect("write garbage");

        let err = TeamConfigStore::load(tmp.path(), team_name).expect_err("expected parse failure");
        match err {
            CoordinationError::StoreError(message) => {
                assert!(message.contains("failed to parse config.json"));
            }
            other => panic!("expected store error, got {other:?}"),
        }
    }

    #[test]
    fn save_does_not_leave_tmp_file_on_success() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        TeamConfigStore::save(teams_dir, team_name, &sample_config(team_name))
            .expect("save should succeed");

        let tmp_path = team_dir(teams_dir, team_name).join(CONFIG_TMP_FILENAME);
        assert!(
            !tmp_path.exists(),
            "tmp file should not linger after successful save"
        );
    }

    #[test]
    fn duplicate_save_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("first save");
        TeamConfigStore::save(teams_dir, team_name, &config).expect("second save should succeed");

        let loaded = TeamConfigStore::load(teams_dir, team_name).expect("load after double save");
        assert_eq!(loaded, config);
    }

    #[test]
    fn duplicate_delete_is_idempotent() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";

        TeamConfigStore::save(teams_dir, team_name, &sample_config(team_name)).expect("save");
        TeamConfigStore::delete(teams_dir, team_name).expect("first delete");
        TeamConfigStore::delete(teams_dir, team_name).expect("second delete should not error");
    }

    #[test]
    fn concurrent_saves_do_not_corrupt() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = Arc::new(tmp.path().to_path_buf());
        let team_name = "concurrent-team";
        let barrier = Arc::new(Barrier::new(8));

        // Pre-create team dir so locks work.
        TeamConfigStore::save(&teams_dir, team_name, &sample_config(team_name))
            .expect("initial save");

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let dir = Arc::clone(&teams_dir);
                let bar = Arc::clone(&barrier);
                let name = team_name.to_string();
                thread::spawn(move || {
                    bar.wait();
                    let mut config = sample_config(&name);
                    config.description = Some(format!("thread-{i}"));
                    TeamConfigStore::save(&dir, &name, &config).expect("concurrent save");
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }

        // The final state should be a valid config from one of the threads.
        let loaded =
            TeamConfigStore::load(&teams_dir, team_name).expect("load after concurrent saves");
        assert_eq!(loaded.schema_version, 1);
        assert_eq!(loaded.name, team_name);
        assert!(
            loaded.description.as_ref().unwrap().starts_with("thread-"),
            "description should be from one of the concurrent writers"
        );
    }

    #[test]
    fn load_mesh_format_config() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "architecture-final";
        let dir = team_dir(tmp.path(), team_name);
        fs::create_dir_all(&dir).expect("create team dir");

        let mesh_json = r#"{
  "name": "architecture-final",
  "description": "Final architecture discussion",
  "createdAt": 1772399806546,
  "leadAgentId": "team-lead@architecture-final",
  "members": [
    {
      "name": "team-lead",
      "agentType": "team-lead",
      "model": "claude-opus-4-6",
      "cwd": "/home/mstie/projects/taurhaus"
    },
    {
      "name": "codex-reviewer",
      "agentType": "general-purpose",
      "model": "gpt-5.2-codex",
      "cwd": "/home/mstie/projects/taurhaus"
    }
  ]
}"#;
        fs::write(dir.join(CONFIG_FILENAME), mesh_json).expect("write mesh config");

        let config = TeamConfigStore::load(tmp.path(), team_name).expect("load should succeed");
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.name, "architecture-final");
        assert_eq!(config.members.len(), 2);
        assert_eq!(config.members[0].role, MemberRole::Lead);
        assert_eq!(config.members[0].cli_tool, CliTool::Claude);
        assert_eq!(config.members[1].role, MemberRole::Agent);
        assert_eq!(config.members[1].cli_tool, CliTool::Codex);
    }

    #[test]
    fn load_missing_config_returns_not_found() {
        let tmp = TempDir::new().expect("tempdir");
        let err = TeamConfigStore::load(tmp.path(), "missing-team").expect_err("missing team");
        match err {
            CoordinationError::NotFound(message) => assert!(message.contains("missing-team")),
            other => panic!("expected not found, got {other:?}"),
        }
    }

    #[test]
    fn list_returns_io_error_when_teams_dir_is_not_a_directory() {
        let tmp = TempDir::new().expect("tempdir");
        let file_path = tmp.path().join("not-a-dir");
        fs::write(&file_path, "x").expect("write file");

        let err = TeamConfigStore::list(&file_path).expect_err("file path should error");
        match err {
            CoordinationError::Io(io) => {
                assert!(
                    io.kind() == std::io::ErrorKind::NotADirectory
                        || io.kind() == std::io::ErrorKind::Other
                );
            }
            other => panic!("expected io error, got {other:?}"),
        }
    }

    #[test]
    fn discover_resolves_lead_project_anchor() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "architecture-final";
        let config = sample_config(team_name);

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save should succeed");
        let discovery = TeamConfigStore::discover(teams_dir).expect("discover should succeed");

        assert_eq!(discovery.warnings.len(), 0);
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, team_name);
        assert_eq!(
            discovery.teams[0].lead_project_path.as_deref(),
            Some(Path::new("/tmp/taurhaus"))
        );
    }

    #[test]
    fn discover_skips_corrupt_folder_with_warning() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let valid_team = "alpha";
        TeamConfigStore::save(teams_dir, valid_team, &sample_config(valid_team))
            .expect("save valid team");

        let corrupt_team = "broken-team";
        let corrupt_dir = team_dir(teams_dir, corrupt_team);
        fs::create_dir_all(&corrupt_dir).expect("create corrupt dir");
        fs::write(corrupt_dir.join(CONFIG_FILENAME), "{ invalid json").expect("write corrupt");

        let discovery = TeamConfigStore::discover(teams_dir).expect("discover should succeed");
        assert_eq!(discovery.teams.len(), 1);
        assert_eq!(discovery.teams[0].team_name, valid_team);
        assert_eq!(discovery.warnings.len(), 1);
        assert!(discovery.warnings[0].contains(corrupt_team));
    }

    #[test]
    fn discover_uses_first_member_when_no_lead_role_present() {
        let tmp = TempDir::new().expect("tempdir");
        let teams_dir = tmp.path();
        let team_name = "agent-only-team";
        let config = TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: test_timestamp(),
            members: vec![Member {
                name: "member-a".to_string(),
                role: MemberRole::Agent,
                instructions: None,
                project_path: PathBuf::from("/tmp/agent-a"),
                cli_tool: CliTool::Codex,
            }],
        };

        TeamConfigStore::save(teams_dir, team_name, &config).expect("save");
        let discovery = TeamConfigStore::discover(teams_dir).expect("discover");
        assert_eq!(
            discovery.teams[0].lead_project_path.as_deref(),
            Some(Path::new("/tmp/agent-a"))
        );
    }
}
