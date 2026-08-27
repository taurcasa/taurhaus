use super::session_listing::CliSessionFreshness;
use super::*;
use crate::commands::logging::{install_global_sink, LogFileState};
use crate::commands::runtime_snapshot::{
    daemon_runtime_session_snapshot, decode_daemon_runtime_session_snapshot,
    RuntimeSnapshotFreshness,
};
use crate::commands::terminal_settings::load_terminal_settings;
use crate::coordination::activity_export::enrich_sessions_with_team_membership;
use crate::coordination::backend::{BackendSelector, CoordinationBackend, FakeBackend};
use crate::coordination::domain::HealthState;
use crate::coordination::domain::{Member, MemberRole};
use crate::coordination::runtime::{
    CoordinationRuntime, RecordingCoordinationRuntime, RuntimeCall,
};
use crate::coordination::state::CoordinationState;
use crate::coordination::stores::{MemberRuntimeRecord, MemberRuntimeStore, TeamConfig};
use crate::session_scanner::launch::base_command;
use crate::session_scanner::tmux::TmuxFocus;
use crate::session_scanner::{DisplaySession, SessionGroupKind, SessionState};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tempfile::NamedTempFile;
use tempfile::TempDir;

struct StubDaemon {
    addr: String,
    last_request: std::sync::Arc<Mutex<Option<protocol::DaemonRequest>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for StubDaemon {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn setup_db_with_project(project_id: &str, project_path: &str) -> (DbState, NamedTempFile) {
    let tmp = NamedTempFile::new().expect("temp db");
    let conn = crate::db::init_db(tmp.path()).expect("init db");

    let now = chrono::Utc::now().to_rfc3339();
    crate::db::queries::insert_project(
        &conn,
        &crate::models::Project {
            id: project_id.to_string(),
            name: "test-project".to_string(),
            path: project_path.to_string(),
            description: None,
            last_activity_at: None,
            hero_preference: None,
            created_at: now.clone(),
            updated_at: now,
            cached_branch: None,
            cached_is_dirty: None,
            claude_account_id: None,
        },
    )
    .expect("insert project");

    (DbState(Mutex::new(conn)), tmp)
}

fn setup_log_file() -> (LogFileState, NamedTempFile) {
    let tmp = NamedTempFile::new().expect("temp log");
    let state = LogFileState::new(tmp.path().to_path_buf()).expect("create log sink");
    (state, tmp)
}

/// The events a test's sink has taken so far.
///
/// Regression: c982822 read the file as soon as *any* line parsed, and the sink
/// is asynchronous — a writer thread owns the file. Under load the reader saw a
/// file that had merely started filling, and the event the test was about was
/// not in it yet: `a_project_pinned_to_a_second_account_launches_with_its_config_dir`
/// panicked on "rendered launch event" in 10 of 40 module runs with every core
/// loaded. `flush_for_test` returns once every record enqueued so far is
/// durable, so one read after it sees everything the call under test emitted.
fn read_log_events(sink: &LogFileState, path: &Path) -> Vec<serde_json::Value> {
    sink.flush_for_test().expect("flush log sink");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn test_coordination_state(
    teams_dir: &Path,
    runtime: Arc<RecordingCoordinationRuntime>,
) -> CoordinationState {
    CoordinationState::with_components_and_runtime(
        teams_dir.to_path_buf(),
        BackendSelector::m0(),
        Arc::new(|_kind, _teams_dir| {
            Ok(Arc::new(FakeBackend::default()) as Arc<dyn CoordinationBackend>)
        }),
        Arc::new(move || runtime.clone() as Arc<dyn CoordinationRuntime>),
    )
}

fn start_stub_daemon(response: serde_json::Value) -> StubDaemon {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub daemon");
    let addr = listener.local_addr().expect("stub daemon addr");
    let addr_string = format!("127.0.0.1:{}", addr.port());
    let request_slot = std::sync::Arc::new(Mutex::new(None));
    let request_slot_clone = request_slot.clone();

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept daemon client");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request");

        let request: protocol::DaemonRequest =
            serde_json::from_str(&line).expect("parse daemon request");
        if let Ok(mut slot) = request_slot_clone.lock() {
            *slot = Some(request.clone());
        }

        let mut resp = response;
        if let Some(map) = resp.as_object_mut() {
            map.insert("id".to_string(), serde_json::Value::String(request.id));
        }

        let mut writer = stream;
        let payload = serde_json::to_string(&resp).expect("serialize daemon response");
        writer
            .write_all(payload.as_bytes())
            .expect("write daemon response");
        writer.write_all(b"\n").expect("write newline");
        writer.flush().expect("flush daemon response");
    });

    StubDaemon {
        addr: addr_string,
        last_request: request_slot,
        handle: Some(handle),
    }
}

fn start_unreachable_stub_daemon() -> StubDaemon {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unreachable daemon");
    let addr = listener.local_addr().expect("unreachable daemon addr");
    let addr_string = format!("127.0.0.1:{}", addr.port());
    let request_slot = std::sync::Arc::new(Mutex::new(None));

    let handle = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("accept daemon client");
        drop(stream);
    });

    StubDaemon {
        addr: addr_string,
        last_request: request_slot,
        handle: Some(handle),
    }
}

#[test]
fn launch_mode_deserializes_valid_values_and_rejects_invalid() {
    for (raw, expected) in [
        ("\"continue\"", LaunchMode::Continue),
        ("\"fresh\"", LaunchMode::Fresh),
        ("\"resume\"", LaunchMode::Resume),
    ] {
        let mode: LaunchMode = serde_json::from_str(raw).unwrap();
        assert_eq!(mode, expected);
    }
    assert!(serde_json::from_str::<LaunchMode>("\"invalid\"").is_err());
}

fn active_session_for(path: &str) -> DisplaySession {
    DisplaySession {
        pid: 1234,
        project_path: path.to_string(),
        tty: "/dev/pts/1".to_string(),
        args: "codex --yolo".to_string(),
        cli_tool: CliTool::Codex,
        tmux_session: Some("taurhaus".to_string()),
        tmux_window: Some("1".to_string()),
        tmux_pane: Some("%1".to_string()),
        tmux_window_name: Some("work".to_string()),
        state: SessionState::Active,
        recent_io: false,
        last_output_age_secs: None,
        activity_confidence: crate::session_scanner::ActivityConfidence::High,
        activity_attribution: crate::session_scanner::ActivityAttribution::Attributed,
        project_unattributed_active: false,
        group_kind: SessionGroupKind::Standalone,
        group_id: None,
        group_label: None,
        member_name: None,
    }
}

fn attached_focus(session_name: &str, window_index: &str) -> TmuxFocus {
    TmuxFocus {
        session: session_name.to_string(),
        window_index: window_index.to_string(),
        pane_id: String::new(),
    }
}

fn save_team_member(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    project_path: &str,
    cli_tool: CliTool,
) {
    TeamConfigStore::save(
        teams_dir,
        team_name,
        &TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: chrono::Utc::now(),
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
                project_path: project_path.into(),
                cli_tool,
                extra: Default::default(),
            }],
            extra: Default::default(),
        },
    )
    .expect("save team config");
}

