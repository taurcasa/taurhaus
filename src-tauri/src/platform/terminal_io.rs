//! Bounded terminal children inherit the lifetime flock as stdin.
use std::cell::RefCell;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const HOLD_BOUND: Duration = Duration::from_secs(10);
thread_local! {
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

/// Resolve once per call, then explicitly address the same server. Never let
/// the tmux client silently select a socket through inherited TMUX.
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
