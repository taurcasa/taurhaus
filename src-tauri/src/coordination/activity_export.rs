use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::coordination::activity_schema::{
    MemberActivitySnapshot, SnapshotActivityConfidence, ACTIVITY_SNAPSHOT_SCHEMA_VERSION,
};
use crate::coordination::roster::get_team_roster_with_attachments;
use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
use crate::coordination::stores::TeamConfigStore;
use crate::provider::path::normalize_project_path;
use crate::session_scanner::cli_tool::CliTool;
use crate::session_scanner::{
    ActivityAttribution, ActivityConfidence, DisplaySession, RuntimeSession, SessionGroupKind,
    SessionState,
};

#[derive(Debug, Clone)]
struct SessionMembershipMetadata {
    group_id: String,
    group_label: String,
    member_name: String,
    pane_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PaneActivityProbe {
    pane_alive: bool,
    active_non_shell_process: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActivitySnapshotExportStats {
    pub teams_exported: usize,
    pub members_written: usize,
    pub write_failures: usize,
}

pub(crate) fn default_activity_export_teams_dir() -> std::path::PathBuf {
    const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

    if let Some(path) = std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV) {
        if !path.is_empty() {
            return std::path::PathBuf::from(path).join("teams");
        }
    }
    if let Some(path) = crate::coordination::mesh_cli::resolve_windows_mesh_teams_dir() {
        return path;
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir().join("taurhaus-home"))
        .join(".claude")
        .join("teams")
}

pub(crate) fn enrich_sessions_with_team_membership(
    teams_dir: &Path,
    sessions: &mut [DisplaySession],
) {
    if sessions.is_empty() {
        return;
    }

    let memberships = load_session_memberships(teams_dir);
    let mut sessions_by_key: HashMap<(String, CliTool), Vec<usize>> = HashMap::new();

    for (index, session) in sessions.iter_mut().enumerate() {
        session.group_kind = SessionGroupKind::Standalone;
        session.group_id = None;
        session.group_label = None;
        session.member_name = None;

        let key = (
            normalize_project_path(&session.project_path),
            session.cli_tool,
        );
        sessions_by_key.entry(key).or_default().push(index);
    }

    for (key, session_indices) in sessions_by_key {
        let Some(candidates) = memberships.get(&key) else {
            continue;
        };
        assign_session_memberships(sessions, &session_indices, candidates);
    }
}

pub(crate) fn enrich_runtime_sessions_with_team_membership(
    teams_dir: &Path,
    sessions: &mut [RuntimeSession],
) {
    if sessions.is_empty() {
        return;
    }

    let memberships = load_session_memberships(teams_dir);
    let mut sessions_by_key: HashMap<(String, CliTool), Vec<usize>> = HashMap::new();

    for (index, session) in sessions.iter_mut().enumerate() {
        session.group_kind = SessionGroupKind::Standalone;
        session.group_id = None;
        session.group_label = None;
        session.member_name = None;

        let key = (
            normalize_project_path(&session.project_path),
            session.cli_tool,
        );
        sessions_by_key.entry(key).or_default().push(index);
    }

    for (key, session_indices) in sessions_by_key {
        let Some(candidates) = memberships.get(&key) else {
            continue;
        };
        assign_runtime_session_memberships(sessions, &session_indices, candidates);
    }
}

pub(crate) fn export_activity_snapshots_for_sessions(
    teams_dir: &Path,
    sessions: &[DisplaySession],
    observed_at: DateTime<Utc>,
) -> ActivitySnapshotExportStats {
    export_activity_snapshots_for_sessions_with_runtime(
        teams_dir,
        sessions,
        observed_at,
        &SystemCoordinationRuntime,
    )
}

fn export_activity_snapshots_for_sessions_with_runtime(
    teams_dir: &Path,
    sessions: &[DisplaySession],
    observed_at: DateTime<Utc>,
    runtime: &dyn CoordinationRuntime,
) -> ActivitySnapshotExportStats {
    let team_names = match TeamConfigStore::list(teams_dir) {
        Ok(team_names) => team_names,
        Err(error) => {
            tracing::warn!(
                teams_dir = %teams_dir.display(),
                error = %error,
                "failed to list teams for activity snapshot export"
            );
            return ActivitySnapshotExportStats::default();
        }
    };

    if team_names.is_empty() {
        return ActivitySnapshotExportStats::default();
    }

    let mut enriched = sessions.to_vec();
    enrich_sessions_with_team_membership(teams_dir, &mut enriched);
    let sessions_by_member = best_sessions_by_member(&enriched);
    let mut stats = ActivitySnapshotExportStats::default();

    for team_name in team_names {
        let roster = match get_team_roster_with_attachments(teams_dir, &team_name) {
            Ok(roster) => roster,
            Err(error) => {
                tracing::warn!(
                    team_name = %team_name,
                    error = %error,
                    "failed to load team roster for activity snapshot export"
                );
                continue;
            }
        };
        if !roster.iter().any(|member| member.has_runtime_record) {
            continue;
        }

        let expected_members: HashSet<String> = roster
            .iter()
            .map(|member| member.member_name.clone())
            .filter(|name| !name.trim().is_empty())
            .collect();
        if expected_members.is_empty() {
            continue;
        }

        // One existence probe per member; teams without any live pane are
        // skipped entirely (their last snapshot goes stale, which readers
        // already handle) instead of costing four tmux probes per member
        // every refresh.
        let live_panes: Vec<bool> = roster
            .iter()
            .map(|member| {
                member.pane_id.as_deref().is_some_and(|pane_id| {
                    probe_pane_exists(runtime, pane_id, &team_name, &member.member_name)
                })
            })
            .collect();
        if !live_panes.iter().any(|alive| *alive) {
            tracing::debug!(
                team_name = %team_name,
                "skipping activity snapshot export: no live pane"
            );
            continue;
        }

        stats.teams_exported += 1;
        for (member, pane_alive) in roster.iter().zip(live_panes) {
            let member_name = &member.member_name;
            let pane_probe = if pane_alive {
                probe_member_pane_state(
                    runtime,
                    member.pane_id.as_deref().unwrap_or_default(),
                    &team_name,
                    member_name,
                )
            } else {
                PaneActivityProbe::default()
            };
            let snapshot = build_member_activity_snapshot(
                sessions_by_member
                    .get(&(team_name.clone(), member_name.clone()))
                    .copied(),
                &pane_probe,
                observed_at,
            );
            if write_member_activity_snapshot(teams_dir, &team_name, member_name, &snapshot).is_ok()
            {
                stats.members_written += 1;
            } else {
                stats.write_failures += 1;
            }
        }

        cleanup_stale_activity_snapshots(teams_dir, &team_name, &expected_members);
    }

    stats
}

fn best_sessions_by_member<'a>(
    sessions: &'a [DisplaySession],
) -> HashMap<(String, String), &'a DisplaySession> {
    let mut by_member: HashMap<(String, String), &'a DisplaySession> = HashMap::new();
    for session in sessions {
        if session.group_kind != SessionGroupKind::MeshTeam {
            continue;
        }
        let (Some(team_name), Some(member_name)) =
            (session.group_id.clone(), session.member_name.clone())
        else {
            continue;
        };
        let key = (team_name, member_name);
        match by_member.get(&key) {
            Some(existing) if preferred_session(existing, session) => {}
            _ => {
                by_member.insert(key, session);
            }
        }
    }
    by_member
}