fn save_team_members(teams_dir: &Path, team_name: &str, members: Vec<Member>) {
    TeamConfigStore::save(
        teams_dir,
        team_name,
        &TeamConfig {
            schema_version: 1,
            name: team_name.to_string(),
            description: None,
            created_at: chrono::Utc::now(),
            members,
            extra: Default::default(),
        },
    )
    .expect("save team config");
}

fn save_member_runtime(teams_dir: &Path, team_name: &str, member_name: &str, pane_id: &str) {
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
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::Healthy,
            delivery_lease: None,
            attached_at: Some(chrono::Utc::now()),
            last_seen_at: Some(chrono::Utc::now()),
        },
    )
    .expect("save runtime record");
}

fn save_member_runtime_record(
    teams_dir: &Path,
    team_name: &str,
    member_name: &str,
    record: MemberRuntimeRecord,
) {
    MemberRuntimeStore::save(teams_dir, team_name, member_name, &record)
        .expect("save runtime record");
}

#[test]
fn promote_activity_from_sessions_touches_dormant_project_once() {
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let sessions = vec![active_session_for("/tmp/project")];

    let promoted = promote_activity_from_sessions_impl(&db, &sessions).expect("promote activity");
    assert_eq!(promoted, 1);

    let promoted_again =
        promote_activity_from_sessions_impl(&db, &sessions).expect("promote activity again");
    assert_eq!(promoted_again, 0);
}

#[test]
fn promote_activity_from_sessions_touches_project_for_unattributed_activity() {
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let mut session = active_session_for("/tmp/project");
    session.state = SessionState::Idle;
    session.project_unattributed_active = true;
    session.activity_attribution = crate::session_scanner::ActivityAttribution::Unattributed;
    session.activity_confidence = crate::session_scanner::ActivityConfidence::Low;

    let promoted =
        promote_activity_from_sessions_impl(&db, &[session]).expect("promote unattributed");

    assert_eq!(promoted, 1);

    let conn = db.0.lock().expect("lock db");
    let thresholds = crate::models::ActivityThresholds::default();
    let detail = crate::services::project::get_project(&conn, "p1", &thresholds)
        .expect("fetch project after unattributed promote");
    assert_eq!(detail.activity_state, crate::models::ActivityState::Recent);
}

#[test]
fn promote_activity_from_sessions_does_not_overpromote_recent_project_for_unattributed_activity() {
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    {
        let conn = db.0.lock().expect("lock db");
        let recent_ts = (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339();
        crate::db::queries::update_project(
            &conn,
            "p1",
            None,
            None,
            None,
            Some(Some(recent_ts.as_str())),
            None,
        )
        .expect("seed recent activity");
    }

    let mut session = active_session_for("/tmp/project");
    session.state = SessionState::Idle;
    session.project_unattributed_active = true;
    session.activity_attribution = crate::session_scanner::ActivityAttribution::Unattributed;
    session.activity_confidence = crate::session_scanner::ActivityConfidence::Low;

    let promoted =
        promote_activity_from_sessions_impl(&db, &[session]).expect("promote unattributed");

    assert_eq!(promoted, 0);

    let conn = db.0.lock().expect("lock db");
    let thresholds = crate::models::ActivityThresholds::default();
    let detail =
        crate::services::project::get_project(&conn, "p1", &thresholds).expect("fetch project");
    assert_eq!(detail.activity_state, crate::models::ActivityState::Recent);
}

// Regression: commits a53ad31 (hook removal) and f9c1e89 (None => remove-all).
// Foreground focus used to arrive through tmux hooks writing a file the app
// read; the app now answers from the daemon hub's own snapshot.
#[test]
fn get_foreground_project_impl_answers_from_the_hub_snapshot() {
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "version": 5,
            "display_sessions": [],
            "runtime_sessions": [],
            "focus": {"session": "taurhaus", "window": "2", "pane_id": "%9"},
            "foreground_project_path": "/tmp/project"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };

    let project_id = get_foreground_project_impl(&db, &provider).expect("resolve foreground");

    assert_eq!(project_id, Some("p1".to_string()));
    let request = daemon.last_request.lock().expect("request slot").clone();
    assert_eq!(
        request.expect("daemon request").method,
        protocol::method::GET_RUNTIME_SESSION_SNAPSHOT
    );
}

#[test]
fn get_foreground_project_impl_returns_none_without_hub_focus() {
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "version": 5,
            "display_sessions": [],
            "runtime_sessions": [],
            "focus": null,
            "foreground_project_path": null
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };

    assert_eq!(
        get_foreground_project_impl(&db, &provider).expect("resolve foreground"),
        None
    );
}

#[test]
fn daemon_runtime_session_snapshot_uses_snapshot_method_and_returns_payload() {
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "version": 3,
            "display_sessions": [],
            "runtime_sessions": [],
            "focus": null,
            "foreground_project_path": "/tmp/project"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let outcome = daemon_runtime_session_snapshot(&provider).expect("snapshot call");
    assert_eq!(outcome.freshness, RuntimeSnapshotFreshness::Fresh);
    let snapshot = outcome.snapshot.expect("connected daemon snapshot");

    assert_eq!(snapshot.version, 3);
    assert_eq!(
        snapshot.foreground_project_path.as_deref(),
        Some("/tmp/project")
    );

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.method,
        protocol::method::GET_RUNTIME_SESSION_SNAPSHOT
    );
}

// Regression: the daemon list path promoted project activity from every
// daemon snapshot. When the WSL daemon's scanner degrades, the hub hands out
// its last good sessions for continuity; those are not an observation and
// the snapshot says so (`degraded`), which the list path must report so
// promotion is skipped — the same rule the local fallback already follows.
#[test]
fn daemon_display_sessions_reports_degraded_snapshot() {
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "version": 3,
            "display_sessions": [serde_json::to_value(active_session_for("/tmp/project")).unwrap()],
            "runtime_sessions": [],
            "focus": null,
            "foreground_project_path": null,
            "degraded": true
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };

    let snapshot = daemon_display_sessions(&provider)
        .expect("snapshot call")
        .expect("connected daemon snapshot");

    assert_eq!(
        snapshot.freshness,
        CliSessionFreshness::Degraded,
        "a degraded daemon snapshot must be reported as such"
    );
    assert_eq!(
        snapshot.sessions,
        vec![active_session_for("/tmp/project")],
        "continuity: the retained sessions still reach the app"
    );
}

