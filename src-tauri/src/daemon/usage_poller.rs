use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex, Once};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Map, Value};

use crate::session_scanner::accounts::{
    self, Account, ReqwestHttpClient, UsageSnapshot, UsageStatus,
};
use crate::session_scanner::cli_tool::{spec, CliTool};

#[derive(Default)]
struct Entry {
    snapshot: Option<UsageSnapshot>,
    next_due: Option<Instant>,
    failures: u32,
    in_flight: bool,
    unauthorized_mtime: Option<Option<SystemTime>>,
    failure_state: Option<&'static str>,
}

#[derive(Default)]
struct PollerState {
    entries: HashMap<(CliTool, String), Entry>,
    refreshed: HashMap<CliTool, Instant>,
}

#[derive(Default)]
struct Poller {
    state: Mutex<PollerState>,
    wake: Condvar,
}

impl Poller {
    fn wait_for_due_tools(&self) -> Vec<CliTool> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            let now = Instant::now();
            let due = state
                .entries
                .iter()
                .filter(|(_, entry)| {
                    !entry.in_flight && entry.next_due.is_some_and(|next| next <= now)
                })
                .map(|((tool, _), _)| *tool)
                .collect::<HashSet<_>>();
            if !due.is_empty() {
                return due.into_iter().collect();
            }

            let next = state
                .entries
                .values()
                .filter(|entry| !entry.in_flight)
                .filter_map(|entry| entry.next_due)
                .min();
            state = match next {
                Some(next) => {
                    let wait = next.saturating_duration_since(Instant::now());
                    self.wake
                        .wait_timeout(state, wait)
                        .unwrap_or_else(|error| error.into_inner())
                        .0
                }
                None => self
                    .wake
                    .wait(state)
                    .unwrap_or_else(|error| error.into_inner()),
            };
        }
    }

    #[cfg(test)]
    fn schedule_for_test(&self, tool: CliTool, delay: Duration) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state
            .entries
            .entry((tool, "test".to_string()))
            .or_default()
            .next_due = Some(Instant::now() + delay);
        drop(state);
        self.wake.notify_one();
    }
}

static POLLER: LazyLock<Poller> = LazyLock::new(Poller::default);
static POLL_SERIAL: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static SCHEDULER: Once = Once::new();

pub fn attach_usage(tool: CliTool, accounts: &mut [Account]) {
    if spec(tool).usage_provider().is_none() {
        return;
    }
    ensure_scheduler();
    let now = Instant::now();
    let mut state = POLLER
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for account in accounts.iter_mut() {
        let entry = state.entries.entry((tool, account.id.clone())).or_default();
        account.usage = entry.snapshot.clone();
        if entry.next_due.is_none() {
            entry.next_due = Some(now);
        }
    }
    drop(state);
    POLLER.wake.notify_one();
}

/// On-demand refresh, debounced per tool for five seconds.
pub fn refresh(tool: CliTool) -> bool {
    if spec(tool).usage_provider().is_none() {
        return false;
    }
    ensure_scheduler();
    let now = Instant::now();
    let mut state = POLLER
        .state
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let force = state
        .refreshed
        .get(&tool)
        .is_none_or(|last| now.duration_since(*last) >= Duration::from_secs(5));
    if force {
        state.refreshed.insert(tool, now);
    }
    drop(state);

    match run_on_poller_thread(move || {
        let _serial = POLL_SERIAL
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        poll(tool, force);
    }) {
        Ok(()) => force,
        Err(error) => {
            tracing::warn!(tool = %tool, error = %error, "Account usage refresh thread failed");
            false
        }
    }
}

fn ensure_scheduler() {
    if cfg!(test) {
        return;
    }
    SCHEDULER.call_once(|| {
        if let Err(error) = std::thread::Builder::new()
            .name("account-usage-poller".to_string())
            .spawn(|| loop {
                for tool in POLLER.wait_for_due_tools() {
                    if spec(tool).usage_provider().is_some() {
                        let _serial = POLL_SERIAL
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        poll(tool, false);
                    }
                }
            })
        {
            tracing::warn!(error = %error, "Account usage scheduler failed to start");
        }
    });
}

fn run_on_poller_thread<F, T>(job: F) -> Result<T, String>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name("account-usage-refresh".to_string())
        .spawn(job)
        .map_err(|error| error.to_string())?
        .join()
        .map_err(|_| "account usage refresh thread panicked".to_string())
}

