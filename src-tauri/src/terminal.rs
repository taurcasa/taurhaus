//! Terminal management — unified Windows Terminal interaction.
//!
//! Single entry point (`handle_terminal`) handles all terminal interaction:
//! focusing an existing window, or launching a new one attached to tmux.
//!
//! Key invariant: if Windows Terminal is already running, we NEVER create
//! a new tab. The tmux session is already visible in the existing tab.
//! We only launch `wt.exe new-tab` when no WT process exists at all.
//!
//! On Linux: no-op (the terminal is already in the user's workspace).

/// What the caller wants to do with Windows Terminal.
#[derive(Debug)]
pub enum TerminalIntent {
    /// Just focus the existing window. No-op if terminal isn't running.
    /// Used by session indicator clicks — quick navigation that shouldn't
    /// spawn a new terminal window.
    FocusOnly,
    /// Ensure a terminal is visible, launching one if needed.
    /// If already running, focuses it. If not, launches with tmux attach.
    /// Used by "Open in Terminal" and tool launch — explicit actions where
    /// the user expects a terminal to appear.
    EnsureOpen {
        distro: Option<String>,
        tmux_session: String,
        /// "windows_terminal" (default) or "custom"
        emulator: String,
        /// Command template when emulator is "custom".
        /// Placeholders: {distro}, {tmux_session}
        custom_command: String,
    },
}

/// Result of checking for Windows Terminal.
///
/// Three-state detection separates "is it running?" from "can we focus it?"
/// This matters because Windows Terminal (WinUI 3) sometimes reports
/// MainWindowHandle as zero even when running — we must not treat that
/// as "not running" and spawn a duplicate tab.
#[cfg(target_os = "windows")]
#[derive(Debug, PartialEq)]
enum TerminalStatus {
    /// Found the window and brought it to foreground.
    Focused,
    /// Process exists but we couldn't get a window handle to focus.
    /// The terminal tab is still there — don't create another.
    Running,
    /// No WindowsTerminal process found. Safe to launch a new one.
    NotRunning,
}

/// Single entry point for all Windows Terminal interactions.
///
/// Both intents start by checking for an existing terminal:
/// - `Focused` or `Running` → terminal exists, return (no new tab).
/// - `NotRunning` → `FocusOnly` returns silently, `EnsureOpen` launches WT.
///
/// The `-w taurhaus` flag on `wt.exe` uses a named window — if a WT window
/// named "taurhaus" already exists, the tab opens there. If not, a new
/// window is created with that name. This is deterministic: we always
/// target *our* window, not "whatever was last used."
#[cfg(target_os = "windows")]
pub fn handle_terminal(intent: TerminalIntent) -> Result<(), String> {
    let status = check_windows_terminal()?;

    match status {
        TerminalStatus::Focused => {
            tracing::debug!("Focused existing Windows Terminal");
            return Ok(());
        }
        TerminalStatus::Running => {
            // Terminal is running but we couldn't focus it (WinUI 3 window
            // handle issue). The tab with our tmux session already exists —
            // don't create a duplicate.
            tracing::debug!("Windows Terminal running (couldn't focus), skipping new tab");
            return Ok(());
        }
        TerminalStatus::NotRunning => {
            tracing::debug!("Windows Terminal not running");
        }
    }

    // Terminal not running. For FocusOnly, nothing more to do.
    let TerminalIntent::EnsureOpen { distro, tmux_session, emulator, custom_command } = intent else {
        return Ok(());
    };

    let Some(distro) = distro else {
        return Err("Cannot open terminal: no WSL distro configured".to_string());
    };

    crate::daemon::launcher::validate_wsl_distro(&distro)
        .map_err(|e| format!("Invalid WSL distro: {e}"))?;

    // Custom emulator: substitute placeholders and run user command
    if emulator == "custom" && !custom_command.trim().is_empty() {
        let cmd = custom_command
            .replace("{distro}", &distro)
            .replace("{tmux_session}", &tmux_session);

        tracing::info!(%cmd, "Launching custom terminal emulator");

        // Split on whitespace: first token is the program, rest are args.
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return Err("Custom terminal command is empty".to_string());
        }

        std::process::Command::new(parts[0])
            .args(&parts[1..])
            .spawn()
            .map_err(|e| format!("Failed to launch custom terminal: {e}"))?;

        return Ok(());
    }

    tracing::info!(%distro, %tmux_session, "Launching Windows Terminal with tmux attach");

    // -w taurhaus: named window — always targets OUR terminal window.
    // If a WT window named "taurhaus" exists, the tab opens there.
    // If not, creates a new window with that name. Consistent identity
    // across the stack: tmux session "taurhaus", WT window "taurhaus".
    let mut args: Vec<String> = vec!["-w".into(), "taurhaus".into(), "new-tab".into()];

    // Use the user's default WT profile for their customized colors/font.
    // Without `-p`, WT auto-matches `wsl.exe -d Ubuntu` to the hidden
    // generic WSL profile which looks different from the user's actual
    // Ubuntu profile.
    if let Some(guid) = detect_wt_default_profile() {
        tracing::info!(%guid, "Using WT default profile");
        args.push("-p".into());
        args.push(guid);
    }

    args.extend([
        "--".into(),
        "wsl.exe".into(), "-d".into(), distro, "--".into(),
        "tmux".into(), "attach-session".into(), "-t".into(), tmux_session,
    ]);

    std::process::Command::new("wt.exe")
        .args(&args)
        .spawn()
        .map_err(|e| format!("Failed to launch Windows Terminal: {e}"))?;

    Ok(())
}

