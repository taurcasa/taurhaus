//! Coordination runtime boundary for external side effects.
//!
//! This isolates host-level operations (tmux, mesh, process control) behind a
//! single interface so tests can run against a deterministic runtime double.

use std::process::Command;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli::{self, CommandInvocation};
use crate::session_scanner::cli_tool::CliTool;

const TMUX_TEXT_TO_ENTER_DELAY: Duration = Duration::from_millis(350);
const TMUX_POST_ENTER_DELAY: Duration = Duration::from_secs(1);
const SESSION_DETECT_ATTEMPTS: usize = 6;
const SESSION_DETECT_INTERVAL: Duration = Duration::from_millis(200);
const TMUX_SPLIT_MAX_PANES: usize = 4;
const TAURHAUS_TMUX_SESSION_NAME: &str = "taurhaus";

pub trait CoordinationRuntime: Send + Sync {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError>;
    fn send_tmux_keys_with_enter(&self, pane_id: &str, keys: &str)
        -> Result<(), CoordinationError>;
    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError>;
    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
    ) -> Result<(), CoordinationError>;
    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError>;
    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError>;
    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError>;
    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError>;
}

#[derive(Debug, Default)]
pub struct SystemCoordinationRuntime;

impl CoordinationRuntime for SystemCoordinationRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        create_tmux_pane_with_layout(project_id, tmux_layout)
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        let target = tmux_target_for_pane(pane_id);
        run_tmux(&[
            "send-keys".to_string(),
            "-t".to_string(),
            target.clone(),
            "-l".to_string(),
            keys.to_string(),
        ])?;
        // Give tmux enough time to flush typed text before sending Enter.
        thread::sleep(TMUX_TEXT_TO_ENTER_DELAY);
        run_tmux(&[
            "send-keys".to_string(),
            "-t".to_string(),
            target,
            "Enter".to_string(),
        ])?;
        thread::sleep(TMUX_POST_ENTER_DELAY);
        Ok(())
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
    ) -> Result<(), CoordinationError> {
        run_mesh(
            &["join", "--team", team_name, "--name", member_name],
            Some(project_id),
        )
        .map(|_| ())
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        for _ in 0..SESSION_DETECT_ATTEMPTS {
            let match_id = scan_sessions_for_runtime()
                .into_iter()
                .find(|session| {
                    session.tmux_pane.as_deref() == Some(pane_id) && session.cli_tool == cli_tool
                })
                .and_then(|session| session.session_id);

            if match_id.is_some() {
                return Ok(match_id);
            }

            thread::sleep(SESSION_DETECT_INTERVAL);
        }

        Ok(None)
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        let invocation = mesh_command_invocation(&[
            "daemon",
            "--pane",
            pane_id,
            "--team",
            team_name,
            "--name",
            member_name,
        ]);
        let child = spawn_system_command(&invocation)?;
        Ok(child.id())
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        run_tmux(&[
            "kill-pane".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
        ])
        .map(|_| ())
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        #[cfg(target_os = "windows")]
        let pid_arg = pid.to_string();
        #[cfg(not(target_os = "windows"))]
        let pid_arg = validate_unix_pid(pid)?;

        #[cfg(target_os = "windows")]
        let invocation = CommandInvocation {
            program: "taskkill".to_string(),
            args: vec!["/PID".to_string(), pid_arg, "/F".to_string()],
        };
        #[cfg(not(target_os = "windows"))]
        let invocation = CommandInvocation {
            program: "kill".to_string(),
            args: vec!["-TERM".to_string(), pid_arg],
        };

        let output = run_system_command(&invocation)?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(CoordinationError::Backend(format!(
                "process kill failed ({} {}): {}",
                invocation.program,
                invocation.args.join(" "),
                stderr
            )))
        }
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        #[cfg(target_os = "windows")]
        let pid_arg = pid.to_string();
        #[cfg(not(target_os = "windows"))]
        if pid == 0 || pid > i32::MAX as u32 {
            return Ok(false);
        }
        #[cfg(not(target_os = "windows"))]
        let pid_arg = pid.to_string();

        #[cfg(target_os = "windows")]
        let invocation = CommandInvocation {
            program: "tasklist".to_string(),
            args: vec!["/FI".to_string(), format!("PID eq {pid_arg}")],
        };
        #[cfg(not(target_os = "windows"))]
        let invocation = CommandInvocation {
            program: "kill".to_string(),
            args: vec!["-0".to_string(), pid_arg.clone()],
        };

        let output = run_system_command(&invocation)?;
        #[cfg(target_os = "windows")]
        {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return Err(CoordinationError::Backend(format!(
                    "pid check failed ({} {}): {}",
                    invocation.program,
                    invocation.args.join(" "),
                    stderr
                )));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            Ok(stdout.contains(&pid_arg))
        }
        #[cfg(not(target_os = "windows"))]
        {
            if output.status.success() {
                return Ok(true);
            }
            let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
            if stderr.contains("operation not permitted") {
                return Ok(true);
            }
            Ok(false)
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeSessionInfo {
    tmux_pane: Option<String>,
    cli_tool: CliTool,
    session_id: Option<String>,
}

#[cfg(not(test))]
fn scan_sessions_for_runtime() -> Vec<RuntimeSessionInfo> {
    crate::session_scanner::scan_sessions()
        .into_iter()
        .map(|session| RuntimeSessionInfo {
            tmux_pane: session.tmux_pane,
            cli_tool: session.cli_tool,
            session_id: session.session_id,
        })
        .collect()
}

#[cfg(test)]
fn scan_sessions_for_runtime() -> Vec<RuntimeSessionInfo> {
    Vec::new()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCall {
    CreatePane {
        project_id: String,
    },
    SendKeys {
        pane_id: String,
        keys: String,
    },
    DetectSessionId {
        pane_id: String,
        cli_tool: CliTool,
    },
    JoinMesh {
        team_name: String,
        member_name: String,
        project_id: String,
    },
    SpawnDaemon {
        pane_id: String,
        team_name: String,
        member_name: String,
    },
    KillPane {
        pane_id: String,
    },
    TerminatePid {
        pid: u32,
    },
    CheckPid {
        pid: u32,
    },
}

#[derive(Debug, Default)]
pub struct RecordingCoordinationRuntime {
    calls: Mutex<Vec<RuntimeCall>>,
    pane_counter: AtomicUsize,
    pid_counter: AtomicU32,
}

impl RecordingCoordinationRuntime {
    pub fn calls(&self) -> Vec<RuntimeCall> {
        self.calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }

    fn push_call(&self, call: RuntimeCall) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
    }
}

impl CoordinationRuntime for RecordingCoordinationRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        _tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        self.push_call(RuntimeCall::CreatePane {
            project_id: project_id.to_string(),
        });
        let idx = self.pane_counter.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(format!("test-pane-{idx}"))
    }

    fn send_tmux_keys_with_enter(
        &self,
        pane_id: &str,
        keys: &str,
    ) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::SendKeys {
            pane_id: pane_id.to_string(),
            keys: keys.to_string(),
        });
        Ok(())
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        self.push_call(RuntimeCall::DetectSessionId {
            pane_id: pane_id.to_string(),
            cli_tool,
        });
        Ok(Some(format!("session-{pane_id}")))
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
    ) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::JoinMesh {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            project_id: project_id.to_string(),
        });
        Ok(())
    }

    fn spawn_mesh_daemon(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.push_call(RuntimeCall::SpawnDaemon {
            pane_id: pane_id.to_string(),
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        });
        let pid = self.pid_counter.fetch_add(1, Ordering::SeqCst) + 10000;
        Ok(pid)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::KillPane {
            pane_id: pane_id.to_string(),
        });
        Ok(())
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::TerminatePid { pid });
        Ok(())
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPid { pid });
        Ok(false)
    }
}

