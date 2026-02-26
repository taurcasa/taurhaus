//! Terminal management — cross-platform terminal emulator integration.
//!
//! Unified decision tree for all platforms:
//!
//!   handle_terminal(intent)
//!     └── Emulator (from settings)
//!         ├── FocusOnly  → is preferred emulator running? → activate it
//!         └── EnsureOpen → is preferred emulator running?
//!             ├── Yes → activate it (don't create duplicate tab/window)
//!             └── No  → launch it with `tmux attach-session -t <session>`
//!
//! Platform differences are only in which emulators are available and how
//! to detect/activate/launch them:
//!   - Windows: "windows_terminal" (default), "custom"
//!   - macOS:   "iterm2" (default), "ghostty", "terminal_app", "custom"
//!   - Linux:   no-op (user manages their own terminal)

/// What the caller wants to do with the terminal.
#[derive(Debug)]
pub enum TerminalIntent {
    /// Just focus the existing terminal window. No-op if not running.
    /// Used by session indicator clicks — quick navigation that shouldn't
    /// spawn a new terminal window.
    FocusOnly {
        /// Emulator preference (so we focus the RIGHT terminal, not just any).
        emulator: String,
    },
    /// Ensure a terminal is visible, launching one if needed.
    /// If already running, focuses it. If not, launches with tmux attach.
    /// Used by "Open in Terminal" and tool launch — explicit actions where
    /// the user expects a terminal to appear.
    EnsureOpen {
        distro: Option<String>,
        tmux_session: String,
        /// Terminal emulator: "iterm2", "ghostty", "terminal_app",
        /// "windows_terminal", or "custom"
        emulator: String,
        /// Command template when emulator is "custom".
        /// Placeholders: {distro}, {tmux_session}
        custom_command: String,
    },
}

// ── Windows Terminal Management ──────────────────────────────────────────────
//
// Windows has only one real emulator choice: Windows Terminal (wt.exe).
// WinUI 3 apps have quirky window handle behavior, so detection uses a
// three-state enum and falls back to EnumWindows if MainWindowHandle is zero.

/// Result of checking for Windows Terminal (three-state for WinUI 3 quirks).
#[cfg(target_os = "windows")]
#[derive(Debug, PartialEq)]
enum WinTerminalStatus {
    /// Found the window and brought it to foreground.
    Focused,
    /// Process exists but we couldn't get a window handle to focus.
    /// The terminal tab is still there — don't create another.
    Running,
    /// No WindowsTerminal process found. Safe to launch a new one.
    NotRunning,
}

