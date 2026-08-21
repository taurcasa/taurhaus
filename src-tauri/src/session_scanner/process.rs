//! Process scanner — find CLI tool processes from the platform process inventory.
//!
//! Linux reads `/proc/*/cmdline` directly; other Unix platforms run `ps`;
//! Windows has no native inventory (sessions are scanned through the WSL
//! daemon). The inventory is fail-soft: when it cannot be read, the last good
//! inventory is reported with `ProcessScan::degraded` set so callers keep
//! their state instead of treating the gap as "no sessions".

use std::io::{self, Read};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use super::cli_tool::CliTool;
use crate::platform::apply_background_command_settings;

/// Information about a running CLI tool process.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub project_path: String,
    pub tty: String,
    pub args: String,
    pub cli_tool: CliTool,
}

/// Result of one process inventory scan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessScan {
    pub processes: Vec<ProcessInfo>,
    /// The inventory could not be read this cycle. `processes` then carries the
    /// last good inventory and must not be used to prune or retire state.
    pub degraded: bool,
}

/// Last successfully read inventory, reported verbatim on degraded scans.
static LAST_GOOD_INVENTORY: Mutex<Vec<ProcessInfo>> = Mutex::new(Vec::new());

/// Name of the inventory source, for the degraded/recovered events.
#[cfg(target_os = "linux")]
const INVENTORY_SOURCE: &str = "proc";
#[cfg(all(unix, not(target_os = "linux")))]
const INVENTORY_SOURCE: &str = "ps";
#[cfg(not(unix))]
const INVENTORY_SOURCE: &str = "none";

/// Test seam: replaces the enriched platform inventory read so the real
/// scanner wiring can be driven through healthy and failed reads.
#[cfg(test)]
pub(crate) type InventoryProvider = fn() -> Option<Vec<ProcessInfo>>;
#[cfg(test)]
static INVENTORY_PROVIDER_OVERRIDE: Mutex<Option<InventoryProvider>> = Mutex::new(None);

#[cfg(test)]
pub(crate) fn set_inventory_provider_override(provider: Option<InventoryProvider>) {
    *INVENTORY_PROVIDER_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = provider;
}

#[cfg(test)]
fn inventory_provider_override() -> Option<InventoryProvider> {
    *INVENTORY_PROVIDER_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Scan supported CLI tool processes and enrich them from the platform.
///
/// Returns one `ProcessInfo` per detected tool process. Gracefully skips
/// processes that disappear between the inventory read and the enrichment.
/// When the inventory itself cannot be read, returns the last good inventory
/// with `degraded` set.
pub fn scan_processes() -> ProcessScan {
    let fresh = read_enriched_inventory();
    let mut last_good = LAST_GOOD_INVENTORY
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    resolve_process_scan(fresh, &mut last_good)
}

/// Fold a fresh inventory read into the last good inventory.
fn resolve_process_scan(
    fresh: Option<Vec<ProcessInfo>>,
    last_good: &mut Vec<ProcessInfo>,
) -> ProcessScan {
    match fresh {
        Some(processes) => {
            *last_good = processes.clone();
            ProcessScan {
                processes,
                degraded: false,
            }
        }
        None => ProcessScan {
            processes: last_good.clone(),
            degraded: true,
        },
    }
}

/// One fresh inventory read, enriched with cwd/tty; `None` when unreadable.
fn read_enriched_inventory() -> Option<Vec<ProcessInfo>> {
    #[cfg(test)]
    if let Some(provider) = inventory_provider_override() {
        return note_inventory_health(provider());
    }

    let entries = list_cli_tool_processes()?;
    Some(
        entries
            .into_iter()
            .filter_map(|(pid, args, tool)| enrich_from_proc(pid, args, tool))
            .collect(),
    )
}

/// Scan only detected CLI-tool PIDs (cheap fingerprint for cache checks).
///
/// Returns `None` when the inventory could not be read.
pub fn scan_process_ids() -> Option<Vec<u32>> {
    #[cfg(test)]
    if let Some(provider) = inventory_provider_override() {
        let processes = note_inventory_health(provider())?;
        let mut pids: Vec<u32> = processes.into_iter().map(|process| process.pid).collect();
        pids.sort_unstable();
        return Some(pids);
    }

    let mut pids: Vec<u32> = list_cli_tool_processes()?
        .into_iter()
        .map(|(pid, _, _)| pid)
        .collect();
    pids.sort_unstable();
    Some(pids)
}

const PID_FINGERPRINT_CACHE_TTL: Duration = Duration::from_secs(2);

#[derive(Default)]
struct PidFingerprintCache {
    pids: Vec<u32>,
    proc_count: Option<usize>,
    scanned_at: Option<Instant>,
}

static PID_FINGERPRINT_CACHE: OnceLock<Mutex<PidFingerprintCache>> = OnceLock::new();

/// Scan detected CLI-tool PIDs with short caching when overall process count is stable.
///
/// Returns `None` when the inventory could not be read; the cache is left
/// untouched so the next call retries.
pub fn scan_process_ids_cached() -> Option<Vec<u32>> {
    #[cfg(test)]
    if inventory_provider_override().is_some() {
        // The injected provider is the inventory; no count-based short-circuit.
        return scan_process_ids();
    }

    let now = Instant::now();
    let proc_count = system_process_count();
    let cache = PID_FINGERPRINT_CACHE.get_or_init(|| Mutex::new(PidFingerprintCache::default()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());

    let cache_fresh = guard
        .scanned_at
        .is_some_and(|ts| now.duration_since(ts) < PID_FINGERPRINT_CACHE_TTL);
    let same_proc_count = proc_count.is_some() && guard.proc_count == proc_count;
    if cache_fresh && same_proc_count {
        return Some(guard.pids.clone());
    }

    let pids = scan_process_ids()?;
    guard.pids = pids.clone();
    guard.proc_count = proc_count;
    guard.scanned_at = Some(now);
    Some(pids)
}

/// Count live process entries for cache invalidation.
#[cfg(target_os = "linux")]
fn system_process_count() -> Option<usize> {
    let entries = std::fs::read_dir("/proc").ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .chars()
                    .all(|c| c.is_ascii_digit())
            })
            .count(),
    )
}