fn mesh_command_invocation(args: &[&str]) -> CommandInvocation {
    mesh_cli::mesh_command_invocation(args)
}

fn tmux_command_invocation(args: &[String]) -> CommandInvocation {
    mesh_cli::command_invocation("tmux", args)
}

fn run_system_command(
    invocation: &CommandInvocation,
) -> Result<std::process::Output, CoordinationError> {
    let output = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args).output()
    } else {
        Command::new(&invocation.program)
            .args(&invocation.args)
            .output()
    };
    output.map_err(CoordinationError::Io)
}

fn spawn_system_command(
    invocation: &CommandInvocation,
) -> Result<std::process::Child, CoordinationError> {
    let child = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        cmd.args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    } else {
        Command::new(&invocation.program)
            .args(&invocation.args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    };
    child.map_err(CoordinationError::Io)
}

fn run_mesh(args: &[&str], cwd: Option<&str>) -> Result<String, CoordinationError> {
    let invocation = mesh_command_invocation(args);
    let output = if invocation.program == "wsl" {
        let mut cmd = mesh_cli::wsl_command_for_coordination();
        if let Some(project_id) = cwd {
            cmd.args(["--cd", project_id]);
        }
        cmd.args(&invocation.args).output()
    } else {
        let mut cmd = Command::new(&invocation.program);
        cmd.args(&invocation.args);
        if let Some(project_id) = cwd {
            cmd.current_dir(project_id);
        }
        cmd.output()
    }
    .map_err(CoordinationError::Io)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "mesh command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

fn run_tmux(args: &[String]) -> Result<String, CoordinationError> {
    let invocation = tmux_command_invocation(args);
    let output = run_system_command(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "tmux command failed ({} {}): {}",
            invocation.program,
            invocation.args.join(" "),
            stderr
        )))
    }
}

