//! Path detection and translation utilities for WSL and Windows.
//!
//! Two path families need translation:
//!
//! 1. **WSL UNC paths** — Windows accessing WSL filesystems:
//!    - `\\wsl$\<distro>\...` ↔ `/home/user/...`
//!    - `\\wsl.localhost\<distro>\...` ↔ `/home/user/...`
//!
//! 2. **Windows drive paths** — WSL accessing Windows filesystems:
//!    - `D:\projects\foo` ↔ `/mnt/d/projects/foo`
//!
//! The daemon expects native Linux paths, so we translate at the boundary.

/// Check whether a path is a WSL UNC path.
pub fn is_wsl_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with(r"\\wsl$\") || lower.starts_with(r"\\wsl.localhost\")
}

/// Convert a Windows UNC WSL path to a native Linux path.
///
/// `\\wsl$\Ubuntu\home\user\projects` → `/home/user/projects`
/// `\\wsl.localhost\Ubuntu\home\user` → `/home/user`
///
/// Returns `None` if the path isn't a valid WSL UNC path or has no content
/// after the distro name.
pub fn wsl_unc_to_linux(unc_path: &str) -> Option<String> {
    let stripped = strip_wsl_prefix(unc_path)?;
    // Skip the distro name (first path segment), rest is the Linux path
    let sep_pos = stripped.find('\\')?;
    let after_distro = &stripped[sep_pos..];
    if after_distro.len() <= 1 {
        // Just the distro root — return "/"
        return Some("/".to_string());
    }
    Some(after_distro.replace('\\', "/"))
}

/// Extract the WSL distro name from a UNC path.
///
/// `\\wsl$\Ubuntu\home\user` → `Some("Ubuntu")`
/// `\\wsl.localhost\Debian\...` → `Some("Debian")`
pub fn wsl_distro_from_path(unc_path: &str) -> Option<String> {
    let stripped = strip_wsl_prefix(unc_path)?;
    let distro = stripped.split('\\').next()?;
    if distro.is_empty() {
        return None;
    }
    Some(distro.to_string())
}

/// Convert a Linux path back to a Windows WSL UNC path.
///
/// `/home/user/projects` + distro `Ubuntu` → `\\wsl.localhost\Ubuntu\home\user\projects`
///
/// Uses `\\wsl.localhost\` form (preferred over `\\wsl$\` since Windows 11).
pub fn linux_to_wsl_unc(linux_path: &str, distro: &str) -> String {
    let windows_subpath = linux_path.replace('/', "\\");
    format!(r"\\wsl.localhost\{distro}{windows_subpath}")
}

// ---------------------------------------------------------------------------
// Windows drive path translation (D:\foo ↔ /mnt/d/foo)
// ---------------------------------------------------------------------------

/// Check if a path is a Windows drive path (e.g., `D:\foo` or `C:\Users`).
pub fn is_windows_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Convert a Windows drive path to a WSL mount path.
///
/// `D:\projects\foo` → `/mnt/d/projects/foo`
/// `C:\Users\me` → `/mnt/c/Users/me`
///
/// Returns `None` if the path isn't a Windows drive path.
pub fn windows_drive_to_linux(path: &str) -> Option<String> {
    if !is_windows_drive_path(path) {
        return None;
    }
    let drive = (path.as_bytes()[0]).to_ascii_lowercase() as char;
    let rest = &path[2..]; // includes leading \ or /
    let linux_rest = rest.replace('\\', "/");
    Some(format!("/mnt/{drive}{linux_rest}"))
}

/// Convert a WSL mount path (`/mnt/<drive>/...`) back to a Windows drive path.
///
/// `/mnt/d/projects/foo` → `D:\projects\foo`
/// `/mnt/c/Users/me` → `C:\Users\me`
///
/// Returns `None` if the path isn't a `/mnt/<single-letter>/` mount.
pub fn linux_mount_to_windows(path: &str) -> Option<String> {
    if !path.starts_with("/mnt/") || path.len() < 6 {
        return None;
    }
    let drive_byte = path.as_bytes()[5];
    if !drive_byte.is_ascii_alphabetic() {
        return None;
    }
    // Must be followed by '/' or end of string (just `/mnt/d`)
    if path.len() > 6 && path.as_bytes()[6] != b'/' {
        return None;
    }
    let drive = (drive_byte).to_ascii_uppercase() as char;
    let rest = if path.len() > 6 { &path[6..] } else { "" };
    let win_rest = rest.replace('/', "\\");
    Some(format!("{drive}:{win_rest}"))
}

