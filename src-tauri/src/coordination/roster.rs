//! Authoritative team roster + runtime attachment join.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::coordination::domain::{DeliveryLease, HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::{RuntimeSession, SessionGroupKind, SessionState};
use crate::templates::types::BehavioralContract;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamMemberActivityState {
    Active,
    Idle,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamMemberView {
    pub team_name: String,
    pub member_name: String,
    pub role: MemberRole,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
    pub focus_area: Option<String>,
    pub context_summary: Option<String>,
    pub behavior_summary: Option<String>,
    pub instructions: Option<String>,
    pub behavioral_contract: Option<BehavioralContract>,
    pub capabilities: Option<Vec<String>>,
    pub configured_cli_tool: CliTool,
    pub configured_project_path: PathBuf,
    pub session_id: Option<String>,
    pub pane_id: Option<String>,
    pub jsonl_path: Option<PathBuf>,
    pub attached_cli_tool: Option<CliTool>,
    pub attached_project_path: Option<PathBuf>,
    pub daemon_pid: Option<u32>,
    pub attached_health: Option<HealthState>,
    pub delivery_lease: Option<DeliveryLease>,
    pub attached_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub activity_state: Option<TeamMemberActivityState>,
    pub has_runtime_record: bool,
}

impl TeamMemberView {
    pub fn configured_member(&self) -> Member {
        Member {
            name: self.member_name.clone(),
            role: self.role,
            role_id: self.role_id.clone(),
            role_name: self.role_name.clone(),
            focus_area: self.focus_area.clone(),
            context_summary: self.context_summary.clone(),
            behavior_summary: self.behavior_summary.clone(),
            instructions: self.instructions.clone(),
            behavioral_contract: self.behavioral_contract.clone(),
            capabilities: self.capabilities.clone(),
            project_path: self.configured_project_path.clone(),
            cli_tool: self.configured_cli_tool,
        }
    }

    pub fn runtime_record(&self) -> Option<MemberRuntimeRecord> {
        if !self.has_runtime_record {
            return None;
        }

        Some(MemberRuntimeRecord {
            schema_version: 3,
            member_name: self.member_name.clone(),
            cli_tool: self.attached_cli_tool,
            project_path: self.attached_project_path.clone(),
            pane_id: self.pane_id.clone(),
            session_id: self.session_id.clone(),
            jsonl_path: self.jsonl_path.clone(),
            daemon_pid: self.daemon_pid,
            health: self.attached_health.unwrap_or(HealthState::SessionDead),
            delivery_lease: self.delivery_lease.clone(),
            attached_at: self.attached_at,
            last_seen_at: self.last_seen_at,
        })
    }

    pub fn latest_runtime_activity(&self) -> Option<DateTime<Utc>> {
        self.last_seen_at.or(self.attached_at)
    }
}

pub fn get_team_roster_with_attachments(
    teams_dir: &Path,
    team_name: &str,
) -> Result<Vec<TeamMemberView>, CoordinationError> {
    get_team_roster_with_attachments_and_activity(teams_dir, team_name, None)
}

pub fn get_team_roster_with_attachments_and_activity(
    teams_dir: &Path,
    team_name: &str,
    activity_by_member: Option<&HashMap<String, TeamMemberActivityState>>,
) -> Result<Vec<TeamMemberView>, CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    let runtime_by_member = MemberRuntimeStore::load_all(teams_dir, team_name)?
        .into_iter()
        .collect::<HashMap<_, _>>();

    Ok(config
        .members
        .into_iter()
        .map(|member| {
            let runtime = runtime_by_member.get(&member.name).cloned();
            let activity_state =
                activity_by_member.and_then(|activity| activity.get(&member.name).copied());
            build_team_member_view(team_name, member, runtime, activity_state)
        })
        .collect())
}

pub fn get_team_roster_with_runtime_sessions(
    teams_dir: &Path,
    team_name: &str,
    runtime_sessions: &[RuntimeSession],
) -> Result<Vec<TeamMemberView>, CoordinationError> {
    let config = TeamConfigStore::load(teams_dir, team_name)?;
    let runtime_by_member = best_runtime_sessions_by_member(team_name, runtime_sessions)
        .into_iter()
        .map(|(member_name, session)| (member_name, member_runtime_record_from_session(session)))
        .collect::<HashMap<_, _>>();

    Ok(config
        .members
        .into_iter()
        .map(|member| {
            let runtime = runtime_by_member.get(&member.name).cloned();
            build_team_member_view(team_name, member, runtime, None)
        })
        .collect())
}

