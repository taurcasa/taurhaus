use std::path::PathBuf;
use std::thread;

use crate::coordination::errors::CoordinationError;
use crate::session_scanner::cli_tool::CliTool;

use super::process::{
    delete_pid_file_if_present, find_existing_mesh_daemon_pids_system,
    is_process_running_by_pid_system, process_matches_team_daemon,
    process_uses_current_mesh_binary, read_pid_file, resolve_mesh_daemon_pid_path,
    resolve_team_daemon_pid_path, run_mesh, run_system_command,
    spawn_mesh_daemon_command_and_resolve_pid, spawn_system_command, terminate_pid_invocation,
    validated_mesh_daemon_pid_file_by_member, validated_team_daemon_pid_file,
    wait_for_team_daemon_pid_file,
};
use super::tmux::{
    create_tmux_pane_with_layout, is_shell_command, run_tmux, run_tmux_output, tmux_target_for_pane,
};
use super::{
    CoordinationRuntime, DetectedRuntimeSession, LivePane, SESSION_DETECT_ATTEMPTS,
    SESSION_DETECT_INTERVAL, TMUX_POST_ENTER_DELAY, TMUX_TEXT_TO_ENTER_DELAY,
};

// tmux 3.4 does not define `pane_start_time`; the empty slot keeps the wire
// shape stable while Linux fills it from /proc process start ticks below.
// macOS and Windows therefore retain only the pane PID portion of identity.
const LIVE_PANE_FORMAT: &str = "#{pane_id}\t#{pane_pid}\t#{pane_start_time}\t#{pane_dead}\t#{pane_current_command}\t#{pane_current_path}";

#[derive(Debug, Default)]
pub struct SystemCoordinationRuntime;

#[derive(Debug, Clone)]
struct RuntimeSessionInfo {
    tmux_pane: Option<String>,
    cli_tool: CliTool,
    session_id: Option<String>,
    jsonl_path: Option<PathBuf>,
}

/// One runtime scan for session identity detection: the sessions and whether
/// the scan was degraded (process inventory unreadable — or, on Windows, the
/// WSL daemon's scanner degraded — so the sessions are a last good snapshot
/// rather than an observation).
fn scan_runtime_sessions() -> (Vec<RuntimeSessionInfo>, bool) {
    let (sessions, degraded) = crate::session_scanner::scan_sessions_for_runtime();
    let sessions = sessions
        .into_iter()
        .map(|session| RuntimeSessionInfo {
            tmux_pane: session.tmux_pane,
            cli_tool: session.cli_tool,
            session_id: session.session_id,
            jsonl_path: session.jsonl_path.map(PathBuf::from),
        })
        .collect();
    (sessions, degraded)
}

#[cfg(not(test))]
fn collect_runtime_sessions() -> (Vec<RuntimeSessionInfo>, bool) {
    scan_runtime_sessions()
}