fn preferred_session(existing: &DisplaySession, candidate: &DisplaySession) -> bool {
    if existing.state == SessionState::Active && candidate.state != SessionState::Active {
        return true;
    }
    existing.tmux_pane.is_some() && candidate.tmux_pane.is_none()
}

fn load_session_memberships(
    teams_dir: &Path,
) -> HashMap<(String, CliTool), Vec<SessionMembershipMetadata>> {
    let team_names = match TeamConfigStore::list(teams_dir) {
        Ok(team_names) => team_names,
        Err(error) => {
            tracing::warn!(
                teams_dir = %teams_dir.display(),
                error = %error,
                "failed to list team configs while enriching session metadata"
            );
            return HashMap::new();
        }
    };

    let mut memberships: HashMap<_, Vec<SessionMembershipMetadata>> = HashMap::new();

    for team_name in team_names {
        let roster = match get_team_roster_with_attachments(teams_dir, &team_name) {
            Ok(roster) => roster,
            Err(error) => {
                tracing::warn!(
                    team_name = %team_name,
                    error = %error,
                    "failed to load team roster while enriching session metadata"
                );
                continue;
            }
        };

        for member in roster {
            let key = (
                normalize_project_path(&member.configured_project_path.display().to_string()),
                member.configured_cli_tool,
            );

            memberships
                .entry(key)
                .or_default()
                .push(SessionMembershipMetadata {
                    group_id: team_name.clone(),
                    group_label: team_name.clone(),
                    member_name: member.member_name.clone(),
                    pane_id: member.pane_id.clone(),
                });
        }
    }

    memberships
}

