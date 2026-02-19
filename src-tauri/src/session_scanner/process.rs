//! Process scanner — find Claude Code CLI processes via ps + /proc.

use std::fs;
use std::process::Command;

/// Information about a running claude process.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub project_path: String,
    pub tty: String,
    pub args: String,
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

/// Run `ps -eo pid,args` and return stdout.
fn run_ps() -> Option<String> {
    Command::new("ps")
        .args(["-eo", "pid,args"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Parse ps output, filter for claude processes, and read /proc for each.
fn parse_and_enrich(ps_output: &str) -> Vec<ProcessInfo> {
    parse_ps_output(ps_output)
        .into_iter()
        .filter_map(|(pid, args)| enrich_from_proc(pid, args))
        .collect()
}

/// Parse `ps -eo pid,args` output into (pid, args) pairs for claude processes.
pub fn parse_ps_output(output: &str) -> Vec<(u32, String)> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // Split into PID and rest (args)
            let (pid_str, args) = line.split_once(char::is_whitespace)?;
            let args = args.trim();

            // Filter: the executable must be `claude` (not our grep or ps process)
            if !is_claude_process(args) {
                return None;
            }

            let pid: u32 = pid_str.parse().ok()?;
            Some((pid, args.to_string()))
        })
        .collect()
}

/// Check if the command args represent a Claude Code CLI process.
///
/// Matches patterns like:
/// - `claude`
/// - `claude --dangerously-skip-permissions`
/// - `claude --dangerously-skip-permissions --continue`
/// - `/home/user/.local/bin/claude ...`
/// - `node /path/to/claude ...`
///
/// Excludes:
/// - `grep claude`
/// - `ps -eo ... claude`
/// - `claude-something-else`
fn is_claude_process(args: &str) -> bool {
    // Get the first token (the binary name/path)
    let first_token = args.split_whitespace().next().unwrap_or("");

    // Direct match: bare `claude` or path ending in `/claude`
    if first_token == "claude" || first_token.ends_with("/claude") {
        return true;
    }

    // Node-launched: `node /path/to/claude ...`
    if first_token == "node" || first_token.ends_with("/node") {
        let second_token = args.split_whitespace().nth(1).unwrap_or("");
        if second_token == "claude" || second_token.ends_with("/claude") {
            return true;
        }
    }

    false
}

/// Read /proc/PID/cwd and /proc/PID/fd/0 to get project path and TTY.
fn enrich_from_proc(pid: u32, args: String) -> Option<ProcessInfo> {
    let cwd = fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()?
        .to_string_lossy()
        .to_string();

    let tty = fs::read_link(format!("/proc/{pid}/fd/0"))
        .ok()?
        .to_string_lossy()
        .to_string();

    Some(ProcessInfo {
        pid,
        project_path: cwd,
        tty,
        args,
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
        assert_eq!(result[0], (1234, "claude".to_string()));
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
    }

    #[test]
    fn parse_ps_finds_node_launched_claude() {
        let output = "\
  PID COMMAND
 2000 node /home/user/.nvm/versions/node/v22.5.0/lib/node_modules/@anthropic-ai/claude-code/dist/cli.js";
        // This won't match because the path doesn't end in /claude
        let result = parse_ps_output(output);
        assert_eq!(result.len(), 0);

        // But this will (if the binary is named claude):
        let output2 = "\
  PID COMMAND
 2000 node /usr/local/bin/claude --dangerously-skip-permissions";
        let result2 = parse_ps_output(output2);
        assert_eq!(result2.len(), 1);
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

    #[test]
    fn is_claude_process_matches_correctly() {
        assert!(is_claude_process("claude"));
        assert!(is_claude_process("claude --dangerously-skip-permissions"));
        assert!(is_claude_process("claude --continue"));
        assert!(is_claude_process("/home/user/.local/bin/claude"));
        assert!(is_claude_process("/home/user/.local/bin/claude --resume"));
        assert!(is_claude_process("node /usr/local/bin/claude"));

        assert!(!is_claude_process("grep claude"));
        assert!(!is_claude_process("claude-code-server"));
        assert!(!is_claude_process("ps -eo pid,args"));
        assert!(!is_claude_process("bash"));
        assert!(!is_claude_process("vim claude.md"));
    }
}
