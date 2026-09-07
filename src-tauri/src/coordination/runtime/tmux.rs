use crate::coordination::errors::CoordinationError;
use crate::coordination::mesh_cli::{self, CommandInvocation};
use crate::tmux_layout::{
    derive_window_name, parse_pane_records, parse_window_records, resolve_layout_allocation,
    resolve_split_target_pane, wait_for_tmux_session_ready, TmuxLayoutAllocation, TmuxLayoutPolicy,
    DEFAULT_SPLIT_MAX_PANES, LIST_PANES_FORMAT, LIST_WINDOWS_FORMAT,
};

use super::process::run_system_command;
use super::TAURHAUS_TMUX_SESSION_NAME;

fn tmux_command_invocation(args: &[String]) -> CommandInvocation {
    mesh_cli::command_invocation("tmux", args)
}

pub(super) fn run_tmux(args: &[String]) -> Result<String, CoordinationError> {
    let invocation = tmux_command_invocation(args);
    let output = run_tmux_output(args)?;
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

pub(super) fn run_tmux_output(args: &[String]) -> Result<std::process::Output, CoordinationError> {
    #[cfg(all(test, target_os = "linux"))]
    if let Some(output) = tests::scratch_tmux_output(args) {
        return output;
    }
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

    wait_for_tmux_session_ready(TAURHAUS_TMUX_SESSION_NAME, verify_tmux_session_ready)
        .map_err(CoordinationError::Backend)?;

    Ok(())
}

pub(super) fn create_tmux_pane_with_layout(
    project_id: &str,
    tmux_layout: &str,
) -> Result<String, CoordinationError> {
    ensure_taurhaus_tmux_session()?;

    let window_name = derive_window_name(project_id, "agent");
    let policy = TmuxLayoutPolicy::from_setting(tmux_layout, DEFAULT_SPLIT_MAX_PANES);
    let windows = match policy {
        TmuxLayoutPolicy::NewWindow => Vec::new(),
        _ => list_tmux_windows(TAURHAUS_TMUX_SESSION_NAME)?,
    };

    match resolve_layout_allocation(&policy, TAURHAUS_TMUX_SESSION_NAME, &window_name, &windows) {
        TmuxLayoutAllocation::NewWindow { window_name } => {
            create_tmux_new_window_pane(project_id, &window_name)
        }
        TmuxLayoutAllocation::SplitExisting { window_index, .. } => {
            let target_pane =
                resolve_split_target_pane_for_window(TAURHAUS_TMUX_SESSION_NAME, &window_index)?;
            let pane_id = create_tmux_split_pane(project_id, &target_pane)?;
            if policy == TmuxLayoutPolicy::PerProject {
                // Resume/add must rebalance just like initialize/app launches:
                // the next member splits this same anchor again.
                if let Err(err) = run_tmux(&[
                    "select-layout".to_string(),
                    "-t".to_string(),
                    pane_id.clone(),
                    "tiled".to_string(),
                ]) {
                    // An Err leaves the caller unable to track this new pane.
                    let _ = run_tmux(&["kill-pane".to_string(), "-t".to_string(), pane_id]);
                    return Err(err);
                }
            }
            Ok(pane_id)
        }
    }
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

fn list_tmux_windows(
    tmux_session: &str,
) -> Result<Vec<crate::tmux_layout::TmuxWindowRecord>, CoordinationError> {
    let out = run_tmux(&[
        "list-windows".to_string(),
        "-t".to_string(),
        tmux_session.to_string(),
        "-F".to_string(),
        LIST_WINDOWS_FORMAT.to_string(),
    ])?;

    Ok(parse_window_records(&out))
}

fn list_tmux_window_panes(
    tmux_session: &str,
    window_index: &str,
) -> Result<Vec<crate::tmux_layout::TmuxPaneRecord>, CoordinationError> {
    let target = format!("{tmux_session}:{window_index}");
    let out = run_tmux(&[
        "list-panes".to_string(),
        "-t".to_string(),
        target,
        "-F".to_string(),
        LIST_PANES_FORMAT.to_string(),
    ])?;

    Ok(parse_pane_records(&out))
}

fn resolve_split_target_pane_for_window(
    tmux_session: &str,
    window_index: &str,
) -> Result<String, CoordinationError> {
    let panes = list_tmux_window_panes(tmux_session, window_index)?;
    let target = resolve_split_target_pane(tmux_session, window_index, &panes)
        .map_err(CoordinationError::Backend)?;

    let validation = run_tmux_output(&[
        "display-message".to_string(),
        "-p".to_string(),
        "-t".to_string(),
        target.clone(),
        "#{pane_id}".to_string(),
    ])?;
    if validation.status.success() {
        return Ok(target);
    }

    let pane_ids = panes
        .iter()
        .map(|pane| pane.pane_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(CoordinationError::Backend(format!(
        "tmux window '{tmux_session}:{window_index}' resolved pane target '{target}' is not addressable; pane_ids=[{pane_ids}]"
    )))
}

fn verify_tmux_session_ready(tmux_session: &str) -> Result<(), String> {
    let windows = list_tmux_windows(tmux_session).map_err(|err| err.to_string())?;
    let first_window = windows
        .first()
        .ok_or_else(|| format!("tmux session '{tmux_session}' has no windows yet"))?;
    let _ = resolve_split_target_pane_for_window(tmux_session, &first_window.index)
        .map_err(|err| err.to_string())?;
    Ok(())
}

pub(super) fn tmux_target_for_pane(pane_id: &str) -> String {
    if pane_id.starts_with('%') {
        pane_id.to_string()
    } else {
        format!(":.{pane_id}")
    }
}

pub(super) fn is_shell_command(raw: &str) -> bool {
    let command = raw
        .trim()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_start_matches('-')
        .to_ascii_lowercase();
    matches!(command.as_str(), "bash" | "zsh" | "sh" | "fish")
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::coordination::runtime::{CoordinationRuntime, SystemCoordinationRuntime};
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn rust_ci_jobs_install_scratch_tmux_dependency() {
        // Regression: 13727b5b added scratch tmux tests to coordination, which
        // integration targets also compile, but only rust-unit installed tmux.
        let workflow = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../.github/workflows/quality-gate.yml"
        ));
        for job in ["rust-unit", "rust-integration"] {
            let job_body = workflow
                .split_once(&format!("\n  {job}:\n"))
                .expect("Rust CI job must exist")
                .1;
            let dependencies = job_body
                .split_once("sudo apt-get install -y")
                .expect("Rust CI job must install system dependencies")
                .1
                .split("\n      - name:")
                .next()
                .unwrap();
            assert!(
                dependencies.split_whitespace().any(|word| word == "tmux"),
                "{job} must install tmux for the scratch-server tests"
            );
        }
    }

    // Keep this fixture local: integration targets recompile coordination with
    // scanner shims, which do not expose the scanner's private test helpers.
    thread_local! {
        static TEST_TMUX_ROOT: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
    }

    fn scratch_tmux_command() -> Option<Command> {
        TEST_TMUX_ROOT.with(|root| {
            root.borrow().as_ref().map(|root| {
                let mut cmd = Command::new("tmux");
                cmd.env_clear()
                    .env("HOME", root)
                    .env("TMUX_TMPDIR", root)
                    .env("PATH", std::env::var_os("PATH").unwrap_or_default())
                    .env("SHELL", "/bin/sh")
                    // tmux otherwise replaces tab-separated record fields with underscores.
                    .env("LC_ALL", "C.UTF-8")
                    .args(["-L", "team-pane-regression", "-f", "/dev/null"]);
                cmd
            })
        })
    }

    struct ScratchTmux {
        root: tempfile::TempDir,
        // The override belongs to the installing thread, including on drop.
        _not_send: std::marker::PhantomData<*const ()>,
    }

    impl ScratchTmux {
        fn new(width: &str, height: &str) -> Self {
            let scratch = Self {
                // Regression: c22b502a inherited long TMPDIR values, exceeding
                // the Unix socket path limit before the test could start.
                root: tempfile::TempDir::new_in("/tmp").unwrap(),
                _not_send: std::marker::PhantomData,
            };
            TEST_TMUX_ROOT
                .with(|root| *root.borrow_mut() = Some(scratch.root.path().to_path_buf()));
            scratch.run(&[
                "new-session",
                "-d",
                "-s",
                TAURHAUS_TMUX_SESSION_NAME,
                "-x",
                width,
                "-y",
                height,
                "-n",
                "team",
                "/bin/sh",
            ]);
            scratch
        }

        fn path(&self) -> &Path {
            self.root.path()
        }

        fn run(&self, args: &[&str]) -> String {
            let output = scratch_tmux_command()
                .unwrap()
                .args(args)
                .output()
                .unwrap_or_else(|err| {
                    panic!("Scratch tmux tests require tmux installed on PATH: {err}")
                });
            assert!(
                output.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
    }

    impl Drop for ScratchTmux {
        fn drop(&mut self) {
            // The private socket root is still installed, including during unwinding.
            let _ = scratch_tmux_command().unwrap().arg("kill-server").output();
            TEST_TMUX_ROOT.with(|root| *root.borrow_mut() = None);
        }
    }

    thread_local! {
        static FAIL_TILING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    struct FailTiling;

    impl FailTiling {
        fn install() -> Self {
            FAIL_TILING.with(|slot| slot.set(true));
            Self
        }
    }

    impl Drop for FailTiling {
        fn drop(&mut self) {
            FAIL_TILING.with(|slot| slot.set(false));
        }
    }

    pub(super) fn scratch_tmux_output(
        args: &[String],
    ) -> Option<Result<std::process::Output, CoordinationError>> {
        let mut cmd = scratch_tmux_command()?;
        let mut args = args.to_vec();
        if args.first().is_some_and(|arg| arg == "select-layout")
            && FAIL_TILING.with(std::cell::Cell::get)
        {
            *args.last_mut().unwrap() = "invalid-test-layout".to_string();
        }
        Some(cmd.args(args).output().map_err(CoordinationError::Io))
    }

    #[test]
    fn resume_add_nine_same_project_members_share_one_window() {
        // Regression: 52714df3 pinned splits to the first pane; c22b502a fixed
        // initialize/app launches but left resume/add's create_aitx_pane untiled.
        for (width, height) in [("240", "60"), ("252", "62"), ("80", "24")] {
            let scratch = ScratchTmux::new(width, height);
            scratch.run(&["set-option", "-g", "default-shell", "/bin/sh"]);
            let project = scratch.path().to_str().unwrap();
            let runtime = SystemCoordinationRuntime;
            let mut members = std::collections::HashSet::new();
            let mut window = None;
            for member in 1..=9 {
                let pane = runtime
                    .create_aitx_pane(project, "per_project")
                    .unwrap_or_else(|err| panic!("member {member} at {width}x{height}: {err}"));
                assert!(members.insert(pane.clone()));
                let actual = scratch.run(&["display-message", "-p", "-t", &pane, "#{window_id}"]);
                assert_eq!(window.get_or_insert(actual.clone()), &actual);
            }
            let window = window.unwrap();
            assert_eq!(
                scratch.run(&["display-message", "-p", "-t", &window, "#{window_panes}"]),
                "9"
            );
            // The scratch bootstrap window remains untouched; the project gets one window.
            assert_eq!(
                scratch
                    .run(&[
                        "list-windows",
                        "-t",
                        TAURHAUS_TMUX_SESSION_NAME,
                        "-F",
                        "#{window_id}"
                    ])
                    .lines()
                    .count(),
                2
            );
            for dimension in scratch
                .run(&[
                    "list-panes",
                    "-t",
                    &window,
                    "-F",
                    "#{pane_width} #{pane_height}",
                ])
                .lines()
            {
                let values: Vec<u32> = dimension
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect();
                assert!(
                    values[0] >= 10 && values[1] >= 3,
                    "unusable pane: {dimension}"
                );
            }
        }
    }

    #[test]
    fn resume_add_tiling_failure_removes_only_the_new_pane() {
        // Regression: c22b502a omitted resume/add's tiling and error cleanup.
        let scratch = ScratchTmux::new("240", "60");
        scratch.run(&["set-option", "-g", "default-shell", "/bin/sh"]);
        let project = scratch.path().to_str().unwrap();
        let runtime = SystemCoordinationRuntime;
        let anchor = runtime.create_aitx_pane(project, "per_project").unwrap();
        let before = scratch.run(&["list-panes", "-a", "-F", "#{pane_id}"]);
        let failure = FailTiling::install();
        let result = runtime.create_aitx_pane(project, "per_project");
        drop(failure);
        assert!(result.unwrap_err().to_string().contains("select-layout"));
        assert_eq!(
            scratch.run(&["list-panes", "-a", "-F", "#{pane_id}"]),
            before
        );
        assert_eq!(
            scratch.run(&["display-message", "-p", "-t", &anchor, "#{pane_id}"]),
            anchor
        );
        assert!(runtime.create_aitx_pane(project, "per_project").is_ok());
    }

    #[test]
    fn resume_add_other_policies_do_not_tile() {
        let scratch = ScratchTmux::new("240", "60");
        scratch.run(&["set-option", "-g", "default-shell", "/bin/sh"]);
        let project = scratch.path().to_str().unwrap();
        let runtime = SystemCoordinationRuntime;
        let _failure = FailTiling::install();
        for policy in ["split", "new_window"] {
            for _ in 0..3 {
                assert!(runtime.create_aitx_pane(project, policy).is_ok());
            }
        }
    }
}