fn assign_session_memberships(
    sessions: &mut [DisplaySession],
    session_indices: &[usize],
    candidates: &[SessionMembershipMetadata],
) {
    if session_indices.is_empty() || candidates.is_empty() {
        return;
    }

    let mut unused_candidates = candidates.to_vec();
    let mut assigned = HashMap::new();

    for &index in session_indices {
        let Some(tmux_pane) = sessions[index].tmux_pane.as_deref() else {
            continue;
        };
        let Some(candidate_index) = unused_candidates
            .iter()
            .position(|candidate| candidate.pane_id.as_deref() == Some(tmux_pane))
        else {
            continue;
        };
        assigned.insert(index, unused_candidates.remove(candidate_index));
    }

    let mut fallback_candidates = unused_candidates.into_iter();
    for &index in session_indices {
        let metadata = assigned
            .remove(&index)
            .or_else(|| fallback_candidates.next());
        let Some(metadata) = metadata else {
            continue;
        };

        sessions[index].group_kind = SessionGroupKind::MeshTeam;
        sessions[index].group_id = Some(metadata.group_id);
        sessions[index].group_label = Some(metadata.group_label);
        sessions[index].member_name = Some(metadata.member_name);
    }
}

fn assign_runtime_session_memberships(
    sessions: &mut [RuntimeSession],
    session_indices: &[usize],
    candidates: &[SessionMembershipMetadata],
) {
    if session_indices.is_empty() || candidates.is_empty() {
        return;
    }

    let mut unused_candidates = candidates.to_vec();
    let mut assigned = HashMap::new();

    for &index in session_indices {
        let Some(tmux_pane) = sessions[index].tmux_pane.as_deref() else {
            continue;
        };
        let Some(candidate_index) = unused_candidates
            .iter()
            .position(|candidate| candidate.pane_id.as_deref() == Some(tmux_pane))
        else {
            continue;
        };
        assigned.insert(index, unused_candidates.remove(candidate_index));
    }

    let mut fallback_candidates = unused_candidates.into_iter();
    for &index in session_indices {
        let metadata = assigned
            .remove(&index)
            .or_else(|| fallback_candidates.next());
        let Some(metadata) = metadata else {
            continue;
        };

        sessions[index].group_kind = SessionGroupKind::MeshTeam;
        sessions[index].group_id = Some(metadata.group_id);
        sessions[index].group_label = Some(metadata.group_label);
        sessions[index].member_name = Some(metadata.member_name);
    }
}

