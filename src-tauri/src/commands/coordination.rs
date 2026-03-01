#![cfg(feature = "mesh-bridged-backend")]

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
}
