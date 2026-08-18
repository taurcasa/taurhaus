//! Shared mesh/WSL command helpers used by coordination runtime + backend.

use std::path::PathBuf;
use std::process::Command;

const COORDINATION_WSL_DISTRO_ENV: &str = "TAURHAUS_WSL_DISTRO";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub program: String,
    pub args: Vec<String>,
}

pub fn command_invocation(program: &str, args: &[String]) -> CommandInvocation {
    command_invocation_for_distro(program, args, None)
}

pub fn command_invocation_for_distro(
    program: &str,
    args: &[String],
    explicit_distro: Option<&str>,
) -> CommandInvocation {
    if cfg!(target_os = "windows") {
        let mut invocation_args = wrap_wsl_args_for_coordination(
            vec!["-e".to_string(), program.to_string()],
            explicit_distro,
        );
        invocation_args.extend(args.iter().cloned());
        CommandInvocation {
            program: "wsl".to_string(),
            args: invocation_args,
        }
    } else {
        CommandInvocation {
            program: program.to_string(),
            args: args.to_vec(),
        }
    }
}

pub fn mesh_binary_path() -> Option<String> {
    mesh_binary_path_for_distro(None)
}

pub fn mesh_binary_path_for_distro(explicit_distro: Option<&str>) -> Option<String> {
    if cfg!(target_os = "windows") {
        resolve_wsl_home_for_coordination_in_distro(explicit_distro)
            .map(|home| format!("{home}/.local/bin/mesh"))
    } else {
        dirs::home_dir().map(|home| home.join(".local/bin/mesh").to_string_lossy().to_string())
    }
}

pub fn mesh_command_invocation(args: &[&str]) -> CommandInvocation {
    mesh_command_invocation_with_env(args, &[])
}

pub fn mesh_command_invocation_with_env(
    args: &[&str],
    env: &[(String, String)],
) -> CommandInvocation {
    mesh_command_invocation_with_env_for_distro(args, env, None)
}

pub fn mesh_command_invocation_with_env_for_distro(
    args: &[&str],
    env: &[(String, String)],
    explicit_distro: Option<&str>,
) -> CommandInvocation {
    let mesh_path =
        mesh_binary_path_for_distro(explicit_distro).unwrap_or_else(|| "mesh".to_string());
    let mesh_args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    if env.is_empty() {
        return command_invocation_for_distro(&mesh_path, &mesh_args, explicit_distro);
    }

    let mut invocation_args = env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>();
    invocation_args.push(mesh_path);
    invocation_args.extend(mesh_args);
    command_invocation_for_distro("env", &invocation_args, explicit_distro)
}

pub fn wsl_command_for_coordination() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new("wsl");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // GUI apps have no stdin; inheriting a broken handle causes wsl.exe
    // to fail or conhost.exe to spin CPU on Windows.
    cmd.stdin(std::process::Stdio::null());
    cmd
}

pub fn resolve_wsl_home_for_coordination() -> Option<String> {
    resolve_wsl_home_for_coordination_in_distro(None)
}

pub fn resolve_wsl_home_for_coordination_in_distro(
    explicit_distro: Option<&str>,
) -> Option<String> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    let output = wsl_command_for_coordination()
        .args(wrap_wsl_args_for_coordination(
            vec![
                "-e".to_string(),
                "sh".to_string(),
                "-c".to_string(),
                "echo $HOME".to_string(),
            ],
            explicit_distro,
        ))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_wsl_unix_path_from_stdout(&output.stdout)
}

pub fn resolve_wsl_binary_path(binary_name: &str) -> Option<String> {
    resolve_wsl_binary_path_for_distro(binary_name, None)
}

pub fn resolve_wsl_binary_path_for_distro(
    binary_name: &str,
    explicit_distro: Option<&str>,
) -> Option<String> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    if !binary_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    if let Some(home) = resolve_wsl_home_for_coordination_in_distro(explicit_distro) {
        let candidate = format!("{home}/.local/bin/{binary_name}");
        let check = wsl_command_for_coordination()
            .args(wrap_wsl_args_for_coordination(
                vec![
                    "--".to_string(),
                    "test".to_string(),
                    "-x".to_string(),
                    candidate.clone(),
                ],
                explicit_distro,
            ))
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        if check.status.success() {
            return Some(candidate);
        }
    }

    let cmd = format!("command -v {binary_name}");
    let output = wsl_command_for_coordination()
        .args(wrap_wsl_args_for_coordination(
            vec!["-e".to_string(), "sh".to_string(), "-c".to_string(), cmd],
            explicit_distro,
        ))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_wsl_unix_path_from_stdout(&output.stdout)
}

pub fn parse_wsl_unix_path_from_stdout(stdout: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stdout);
    text.lines()
        .map(str::trim)
        .rev()
        .find(|line| !line.is_empty() && line.starts_with('/'))
        .map(ToString::to_string)
}

fn parse_wsl_distro_from_stdout(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    text.lines()
        .map(|line| line.replace('\0', "").trim().to_string())
        .find(|line| !line.is_empty())
}

pub fn set_preferred_wsl_distro_for_coordination(distro: Option<&str>) {
    match distro
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "native")
    {
        Some(value) => std::env::set_var(COORDINATION_WSL_DISTRO_ENV, value),
        None => std::env::remove_var(COORDINATION_WSL_DISTRO_ENV),
    }
}

fn preferred_wsl_distro_for_coordination() -> Option<String> {
    std::env::var(COORDINATION_WSL_DISTRO_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "native")
}

