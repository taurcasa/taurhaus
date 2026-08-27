use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
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
    unauthorized_mtime: Option<SystemTime>,
    failure_state: Option<&'static str>,
}

static STATE: LazyLock<Mutex<HashMap<(CliTool, String), Entry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static REFRESHED: LazyLock<Mutex<HashMap<CliTool, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn attach_usage(tool: CliTool, accounts: &mut [Account]) {
    let now = Instant::now();
    let mut due = false;
    {
        let state = STATE.lock().unwrap_or_else(|error| error.into_inner());
        for account in accounts.iter_mut() {
            if let Some(entry) = state.get(&(tool, account.id.clone())) {
                account.usage = entry.snapshot.clone();
                due |= !entry.in_flight && entry.next_due.is_none_or(|next| next <= now);
            } else {
                due = true;
            }
        }
    }
    if due {
        spawn_poll(tool, false);
    }
}

/// On-demand refresh, debounced per tool for five seconds.
pub fn refresh(tool: CliTool) -> bool {
    let now = Instant::now();
    let mut refreshed = REFRESHED.lock().unwrap_or_else(|error| error.into_inner());
    if refreshed
        .get(&tool)
        .is_some_and(|last| now.duration_since(*last) < Duration::from_secs(5))
    {
        return false;
    }
    refreshed.insert(tool, now);
    drop(refreshed);
    spawn_poll(tool, true);
    true
}

fn spawn_poll(tool: CliTool, force: bool) {
    if cfg!(test) {
        // Unit tests exercise providers with injected clients. Never let
        // account listing fall through to the production HTTP client.
        return;
    }
    if spec(tool).usage_provider().is_none() {
        return;
    }
    std::thread::spawn(move || poll(tool, force));
}

fn poll(tool: CliTool, force: bool) {
    let tool_spec = spec(tool);
    let Some(provider) = tool_spec.usage_provider() else {
        return;
    };
    let account_provider = tool_spec.account_provider();
    let accounts = accounts::detect(tool);
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
        let credential_mtime = credential_mtime(&account.dir);
        {
            let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
            let entry = state.entry(key.clone()).or_default();
            if entry.in_flight || (!force && entry.next_due.is_some_and(|due| due > Instant::now()))
            {
                continue;
            }
            if entry.unauthorized_mtime.is_some() && entry.unauthorized_mtime == credential_mtime {
                continue;
            }
            entry.in_flight = true;
        }

        let mut snapshot = provider.fetch(&account.dir, &ReqwestHttpClient);
        let live = live_dirs.iter().any(|dir| same_path(dir, &account.dir));
        let mut state = STATE.lock().unwrap_or_else(|error| error.into_inner());
        let entry = state.entry(key).or_default();
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
        entry.unauthorized_mtime = (snapshot.status == UsageStatus::Unauthorized)
            .then_some(credential_mtime)
            .flatten();
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
    }
}

fn failure_backoff(failures: u32) -> Duration {
    Duration::from_secs(
        (60_u64.saturating_mul(1_u64 << failures.saturating_sub(1).min(3))).min(300),
    )
}

fn credential_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir.join(".credentials.json"))
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
}
