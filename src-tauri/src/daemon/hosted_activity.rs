//! Host publication shares the hub lock with scan commits: a scan cannot overwrite a newer edge.
use super::{HubState, SessionActivityHub};
use crate::coordination::stores::MemberRuntimeRecord;
use crate::session_scanner::process::ProcessInfo;
use crate::session_scanner::{RuntimeSession, SessionGroupKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) struct HostedEntry {
    pub session: RuntimeSession,
    pub account_root: PathBuf,
    pub refresh: Arc<dyn Fn() + Send + Sync>,
}

pub(crate) struct HostedActivityLease {
    hub: Arc<SessionActivityHub>,
    socket: PathBuf,
}
impl Drop for HostedActivityLease {
    fn drop(&mut self) {
        let mut state = self.hub.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.hosted.remove(&self.socket) {
            state
                .runtime_sessions
                .retain(|s| !same_seat(s, &entry.session));
            state.display_sessions.retain(|s| {
                !(s.project_path == entry.session.project_path
                    && s.group_id == entry.session.group_id
                    && s.member_name == entry.session.member_name)
            });
            state.version = state.version.saturating_add(1);
            self.hub.changed_cv.notify_all();
        }
    }
}
fn same_seat(a: &RuntimeSession, b: &RuntimeSession) -> bool {
    a.project_path == b.project_path && a.group_id == b.group_id && a.member_name == b.member_name
}

pub(super) fn overlay_hosted(state: &mut HubState, sessions: &mut Vec<RuntimeSession>) {
    for session in sessions.iter_mut() {
        if session.source.as_deref() == Some("host")
            && !state
                .hosted
                .values()
                .any(|e| same_seat(session, &e.session))
        {
            crate::session_scanner::classify_host_activity(session, &serde_json::Value::Null);
        }
    }
    for entry in state.hosted.values_mut() {
        if let Some(scanned) = sessions.iter().find(|s| same_seat(s, &entry.session)) {
            entry.session.jsonl_path = scanned.jsonl_path.clone();
            entry.session.tmux_session = scanned.tmux_session.clone();
            entry.session.tmux_window = scanned.tmux_window.clone();
            entry.session.tmux_window_name = scanned.tmux_window_name.clone();
        }
        sessions.retain(|s| !same_seat(s, &entry.session));
        sessions.push(entry.session.clone());
    }
}

pub(crate) fn remote_session(
    process: &ProcessInfo,
    pane: Option<&str>,
    thread: &str,
) -> RuntimeSession {
    RuntimeSession {
        pid: process.pid,
        project_path: process.project_path.clone(),
        tty: process.tty.clone(),
        args: process.args.clone(),
        cli_tool: process.cli_tool,
        tmux_pane: pane.map(str::to_owned),
        session_id: Some(thread.into()),
        source: Some("host_unavailable".into()),
        ..Default::default()
    }
}

impl SessionActivityHub {
    pub(crate) fn register_host(
        self: &Arc<Self>,
        record: &MemberRuntimeRecord,
        team: &str,
        refresh: Arc<dyn Fn() + Send + Sync>,
    ) -> HostedActivityLease {
        let host = record.app_server.as_ref().expect("owned host attachment");
        let session = RuntimeSession {
            project_path: record
                .project_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            args: host.attach_argv.join(" "),
            cli_tool: record.cli_tool.unwrap_or_default(),
            tmux_pane: record.pane_id.clone(),
            session_id: Some(host.thread_id.clone()),
            source: Some("host_unavailable".into()),
            group_kind: SessionGroupKind::MeshTeam,
            group_id: Some(team.into()),
            group_label: Some(team.into()),
            member_name: Some(record.member_name.clone()),
            ..Default::default()
        };
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hosted
            .insert(
                host.socket_path.clone(),
                HostedEntry {
                    session,
                    account_root: host.account_root.clone(),
                    refresh,
                },
            );
        self.publish_host_status(&host.socket_path, &host.thread_id, &serde_json::Value::Null);
        HostedActivityLease {
            hub: self.clone(),
            socket: host.socket_path.clone(),
        }
    }

    pub(crate) fn publish_host_status(
        &self,
        socket: &Path,
        thread: &str,
        status: &serde_json::Value,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = state
            .hosted
            .get_mut(socket)
            .filter(|e| e.session.session_id.as_deref() == Some(thread))
        else {
            return;
        };
        let before = entry.session.clone();
        crate::session_scanner::classify_host_activity(&mut entry.session, status);
        let session = entry.session.clone();
        if before == session && state.runtime_sessions.contains(&session) {
            return;
        }
        state.runtime_sessions.retain(|s| !same_seat(s, &session));
        state.runtime_sessions.push(session.clone());
        state.display_sessions.retain(|s| {
            !(s.project_path == session.project_path
                && s.group_id == session.group_id
                && s.member_name == session.member_name)
        });
        state.display_sessions.push(session.into());
        state.version = state.version.saturating_add(1);
        self.changed_cv.notify_all();
    }

    pub(crate) fn host_session(
        &self,
        socket: &Path,
        thread: &str,
        process: &ProcessInfo,
        pane: Option<&str>,
    ) -> Option<(RuntimeSession, PathBuf)> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state.hosted.get_mut(socket).filter(|e| {
            e.session.session_id.as_deref() == Some(thread)
                && e.session.project_path == process.project_path
                && e.session
                    .tmux_pane
                    .as_deref()
                    .is_none_or(|expected| Some(expected) == pane)
        })?;
        entry.session.pid = process.pid;
        entry.session.tty = process.tty.clone();
        entry.session.args = process.args.clone();
        entry.session.tmux_pane = pane.map(str::to_owned);
        Some((entry.session.clone(), entry.account_root.clone()))
    }

    pub(crate) fn attach_host_pane(&self, socket: &Path, pane: &str, pid: Option<u32>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.hosted.get_mut(socket) {
            entry.session.tmux_pane = Some(pane.into());
            entry.session.pid = pid.unwrap_or(0);
        }
    }

    pub(crate) fn refresh_hosts(&self) {
        let refreshers: Vec<_> = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hosted
            .values()
            .map(|e| e.refresh.clone())
            .collect();
        for refresh in refreshers {
            refresh();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hosted_teardown_rejects_in_flight_working_scan() {
        // Regression: 6f61f611 had no host authority fence for in-flight scanner results.
        let mut state = HubState::default();
        let mut sessions = vec![RuntimeSession {
            source: Some("host".into()),
            state: crate::session_scanner::SessionState::Active,
            activity_attribution: crate::session_scanner::ActivityAttribution::Attributed,
            ..Default::default()
        }];
        overlay_hosted(&mut state, &mut sessions);
        assert_eq!(sessions[0].source.as_deref(), Some("host_unavailable"));
    }
}
