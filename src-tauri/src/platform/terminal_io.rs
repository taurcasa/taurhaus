//! Bounded terminal children inherit the lifetime flock as stdin.
use std::cell::RefCell;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const HOLD_BOUND: Duration = Duration::from_secs(10);
thread_local! {
    static CHILD_EXECUTABLE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static ACTIVE: RefCell<Option<(File, Instant)>> = const { RefCell::new(None) };
}
pub fn active() -> bool {
    ACTIVE.with(|a| a.borrow().is_some())
}
pub fn enter(file: File) {
    ACTIVE.with(|a| *a.borrow_mut() = Some((file, Instant::now() + HOLD_BOUND)));
}
pub fn leave() {
    ACTIVE.with(|a| *a.borrow_mut() = None);
}

/// Supply the supervisor explicitly for transport fixtures. The override is
/// thread-local and unwind-safe; the production command construction is unchanged.
#[doc(hidden)]
pub fn with_child_executable<T>(path: &Path, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<PathBuf>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CHILD_EXECUTABLE.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CHILD_EXECUTABLE.with(|slot| slot.replace(Some(path.into()))));
    run()
}

pub fn output(command: &mut Command) -> std::io::Result<Output> {
    let inherited = ACTIVE.with(|a| {
        a.borrow()
            .as_ref()
            .map(|(f, end)| f.try_clone().map(|f| (f, *end)))
            .transpose()
    })?;
    let Some((file, end)) = inherited else {
        return command.output();
    };
    if Instant::now() >= end {
        return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "terminal hold deadline",
        ));
    }
    // The production child supervises tmux independently of the original
    // holder. Killing the holder cannot strand a client with the lifetime fd.
    let mut supervisor = child_command(command, end)?;
    let command = &mut supervisor;
    let mut child = command
        .stdin(Stdio::from(file))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output(),
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().unwrap_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "terminal child deadline")
                }));
            }
        }
    }
}

const CHILD_MODE: &str = "--terminal-child";

fn child_command(command: &Command, end: Instant) -> std::io::Result<Command> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let remaining = end
        .saturating_duration_since(Instant::now())
        .saturating_sub(Duration::from_millis(50));
    let deadline = (SystemTime::now() + remaining)
        .duration_since(UNIX_EPOCH)
        .map_err(std::io::Error::other)?
        .as_millis();
    let executable = CHILD_EXECUTABLE
        .with(|slot| slot.borrow().clone())
        .map(Ok)
        .unwrap_or_else(std::env::current_exe)?;
    let mut child = Command::new(executable);
    child
        .arg(CHILD_MODE)
        .arg(deadline.to_string())
        .arg(command.get_program())
        .args(command.get_args());
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            child.env(key, value);
        } else {
            child.env_remove(key);
        }
    }
    if let Some(dir) = command.get_current_dir() {
        child.current_dir(dir);
    }
    super::apply_background_command_settings(&mut child);
    Ok(child)
}

/// Internal entry point, before app/daemon initialization. It inherits fd 0
/// from the holder and passes it to exactly one bounded terminal transport.
pub fn maybe_run_child() {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(CHILD_MODE)) {
        return;
    }
    let deadline = args
        .next()
        .and_then(|s| s.to_str().and_then(|s| s.parse::<u64>().ok()));
    let Some((deadline, program)) = deadline.zip(args.next()) else {
        std::process::exit(2);
    };
    let remaining = (UNIX_EPOCH + Duration::from_millis(deadline))
        .duration_since(SystemTime::now())
        .unwrap_or_default()
        .min(HOLD_BOUND);
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let code = run_child(remaining, &mut command)
        .map(|s| s.code().unwrap_or(1))
        .unwrap_or(124);
    std::process::exit(code);
}

fn run_child(limit: Duration, command: &mut Command) -> std::io::Result<std::process::ExitStatus> {
    let timeout = || std::io::Error::new(std::io::ErrorKind::TimedOut, "terminal child deadline");
    if limit.is_zero() {
        return Err(timeout());
    }
    let deadline = Instant::now() + limit.min(HOLD_BOUND);
    let mut child = command.spawn()?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().unwrap_or_else(timeout));
            }
        }
    }
}

/// Resolve once per call, then explicitly address the same server. Never let
/// the tmux client silently select a socket through inherited TMUX.
#[cfg(not(target_os = "windows"))]
pub fn socket_path() -> std::io::Result<PathBuf> {
    if let Some(path) = std::env::var("TMUX")
        .ok()
        .and_then(|s| s.split(',').next().map(PathBuf::from))
        .filter(|p| p.is_absolute())
    {
        return Ok(path);
    }
    #[cfg(target_os = "linux")]
    let uid = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self")?.uid().to_string()
    };
    #[cfg(not(target_os = "linux"))]
    let uid = {
        let out = Command::new("id").arg("-u").output()?;
        let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() || uid.parse::<u32>().is_err() {
            return Err(std::io::Error::other("cannot resolve tmux uid"));
        }
        uid
    };
    let root = std::env::var_os("TMUX_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    if !root.is_absolute() {
        return Err(std::io::Error::other("tmux socket root must be absolute"));
    }
    Ok(root.join(format!("tmux-{uid}/default")))
}

#[cfg(target_os = "windows")]
pub fn socket_path() -> std::io::Result<PathBuf> {
    // This caller addresses WSL tmux, so resolve its uid/root in that namespace.
    let output = crate::daemon::launcher::wsl_command()
        .args([
            "-e",
            "sh",
            "-c",
            r#"printf '%s/tmux-%s/default' "${TMUX_TMPDIR:-/tmp}" "$(id -u)""#,
        ])
        .output()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || !path.starts_with('/') {
        return Err(std::io::Error::other("WSL tmux socket unavailable"));
    }
    Ok(PathBuf::from(path))
}

/// Extension keeps existing command construction readable while routing all
/// managed terminal children through the inherited lock and hold deadline.
pub trait TerminalOutput {
    fn terminal_output(&mut self) -> std::io::Result<Output>;
}
impl TerminalOutput for Command {
    fn terminal_output(&mut self) -> std::io::Result<Output> {
        output(self)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn terminal_child_watchdog_reaps_its_own_timed_out_transport() {
        let mut command = Command::new("/bin/sleep");
        command.arg("1");
        let started = Instant::now();
        assert_eq!(
            run_child(Duration::from_millis(20), &mut command)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