// Regression: fa572d4 suspended the app's activity trackers on a degraded
// daemon snapshot, but the fallback polling path — the one the app runs
// precisely when the daemon bridge is down — returned a bare `Vec` and dropped
// both the degradation flag and the fact that the list came out of the on-disk
// cache. Every fallback poll therefore looked like a fresh observation, undid
// the suspension, and credited the outage to whatever state preceded it. The
// polling answer now says how it was obtained.
#[test]
fn cli_session_freshness_says_whether_the_list_was_observed() {
    assert_eq!(
        CliSessionFreshness::classify(RuntimeSnapshotFreshness::Fresh, false),
        CliSessionFreshness::Fresh
    );
    assert_eq!(
        CliSessionFreshness::classify(RuntimeSnapshotFreshness::Fresh, true),
        CliSessionFreshness::Degraded,
        "a reachable daemon replaying its last good sessions is not an observation"
    );
    assert_eq!(
        CliSessionFreshness::classify(RuntimeSnapshotFreshness::Cached, false),
        CliSessionFreshness::Cached,
        "an unreachable daemon's disk cache is older still"
    );
    assert_eq!(
        CliSessionFreshness::classify(RuntimeSnapshotFreshness::Cached, true),
        CliSessionFreshness::Cached
    );
    assert_eq!(
        CliSessionFreshness::classify(RuntimeSnapshotFreshness::Unavailable, false),
        CliSessionFreshness::Unavailable
    );

    // The app branches on these strings; they are part of the IPC contract.
    assert_eq!(
        serde_json::to_value(CliSessionFreshness::Fresh).unwrap(),
        serde_json::json!("fresh")
    );
    assert_eq!(
        serde_json::to_value(CliSessionFreshness::Degraded).unwrap(),
        serde_json::json!("degraded")
    );
    assert_eq!(
        serde_json::to_value(CliSessionFreshness::Cached).unwrap(),
        serde_json::json!("cached")
    );
    assert_eq!(
        serde_json::to_value(CliSessionFreshness::Unavailable).unwrap(),
        serde_json::json!("unavailable")
    );

    // A daemon that is configured but has neither a live snapshot nor a cached
    // one reports no sessions. That empty list is not an empty machine, and the
    // app has to be able to tell them apart — otherwise it retires every
    // tracker and persists the outage as "every session ended".
    let unavailable = CliSessionSnapshot::unavailable();
    assert!(unavailable.sessions.is_empty());
    assert_eq!(unavailable.freshness, CliSessionFreshness::Unavailable);
}

#[test]
fn daemon_session_decode_handles_missing_invalid_and_valid_payloads() {
    assert!(decode_daemon_session_list(None).unwrap().is_empty());
    assert!(
        decode_daemon_session_list(Some(serde_json::json!({"not": "a session list"}))).is_err()
    );
    assert!(decode_daemon_session_list(Some(serde_json::json!([])))
        .unwrap()
        .is_empty());

    let payload = Some(serde_json::json!([
        {"pid": 1234, "project_path": "/tmp/project-a", "tty": "/dev/pts/1", "args": "claude --continue", "cli_tool": "claude", "tmux_session": "taurhaus", "tmux_window": "1", "tmux_pane": "%1", "tmux_window_name": "a", "state": "active"},
        {"pid": 5678, "project_path": "/tmp/project-b", "tty": "/dev/pts/2", "args": "codex --yolo", "cli_tool": "codex", "tmux_session": null, "tmux_window": null, "tmux_pane": null, "tmux_window_name": null, "state": "idle"}
    ]));
    let sessions = decode_daemon_session_list(payload).expect("valid session payload");
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].project_path, "/tmp/project-a");
    assert_eq!(sessions[1].project_path, "/tmp/project-b");
    assert_eq!(sessions[0].group_kind, SessionGroupKind::Standalone);
    assert_eq!(sessions[1].group_kind, SessionGroupKind::Standalone);
}

#[test]
fn daemon_runtime_session_snapshot_decode_handles_missing_invalid_and_valid_payloads() {
    let empty = decode_daemon_runtime_session_snapshot(None).expect("missing payload defaults");
    assert_eq!(empty.version, 0);
    assert!(empty.display_sessions.is_empty());
    assert!(empty.runtime_sessions.is_empty());
    assert!(empty.focus.is_none());
    assert!(empty.foreground_project_path.is_none());

    assert!(
        decode_daemon_runtime_session_snapshot(Some(serde_json::json!({"not": "a snapshot"})))
            .is_err()
    );

    let payload = Some(serde_json::json!({
        "version": 7,
        "display_sessions": [
            {
                "pid": 1234,
                "project_path": "/tmp/project-a",
                "tty": "/dev/pts/1",
                "args": "codex --yolo",
                "cli_tool": "codex",
                "tmux_session": "taurhaus",
                "tmux_window": "1",
                "tmux_pane": "%1",
                "tmux_window_name": "work",
                "state": "active"
            }
        ],
        "runtime_sessions": [
            {
                "pid": 1234,
                "project_path": "/tmp/project-a",
                "tty": "/dev/pts/1",
                "args": "codex --yolo",
                "cli_tool": "codex",
                "tmux_session": "taurhaus",
                "tmux_window": "1",
                "tmux_pane": "%1",
                "tmux_window_name": "work",
                "state": "active",
                "session_id": "session-1",
                "jsonl_path": "/tmp/project-a/.codex/session.jsonl",
                "recent_io": true,
                "last_output_age_secs": 1,
                "activity_confidence": "high",
                "activity_attribution": "attributed",
                "project_unattributed_active": false,
                "group_kind": "standalone",
                "group_id": null,
                "group_label": null,
                "member_name": "developer1"
            }
        ],
        "focus": {
            "session": "taurhaus",
            "window": "work",
            "timestamp": 123
        },
        "foreground_project_path": "/tmp/project-a"
    }));

    let snapshot = decode_daemon_runtime_session_snapshot(payload).expect("valid snapshot");
    assert_eq!(snapshot.version, 7);
    assert_eq!(snapshot.display_sessions.len(), 1);
    assert_eq!(snapshot.runtime_sessions.len(), 1);
    assert_eq!(snapshot.focus, Some(attached_focus("taurhaus", "work")));
    assert_eq!(
        snapshot.foreground_project_path.as_deref(),
        Some("/tmp/project-a")
    );
    assert_eq!(
        snapshot.runtime_sessions[0].session_id.as_deref(),
        Some("session-1")
    );
}

#[test]
fn enrich_sessions_with_team_membership_marks_matching_sessions() {
    let tmp = TempDir::new().expect("temp teams dir");
    save_team_member(
        tmp.path(),
        "architecture-final",
        "developer2",
        "/home/dev/projects/taurhaus",
        CliTool::Codex,
    );

    let mut sessions = vec![active_session_for(
        r"\\wsl.localhost\Ubuntu\home\dev\projects\taurhaus",
    )];

    enrich_sessions_with_team_membership(tmp.path(), &mut sessions);

    assert_eq!(sessions[0].group_kind, SessionGroupKind::MeshTeam);
    assert_eq!(sessions[0].group_id.as_deref(), Some("architecture-final"));
    assert_eq!(
        sessions[0].group_label.as_deref(),
        Some("architecture-final")
    );
    assert_eq!(sessions[0].member_name.as_deref(), Some("developer2"));
}

#[test]
fn enrich_sessions_with_team_membership_leaves_unmatched_tool_standalone() {
    let tmp = TempDir::new().expect("temp teams dir");
    save_team_member(
        tmp.path(),
        "architecture-final",
        "lead",
        "/home/dev/projects/taurhaus",
        CliTool::Claude,
    );

    let mut sessions = vec![active_session_for("/home/dev/projects/taurhaus")];

    enrich_sessions_with_team_membership(tmp.path(), &mut sessions);

    assert_eq!(sessions[0].group_kind, SessionGroupKind::Standalone);
    assert_eq!(sessions[0].group_id, None);
    assert_eq!(sessions[0].group_label, None);
    assert_eq!(sessions[0].member_name, None);
}

