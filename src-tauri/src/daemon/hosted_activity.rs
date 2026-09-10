//! Host publication shares the hub lock with scan commits: a scan cannot overwrite a newer edge.
use super::{HubState, SessionActivityHub};
use crate::provider::path::normalize_project_path;
use crate::session_scanner::process::ProcessInfo;
use crate::session_scanner::RuntimeSession;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(super) struct HostedEntry {
    pub session: RuntimeSession,
    pub account_root: PathBuf,
    transcript_attempt: Option<std::time::Instant>,
    pub refresh: Arc<dyn Fn() + Send + Sync>,
}

pub struct HostedActivityLease {
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
    normalize_project_path(&a.project_path) == normalize_project_path(&b.project_path)
        && a.group_id == b.group_id
        && a.member_name == b.member_name
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
        } else {
            entry.session.pid = 0;
            entry.session.tty.clear();
            entry.session.args.clear();
            entry.session.tmux_pane = None;
            entry.session.tmux_session = None;
            entry.session.tmux_window = None;
            entry.session.tmux_window_name = None;
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
    pub fn register_host(
        self: &Arc<Self>,
        socket: PathBuf,
        account_root: PathBuf,
        session: RuntimeSession,
        refresh: Arc<dyn Fn() + Send + Sync>,
    ) -> HostedActivityLease {
        let thread = session.session_id.clone().expect("owned host identity");
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hosted
            .insert(
                socket.clone(),
                HostedEntry {
                    session,
                    account_root,
                    transcript_attempt: None,
                    refresh,
                },
            );
        self.publish_host_status(&socket, &thread, &serde_json::Value::Null);
        HostedActivityLease {
            hub: self.clone(),
            socket,
        }
    }

    pub fn publish_host_status(&self, socket: &Path, thread: &str, status: &serde_json::Value) {
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
    ) -> Option<(RuntimeSession, Option<PathBuf>)> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state.hosted.get_mut(socket).filter(|e| {
            e.session.session_id.as_deref() == Some(thread)
                && normalize_project_path(&e.session.project_path)
                    == normalize_project_path(&process.project_path)
                && e.session
                    .tmux_pane
                    .as_deref()
                    .is_none_or(|expected| Some(expected) == pane)
        })?;
        entry.session.pid = process.pid;
        entry.session.tty = process.tty.clone();
        entry.session.args = process.args.clone();
        entry.session.tmux_pane = pane.map(str::to_owned);
        let needs_transcript = entry
            .session
            .jsonl_path
            .as_deref()
            .is_none_or(|p| !Path::new(p).is_file());
        let account = (needs_transcript
            && entry
                .transcript_attempt
                .is_none_or(|at| at.elapsed() >= std::time::Duration::from_secs(30)))
        .then(|| {
            entry.transcript_attempt = Some(std::time::Instant::now());
            entry.account_root.clone()
        });
        Some((entry.session.clone(), account))
    }

    pub fn attach_host_pane(&self, socket: &Path, pane: &str, pid: Option<u32>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = state.hosted.get_mut(socket) {
            entry.session.tmux_pane = Some(pane.into());
            entry.session.pid = pid.unwrap_or(0);
        }
    }

    pub fn refresh_hosts(&self) {
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
    fn hosted_activity_regression_normalizes_identity_and_overlay() {
        // Regression: 1b19edd2 compared configured paths with kernel cwd verbatim.
        let hub = Arc::new(SessionActivityHub::new());
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("socket");
        let session = RuntimeSession {
            project_path: format!("{}//", tmp.path().display()),
            session_id: Some("thread".into()),
            group_id: Some("team".into()),
            member_name: Some("seat".into()),
            ..Default::default()
        };
        let _lease = hub.register_host(
            socket.clone(),
            tmp.path().into(),
            session.clone(),
            Arc::new(|| {}),
        );
        let process = ProcessInfo {
            pid: 42,
            project_path: tmp.path().to_string_lossy().into_owned(),
            tty: "pts/42".into(),
            args: format!("codex --remote unix://{} resume thread", socket.display()),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Codex,
        };
        let resolved = hub.host_session(&socket, "thread", &process, Some("%42"));
        // Check overlay independently too: enrichment stamps membership on the cwd row.
        let mut sessions = vec![RuntimeSession {
            project_path: process.project_path.clone(),
            pid: process.pid,
            tmux_pane: Some("%42".into()),
            ..session
        }];
        overlay_hosted(&mut hub.state.lock().unwrap(), &mut sessions);
        assert_eq!(sessions.len(), 1, "one seat must not survive as two rows");
        let resolved = resolved
            .expect("normalized cwd must resolve the owned seat")
            .0;
        assert_eq!(resolved.session_id.as_deref(), Some("thread"));
        assert_eq!(sessions[0].pid, process.pid);
        assert_eq!(sessions[0].tmux_pane.as_deref(), Some("%42"));
    }

    #[test]
    fn missing_hosted_transcript_is_not_walked_each_cycle() {
        // Regression: 1b19edd2 walked the account history at scanner frequency after a miss.
        let hub = SessionActivityHub::shared();
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("socket");
        let _lease = hub.register_host(
            socket.clone(),
            tmp.path().into(),
            RuntimeSession {
                session_id: Some("thread".into()),
                project_path: tmp.path().to_string_lossy().into_owned(),
                ..Default::default()
            },
            Arc::new(|| {}),
        );
        let process = ProcessInfo {
            pid: 42,
            project_path: tmp.path().to_string_lossy().into_owned(),
            tty: "pts/42".into(),
            args: format!("codex --remote unix://{} resume thread", socket.display()),
            cli_tool: crate::session_scanner::cli_tool::CliTool::Codex,
        };
        let source = crate::session_scanner::cli_tool::spec(process.cli_tool).session_source();
        assert!(source
            .process_session(&process, None)
            .unwrap()
            .jsonl_path
            .is_none());
        std::fs::create_dir(tmp.path().join("sessions")).unwrap();
        std::fs::write(
            tmp.path().join("sessions/rollout-thread.jsonl"),
            serde_json::json!({"type":"session_meta","payload":{"id":"thread","cwd":tmp.path()}})
                .to_string(),
        )
        .unwrap();
        assert!(source
            .process_session(&process, None)
            .unwrap()
            .jsonl_path
            .is_none());
        hub.state
            .lock()
            .unwrap()
            .hosted
            .get_mut(&socket)
            .unwrap()
            .transcript_attempt =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(31));
        assert!(source
            .process_session(&process, None)
            .unwrap()
            .jsonl_path
            .is_some());
    }

    #[test]
    fn absent_tui_clears_process_and_pane_identity() {
        // Regression: 1b19edd2 replayed a dead TUI pid/pane from the owned entry.
        let hub = Arc::new(SessionActivityHub::new());
        let tmp = tempfile::tempdir().unwrap();
        let _lease = hub.register_host(
            tmp.path().join("socket"),
            tmp.path().into(),
            RuntimeSession {
                session_id: Some("thread".into()),
                pid: 42,
                tty: "pts/42".into(),
                args: "remote".into(),
                tmux_pane: Some("%42".into()),
                ..Default::default()
            },
            Arc::new(|| {}),
        );
        let mut sessions = Vec::new();
        overlay_hosted(&mut hub.state.lock().unwrap(), &mut sessions);
        assert_eq!(sessions[0].pid, 0);
        assert!(sessions[0].tty.is_empty() && sessions[0].args.is_empty());
        assert!(sessions[0].tmux_pane.is_none());
        assert_eq!(sessions[0].session_id.as_deref(), Some("thread"));
    }

    #[test]
    fn orphaned_host_row_is_downgraded_to_unavailable() {
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
