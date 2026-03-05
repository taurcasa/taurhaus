//! Process scanner — find CLI tool processes via ps + /proc.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::cli_tool::CliTool;

/// Information about a running CLI tool process.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub project_path: String,
    pub tty: String,
    pub args: String,
    pub cli_tool: CliTool,
}

/// Scan for claude processes by running `ps` and reading `/proc`.
///
/// Returns one `ProcessInfo` per detected claude process. Gracefully
/// skips processes that disappear between the `ps` call and `/proc` reads.
pub fn scan_processes() -> Vec<ProcessInfo> {
    let ps_output = match run_ps() {
        Some(output) => output,
        None => return vec![],
    };
    parse_and_enrich(&ps_output)
}

/// Scan only detected CLI-tool PIDs (cheap fingerprint for cache checks).
pub fn scan_process_ids() -> Vec<u32> {
    let ps_output = match run_ps() {
        Some(output) => output,
        None => return vec![],
    };
    let mut pids: Vec<u32> = parse_ps_output(&ps_output)
        .into_iter()
        .map(|(pid, _, _)| pid)
        .collect();
    pids.sort_unstable();
    pids
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
pub fn scan_process_ids_cached() -> Vec<u32> {
    let now = Instant::now();
    let proc_count = system_process_count();
    let cache = PID_FINGERPRINT_CACHE.get_or_init(|| Mutex::new(PidFingerprintCache::default()));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());

    let cache_fresh = guard
        .scanned_at
        .is_some_and(|ts| now.duration_since(ts) < PID_FINGERPRINT_CACHE_TTL);
    let same_proc_count = proc_count.is_some() && guard.proc_count == proc_count;
    if cache_fresh && same_proc_count {
        return guard.pids.clone();
    }

    let pids = scan_process_ids();
    guard.pids = pids.clone();
    guard.proc_count = proc_count;
    guard.scanned_at = Some(now);
    pids
}

/// Count live process entries from `/proc` (Linux).
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

/// Timeout for subprocess execution. If `ps` or similar hangs (e.g. stale
/// NFS mount affecting `/proc`), we bail out instead of blocking forever.
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(2);

/// Run `ps -eo pid,args` and return stdout.
///
/// Returns `None` if the command fails, is unavailable, or exceeds the timeout.
fn run_ps() -> Option<String> {
    run_with_timeout("ps", &["-eo", "pid,args"])
}

/// Spawn a subprocess and wait for it with a timeout.
///
/// Returns the stdout as a string on success. Returns `None` if the command
/// fails to spawn, exits with error, or exceeds `SUBPROCESS_TIMEOUT`.
/// On timeout, the child process is killed.
pub(super) fn run_with_timeout(cmd: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + SUBPROCESS_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited. Read stdout from the pipe buffer.
                let mut buf = String::new();
                if let Some(ref mut stdout) = child.stdout {
                    let _ = stdout.read_to_string(&mut buf);
                }
                return if status.success() { Some(buf) } else { None };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait(); // reap zombie
                    tracing::warn!(cmd, "Subprocess timed out after {SUBPROCESS_TIMEOUT:?}");
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Parse ps output, filter for CLI tool processes, and read /proc for each.
fn parse_and_enrich(ps_output: &str) -> Vec<ProcessInfo> {
    parse_ps_output(ps_output)
        .into_iter()
        .filter_map(|(pid, args, tool)| enrich_from_proc(pid, args, tool))
        .collect()
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
