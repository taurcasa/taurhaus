//! Pre-turn readiness for attributed TUI seats; never a hosted activity source.
use super::*;
use chrono::{DateTime, Utc};

use super::ActivityObservation as Observation;
use crate::session_scanner::proc_io;

#[derive(Default)]
struct Quiet {
    previous: Option<(u64, DateTime<Utc>)>,
    since: Option<DateTime<Utc>>,
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
    // Share the calibrated rate and minimum cadence with working detection.
    // Sub-threshold keep-alive reads must let the quiet window accrue; close
    // polls retain the previous sample rather than amplifying noise into a burst.
    let elapsed = quiet
        .previous
        .and_then(|(_, at)| now.signed_duration_since(at).to_std().ok());
    if elapsed.is_none_or(|age| age >= proc_io::MIN_SAMPLE_INTERVAL) {
        if quiet.previous.is_none_or(|(previous, _)| {
            rchar < previous
                || elapsed
                    .is_none_or(|age| proc_io::is_active_rate(rchar.saturating_sub(previous), age))
        }) {
            quiet.since = Some(now);
        }
        quiet.previous = Some((rchar, now));
    }
    let notify = notify.filter(|record| {
        record.ts >= launch && record.ts <= now && record.event == "agent-turn-complete"
    });
    // refresh supplies only a completion validated against the bound transcript.
    // Working remains the classifier's responsibility, using IO hysteresis and
    // transcript activity; legacy Codex notify has no turn-start producer.
    let (state, source) = if notify.is_some() {
        (SessionState::Idle, "notify")
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

fn prompt_before_first_turn(path: Option<&str>, probe: impl FnOnce() -> bool) -> (bool, bool) {
    let no_rollout = no_turn_yet(path);
    (no_rollout, no_rollout && probe())
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

#[cfg(test)]
pub(crate) fn seed_observation_for_test(
    pid: u32,
    project: &str,
    pane: &str,
    source: &'static str,
    state: SessionState,
    scanned: DateTime<Utc>,
) {
    SEATS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(
            pid,
            Seat {
                identity: "test-seat".into(),
                project: project.into(),
                pane: pane.into(),
                quiet: Quiet::default(),
                observation: Some(Observation {
                    state,
                    source,
                    last_observed_at: scanned,
                }),
                scanned,
            },
        );
}

pub fn observation(pid: u32, project: &str, pane: Option<&str>) -> Option<Observation> {
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
    let ancestors = super::codex::codex_process_ancestors(pid);
    let bound: Vec<_> = records
        .iter()
        .filter(|record| super::codex::codex_runtime_matches(record, project, &ancestors, None))
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
    let socket_text = socket.to_string_lossy();
    let args = ["-S", &socket_text, "capture-pane", "-p", "-t", pane];
    let (no_rollout, prompt) = prompt_before_first_turn(result.jsonl_path.as_deref(), || {
        super::super::process::run_with_timeout_within("tmux", &args, Duration::from_millis(200))
            .is_some_and(|text| {
                text.trim_end()
                    .lines()
                    .rev()
                    .take(8)
                    .any(|line| line.trim_start().starts_with("› ") || line.trim() == "›")
            })
    });
    // apply_notify_edge already checks the transcript boundary. Reattachment
    // cannot invalidate that completion; without a transcript there is no way
    // to tell an old completion from evidence for the current turn.
    let notify_since = if result.authoritative {
        DateTime::<Utc>::UNIX_EPOCH
    } else {
        launch
    };
    let notify = (result.jsonl_path.is_some() && result.authoritative)
        .then(|| {
            crate::daemon::codex_notify::latest_activity_record_for_session_after(
                notify_path,
                id,
                notify_since.into(),
            )
        })
        .flatten();
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
        no_rollout,
        notify.as_ref(),
        notify_since,
        now,
    );
    if let Some(observed) = &seat.observation {
        result.state = observed.state;
        result.authoritative |= observed.source == "notify";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::codex_notify::{append_event_at, latest_activity_record_for_session_after};

    // Regression: b9e4a855 (still present in bb67081e) counted every idle
    // keep-alive byte as work and restarted the launch quiet window forever.
    #[test]
    fn codex_round3_idle_keepalive_trace_reaches_launch_ready() {
        let launch = Utc::now();
        let mut quiet = Quiet::default();
        for tick in 0..=24 {
            let now = launch + chrono::Duration::milliseconds(tick * 500);
            let observed = sample(
                &mut quiet,
                Some(1000 + tick as u64 * 224),
                true,
                true,
                None,
                launch,
                now,
            );
            if tick < 20 {
                assert!(
                    observed.is_none(),
                    "idle noise became work at tick {tick}: {observed:?}"
                );
            } else {
                let observed = observed.expect("idle TUI must accrue its quiet window");
                assert_eq!(observed.state, SessionState::Idle);
                assert_eq!(observed.source, "launch_ready");
                assert_eq!(observed.last_observed_at, now);
            }
        }
    }

    // Regression: b9e4a855 promoted even a single read burst to authoritative
    // work; only the classifier's calibrated hysteresis may make that claim.
    #[test]
    fn codex_round3_readiness_does_not_claim_process_io_authority() {
        let launch = Utc::now();
        let mut quiet = Quiet::default();
        sample(&mut quiet, Some(1000), true, true, None, launch, launch);
        let burst = launch + chrono::Duration::milliseconds(500);
        assert!(sample(&mut quiet, Some(8000), true, true, None, launch, burst).is_none());
        assert!(sample(
            &mut quiet,
            Some(8000),
            true,
            true,
            None,
            launch,
            burst + chrono::Duration::seconds(9)
        )
        .is_none());
        assert_eq!(
            sample(
                &mut quiet,
                Some(8000),
                true,
                true,
                None,
                launch,
                burst + chrono::Duration::seconds(10)
            )
            .unwrap()
            .source,
            "launch_ready"
        );
    }

    // Regression: b9e4a855's attachment floor hid a completion that the bound
    // transcript had already validated, then raw IO cleared its idle authority.
    #[cfg(target_os = "linux")]
    #[test]
    fn codex_round3_reattachment_preserves_validated_completion() {
        let tmp = tempfile::tempdir().unwrap();
        let transcript = tmp.path().join("rollout.jsonl");
        fs::write(&transcript, "{\"type\":\"response_item\"}\n").unwrap();
        let notify = tmp.path().join("notify.jsonl");
        let now = Utc::now();
        append_event_at(
            &notify,
            r#"{"type":"agent-turn-complete","thread-id":"reattached"}"#,
            now - chrono::Duration::seconds(5),
        )
        .unwrap();
        let pid = std::process::id();
        let project = tmp.path().to_str().unwrap();
        let records = vec![serde_json::from_value(serde_json::json!({
            "paneId":"%round3", "panePid":pid,
            "paneStartTime":crate::platform::process_start_ticks(pid).unwrap().to_string(),
            "tmuxSocket":tmp.path().join("private.sock"), "cli_tool":"codex",
            "project_path":project, "attachedAt":now,
        }))
        .unwrap()];
        let mut result = IdleResult {
            session_id: Some("reattached".into()),
            jsonl_path: Some(transcript.to_string_lossy().into_owned()),
            authoritative: true,
            ..IdleResult::idle()
        };
        refresh(&mut result, project, pid, &records, &notify);
        let observed = observation(pid, project, Some("%round3"))
            .expect("validated notify must survive a later attachment");
        assert_eq!(observed.source, "notify");
        assert_eq!(observed.state, SessionState::Idle);
        assert!(result.authoritative);

        // A lock-only seat cannot validate an old completion against its next
        // turn. Refuse notify authority until a transcript is bound.
        result.jsonl_path = None;
        result.authoritative = false;
        let mut records = records;
        records[0].attached_at = Some(now - chrono::Duration::seconds(10));
        refresh(&mut result, project, pid, &records, &notify);
        assert!(
            !result.authoritative,
            "lock-only completion must not become authoritative"
        );
        assert!(observation(pid, project, Some("%round3")).is_none());
        invalidate(pid);
    }

    // Regression: b9e4a855 let any post-completion rchar delta override notify
    // idle; ef5fb097 retained that growth and pinned later quiet scans to working.
    #[test]
    fn codex_review_completion_survives_post_turn_io_and_quiet_scans() {
        let launch = Utc::now();
        let completed = launch + chrono::Duration::seconds(1);
        let mut quiet = Quiet::default();
        let record = crate::daemon::codex_notify::CodexNotifyRecord {
            ts: completed,
            session_id: Some("seat".into()),
            event: "agent-turn-complete".into(),
            turn_id: None,
        };
        sample(&mut quiet, Some(100), false, false, None, launch, launch);
        // Establish idle after completion before a later keep-alive read.
        let idle = sample(
            &mut quiet,
            Some(100),
            false,
            false,
            Some(&record),
            launch,
            completed + chrono::Duration::seconds(1),
        )
        .unwrap();
        assert_eq!(idle.state, SessionState::Idle);
        for seconds in [2, 3, 30] {
            let now = completed + chrono::Duration::seconds(seconds);
            let observed = sample(
                &mut quiet,
                Some(101),
                false,
                false,
                Some(&record),
                launch,
                now,
            )
            .unwrap();
            assert_eq!(observed.state, SessionState::Idle, "scan at {seconds}s");
            assert_eq!(observed.source, "notify");
            assert_eq!(observed.last_observed_at, now);
        }
    }

    // Regression: b9e4a855 spawned capture-pane even after a rollout contained a turn.
    #[test]
    fn codex_review_existing_turn_skips_prompt_probe() {
        let tmp = tempfile::tempdir().unwrap();
        let rollout = tmp.path().join("rollout.jsonl");
        fs::write(&rollout, "{\"type\":\"response_item\"}\n").unwrap();
        assert_eq!(
            prompt_before_first_turn(rollout.to_str(), || panic!(
                "must not capture a pane after a turn"
            )),
            (false, false)
        );
        assert_eq!(prompt_before_first_turn(None, || true), (true, true));
    }

    // Regression: b9e4a855 treated every unknown notification as authoritative working.
    #[test]
    fn codex_review_unknown_notify_is_not_working() {
        let launch = Utc::now() - chrono::Duration::seconds(20);
        for event in ["unknown", "", "future-event", "agent-turn-started"] {
            let mut quiet = Quiet::default();
            sample(&mut quiet, Some(1), true, true, None, launch, launch);
            let record = crate::daemon::codex_notify::CodexNotifyRecord {
                ts: launch,
                session_id: Some("seat".into()),
                event: event.into(),
                turn_id: None,
            };
            let result = sample(
                &mut quiet,
                Some(1),
                true,
                true,
                Some(&record),
                launch,
                Utc::now(),
            )
            .unwrap();
            assert_eq!(result.source, "launch_ready");
            assert_eq!(result.state, SessionState::Idle);
        }
    }

    // Regression: 664feab6 attributed a pre-rollout seat but left its activity
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
        assert_eq!(poll(11, None, now).unwrap().state, SessionState::Idle);
        let raw =
            serde_json::json!({"type": "agent-turn-complete", "thread-id": "seat"}).to_string();
        append_event_at(&notify, &raw, now).unwrap();
        let record =
            latest_activity_record_for_session_after(&notify, "seat", launch.into()).unwrap();
        let observed = poll(11, Some(&record), now + chrono::Duration::seconds(11)).unwrap();
        assert_eq!(observed.state, SessionState::Idle);
        assert_eq!(observed.source, "notify");
        // Regression: b9e4a855 timestamped an IO delta at the end of its
        // interval, hiding a completion that landed between those two polls.
        let later = now + chrono::Duration::seconds(20);
        assert_eq!(poll(12, None, later).unwrap().state, SessionState::Idle);
        let record = crate::daemon::codex_notify::CodexNotifyRecord {
            ts: now + chrono::Duration::seconds(15),
            session_id: Some("seat".into()),
            event: "agent-turn-complete".into(),
            turn_id: None,
        };
        assert_eq!(
            poll(12, Some(&record), later).unwrap().state,
            SessionState::Idle
        );
    }

    // Regression: 664feab6 must not turn unresolved or non-prompt quiet into readiness.
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
            let end = launch + chrono::Duration::seconds(10);
            assert!(sample(&mut quiet, io, prompt, no_rollout, None, launch, end).is_none());
        }
    }
}