fn choose_wsl_distro_for_coordination(
    explicit_distro: Option<&str>,
    preferred_distro: Option<&str>,
    detected_default: Option<&str>,
) -> Option<String> {
    explicit_distro
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "native")
        .map(ToString::to_string)
        .or_else(|| {
            preferred_distro
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "native")
                .map(ToString::to_string)
        })
        .or_else(|| {
            detected_default
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "native")
                .map(ToString::to_string)
        })
}

pub fn resolve_wsl_distro_for_coordination(explicit_distro: Option<&str>) -> Option<String> {
    if !cfg!(target_os = "windows") {
        return None;
    }

    choose_wsl_distro_for_coordination(
        explicit_distro,
        preferred_wsl_distro_for_coordination().as_deref(),
        resolve_default_wsl_distro().as_deref(),
    )
}

fn resolve_default_wsl_distro() -> Option<String> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    let output = wsl_command_for_coordination()
        .args(["--list", "--quiet"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_wsl_distro_from_stdout(&output.stdout)
}

fn windows_mesh_teams_dir_from_parts(distro: &str, wsl_home: &str) -> PathBuf {
    let home = wsl_home.trim_end_matches('/');
    let linux_path = format!("{home}/.claude/teams");
    let windows_subpath = linux_path.replace('/', "\\");
    PathBuf::from(format!(r"\\wsl.localhost\{distro}{windows_subpath}"))
}

pub fn resolve_windows_mesh_teams_dir() -> Option<PathBuf> {
    resolve_windows_mesh_teams_dir_for_distro(None)
}

pub fn resolve_windows_mesh_teams_dir_for_distro(explicit_distro: Option<&str>) -> Option<PathBuf> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    let distro = resolve_wsl_distro_for_coordination(explicit_distro)?;
    let home = resolve_wsl_home_for_coordination_in_distro(Some(&distro))?;
    Some(windows_mesh_teams_dir_from_parts(&distro, &home))
}

pub fn wrap_wsl_args_for_coordination(
    mut args: Vec<String>,
    explicit_distro: Option<&str>,
) -> Vec<String> {
    if !cfg!(target_os = "windows") {
        return args;
    }
    if let Some(distro) = resolve_wsl_distro_for_coordination(explicit_distro) {
        let mut prefixed = vec!["-d".to_string(), distro];
        prefixed.append(&mut args);
        return prefixed;
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{LazyLock, Mutex, MutexGuard};

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    struct EnvGuard {
        _guard: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self {
                _guard: ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner()),
            }
        }
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_handles_clean_output() {
        let stdout = b"/home/user\n";
        assert_eq!(
            parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/user".to_string())
        );
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_ignores_banner_noise() {
        let stdout = b"Welcome to Ubuntu 22.04.5 LTS\nThis message is shown once a day.\n/home/user/.local/bin/mesh\n";
        assert_eq!(
            parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/user/.local/bin/mesh".to_string())
        );
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_returns_none_without_path() {
        let stdout = b"Welcome to Ubuntu 22.04.5 LTS\nNo path here\n";
        assert_eq!(parse_wsl_unix_path_from_stdout(stdout), None);
    }

    #[test]
    fn parse_wsl_distro_from_stdout_handles_utf16le_nulls() {
        let raw = b"U\0b\0u\0n\0t\0u\0\n\0";
        assert_eq!(
            parse_wsl_distro_from_stdout(raw),
            Some("Ubuntu".to_string())
        );
    }

    #[test]
    fn parse_wsl_distro_from_stdout_handles_utf8() {
        assert_eq!(
            parse_wsl_distro_from_stdout(b"Ubuntu\nDebian\n"),
            Some("Ubuntu".to_string())
        );
    }

    #[test]
    fn windows_mesh_teams_dir_builder_uses_unc_path() {
        let actual = windows_mesh_teams_dir_from_parts("Ubuntu", "/home/user");
        assert_eq!(
            actual.to_string_lossy(),
            r"\\wsl.localhost\Ubuntu\home\user\.claude\teams"
        );
    }

    #[test]
    fn explicit_or_preferred_distro_wins_over_default_for_coordination() {
        assert_eq!(
            choose_wsl_distro_for_coordination(Some("Debian"), Some("Kali"), Some("Ubuntu")),
            Some("Debian".to_string())
        );
        assert_eq!(
            choose_wsl_distro_for_coordination(None, Some("Debian"), Some("Ubuntu")),
            Some("Debian".to_string())
        );
        assert_eq!(
            choose_wsl_distro_for_coordination(None, None, Some("Ubuntu")),
            Some("Ubuntu".to_string())
        );
    }

    #[test]
    fn startup_selected_distro_keeps_coordination_root_aligned_with_project_distro() {
        let _guard = EnvGuard::new();
        set_preferred_wsl_distro_for_coordination(Some("Debian"));

        let teams_dir = resolve_windows_mesh_teams_dir_for_distro(Some("Debian"))
            .unwrap_or_else(|| windows_mesh_teams_dir_from_parts("Debian", "/home/user"));

        set_preferred_wsl_distro_for_coordination(None);

        assert_eq!(
            teams_dir.to_string_lossy(),
            r"\\wsl.localhost\Debian\home\user\.claude\teams"
        );
    }

    #[test]
    fn wrap_wsl_args_includes_selected_distro_before_command_args() {
        let _guard = EnvGuard::new();
        set_preferred_wsl_distro_for_coordination(Some("Debian"));

        let args =
            wrap_wsl_args_for_coordination(vec!["-e".to_string(), "which".to_string()], None);

        set_preferred_wsl_distro_for_coordination(None);

        if cfg!(target_os = "windows") {
            assert_eq!(args[..4], ["-d", "Debian", "-e", "which"]);
        } else {
            assert_eq!(args, vec!["-e".to_string(), "which".to_string()]);
        }
    }
}
