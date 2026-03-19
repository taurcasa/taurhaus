use std::path::PathBuf;
use std::thread;

use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;

use super::process::{
    delete_pid_file_if_present, find_existing_mesh_daemon_pids_system,
    is_process_running_by_pid_system, process_matches_team_daemon,
    process_uses_current_mesh_binary, read_pid_file, resolve_mesh_cli_claude_dir_arg,
    resolve_mesh_daemon_pid_path, resolve_team_daemon_pid_path, run_mesh, run_system_command,
    spawn_mesh_daemon_command_and_resolve_pid, spawn_system_command, terminate_pid_invocation,
    validated_mesh_daemon_pid_file_by_member, validated_team_daemon_pid_file,
    wait_for_team_daemon_pid_file,
};
use super::tmux::{
    create_tmux_pane_with_layout, is_shell_command, run_tmux, run_tmux_output, tmux_target_for_pane,
};
use super::{
    CoordinationRuntime, DetectedRuntimeSession, SESSION_DETECT_ATTEMPTS, SESSION_DETECT_INTERVAL,
    TMUX_POST_ENTER_DELAY, TMUX_TEXT_TO_ENTER_DELAY,
};

#[derive(Debug, Default)]
pub struct SystemCoordinationRuntime;

#[derive(Debug, Clone)]
struct RuntimeSessionInfo {
    tmux_pane: Option<String>,
    cli_tool: CliTool,
    session_id: Option<String>,
    jsonl_path: Option<PathBuf>,
}

#[cfg(not(test))]
fn collect_runtime_sessions() -> Vec<RuntimeSessionInfo> {
    crate::session_scanner::scan_sessions_for_runtime()
        .into_iter()
        .map(|session| RuntimeSessionInfo {
            tmux_pane: session.tmux_pane,
            cli_tool: session.cli_tool,
            session_id: session.session_id,
            jsonl_path: session.jsonl_path.map(PathBuf::from),
        })
        .collect()
}

#[cfg(test)]
fn collect_runtime_sessions() -> Vec<RuntimeSessionInfo> {
    Vec::new()
}

impl CoordinationRuntime for SystemCoordinationRuntime {
    fn create_aitx_pane(
        &self,
        project_id: &str,
        tmux_layout: &str,
    ) -> Result<String, CoordinationError> {
        create_tmux_pane_with_layout(project_id, tmux_layout)
    }

    fn create_aitx_pane_and_launch(
        &self,
        project_id: &str,
        tmux_layout: &str,
        launch_cmd: &str,
    ) -> Result<String, CoordinationError> {
        let (_, _, pane_id) = crate::session_scanner::control::launch_command_in_tmux_with_layout(
            project_id,
            tmux_layout,
            launch_cmd,
        )
        .map_err(CoordinationError::Backend)?;
        Ok(pane_id)
    }

    fn create_aitx_pane_and_launch_in_target(
        &self,
        project_id: &str,
        target_pane: &str,
        launch_cmd: &str,
    ) -> Result<String, CoordinationError> {
        crate::session_scanner::control::split_command_in_tmux_target_pane(
            project_id,
            target_pane,
            launch_cmd,
        )
        .map_err(CoordinationError::Backend)
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
        Ok(self.detect_runtime_session(pane_id, cli_tool)?.session_id)
    }

    fn detect_runtime_session(
        &self,
        pane_id: &str,
        cli_tool: CliTool,
    ) -> Result<DetectedRuntimeSession, CoordinationError> {
        for _ in 0..SESSION_DETECT_ATTEMPTS {
            let matched = collect_runtime_sessions().into_iter().find(|session| {
                session.tmux_pane.as_deref() == Some(pane_id) && session.cli_tool == cli_tool
            });

            if let Some(session) = matched {
                return Ok(DetectedRuntimeSession {
                    session_id: session.session_id,
                    jsonl_path: session.jsonl_path,
                });
            }

            thread::sleep(SESSION_DETECT_INTERVAL);
        }

        Ok(DetectedRuntimeSession::default())
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
        let args_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let invocation =
            super::mesh_command_invocation_for_member(&args_refs, team_name, member_name);
        let daemon_pid_path = resolve_mesh_daemon_pid_path(team_name, member_name);
        if daemon_pid_path.is_none() {
            tracing::warn!(
                team = %team_name,
                member = %member_name,
                "unable to resolve mesh daemon pid path; falling back to launcher pid"
            );
        }
        spawn_mesh_daemon_command_and_resolve_pid(
            &invocation,
            daemon_pid_path.as_deref(),
            pane_id,
            team_name,
            member_name,
        )
    }