/// Test seam: stands in for the scanner so identity detection can be driven
/// through healthy and degraded scans.
///
/// Scoped to the thread that installs it. Detection is synchronous, so the
/// installing test is the only caller its scan can reach — a sibling test
/// polling the scanner in parallel is served the ordinary empty scan and can
/// neither observe nor consume someone else's script.
#[cfg(test)]
type RuntimeScanOverride = fn() -> (Vec<RuntimeSessionInfo>, bool);
#[cfg(test)]
thread_local! {
    static RUNTIME_SCAN_OVERRIDE: std::cell::Cell<Option<RuntimeScanOverride>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn collect_runtime_sessions() -> (Vec<RuntimeSessionInfo>, bool) {
    match RUNTIME_SCAN_OVERRIDE.with(std::cell::Cell::get) {
        Some(scan) => scan(),
        None => (Vec::new(), false),
    }
}

/// Install or clear this thread's scan override.
#[cfg(test)]
fn set_runtime_scan_override(scan: Option<RuntimeScanOverride>) {
    RUNTIME_SCAN_OVERRIDE.with(|slot| slot.set(scan));
}

/// Test seam for scanner-level tests: routes identity detection through the
/// real `scan_sessions_for_runtime` for as long as the guard lives, on the
/// thread that installed it.
#[cfg(test)]
pub(crate) struct RealRuntimeScan {
    /// The override lives on the installing thread, so the guard must not
    /// travel to another one and clear the wrong slot.
    _not_send: std::marker::PhantomData<*const ()>,
}

#[cfg(test)]
impl RealRuntimeScan {
    pub(crate) fn install() -> Self {
        set_runtime_scan_override(Some(scan_runtime_sessions));
        Self {
            _not_send: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
impl Drop for RealRuntimeScan {
    fn drop(&mut self) {
        set_runtime_scan_override(None);
    }
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
        member_type: &str,
        model: &str,
        claude_dir: &str,
    ) -> Result<(), CoordinationError> {
        run_mesh(
            &[
                "join",
                "--team",
                team_name,
                "--name",
                member_name,
                "--type",
                member_type,
                "--model",
                model,
                "--claude-dir",
                claude_dir,
            ],
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
            let (sessions, degraded) = collect_runtime_sessions();
            // Identity binding: a degraded scan hands back the scanner's last
            // good snapshot, which can still map this pane to the previous
            // CLI's transcript. That is no observation — never match it, poll
            // again.
            if !degraded {
                let matched = sessions.into_iter().find(|session| {
                    session.tmux_pane.as_deref() == Some(pane_id) && session.cli_tool == cli_tool
                });

                if let Some(session) = matched {
                    return Ok(DetectedRuntimeSession {
                        session_id: session.session_id,
                        jsonl_path: session.jsonl_path,
                    });
                }
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
        self.spawn_mesh_daemon_at_root(
            pane_id,
            team_name,
            member_name,
            &crate::provider::platform_paths::PlatformPaths::teams_dir(),
        )
    }

    fn spawn_mesh_daemon_at_root(
        &self,
        pane_id: &str,
        team_name: &str,
        member_name: &str,
        teams_dir: &std::path::Path,
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
        if let Some(claude_dir) = teams_dir.parent() {
            args.push("--claude-dir".to_string());
            args.push(super::process::mesh_cli_claude_dir_arg_from_path(
                claude_dir,
            ));
        }
        let args_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let invocation = super::process::mesh_command_invocation_for_member_at(
            &args_refs,
            team_name,
            member_name,
            teams_dir,
        );
        let daemon_pid_path = Some(
            teams_dir
                .join(team_name)
                .join("daemons")
                .join(format!("{member_name}.pid")),
        );
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
        self.spawn_team_daemon_at_root(
            team_name,
            operator_name,
            &crate::provider::platform_paths::PlatformPaths::teams_dir(),
        )
    }

    fn spawn_team_daemon_at_root(
        &self,
        team_name: &str,
        operator_name: &str,
        teams_dir: &std::path::Path,
    ) -> Result<u32, CoordinationError> {
        let daemon_pid_path = Some(teams_dir.join(team_name).join("daemons").join("team.pid"));
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
        if let Some(claude_dir) = teams_dir.parent() {
            args.push("--claude-dir".to_string());
            args.push(super::process::mesh_cli_claude_dir_arg_from_path(
                claude_dir,
            ));
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
        let Some(live_pane) = self.live_pane(pane_id)? else {
            return Ok(false);
        };
        let Some(pane_path) = live_pane.current_path else {
            return Ok(false);
        };
        Ok(
            crate::provider::path::normalize_project_path(&pane_path.display().to_string())
                == crate::provider::path::normalize_project_path(project_id),
        )
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

    fn live_pane(&self, pane_id: &str) -> Result<Option<LivePane>, CoordinationError> {
        let out = run_tmux_output(&[
            "display-message".to_string(),
            "-p".to_string(),
            "-t".to_string(),
            tmux_target_for_pane(pane_id),
            LIVE_PANE_FORMAT.to_string(),
        ])?;
        if !out.status.success() {
            return Ok(None);
        }
        let Some(mut live_pane) =
            parse_live_pane_output(&String::from_utf8_lossy(&out.stdout), pane_id)?
        else {
            return Ok(None);
        };
        if live_pane.pane_start_time.is_none() {
            live_pane.pane_start_time = live_pane
                .pane_pid
                .and_then(taurhaus_lib::platform::process_start_ticks);
        }
        Ok(Some(live_pane))
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

fn parse_live_pane(raw: &str) -> Option<LivePane> {
    let mut fields = raw.trim_end_matches(['\r', '\n']).splitn(6, '\t');
    let pane_id = fields.next()?.trim();
    if pane_id.is_empty() {
        return None;
    }
    let pane_pid = fields.next()?.trim().parse::<u32>().ok();
    let pane_start_time = fields.next()?.trim().parse::<u64>().ok();
    let is_dead = matches!(fields.next()?.trim(), "1" | "true");
    let current_command = non_empty(fields.next()?);
    let current_path = non_empty(fields.next()?).map(PathBuf::from);
    Some(LivePane {
        pane_id: pane_id.to_string(),
        pane_pid,
        pane_start_time,
        current_command,
        current_path,
        is_dead,
    })
}

fn parse_live_pane_output(raw: &str, pane_id: &str) -> Result<Option<LivePane>, CoordinationError> {
    if raw
        .chars()
        .all(|character| character == '\t' || character.is_whitespace())
    {
        return Ok(None);
    }
    parse_live_pane(raw).map(Some).ok_or_else(|| {
        CoordinationError::Backend(format!(
            "tmux returned malformed pane identity for {pane_id}"
        ))
    })
}

fn non_empty(raw: &str) -> Option<String> {
    let value = raw.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        /// Scans the installed script has served on this thread.
        static SCAN_CALLS: Cell<usize> = const { Cell::new(0) };
        /// Scans before this count are degraded; the rest are healthy.
        static DEGRADED_SCANS: Cell<usize> = const { Cell::new(usize::MAX) };
    }

    const PANE: &str = "%7";

    fn stale_cached_session() -> RuntimeSessionInfo {
        RuntimeSessionInfo {
            tmux_pane: Some(PANE.to_string()),
            cli_tool: CliTool::Codex,
            session_id: Some("stale-session".to_string()),
            jsonl_path: Some(PathBuf::from("/tmp/stale-session.jsonl")),
        }
    }

    fn fresh_session() -> RuntimeSessionInfo {
        RuntimeSessionInfo {
            tmux_pane: Some(PANE.to_string()),
            cli_tool: CliTool::Codex,
            session_id: Some("fresh-session".to_string()),
            jsonl_path: Some(PathBuf::from("/tmp/fresh-session.jsonl")),
        }
    }

    #[test]
    fn live_pane_parser_captures_tmux_identity_and_foreground_command() {
        // Regression: mesh-findings P3, tmux reused pane ids; daemons for
        // taurrust/gotaurus/espn pointed at claude panes.
        let pane = parse_live_pane("%9\t4242\t1755000000\t0\tclaude\t/tmp/taurhaus\n")
            .expect("parse live pane");

        assert_eq!(pane.pane_id, "%9");
        assert_eq!(pane.pane_pid, Some(4242));
        assert_eq!(pane.pane_start_time, Some(1_755_000_000));
        assert_eq!(pane.current_command.as_deref(), Some("claude"));
        assert_eq!(pane.current_path, Some(PathBuf::from("/tmp/taurhaus")));
        assert!(!pane.is_dead);
    }

    #[test]
    fn vanished_pane_output_is_not_a_malformed_identity_error() {
        // Regression: aecc8ac treated tmux's successful empty response for a
        // vanished pane as malformed, aborting resume and team-daemon repair.
        assert!(parse_live_pane("\t\t\t\t\t\n").is_none());
        assert!(parse_live_pane_output("\t\t\t\t\t\n", "%20")
            .expect("vanished pane is not an error")
            .is_none());
    }

    #[test]
    fn nonempty_malformed_pane_output_remains_an_error() {
        assert!(parse_live_pane_output("%20\tbroken\n", "%20").is_err());
    }

    /// Degraded scans hand back the last good snapshot, which still maps the
    /// pane to the previous CLI's transcript; healthy scans see the new one.
    fn scripted_scan() -> (Vec<RuntimeSessionInfo>, bool) {
        let call = SCAN_CALLS.with(|calls| {
            let call = calls.get();
            calls.set(call + 1);
            call
        });
        if call < DEGRADED_SCANS.with(Cell::get) {
            (vec![stale_cached_session()], true)
        } else {
            (vec![fresh_session()], false)
        }
    }

    /// Owns the script and its counter for the test that installs it: both live
    /// on this thread, so no sibling test can be served or counted here.
    struct ScanOverride {
        _not_send: std::marker::PhantomData<*const ()>,
    }

    impl ScanOverride {
        fn install(degraded_scans: usize) -> Self {
            SCAN_CALLS.with(|calls| calls.set(0));
            DEGRADED_SCANS.with(|scans| scans.set(degraded_scans));
            set_runtime_scan_override(Some(scripted_scan));
            Self {
                _not_send: std::marker::PhantomData,
            }
        }

        /// Scans this guard's script has served.
        fn calls(&self) -> usize {
            SCAN_CALLS.with(Cell::get)
        }
    }

    impl Drop for ScanOverride {
        fn drop(&mut self) {
            set_runtime_scan_override(None);
        }
    }

    // Regression: `detect_runtime_session` discarded the degraded flag of
    // `scan_sessions_for_runtime` and matched the pane/tool against the
    // scanner's last good snapshot. During an inventory outage after a CLI
    // restart in an existing pane, that bound the new member runtime to the
    // previous CLI's session_id/jsonl_path, which initialization then
    // persisted. A degraded scan is no observation: never match cached
    // identities, keep polling, and report no session when the outage lasts.
    #[test]
    fn detect_runtime_session_ignores_degraded_snapshot_and_keeps_polling() {
        let scans = ScanOverride::install(usize::MAX);

        let detected = SystemCoordinationRuntime
            .detect_runtime_session(PANE, CliTool::Codex)
            .expect("detection succeeds");

        assert_eq!(
            detected,
            DetectedRuntimeSession::default(),
            "a degraded scan must never bind the cached identity"
        );
        assert_eq!(
            scans.calls(),
            SESSION_DETECT_ATTEMPTS,
            "every attempt must poll the scanner again"
        );
    }

    // Regression companion: once the inventory is readable again within the
    // detection window, the fresh observation is bound, not the stale one.
    #[test]
    fn detect_runtime_session_binds_first_healthy_scan_after_degraded_ones() {
        let scans = ScanOverride::install(2);

        let detected = SystemCoordinationRuntime
            .detect_runtime_session(PANE, CliTool::Codex)
            .expect("detection succeeds");

        assert_eq!(
            detected,
            DetectedRuntimeSession {
                session_id: Some("fresh-session".to_string()),
                jsonl_path: Some(PathBuf::from("/tmp/fresh-session.jsonl")),
            }
        );
        assert_eq!(scans.calls(), 3);
    }

    // Regression: 790d7a2 made the scan override and its call counter
    // process-global while only the tests that install one take a lock. Any
    // sibling test that polls the scanner meanwhile — the onboarding E2E drives
    // `detect_runtime_session` on the real runtime in the same binary — was
    // served this script and counted against it, so
    // `detect_runtime_session_ignores_degraded_snapshot_and_keeps_polling`
    // failed with `left: 11, right: 6`. The script belongs to the thread that
    // installed it: nobody else is served it, and nobody else can count.
    #[test]
    fn scripted_scan_is_scoped_to_the_thread_that_installed_it() {
        let scans = ScanOverride::install(usize::MAX);

        let sibling = std::thread::spawn(|| {
            (0..5)
                .map(|_| collect_runtime_sessions())
                .collect::<Vec<_>>()
        })
        .join()
        .expect("sibling scan thread");

        assert!(
            sibling
                .iter()
                .all(|(sessions, degraded)| sessions.is_empty() && !*degraded),
            "a sibling thread must never be served this test's scripted scan"
        );

        let (sessions, degraded) = collect_runtime_sessions();
        assert!(degraded);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id.as_deref(), Some("stale-session"));
        assert_eq!(
            scans.calls(),
            1,
            "the guard counts only the scans of the test that installed it"
        );
    }
}
