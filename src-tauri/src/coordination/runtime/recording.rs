use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;

use super::{CoordinationRuntime, DetectedRuntimeSession};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCall {
    CreatePane {
        project_id: String,
    },
    CreatePaneInTarget {
        project_id: String,
        target_pane: String,
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
        model: String,
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
    FindDaemonByMember {
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
    CheckPidCurrentMeshBinary {
        pid: u32,
    },
    CheckTeamDaemonCurrentMeshBinary {
        team_name: String,
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
    send_keys_failures_remaining: Mutex<HashMap<String, usize>>,
    send_keys_failure_message: Mutex<HashMap<String, String>>,
    pid_running: Mutex<HashMap<u32, bool>>,
    pid_current_mesh_binary: Mutex<HashMap<u32, bool>>,
    team_daemon_current_mesh_binary: Mutex<HashMap<String, bool>>,
    daemon_matches: Mutex<HashMap<(String, String, String), Vec<u32>>>,
    member_daemon_pid_matches: Mutex<HashMap<(String, String), Option<u32>>>,
    detected_runtime_sessions: Mutex<HashMap<(String, CliTool), DetectedRuntimeSession>>,
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

    pub fn set_send_keys_failures(&self, pane_id: &str, failures: usize, message: &str) {
        if let Ok(mut map) = self.send_keys_failures_remaining.lock() {
            map.insert(pane_id.to_string(), failures);
        }
        if let Ok(mut map) = self.send_keys_failure_message.lock() {
            map.insert(pane_id.to_string(), message.to_string());
        }
    }

    pub fn set_pid_running(&self, pid: u32, running: bool) {
        if let Ok(mut map) = self.pid_running.lock() {
            map.insert(pid, running);
        }
    }

    pub fn set_pid_current_mesh_binary(&self, pid: u32, current: bool) {
        if let Ok(mut map) = self.pid_current_mesh_binary.lock() {
            map.insert(pid, current);
        }
    }

    pub fn set_team_daemon_current_mesh_binary(&self, team_name: &str, current: bool) {
        if let Ok(mut map) = self.team_daemon_current_mesh_binary.lock() {
            map.insert(team_name.to_string(), current);
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

    pub fn set_member_daemon_pid_match(
        &self,
        team_name: &str,
        member_name: &str,
        pid: Option<u32>,
    ) {
        if let Ok(mut map) = self.member_daemon_pid_matches.lock() {
            map.insert((team_name.to_string(), member_name.to_string()), pid);
        }
    }

    pub fn set_detected_runtime_session(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
        session_id: Option<&str>,
        jsonl_path: Option<&str>,
    ) {
        if let Ok(mut map) = self.detected_runtime_sessions.lock() {
            map.insert(
                (pane_id.to_string(), cli_tool),
                DetectedRuntimeSession {
                    session_id: session_id.map(ToString::to_string),
                    jsonl_path: jsonl_path.map(PathBuf::from),
                },
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
        if let Ok(mut map) = self.send_keys_failures_remaining.lock() {
            let remaining = map.get_mut(pane_id);
            if let Some(remaining) = remaining {
                if *remaining > 0 {
                    *remaining -= 1;
                    let message = self
                        .send_keys_failure_message
                        .lock()
                        .ok()
                        .and_then(|messages| messages.get(pane_id).cloned())
                        .unwrap_or_else(|| "forced send-keys failure".to_string());
                    return Err(CoordinationError::Backend(message));
                }
            }
        }
        Ok(())
    }

    fn create_aitx_pane_and_launch_in_target(
        &self,
        project_id: &str,
        target_pane: &str,
        _launch_cmd: &str,
    ) -> Result<String, CoordinationError> {
        self.push_call(RuntimeCall::CreatePaneInTarget {
            project_id: project_id.to_string(),
            target_pane: target_pane.to_string(),
        });
        let idx = self.pane_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let pane_id = format!("test-pane-{idx}");
        self.set_pane_exists(&pane_id, true);
        self.set_pane_dead(&pane_id, false);
        self.set_pane_shell(&pane_id, false);
        self.set_pane_ownership(&pane_id, true);
        Ok(pane_id)
    }

    fn detect_session_id(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<Option<String>, CoordinationError> {
        Ok(self.detect_runtime_session(pane_id, cli_tool)?.session_id)
    }

    fn detect_runtime_session(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<DetectedRuntimeSession, CoordinationError> {
        self.push_call(RuntimeCall::DetectSessionId {
            pane_id: pane_id.to_string(),
            cli_tool,
        });
        let key = (pane_id.to_string(), cli_tool);
        Ok(self
            .detected_runtime_sessions
            .lock()
            .ok()
            .and_then(|map| map.get(&key).cloned())
            .unwrap_or_else(|| DetectedRuntimeSession {
                session_id: Some(format!("session-{pane_id}")),
                jsonl_path: None,
            }))
    }

    fn join_mesh(
        &self,
        team_name: &str,
        member_name: &str,
        project_id: &str,
        model: &str,
    ) -> Result<(), CoordinationError> {
        self.push_call(RuntimeCall::JoinMesh {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
            project_id: project_id.to_string(),
            model: model.to_string(),
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

    fn find_existing_mesh_daemon_pid_by_member(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Result<Option<u32>, CoordinationError> {
        self.push_call(RuntimeCall::FindDaemonByMember {
            team_name: team_name.to_string(),
            member_name: member_name.to_string(),
        });
        let key = (team_name.to_string(), member_name.to_string());
        Ok(self
            .member_daemon_pid_matches
            .lock()
            .ok()
            .and_then(|map| map.get(&key).cloned())
            .flatten())
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

    fn mesh_daemon_uses_current_binary(&self, pid: u32) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckPidCurrentMeshBinary { pid });
        let current = self
            .pid_current_mesh_binary
            .lock()
            .ok()
            .and_then(|map| map.get(&pid).copied())
            .unwrap_or(true);
        Ok(current)
    }

    fn team_daemon_uses_current_binary(&self, team_name: &str) -> Result<bool, CoordinationError> {
        self.push_call(RuntimeCall::CheckTeamDaemonCurrentMeshBinary {
            team_name: team_name.to_string(),
        });
        let current = self
            .team_daemon_current_mesh_binary
            .lock()
            .ok()
            .and_then(|map| map.get(team_name).copied())
            .unwrap_or(true);
        Ok(current)
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
