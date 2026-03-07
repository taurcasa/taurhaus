//! Coordination runtime boundary for external side effects.
//!
//! This isolates host-level operations (tmux, mesh, process control) behind a
//! single interface so tests can run against a deterministic runtime double.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crate::coordination::domain::Member;
use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli::{self, CommandInvocation};
use crate::coordination::stores::MemberRuntimeRecord;
use crate::session_scanner::cli_tool::CliTool;

const TMUX_TEXT_TO_ENTER_DELAY: Duration = Duration::from_millis(350);
const TMUX_POST_ENTER_DELAY: Duration = Duration::from_secs(1);
const SESSION_DETECT_ATTEMPTS: usize = 6;
const SESSION_DETECT_INTERVAL: Duration = Duration::from_millis(200);
const DAEMON_START_ATTEMPTS: usize = 30;
const DAEMON_START_INTERVAL: Duration = Duration::from_millis(100);
const TMUX_SPLIT_MAX_PANES: usize = 4;
const TAURHAUS_TMUX_SESSION_NAME: &str = "taurhaus";
const CLAUDE_DIR_OVERRIDE_ENV: &str = "TAURHAUS_CLAUDE_DIR";

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
    fn spawn_team_daemon(
        &self,
        _team_name: &str,
        _operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        Err(CoordinationError::Backend(
            "team daemon start not implemented".to_string(),
        ))
    }
    fn find_existing_mesh_daemon_pids(
        &self,
        _pane_id: &str,
        _team_name: &str,
        _member_name: &str,
    ) -> Result<Vec<u32>, CoordinationError> {
        Ok(Vec::new())
    }
    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError>;
    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError>;
    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError>;
    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError>;
    fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError>;
    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError>;
    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError>;
    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError>;
    fn clear_mesh_daemon_pid_file(
        &self,
        _team_name: &str,
        _member_name: &str,
    ) -> Result<(), CoordinationError> {
        Ok(())
    }
    fn stop_team_daemon(&self, _team_name: &str) -> Result<(), CoordinationError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneResolution {
    pub pane_id: String,
    pub reused_pane: bool,
    pub created_new_pane: bool,
}