#[test]
fn enrich_sessions_with_team_membership_skips_invalid_team_configs() {
    let tmp = TempDir::new().expect("temp teams dir");
    save_team_member(
        tmp.path(),
        "valid-team",
        "developer2",
        "/home/dev/projects/taurhaus",
        CliTool::Codex,
    );
    let broken_dir = tmp.path().join("broken-team");
    std::fs::create_dir_all(&broken_dir).expect("create broken dir");
    std::fs::write(broken_dir.join("config.json"), "{ invalid json").expect("write broken config");

    let mut sessions = vec![
        active_session_for("/home/dev/projects/taurhaus"),
        active_session_for("/home/dev/projects/other"),
    ];

    enrich_sessions_with_team_membership(tmp.path(), &mut sessions);

    assert_eq!(sessions[0].group_kind, SessionGroupKind::MeshTeam);
    assert_eq!(sessions[0].group_id.as_deref(), Some("valid-team"));
    assert_eq!(sessions[1].group_kind, SessionGroupKind::Standalone);
    assert_eq!(sessions[1].group_id, None);
}

#[test]
fn enrich_sessions_with_team_membership_distinguishes_same_tool_members_by_pane() {
    let tmp = TempDir::new().expect("temp teams dir");
    save_team_members(
        tmp.path(),
        "architecture-final",
        vec![
            Member {
                name: "developer1".to_string(),
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
                project_path: "/home/dev/projects/taurhaus".into(),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            },
            Member {
                name: "developer2".to_string(),
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
                project_path: "/home/dev/projects/taurhaus".into(),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            },
        ],
    );
    save_member_runtime(tmp.path(), "architecture-final", "developer1", "%11");
    save_member_runtime(tmp.path(), "architecture-final", "developer2", "%12");

    let mut first = active_session_for("/home/dev/projects/taurhaus");
    first.tmux_pane = Some("%11".to_string());
    let mut second = active_session_for("/home/dev/projects/taurhaus");
    second.pid = 4321;
    second.tmux_pane = Some("%12".to_string());

    let mut sessions = vec![first, second];
    enrich_sessions_with_team_membership(tmp.path(), &mut sessions);

    assert_eq!(sessions[0].group_kind, SessionGroupKind::MeshTeam);
    assert_eq!(sessions[1].group_kind, SessionGroupKind::MeshTeam);
    assert_eq!(sessions[0].member_name.as_deref(), Some("developer1"));
    assert_eq!(sessions[1].member_name.as_deref(), Some("developer2"));
    assert_ne!(sessions[0].member_name, sessions[1].member_name);
}

#[test]
fn daemon_launch_decode_handles_missing_invalid_and_valid_payloads() {
    let payload = Some(
        serde_json::json!({"tmux_session": "taurhaus", "tmux_window": "1", "tmux_pane": "%2"}),
    );
    let result = decode_daemon_launch_result(payload).expect("valid launch payload");
    assert_eq!(result.tmux_session.as_deref(), Some("taurhaus"));
    assert_eq!(result.tmux_window, "1");
    assert_eq!(result.tmux_pane, "%2");

    let err = decode_daemon_launch_result(Some(serde_json::json!({"unexpected": "shape"})))
        .expect_err("invalid payload should error");
    assert!(err.contains("Invalid launch result from daemon"));
    assert_eq!(
        decode_daemon_launch_result(None).expect_err("missing payload should error"),
        "Invalid launch result from daemon"
    );
}

#[test]
fn configured_base_command_defaults_are_non_empty_and_match_expected_values() {
    let cmds = crate::models::CliCommandSettings::default();
    for tool in [CliTool::Claude, CliTool::Codex, CliTool::Gemini] {
        for mode in [LaunchMode::Continue, LaunchMode::Fresh, LaunchMode::Resume] {
            let command = base_command(&cmds, tool, mode);
            assert!(
                !command.trim().is_empty(),
                "command must be non-empty for {tool:?}/{mode:?}"
            );
        }
    }
    for (tool, mode, expected) in [
        (
            CliTool::Claude,
            LaunchMode::Continue,
            "claude --dangerously-skip-permissions --continue",
        ),
        (
            CliTool::Claude,
            LaunchMode::Fresh,
            "claude --dangerously-skip-permissions",
        ),
        (
            CliTool::Claude,
            LaunchMode::Resume,
            "claude --dangerously-skip-permissions --resume",
        ),
        (CliTool::Codex, LaunchMode::Continue, "codex --yolo"),
        (CliTool::Codex, LaunchMode::Fresh, "codex --yolo"),
        (
            CliTool::Codex,
            LaunchMode::Resume,
            "codex resume --last --yolo",
        ),
        (
            CliTool::Gemini,
            LaunchMode::Continue,
            "gemini --yolo --resume",
        ),
        (CliTool::Gemini, LaunchMode::Fresh, "gemini --yolo"),
        (
            CliTool::Gemini,
            LaunchMode::Resume,
            "gemini --yolo --resume",
        ),
    ] {
        assert_eq!(base_command(&cmds, tool, mode), expected);
    }
}

#[test]
fn load_terminal_settings_returns_default_on_query_and_lock_errors() {
    let db = DbState(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    assert_eq!(
        load_terminal_settings(&db),
        crate::models::TerminalSettings::default()
    );

    let poisoned = DbState(Mutex::new(rusqlite::Connection::open_in_memory().unwrap()));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = poisoned.0.lock().unwrap();
        panic!("intentional poison");
    }));
    assert_eq!(
        load_terminal_settings(&poisoned),
        crate::models::TerminalSettings::default()
    );
}

#[test]
fn launch_cli_session_uses_daemon_success_response() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "tmux_session": "taurhaus",
            "tmux_window": "2",
            "tmux_pane": "%7"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let (log_file, _log_file) = setup_log_file();

    let result = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    assert_eq!(result.tmux_session.as_deref(), Some("taurhaus"));
    assert_eq!(result.tmux_window, "2");
    assert_eq!(result.tmux_pane, "%7");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(request.method, protocol::method::LAUNCH_SESSION);
}

#[test]
fn launch_cli_session_logs_daemon_request_context() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "tmux_session": "taurhaus",
            "tmux_window": "2",
            "tmux_pane": "%7"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let (log_file, log_file_path) = setup_log_file();

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let events = read_log_events(&log_file, log_file_path.path());
    let request = events
        .iter()
        .find(|event| event["event"] == "command_center.launch.daemon_request")
        .expect("daemon request event");
    assert_eq!(request["caller"], "command_center.launch");
    assert_eq!(request["daemon_request_id"], "launch-session");
    assert_eq!(request["daemon_method"], protocol::method::LAUNCH_SESSION);
    assert_eq!(request["project_id"], "p1");
    assert_eq!(request["project_path"], "/tmp/project");
}