/// Check if Windows Terminal is running and try to focus it.
///
/// Uses a two-tier approach for reliability:
/// 1. `Get-Process` finds the WindowsTerminal process
/// 2. If `MainWindowHandle` is zero (common with WinUI 3 apps),
///    falls back to `EnumWindows` to find a visible window owned
///    by the process
///
/// Returns `TerminalStatus::Focused` if we found and focused the window,
/// `Running` if the process exists but we couldn't get a window handle,
/// or `NotRunning` if no WindowsTerminal process was found.
#[cfg(target_os = "windows")]
fn check_windows_terminal() -> Result<TerminalStatus, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Two-tier window detection:
    // 1. Try MainWindowHandle (fast, works for classic Win32 apps)
    // 2. Fall back to EnumWindows (finds WinUI 3 / XAML windows)
    let script = r#"
        Add-Type @"
            using System;
            using System.Runtime.InteropServices;
            public class WinApi {
                [DllImport("user32.dll")]
                public static extern bool SetForegroundWindow(IntPtr hWnd);
                [DllImport("user32.dll")]
                public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
                [DllImport("user32.dll")]
                public static extern bool IsWindowVisible(IntPtr hWnd);
                [DllImport("user32.dll")]
                public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

                public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

                [DllImport("user32.dll")]
                public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

                public static IntPtr FindVisibleWindowByPid(uint targetPid) {
                    IntPtr found = IntPtr.Zero;
                    EnumWindows((hWnd, lParam) => {
                        if (!IsWindowVisible(hWnd)) return true;
                        uint pid;
                        GetWindowThreadProcessId(hWnd, out pid);
                        if (pid == targetPid) {
                            found = hWnd;
                            return false;
                        }
                        return true;
                    }, IntPtr.Zero);
                    return found;
                }
            }
"@
        $proc = Get-Process -Name WindowsTerminal -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $proc) {
            Write-Output 'NOT_RUNNING'
            return
        }

        # Try MainWindowHandle first (fast path)
        $hwnd = $proc.MainWindowHandle
        if ($hwnd -eq [IntPtr]::Zero) {
            # WinUI 3 apps often have zero MainWindowHandle — enumerate windows instead
            $hwnd = [WinApi]::FindVisibleWindowByPid([uint32]$proc.Id)
        }

        if ($hwnd -ne [IntPtr]::Zero) {
            [WinApi]::ShowWindow($hwnd, 9) | Out-Null   # SW_RESTORE
            [WinApi]::SetForegroundWindow($hwnd) | Out-Null
            Write-Output 'FOCUSED'
        } else {
            # Process exists but no visible window found
            Write-Output 'RUNNING'
        }
    "#;

    let output = std::process::Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    tracing::debug!(%stdout, "Windows Terminal check result");

    match stdout.as_str() {
        "FOCUSED" => Ok(TerminalStatus::Focused),
        "RUNNING" => Ok(TerminalStatus::Running),
        _ => Ok(TerminalStatus::NotRunning),
    }
}

/// Read the default profile GUID from Windows Terminal's settings.json.
///
/// Returns the GUID string (e.g., `{51855cb2-...}`) or None if settings
/// can't be read. This is the profile whose visual settings (colors, font)
/// we want when opening a new tab.
#[cfg(target_os = "windows")]
fn detect_wt_default_profile() -> Option<String> {
    let localappdata = std::env::var("LOCALAPPDATA").ok()?;
    let settings_path = std::path::PathBuf::from(&localappdata)
        .join("Packages")
        .join("Microsoft.WindowsTerminal_8wekyb3d8bbwe")
        .join("LocalState")
        .join("settings.json");

    let content = std::fs::read_to_string(&settings_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("defaultProfile")?.as_str().map(String::from)
}

// macOS: Terminal.app (default), iTerm2, or custom emulator.
//
// Key invariant (mirrors Windows logic): if the terminal emulator is already
// attached to our tmux session, we NEVER create a new tab/window. We just
// activate (focus) the existing one. Only launch a new attachment when
// none exists.

/// Check if Terminal.app is already running with a tmux-attached tab.
///
/// Uses AppleScript to inspect Terminal.app processes. Returns true if
/// Terminal.app has at least one window whose shell is running `tmux attach`.
#[cfg(target_os = "macos")]
fn is_terminal_app_attached(tmux_session: &str) -> bool {
    // Check if any Terminal.app tab is running tmux with our session name.
    // We look for the process rather than inspecting AppleScript window contents
    // because that's more reliable across Terminal.app versions.
    let output = std::process::Command::new("sh")
        .args(["-c", &format!(
            "pgrep -f 'tmux attach-session -t {tmux_session}' >/dev/null 2>&1 && echo attached || echo none"
        )])
        .output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "attached",
        Err(_) => false,
    }
}

