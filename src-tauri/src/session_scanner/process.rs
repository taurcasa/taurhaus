//! Process scanner — find CLI tool processes from the platform process inventory.
//!
//! Linux reads `/proc/*/cmdline` directly; other Unix platforms run `ps`.
//! The inventory is fail-soft: when it cannot be read, the last good
//! inventory is reported with `ProcessScan::degraded` set so callers keep
//! their state instead of treating the gap as "no sessions".

use std::io::Read;
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

/// Name of the inventory source, for the degraded event.
#[cfg(target_os = "linux")]
const INVENTORY_SOURCE: &str = "proc";
#[cfg(not(target_os = "linux"))]
const INVENTORY_SOURCE: &str = "ps";

/// Scan supported CLI tool processes and enrich them from the platform.
///
/// Returns one `ProcessInfo` per detected tool process. Gracefully skips
/// processes that disappear between the inventory read and the enrichment.
/// When the inventory itself cannot be read, returns the last good inventory
/// with `degraded` set.
pub fn scan_processes() -> ProcessScan {
    let fresh = list_cli_tool_processes().map(|entries| {
        entries
            .into_iter()
            .filter_map(|(pid, args, tool)| enrich_from_proc(pid, args, tool))
            .collect()
    });
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

/// Scan only detected CLI-tool PIDs (cheap fingerprint for cache checks).
///
/// Returns `None` when the inventory could not be read.
pub fn scan_process_ids() -> Option<Vec<u32>> {
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
fn system_process_count() -> Option<usize> {
    #[cfg(target_os = "linux")]
    {
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

    #[cfg(not(target_os = "linux"))]
    {
        let output = run_with_timeout("ps", &["-A", "-o", "pid="])?;
        Some(
            output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
        )
    }
}

/// Read the raw CLI-tool inventory as `(pid, args, cli_tool)` tuples.
///
/// Returns `None` when the platform inventory is unavailable and emits
/// `session_scanner.process_scan.degraded`.
fn list_cli_tool_processes() -> Option<Vec<(u32, String, CliTool)>> {
    let inventory = read_cli_tool_inventory();
    if inventory.is_none() {
        emit_process_scan_degraded();
    }
    inventory
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
#[cfg(not(target_os = "linux"))]
fn read_cli_tool_inventory() -> Option<Vec<(u32, String, CliTool)>> {
    let output = run_with_timeout("ps", &["-eo", "pid,args"])?;
    Some(parse_ps_output(&output))
}

fn emit_process_scan_degraded() {
    let previous_inventory = LAST_GOOD_INVENTORY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len();
    tracing::warn!(
        source = INVENTORY_SOURCE,
        previous_inventory,
        "process inventory unavailable; keeping previous inventory"
    );
    let mut fields = Map::new();
    fields.insert(
        "source".to_string(),
        Value::String(INVENTORY_SOURCE.to_string()),
    );
    fields.insert(
        "previous_inventory".to_string(),
        Value::Number(serde_json::Number::from(previous_inventory as u64)),
    );
    crate::commands::logging::emit_global(
        "warn",
        "backend",
        "session_scanner.process_scan.degraded",
        Some("Process inventory unavailable; keeping previous inventory".to_string()),
        fields,
    );
}

/// Timeout for subprocess execution. If `ps` or similar hangs (e.g. stale
/// NFS mount affecting `/proc`), we bail out instead of blocking forever.
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(2);

/// Spawn a subprocess, drain its stdout concurrently, and wait with a timeout.
///
/// Returns the stdout as a string on success. Returns `None` if the command
/// fails to spawn, exits with error, or exceeds `SUBPROCESS_TIMEOUT`.
/// On timeout, the child process is killed.
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
    let (output_tx, output_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = output_tx.send(buf);
    });

    let Ok(output) = output_rx.recv_timeout(SUBPROCESS_TIMEOUT) else {
        kill_and_reap(&mut child);
        tracing::warn!(cmd, "Subprocess timed out after {SUBPROCESS_TIMEOUT:?}");
        return None;
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

    // Regression: latent since 9a66d1c. A timed-out `ps` became `vec![]`, which
    // every consumer read as "zero sessions" — trackers were pruned, the hub
    // bumped its version and the export wrote stall_no_active_process for every
    // member. A failed inventory read must report the previous inventory as
    // degraded instead.
    #[cfg(unix)]
    #[test]
    fn scan_processes_keeps_previous_inventory_on_degraded() {
        let mut last_good = Vec::new();

        let healthy = resolve_process_scan(Some(vec![process_info(42)]), &mut last_good);
        assert_eq!(
            healthy,
            ProcessScan {
                processes: vec![process_info(42)],
                degraded: false,
            }
        );

        let degraded = resolve_process_scan(None, &mut last_good);
        assert_eq!(
            degraded,
            ProcessScan {
                processes: vec![process_info(42)],
                degraded: true,
            }
        );
        assert_eq!(last_good, vec![process_info(42)]);

        // A healthy empty inventory is a real result, not a degraded one.
        let emptied = resolve_process_scan(Some(vec![]), &mut last_good);
        assert_eq!(
            emptied,
            ProcessScan {
                processes: vec![],
                degraded: false,
            }
        );
        assert!(last_good.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scan_process_ids_reads_live_inventory_without_ps() {
        // The Linux inventory comes from /proc; it must be readable and sorted.
        let pids = scan_process_ids().expect("/proc inventory readable");
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