#[test]
fn launch_cli_session_renders_non_team_base_only_and_logs_command() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "tmux_session": "taurhaus",
            "tmux_window": "2",
            "tmux_pane": "%7"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let (log_file, log_file_path) = setup_log_file();
    install_global_sink(&log_file);

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.params["command_override"],
        "claude --dangerously-skip-permissions"
    );

    let rendered = (0..100)
        .find_map(|_| {
            let content = std::fs::read_to_string(log_file_path.path()).unwrap_or_default();
            let event = content
                .lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .find(|event| {
                    // `component` is what separates this launch from the
                    // coordination pipelines' identically-named event, which
                    // carries the same tool/mode/command for a Claude member
                    // and lands in this process-global sink unguarded.
                    event["event"] == "launch.command.rendered"
                        && event["component"] == "command_center"
                        && event["tool"] == "claude"
                        && event["mode"] == "fresh"
                        && event["command"] == "claude --dangerously-skip-permissions"
                });
            if event.is_none() {
                thread::sleep(Duration::from_millis(20));
            }
            event
        })
        .expect("rendered launch event");
    assert_eq!(rendered["tool"], "claude");
    assert_eq!(rendered["mode"], "fresh");
    assert_eq!(rendered["command"], "claude --dangerously-skip-permissions");
}

/// Detection is faked here on purpose: no test may read the developer's real
/// `~/.claude*`, and this exercises the launch path, not detection.
fn fake_accounts() -> Vec<crate::session_scanner::claude_accounts::ClaudeAccount> {
    vec![
        crate::session_scanner::claude_accounts::ClaudeAccount {
            id: "account-1".to_string(),
            config_dir: PathBuf::from("/home/user/.claude"),
            email: "primary@example.com".to_string(),
            display_name: Some("Primary".to_string()),
            organization: None,
            seat_tier: None,
            logged_in: true,
            is_default: true,
            is_process_default: true,
            usage: None,
        },
        crate::session_scanner::claude_accounts::ClaudeAccount {
            id: "account-2".to_string(),
            config_dir: PathBuf::from("/home/user/.claude-account2"),
            email: "second@example.com".to_string(),
            display_name: Some("Second".to_string()),
            organization: None,
            seat_tier: None,
            logged_in: true,
            is_default: false,
            is_process_default: false,
            usage: None,
        },
    ]
}

use crate::session_scanner::claude_accounts::{install_detection_override, DetectionOverrideGuard};

fn with_fake_accounts() -> DetectionOverrideGuard {
    install_detection_override(fake_accounts())
}

fn stub_launch_provider(daemon: &StubDaemon) -> ProviderState {
    ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    }
}

fn launch_stub_daemon() -> StubDaemon {
    start_stub_daemon(serde_json::json!({
        "result": {
            "tmux_session": "taurhaus",
            "tmux_window": "2",
            "tmux_pane": "%7"
        },
        "error": null
    }))
}

// Regression: 791f6be rendered every Claude launch without a config dir, so a
// project pinned to a second subscription still ran on `~/.claude`.
#[test]
fn a_project_pinned_to_a_second_account_launches_with_its_config_dir() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let _accounts = with_fake_accounts();
    let daemon = launch_stub_daemon();
    let provider = stub_launch_provider(&daemon);
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    {
        let conn = db.0.lock().expect("db lock");
        crate::db::queries::set_project_claude_account(&conn, "p1", Some("account-2"))
            .expect("store account");
    }
    let (log_file, log_file_path) = setup_log_file();
    install_global_sink(&log_file);

    // Regression: 0619d6b answered the same failure by making every launch test
    // in *this* file take the log guard, but the sink `install_global_sink`
    // swaps is process-global and `emit_global` takes no guard at all. The
    // coordination pipelines emit the very same `launch.command.rendered` event
    // (`coordination/pipelines/helpers.rs`) from tests that hold no log guard,
    // so their record still lands in this file and a `find` on the event name
    // alone can take it — it carries no `claude_account`, and the assertion
    // below failed on a null. This decoy is that record, written where the race
    // would put it, so the selection has to be provenance-based to pass.
    crate::commands::logging::emit_global(
        "info",
        "coordination",
        "launch.command.rendered",
        Some("Rendered team member launch command".to_string()),
        serde_json::json!({
            "team": "architecture-final",
            "member": "builder",
            "tool": "claude",
            "mode": "fresh",
            "command": "claude --dangerously-skip-permissions",
        })
        .as_object()
        .expect("decoy fields")
        .clone(),
    );

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.params["command_override"],
        "CLAUDE_CONFIG_DIR='/home/user/.claude-account2' claude --dangerously-skip-permissions"
    );

    let events = read_log_events(&log_file, log_file_path.path());
    let rendered = events
        .iter()
        .find(|event| {
            event["event"] == "launch.command.rendered" && event["component"] == "command_center"
        })
        .expect("rendered launch event");
    assert_eq!(rendered["claude_account"], "second@example.com");
}

#[test]
fn a_project_pinned_to_a_vanished_account_falls_back_and_says_so() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let _accounts = with_fake_accounts();
    let daemon = launch_stub_daemon();
    let provider = stub_launch_provider(&daemon);
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    {
        let conn = db.0.lock().expect("db lock");
        crate::db::queries::set_project_claude_account(&conn, "p1", Some("deleted-account"))
            .expect("store account");
    }
    let (log_file, log_file_path) = setup_log_file();
    install_global_sink(&log_file);

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.params["command_override"],
        "claude --dangerously-skip-permissions"
    );

    let events = read_log_events(&log_file, log_file_path.path());
    let fallback = events
        .iter()
        .find(|event| event["event"] == "launch.account.fallback")
        .expect("fallback event");
    assert_eq!(fallback["project"], "p1");
    assert_eq!(fallback["wanted"], "deleted-account");
    assert_eq!(fallback["used"], "primary@example.com");
    assert_eq!(fallback["reason"], "account_unavailable");
}

// Regression: 518aace read every daemon failure as an empty account list. A
// Windows resume whose transcript lookup never ran then dropped to the
// physical default config dir — a different subscription's history — without
// a word in the log to say the question had gone unanswered.
#[test]
fn a_launch_whose_detection_failed_falls_back_and_says_why() {
    use super::launching::{decide_launch_account, log_account_resolution};
    use crate::commands::claude_accounts::{ClaudeAccountsResult, TranscriptLookup};
    use crate::session_scanner::claude_accounts::AccountRequest;

    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let (log_file, log_file_path) = setup_log_file();
    install_global_sink(&log_file);

    let launch = decide_launch_account(
        &ClaudeAccountsResult {
            accounts: Vec::new(),
            source: "daemon".to_string(),
            degraded: true,
            error: Some("Daemon transport error: connection reset by peer".to_string()),
        },
        &TranscriptLookup {
            transcript: None,
            unavailable: Some("timed out waiting for daemon".to_string()),
        },
        AccountRequest {
            requested_account_id: None,
            session_transcript: None,
            project_account_id: Some("account-2"),
            default_account_id: None,
        },
    );
    log_account_resolution("p1", &launch);

    let events = read_log_events(&log_file, log_file_path.path());
    let fallback = events
        .iter()
        .find(|event| event["event"] == "launch.account.fallback")
        .expect("fallback event");
    assert_eq!(fallback["project"], "p1");
    assert_eq!(fallback["reason"], "daemon_unavailable");
    assert_eq!(fallback["wanted"], "account-2");
    assert_eq!(fallback["used"], serde_json::Value::Null);
}