fn build_member_activity_snapshot(
    session: Option<&DisplaySession>,
    pane_probe: &PaneActivityProbe,
    observed_at: DateTime<Utc>,
) -> MemberActivitySnapshot {
    let has_active_session = session.is_some_and(|session| session.state == SessionState::Active);
    let has_any_session = session.is_some();
    let recent_io = session.is_some_and(|session| session.recent_io);
    let last_output_age_secs = session.and_then(|session| session.last_output_age_secs);
    let activity_confidence =
        classify_activity_confidence(session, pane_probe, last_output_age_secs);

    MemberActivitySnapshot {
        version: ACTIVITY_SNAPSHOT_SCHEMA_VERSION,
        observed_at: observed_at.to_rfc3339(),
        stall_recent_activity: has_active_session,
        stall_no_output: !has_active_session,
        stall_no_active_process: !has_any_session,
        active_non_shell_process: pane_probe.active_non_shell_process,
        recent_io,
        pane_alive: pane_probe.pane_alive,
        last_output_age_secs,
        activity_confidence,
    }
}

fn classify_activity_confidence(
    session: Option<&DisplaySession>,
    pane_probe: &PaneActivityProbe,
    last_output_age_secs: Option<u64>,
) -> SnapshotActivityConfidence {
    const RECENT_OUTPUT_WINDOW_SECS: u64 = 10;

    if !pane_probe.pane_alive {
        return SnapshotActivityConfidence::Dead;
    }

    if session.is_some_and(|session| session.recent_io) {
        return SnapshotActivityConfidence::Active;
    }

    if pane_probe.active_non_shell_process
        && session.is_some_and(|session| {
            session.state == SessionState::Active
                || (session.activity_confidence != ActivityConfidence::Low
                    && session.activity_attribution == ActivityAttribution::Attributed)
                || last_output_age_secs.is_some_and(|age| age <= RECENT_OUTPUT_WINDOW_SECS)
        })
    {
        return SnapshotActivityConfidence::LikelyWorking;
    }

    if pane_probe.active_non_shell_process
        || session.is_some_and(|session| {
            session.project_unattributed_active
                || (session.state == SessionState::Active
                    && last_output_age_secs.is_some_and(|age| age <= RECENT_OUTPUT_WINDOW_SECS))
        })
    {
        return SnapshotActivityConfidence::Uncertain;
    }

    SnapshotActivityConfidence::Idle
}

