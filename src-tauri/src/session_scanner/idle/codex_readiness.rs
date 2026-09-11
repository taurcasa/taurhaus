//! Pre-turn readiness for attributed TUI seats; never a hosted activity source.
use super::*;
use chrono::{DateTime, Utc};

use super::ActivityObservation as Observation;
fn sample(
    prompt: Option<bool>,
    pre_turn: bool,
    notify: Option<&crate::daemon::codex_notify::CodexNotifyRecord>,
    launch: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<Observation> {
    let notify = notify.filter(|record| {
        record.ts >= launch && record.ts <= now && record.event == "agent-turn-complete"
    });
    // Only a transcript-validated completion is passed here. Before completion,
    // the attributed pane decides readiness independently of process polling IO.
    let (state, source) = match (notify.is_some(), pre_turn, prompt) {
        (true, _, _) => (SessionState::Idle, "notify"),
        (false, true, Some(true)) => (SessionState::Idle, "launch_ready"),
        (false, true, Some(false)) => (SessionState::Active, "pane_working"),
        _ => return None,
    };
    Some(Observation {
        state,
        source,
        last_observed_at: now,
    })
}

fn idle_prompt(text: &str) -> Option<bool> {
    let tail: Vec<_> = text
        .trim_end()
        .lines()
        .rev()
        .take(8)
        .map(str::trim)
        .collect();
    if tail.is_empty() {
        return None;
    }
    // Status lines can sit ABOVE the composer (e.g. MCP startup). Match their
    // chrome, not words in tips, warnings, the composer or directory footer.
    let busy = tail
        .iter()
        .filter(|line| !line.starts_with('›'))
        .any(|line| {
            let lower = line.to_lowercase();
            (line.starts_with('•')
                && (lower.starts_with("• working")
                    || lower.starts_with("• thinking")
                    || lower.contains("esc to interrupt")))
                || line.starts_with(|ch| ('\u{2800}'..='\u{28ff}').contains(&ch))
        });
    if busy {
        return Some(false);
    }
    // No busy chrome and no composer in the tail (a dialog, a picker, a scrolled
    // screen): unknown, never an authoritative "working" that nothing times out.
    if !tail
        .iter()
        .any(|line| line.starts_with("› ") || *line == "›")
    {
        return None;
    }
    tail[0]
        .split_once(" · ")
        .filter(|(model, directory)| !model.is_empty() && !directory.is_empty())
        .map(|_| true)
}

#[cfg(test)]
pub(crate) const IDLE_PANE: &str =
    "› Ask Codex to do anything\n\n  gpt-5.6-luna low · <scratch>/project\n";

fn no_turn_completed_since(path: Option<&str>, launch: DateTime<Utc>) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Some(path) = path else {
        return true;
    };
    let read = || -> Option<bool> {
        let mut file = fs::File::open(path).ok()?;
        let start = file.metadata().ok()?.len().saturating_sub(65_536);
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 65_536 || (!bytes.is_empty() && !bytes.ends_with(b"\n")) {
            return None;
        }
        let mut lines = bytes.split(|b| *b == b'\n');
        if start > 0 {
            lines.next(); // The bounded tail may start inside a row.
        }
        // A truncated tail must reach a pre-launch row to rule out a completion
        // outside the read window. Never infer that boundary from file mtime.
        let mut covers_launch = start == 0;
        for line in lines.filter(|line| !line.is_empty()) {
            let row: serde_json::Value = serde_json::from_slice(line).ok()?;
            if row["type"] == "session_meta" {
                continue;
            }
            let at = DateTime::parse_from_rfc3339(row["timestamp"].as_str()?).ok()?;
            covers_launch |= at < launch;
            if at >= launch
                && row["type"] == "event_msg"
                && row["payload"]["type"] == "task_complete"
            {
                return Some(false);
            }
        }
        Some(covers_launch)
    };
    read().unwrap_or(false)
}

fn prompt_before_first_turn(
    path: Option<&str>,
    launch: DateTime<Utc>,
    probe: impl FnOnce() -> Option<bool>,
) -> (bool, Option<bool>) {
    let pre_turn = no_turn_completed_since(path, launch);
    (pre_turn, pre_turn.then(probe).flatten())
}