/// A transcript recovered from this process's own sightings placed the resume,
/// so nothing fell back — a warning here would be noise.
#[test]
fn a_resume_the_remembered_transcript_placed_is_not_a_fallback() {
    use super::launching::decide_launch_account;
    use crate::commands::claude_accounts::{ClaudeAccountsResult, TranscriptLookup};
    use crate::session_scanner::claude_accounts::AccountRequest;

    let transcript =
        PathBuf::from("/home/user/.claude-account2/projects/-tmp-project/session.jsonl");
    let launch = decide_launch_account(
        &ClaudeAccountsResult {
            accounts: fake_accounts(),
            source: "daemon".to_string(),
            degraded: false,
            error: None,
        },
        &TranscriptLookup {
            transcript: Some(transcript.clone()),
            unavailable: Some("timed out waiting for daemon".to_string()),
        },
        AccountRequest {
            requested_account_id: None,
            session_transcript: Some(transcript.as_path()),
            project_account_id: Some("account-1"),
            default_account_id: None,
        },
    );

    assert!(launch.degraded.is_none());
}

#[test]
fn a_codex_launch_never_receives_a_claude_config_dir() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let _accounts = with_fake_accounts();
    let daemon = launch_stub_daemon();
    let provider = stub_launch_provider(&daemon);
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    {
        let conn = db.0.lock().expect("db lock");
        crate::db::queries::set_project_claude_account(&conn, "p1", Some("account-2"))
            .expect("store account");
    }
    let (log_file, _log_file_path) = setup_log_file();

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Codex),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(request.params["command_override"], "codex --yolo");
}

fn exited_claude_session(
    project_path: &str,
    transcript: &str,
) -> crate::session_scanner::RuntimeSession {
    crate::session_scanner::RuntimeSession {
        pid: 9182,
        project_path: project_path.to_string(),
        tty: "/dev/pts/5".to_string(),
        args: "claude".to_string(),
        cli_tool: CliTool::Claude,
        tmux_session: None,
        tmux_window: None,
        tmux_pane: None,
        tmux_window_name: None,
        state: SessionState::Idle,
        session_id: Some("f3286b16-ffc7-4d16-915d-046705823a3d".to_string()),
        jsonl_path: Some(transcript.to_string()),
        recent_io: false,
        last_output_age_secs: Some(12),
        activity_confidence: Default::default(),
        activity_attribution: Default::default(),
        project_unattributed_active: false,
        group_kind: SessionGroupKind::Standalone,
        group_id: None,
        group_label: None,
        member_name: None,
    }
}

// Regression: c982822 took the resume transcript from the live runtime
// snapshot, which lists running processes only. Resume is reached for after
// Claude has exited, and by then the session is gone from that snapshot — so
// the subscription that owns the history was unavailable exactly when it had
// to decide where `--resume` runs.
#[test]
fn resume_runs_on_the_account_of_the_last_session_after_it_exited() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let _accounts = with_fake_accounts();
    let daemon = launch_stub_daemon();
    let provider = stub_launch_provider(&daemon);
    let (db, _db_file) = setup_db_with_project("p-resume", "/tmp/resume-project");
    let (log_file, log_file_path) = setup_log_file();
    install_global_sink(&log_file);

    // The last session for this project ran on the second subscription.
    crate::session_scanner::claude_accounts::record_claude_transcripts(&[exited_claude_session(
        "/tmp/resume-project",
        "/home/user/.claude-account2/projects/-tmp-resume-project/f3286b16.jsonl",
    )]);
    // Claude exited: no runtime snapshot mentions it any more.
    crate::session_snapshot_cache::clear();

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p-resume".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.params["command_override"],
        "CLAUDE_CONFIG_DIR='/home/user/.claude-account2' claude --dangerously-skip-permissions --resume"
    );

    let events = read_log_events(&log_file, log_file_path.path());
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "launch.account.derived_from_session"),
        "{events:?}"
    );
}

/// Accounts whose config dirs are real directories under `home`, so a launch
/// can read the transcripts that decide a resume. Detection itself is faked:
/// no test may read the developer's real `~/.claude*`.
fn with_fake_accounts_under(home: &Path) -> DetectionOverrideGuard {
    let mut accounts = fake_accounts();
    accounts[0].config_dir = home.join(".claude");
    accounts[1].config_dir = home.join(".claude-account2");
    install_detection_override(accounts)
}

/// A transcript where Claude Code writes one: `<config dir>/projects/<slug>/`.
fn write_transcript(config_dir: &Path, project_path: &str, name: &str) -> PathBuf {
    let dir = config_dir
        .join("projects")
        .join(crate::session_scanner::idle::path_to_slug(project_path));
    std::fs::create_dir_all(&dir).expect("transcript dir");
    let path = dir.join(name);
    std::fs::write(&path, "{}\n").expect("transcript");
    path
}

// Regression: 518aace kept the project→transcript map in a process-local
// mutex that only scans run by this process fill. A restarted app reaches
// Resume with an empty map — and on Windows so does a running one, because the
// app records sightings while seeding the bridge and steady-state updates
// carry no transcript at all. The subscription that owns the project's history
// was therefore lost exactly when it had to decide where `--resume` runs.
#[test]
fn resume_reads_the_account_from_the_transcripts_on_disk() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let home = TempDir::new().expect("home");
    let _accounts = with_fake_accounts_under(home.path());
    let project_path = "/tmp/restarted-project";
    write_transcript(
        &home.path().join(".claude-account2"),
        project_path,
        "f3286b16.jsonl",
    );
    let daemon = launch_stub_daemon();
    let provider = stub_launch_provider(&daemon);
    let (db, _db_file) = setup_db_with_project("p-restart", project_path);
    let (log_file, _log_file_path) = setup_log_file();

    // This process never saw the session: nothing recorded a sighting.
    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p-restart".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.params["command_override"],
        format!(
            "CLAUDE_CONFIG_DIR='{}' claude --dangerously-skip-permissions --resume",
            home.path().join(".claude-account2").display()
        )
    );
}

// Regression: c982822 rendered no `CLAUDE_CONFIG_DIR` whenever the selected
// account sat in `PlatformPaths::claude_dir()` — which `TAURHAUS_CLAUDE_DIR`
// moves. Claude Code reads only `CLAUDE_CONFIG_DIR`: with the variable unset it
// uses the process's own `~/.claude`, so the configured root was silently
// swapped for whatever subscription lives there. The fixture is the state that
// override produces: taurhaus's default account is not the dir Claude Code
// reads by itself.
#[test]
fn a_configured_claude_root_is_named_in_the_launch() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let mut accounts = fake_accounts();
    accounts[0].config_dir = PathBuf::from("/tmp/e2e-run/claude");
    accounts[0].is_process_default = false;
    let _guard = install_detection_override(accounts);
    let daemon = launch_stub_daemon();
    let provider = stub_launch_provider(&daemon);
    let (db, _db_file) = setup_db_with_project("p-isolated", "/tmp/isolated-project");
    let (log_file, _log_file_path) = setup_log_file();

    launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p-isolated".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect("daemon launch should succeed");

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(
        request.params["command_override"],
        "CLAUDE_CONFIG_DIR='/tmp/e2e-run/claude' claude --dangerously-skip-permissions"
    );
}