    fn spawn_team_daemon(
        &self,
        team_name: &str,
        operator_name: &str,
    ) -> Result<u32, CoordinationError> {
        let daemon_pid_path = resolve_team_daemon_pid_path(team_name);
        if let Some(pid_path) = daemon_pid_path.as_deref() {
            if let Some(pid) = validated_team_daemon_pid_file(pid_path, team_name, true)? {
                return Ok(pid);
            }
            if pid_path.exists() {
                if let Err(err) = delete_pid_file_if_present(Some(pid_path)) {
                    tracing::warn!(
                        team = %team_name,
                        path = %pid_path.display(),
                        error = %err,
                        "failed to clear invalid team daemon pid file before restart"
                    );
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
        let args_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let invocation =
            super::mesh_command_invocation_for_member(&args_refs, team_name, operator_name);
        let mut child = spawn_system_command(&invocation)?;

        let Some(pid_path) = daemon_pid_path.as_deref() else {
            return Ok(child.id());
        };

        match wait_for_team_daemon_pid_file(pid_path, team_name) {
            Ok(pid) => Ok(pid),
            Err(err) => {
                let launcher_status = child
                    .try_wait()
                    .map_err(CoordinationError::Io)?
                    .map(|status| format!("launcher exited with status {status}"))
                    .unwrap_or_else(|| format!("launcher pid {} still alive", child.id()));
                Err(CoordinationError::Backend(format!(
                    "team daemon startup verification failed for {} {}: {err}; {launcher_status}",
                    invocation.program,
                    invocation.args.join(" ")
                )))
            }
        }
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
        Ok(crate::provider::path::normalize_project_path(&pane_path)
            == crate::provider::path::normalize_project_path(project_id))
    }

    fn find_existing_mesh_daemon_pids(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
    ) -> Result<Vec<u32>, CoordinationError> {
        find_existing_mesh_daemon_pids_system(pane_id, team_name, member_name)
    }

    fn find_existing_mesh_daemon_pid_by_member(
        &self,
        team_name: &str,
        member_name: &str,
    ) -> Result<Option<u32>, CoordinationError> {
        let Some(pid_path) = resolve_mesh_daemon_pid_path(team_name, member_name) else {
            return Ok(None);
        };
        validated_mesh_daemon_pid_file_by_member(&pid_path, team_name, member_name)
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
        let invocation = terminate_pid_invocation(pid, false)?;
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

    fn mesh_daemon_uses_current_binary(&self, pid: u32) -> Result<bool, CoordinationError> {
        process_uses_current_mesh_binary(pid)
    }

    fn team_daemon_uses_current_binary(&self, team_name: &str) -> Result<bool, CoordinationError> {
        let Some(pid_path) = resolve_team_daemon_pid_path(team_name) else {
            return Ok(true);
        };
        let Some(pid) = read_pid_file(&pid_path) else {
            return Ok(true);
        };
        if !is_process_running_by_pid_system(pid)? {
            return Ok(true);
        }
        if !process_matches_team_daemon(pid, team_name)? {
            return Ok(true);
        }
        process_uses_current_mesh_binary(pid)
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
                    if !process_matches_team_daemon(pid, team_name)? {
                        return Err(CoordinationError::Backend(format!(
                            "refusing to stop pid {pid}: process is not the expected mesh team daemon for {team_name}"
                        )));
                    }
                    self.terminate_process_by_pid(pid)?;
                }
            }
        }
        delete_pid_file_if_present(pid_path.as_deref())
    }
}