fn poll(tool: CliTool, force: bool) {
    let tool_spec = spec(tool);
    let Some(provider) = tool_spec.usage_provider() else {
        return;
    };
    let account_provider = tool_spec.account_provider();
    let accounts = accounts::detect(tool);
    let account_ids = accounts
        .iter()
        .map(|account| account.id.as_str())
        .collect::<HashSet<_>>();
    {
        let mut state = POLLER
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.entries.retain(|(entry_tool, account_id), _| {
            *entry_tool != tool || account_ids.contains(account_id.as_str())
        });
    }
    let live_dirs = crate::session_scanner::latest_compaction_runtime_sessions()
        .into_iter()
        .filter(|session| session.cli_tool == tool)
        .filter_map(|session| session.jsonl_path)
        .filter_map(|path| {
            account_provider.and_then(|provider| provider.session_dir(Path::new(&path)))
        })
        .collect::<Vec<_>>();

    for account in accounts {
        let key = (tool, account.id.clone());
        let credential_mtime = provider
            .credential_path(&account.dir)
            .as_deref()
            .and_then(credential_mtime);
        {
            let mut state = POLLER
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let entry = state.entries.entry(key.clone()).or_default();
            if entry.in_flight || (!force && entry.next_due.is_some_and(|due| due > Instant::now()))
            {
                continue;
            }
            if entry
                .unauthorized_mtime
                .is_some_and(|observed| observed == credential_mtime)
            {
                entry.next_due = Some(Instant::now() + Duration::from_secs(600));
                drop(state);
                POLLER.wake.notify_one();
                continue;
            }
            entry.in_flight = true;
        }

        let mut snapshot = provider.fetch(&account.dir, &ReqwestHttpClient);
        let live = live_dirs.iter().any(|dir| same_path(dir, &account.dir));
        let mut state = POLLER
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let entry = state.entries.entry(key).or_default();
        entry.in_flight = false;
        let failed = snapshot.status == UsageStatus::Stale;
        if failed && snapshot.windows.is_empty() {
            if let Some(previous) = entry.snapshot.as_ref() {
                snapshot.windows = previous.windows.clone();
            }
        }
        entry.failures = if failed {
            entry.failures.saturating_add(1)
        } else {
            0
        };
        entry.unauthorized_mtime =
            (snapshot.status == UsageStatus::Unauthorized).then_some(credential_mtime);
        let cadence = if failed {
            failure_backoff(entry.failures)
        } else if live {
            Duration::from_secs(60)
        } else {
            Duration::from_secs(600)
        };
        entry.next_due = Some(Instant::now() + cadence);
        let failure_state = failed.then_some("stale");
        if failure_state != entry.failure_state {
            emit_result(tool, &account.id, &snapshot, failed);
            entry.failure_state = failure_state;
        } else if !failed {
            emit_result(tool, &account.id, &snapshot, false);
        }
        entry.snapshot = Some(snapshot);
        drop(state);
        POLLER.wake.notify_one();
    }
}

fn failure_backoff(failures: u32) -> Duration {
    Duration::from_secs(
        (60_u64.saturating_mul(1_u64 << failures.saturating_sub(1).min(3))).min(300),
    )
}

fn credential_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn same_path(left: &Path, right: &Path) -> bool {
    std::fs::canonicalize(left).unwrap_or_else(|_| PathBuf::from(left))
        == std::fs::canonicalize(right).unwrap_or_else(|_| PathBuf::from(right))
}

fn emit_result(tool: CliTool, account_id: &str, snapshot: &UsageSnapshot, failed: bool) {
    let mut fields = Map::new();
    fields.insert("tool".to_string(), Value::String(tool.to_string()));
    fields.insert(
        "account_id".to_string(),
        Value::String(account_id.to_string()),
    );
    if failed {
        fields.insert("kind".to_string(), Value::String("stale".to_string()));
        crate::commands::logging::emit_global(
            "warn",
            "usage",
            "usage.failed",
            Some("Account usage fetch failed".to_string()),
            fields,
        );
    } else {
        fields.insert(
            "status".to_string(),
            Value::String(format!("{:?}", snapshot.status).to_ascii_lowercase()),
        );
        fields.insert(
            "windows".to_string(),
            Value::from(snapshot.windows.len() as u64),
        );
        crate::commands::logging::emit_global(
            "debug",
            "usage",
            "usage.fetched",
            Some("Account usage fetched".to_string()),
            fields,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_back_off_from_one_minute_to_five() {
        // Regression: a574720 polled bridge files on a fixed cadence; network
        // failures must not create a retry storm after moving to live HTTP.
        assert_eq!(failure_backoff(1), Duration::from_secs(60));
        assert_eq!(failure_backoff(2), Duration::from_secs(120));
        assert_eq!(failure_backoff(9), Duration::from_secs(300));
    }

    #[test]
    fn scheduler_wakes_when_next_due_arrives_without_another_request() {
        // Regression: c11770e calculated `next_due` but only started a poll
        // from list/refresh requests, so an idle UI stopped polling forever.
        let poller = std::sync::Arc::new(Poller::default());
        poller.schedule_for_test(CliTool::Claude, Duration::from_millis(20));
        let waiting = std::sync::Arc::clone(&poller);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || tx.send(waiting.wait_for_due_tools()).unwrap());

        assert_eq!(
            rx.recv_timeout(Duration::from_millis(500)).unwrap(),
            vec![CliTool::Claude]
        );
    }

    #[test]
    fn requested_refresh_waits_for_the_poller_thread() {
        // Regression: c11770e detached the requested fetch, then let the same
        // interaction list the previous snapshot before HTTP could complete.
        let before = Instant::now();
        let worker_name = run_on_poller_thread(|| {
            std::thread::sleep(Duration::from_millis(30));
            std::thread::current().name().map(str::to_string)
        })
        .expect("poller thread");

        assert!(before.elapsed() >= Duration::from_millis(30));
        assert_eq!(worker_name.as_deref(), Some("account-usage-refresh"));
    }

    #[test]
    fn unauthorized_state_remembers_a_missing_credential_file() {
        // Regression: c11770e represented both "not unauthorized" and an
        // unauthorized missing credential file as None, defeating the mtime
        // pause and spawning a poll thread on every account listing.
        let entry = Entry {
            unauthorized_mtime: Some(None),
            ..Default::default()
        };

        assert!(entry
            .unauthorized_mtime
            .is_some_and(|observed| observed.is_none()));
    }
}