/// Count live process entries for cache invalidation (macOS and other non-Linux Unix).
#[cfg(all(unix, not(target_os = "linux")))]
fn system_process_count() -> Option<usize> {
    let output = run_with_timeout("ps", &["-A", "-o", "pid="])?;
    Some(
        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
    )
}

/// Windows has no native inventory; process inspection routes through the WSL daemon.
#[cfg(not(unix))]
fn system_process_count() -> Option<usize> {
    None
}

/// Read the raw CLI-tool inventory as `(pid, args, cli_tool)` tuples.
///
/// Returns `None` when the platform inventory is unavailable; the degraded /
/// recovered transition events are emitted from here.
fn list_cli_tool_processes() -> Option<Vec<(u32, String, CliTool)>> {
    note_inventory_health(read_cli_tool_inventory())
}

/// Detect CLI tools from `/proc/*/cmdline`.
#[cfg(target_os = "linux")]
fn read_cli_tool_inventory() -> Option<Vec<(u32, String, CliTool)>> {
    let processes = crate::platform::list_processes()?;
    Some(
        processes
            .into_iter()
            .filter_map(|(pid, args)| {
                let tool = detect_cli_tool(&args)?;
                Some((pid, args, tool))
            })
            .collect(),
    )
}

/// Detect CLI tools from `ps -eo pid,args` (macOS and other non-Linux Unix).
#[cfg(all(unix, not(target_os = "linux")))]
fn read_cli_tool_inventory() -> Option<Vec<(u32, String, CliTool)>> {
    let output = run_with_timeout("ps", &["-eo", "pid,args"])?;
    Some(parse_ps_output(&output))
}

/// Windows has no native inventory (CLI tools run inside WSL2); the session
/// scan goes through the WSL daemon and this local read is always degraded.
#[cfg(not(unix))]
fn read_cli_tool_inventory() -> Option<Vec<(u32, String, CliTool)>> {
    None
}

/// Bounded reminder cadence while the inventory stays unreadable.
const DEGRADED_REMINDER_INTERVAL: Duration = Duration::from_secs(60);

