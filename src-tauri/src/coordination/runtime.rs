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

const TMUX_TEXT_TO_ENTER_DELAY: Duration = Duration::from_millis(350);
const TMUX_POST_ENTER_DELAY: Duration = Duration::from_secs(1);

pub trait CoordinationRuntime: Send + Sync {
    fn create_aitx_pane(&self, project_id: &str) -> Result<String, CoordinationError>;
    fn send_tmux_keys_with_enter(&self, pane_id: &str, keys: &str)
        -> Result<(), CoordinationError>;
    fn join_mesh(&self, team_name: &str, member_name: &str) -> Result<(), CoordinationError>;
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
    fn create_aitx_pane(&self, project_id: &str) -> Result<String, CoordinationError> {
        let stdout = run_aitx(&["new", "--path", project_id])?;
        stdout
            .split_whitespace()
            .find(|token| !token.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                CoordinationError::Backend(
                    "aitx new returned empty output; expected pane identifier".to_string(),
                )
            })
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

    fn join_mesh(&self, team_name: &str, member_name: &str) -> Result<(), CoordinationError> {
        run_mesh(&["join", "--team", team_name, "--name", member_name]).map(|_| ())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCall {
    CreatePane {
        project_id: String,
    },
    SendKeys {
        pane_id: String,
        keys: String,
    },
    JoinMesh {
        team_name: String,
        member_name: String,
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
    fn create_aitx_pane(&self, project_id: &str) -> Result<String, CoordinationError> {
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

    fn join_mesh(&self, team_name: &str, member_name: &str) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::JoinMesh {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
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

fn aitx_binary_path() -> Option<String> {
    if cfg!(target_os = "windows") {
        mesh_cli::resolve_wsl_binary_path("aitx")
    } else {
        None
    }
}

fn mesh_command_invocation(args: &[&str]) -> CommandInvocation {
    mesh_cli::mesh_command_invocation(args)
}

fn aitx_command_invocation(args: &[&str]) -> CommandInvocation {
    let aitx_path = aitx_binary_path().unwrap_or_else(|| "aitx".to_string());
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    mesh_cli::command_invocation(&aitx_path, &args)
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

fn run_mesh(args: &[&str]) -> Result<String, CoordinationError> {
    let invocation = mesh_command_invocation(args);
    let output = run_system_command(&invocation)?;
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

fn run_aitx(args: &[&str]) -> Result<String, CoordinationError> {
    let invocation = aitx_command_invocation(args);
    let output = run_system_command(&invocation)?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(CoordinationError::Backend(format!(
            "aitx command failed ({} {}): {}",
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

        let pane = runtime.create_aitx_pane("/tmp/project").expect("pane");
        let pid = runtime
            .spawn_mesh_daemon(&pane, "alpha", "agent-a")
            .expect("pid");
        runtime
            .send_tmux_keys_with_enter(&pane, "codex --yolo")
            .expect("keys");
        runtime.join_mesh("alpha", "agent-a").expect("join");
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
                    member_name: "agent-a".to_string()
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