/// Convert any Windows path form to a Linux path.
///
/// Tries WSL UNC first (`\\wsl$\...`), then Windows drive (`D:\...`).
/// Returns `None` if the path is neither.
pub fn to_linux(path: &str) -> Option<String> {
    wsl_unc_to_linux(path).or_else(|| windows_drive_to_linux(path))
}

/// Convert a Linux path back to the appropriate Windows form.
///
/// - `/mnt/d/...` → `D:\...` (Windows-native mount)
/// - `/home/user/...` → `\\wsl.localhost\<distro>\...` (WSL-native)
///
/// Falls back to WSL UNC if the path isn't a `/mnt/<drive>/` mount.
pub fn to_windows(path: &str, distro: &str) -> String {
    linux_mount_to_windows(path).unwrap_or_else(|| linux_to_wsl_unc(path, distro))
}

/// Normalize a project path for stable cross-platform matching and DB keys.
///
/// Rules:
/// - Convert WSL UNC and Windows drive paths to Linux form when applicable.
/// - Convert backslashes to forward slashes.
/// - Collapse repeated separators.
/// - Strip trailing separators (except root `/`).
pub fn normalize_project_path(path: &str) -> String {
    let converted = to_linux(path).unwrap_or_else(|| path.to_string());
    normalize_linux_separators(&converted)
}