fn build_team_member_view(
    team_name: &str,
    member: Member,
    runtime: Option<MemberRuntimeRecord>,
    activity_state: Option<TeamMemberActivityState>,
) -> TeamMemberView {
    let has_runtime_record = runtime.is_some();

    TeamMemberView {
        team_name: team_name.to_string(),
        member_name: member.name,
        role: member.role,
        role_id: member.role_id,
        role_name: member.role_name,
        focus_area: member.focus_area,
        context_summary: member.context_summary,
        behavior_summary: member.behavior_summary,
        instructions: member.instructions,
        behavioral_contract: member.behavioral_contract,
        capabilities: member.capabilities,
        configured_cli_tool: member.cli_tool,
        configured_project_path: member.project_path,
        session_id: runtime
            .as_ref()
            .and_then(|record| record.session_id.clone()),
        pane_id: runtime.as_ref().and_then(|record| record.pane_id.clone()),
        jsonl_path: runtime
            .as_ref()
            .and_then(|record| record.jsonl_path.clone()),
        attached_cli_tool: runtime.as_ref().and_then(|record| record.cli_tool),
        attached_project_path: runtime
            .as_ref()
            .and_then(|record| record.project_path.clone()),
        daemon_pid: runtime.as_ref().and_then(|record| record.daemon_pid),
        attached_health: runtime.as_ref().map(|record| record.health),
        delivery_lease: runtime
            .as_ref()
            .and_then(|record| record.delivery_lease.clone()),
        attached_at: runtime.as_ref().and_then(|record| record.attached_at),
        last_seen_at: runtime.as_ref().and_then(|record| record.last_seen_at),
        activity_state,
        has_runtime_record,
    }
}

fn best_runtime_sessions_by_member<'a>(
    team_name: &str,
    sessions: &'a [RuntimeSession],
) -> HashMap<String, &'a RuntimeSession> {
    let mut by_member: HashMap<String, &'a RuntimeSession> = HashMap::new();
    for session in sessions {
        if session.group_kind != SessionGroupKind::MeshTeam {
            continue;
        }
        if session.group_id.as_deref() != Some(team_name) {
            continue;
        }
        let Some(member_name) = session.member_name.clone() else {
            continue;
        };
        match by_member.get(&member_name) {
            Some(existing) if preferred_runtime_session(existing, session) => {}
            _ => {
                by_member.insert(member_name, session);
            }
        }
    }
    by_member
}

fn preferred_runtime_session(existing: &RuntimeSession, candidate: &RuntimeSession) -> bool {
    if existing.state == SessionState::Active && candidate.state != SessionState::Active {
        return true;
    }
    existing.tmux_pane.is_some() && candidate.tmux_pane.is_none()
}