/// Edge-triggered health of the inventory source: one `degraded` event on
/// entry, a bounded periodic reminder while it lasts, one `recovered` on exit.
struct InventoryHealth {
    degraded_since: Option<Instant>,
    failed_reads: u64,
    last_emitted_at: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
enum InventoryHealthEvent {
    Degraded {
        failed_reads: u64,
        degraded_for: Duration,
    },
    Recovered {
        failed_reads: u64,
        degraded_for: Duration,
    },
}

impl InventoryHealth {
    const fn new() -> Self {
        Self {
            degraded_since: None,
            failed_reads: 0,
            last_emitted_at: None,
        }
    }

    fn note(&mut self, readable: bool, now: Instant) -> Option<InventoryHealthEvent> {
        match (readable, self.degraded_since) {
            (true, None) => None,
            (true, Some(since)) => {
                let event = InventoryHealthEvent::Recovered {
                    failed_reads: self.failed_reads,
                    degraded_for: now.duration_since(since),
                };
                *self = Self::new();
                Some(event)
            }
            (false, None) => {
                self.degraded_since = Some(now);
                self.failed_reads = 1;
                self.last_emitted_at = Some(now);
                Some(InventoryHealthEvent::Degraded {
                    failed_reads: 1,
                    degraded_for: Duration::ZERO,
                })
            }
            (false, Some(since)) => {
                self.failed_reads += 1;
                let reminder_due = self
                    .last_emitted_at
                    .is_none_or(|at| now.duration_since(at) >= DEGRADED_REMINDER_INTERVAL);
                if !reminder_due {
                    return None;
                }
                self.last_emitted_at = Some(now);
                Some(InventoryHealthEvent::Degraded {
                    failed_reads: self.failed_reads,
                    degraded_for: now.duration_since(since),
                })
            }
        }
    }
}

static INVENTORY_HEALTH: Mutex<InventoryHealth> = Mutex::new(InventoryHealth::new());

/// Record one inventory read's outcome and emit the transition events.
fn note_inventory_health<T>(read: Option<T>) -> Option<T> {
    let event = INVENTORY_HEALTH
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .note(read.is_some(), Instant::now());
    match event {
        Some(InventoryHealthEvent::Degraded {
            failed_reads,
            degraded_for,
        }) => emit_process_scan_degraded(failed_reads, degraded_for),
        Some(InventoryHealthEvent::Recovered {
            failed_reads,
            degraded_for,
        }) => emit_process_scan_recovered(failed_reads, degraded_for),
        None => {}
    }
    read
}

fn inventory_health_fields(failed_reads: u64, degraded_for: Duration) -> Map<String, Value> {
    let previous_inventory = LAST_GOOD_INVENTORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len();
    let mut fields = Map::new();
    fields.insert(
        "source".to_string(),
        Value::String(INVENTORY_SOURCE.to_string()),
    );
    fields.insert(
        "previous_inventory".to_string(),
        Value::Number(serde_json::Number::from(previous_inventory as u64)),
    );
    fields.insert(
        "failed_reads".to_string(),
        Value::Number(serde_json::Number::from(failed_reads)),
    );
    fields.insert(
        "degraded_ms".to_string(),
        Value::Number(serde_json::Number::from(degraded_for.as_millis() as u64)),
    );
    fields
}

fn emit_process_scan_degraded(failed_reads: u64, degraded_for: Duration) {
    let fields = inventory_health_fields(failed_reads, degraded_for);
    tracing::warn!(
        source = INVENTORY_SOURCE,
        failed_reads,
        degraded_ms = degraded_for.as_millis() as u64,
        "process inventory unavailable; keeping previous inventory"
    );
    crate::commands::logging::emit_global(
        "warn",
        "backend",
        "session_scanner.process_scan.degraded",
        Some("Process inventory unavailable; keeping previous inventory".to_string()),
        fields,
    );
}

fn emit_process_scan_recovered(failed_reads: u64, degraded_for: Duration) {
    let fields = inventory_health_fields(failed_reads, degraded_for);
    tracing::info!(
        source = INVENTORY_SOURCE,
        failed_reads,
        degraded_ms = degraded_for.as_millis() as u64,
        "process inventory readable again"
    );
    crate::commands::logging::emit_global(
        "info",
        "backend",
        "session_scanner.process_scan.recovered",
        Some("Process inventory readable again".to_string()),
        fields,
    );
}

/// Timeout for subprocess execution. If `ps` or similar hangs (e.g. stale
/// NFS mount affecting `/proc`), we bail out instead of blocking forever.
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(2);

/// Spawn a subprocess, drain its stdout concurrently, and wait with a timeout.
///
/// Returns the stdout as a string on success. Returns `None` if the command
/// fails to spawn, its stdout cannot be drained (thread spawn or read error),
/// exits with error, or exceeds `SUBPROCESS_TIMEOUT`. On timeout, the child
/// process is killed. A partial read is never reported as success.
///
/// Stdout is drained on a separate thread while the child runs: a child whose
/// output exceeds the pipe buffer would otherwise block on write and never
/// exit within the budget.
pub(super) fn run_with_timeout(cmd: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(cmd);
    apply_background_command_settings(&mut command);
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + SUBPROCESS_TIMEOUT;

    let Some(mut stdout) = child.stdout.take() else {
        kill_and_reap(&mut child);
        return None;
    };
    let (output_tx, output_rx) = mpsc::channel::<io::Result<Vec<u8>>>();
    let drain = std::thread::Builder::new()
        .name("scanner-stdout-drain".to_string())
        .spawn(move || {
            let mut buf = Vec::new();
            let result = stdout.read_to_end(&mut buf).map(|_| buf);
            let _ = output_tx.send(result);
        });
    if let Err(error) = drain {
        kill_and_reap(&mut child);
        tracing::warn!(cmd, %error, "Could not spawn stdout drain thread");
        return None;
    }

    let output = match output_rx.recv_timeout(SUBPROCESS_TIMEOUT) {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            kill_and_reap(&mut child);
            tracing::warn!(cmd, %error, "Subprocess stdout read failed");
            return None;
        }
        Err(_) => {
            kill_and_reap(&mut child);
            tracing::warn!(cmd, "Subprocess timed out after {SUBPROCESS_TIMEOUT:?}");
            return None;
        }
    };

