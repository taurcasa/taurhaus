//! Parse Claude Code team configs and task lists (v1.1 UI).
//!
//! Team configs live at `~/.claude/teams/{name}/config.json`.
//! Task lists live at `~/.claude/tasks/{name}/`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

/// A team member from a Claude Code team configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub name: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "agentType")]
    pub agent_type: String,
}

/// Parsed Claude Code team configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    /// Team name (derived from directory name).
    #[serde(skip)]
    pub name: String,
    /// Team members.
    #[serde(default)]
    pub members: Vec<TeamMember>,
}

/// List all teams from the Claude Code teams directory.
///
/// Returns an empty vec if the directory doesn't exist.
pub fn list_teams(teams_dir: &Path) -> Result<Vec<TeamConfig>, AppError> {
    if !teams_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut teams = Vec::new();

    for entry in std::fs::read_dir(teams_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let config_path = path.join("config.json");
        if !config_path.exists() {
            continue;
        }

        let team_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        match std::fs::read_to_string(&config_path) {
            Ok(content) => {
                match serde_json::from_str::<TeamConfig>(&content) {
                    Ok(mut config) => {
                        config.name = team_name;
                        teams.push(config);
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %config_path.display(),
                            error = %e,
                            "Failed to parse team config"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    path = %config_path.display(),
                    error = %e,
                    "Failed to read team config"
                );
            }
        }
    }

    teams.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(teams)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn list_teams_empty_dir() {
        let dir = TempDir::new().unwrap();
        let teams = list_teams(dir.path()).unwrap();
        assert!(teams.is_empty());
    }

    #[test]
    fn list_teams_nonexistent_dir() {
        let teams = list_teams(Path::new("/nonexistent/teams")).unwrap();
        assert!(teams.is_empty());
    }

    #[test]
    fn list_teams_with_configs() {
        let dir = TempDir::new().unwrap();

        // Create team alpha
        let alpha_dir = dir.path().join("alpha");
        std::fs::create_dir_all(&alpha_dir).unwrap();
        std::fs::write(
            alpha_dir.join("config.json"),
            r#"{
                "members": [
                    {"name": "lead", "agentId": "a1", "agentType": "general-purpose"},
                    {"name": "worker", "agentId": "a2", "agentType": "Bash"}
                ]
            }"#,
        )
        .unwrap();

        // Create team beta
        let beta_dir = dir.path().join("beta");
        std::fs::create_dir_all(&beta_dir).unwrap();
        std::fs::write(
            beta_dir.join("config.json"),
            r#"{"members": []}"#,
        )
        .unwrap();

        let teams = list_teams(dir.path()).unwrap();
        assert_eq!(teams.len(), 2);
        assert_eq!(teams[0].name, "alpha");
        assert_eq!(teams[0].members.len(), 2);
        assert_eq!(teams[0].members[0].name, "lead");
        assert_eq!(teams[1].name, "beta");
        assert!(teams[1].members.is_empty());
    }

    #[test]
    fn list_teams_skips_invalid_config() {
        let dir = TempDir::new().unwrap();

        // Valid team
        let good_dir = dir.path().join("good");
        std::fs::create_dir_all(&good_dir).unwrap();
        std::fs::write(
            good_dir.join("config.json"),
            r#"{"members": [{"name": "a", "agentId": "1", "agentType": "Bash"}]}"#,
        )
        .unwrap();

        // Invalid config (not valid JSON)
        let bad_dir = dir.path().join("bad");
        std::fs::create_dir_all(&bad_dir).unwrap();
        std::fs::write(bad_dir.join("config.json"), "not json").unwrap();

        // Directory without config.json
        let noconfig_dir = dir.path().join("noconfig");
        std::fs::create_dir_all(&noconfig_dir).unwrap();

        let teams = list_teams(dir.path()).unwrap();
        assert_eq!(teams.len(), 1);
        assert_eq!(teams[0].name, "good");
    }

    #[test]
    fn team_config_serialization() {
        let config = TeamConfig {
            name: "test".to_string(),
            members: vec![TeamMember {
                name: "worker".to_string(),
                agent_id: "abc".to_string(),
                agent_type: "Bash".to_string(),
            }],
        };

        let json = serde_json::to_string(&config).unwrap();
        let back: TeamConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.members.len(), 1);
        assert_eq!(back.members[0].name, "worker");
    }
}