struct Seat {
    project: String,
    pane: String,
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
                project: project.into(),
                pane: pane.into(),
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
    // A reserved launch fences a new process. Bookkeeping-only reattachment
    // preserves validated completion when no launch was reserved. Without a
    // transcript, an old completion cannot prove readiness for the current turn.
    let notify_since = record
        .recovery
        .reserved_attachment
        .unwrap_or(if result.authoritative {
            DateTime::<Utc>::UNIX_EPOCH
        } else {
            launch
        });
    let completion = crate::daemon::codex_notify::latest_activity_record_for_session_after(
        notify_path,
        id,
        notify_since.into(),
    )
    .filter(|record| record.ts >= notify_since && record.ts <= now);
    if record.recovery.reserved_attachment.is_some() && completion.is_none() {
        result.authoritative = false;
    }
    // Even an unvalidated completion ends the pre-turn probe. A writer lock
    // alone cannot validate the completion or establish readiness for a later turn.
    let (pre_turn, prompt) = if completion.is_none() {
        let socket_text = socket.to_string_lossy();
        let args = ["-S", &socket_text, "capture-pane", "-p", "-t", pane];
        prompt_before_first_turn(
            result.jsonl_path.as_deref(),
            record.recovery.reserved_attachment.unwrap_or(launch),
            || {
                super::super::process::run_with_timeout_within(
                    "tmux",
                    &args,
                    Duration::from_millis(200),
                )
                .and_then(|text| idle_prompt(&text))
            },
        )
    } else {
        (false, None)
    };
    let notify = completion
        .as_ref()
        .filter(|_| result.jsonl_path.is_some() && result.authoritative);
    let mut guard = SEATS.lock().unwrap_or_else(|e| e.into_inner());
    let seats = guard.get_or_insert_with(HashMap::new);
    seats
        .retain(|_, seat| now.signed_duration_since(seat.scanned) < chrono::Duration::seconds(120));
    let seat = seats.entry(pid).or_insert_with(|| Seat {
        project: project.into(),
        pane: pane.clone(),
        observation: None,
        scanned: now,
    });
    seat.project = project.into();
    seat.pane = pane.clone();
    seat.scanned = now;
    seat.observation = sample(prompt, pre_turn, notify, notify_since, now);
    if let Some(observed) = &seat.observation {
        result.state = observed.state;
        result.authoritative |= observed.source == "notify";
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::codex_notify::{append_event_at, latest_activity_record_for_session_after};

    // Regression: 6398bfa3 (#163), L2 run 4f: a resume must use its launch
    // boundary, while retaining launch_ready until the rebound rollout's first turn.
    #[test]
    #[cfg(target_os = "linux")]
    fn codex_resume_run4f_launch_floor_and_pre_turn_window() {
        // Shares the process PID with resolver tests, not just a binding store.
        let _guard = super::super::codex::CODEX_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let now = Utc::now();
        let launch = now - chrono::Duration::seconds(5);
        fs::write(&path, "{\"type\":\"response_item\"}\n").unwrap();
        let notify = tmp.path().join("notify.jsonl");
        append_event_at(
            &notify,
            r#"{"type":"agent-turn-complete","thread-id":"rebound"}"#,
            launch - chrono::Duration::seconds(1),
        )
        .unwrap();
        let pid = std::process::id();
        let project = tmp.path().to_str().unwrap();
        let record = serde_json::from_value(serde_json::json!({
            "paneId":"%resume-floor","panePid":pid,
            "paneStartTime":crate::platform::process_start_ticks(pid).unwrap().to_string(),
            "tmuxSocket":tmp.path().join("never-probed.sock"),"cli_tool":"codex","project_path":project,
            "attached_at":now,"recovery":{"reserved_attachment":launch}
        })).unwrap();
        let mut result = IdleResult {
            session_id: Some("rebound".into()),
            jsonl_path: Some(path.to_string_lossy().into_owned()),
            authoritative: true,
            ..IdleResult::idle()
        };
        refresh(&mut result, project, pid, &[record], &notify);
        assert!(!result.authoritative);
        assert!(observation(pid, project, Some("%resume-floor")).is_none());
        fs::write(&path, "{\"type\":\"session_meta\"}\n").unwrap();
        let (no_turn, prompt) =
            prompt_before_first_turn(path.to_str(), launch, || idle_prompt(IDLE_PANE));
        let ready = sample(prompt, no_turn, None, launch, now).unwrap();
        assert_eq!(ready.source, "launch_ready");
        invalidate(pid);
    }

    fn resumed_rollout(path: &Path, launch: DateTime<Utc>) {
        use std::io::Write;
        let mut file = fs::File::create(path).unwrap();
        // A sparse prefix forces readiness to inspect the bounded tail.
        file.set_len(70_000).unwrap();
        drop(file);
        file = fs::OpenOptions::new().append(true).open(path).unwrap();
        writeln!(file).unwrap();
        for kind in ["task_started", "task_complete"] {
            writeln!(
                file,
                "{}",
                serde_json::json!({
                    "timestamp": (launch - chrono::Duration::seconds(1)).to_rfc3339(),
                    "type": "event_msg", "payload": {"type": kind}
                })
            )
            .unwrap();
        }
    }

    // Regression: resumed-seat-readiness lane, 6398bfa3 (#163) only admitted
    // new rollouts, deadlocking a resumed TUI that appended to its prior rollout.
    #[test]
    fn codex_resumed_prior_turns_allow_launch_ready() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let launch = Utc::now() - chrono::Duration::seconds(30);
        resumed_rollout(&path, launch);
        let notify_path = tmp.path().join("notify.jsonl");
        append_event_at(
            &notify_path,
            r#"{"type":"agent-turn-complete","thread-id":"resumed"}"#,
            launch - chrono::Duration::seconds(1),
        )
        .unwrap();
        let notify =
            latest_activity_record_for_session_after(&notify_path, "resumed", launch.into())
                .filter(|record| record.ts >= launch);
        assert!(notify.is_none());
        let (pre_turn, prompt) =
            prompt_before_first_turn(path.to_str(), launch, || idle_prompt(IDLE_PANE));
        let observed = sample(prompt, pre_turn, notify.as_ref(), launch, launch)
            .expect("resumed composer must be ready");
        assert_eq!(observed.state, SessionState::Idle);
        assert_eq!(observed.source, "launch_ready");
        assert_eq!(observed.last_observed_at, launch);
    }