fn preflight_provider() -> ProviderState {
    ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    }
}

// Regression: c982822 let the chooser answer for every mode. `--resume` only
// sees the history of the config dir it runs in, and an explicit answer
// outranks the transcript that owns it — so a project whose history lives in a
// second subscription resumed on whichever account the user picked in a dialog
// that should never have opened. The frontend can only skip that dialog if the
// backend says the launch is already placed.
#[test]
fn the_preflight_places_a_resume_from_the_transcript_that_owns_the_history() {
    let home = TempDir::new().expect("home");
    let _accounts = with_fake_accounts_under(home.path());
    let project_path = "/tmp/preflight-project";
    write_transcript(
        &home.path().join(".claude-account2"),
        project_path,
        "abc.jsonl",
    );
    let (db, _db_file) = setup_db_with_project("p-preflight", project_path);

    let placed = resolve_claude_launch_account_impl(
        &db,
        &preflight_provider(),
        "p-preflight".to_string(),
        LaunchMode::Resume,
    )
    .expect("preflight");

    assert_eq!(placed.source, "session");
    assert_eq!(placed.email.as_deref(), Some("second@example.com"));
    assert!(!placed.needs_choice);
}

#[test]
fn the_preflight_asks_when_no_history_and_no_choice_place_the_launch() {
    let home = TempDir::new().expect("home");
    let _accounts = with_fake_accounts_under(home.path());
    let (db, _db_file) = setup_db_with_project("p-unplaced", "/tmp/unplaced-project");

    let placed = resolve_claude_launch_account_impl(
        &db,
        &preflight_provider(),
        "p-unplaced".to_string(),
        LaunchMode::Resume,
    )
    .expect("preflight");

    assert!(placed.needs_choice);
}

#[test]
fn the_preflight_needs_no_choice_once_the_project_stored_one() {
    let home = TempDir::new().expect("home");
    let _accounts = with_fake_accounts_under(home.path());
    let (db, _db_file) = setup_db_with_project("p-stored", "/tmp/stored-project");
    {
        let conn = db.0.lock().expect("db lock");
        crate::db::queries::set_project_claude_account(&conn, "p-stored", Some("account-2"))
            .expect("store account");
    }

    let placed = resolve_claude_launch_account_impl(
        &db,
        &preflight_provider(),
        "p-stored".to_string(),
        LaunchMode::Fresh,
    )
    .expect("preflight");

    assert_eq!(placed.source, "project");
    assert_eq!(placed.account_id.as_deref(), Some("account-2"));
    assert!(!placed.needs_choice);
}

#[test]
fn launch_cli_session_surfaces_daemon_error_message() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let daemon = start_stub_daemon(serde_json::json!({
        "result": null,
        "error": {
            "code": "LAUNCH_ERROR",
            "message": "simulated launch failure"
        }
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let (log_file, _log_file) = setup_log_file();

    let err = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Fresh,
        Some(CliTool::Claude),
        None,
    )
    .expect_err("daemon launch should return error");

    assert!(err.contains("simulated launch failure"));
}

#[test]
fn stop_cli_session_surfaces_daemon_error_message() {
    let daemon = start_stub_daemon(serde_json::json!({
        "result": null,
        "error": {
            "code": "STOP_ERROR",
            "message": "cannot stop session"
        }
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };

    let (log_file, _log_file_path) = setup_log_file();
    let err = stop_cli_session_impl(
        &log_file,
        &provider,
        "%10".to_string(),
        Some(CliTool::Codex),
    )
    .expect_err("daemon stop should return error");
    assert!(err.contains("cannot stop session"));
}

#[test]
fn record_session_activity_persists_lowercase_cli_tool_from_enum() {
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    record_session_activity_impl(
        &db,
        "p1".to_string(),
        CliTool::Gemini,
        "2026-03-04T10:00:00Z".to_string(),
        "2026-03-04T11:00:00Z".to_string(),
        1_000,
        2_000,
    )
    .expect("record activity");

    let conn = db.0.lock().expect("db lock");
    let stored_tool: String = conn
        .query_row("SELECT cli_tool FROM session_activity LIMIT 1", [], |row| {
            row.get(0)
        })
        .expect("query cli_tool");
    assert_eq!(stored_tool, "gemini");
}

#[test]
fn launch_codex_resume_returns_project_not_found_for_invalid_project_id() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };
    let (log_file, _log_file) = setup_log_file();

    let err = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "missing-project".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Codex),
        None,
    )
    .expect_err("missing project should fail");

    assert_eq!(err, "Project not found: missing-project");
}

#[test]
fn launch_codex_resume_surfaces_fallback_error_when_daemon_is_unreachable() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let daemon = start_unreachable_stub_daemon();
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/path/that/does/not/exist");
    let (log_file, _log_file) = setup_log_file();

    let err = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        None,
        "p1".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Codex),
        None,
    )
    .expect_err("daemon-unreachable fallback should still fail with useful error");

    assert!(
        err.contains("Failed to launch session: Project path does not exist"),
        "unexpected error: {err}"
    );
}

#[test]
fn generic_resume_delegates_to_coordination_for_unique_team_member_match() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("temp teams dir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    runtime.set_pane_exists("%9", false);
    runtime.set_pid_running(4242, true);
    let coordination_state = test_coordination_state(tmp.path(), runtime.clone());
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };
    let (log_file, _log_file) = setup_log_file();

    save_team_member(
        tmp.path(),
        "architecture-final",
        "developer2",
        "/tmp/project",
        CliTool::Codex,
    );
    save_member_runtime_record(
        tmp.path(),
        "architecture-final",
        "developer2",
        MemberRuntimeRecord {
            schema_version: 3,
            member_name: "developer2".to_string(),
            cli_tool: Some(CliTool::Codex),
            project_path: Some(PathBuf::from("/tmp/project")),
            pane_id: Some("%9".to_string()),
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: Some(4242),
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: Some(chrono::Utc::now()),
            last_seen_at: Some(chrono::Utc::now()),
        },
    );

    let result = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        Some(&coordination_state),
        "p1".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Codex),
        None,
    )
    .expect("delegated resume should succeed");

    assert_eq!(result.tmux_session.as_deref(), Some(TMUX_SESSION_NAME));
    assert_eq!(result.tmux_window, "0");
    assert_eq!(result.tmux_pane, "test-pane-1");

    let runtime_record = MemberRuntimeStore::load(tmp.path(), "architecture-final", "developer2")
        .expect("load runtime");
    assert_eq!(runtime_record.pane_id.as_deref(), Some("test-pane-1"));
    assert_eq!(runtime_record.daemon_pid, Some(10000));

    let config_json = std::fs::read_to_string(tmp.path().join("architecture-final/config.json"))
        .expect("read config");
    assert!(config_json.contains("\"tmuxPaneId\": \"test-pane-1\""));

    let calls = runtime.calls();
    assert!(calls.contains(&RuntimeCall::CheckPaneExists {
        pane_id: "%9".to_string(),
    }));
    assert!(calls.contains(&RuntimeCall::CreatePane {
        project_id: "/tmp/project".to_string(),
    }));
    assert!(calls.contains(&RuntimeCall::TerminatePid { pid: 4242 }));
    assert!(calls.contains(&RuntimeCall::SpawnDaemon {
        pane_id: "test-pane-1".to_string(),
        team_name: "architecture-final".to_string(),
        member_name: "developer2".to_string(),
    }));
    assert!(calls.contains(&RuntimeCall::JoinMesh {
        team_name: "architecture-final".to_string(),
        member_name: "developer2".to_string(),
        project_id: "/tmp/project".to_string(),
        member_type: "general-purpose".to_string(),
        model: "gpt-5.6-sol".to_string(),
        claude_dir: crate::coordination::runtime::resolve_mesh_cli_claude_dir_arg()
            .expect("Claude config dir"),
    }));
}