/// Resolve a reusable pane for a member or create a fresh one.
///
/// Classification:
/// - Missing pane id -> create
/// - Lookup fail / missing pane target -> create
/// - Dead pane + ownership match -> kill + create
/// - Alive pane + ownership match -> reuse
/// - Ownership mismatch/check failure -> warn + create
pub fn resolve_or_create_pane_for_member(
    runtime: &dyn CoordinationRuntime,
    member: &Member,
    runtime_record: Option<&MemberRuntimeRecord>,
    tmux_layout: &str,
) -> Result<PaneResolution, CoordinationError> {
    let project_id = member.project_path.display().to_string();

    let create_new = || -> Result<PaneResolution, CoordinationError> {
        let pane_id = runtime.create_aitx_pane(&project_id, tmux_layout)?;
        Ok(PaneResolution {
            pane_id,
            reused_pane: false,
            created_new_pane: true,
        })
    };

    let Some(existing_pane_id) = runtime_record.and_then(|record| record.pane_id.as_deref()) else {
        return create_new();
    };

    if !runtime.pane_exists(existing_pane_id)? {
        return create_new();
    }

    let is_dead = runtime.pane_is_dead(existing_pane_id)?;
    if is_dead {
        match runtime.pane_belongs_to_project(existing_pane_id, &project_id) {
            Ok(true) => {
                if let Err(err) = runtime.kill_aitx_pane(existing_pane_id) {
                    tracing::warn!(
                        pane_id = %existing_pane_id,
                        member = %member.name,
                        team_project = %project_id,
                        error = %err,
                        "resume pane resolution: failed to kill dead pane before recreate"
                    );
                }
                return create_new();
            }
            Ok(false) => {
                tracing::warn!(
                    pane_id = %existing_pane_id,
                    member = %member.name,
                    team_project = %project_id,
                    "resume pane resolution: dead pane ownership mismatch, creating new pane"
                );
                return create_new();
            }
            Err(err) => {
                tracing::warn!(
                    pane_id = %existing_pane_id,
                    member = %member.name,
                    team_project = %project_id,
                    error = %err,
                    "resume pane resolution: failed ownership check for dead pane, creating new pane"
                );
                return create_new();
            }
        }
    }

    match runtime.pane_belongs_to_project(existing_pane_id, &project_id) {
        Ok(true) => Ok(PaneResolution {
            pane_id: existing_pane_id.to_string(),
            reused_pane: true,
            created_new_pane: false,
        }),
        Ok(false) => {
            tracing::warn!(
                pane_id = %existing_pane_id,
                member = %member.name,
                team_project = %project_id,
                "resume pane resolution: pane ownership mismatch, creating new pane"
            );
            create_new()
        }
        Err(err) => {
            tracing::warn!(
                pane_id = %existing_pane_id,
                member = %member.name,
                team_project = %project_id,
                error = %err,
                "resume pane resolution: ownership check failed, creating new pane"
            );
            create_new()
        }
    }
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
        let mut args = vec![
            "daemon".to_string(),
            "--pane".to_string(),
            pane_id.to_string(),
            "--team".to_string(),
            team_name.to_string(),
            "--name".to_string(),
            member_name.to_string(),
        ];
        if let Some(claude_dir) = resolve_mesh_cli_claude_dir_arg() {
            args.push("--claude-dir".to_string());
            args.push(claude_dir);
        }
        let mesh_path = mesh_cli::mesh_binary_path().unwrap_or_else(|| "mesh".to_string());
        let invocation = mesh_cli::command_invocation(&mesh_path, &args);
        let daemon_pid_path = resolve_mesh_daemon_pid_path(team_name, member_name);
        if daemon_pid_path.is_none() {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                "unable to resolve mesh daemon pid path; falling back to launcher pid"
            );
        }
        spawn_command_and_resolve_daemon_pid(&invocation, daemon_pid_path.as_deref())
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        let daemon_pid_path = resolve_team_daemon_pid_path(team_name);
        if let Some(pid_path) = daemon_pid_path.as_deref() {
            if let Some(pid) = read_pid_file(pid_path) {
                if is_process_running_by_pid_system(pid)? {
                    return Ok(pid);
                }
            }
        }

        let mut args = vec![
            "team-daemon".to_string(),
            "start".to_string(),
            "--team".to_string(),
            team_name.to_string(),
            "--name".to_string(),
            operator_name.to_string(),
        ];
        if let Some(claude_dir) = resolve_mesh_cli_claude_dir_arg() {
            args.push("--claude-dir".to_string());
            args.push(claude_dir);
        }
        let mesh_path = mesh_cli::mesh_binary_path().unwrap_or_else(|| "mesh".to_string());
        let invocation = mesh_cli::command_invocation(&mesh_path, &args);
        spawn_command_and_resolve_daemon_pid(&invocation, daemon_pid_path.as_deref())
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        if project_id.trim().is_empty() {
            return Ok(false);
        }
        let pane_path = run_tmux(&[
            "display-message".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
            "#{pane_current_path}".to_string(),
        ])?;
        Ok(normalize_path_for_compare(&pane_path) == normalize_path_for_compare(project_id))
    }

    fn find_existing_mesh_daemon_pids(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<Vec<u32>, CoordinationError> {
        find_existing_mesh_daemon_pids_system(pane_id, team_name, member_name)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        let out = run_tmux_output(&[
            "display-message".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
            "#{pane_id}".to_string(),
        ])?;
        if !out.status.success() {
            return Ok(false);
        }
        Ok(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        let out = run_tmux_output(&[
            "display-message".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
            "#{pane_dead}".to_string(),
        ])?;
        if !out.status.success() {
            return Ok(false);
        }
        let raw = String::from_utf8_lossy(&out.stdout)
            .trim()
            .to_ascii_lowercase();
        Ok(raw == "1" || raw == "true")
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        let out = run_tmux_output(&[
            "display-message".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
            "#{pane_current_command}".to_string(),
        ])?;
        if !out.status.success() {
            return Ok(false);
        }
        let raw = String::from_utf8_lossy(&out.stdout);
        Ok(is_shell_command(raw.as_ref()))
    }

    fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError> {
        let out = run_tmux_output(&[
            "display-message".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
            "#{pane_current_command}".to_string(),
        ])?;
        if !out.status.success() {
            return Ok(None);
        }
        let command = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if command.is_empty() {
            Ok(None)
        } else {
            Ok(Some(command))
        }
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
        is_process_running_by_pid_system(pid)
    }

    fn clear_mesh_daemon_pid_file(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Result<(), CoordinationError> {
        delete_pid_file_if_present(resolve_mesh_daemon_pid_path(team_name, member_name).as_deref())
    }

    fn stop_team_daemon(&self, team_name: &str) -> Result<(), CoordinationError> {
        let pid_path = resolve_team_daemon_pid_path(team_name);
        if let Some(pid_path) = pid_path.as_deref() {
            if let Some(pid) = read_pid_file(pid_path) {
                if is_process_running_by_pid_system(pid)? {
                    self.terminate_process_by_pid(pid)?;
                }
            }
        }
        delete_pid_file_if_present(pid_path.as_deref())
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
    SpawnTeamDaemon {
        team_name: String,
        operator_name: String,
    },
    FindDaemon {
        pane_id: String,
        team_name: String,
        member_name: String,
    },
    CheckPaneOwnership {
        pane_id: String,
        project_id: String,
    },
    CheckPaneExists {
        pane_id: String,
    },
    CheckPaneDead {
        pane_id: String,
    },
    CheckPaneShell {
        pane_id: String,
    },
    CheckPaneCurrentCommand {
        pane_id: String,
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
    ClearDaemonPidFile {
        team_name: String,
        member_name: String,
    },
    StopTeamDaemon {
        team_name: String,
    },
}

#[derive(Debug, Default)]
pub struct RecordingCoordinationRuntime {
    calls: Mutex<Vec<RuntimeCall>>,
    pane_exists: Mutex<HashMap<String, bool>>,
    pane_dead: Mutex<HashMap<String, bool>>,
    pane_shell: Mutex<HashMap<String, bool>>,
    pane_command: Mutex<HashMap<String, Option<String>>>,
    pane_ownership: Mutex<HashMap<String, bool>>,
    pid_running: Mutex<HashMap<u32, bool>>,
    daemon_matches: Mutex<HashMap<(String, String, String), Vec<u32>>>,
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

    pub fn set_pane_exists(&self, pane_id: &str, exists: bool) {
        if let Ok(mut map) = self.pane_exists.lock() {
            map.insert(pane_id.to_string(), exists);
        }
    }

    pub fn set_pane_dead(&self, pane_id: &str, dead: bool) {
        if let Ok(mut map) = self.pane_dead.lock() {
            map.insert(pane_id.to_string(), dead);
        }
    }

    pub fn set_pane_shell(&self, pane_id: &str, shell: bool) {
        if let Ok(mut map) = self.pane_shell.lock() {
            map.insert(pane_id.to_string(), shell);
        }
    }

    pub fn set_pane_current_command(&self, pane_id: &str, command: Option<&str>) {
        if let Ok(mut map) = self.pane_command.lock() {
            map.insert(pane_id.to_string(), command.map(ToString::to_string));
        }
    }

    pub fn set_pane_ownership(&self, pane_id: &str, matches_project: bool) {
        if let Ok(mut map) = self.pane_ownership.lock() {
            map.insert(pane_id.to_string(), matches_project);
        }
    }

    pub fn set_pid_running(&self, pid: u32, running: bool) {
        if let Ok(mut map) = self.pid_running.lock() {
            map.insert(pid, running);
        }
    }

    pub fn set_matching_daemon_pids(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
        pids: &[u32],
    ) {
        if let Ok(mut map) = self.daemon_matches.lock() {
            map.insert(
                (
                    pane_id.to_string(),
                    team_name.to_string(),
                    member_name.to_string(),
                ),
                pids.to_vec(),
            );
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
        let pane_id = format!("test-pane-{idx}");
        self.set_pane_exists(&pane_id, true);
        self.set_pane_dead(&pane_id, false);
        self.set_pane_shell(&pane_id, false);
        self.set_pane_ownership(&pane_id, true);
        Ok(pane_id)
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

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        self.push_call(RuntimeCall::SpawnTeamDaemon {
            team_name: team_name.to_string(),
            operator_name: operator_name.to_string(),
        });
        let pid = self.pid_counter.fetch_add(1, Ordering::SeqCst) + 10000;
        Ok(pid)
    }

    fn find_existing_mesh_daemon_pids(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<Vec<u32>, CoordinationError> {
        self.push_call(RuntimeCall::FindDaemon {
            pane_id: pane_id.to_string(),
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        });
        let key = (
            pane_id.to_string(),
            team_name.to_string(),
            member_name.to_string(),
        );
        Ok(self
            .daemon_matches
            .lock()
            .ok()
            .and_then(|map| map.get(&key).cloned())
            .unwrap_or_default())
    }

    fn pane_belongs_to_project(
        &self,
        pane_id: &str,
        project_id: &str,
    ) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPaneOwnership {
            pane_id: pane_id.to_string(),
            project_id: project_id.to_string(),
        });
        let matches = self
            .pane_ownership
            .lock()
            .ok()
            .and_then(|map| map.get(pane_id).copied())
            .unwrap_or(true);
        Ok(matches)
    }

    fn pane_exists(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPaneExists {
            pane_id: pane_id.to_string(),
        });
        let exists = self
            .pane_exists
            .lock()
            .ok()
            .and_then(|map| map.get(pane_id).copied())
            .unwrap_or(true);
        Ok(exists)
    }

    fn pane_is_dead(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPaneDead {
            pane_id: pane_id.to_string(),
        });
        let dead = self
            .pane_dead
            .lock()
            .ok()
            .and_then(|map| map.get(pane_id).copied())
            .unwrap_or(false);
        Ok(dead)
    }

    fn pane_is_shell(&self, pane_id: &str) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPaneShell {
            pane_id: pane_id.to_string(),
        });
        let is_shell = self
            .pane_shell
            .lock()
            .ok()
            .and_then(|map| map.get(pane_id).copied())
            .unwrap_or(false);
        Ok(is_shell)
    }

    fn pane_current_command(&self, pane_id: &str) -> Result<Option<String>, CoordinationError> {
        self.push_call(RuntimeCall::CheckPaneCurrentCommand {
            pane_id: pane_id.to_string(),
        });
        let command = self
            .pane_command
            .lock()
            .ok()
            .and_then(|map| map.get(pane_id).cloned())
            .flatten();
        Ok(command)
    }

    fn kill_aitx_pane(&self, pane_id: &str) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::KillPane {
            pane_id: pane_id.to_string(),
        });
        self.set_pane_exists(pane_id, false);
        Ok(())
    }

    fn terminate_process_by_pid(&self, pid: u32) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::TerminatePid { pid });
        Ok(())
    }

    fn is_process_running_by_pid(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPid { pid });
        let running = self
            .pid_running
            .lock()
            .ok()
            .and_then(|map| map.get(&pid).copied())
            .unwrap_or(false);
        Ok(running)
    }

    fn clear_mesh_daemon_pid_file(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::ClearDaemonPidFile {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        });
        Ok(())
    }

    fn stop_team_daemon(&self, team_name: &str) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::StopTeamDaemon {
            team_name: team_name.to_string(),
        });
        Ok(())
    }
}

