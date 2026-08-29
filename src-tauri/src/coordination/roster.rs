//! Authoritative team roster + runtime attachment join.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::coordination::domain::{DeliveryLease, HealthState, Member, MemberRole};
use crate::coordination::errors::CoordinationError;
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfigStore};
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::{RuntimeSession, SessionGroupKind, SessionState};
use crate::templates::types::{BehavioralContract, RuntimeCompactSummary};

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
    pub communication_style: Option<String>,
    pub runtime_compact_summary: Option<RuntimeCompactSummary>,
    pub instructions: Option<String>,
    pub behavioral_contract: Option<BehavioralContract>,
    pub quality_gates: Option<Vec<String>>,
    pub handoff_expectations: Option<Vec<String>>,
    pub definition_of_done: Option<Vec<String>>,
    pub phase_scope: Option<Vec<String>>,
    pub mode: Option<String>,
    pub inherits_from: Option<String>,
    pub required_artifacts: Option<Vec<String>>,
    pub capabilities: Option<Vec<String>>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub configured_cli_tool: CliTool,
    pub configured_project_path: PathBuf,
    pub extra: BTreeMap<String, Value>,
    pub session_id: Option<String>,
    pub pane_id: Option<String>,
    pub pane_pid: Option<u32>,
    pub pane_start_time: Option<u64>,
    pub jsonl_path: Option<PathBuf>,
    pub attached_cli_tool: Option<CliTool>,
    pub attached_project_path: Option<PathBuf>,
    pub daemon_pid: Option<u32>,
    pub attached_health: Option<HealthState>,
    pub delivery_lease: Option<DeliveryLease>,
    pub attached_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    /// The workflow hint the daemon computed for this member's session, when
    /// the runtime snapshot carried one.
    ///
    /// It is derived where the transcript is readable, which on Windows is the
    /// WSL daemon and never the desktop process, so it travels with the join
    /// rather than being re-derived downstream.
    pub workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
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
            communication_style: self.communication_style.clone(),
            runtime_compact_summary: self.runtime_compact_summary.clone(),
            instructions: self.instructions.clone(),
            behavioral_contract: self.behavioral_contract.clone(),
            quality_gates: self.quality_gates.clone(),
            handoff_expectations: self.handoff_expectations.clone(),
            definition_of_done: self.definition_of_done.clone(),
            phase_scope: self.phase_scope.clone(),
            mode: self.mode.clone(),
            inherits_from: self.inherits_from.clone(),
            required_artifacts: self.required_artifacts.clone(),
            capabilities: self.capabilities.clone(),
            model: self.model.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
            project_path: self.configured_project_path.clone(),
            cli_tool: self.configured_cli_tool,
            extra: self.extra.clone(),
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
            pane_pid: self.pane_pid,
            pane_start_time: self.pane_start_time,
            session_id: self.session_id.clone(),
            jsonl_path: self.jsonl_path.clone(),
            daemon_pid: self.daemon_pid,
            health: self.attached_health.unwrap_or(HealthState::SessionDead),
            delivery_lease: self.delivery_lease.clone(),
            attached_at: self.attached_at,
            last_seen_at: self.last_seen_at,
            applied_effort: None,
            launch_effort: None,
            effort_default: None,
            effort_resume_failure: None,
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
            build_team_member_view(team_name, member, runtime, None, activity_state)
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
        .map(|(member_name, session)| {
            (
                member_name,
                (
                    member_runtime_record_from_session(session),
                    session.workflow_activity.clone(),
                ),
            )
        })
        .collect::<HashMap<_, _>>();

    Ok(config
        .members
        .into_iter()
        .map(|member| {
            let (runtime, workflow_activity) = runtime_by_member.get(&member.name).cloned().unzip();
            build_team_member_view(
                team_name,
                member,
                runtime,
                workflow_activity.flatten(),
                None,
            )
        })
        .collect())
}