#[test]
fn generic_resume_falls_back_to_raw_launch_when_team_match_is_ambiguous() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("temp teams dir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let coordination_state = test_coordination_state(tmp.path(), runtime.clone());
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "tmux_session": "taurhaus",
            "tmux_window": "2",
            "tmux_pane": "%7"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let (log_file, _log_file) = setup_log_file();

    save_team_members(
        tmp.path(),
        "architecture-final",
        vec![
            Member {
                name: "developer1".to_string(),
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
                project_path: "/tmp/project".into(),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            },
            Member {
                name: "developer2".to_string(),
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
                project_path: "/tmp/project".into(),
                cli_tool: CliTool::Codex,
                extra: Default::default(),
            },
        ],
    );

    let result = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        Some(&coordination_state),
        "p1".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Codex),
        None,
    )
    .expect("ambiguous match should use raw launch");

    assert_eq!(result.tmux_pane, "%7");
    assert!(runtime.calls().is_empty());

    let request = daemon
        .last_request
        .lock()
        .expect("request slot")
        .clone()
        .expect("captured request");
    assert_eq!(request.method, protocol::method::LAUNCH_SESSION);
}

#[test]
fn generic_resume_falls_back_to_raw_launch_for_non_team_session() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("temp teams dir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let coordination_state = test_coordination_state(tmp.path(), runtime.clone());
    let daemon = start_stub_daemon(serde_json::json!({
        "result": {
            "tmux_session": "taurhaus",
            "tmux_window": "3",
            "tmux_pane": "%8"
        },
        "error": null
    }));
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: Some(
            crate::provider::daemon_client::DaemonProvider::connect(&daemon.addr)
                .expect("connect daemon provider"),
        ),
        wsl_distro: None,
    };
    let (db, _db_file) = setup_db_with_project("p1", "/tmp/project");
    let (log_file, _log_file) = setup_log_file();

    let result = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        Some(&coordination_state),
        "p1".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Codex),
        None,
    )
    .expect("non-team resume should use raw launch");

    assert_eq!(result.tmux_pane, "%8");
    assert!(runtime.calls().is_empty());
}

// Regression: 74c7761 gave every Claude launch row an account submenu, and the
// row forwards the account it names. A Continue/Resume for a project that is
// exactly one team member's is delegated to coordination before
// `claude_account_id` is ever read, so the picked subscription did nothing and
// said nothing. Teams run on the team's own config dir; the launch now reports
// the account it could not apply.
#[test]
fn delegated_resume_reports_the_account_it_could_not_apply() {
    // A launch emits into the process-global sink; hold the log guard so it
    // never lands in the file another test is reading.
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("temp teams dir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let coordination_state = test_coordination_state(tmp.path(), runtime.clone());
    let (db, _db_file) = setup_db_with_project("p-team-account", "/tmp/project");
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };
    let (log_file, log_file_path) = setup_log_file();
    install_global_sink(&log_file);

    save_team_member(
        tmp.path(),
        "architecture-final",
        "developer2",
        "/tmp/project",
        CliTool::Claude,
    );
    save_member_runtime_record(
        tmp.path(),
        "architecture-final",
        "developer2",
        MemberRuntimeRecord {
            schema_version: 3,
            member_name: "developer2".to_string(),
            cli_tool: Some(CliTool::Claude),
            project_path: Some(PathBuf::from("/tmp/project")),
            pane_id: Some("%9".to_string()),
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: Some(chrono::Utc::now()),
            last_seen_at: Some(chrono::Utc::now()),
        },
    );

    let result = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        Some(&coordination_state),
        "p-team-account".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Claude),
        Some("account-2".to_string()),
    )
    .expect("delegated resume should succeed");

    assert_eq!(result.account_applied, Some(false));
    assert_eq!(result.account_note.as_deref(), Some("team_default"));

    let events = read_log_events(&log_file, log_file_path.path());
    let ignored = events
        .iter()
        .find(|event| event["event"] == "launch.account.ignored_for_team")
        .expect("ignored-for-team event");
    assert_eq!(ignored["project_id"], "p-team-account");
    assert_eq!(ignored["wanted"], "account-2");
}

// A delegated resume nobody named an account for has nothing to report: the
// team's config dir is the only answer there ever was.
#[test]
fn delegated_resume_without_a_requested_account_reports_nothing() {
    let _log_guard = crate::test_support::acquire_global_log_test_guard();
    let tmp = TempDir::new().expect("temp teams dir");
    let runtime = Arc::new(RecordingCoordinationRuntime::default());
    let coordination_state = test_coordination_state(tmp.path(), runtime.clone());
    let (db, _db_file) = setup_db_with_project("p-team-plain", "/tmp/project");
    let provider = ProviderState {
        local: crate::provider::local::LocalProvider,
        daemon: None,
        wsl_distro: None,
    };
    let (log_file, _log_file) = setup_log_file();

    save_team_member(
        tmp.path(),
        "architecture-final",
        "developer2",
        "/tmp/project",
        CliTool::Claude,
    );
    save_member_runtime_record(
        tmp.path(),
        "architecture-final",
        "developer2",
        MemberRuntimeRecord {
            schema_version: 3,
            member_name: "developer2".to_string(),
            cli_tool: Some(CliTool::Claude),
            project_path: Some(PathBuf::from("/tmp/project")),
            pane_id: Some("%9".to_string()),
            pane_pid: None,
            pane_start_time: None,
            session_id: None,
            jsonl_path: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: Some(chrono::Utc::now()),
            last_seen_at: Some(chrono::Utc::now()),
        },
    );

    let result = launch_cli_session_impl(
        &db,
        &provider,
        &log_file,
        Some(&coordination_state),
        "p-team-plain".to_string(),
        LaunchMode::Resume,
        Some(CliTool::Claude),
        None,
    )
    .expect("delegated resume should succeed");

    assert_eq!(result.account_applied, None);
    assert_eq!(result.account_note, None);
}

// One warning per project per run: the menu offers the choice on every
// right-click, and a log line per click says nothing the first one did not.
#[test]
fn team_account_notice_warns_once_per_project() {
    use super::launching::first_team_account_notice;

    assert!(first_team_account_notice("project-notice-once"));
    assert!(!first_team_account_notice("project-notice-once"));
    assert!(first_team_account_notice("project-notice-other"));
}