fn run_tmux_output(args: &[String]) -> Result<std::process::Output, CoordinationError> {
    let invocation = tmux_command_invocation(args);
    run_system_command(&invocation)
}

fn ensure_taurhaus_tmux_session() -> Result<(), CoordinationError> {
    let check = run_tmux_output(&[
        "has-session".to_string(),
        "-t".to_string(),
        TAURHAUS_TMUX_SESSION_NAME.to_string(),
    ])?;

    if check.status.success() {
        return Ok(());
    }

    run_tmux(&[
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        TAURHAUS_TMUX_SESSION_NAME.to_string(),
    ])?;
    Ok(())
}

fn create_tmux_pane_with_layout(
    project_id: &str,
    tmux_layout: &str,
) -> Result<String, CoordinationError> {
    ensure_taurhaus_tmux_session()?;

    let window_name = std::path::Path::new(project_id)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "agent".to_string());

    if tmux_layout == "split" {
        if let Some(target) =
            find_tmux_window_with_space(TAURHAUS_TMUX_SESSION_NAME, TMUX_SPLIT_MAX_PANES)?
        {
            return create_tmux_split_pane(project_id, &target);
        }
    } else if tmux_layout == "per_project" {
        if let Some(target) = find_tmux_project_window(TAURHAUS_TMUX_SESSION_NAME, &window_name)? {
            return create_tmux_split_pane(project_id, &target);
        }
    }

    create_tmux_new_window_pane(project_id, &window_name)
}

fn create_tmux_new_window_pane(
    project_id: &str,
    window_name: &str,
) -> Result<String, CoordinationError> {
    let pane_id = run_tmux(&[
        "new-window".to_string(),
        "-n".to_string(),
        window_name.to_string(),
        "-t".to_string(),
        format!("{TAURHAUS_TMUX_SESSION_NAME}:"),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        "-c".to_string(),
        project_id.to_string(),
    ])?;

    parse_tmux_created_pane_id(&pane_id).ok_or_else(|| {
        CoordinationError::Backend(
            "tmux new-window returned empty output; expected pane identifier".to_string(),
        )
    })
}

fn create_tmux_split_pane(project_id: &str, target: &str) -> Result<String, CoordinationError> {
    let pane_id = run_tmux(&[
        "split-window".to_string(),
        "-h".to_string(),
        "-t".to_string(),
        target.to_string(),
        "-P".to_string(),
        "-F".to_string(),
        "#{pane_id}".to_string(),
        "-c".to_string(),
        project_id.to_string(),
    ])?;

    parse_tmux_created_pane_id(&pane_id).ok_or_else(|| {
        CoordinationError::Backend(
            "tmux split-window returned empty output; expected pane identifier".to_string(),
        )
    })
}

fn parse_tmux_created_pane_id(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .find(|token| !token.trim().is_empty())
        .map(str::to_string)
}