    // Stdout reached EOF; reap the child within the remaining budget.
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return status
                    .success()
                    .then(|| String::from_utf8_lossy(&output).into_owned());
            }
            Ok(None) if Instant::now() >= deadline => {
                kill_and_reap(&mut child);
                tracing::warn!(cmd, "Subprocess timed out after {SUBPROCESS_TIMEOUT:?}");
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Err(_) => {
                kill_and_reap(&mut child);
                return None;
            }
        }
    }
}

fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Parse `ps -eo pid,args` output into (pid, args, cli_tool) tuples for detected CLI tools.
pub fn parse_ps_output(output: &str) -> Vec<(u32, String, CliTool)> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // Split into PID and rest (args)
            let (pid_str, args) = line.split_once(char::is_whitespace)?;
            let args = args.trim();

            let tool = detect_cli_tool(args)?;
            let pid: u32 = pid_str.parse().ok()?;
            Some((pid, args.to_string(), tool))
        })
        .collect()
}

/// Detect which CLI tool a process belongs to from its command line args.
///
/// Returns `Some(CliTool)` if the args match a known CLI tool, `None` otherwise.
///
/// Matches:
/// - **Codex**: `codex`, `/path/to/codex`
/// - **Claude**: `claude`, `/path/to/claude`, `node .../claude`, `node .../@anthropic-ai/claude-code/...`
/// - **Gemini**: `gemini`, `/path/to/gemini`, `node .../@google/gemini-cli/...`
///
/// Excludes:
/// - `grep claude`, `ps -eo ...`, `claude-something-else`, `vim claude.md`, etc.
pub fn detect_cli_tool(args: &str) -> Option<CliTool> {
    let first = args.split_whitespace().next().unwrap_or("");

    // Codex: native Rust binary "codex" or "/path/to/codex"
    if token_is_codex(first) {
        return Some(CliTool::Codex);
    }

    // Claude: bare "claude" or "/path/to/claude"
    if token_is_claude(first) {
        return Some(CliTool::Claude);
    }

    // Gemini: bare "gemini" or "/path/to/gemini"
    if token_is_gemini(first) {
        return Some(CliTool::Gemini);
    }

    // Node-launched tools: `node /path/to/tool` or `/path/to/node /path/to/tool`.
    // Newer Node invocations can include runtime flags before the script path
    // (e.g. `node --no-warnings=DEP0040 /run/.../gemini --yolo`).
    if first == "node" || first.ends_with("/node") {
        let tokens: Vec<&str> = args.split_whitespace().skip(1).collect();

        // Prefer the first non-flag token as the script path, then fall back to
        // checking all tokens to tolerate unusual Node flag/value combinations.
        if let Some(script_token) = tokens.iter().copied().find(|token| !token.starts_with('-')) {
            if token_is_codex(script_token) {
                return Some(CliTool::Codex);
            }
            if token_is_claude(script_token) {
                return Some(CliTool::Claude);
            }
            if token_is_gemini(script_token) {
                return Some(CliTool::Gemini);
            }
        }

        for token in tokens {
            if token_is_codex(token) {
                return Some(CliTool::Codex);
            }
            if token_is_claude(token) {
                return Some(CliTool::Claude);
            }
            if token_is_gemini(token) {
                return Some(CliTool::Gemini);
            }
        }
    }

    None
}

