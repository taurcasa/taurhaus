use std::io;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub fn run_command_with_timeout(
    command: &mut Command,
    timeout: Duration,
    description: &str,
) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let started_at = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output(),
            Ok(None) => {
                if started_at.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    tracing::warn!(
                        command = description,
                        timeout_ms = timeout.as_millis() as u64,
                        "subprocess timed out; terminating process"
                    );
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("{description} timed out after {} ms", timeout.as_millis()),
                    ));
                }
                thread::sleep(COMMAND_POLL_INTERVAL);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_command_with_timeout_returns_output_for_fast_command() {
        let output = run_command_with_timeout(
            Command::new("sh").args(["-lc", "printf 'ok'"]),
            Duration::from_secs(1),
            "printf",
        )
        .expect("command should succeed");

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "ok");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn run_command_with_timeout_kills_hung_command() {
        let error = run_command_with_timeout(
            Command::new("sh").args(["-lc", "sleep 1"]),
            Duration::from_millis(50),
            "sleep",
        )
        .expect_err("sleep should time out");

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