fn find_tmux_window_with_space(
    tmux_session: &str,
    max_panes: usize,
) -> Result<Option<String>, CoordinationError> {
    let out = run_tmux(&[
        "list-windows".to_string(),
        "-t".to_string(),
        tmux_session.to_string(),
        "-F".to_string(),
        "#{window_index}\t#{window_panes}".to_string(),
    ])?;

    for line in out.lines() {
        let mut parts = line.split('\t');
        let window_index = match parts.next() {
            Some(idx) if !idx.trim().is_empty() => idx.trim(),
            _ => continue,
        };
        let pane_count = parts
            .next()
            .and_then(|count| count.trim().parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        if pane_count < max_panes {
            return Ok(Some(format!("{tmux_session}:{window_index}.0")));
        }
    }

    Ok(None)
}

fn find_tmux_project_window(
    tmux_session: &str,
    window_name: &str,
) -> Result<Option<String>, CoordinationError> {
    let out = run_tmux(&[
        "list-windows".to_string(),
        "-t".to_string(),
        tmux_session.to_string(),
        "-F".to_string(),
        "#{window_index}\t#{window_name}".to_string(),
    ])?;

    for line in out.lines() {
        let mut parts = line.split('\t');
        let window_index = match parts.next() {
            Some(idx) if !idx.trim().is_empty() => idx.trim(),
            _ => continue,
        };
        let name = match parts.next() {
            Some(name) => name.trim(),
            None => continue,
        };
        if name == window_name {
            return Ok(Some(format!("{tmux_session}:{window_index}.0")));
        }
    }

    Ok(None)
}

fn tmux_target_for_pane(pane_id: &str) -> String {
    if pane_id.starts_with('%') {
        pane_id.to_string()
    } else {
        format!(":.{pane_id}")
    }
}

#[cfg(not(target_os = "windows"))]
fn validate_unix_pid(pid: u32) -> Result<String, CoordinationError> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(CoordinationError::Validation(format!(
            "pid out of Unix kill range: {pid}"
        )));
    }
    Ok(pid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_target_uses_pane_id_when_present() {
        assert_eq!(tmux_target_for_pane("%12"), "%12");
    }

    #[test]
    fn tmux_target_wraps_numeric_index() {
        assert_eq!(tmux_target_for_pane("3"), ":.3");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_accepts_normal_pid() {
        assert_eq!(validate_unix_pid(12345).unwrap(), "12345");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_rejects_zero() {
        let err = validate_unix_pid(0).expect_err("pid 0 should be rejected");
        assert!(matches!(err, CoordinationError::Validation(_)));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_pid_validation_rejects_values_above_i32_max() {
        let err = validate_unix_pid(u32::MAX).expect_err("out-of-range pid should be rejected");
        assert!(matches!(err, CoordinationError::Validation(_)));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn system_runtime_is_process_running_returns_false_for_out_of_range_pid() {
        let runtime = SystemCoordinationRuntime;
        assert!(!runtime.is_process_running_by_pid(u32::MAX).unwrap());
        assert!(!runtime.is_process_running_by_pid(0).unwrap());
    }

    #[test]
    fn recording_runtime_returns_deterministic_values_and_records_calls() {
        let runtime = RecordingCoordinationRuntime::default();

        let pane = runtime
            .create_aitx_pane("/tmp/project", "new_window")
            .expect("pane");
        let pid = runtime
            .spawn_mesh_daemon(&pane, "alpha", "agent-a")
            .expect("pid");
        runtime
            .send_tmux_keys_with_enter(&pane, "codex --yolo")
            .expect("keys");
        runtime
            .join_mesh("alpha", "agent-a", "/tmp/project")
            .expect("join");
        runtime.terminate_process_by_pid(pid).expect("terminate");
        runtime.kill_aitx_pane(&pane).expect("kill pane");
        assert!(!runtime.is_process_running_by_pid(pid).expect("check pid"));

        assert_eq!(pane, "test-pane-1");
        assert_eq!(pid, 10000);
        assert_eq!(
            runtime.calls(),
            vec![
                RuntimeCall::CreatePane {
                    project_id: "/tmp/project".to_string()
                },
                RuntimeCall::SpawnDaemon {
                    pane_id: "test-pane-1".to_string(),
                    team_name: "alpha".to_string(),
                    member_name: "agent-a".to_string()
                },
                RuntimeCall::SendKeys {
                    pane_id: "test-pane-1".to_string(),
                    keys: "codex --yolo".to_string()
                },
                RuntimeCall::JoinMesh {
                    team_name: "alpha".to_string(),
                    member_name: "agent-a".to_string(),
                    project_id: "/tmp/project".to_string(),
                },
                RuntimeCall::TerminatePid { pid: 10000 },
                RuntimeCall::KillPane {
                    pane_id: "test-pane-1".to_string()
                },
                RuntimeCall::CheckPid { pid: 10000 },
            ]
        );
    }
}
