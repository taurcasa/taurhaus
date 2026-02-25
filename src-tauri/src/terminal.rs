//! Terminal management — unified Windows Terminal interaction.
//!
//! Single entry point (`handle_terminal`) handles all terminal interaction:
//! focusing an existing window, or launching a new one attached to tmux.
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
    },
}

/// Single entry point for all Windows Terminal interactions.
///
/// Both intents start by checking for an existing terminal window:
/// - If found → focus it (restore + foreground) and return.
/// - If not found → `FocusOnly` returns silently, `EnsureOpen` launches WT.
///
/// The `-w taurhaus` flag on `wt.exe` uses a named window — if a WT window
/// named "taurhaus" already exists, the tab opens there. If not, a new
/// window is created with that name. This is deterministic: we always
/// target *our* window, not "whatever was last used."
#[cfg(target_os = "windows")]
pub fn handle_terminal(intent: TerminalIntent) -> Result<(), String> {
    let (focused, _) = try_focus_windows_terminal()?;

    if focused {
        tracing::debug!("Focused existing Windows Terminal");
        return Ok(());
    }

    // Terminal not running. For FocusOnly, nothing more to do.
    let TerminalIntent::EnsureOpen { distro, tmux_session } = intent else {
        return Ok(());
    };

    let Some(distro) = distro else {
        return Err("Cannot open terminal: no WSL distro configured".to_string());
    };

    crate::daemon::launcher::validate_wsl_distro(&distro)
        .map_err(|e| format!("Invalid WSL distro: {e}"))?;

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

/// Try to find and focus an existing Windows Terminal window.
///
/// Returns `(focused, stdout)` — `focused` is true if a terminal was found
/// and brought to foreground.
#[cfg(target_os = "windows")]
fn try_focus_windows_terminal() -> Result<(bool, String), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let script = r#"
        Add-Type @"
            using System;
            using System.Runtime.InteropServices;
            public class WinApi {
                [DllImport("user32.dll")]
                public static extern bool SetForegroundWindow(IntPtr hWnd);
                [DllImport("user32.dll")]
                public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
            }
"@
        $proc = Get-Process -Name WindowsTerminal -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($proc -and $proc.MainWindowHandle -ne [IntPtr]::Zero) {
            [WinApi]::ShowWindow($proc.MainWindowHandle, 9)  # SW_RESTORE
            [WinApi]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
            Write-Output 'FOCUSED'
        } else {
            Write-Output 'NOT_RUNNING'
        }
    "#;

    let output = std::process::Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((stdout == "FOCUSED", stdout))
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

// Linux: no-op — terminal is already in the user's workspace.

#[cfg(not(target_os = "windows"))]
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
        });
        assert!(result.is_ok());
    }
}