fn mesh_command_invocation(args: &[&str]) -> CommandInvocation {
    mesh_cli::mesh_command_invocation(args)
}

fn spawn_command_and_resolve_daemon_pid(
    invocation: &CommandInvocation,
    daemon_pid_path: Option<&Path>,
) -> Result<u32, CoordinationError> {
    let mut child = spawn_system_command(invocation)?;
    let Some(daemon_pid_path) = daemon_pid_path else {
        return Ok(child.id());
    };

    match wait_for_daemon_pid_file(daemon_pid_path) {
        Ok(pid) => Ok(pid),
        Err(err) => {
            let launcher_status = child
                .try_wait()
                .map_err(CoordinationError::Io)?
                .map(|status| format!("launcher exited with status {status}"))
                .unwrap_or_else(|| format!("launcher pid {} still alive", child.id()));
            Err(CoordinationError::Backend(format!(
                "daemon startup verification failed for {} {}: {err}; {launcher_status}",
                invocation.program,
                invocation.args.join(" ")
            )))
        }
    }
}

fn wait_for_daemon_pid_file(daemon_pid_path: &Path) -> Result<u32, CoordinationError> {
    wait_for_daemon_pid_file_with_retries(
        daemon_pid_path,
        DAEMON_START_ATTEMPTS,
        DAEMON_START_INTERVAL,
    )
}