    // Regression: resumed-seat-readiness lane, 6398bfa3 (#163): a same-rollout
    // resume must hand readiness to notify after its first post-launch completion.
    #[test]
    fn codex_resumed_completion_hands_off_to_notify() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let launch = Utc::now() - chrono::Duration::seconds(30);
        resumed_rollout(&path, launch);
        let completed = launch + chrono::Duration::seconds(1);
        writeln!(
            fs::OpenOptions::new().append(true).open(&path).unwrap(),
            "{}",
            serde_json::json!({"timestamp": completed.to_rfc3339(), "type": "event_msg",
                "payload": {"type": "task_complete", "turn_id": "first-resumed-turn"}})
        )
        .unwrap();
        let (pre_turn, prompt) = prompt_before_first_turn(path.to_str(), launch, || {
            panic!("completed turn must end the prompt probe")
        });
        assert!(sample(prompt, pre_turn, None, launch, completed).is_none());
        let notify_path = tmp.path().join("notify.jsonl");
        append_event_at(&notify_path,
            r#"{"type":"agent-turn-complete","thread-id":"resumed","turn-id":"first-resumed-turn"}"#,
            completed).unwrap();
        let notify =
            latest_activity_record_for_session_after(&notify_path, "resumed", launch.into())
                .unwrap();
        let observed = sample(prompt, pre_turn, Some(&notify), launch, completed).unwrap();
        assert_eq!(observed.state, SessionState::Idle);
        assert_eq!(observed.source, "notify");
    }

    // Regression: resumed-seat-readiness lane, 6398bfa3 (#163): opening the
    // resumed pre-turn window must preserve busy evidence during the first turn.
    #[test]
    fn codex_resumed_turn_in_progress_is_not_launch_ready() {
        use std::io::Write;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let launch = Utc::now() - chrono::Duration::seconds(30);
        resumed_rollout(&path, launch);
        writeln!(
            fs::OpenOptions::new().append(true).open(&path).unwrap(),
            "{}",
            serde_json::json!({"timestamp": launch.to_rfc3339(), "type": "event_msg",
                "payload": {"type": "task_started"}})
        )
        .unwrap();
        let (pre_turn, prompt) = prompt_before_first_turn(path.to_str(), launch, || {
            idle_prompt(&format!("• Working (1s • esc to interrupt)\n{IDLE_PANE}"))
        });
        if let Some(observed) = sample(prompt, pre_turn, None, launch, launch) {
            assert_eq!(observed.state, SessionState::Active);
            assert_eq!(observed.source, "pane_working");
        }
    }

    // Regression: 36c5da85 collapsed failed/unrecognized captures into pane_working,
    // recreating L2 run 3's onboarding hang and idle→active→idle flap.
    #[test]
    fn codex_unavailable_or_unrecognized_pane_has_no_observation() {
        let now = Utc::now();
        let unknown = "› Ask Codex to do anything\n  local-model";
        for text in [None, Some(""), Some(unknown)] {
            let (no_turn, prompt) =
                prompt_before_first_turn(None, Utc::now(), || text.and_then(idle_prompt));
            assert!(sample(prompt, no_turn, None, now, now).is_none());
        }
    }

    // Regression: 6398bfa3 (#163), L2 run 3: IO flapping blocked onboarding.
    // Pane authority pre-empts IO (including startup); injected samples are unused.
    // Then replay 90 s of settled noise without pane authority to exercise IO.
    #[test]
    fn codex_prompt_replay_is_idle_fresh_and_event_silent_for_90_seconds() {
        use crate::coordination::activity_export::{
            build_member_activity_snapshot, PaneActivityProbe,
        };
        use crate::session_scanner::{
            cache, classification, proc_io, process::ProcessInfo, tmux::parse_tmux_output, CliTool,
            StateChangeCapture, SCANNER_TEST_LOCK,
        };
        let _lock = SCANNER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let capture = StateChangeCapture::install();
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                classification::set_runtime_idle_detector_override(None);
                proc_io::CODEX_TEST_SAMPLE.set(None);
                cache::remove_state_tracker(941_035);
            }
        }
        let _reset = Reset;
        classification::set_runtime_idle_detector_override(Some(|_| IdleResult::idle()));
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_str().unwrap();
        let proc = ProcessInfo {
            pid: 941_035,
            project_path: project.into(),
            tty: "replay".into(),
            args: "codex".into(),
            cli_tool: CliTool::Codex,
        };
        let panes = parse_tmux_output("%replay replay 0 test test");
        let probe = PaneActivityProbe {
            pane_alive: true,
            active_non_shell_process: true,
            ..Default::default()
        };
        let start = std::time::Instant::now();
        let launch = Utc::now();
        for pane_ready in [true, false] {
            let deltas = &proc_io::CODEX_IDLE_RCHAR_DELTAS[if pane_ready { 0 } else { 22 }..];
            let mut rchar = 1000;
            for (tick, delta) in deltas.iter().cycle().take(180).enumerate() {
                rchar += delta;
                let at = start + Duration::from_millis((tick as u64 + 1) * 500);
                proc_io::CODEX_TEST_SAMPLE.set(Some((proc.pid, rchar, at)));
                let now = Utc::now();
                if pane_ready {
                    let o = sample(idle_prompt(IDLE_PANE), true, None, launch, now).unwrap();
                    seed_observation_for_test(proc.pid, project, "%replay", o.source, o.state, now);
                } else {
                    invalidate(proc.pid);
                }
                let (sessions, _, _, _) = classification::classify_display_runtime_sessions_with(
                    vec![proc.clone()],
                    panes.clone(),
                    &HashMap::new(),
                    &|_| IdleResult::idle(),
                );
                assert_eq!(sessions[0].state, SessionState::Idle, "tick {tick}");
                assert!(!sessions[0].recent_io);
                if pane_ready {
                    let display = sessions[0].clone().into();
                    let snapshot = build_member_activity_snapshot(Some(&display), &probe, now);
                    let value = serde_json::to_value(snapshot).unwrap();
                    assert_eq!(value["activity_confidence"], "idle");
                    assert_eq!(value["observed_at"], now.to_rfc3339());
                    assert_eq!(value["source"], "launch_ready");
                }
            }
        }
        assert!(capture.transitions_for(proc.pid).is_empty());
        invalidate(proc.pid);
    }

    // Regression: 6398bfa3 (#163), L2 run 3: a composer alone also appears
    // during startup/working; readiness requires the loaded footer and no spinner.
    #[test]
    fn codex_prompt_rejects_busy_and_incomplete_composer() {
        // Regression: 36c5da85 treated banner prose/model names/directories as busy.
        for text in [
            IDLE_PANE.to_owned(),
            format!("Tip: Try thinking hard while working.\n⚠ --dangerously-bypass-hook-trust is enabled.\n{IDLE_PANE}"),
            IDLE_PANE.replace("gpt-5.6-luna", "local-model"),
            IDLE_PANE.replace("<scratch>/project", "/tmp/thinking-working"),
        ] {
            assert_eq!(idle_prompt(&text), Some(true));
        }
        for status in [
            "• Working (1s • esc to interrupt)",
            "⠋ Thinking",
            "⠙",
            "• Booting MCP server: codex_apps (0s • esc to interrupt)",
        ] {
            let now = Utc::now();
            let prompt = idle_prompt(&format!("{status}\n{IDLE_PANE}"));
            let observed = sample(prompt, true, None, now, now).unwrap();
            assert_eq!(observed.state, SessionState::Active);
            assert_eq!(observed.source, "pane_working");
        }
        assert_eq!(idle_prompt("› Ask Codex\n  ? for shortcuts"), None);
        // Regression: review of 36c5da85 — a tail without the composer and without busy
        // chrome was asserted as authoritative working with nothing to time it out.
        assert_eq!(idle_prompt("gpt-5.6-luna low · /tmp"), None);
        assert_eq!(
            idle_prompt("• Working (esc to interrupt)\n› \n  gpt-5.6-luna low · /tmp"),
            Some(false)
        );
    }

    // Regression: b9e4a855's attachment floor hid a completion that the bound
    // transcript had already validated, then raw IO cleared its idle authority.
    #[cfg(target_os = "linux")]
    #[test]
    fn codex_round3_reattachment_preserves_validated_completion() {
        let _guard = super::super::codex::CODEX_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        // Regression: 6398bfa3 omitted CODEX_TEST_LOCK while using the test
        // process PID. A parallel resolver could invalidate this exact seat.
        std::thread::spawn(move || {
            if let Ok(_guard) = super::super::codex::CODEX_TEST_LOCK.try_lock() {
                invalidate(pid);
            }
        })
        .join()
        .unwrap();
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
        let record = crate::daemon::codex_notify::CodexNotifyRecord {
            ts: completed,
            session_id: Some("seat".into()),
            event: "agent-turn-complete".into(),
            turn_id: None,
        };
        sample(Some(false), false, None, launch, launch);
        // Establish idle after completion before a later keep-alive read.
        let idle = sample(
            None,
            false,
            Some(&record),
            launch,
            completed + chrono::Duration::seconds(1),
        )
        .unwrap();
        assert_eq!(idle.state, SessionState::Idle);
        for seconds in [2, 3, 30] {
            let now = completed + chrono::Duration::seconds(seconds);
            let observed = sample(Some(false), false, Some(&record), launch, now).unwrap();
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
            prompt_before_first_turn(rollout.to_str(), Utc::now(), || panic!(
                "must not capture a pane after a turn"
            )),
            (false, None)
        );
        assert_eq!(
            prompt_before_first_turn(None, Utc::now(), || Some(true)),
            (true, Some(true))
        );
    }

    // Regression: b9e4a855 treated every unknown notification as authoritative working.
    #[test]
    fn codex_review_unknown_notify_is_not_working() {
        let launch = Utc::now() - chrono::Duration::seconds(20);
        for event in ["unknown", "", "future-event", "agent-turn-started"] {
            let record = crate::daemon::codex_notify::CodexNotifyRecord {
                ts: launch,
                session_id: Some("seat".into()),
                event: event.into(),
                turn_id: None,
            };
            let result = sample(Some(true), true, Some(&record), launch, Utc::now()).unwrap();
            assert_eq!(result.source, "launch_ready");
            assert_eq!(result.state, SessionState::Idle);
        }
    }

    // Regression: 664feab6 attributed a pre-rollout seat but left its activity
    // uncertain, so team-owned delivery could never send its first card.
    #[test]
    fn codex_launch_ready_prompt_and_notify_handoff() {
        let tmp = tempfile::tempdir().unwrap();
        let notify = tmp.path().join("notify.jsonl");
        let launch = Utc::now() - chrono::Duration::seconds(30);
        let ready = launch;
        let poll = |record: Option<&crate::daemon::codex_notify::CodexNotifyRecord>, at| {
            sample(Some(true), true, record, launch, at)
        };
        let idle = poll(None, ready).unwrap();
        assert_eq!(idle.state, SessionState::Idle);
        assert_eq!(idle.source, "launch_ready");
        assert_eq!(idle.last_observed_at, ready);
        let now = Utc::now();
        assert_eq!(poll(None, now).unwrap().last_observed_at, now);
        assert_eq!(poll(None, now).unwrap().state, SessionState::Idle);
        let raw =
            serde_json::json!({"type": "agent-turn-complete", "thread-id": "seat"}).to_string();
        append_event_at(&notify, &raw, now).unwrap();
        let record =
            latest_activity_record_for_session_after(&notify, "seat", launch.into()).unwrap();
        let observed = poll(Some(&record), now + chrono::Duration::seconds(11)).unwrap();
        assert_eq!(observed.state, SessionState::Idle);
        assert_eq!(observed.source, "notify");
        // Regression: b9e4a855 timestamped an IO delta at the end of its
        // interval, hiding a completion that landed between those two polls.
        let later = now + chrono::Duration::seconds(20);
        assert_eq!(poll(None, later).unwrap().state, SessionState::Idle);
        let record = crate::daemon::codex_notify::CodexNotifyRecord {
            ts: now + chrono::Duration::seconds(15),
            session_id: Some("seat".into()),
            event: "agent-turn-complete".into(),
            turn_id: None,
        };
        assert_eq!(
            poll(Some(&record), later).unwrap().state,
            SessionState::Idle
        );
    }

    // Regression: 664feab6 must not turn unresolved rollout evidence into readiness.
    #[test]
    fn codex_launch_ready_requires_no_completed_turn_since_launch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("rollout.jsonl");
        let now = Utc::now();
        assert!(!no_turn_completed_since(path.to_str(), now));
        fs::write(&path, "not-json").unwrap();
        assert!(!no_turn_completed_since(path.to_str(), now));
        fs::write(&path, "{\"type\":\"session_meta\"}\n").unwrap();
        assert!(no_turn_completed_since(path.to_str(), now));
        // Regression: resumed-seat-readiness lane, 6398bfa3 (#163) treated
        // historical turns as current; only completions at/after launch close it.
        for seconds in [-1, 0, 1] {
            fs::write(
                &path,
                format!(
                    "{}\n",
                    serde_json::json!({
                        "timestamp": (now + chrono::Duration::seconds(seconds)).to_rfc3339(),
                        "type": "event_msg", "payload": {"type": "task_complete"}
                    })
                ),
            )
            .unwrap();
            assert_eq!(no_turn_completed_since(path.to_str(), now), seconds < 0);
        }
        for row in [
            "{\"type\":\"response_item\"}\n",
            "{\"type\":\"event_msg\",\"timestamp\":\"invalid\"}\n",
            "{\"type\":\"session_meta\"}\n{",
        ] {
            fs::write(&path, row).unwrap();
            assert!(!no_turn_completed_since(path.to_str(), now));
        }
        assert!(sample(Some(true), false, None, now, now).is_none());
    }
}