fn member_runtime_record_from_session(session: &RuntimeSession) -> MemberRuntimeRecord {
    MemberRuntimeRecord {
        schema_version: 3,
        member_name: session.member_name.clone().unwrap_or_default(),
        cli_tool: Some(session.cli_tool),
        project_path: Some(PathBuf::from(session.project_path.clone())),
        pane_id: session.tmux_pane.clone(),
        session_id: session.session_id.clone(),
        jsonl_path: session.jsonl_path.as_ref().map(PathBuf::from),
        daemon_pid: None,
        health: match session.state {
            SessionState::Active => HealthState::Healthy,
            SessionState::Idle => HealthState::Suppressed,
        },
        delivery_lease: None,
        attached_at: None,
        last_seen_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::coordination::domain::MemberRole;
    use crate::coordination::stores::TeamConfig;

    fn ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_member(member_name: &str) -> Member {
        Member {
            name: member_name.to_string(),
            role: MemberRole::Agent,
            role_id: Some("codex-reviewer".to_string()),
            role_name: Some("Codex Reviewer".to_string()),
            focus_area: Some("Code review".to_string()),
            context_summary: Some("Retains review context".to_string()),
            behavior_summary: Some("Flags concrete issues".to_string()),
            instructions: Some("Review current branch".to_string()),
            behavioral_contract: Some(BehavioralContract {
                communication: vec!["be concise".to_string()],
                execution: vec!["review before patch".to_string()],
                escalation: vec!["raise ambiguity".to_string()],
            }),
            capabilities: Some(vec!["review".to_string()]),
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: CliTool::Codex,
        }
    }

    fn save_team(teams_dir: &Path, team_name: &str, members: Vec<Member>) {
        TeamConfigStore::save(
            teams_dir,
            team_name,
            &TeamConfig {
                schema_version: 1,
                name: team_name.to_string(),
                description: Some("team".to_string()),
                created_at: ts("2026-03-08T21:00:00Z"),
                members,
            },
        )
        .expect("save team");
    }

    fn save_runtime(teams_dir: &Path, team_name: &str, member_name: &str, health: HealthState) {
        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            member_name,
            &MemberRuntimeRecord {
                schema_version: 3,
                member_name: member_name.to_string(),
                cli_tool: Some(CliTool::Codex),
                project_path: Some(PathBuf::from("/tmp/taurhaus")),
                pane_id: Some("%17".to_string()),
                session_id: Some("sess-123".to_string()),
                jsonl_path: Some(PathBuf::from("/tmp/taurhaus/.codex/session.jsonl")),
                daemon_pid: Some(4242),
                health,
                delivery_lease: None,
                attached_at: Some(ts("2026-03-08T21:01:00Z")),
                last_seen_at: Some(ts("2026-03-08T21:02:00Z")),
            },
        )
        .expect("save runtime");
    }

    #[test]
    fn roster_view_returns_complete_data_when_config_and_runtime_exist() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);
        save_runtime(tmp.path(), "team-a", "developer1", HealthState::Healthy);

        let roster = get_team_roster_with_attachments(tmp.path(), "team-a").expect("load roster");

        assert_eq!(roster.len(), 1);
        let member = &roster[0];
        assert_eq!(member.member_name, "developer1");
        assert_eq!(member.configured_cli_tool, CliTool::Codex);
        assert_eq!(
            member.configured_project_path,
            PathBuf::from("/tmp/taurhaus")
        );
        assert_eq!(member.pane_id.as_deref(), Some("%17"));
        assert_eq!(member.session_id.as_deref(), Some("sess-123"));
        assert_eq!(
            member.jsonl_path.as_deref(),
            Some(Path::new("/tmp/taurhaus/.codex/session.jsonl"))
        );
        assert_eq!(member.attached_health, Some(HealthState::Healthy));
        assert!(member.has_runtime_record);
        assert_eq!(
            member.runtime_record().expect("runtime").jsonl_path,
            Some(PathBuf::from("/tmp/taurhaus/.codex/session.jsonl"))
        );
    }

    #[test]
    fn roster_view_handles_missing_runtime_record_gracefully() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);

        let roster = get_team_roster_with_attachments(tmp.path(), "team-a").expect("load roster");

        assert_eq!(roster.len(), 1);
        let member = &roster[0];
        assert_eq!(member.member_name, "developer1");
        assert!(!member.has_runtime_record);
        assert_eq!(member.pane_id, None);
        assert_eq!(member.session_id, None);
        assert_eq!(member.jsonl_path, None);
        assert_eq!(member.runtime_record(), None);
    }

    #[test]
    fn roster_view_preserves_stale_runtime_record() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);
        save_runtime(tmp.path(), "team-a", "developer1", HealthState::SessionDead);

        let roster = get_team_roster_with_attachments(tmp.path(), "team-a").expect("load roster");

        assert_eq!(roster.len(), 1);
        let member = &roster[0];
        assert_eq!(member.attached_health, Some(HealthState::SessionDead));
        assert!(member.has_runtime_record);
        assert_eq!(
            member.runtime_record().expect("runtime").health,
            HealthState::SessionDead
        );
    }

    #[test]
    fn roster_view_layers_optional_activity_state() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);

        let activity =
            HashMap::from([(String::from("developer1"), TeamMemberActivityState::Active)]);
        let roster =
            get_team_roster_with_attachments_and_activity(tmp.path(), "team-a", Some(&activity))
                .expect("load roster");

        assert_eq!(
            roster[0].activity_state,
            Some(TeamMemberActivityState::Active)
        );
    }
}