fn wait_for_daemon_pid_file_with_retries(
    daemon_pid_path: &Path,
    attempts: usize,
    interval: Duration,
) -> Result<u32, CoordinationError> {
    for attempt in 0..attempts {
        if let Some(pid) = read_pid_file(daemon_pid_path) {
            if is_process_running_by_pid_system(pid)? {
                return Ok(pid);
            }
        }
        if attempt + 1 < attempts {
            thread::sleep(interval);
        }
    }

    Err(CoordinationError::Backend(format!(
        "timed out waiting for live daemon pid at {}",
        daemon_pid_path.display()
    )))
}

fn read_pid_file(pid_path: &Path) -> Option<u32> {
    fs::read_to_string(pid_path)
        .ok()
        .and_then(|raw| raw.trim().parse::<u32>().ok())
}

fn delete_pid_file_if_present(pid_path: Option<&Path>) -> Result<(), CoordinationError> {
    let Some(pid_path) = pid_path else {
        return Ok(());
    };
    match fs::remove_file(pid_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(CoordinationError::Io(err)),
    }
}

fn find_existing_mesh_daemon_pids_system(
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<Vec<u32>, CoordinationError> {
    let mut matches = Vec::new();

    if let Some(pid_path) = resolve_mesh_daemon_pid_path(team_name, member_name) {
        if let Some(pid) = read_pid_file(&pid_path) {
            if is_process_running_by_pid_system(pid)?
                && process_matches_mesh_daemon(pid, pane_id, team_name, member_name)?
            {
                matches.push(pid);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        for entry in fs::read_dir("/proc").map_err(CoordinationError::Io)? {
            let entry = entry.map_err(CoordinationError::Io)?;
            let raw_name = entry.file_name();
            let Ok(pid) = raw_name.to_string_lossy().parse::<u32>() else {
                continue;
            };
            if matches.contains(&pid) {
                continue;
            }
            if process_matches_mesh_daemon(pid, pane_id, team_name, member_name)? {
                matches.push(pid);
            }
        }
    }

    Ok(matches)
}

fn resolve_mesh_daemon_pid_path(team_name: &str, member_name: &str) -> Option<PathBuf> {
    resolve_host_claude_dir().map(|claude_dir| {
        claude_dir
            .join("teams")
            .join(team_name)
            .join("daemons")
            .join(format!("{member_name}.pid"))
    })
}

fn resolve_team_daemon_pid_path(team_name: &str) -> Option<PathBuf> {
    resolve_host_claude_dir().map(|claude_dir| {
        claude_dir
            .join("teams")
            .join(team_name)
            .join("daemons")
            .join("team.pid")
    })
}

fn resolve_host_claude_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        mesh_cli::resolve_windows_mesh_teams_dir()
            .and_then(|teams_dir| teams_dir.parent().map(Path::to_path_buf))
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = std::env::var_os(CLAUDE_DIR_OVERRIDE_ENV) {
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
        dirs::home_dir().map(|home| home.join(".claude"))
    }
}

fn resolve_mesh_cli_claude_dir_arg() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        mesh_cli::resolve_wsl_home_for_coordination().map(|home| format!("{home}/.claude"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        resolve_host_claude_dir().map(|path| path.to_string_lossy().to_string())
    }
}

fn process_matches_mesh_daemon(
    pid: u32,
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> Result<bool, CoordinationError> {
    #[cfg(target_os = "windows")]
    {
        let _ = (pid, pane_id, team_name, member_name);
        Ok(true)
    }

    #[cfg(not(target_os = "windows"))]
    {
        if !is_process_running_by_pid_system(pid)? {
            return Ok(false);
        }
        let cmdline_path = PathBuf::from("/proc").join(pid.to_string()).join("cmdline");
        let raw = match fs::read(cmdline_path) {
            Ok(raw) => raw,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(CoordinationError::Io(err)),
        };
        if raw.is_empty() {
            return Ok(false);
        }
        let args = raw
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).to_string())
            .collect::<Vec<_>>();
        Ok(command_matches_mesh_daemon(
            &args,
            pane_id,
            team_name,
            member_name,
        ))
    }
}

fn command_matches_mesh_daemon(
    args: &[String],
    pane_id: &str,
    team_name: &str,
    member_name: &str,
) -> bool {
    let Some(program) = args.first() else {
        return false;
    };
    let binary = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    if binary != "mesh" && binary != "mesh.exe" {
        return false;
    }
    args.iter().any(|arg| arg == "daemon")
        && command_has_flag_value(args, "--pane", pane_id)
        && command_has_flag_value(args, "--team", team_name)
        && command_has_flag_value(args, "--name", member_name)
}

fn command_has_flag_value(args: &[String], flag: &str, expected: &str) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == flag && pair[1] == expected)
}