fn probe_pane_exists(
    runtime: &dyn CoordinationRuntime,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> bool {
    match runtime.pane_exists(pane_id) {
        Ok(exists) => exists,
        Err(error) => {
            tracing::warn!(
                team_name = %team_name,
                member_name = %member_name,
                pane_id = %pane_id,
                error = %error,
                "failed to probe pane existence during activity snapshot export"
            );
            false
        }
    }
}

/// Probe a pane that is known to exist for liveness and foreground command.
fn probe_member_pane_state(
    runtime: &dyn CoordinationRuntime,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> PaneActivityProbe {
    let pane_is_dead = match runtime.pane_is_dead(pane_id) {
        Ok(is_dead) => is_dead,
        Err(error) => {
            tracing::warn!(
                team_name = %team_name,
                member_name = %member_name,
                pane_id = %pane_id,
                error = %error,
                "failed to probe pane death during activity snapshot export"
            );
            return PaneActivityProbe::default();
        }
    };
    if pane_is_dead {
        return PaneActivityProbe::default();
    }

    let pane_is_shell = match runtime.pane_is_shell(pane_id) {
        Ok(is_shell) => is_shell,
        Err(error) => {
            tracing::warn!(
                team_name = %team_name,
                member_name = %member_name,
                pane_id = %pane_id,
                error = %error,
                "failed to probe pane shell state during activity snapshot export"
            );
            return PaneActivityProbe {
                pane_alive: true,
                active_non_shell_process: false,
            };
        }
    };
    let pane_current_command = match runtime.pane_current_command(pane_id) {
        Ok(command) => command,
        Err(error) => {
            tracing::warn!(
                team_name = %team_name,
                member_name = %member_name,
                pane_id = %pane_id,
                error = %error,
                "failed to probe pane current command during activity snapshot export"
            );
            None
        }
    };

    PaneActivityProbe {
        pane_alive: true,
        active_non_shell_process: !pane_is_shell
            && pane_current_command
                .as_ref()
                .is_some_and(|command| !command.trim().is_empty()),
    }
}

fn write_member_activity_snapshot(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    snapshot: &MemberActivitySnapshot,
) -> Result<(), ()> {
    let dir = activity_snapshot_dir(teams_dir, team_name);
    if let Err(err) = fs::create_dir_all(&dir) {
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            error = %err,
            "failed to create activity snapshot directory"
        );
        return Err(());
    }

    let target_path = activity_snapshot_path(teams_dir, team_name, member_name);
    let tmp_path = activity_snapshot_tmp_path(teams_dir, team_name, member_name);
    let Ok(raw) = serde_json::to_vec_pretty(snapshot) else {
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            "failed to serialize activity snapshot"
        );
        return Err(());
    };

    if let Err(err) = fs::write(&tmp_path, raw) {
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            error = %err,
            "failed to write temporary activity snapshot file"
        );
        return Err(());
    }

    if let Err(rename_err) = fs::rename(&tmp_path, &target_path) {
        #[cfg(target_os = "windows")]
        {
            if target_path.exists() && fs::remove_file(&target_path).is_ok() {
                if fs::rename(&tmp_path, &target_path).is_ok() {
                    return Ok(());
                }
            }
        }
        let _ = fs::remove_file(&tmp_path);
        tracing::warn!(
            team_name = %team_name,
            member_name = %member_name,
            error = %rename_err,
            "failed to atomically replace activity snapshot file"
        );
        return Err(());
    }

    Ok(())
}