/// Single entry point for Windows terminal interactions.
///
/// Follows the same decision tree as macOS:
///   1. Is Windows Terminal running? → focus it (both FocusOnly and EnsureOpen)
///   2. Not running + FocusOnly → no-op
///   3. Not running + EnsureOpen → launch WT with tmux attach via WSL
#[cfg(target_os = "windows")]
pub fn handle_terminal(intent: TerminalIntent) -> Result<(), String> {
    // Step 1: Check if Windows Terminal is already running.
    let status = check_windows_terminal()?;

    match status {
        WinTerminalStatus::Focused => {
            tracing::debug!("Focused existing Windows Terminal");
            return Ok(());
        }
        WinTerminalStatus::Running => {
            tracing::debug!("Windows Terminal running (couldn't focus), skipping new tab");
            return Ok(());
        }
        WinTerminalStatus::NotRunning => {
            tracing::debug!("Windows Terminal not running");
        }
    }

    // Step 2: Not running. FocusOnly → nothing to do.
    let TerminalIntent::EnsureOpen { distro, tmux_session, emulator, custom_command } = intent
    else {
        return Ok(());
    };

    // Step 3: Launch the emulator.
    let Some(distro) = distro else {
        return Err("Cannot open terminal: no WSL distro configured".to_string());
    };

    crate::daemon::launcher::validate_wsl_distro(&distro)
        .map_err(|e| format!("Invalid WSL distro: {e}"))?;

    // Custom command: substitute placeholders and run.
    if emulator == "custom" && !custom_command.trim().is_empty() {
        return launch_custom_command(&custom_command, &distro, &tmux_session);
    }

    // Windows Terminal launch with named window.
    tracing::info!(%distro, %tmux_session, "Launching Windows Terminal with tmux attach");

    let mut args: Vec<String> = vec!["-w".into(), "taurhaus".into(), "new-tab".into()];

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

/// Launch a custom terminal command with placeholder substitution.
#[cfg(target_os = "windows")]
fn launch_custom_command(template: &str, distro: &str, tmux_session: &str) -> Result<(), String> {
    let cmd = template
        .replace("{distro}", distro)
        .replace("{tmux_session}", tmux_session);
    tracing::info!(%cmd, "Launching custom terminal emulator");
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Custom terminal command is empty".to_string());
    }
    std::process::Command::new(parts[0])
        .args(&parts[1..])
        .spawn()
        .map_err(|e| format!("Failed to launch custom terminal: {e}"))?;
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
/// Returns `Focused` if we found and focused the window,
/// `Running` if the process exists but we couldn't get a window handle,
/// or `NotRunning` if no WindowsTerminal process was found.
#[cfg(target_os = "windows")]
fn check_windows_terminal() -> Result<WinTerminalStatus, String> {
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
        "FOCUSED" => Ok(WinTerminalStatus::Focused),
        "RUNNING" => Ok(WinTerminalStatus::Running),
        _ => Ok(WinTerminalStatus::NotRunning),
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

// ── macOS Terminal Management ─────────────────────────────────────────────────
//
// Decision tree (the only path through the code):
//
//   OS (compile-time)
//   └── Emulator (from settings: "iterm2" | "ghostty" | "terminal_app" | "custom")
//       └── Action
//           ├── FocusOnly  → activate the preferred emulator if running
//           └── EnsureOpen → is preferred emulator already attached to tmux?
//               ├── Yes → just activate it
//               └── No  → launch it with `tmux attach-session -t <session>`
//
// Key invariant: we ALWAYS respect the user's emulator preference. We never
// fall through to a different terminal just because it happens to be running.
// tmux supports multiple clients — if an old Terminal.app is still attached,
// that's fine; we still open iTerm2 if that's what the user chose.

/// Resolved terminal emulator for macOS.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq)]
enum MacEmulator {
    ITerm2,
    Ghostty,
    TerminalApp,
}

#[cfg(target_os = "macos")]
impl MacEmulator {
    /// Resolve the user's emulator setting string to a concrete emulator.
    /// Falls back through auto-detect if the preferred app isn't installed.
    fn from_setting(pref: &str) -> Self {
        match pref {
            "iterm2" => {
                if is_app_installed("iTerm") { Self::ITerm2 }
                else { Self::auto_detect() }
            }
            "ghostty" => {
                if is_app_installed("Ghostty") { Self::Ghostty }
                else { Self::auto_detect() }
            }
            "terminal_app" => Self::TerminalApp,
            _ => Self::auto_detect(),
        }
    }

    /// Auto-detect best available emulator: iTerm2 > Ghostty > Terminal.app.
    fn auto_detect() -> Self {
        if is_app_installed("iTerm") { Self::ITerm2 }
        else if is_app_installed("Ghostty") { Self::Ghostty }
        else { Self::TerminalApp }
    }

    /// The macOS application name used in AppleScript and process checks.
    fn app_name(self) -> &'static str {
        match self {
            Self::ITerm2 => "iTerm",
            Self::Ghostty => "Ghostty",
            Self::TerminalApp => "Terminal",
        }
    }

    /// Check if this emulator is currently running (has a process).
    fn is_running(self) -> bool {
        let script = format!(
            r#"if application "{}" is running then return "yes"
return "no""#,
            self.app_name()
        );
        std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "yes")
            .unwrap_or(false)
    }

    /// Activate (bring to front) this emulator.
    fn activate(self) -> Result<(), String> {
        let script = format!(
            r#"tell application "{}" to activate"#,
            self.app_name()
        );
        tracing::debug!(emulator = ?self, "Activating terminal");
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .spawn();
        Ok(())
    }

    /// Launch this emulator with a tmux attach command.
    fn launch_with_tmux(self, tmux_session: &str) -> Result<(), String> {
        tracing::info!(emulator = ?self, %tmux_session, "Launching terminal with tmux attach");
        match self {
            Self::ITerm2 => {
                let script = format!(
                    r#"tell application "iTerm"
    activate
    if (count of windows) > 0 then
        tell current window
            create tab with default profile command "tmux attach-session -t {tmux_session}"
        end tell
    else
        create window with default profile command "tmux attach-session -t {tmux_session}"
    end if
end tell"#
                );
                std::process::Command::new("osascript")
                    .args(["-e", &script])
                    .spawn()
                    .map_err(|e| format!("Failed to launch iTerm2: {e}"))?;
            }
            Self::Ghostty => {
                std::process::Command::new("ghostty")
                    .args(["-e", "tmux", "attach-session", "-t", tmux_session])
                    .spawn()
                    .map_err(|e| format!("Failed to launch Ghostty: {e}"))?;
            }
            Self::TerminalApp => {
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
                std::process::Command::new("osascript")
                    .args(["-e", &script])
                    .spawn()
                    .map_err(|e| format!("Failed to launch Terminal.app: {e}"))?;
            }
        }
        Ok(())
    }
}