fn normalize_linux_separators(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut prev_slash = false;
    for ch in path.trim().chars() {
        let mapped = if ch == '\\' { '/' } else { ch };
        if mapped == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        out.push(mapped);
    }
    while out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

/// Strip the WSL UNC prefix, returning the rest starting with the distro name.
fn strip_wsl_prefix(path: &str) -> Option<&str> {
    // Case-insensitive prefix matching: normalize to lowercase for comparison
    // but return the original-cased remainder.
    let lower = path.to_ascii_lowercase();
    if lower.starts_with(r"\\wsl$\") {
        Some(&path[7..]) // len of `\\wsl$\` = 7
    } else if lower.starts_with(r"\\wsl.localhost\") {
        Some(&path[16..]) // len of `\\wsl.localhost\` = 16
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- is_wsl_path --

    #[test]
    fn detects_wsl_dollar_path() {
        assert!(is_wsl_path(r"\\wsl$\Ubuntu\home\user\projects"));
    }

    #[test]
    fn detects_wsl_localhost_path() {
        assert!(is_wsl_path(r"\\wsl.localhost\Ubuntu\home\user"));
    }

    #[test]
    fn detects_wsl_path_case_insensitive() {
        assert!(is_wsl_path(r"\\WSL$\Ubuntu\home\user"));
        assert!(is_wsl_path(r"\\WSL.LOCALHOST\Debian\home"));
    }

    #[test]
    fn rejects_windows_local_path() {
        assert!(!is_wsl_path(r"C:\Users\me\projects"));
        assert!(!is_wsl_path(r"D:\code"));
    }

    #[test]
    fn rejects_linux_path() {
        assert!(!is_wsl_path("/home/user/projects"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_wsl_path(""));
    }

    // -- wsl_unc_to_linux --

    #[test]
    fn converts_wsl_dollar_to_linux() {
        assert_eq!(
            wsl_unc_to_linux(r"\\wsl$\Ubuntu\home\user\projects\foo"),
            Some("/home/user/projects/foo".to_string())
        );
    }

    #[test]
    fn converts_wsl_localhost_to_linux() {
        assert_eq!(
            wsl_unc_to_linux(r"\\wsl.localhost\Ubuntu\home\user"),
            Some("/home/user".to_string())
        );
    }

    #[test]
    fn distro_root_returns_slash() {
        assert_eq!(wsl_unc_to_linux(r"\\wsl$\Ubuntu\"), Some("/".to_string()));
    }

    #[test]
    fn no_path_after_distro_returns_none() {
        // Just the distro name with no trailing backslash
        assert_eq!(wsl_unc_to_linux(r"\\wsl$\Ubuntu"), None);
    }

    #[test]
    fn conversion_preserves_original_case() {
        assert_eq!(
            wsl_unc_to_linux(r"\\WSL$\Ubuntu\Home\User"),
            Some("/Home/User".to_string())
        );
    }

    #[test]
    fn not_wsl_path_returns_none() {
        assert_eq!(wsl_unc_to_linux(r"C:\Users\me"), None);
    }

    // -- wsl_distro_from_path --

    #[test]
    fn extracts_distro_from_wsl_dollar() {
        assert_eq!(
            wsl_distro_from_path(r"\\wsl$\Ubuntu\home\user"),
            Some("Ubuntu".to_string())
        );
    }

    #[test]
    fn extracts_distro_from_wsl_localhost() {
        assert_eq!(
            wsl_distro_from_path(r"\\wsl.localhost\Debian\home"),
            Some("Debian".to_string())
        );
    }

    #[test]
    fn distro_preserves_case() {
        assert_eq!(
            wsl_distro_from_path(r"\\WSL$\MyDistro\home"),
            Some("MyDistro".to_string())
        );
    }

    #[test]
    fn no_distro_returns_none() {
        assert_eq!(wsl_distro_from_path(r"C:\Users\me"), None);
    }

    // -- linux_to_wsl_unc --

    #[test]
    fn converts_linux_to_unc() {
        assert_eq!(
            linux_to_wsl_unc("/home/user/projects", "Ubuntu"),
            r"\\wsl.localhost\Ubuntu\home\user\projects"
        );
    }

    #[test]
    fn converts_root_to_unc() {
        assert_eq!(linux_to_wsl_unc("/", "Ubuntu"), r"\\wsl.localhost\Ubuntu\");
    }

    // -- round-trip --

    #[test]
    fn round_trip_unc_to_linux_and_back() {
        let original = r"\\wsl.localhost\Ubuntu\home\user\projects\foo";
        let linux = wsl_unc_to_linux(original).unwrap();
        let distro = wsl_distro_from_path(original).unwrap();
        let back = linux_to_wsl_unc(&linux, &distro);
        assert_eq!(back, original);
    }

    #[test]
    fn round_trip_wsl_dollar_normalizes_to_wsl_localhost() {
        let original = r"\\wsl$\Ubuntu\home\user";
        let linux = wsl_unc_to_linux(original).unwrap();
        let distro = wsl_distro_from_path(original).unwrap();
        let back = linux_to_wsl_unc(&linux, &distro);
        // Round-trip normalizes \\wsl$ to \\wsl.localhost
        assert_eq!(back, r"\\wsl.localhost\Ubuntu\home\user");
    }

    // -- is_windows_drive_path --

    #[test]
    fn detects_windows_drive_paths() {
        assert!(is_windows_drive_path(r"D:\projects\foo"));
        assert!(is_windows_drive_path(r"C:\Users\me"));
        assert!(is_windows_drive_path(r"c:\lowercase"));
        assert!(is_windows_drive_path("D:/forward/slashes"));
    }

    #[test]
    fn rejects_non_drive_paths() {
        assert!(!is_windows_drive_path("/home/user"));
        assert!(!is_windows_drive_path(r"\\wsl$\Ubuntu\home"));
        assert!(!is_windows_drive_path(""));
        assert!(!is_windows_drive_path("D:")); // no separator after colon
        assert!(!is_windows_drive_path("1:\\bad")); // digit, not letter
    }

    // -- windows_drive_to_linux --

    #[test]
    fn converts_drive_path_to_linux() {
        assert_eq!(
            windows_drive_to_linux(r"D:\projects\foo"),
            Some("/mnt/d/projects/foo".to_string())
        );
    }

    #[test]
    fn converts_c_drive_to_linux() {
        assert_eq!(
            windows_drive_to_linux(r"C:\Users\me\code"),
            Some("/mnt/c/Users/me/code".to_string())
        );
    }

    #[test]
    fn converts_uppercase_drive_to_lowercase_mount() {
        assert_eq!(
            windows_drive_to_linux(r"E:\data"),
            Some("/mnt/e/data".to_string())
        );
    }

    #[test]
    fn drive_conversion_returns_none_for_non_drive() {
        assert_eq!(windows_drive_to_linux("/home/user"), None);
        assert_eq!(windows_drive_to_linux(r"\\wsl$\Ubuntu\home"), None);
    }

    // -- linux_mount_to_windows --

    #[test]
    fn converts_linux_mount_to_drive() {
        assert_eq!(
            linux_mount_to_windows("/mnt/d/projects/foo"),
            Some(r"D:\projects\foo".to_string())
        );
    }

    #[test]
    fn converts_linux_mount_uppercase() {
        assert_eq!(
            linux_mount_to_windows("/mnt/c/Users/me"),
            Some(r"C:\Users\me".to_string())
        );
    }

    #[test]
    fn mount_bare_drive() {
        assert_eq!(linux_mount_to_windows("/mnt/d"), Some("D:".to_string()));
    }

    #[test]
    fn mount_rejects_non_mount_paths() {
        assert_eq!(linux_mount_to_windows("/home/user"), None);
        assert_eq!(linux_mount_to_windows("/mnt/"), None); // no drive letter
        assert_eq!(linux_mount_to_windows("/mnt/dd/foo"), None); // multi-char
    }

    // -- round-trip drive paths --

    #[test]
    fn round_trip_drive_to_linux_and_back() {
        let original = r"D:\projects\taurhaus";
        let linux = windows_drive_to_linux(original).unwrap();
        assert_eq!(linux, "/mnt/d/projects/taurhaus");
        let back = linux_mount_to_windows(&linux).unwrap();
        assert_eq!(back, original);
    }

    // -- to_linux / to_windows --

    #[test]
    fn to_linux_handles_both_path_types() {
        assert_eq!(
            to_linux(r"\\wsl$\Ubuntu\home\user"),
            Some("/home/user".to_string())
        );
        assert_eq!(
            to_linux(r"D:\projects\foo"),
            Some("/mnt/d/projects/foo".to_string())
        );
        assert_eq!(to_linux("/already/linux"), None);
    }

    #[test]
    fn to_windows_routes_correctly() {
        // /mnt mount → drive path
        assert_eq!(
            to_windows("/mnt/d/projects/foo", "Ubuntu"),
            r"D:\projects\foo"
        );
        // WSL-native → UNC path
        assert_eq!(
            to_windows("/home/user/projects", "Ubuntu"),
            r"\\wsl.localhost\Ubuntu\home\user\projects"
        );
    }

    #[test]
    fn normalize_project_path_converts_wsl_unc_to_linux() {
        assert_eq!(
            normalize_project_path(r"\\wsl.localhost\Ubuntu\home\user\proj\"),
            "/home/user/proj".to_string()
        );
    }

    #[test]
    fn normalize_project_path_converts_drive_paths_and_trims() {
        assert_eq!(
            normalize_project_path(r"D:\projects\taurhaus\\"),
            "/mnt/d/projects/taurhaus".to_string()
        );
    }

    #[test]
    fn normalize_project_path_normalizes_relative_and_repeated_separators() {
        assert_eq!(
            normalize_project_path(r"foo\\bar///baz/"),
            "foo/bar/baz".to_string()
        );
    }

    // -----------------------------------------------------------------------
    // Cross-platform path safety tests
    //
    // These verify that macOS/Linux native paths are never corrupted by
    // Windows/WSL path conversion logic. Critical for the unified daemon
    // architecture where the same code runs on all platforms.
    // -----------------------------------------------------------------------

    #[test]
    fn macos_path_is_not_wsl() {
        assert!(!is_wsl_path("/Users/dev/projects/myapp"));
        assert!(!is_wsl_path("/Users/dev"));
        assert!(!is_wsl_path("/"));
    }

    #[test]
    fn linux_native_path_is_not_wsl() {
        assert!(!is_wsl_path("/home/user/projects/myapp"));
        assert!(!is_wsl_path("/opt/data"));
        assert!(!is_wsl_path("/var/lib/myapp"));
    }

    #[test]
    fn to_linux_returns_none_for_native_paths() {
        // Native macOS/Linux paths should NOT be converted — they're already valid.
        assert_eq!(to_linux("/Users/dev/projects/myapp"), None);
        assert_eq!(to_linux("/home/user/code"), None);
        assert_eq!(to_linux("/opt/data"), None);
    }

    #[test]
    fn to_linux_unwrap_preserves_native_paths() {
        // The pattern used in command_center.rs: to_linux().unwrap_or(original)
        // Must preserve native paths untouched.
        let macos_path = "/Users/dev/projects/myapp";
        let result = to_linux(macos_path).unwrap_or_else(|| macos_path.to_string());
        assert_eq!(result, macos_path);

        let linux_path = "/home/user/code/app";
        let result = to_linux(linux_path).unwrap_or_else(|| linux_path.to_string());
        assert_eq!(result, linux_path);
    }

    #[test]
    fn to_windows_corrupts_native_paths() {
        // This documents WHY we skip to_windows on native platforms:
        // calling it with a native path and "native" distro produces garbage.
        let native_path = "/Users/dev/projects/myapp";
        let corrupted = to_windows(native_path, "native");
        assert!(
            corrupted.contains("wsl.localhost"),
            "to_windows converts native paths to UNC — must not be called on native platforms"
        );
    }

    #[test]
    fn wsl_distro_from_native_path_is_none() {
        // On macOS/Linux, no project paths should extract a WSL distro.
        assert_eq!(wsl_distro_from_path("/Users/dev/projects"), None);
        assert_eq!(wsl_distro_from_path("/home/user/code"), None);
    }

    #[test]
    fn native_path_routes_to_local_provider() {
        // Verify the provider routing logic works for native paths.
        // All native paths (macOS/Linux) should route to local, never daemon.
        assert!(!is_wsl_path("/Users/dev/myapp"));
        assert!(!is_wsl_path("/home/user/code"));
        // Windows drive paths also route to local
        assert!(!is_wsl_path(r"C:\Users\me\projects"));
    }
}