fn token_is_codex(token: &str) -> bool {
    token == "codex" || token.ends_with("/codex") || token.contains("@openai/codex")
}

fn token_is_claude(token: &str) -> bool {
    token == "claude" || token.ends_with("/claude") || token.contains("@anthropic-ai/claude-code")
}

fn token_is_gemini(token: &str) -> bool {
    token == "gemini" || token.ends_with("/gemini") || token.contains("@google/gemini-cli")
}

/// Read process CWD and TTY via platform-specific APIs.
fn enrich_from_proc(pid: u32, args: String, cli_tool: CliTool) -> Option<ProcessInfo> {
    let cwd = crate::platform::process_cwd(pid)?
        .to_string_lossy()
        .to_string();

    let tty = crate::platform::process_tty(pid)?;

    Some(ProcessInfo {
        pid,
        project_path: cwd,
        tty,
        args,
        cli_tool,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_scanner::SCANNER_TEST_LOCK;
    use std::sync::atomic::{AtomicU8, Ordering};

    #[test]
    fn parse_ps_finds_bare_claude() {
        let output = "\
  PID COMMAND
 1234 claude
 5678 bash";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 1234);
        assert_eq!(result[0].1, "claude");
        assert_eq!(result[0].2, CliTool::Claude);
    }

    #[test]
    fn parse_ps_finds_claude_with_flags() {
        let output = "\
  PID COMMAND
 4927 claude --dangerously-skip-permissions
 4928 claude --dangerously-skip-permissions --continue
 4929 claude --resume";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, 4927);
        assert!(result[0].1.contains("--dangerously-skip-permissions"));
        assert_eq!(result[0].2, CliTool::Claude);
        assert_eq!(result[1].0, 4928);
        assert!(result[1].1.contains("--continue"));
        assert_eq!(result[2].0, 4929);
        assert!(result[2].1.contains("--resume"));
    }

    #[test]
    fn parse_ps_finds_full_path_claude() {
        let output = "\
  PID COMMAND
 1000 /home/user/.local/bin/claude --dangerously-skip-permissions";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 1000);
        assert_eq!(result[0].2, CliTool::Claude);
    }

    #[test]
    fn parse_ps_finds_node_launched_claude() {
        let output = "\
  PID COMMAND
 2000 node /home/user/.nvm/versions/node/v22.5.0/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 2000);
        assert_eq!(result[0].2, CliTool::Claude);

        let output2 = "\
  PID COMMAND
 2000 node /usr/local/bin/claude --dangerously-skip-permissions";
        let result2 = parse_ps_output(output2);
        assert_eq!(result2.len(), 1);
        assert_eq!(result2[0].2, CliTool::Claude);
    }

    #[test]
    fn parse_ps_excludes_grep_and_ps() {
        let output = "\
  PID COMMAND
 1234 claude --dangerously-skip-permissions
 5555 grep claude
 6666 ps -eo pid,args";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 1234);
    }

    #[test]
    fn parse_ps_excludes_claude_prefixed() {
        let output = "\
  PID COMMAND
 1234 claude-code-server
 5678 claude";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 5678);
    }

    #[test]
    fn parse_ps_handles_empty_output() {
        let result = parse_ps_output("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_ps_handles_header_only() {
        let result = parse_ps_output("  PID COMMAND\n");
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // run_with_timeout tests
    // -----------------------------------------------------------------------

    // Regression: read-after-exit since 9a66d1c; a 131 KB argv on this host made
    // ps block, every ~40 s the scanner reported zero sessions. `run_with_timeout`
    // only read stdout after `try_wait()` reported exit, so any child whose output
    // exceeded the 64 KB pipe buffer blocked on write until the 2 s budget killed it.
    #[cfg(unix)]
    #[test]
    fn run_with_timeout_drains_stdout_larger_than_pipe_buffer() {
        let started = Instant::now();
        let output = run_with_timeout("sh", &["-c", "head -c 300000 /dev/zero | tr \"\\0\" a"]);
        let elapsed = started.elapsed();

        let output = output.expect("child producing 300 KB of stdout must not time out");
        assert_eq!(output.len(), 300_000);
        assert!(output.bytes().all(|byte| byte == b'a'));
        assert!(
            elapsed < SUBPROCESS_TIMEOUT,
            "drained child must finish well under the timeout, took {elapsed:?}"
        );
    }

    // -----------------------------------------------------------------------
    // fail-soft inventory tests
    // -----------------------------------------------------------------------

    fn process_info(pid: u32) -> ProcessInfo {
        ProcessInfo {
            pid,
            project_path: "/home/user/project".to_string(),
            tty: "/dev/pts/1".to_string(),
            args: "claude --continue".to_string(),
            cli_tool: CliTool::Claude,
        }
    }

    const INVENTORY_HEALTHY: u8 = 0;
    const INVENTORY_FAILS: u8 = 1;
    const INVENTORY_EMPTY: u8 = 2;
    static TEST_INVENTORY_MODE: AtomicU8 = AtomicU8::new(INVENTORY_HEALTHY);

    fn test_inventory() -> Option<Vec<ProcessInfo>> {
        match TEST_INVENTORY_MODE.load(Ordering::SeqCst) {
            INVENTORY_FAILS => None,
            INVENTORY_EMPTY => Some(Vec::new()),
            _ => Some(vec![process_info(42)]),
        }
    }

    // Regression: latent since 9a66d1c. A timed-out `ps` became `vec![]`, which
    // every consumer read as "zero sessions" — trackers were pruned, the hub
    // bumped its version and the export wrote stall_no_active_process for every
    // member. A failed inventory read must report the previous inventory as
    // degraded instead. Drives the real `scan_processes`/`scan_process_ids`
    // wiring (`LAST_GOOD_INVENTORY`) through the inventory seam.
    #[test]
    fn scan_processes_keeps_previous_inventory_on_degraded() {
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        TEST_INVENTORY_MODE.store(INVENTORY_HEALTHY, Ordering::SeqCst);
        set_inventory_provider_override(Some(test_inventory));

        let healthy = scan_processes();
        assert_eq!(
            healthy,
            ProcessScan {
                processes: vec![process_info(42)],
                degraded: false,
            }
        );
        assert_eq!(scan_process_ids(), Some(vec![42]));

        TEST_INVENTORY_MODE.store(INVENTORY_FAILS, Ordering::SeqCst);
        let degraded = scan_processes();
        assert_eq!(
            degraded,
            ProcessScan {
                processes: vec![process_info(42)],
                degraded: true,
            }
        );
        assert_eq!(
            scan_process_ids(),
            None,
            "failed fingerprint read must report unreadable, not empty"
        );
        assert_eq!(
            *LAST_GOOD_INVENTORY
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
            vec![process_info(42)]
        );

        // A healthy empty inventory is a real result, not a degraded one.
        TEST_INVENTORY_MODE.store(INVENTORY_EMPTY, Ordering::SeqCst);
        let emptied = scan_processes();
        assert_eq!(
            emptied,
            ProcessScan {
                processes: vec![],
                degraded: false,
            }
        );
        assert!(LAST_GOOD_INVENTORY
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty());

        set_inventory_provider_override(None);
        TEST_INVENTORY_MODE.store(INVENTORY_HEALTHY, Ordering::SeqCst);
    }

    // Regression: `session_scanner.process_scan.degraded` was emitted on every
    // failed read, one WARN + JSONL record per cycle for as long as the
    // inventory stayed unreadable. Health is edge-triggered: one event on
    // entry, a bounded reminder while it lasts, one `recovered` on exit.
    #[test]
    fn inventory_health_is_edge_triggered() {
        let t0 = Instant::now();
        let second = Duration::from_secs(1);
        let mut health = InventoryHealth::new();

        assert_eq!(health.note(true, t0), None);
        assert_eq!(
            health.note(false, t0),
            Some(InventoryHealthEvent::Degraded {
                failed_reads: 1,
                degraded_for: Duration::ZERO,
            })
        );
        assert_eq!(health.note(false, t0 + second), None);
        assert_eq!(health.note(false, t0 + 2 * second), None);
        assert_eq!(
            health.note(false, t0 + DEGRADED_REMINDER_INTERVAL),
            Some(InventoryHealthEvent::Degraded {
                failed_reads: 4,
                degraded_for: DEGRADED_REMINDER_INTERVAL,
            })
        );
        assert_eq!(
            health.note(false, t0 + DEGRADED_REMINDER_INTERVAL + second),
            None
        );
        assert_eq!(
            health.note(true, t0 + DEGRADED_REMINDER_INTERVAL + 2 * second),
            Some(InventoryHealthEvent::Recovered {
                failed_reads: 5,
                degraded_for: DEGRADED_REMINDER_INTERVAL + 2 * second,
            })
        );
        assert_eq!(
            health.note(true, t0 + DEGRADED_REMINDER_INTERVAL + 3 * second),
            None
        );
        assert_eq!(
            health.note(false, t0 + DEGRADED_REMINDER_INTERVAL + 4 * second),
            Some(InventoryHealthEvent::Degraded {
                failed_reads: 1,
                degraded_for: Duration::ZERO,
            })
        );
    }

    // The Linux inventory comes from /proc, not ps: a live child whose argv[0]
    // is "claude" must show up in the fingerprint.
    #[cfg(target_os = "linux")]
    #[test]
    fn scan_process_ids_reads_live_inventory_without_ps() {
        use std::os::unix::process::CommandExt;

        // The inventory seam is scanner-global; hold the lock so no test has
        // an override installed while this reads the live inventory.
        let _lock = SCANNER_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut child = Command::new("sleep")
            .arg0("claude")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep as claude");
        let pids = scan_process_ids();
        kill_and_reap(&mut child);

        let pids = pids.expect("/proc inventory readable");
        assert!(
            pids.contains(&child.id()),
            "live /proc inventory must list the claude-named child {}",
            child.id()
        );
        assert!(pids.windows(2).all(|pair| pair[0] < pair[1]));
    }

    // -----------------------------------------------------------------------
    // detect_cli_tool tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_claude_processes() {
        assert_eq!(detect_cli_tool("claude"), Some(CliTool::Claude));
        assert_eq!(
            detect_cli_tool("claude --dangerously-skip-permissions"),
            Some(CliTool::Claude)
        );
        assert_eq!(detect_cli_tool("claude --continue"), Some(CliTool::Claude));
        assert_eq!(
            detect_cli_tool("/home/user/.local/bin/claude"),
            Some(CliTool::Claude)
        );
        assert_eq!(
            detect_cli_tool("/home/user/.local/bin/claude --resume"),
            Some(CliTool::Claude)
        );
        assert_eq!(
            detect_cli_tool("node /usr/local/bin/claude"),
            Some(CliTool::Claude)
        );
        assert_eq!(
            detect_cli_tool("node /home/user/.nvm/versions/node/v22.5.0/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js"),
            Some(CliTool::Claude)
        );
        assert_eq!(
            detect_cli_tool("/usr/bin/node /usr/local/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js --dangerously-skip-permissions"),
            Some(CliTool::Claude)
        );
    }

    #[test]
    fn detect_codex_processes() {
        assert_eq!(detect_cli_tool("codex --full-auto"), Some(CliTool::Codex));
        assert_eq!(detect_cli_tool("codex"), Some(CliTool::Codex));
        assert_eq!(detect_cli_tool("codex --yolo"), Some(CliTool::Codex));
        assert_eq!(
            detect_cli_tool("/usr/local/bin/codex --full-auto"),
            Some(CliTool::Codex)
        );
        assert_eq!(
            detect_cli_tool("/home/user/.cargo/bin/codex resume --last"),
            Some(CliTool::Codex)
        );
        // Real fnm shim path (observed from live ps output)
        assert_eq!(
            detect_cli_tool(
                "node /run/user/1000/fnm_multishells/587700_1771710301602/bin/codex --yolo"
            ),
            Some(CliTool::Codex)
        );
        // Real native binary path (observed from live ps output)
        assert_eq!(
            detect_cli_tool("/home/testuser/.local/share/fnm/node-versions/v22.19.0/installation/lib/node_modules/@openai/codex/node_modules/@openai/codex-linux-x64/vendor/x86_64-unknown-linux-musl/codex/codex --yolo"),
            Some(CliTool::Codex)
        );
    }

    #[test]
    fn detect_gemini_processes() {
        assert_eq!(
            detect_cli_tool("node /path/@google/gemini-cli/dist/cli.mjs"),
            Some(CliTool::Gemini)
        );
        assert_eq!(
            detect_cli_tool("/usr/bin/node /home/user/.nvm/versions/node/v22.5.0/lib/node_modules/@google/gemini-cli/dist/cli.mjs --sandbox"),
            Some(CliTool::Gemini)
        );
        assert_eq!(detect_cli_tool("gemini --sandbox"), Some(CliTool::Gemini));
        assert_eq!(detect_cli_tool("gemini --yolo"), Some(CliTool::Gemini));
        assert_eq!(
            detect_cli_tool("/usr/local/bin/gemini --resume"),
            Some(CliTool::Gemini)
        );
        // Real fnm shim path (observed from live ps output)
        assert_eq!(
            detect_cli_tool(
                "node /run/user/1000/fnm_multishells/587826_1771710305315/bin/gemini --yolo"
            ),
            Some(CliTool::Gemini)
        );
        // Real node-launched via full path (observed from live ps output)
        assert_eq!(
            detect_cli_tool("/home/testuser/.local/share/fnm/node-versions/v22.19.0/installation/bin/node /run/user/1000/fnm_multishells/587826_1771710305315/bin/gemini --yolo"),
            Some(CliTool::Gemini)
        );
        // Newer observed launch includes node runtime flags before script path.
        assert_eq!(
            detect_cli_tool("node --no-warnings=DEP0040 /run/user/1000/fnm_multishells/764222_1772661944031/bin/gemini --yolo"),
            Some(CliTool::Gemini)
        );
        assert_eq!(
            detect_cli_tool("/home/testuser/.local/share/fnm/node-versions/v22.19.0/installation/bin/node --no-warnings=DEP0040 /run/user/1000/fnm_multishells/764222_1772661944031/bin/gemini --yolo"),
            Some(CliTool::Gemini)
        );
    }

    #[test]
    fn detect_non_cli_processes() {
        assert_eq!(detect_cli_tool("vim"), None);
        assert_eq!(detect_cli_tool("bash"), None);
        assert_eq!(detect_cli_tool("grep claude"), None);
        assert_eq!(detect_cli_tool("claude-code-server"), None);
        assert_eq!(detect_cli_tool("ps -eo pid,args"), None);
        assert_eq!(detect_cli_tool("vim claude.md"), None);
        assert_eq!(detect_cli_tool("node server.js"), None);
        assert_eq!(
            detect_cli_tool("node --no-warnings=DEP0040 server.js"),
            None
        );
        assert_eq!(detect_cli_tool("node /path/to/other-app/cli.js"), None);
    }

    #[test]
    fn parse_ps_detects_mixed_tools() {
        let output = "\
  PID COMMAND
 1000 claude --continue
 2000 codex --full-auto
 3000 node /path/@google/gemini-cli/dist/cli.mjs
 4000 bash
 5000 vim";
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 3);
        assert_eq!(
            result[0],
            (1000, "claude --continue".to_string(), CliTool::Claude)
        );
        assert_eq!(
            result[1],
            (2000, "codex --full-auto".to_string(), CliTool::Codex)
        );
        assert_eq!(
            result[2],
            (
                3000,
                "node /path/@google/gemini-cli/dist/cli.mjs".to_string(),
                CliTool::Gemini
            )
        );
    }
}
