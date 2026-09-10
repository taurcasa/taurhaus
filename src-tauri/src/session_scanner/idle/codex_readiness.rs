//! Pre-turn readiness for attributed TUI seats; never a hosted activity source.
use super::*;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub(crate) struct Observation {
    pub state: SessionState,
    pub source: &'static str,
    pub last_observed_at: DateTime<Utc>,
}

#[derive(Default)]
struct Quiet {
    previous: Option<u64>,
    since: Option<DateTime<Utc>>,
    last_growth: Option<DateTime<Utc>>,
}

fn sample(
    quiet: &mut Quiet,
    rchar: Option<u64>,
    prompt: bool,
    no_rollout: bool,
    notify: Option<&crate::daemon::codex_notify::CodexNotifyRecord>,
    launch: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<Observation> {
    let Some(rchar) = rchar else {
        *quiet = Quiet::default();
        return None;
    };
    let changed = quiet.previous.is_some_and(|previous| previous != rchar);
    if quiet.previous.is_none() || changed {
        quiet.since = Some(now);
    }
    if changed {
        quiet.last_growth = Some(now);
    }
    quiet.previous = Some(rchar);
    let notify = notify.filter(|record| record.ts >= launch && record.ts <= now);
    let (state, source) = if changed {
        (SessionState::Active, "process_io")
    } else if let Some(record) = notify {
        if record.event == "agent-turn-complete"
            && quiet.last_growth.is_none_or(|since| since <= record.ts)
        {
            (SessionState::Idle, "notify")
        } else {
            (SessionState::Active, "notify")
        }
    } else if no_rollout
        && prompt
        && quiet.since.is_some_and(|since| {
            now.signed_duration_since(since)
                .to_std()
                .is_ok_and(|age| age >= CODEX_ACTIVE_THRESHOLD)
        })
    {
        (SessionState::Idle, "launch_ready")
    } else {
        return None;
    };
    Some(Observation {
        state,
        source,
        last_observed_at: now,
    })
}

fn no_turn_yet(path: Option<&str>) -> bool {
    use std::io::Read;
    let Some(path) = path else {
        return true;
    };
    let mut bytes = Vec::new();
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    if file.take(65_537).read_to_end(&mut bytes).is_err() || bytes.len() > 65_536 {
        return false;
    }
    bytes
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .all(|line| {
            serde_json::from_slice::<serde_json::Value>(line)
                .is_ok_and(|v| v["type"] == "session_meta")
        })
}

struct Seat {
    identity: String,
    project: String,
    pane: String,
    quiet: Quiet,
    observation: Option<Observation>,
    scanned: DateTime<Utc>,
}
static SEATS: Mutex<Option<HashMap<u32, Seat>>> = Mutex::new(None);

pub(super) fn invalidate(pid: u32) {
    if let Some(seat) = SEATS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(|seats| seats.get_mut(&pid))
    {
        seat.observation = None;
    }
}

#[cfg(test)]
pub(super) fn elapse_quiet_window(pid: u32) {
    let mut guard = SEATS.lock().unwrap();
    guard.as_mut().unwrap().get_mut(&pid).unwrap().quiet.since =
        Some(Utc::now() - chrono::Duration::seconds(10));
}

pub(crate) fn observation(pid: u32, project: &str, pane: Option<&str>) -> Option<Observation> {
    let guard = SEATS.lock().unwrap_or_else(|e| e.into_inner());
    let seat = guard.as_ref()?.get(&pid)?;
    let age = Utc::now().signed_duration_since(seat.scanned);
    (seat.project == project
        && Some(seat.pane.as_str()) == pane
        && age >= chrono::Duration::zero()
        && age <= chrono::Duration::seconds(2))
    .then(|| seat.observation.clone())
    .flatten()
}

pub(super) fn refresh(
    result: &mut IdleResult,
    project: &str,
    pid: u32,
    records: &[crate::coordination::stores::MemberRuntimeRecord],
    notify_path: &Path,
) {
    let now = Utc::now();
    let bound: Vec<_> = records
        .iter()
        .filter(|record| super::codex::codex_runtime_matches(record, project, pid, None))
        .collect();
    let ([record], Some(id)) = (bound.as_slice(), result.session_id.as_deref()) else {
        return;
    };
    let (Some(pane), Some(socket), Some(launch)) =
        (&record.pane_id, &record.tmux_socket, record.attached_at)
    else {
        return;
    };
    // Capture only the explicitly attributed runtime socket, never a default server.
    let prompt = super::super::process::run_with_timeout_within(
        "tmux",
        &[
            "-S",
            &socket.to_string_lossy(),
            "capture-pane",
            "-p",
            "-t",
            pane,
        ],
        Duration::from_millis(200),
    )
    .is_some_and(|text| {
        text.trim_end()
            .lines()
            .rev()
            .take(8)
            .any(|line| line.trim_start().starts_with("› ") || line.trim() == "›")
    });
    let notify = crate::daemon::codex_notify::latest_record_for_session_after(
        notify_path,
        id,
        "",
        launch.into(),
    );
    let identity = format!(
        "{id}:{:?}:{launch}:{socket:?}",
        crate::platform::process_start_ticks(pid)
    );
    let rchar = crate::platform::process_rchar(pid);
    let mut guard = SEATS.lock().unwrap_or_else(|e| e.into_inner());
    let seats = guard.get_or_insert_with(HashMap::new);
    seats
        .retain(|_, seat| now.signed_duration_since(seat.scanned) < chrono::Duration::seconds(120));
    let seat = seats.entry(pid).or_insert_with(|| Seat {
        identity: identity.clone(),
        project: project.into(),
        pane: pane.clone(),
        quiet: Quiet::default(),
        observation: None,
        scanned: now,
    });
    if seat.identity != identity || seat.project != project || seat.pane != *pane {
        seat.quiet = Quiet::default();
        seat.identity = identity;
        seat.project = project.into();
        seat.pane = pane.clone();
    }
    seat.scanned = now;
    seat.observation = sample(
        &mut seat.quiet,
        rchar,
        prompt,
        no_turn_yet(result.jsonl_path.as_deref()),
        notify.as_ref(),
        launch,
        now,
    );
    if let Some(observed) = &seat.observation {
        result.state = observed.state;
        result.authoritative = observed.source == "notify";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::codex_notify::{append_event_at, latest_record_for_session_after};

    // Regression: 32bfd698 attributed a pre-rollout seat but left its activity
    // uncertain, so team-owned delivery could never send its first card.
    #[test]
    fn codex_launch_ready_quiet_prompt_and_notify_handoff() {
        let tmp = tempfile::tempdir().unwrap();
        let notify = tmp.path().join("notify.jsonl");
        let launch = Utc::now() - chrono::Duration::seconds(30);
        let ready = launch + chrono::Duration::seconds(10);
        let mut quiet = Quiet::default();
        let mut poll = |io, record: Option<&crate::daemon::codex_notify::CodexNotifyRecord>, at| {
            sample(&mut quiet, Some(io), true, true, record, launch, at)
        };
        assert!(poll(10, None, launch).is_none());
        assert!(poll(10, None, ready - chrono::Duration::milliseconds(1)).is_none());
        let idle = poll(10, None, ready).unwrap();
        assert_eq!(idle.state, SessionState::Idle);
        assert_eq!(idle.source, "launch_ready");
        assert_eq!(idle.last_observed_at, ready);
        let now = Utc::now();
        assert_eq!(poll(10, None, now).unwrap().last_observed_at, now);
        assert_eq!(poll(11, None, now).unwrap().state, SessionState::Active);
        for (event, expected) in [
            ("agent-turn-started", SessionState::Active),
            ("agent-turn-complete", SessionState::Idle),
        ] {
            let raw = serde_json::json!({"type": event, "thread-id": "seat"}).to_string();
            append_event_at(&notify, &raw, now).unwrap();
            let record =
                latest_record_for_session_after(&notify, "seat", "", launch.into()).unwrap();
            let observed = poll(11, Some(&record), now + chrono::Duration::seconds(11)).unwrap();
            assert_eq!(observed.state, expected);
            assert_eq!(observed.source, "notify");
        }
    }

    // Regression: 32bfd698 must not turn unresolved or non-prompt quiet into readiness.
    #[test]
    fn codex_launch_ready_requires_readable_io_prompt_and_no_turn() {
        let launch = Utc::now();
        for (io, prompt, no_rollout) in [
            (None, true, true),
            (Some(1), false, true),
            (Some(1), true, false),
        ] {
            let mut quiet = Quiet::default();
            sample(&mut quiet, io, prompt, no_rollout, None, launch, launch);
            assert!(sample(
                &mut quiet,
                io,
                prompt,
                no_rollout,
                None,
                launch,
                launch + chrono::Duration::seconds(10)
            )
            .is_none());
        }
    }
}
