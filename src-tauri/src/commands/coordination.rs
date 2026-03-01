//! Coordination IPC commands for team management (M0 surface).

use serde::{Deserialize, Serialize};

/// Lightweight team list entry returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub team_name: String,
}

/// Team status payload returned to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamStatus {
    pub team_name: String,
    pub members: Vec<String>,
}

#[tauri::command]
pub fn coordination_create_team(team_name: String) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    Err("coordination not initialized".to_string())
}

#[tauri::command]
pub fn coordination_disband_team(team_name: String) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    Err("coordination not initialized".to_string())
}

#[tauri::command]
pub fn coordination_add_member(
    team_name: String,
    member_name: String,
    backend_kind: String,
) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    validate_non_empty("backend_kind", &backend_kind)?;
    Err("coordination not initialized".to_string())
}

#[tauri::command]
pub fn coordination_remove_member(team_name: String, member_name: String) -> Result<(), String> {
    validate_non_empty("team_name", &team_name)?;
    validate_non_empty("member_name", &member_name)?;
    Err("coordination not initialized".to_string())
}

#[tauri::command]
pub fn coordination_list_teams() -> Result<Vec<TeamSummary>, String> {
    Err("coordination not initialized".to_string())
}

#[tauri::command]
pub fn coordination_get_team_status(team_name: String) -> Result<TeamStatus, String> {
    validate_non_empty("team_name", &team_name)?;
    Err("coordination not initialized".to_string())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_summary_serialization_round_trip() {
        let value = TeamSummary {
            team_name: "architecture-final".to_string(),
        };
        let json = serde_json::to_string(&value).expect("serialize team summary");
        let decoded: TeamSummary = serde_json::from_str(&json).expect("deserialize team summary");
        assert_eq!(decoded, value);
    }

    #[test]
    fn team_status_serialization_round_trip() {
        let value = TeamStatus {
            team_name: "architecture-final".to_string(),
            members: vec!["team-lead".to_string(), "codex-reviewer".to_string()],
        };
        let json = serde_json::to_string(&value).expect("serialize team status");
        let decoded: TeamStatus = serde_json::from_str(&json).expect("deserialize team status");
        assert_eq!(decoded, value);
    }

    #[test]
    fn create_team_rejects_empty_or_whitespace_name() {
        let err = coordination_create_team("".to_string()).expect_err("empty should fail");
        assert!(err.contains("team_name"));

        let err =
            coordination_create_team("   \n\t  ".to_string()).expect_err("whitespace should fail");
        assert!(err.contains("team_name"));
    }

    #[test]
    fn member_commands_validate_all_required_fields() {
        let err = coordination_disband_team("   ".to_string()).expect_err("blank team should fail");
        assert!(err.contains("team_name"));

        let err = coordination_add_member("team".to_string(), "".to_string(), "mesh".to_string())
            .expect_err("empty member should fail");
        assert!(err.contains("member_name"));

        let err = coordination_add_member("team".to_string(), "alice".to_string(), "".to_string())
            .expect_err("empty backend should fail");
        assert!(err.contains("backend_kind"));

        let err = coordination_remove_member("".to_string(), "alice".to_string())
            .expect_err("empty team should fail");
        assert!(err.contains("team_name"));

        let err = coordination_remove_member("team".to_string(), "  ".to_string())
            .expect_err("whitespace member should fail");
        assert!(err.contains("member_name"));
    }

    #[test]
    fn get_team_status_validates_non_empty_team_name() {
        let err = coordination_get_team_status(" ".to_string()).expect_err("whitespace invalid");
        assert!(err.contains("team_name"));
    }
}
