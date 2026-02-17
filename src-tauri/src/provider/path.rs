/// WSL path detection and translation utilities.
///
/// Windows accesses WSL filesystems via UNC paths:
/// - `\\wsl$\<distro>\...`
/// - `\\wsl.localhost\<distro>\...`
///
/// The daemon expects native Linux paths (`/home/user/...`), so we need to
/// translate between the two forms.

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
        assert_eq!(
            wsl_unc_to_linux(r"\\wsl$\Ubuntu\"),
            Some("/".to_string())
        );
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
        assert_eq!(
            linux_to_wsl_unc("/", "Ubuntu"),
            r"\\wsl.localhost\Ubuntu\"
        );
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
}