fn build_team_member_view(
    team_name: &str,
    member: Member,
    runtime: Option<MemberRuntimeRecord>,
    workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
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
        communication_style: member.communication_style,
        runtime_compact_summary: member.runtime_compact_summary,
        instructions: member.instructions,
        behavioral_contract: member.behavioral_contract,
        quality_gates: member.quality_gates,
        handoff_expectations: member.handoff_expectations,
        definition_of_done: member.definition_of_done,
        phase_scope: member.phase_scope,
        mode: member.mode,
        inherits_from: member.inherits_from,
        required_artifacts: member.required_artifacts,
        capabilities: member.capabilities,
        model: member.model,
        reasoning_effort: member.reasoning_effort,
        configured_cli_tool: member.cli_tool,
        configured_project_path: member.project_path,
        extra: member.extra,
        session_id: runtime
            .as_ref()
            .and_then(|record| record.session_id.clone()),
        pane_id: runtime.as_ref().and_then(|record| record.pane_id.clone()),
        pane_pid: runtime.as_ref().and_then(|record| record.pane_pid),
        pane_start_time: runtime.as_ref().and_then(|record| record.pane_start_time),
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
        workflow_activity,
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
        pane_pid: None,
        pane_start_time: None,
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
        applied_effort: None,
        launch_effort: None,
        effort_default: None,
        effort_resume_failure: None,
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
            communication_style: None,
            runtime_compact_summary: None,
            instructions: Some("Review current branch".to_string()),
            behavioral_contract: Some(BehavioralContract {
                communication: vec!["be concise".to_string()],
                execution: vec!["review before patch".to_string()],
                escalation: vec!["raise ambiguity".to_string()],
            }),
            quality_gates: None,
            handoff_expectations: None,
            definition_of_done: None,
            phase_scope: None,
            mode: None,
            inherits_from: None,
            required_artifacts: None,
            capabilities: Some(vec!["review".to_string()]),
            model: None,
            reasoning_effort: None,
            project_path: PathBuf::from("/tmp/taurhaus"),
            cli_tool: CliTool::Codex,
            extra: Default::default(),
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
                extra: Default::default(),
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
                pane_pid: None,
                pane_start_time: None,
                session_id: Some("sess-123".to_string()),
                jsonl_path: Some(PathBuf::from("/tmp/taurhaus/.codex/session.jsonl")),
                daemon_pid: Some(4242),
                health,
                delivery_lease: None,
                attached_at: Some(ts("2026-03-08T21:01:00Z")),
                last_seen_at: Some(ts("2026-03-08T21:02:00Z")),
                applied_effort: None,
                launch_effort: None,
                effort_default: None,
                effort_resume_failure: None,
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

    fn workflow_activity(live_runs: u32) -> crate::workflow_runs::WorkflowActivity {
        crate::workflow_runs::WorkflowActivity {
            live_runs,
            last_write_at: 1_772_000_000_000,
        }
    }

    fn daemon_runtime_session(
        team_name: &str,
        member_name: &str,
        jsonl_path: &str,
        workflow_activity: Option<crate::workflow_runs::WorkflowActivity>,
    ) -> RuntimeSession {
        RuntimeSession {
            pid: 4242,
            project_path: "/tmp/taurhaus".to_string(),
            tty: "/dev/pts/3".to_string(),
            args: "claude".to_string(),
            cli_tool: CliTool::Claude,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: None,
            tmux_pane: Some("%17".to_string()),
            tmux_window_name: None,
            state: SessionState::Active,
            session_id: Some("sess-123".to_string()),
            jsonl_path: Some(jsonl_path.to_string()),
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: Default::default(),
            activity_attribution: Default::default(),
            project_unattributed_active: false,
            group_kind: SessionGroupKind::MeshTeam,
            group_id: Some(team_name.to_string()),
            group_label: None,
            member_name: Some(member_name.to_string()),
            workflow_activity,
        }
    }

    // Regression: acefb7a read a member's workflow hint in the desktop process
    // by rescanning the transcript its runtime record names. The daemon has
    // already computed that hint and ships it on the runtime session, and on
    // Windows the transcript it names is a WSL path the desktop cannot open --
    // so a member driving a live run never showed Working beside its run tree.
    // The join has to carry the daemon's value instead of dropping it.
    #[test]
    fn runtime_session_roster_carries_the_daemon_workflow_activity() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);
        let sessions = vec![daemon_runtime_session(
            "team-a",
            "developer1",
            // The daemon's own path: readable in WSL, absent on the desktop host.
            "/home/daemon-host/.claude/projects/-tmp-taurhaus/sess-123.jsonl",
            Some(workflow_activity(2)),
        )];

        let roster = get_team_roster_with_runtime_sessions(tmp.path(), "team-a", &sessions)
            .expect("load roster");

        assert_eq!(roster.len(), 1);
        assert_eq!(roster[0].workflow_activity, Some(workflow_activity(2)));
        assert_eq!(
            roster[0].jsonl_path.as_deref(),
            Some(Path::new(
                "/home/daemon-host/.claude/projects/-tmp-taurhaus/sess-123.jsonl"
            ))
        );
    }

    #[test]
    fn runtime_session_roster_leaves_workflow_activity_unset_when_the_daemon_reports_none() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);
        let sessions = vec![daemon_runtime_session(
            "team-a",
            "developer1",
            "/home/daemon-host/.claude/projects/-tmp-taurhaus/sess-123.jsonl",
            None,
        )];

        let roster = get_team_roster_with_runtime_sessions(tmp.path(), "team-a", &sessions)
            .expect("load roster");

        assert_eq!(roster[0].workflow_activity, None);
    }

    #[test]
    fn attachment_roster_has_no_daemon_workflow_activity() {
        let tmp = TempDir::new().expect("tempdir");
        save_team(tmp.path(), "team-a", vec![sample_member("developer1")]);
        save_runtime(tmp.path(), "team-a", "developer1", HealthState::Healthy);

        let roster = get_team_roster_with_attachments(tmp.path(), "team-a").expect("load roster");

        assert_eq!(roster[0].workflow_activity, None);
    }
}