/// Check if a macOS application is installed in /Applications.
#[cfg(target_os = "macos")]
fn is_app_installed(app_name: &str) -> bool {
    std::path::Path::new(&format!("/Applications/{app_name}.app")).exists()
}

/// Check if our tmux session has any attached clients.
///
/// Uses `tmux list-clients` which is authoritative — it reports actual
/// terminal connections, not stale pgrep matches.
#[cfg(target_os = "macos")]
fn tmux_session_has_client(tmux_session: &str) -> bool {
    std::process::Command::new("tmux")
        .args(["list-clients", "-t", tmux_session])
        .output()
        .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
        .unwrap_or(false)
}

/// Single entry point for all macOS terminal interactions.
///
/// Decision flow:
///   1. Resolve which emulator to use (from setting + availability)
///   2. FocusOnly → activate preferred emulator if running
///   3. EnsureOpen → if preferred is already running & tmux has client, activate it
///                    otherwise launch preferred with tmux attach
#[cfg(target_os = "macos")]
pub fn handle_terminal(intent: TerminalIntent) -> Result<(), String> {
    match intent {
        TerminalIntent::FocusOnly { ref emulator } => {
            // Focus the user's preferred emulator if running.
            let resolved = MacEmulator::from_setting(emulator);
            if resolved.is_running() {
                return resolved.activate();
            }
            // Fallback: activate any running terminal.
            for candidate in &[MacEmulator::ITerm2, MacEmulator::Ghostty, MacEmulator::TerminalApp] {
                if candidate.is_running() {
                    return candidate.activate();
                }
            }
            Ok(()) // Nothing running, nothing to focus
        }

        TerminalIntent::EnsureOpen { tmux_session, emulator, custom_command, .. } => {
            // Custom command: run it directly, no further logic.
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

            // Resolve setting → concrete emulator
            let resolved = MacEmulator::from_setting(&emulator);
            tracing::info!(?resolved, setting = %emulator, "Resolved terminal emulator");

            // If the preferred emulator is already running AND tmux has a
            // client attached, just bring it to front. No new tab/window.
            if resolved.is_running() && tmux_session_has_client(&tmux_session) {
                tracing::debug!(?resolved, "Preferred emulator running with tmux client, activating");
                return resolved.activate();
            }

            // Otherwise: launch the preferred emulator with tmux attach.
            // This is correct even if another terminal has a tmux attachment —
            // tmux supports multiple clients simultaneously.
            resolved.launch_with_tmux(&tmux_session)
        }
    }
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
        let result = handle_terminal(TerminalIntent::FocusOnly {
            emulator: "windows_terminal".to_string(),
        });
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