fn cleanup_stale_activity_snapshots(
    teams_dir: &Path,
    team_name: &str,
    expected_members: &HashSet<String>,
) {
    let dir = activity_snapshot_dir(teams_dir, team_name);
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("tmp") {
            let _ = fs::remove_file(path);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(member_name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if expected_members.contains(member_name) {
            continue;
        }
        let _ = fs::remove_file(path);
    }
}

fn activity_snapshot_dir(teams_dir: &Path, team_name: &str) -> std::path::PathBuf {
    teams_dir.join(team_name).join("state").join("activity")
}

fn activity_snapshot_path(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> std::path::PathBuf {
    activity_snapshot_dir(teams_dir, team_name).join(format!("{member_name}.json"))
}

fn activity_snapshot_tmp_path(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
) -> std::path::PathBuf {
    activity_snapshot_dir(teams_dir, team_name).join(format!("{member_name}.json.tmp"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use super::*;
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::runtime::{RecordingCoordinationRuntime, RuntimeCall};
    use crate::coordination::stores::config::TeamConfig;
    use crate::coordination::stores::runtime::MemberRuntimeRecord;
    use crate::coordination::stores::MemberRuntimeStore;
    use crate::session_scanner::ActivityAttribution;
    use crate::session_scanner::ActivityConfidence;

    fn ts(raw: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(raw)
            .expect("valid timestamp")
            .with_timezone(&Utc)
    }

    fn sample_team_config(team_name: &str, member_name: &str, project_path: &str) -> TeamConfig {
        TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: ts("2026-03-07T13:30:00+00:00"),
            members: vec![Member {
                name: member_name.to_string(),
                role: MemberRole::Agent,
                role_id: None,
                role_name: None,
                focus_area: None,
                context_summary: None,
                behavior_summary: None,
                communication_style: None,
                runtime_compact_summary: None,
                instructions: None,
                behavioral_contract: None,
                quality_gates: None,
                handoff_expectations: None,
                definition_of_done: None,
                phase_scope: None,
                mode: None,
                inherits_from: None,
                required_artifacts: None,
                capabilities: None,
                model: None,
                reasoning_effort: None,
                project_path: PathBuf::from(project_path),
                cli_tool: CliTool::Codex,
            }],
        }
    }

    fn save_runtime(teams_dir: &Path, team_name: &str, member_name: &str, pane_id: &str) {
        MemberRuntimeStore::save(
            teams_dir,
            team_name,
            member_name,
            &MemberRuntimeRecord {
                schema_version: 3,
                member_name: member_name.to_string(),
                cli_tool: None,
                project_path: None,
                pane_id: Some(pane_id.to_string()),
                session_id: None,
                jsonl_path: None,
                daemon_pid: Some(42),
                health: HealthState::Healthy,
                delivery_lease: None,
                attached_at: Some(ts("2026-03-07T13:31:00+00:00")),
                last_seen_at: Some(ts("2026-03-07T13:31:05+00:00")),
            },
        )
        .expect("runtime saved");
    }

    fn sample_session(project_path: &str, pane_id: &str, state: SessionState) -> DisplaySession {
        DisplaySession {
            pid: 1234,
            project_path: project_path.to_string(),
            tty: "/dev/pts/9".to_string(),
            args: "codex".to_string(),
            cli_tool: CliTool::Codex,
            tmux_session: Some("taurhaus".to_string()),
            tmux_window: Some("1".to_string()),
            tmux_pane: Some(pane_id.to_string()),
            tmux_window_name: Some("project".to_string()),
            state,
            recent_io: false,
            last_output_age_secs: None,
            activity_confidence: ActivityConfidence::High,
            activity_attribution: ActivityAttribution::Attributed,
            project_unattributed_active: false,
            group_kind: SessionGroupKind::Standalone,
            group_id: None,
            group_label: None,
            member_name: None,
        }
    }

    #[test]
    fn exports_snapshot_for_active_runtime_member_from_scanner_sessions() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "team-a";
        let member_name = "developer2";
        let project_path = "/tmp/taurhaus";
        TeamConfigStore::save(
            tmp.path(),
            team_name,
            &sample_team_config(team_name, member_name, project_path),
        )
        .expect("config saved");
        save_runtime(tmp.path(), team_name, member_name, "%12");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%12", true);
        runtime.set_pane_current_command("%12", Some("codex"));
        let stats = export_activity_snapshots_for_sessions_with_runtime(
            tmp.path(),
            &[sample_session(project_path, "%12", SessionState::Active)],
            ts("2026-03-07T13:32:00+00:00"),
            &runtime,
        );

        assert_eq!(stats.teams_exported, 1);
        assert_eq!(stats.members_written, 1);
        assert_eq!(stats.write_failures, 0);

        let raw = fs::read_to_string(activity_snapshot_path(tmp.path(), team_name, member_name))
            .expect("snapshot written");
        let parsed: Value = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(parsed.get("version").and_then(Value::as_u64), Some(1));
        assert_eq!(
            parsed.get("stall_recent_activity").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            parsed.get("stall_no_output").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            parsed
                .get("stall_no_active_process")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            parsed.get("recent_io").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            parsed.get("pane_alive").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            parsed.get("activity_confidence").and_then(Value::as_str),
            Some("likely_working")
        );
    }

    #[test]
    fn exports_snapshot_for_runtime_member_even_without_live_session() {
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "team-a";
        let member_name = "developer2";
        let project_path = "/tmp/taurhaus";
        TeamConfigStore::save(
            tmp.path(),
            team_name,
            &sample_team_config(team_name, member_name, project_path),
        )
        .expect("config saved");
        save_runtime(tmp.path(), team_name, member_name, "%12");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%12", true);
        runtime.set_pane_shell("%12", true);
        let stats = export_activity_snapshots_for_sessions_with_runtime(
            tmp.path(),
            &[],
            ts("2026-03-07T13:33:00+00:00"),
            &runtime,
        );

        assert_eq!(stats.teams_exported, 1);
        assert_eq!(stats.members_written, 1);
        assert_eq!(stats.write_failures, 0);

        let raw = fs::read_to_string(activity_snapshot_path(tmp.path(), team_name, member_name))
            .expect("snapshot written");
        let parsed: Value = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(
            parsed.get("stall_recent_activity").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            parsed.get("stall_no_output").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            parsed
                .get("stall_no_active_process")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            parsed.get("pane_alive").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            parsed.get("activity_confidence").and_then(Value::as_str),
            Some("idle")
        );
    }

    #[test]
    fn skips_team_without_live_pane_after_one_existence_probe_per_member() {
        // Teams that once ran but whose panes are gone used to be exported
        // every refresh (4 tmux probes per member); they are skipped now and
        // their last snapshot simply goes stale for readers.
        let tmp = TempDir::new().expect("tempdir");
        let team_name = "team-dead";
        let member_name = "developer2";
        let project_path = "/tmp/taurhaus";
        TeamConfigStore::save(
            tmp.path(),
            team_name,
            &sample_team_config(team_name, member_name, project_path),
        )
        .expect("config saved");
        save_runtime(tmp.path(), team_name, member_name, "%12");

        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%12", false);
        let stats = export_activity_snapshots_for_sessions_with_runtime(
            tmp.path(),
            &[sample_session(project_path, "%12", SessionState::Active)],
            ts("2026-03-07T13:34:00+00:00"),
            &runtime,
        );

        assert_eq!(stats, ActivitySnapshotExportStats::default());
        assert!(!activity_snapshot_path(tmp.path(), team_name, member_name).exists());
        assert_eq!(
            runtime.calls(),
            vec![RuntimeCall::CheckPaneExists {
                pane_id: "%12".to_string()
            }]
        );
    }

    #[test]
    fn member_snapshot_marks_recent_io_as_active() {
        let mut session = sample_session("/tmp/taurhaus", "%12", SessionState::Active);
        session.recent_io = true;
        session.last_output_age_secs = Some(2);

        let snapshot = build_member_activity_snapshot(
            Some(&session),
            &PaneActivityProbe {
                pane_alive: true,
                active_non_shell_process: true,
            },
            ts("2026-03-07T13:34:00+00:00"),
        );

        assert!(snapshot.stall_recent_activity);
        assert!(!snapshot.stall_no_output);
        assert!(!snapshot.stall_no_active_process);
        assert!(snapshot.active_non_shell_process);
        assert!(snapshot.recent_io);
        assert!(snapshot.pane_alive);
        assert_eq!(snapshot.last_output_age_secs, Some(2));
        assert_eq!(
            snapshot.activity_confidence,
            SnapshotActivityConfidence::Active
        );
    }

    #[test]
    fn member_snapshot_marks_live_shell_without_activity_as_idle() {
        let snapshot = build_member_activity_snapshot(
            None,
            &PaneActivityProbe {
                pane_alive: true,
                active_non_shell_process: false,
            },
            ts("2026-03-07T13:35:00+00:00"),
        );

        assert!(!snapshot.stall_recent_activity);
        assert!(snapshot.stall_no_output);
        assert!(snapshot.stall_no_active_process);
        assert!(!snapshot.active_non_shell_process);
        assert!(!snapshot.recent_io);
        assert!(snapshot.pane_alive);
        assert_eq!(snapshot.last_output_age_secs, None);
        assert_eq!(
            snapshot.activity_confidence,
            SnapshotActivityConfidence::Idle
        );
    }

    #[test]
    fn member_snapshot_marks_missing_pane_as_dead() {
        let snapshot = build_member_activity_snapshot(
            None,
            &PaneActivityProbe::default(),
            ts("2026-03-07T13:36:00+00:00"),
        );

        assert_eq!(
            snapshot.activity_confidence,
            SnapshotActivityConfidence::Dead
        );
        assert!(!snapshot.pane_alive);
    }
}
