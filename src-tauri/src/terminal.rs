//! Terminal focus — bring Windows Terminal to the foreground.
//!
//! On Windows: uses PowerShell to find and focus the Windows Terminal window.
//! On Linux: no-op (the terminal is already in the user's workspace).

/// Attempt to bring Windows Terminal to the foreground.
///
/// Called after launching or navigating to a Claude Code session so the user
/// sees the terminal where the session is running.
#[cfg(target_os = "windows")]
pub fn focus_windows_terminal() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Use PowerShell to find and focus the Windows Terminal window.
    // The script finds the process, gets its main window handle, and calls
    // SetForegroundWindow via Add-Type interop.
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
        }
    "#;

    let output = std::process::Command::new("powershell")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(%stderr, "PowerShell focus script returned non-zero");
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn focus_windows_terminal() -> Result<(), String> {
    // On Linux, the terminal is already in the user's workspace.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_returns_ok_on_current_platform() {
        // On Linux (dev), this is a no-op and should always succeed.
        // On Windows, it may fail if PowerShell isn't available, but
        // shouldn't panic.
        let result = focus_windows_terminal();
        assert!(result.is_ok());
    }
}