/// Check if iTerm2 is already running with a tmux-attached tab.
#[cfg(target_os = "macos")]
fn is_iterm2_attached(tmux_session: &str) -> bool {
    // Same pgrep approach — works regardless of which terminal hosts the attachment.
    is_terminal_app_attached(tmux_session)
}

/// Activate (focus) Terminal.app without creating a new tab.
#[cfg(target_os = "macos")]
fn focus_terminal_app() -> Result<(), String> {
    let script = r#"tell application "Terminal" to activate"#;
    std::process::Command::new("osascript")
        .args(["-e", script])
        .spawn()
        .map_err(|e| format!("Failed to activate Terminal.app: {e}"))?;
    Ok(())
}

/// Activate (focus) iTerm2 without creating a new tab.
#[cfg(target_os = "macos")]
fn focus_iterm2() -> Result<(), String> {
    let script = r#"tell application "iTerm" to activate"#;
    std::process::Command::new("osascript")
        .args(["-e", script])
        .spawn()
        .map_err(|e| format!("Failed to activate iTerm2: {e}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn handle_terminal(intent: TerminalIntent) -> Result<(), String> {
    match intent {
        TerminalIntent::FocusOnly => {
            // Try to activate the terminal that's attached to our tmux session.
            // Check which terminal app is running and focus it.
            let script = r#"
                if application "iTerm" is running then
                    tell application "iTerm" to activate
                else if application "Terminal" is running then
                    tell application "Terminal" to activate
                end if
            "#;
            let _ = std::process::Command::new("osascript")
                .args(["-e", script])
                .spawn();
            Ok(())
        }
        TerminalIntent::EnsureOpen { tmux_session, emulator, custom_command, .. } => {
            // Custom emulator
            if emulator == "custom" && !custom_command.trim().is_empty() {
                let cmd = custom_command.replace("{tmux_session}", &tmux_session);
                tracing::info!(%cmd, "Launching custom terminal emulator (macOS)");
                let parts: Vec<&str> = cmd.split_whitespace().collect();
                if parts.is_empty() {
                    return Err("Custom terminal command is empty".to_string());
                }
                std::process::Command::new(parts[0])
                    .args(&parts[1..])
                    .spawn()
                    .map_err(|e| format!("Failed to launch custom terminal: {e}"))?;
                return Ok(());
            }

            // iTerm2
            if emulator == "iterm2" {
                if is_iterm2_attached(&tmux_session) {
                    tracing::debug!("iTerm2 already attached to tmux, just focusing");
                    return focus_iterm2();
                }
                return launch_iterm2(&tmux_session);
            }

            // Default: Terminal.app
            if is_terminal_app_attached(&tmux_session) {
                tracing::debug!("Terminal.app already attached to tmux, just focusing");
                return focus_terminal_app();
            }
            launch_terminal_app(&tmux_session)
        }
    }
}

/// Launch Terminal.app and attach to a tmux session via AppleScript.
///
/// Uses `do script` in the FRONT window to reuse an existing idle tab when
/// Terminal.app is already open. Only creates a new window if Terminal isn't
/// running yet.
#[cfg(target_os = "macos")]
fn launch_terminal_app(tmux_session: &str) -> Result<(), String> {
    // Check if Terminal.app is already running. If it is, reuse the front window.
    // If not, `do script` without `in window` creates a new window automatically.
    let script = format!(
        r#"tell application "Terminal"
    activate
    if (count of windows) > 0 then
        do script "tmux attach-session -t {tmux_session}" in front window
    else
        do script "tmux attach-session -t {tmux_session}"
    end if
end tell"#
    );

    tracing::info!(%tmux_session, "Launching Terminal.app with tmux attach");

    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map_err(|e| format!("Failed to launch Terminal.app: {e}"))?;

    Ok(())
}

/// Launch iTerm2 and attach to a tmux session via its AppleScript API.
#[cfg(target_os = "macos")]
fn launch_iterm2(tmux_session: &str) -> Result<(), String> {
    let script = format!(
        r#"tell application "iTerm"
    activate
    tell current window
        create tab with default profile command "tmux attach-session -t {tmux_session}"
    end tell
end tell"#
    );

    tracing::info!(%tmux_session, "Launching iTerm2 with tmux attach");

    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map_err(|e| format!("Failed to launch iTerm2: {e}"))?;

    Ok(())
}

// Linux: no-op — terminal is already in the user's workspace.

#[cfg(target_os = "linux")]
pub fn handle_terminal(_intent: TerminalIntent) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_only_returns_ok() {
        let result = handle_terminal(TerminalIntent::FocusOnly);
        assert!(result.is_ok());
    }

    #[test]
    fn ensure_open_returns_ok() {
        let result = handle_terminal(TerminalIntent::EnsureOpen {
            distro: Some("Ubuntu".to_string()),
            tmux_session: "taurhaus".to_string(),
            emulator: "windows_terminal".to_string(),
            custom_command: String::new(),
        });
        assert!(result.is_ok());
    }
}