fn is_process_running_by_pid_system(pid: u32) -> Result<bool, CoordinationError> {
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

fn normalize_path_for_compare(raw: &str) -> String {
    let mut value = raw.trim().replace('\\', "/");
    while value.contains("//") {
        value = value.replace("//", "/");
    }
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
    value
}

fn is_shell_command(raw: &str) -> bool {
    let command = raw
        .trim()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_start_matches('-')
        .to_ascii_lowercase();
    matches!(command.as_str(), "bash" | "zsh" | "sh" | "fish")
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
    use crate::coordination::domain::{HealthState, Member, MemberRole};
    use crate::coordination::stores::MemberRuntimeRecord;
    use std::path::PathBuf;

    fn sample_member(name: &str, project_path: &str) -> Member {
        Member {
            name: name.to_string(),
            role: MemberRole::Agent,
            role_id: None,
            role_name: None,
            focus_area: None,
            context_summary: None,
            behavior_summary: None,
            instructions: None,
            behavioral_contract: None,
            capabilities: None,
            project_path: PathBuf::from(project_path),
            cli_tool: CliTool::Codex,
        }
    }

    fn sample_runtime_with_pane(member_name: &str, pane_id: &str) -> MemberRuntimeRecord {
        MemberRuntimeRecord {
            schema_version: 1,
            member_name: member_name.to_string(),
            pane_id: Some(pane_id.to_string()),
            session_id: None,
            daemon_pid: None,
            health: HealthState::SessionDead,
            delivery_lease: None,
            attached_at: None,
            last_seen_at: None,
        }
    }

    #[test]
    fn tmux_target_uses_pane_id_when_present() {
        assert_eq!(tmux_target_for_pane("%12"), "%12");
    }

    #[test]
    fn tmux_target_wraps_numeric_index() {
        assert_eq!(tmux_target_for_pane("3"), ":.3");
    }

    #[test]
    fn normalize_path_for_compare_handles_slashes_and_trailing_separator() {
        assert_eq!(
            normalize_path_for_compare("/home/mstie/projects/taurhaus/"),
            "/home/mstie/projects/taurhaus"
        );
        assert_eq!(
            normalize_path_for_compare("\\\\home\\\\mstie\\\\projects\\\\taurhaus\\\\"),
            "/home/mstie/projects/taurhaus"
        );
    }

    #[test]
    fn shell_command_detection_matches_supported_shells() {
        assert!(is_shell_command("bash"));
        assert!(is_shell_command("zsh"));
        assert!(is_shell_command("/usr/bin/fish"));
        assert!(is_shell_command("-sh"));
    }

    #[test]
    fn shell_command_detection_rejects_non_shell_commands() {
        assert!(!is_shell_command("codex"));
        assert!(!is_shell_command("claude"));
        assert!(!is_shell_command(""));
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
        assert!(runtime
            .pane_belongs_to_project(&pane, "/tmp/project")
            .expect("ownership check"));
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
                RuntimeCall::CheckPaneOwnership {
                    pane_id: "test-pane-1".to_string(),
                    project_id: "/tmp/project".to_string()
                },
                RuntimeCall::TerminatePid { pid: 10000 },
                RuntimeCall::KillPane {
                    pane_id: "test-pane-1".to_string()
                },
                RuntimeCall::CheckPid { pid: 10000 },
            ]
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn wait_for_daemon_pid_file_with_retries_rejects_stale_pid() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let pid_path = dir.path().join("agent.pid");
        std::fs::write(&pid_path, "0\n").expect("write pid");

        let err = wait_for_daemon_pid_file_with_retries(&pid_path, 1, Duration::ZERO)
            .expect_err("stale pid should be rejected");

        assert!(err
            .to_string()
            .contains("timed out waiting for live daemon pid"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn spawn_command_and_resolve_daemon_pid_returns_pid_file_process() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let script_path = dir.path().join("spawn-daemon.sh");
        let pid_path = dir.path().join("agent.pid");

        std::fs::write(
            &script_path,
            "#!/bin/sh\nsleep 5 &\nprintf '%s\\n' \"$!\" > \"$1\"\n",
        )
        .expect("write script");

        let invocation = CommandInvocation {
            program: "sh".to_string(),
            args: vec![
                script_path.to_string_lossy().to_string(),
                pid_path.to_string_lossy().to_string(),
            ],
        };

        let pid = spawn_command_and_resolve_daemon_pid(&invocation, Some(&pid_path))
            .expect("resolve daemon pid");
        let recorded_pid = std::fs::read_to_string(&pid_path)
            .expect("read pid")
            .trim()
            .parse::<u32>()
            .expect("parse pid");

        assert_eq!(pid, recorded_pid);
        assert!(is_process_running_by_pid_system(pid).expect("pid should be running"));

        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }

    #[test]
    fn resolve_or_create_pane_missing_creates_new_pane() {
        let runtime = RecordingCoordinationRuntime::default();
        let member = sample_member("agent-a", "/tmp/project");

        let resolution = resolve_or_create_pane_for_member(&runtime, &member, None, "new_window")
            .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
    }

    #[test]
    fn resolve_or_create_pane_lookup_fail_creates_new_pane() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", false);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
        assert!(runtime.calls().contains(&RuntimeCall::CheckPaneExists {
            pane_id: "%9".to_string()
        }));
    }

    #[test]
    fn resolve_or_create_pane_dead_and_owned_kills_then_recreates() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", true);
        runtime.set_pane_ownership("%9", true);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
        let calls = runtime.calls();
        assert!(calls.contains(&RuntimeCall::KillPane {
            pane_id: "%9".to_string()
        }));
    }

    #[test]
    fn resolve_or_create_pane_alive_and_owned_reuses_existing() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_ownership("%9", true);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(!resolution.created_new_pane);
        assert!(resolution.reused_pane);
        assert_eq!(resolution.pane_id, "%9");
        let calls = runtime.calls();
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CreatePane { .. })));
    }

    #[test]
    fn resolve_or_create_pane_ownership_mismatch_creates_without_kill() {
        let runtime = RecordingCoordinationRuntime::default();
        runtime.set_pane_exists("%9", true);
        runtime.set_pane_dead("%9", false);
        runtime.set_pane_ownership("%9", false);
        let member = sample_member("agent-a", "/tmp/project");
        let record = sample_runtime_with_pane("agent-a", "%9");

        let resolution =
            resolve_or_create_pane_for_member(&runtime, &member, Some(&record), "new_window")
                .expect("resolution");

        assert!(resolution.created_new_pane);
        assert!(!resolution.reused_pane);
        assert_eq!(resolution.pane_id, "test-pane-1");
        let calls = runtime.calls();
        assert!(calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::CreatePane { .. })));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, RuntimeCall::KillPane { pane_id } if pane_id == "%9")));
    }
}
