//! Shared mesh/WSL command helpers used by coordination runtime + backend.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandInvocation {
    pub program: String,
    pub args: Vec<String>,
}

pub fn command_invocation(program: &str, args: &[String]) -> CommandInvocation {
    if cfg!(target_os = "windows") {
        let mut invocation_args = vec!["-e".to_string(), program.to_string()];
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
    if cfg!(target_os = "windows") {
        resolve_wsl_home_for_coordination().map(|home| format!("{home}/.local/bin/mesh"))
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
    let mesh_path = mesh_binary_path().unwrap_or_else(|| "mesh".to_string());
    let mesh_args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    if env.is_empty() {
        return command_invocation(&mesh_path, &mesh_args);
    }

    let mut invocation_args = env
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>();
    invocation_args.push(mesh_path);
    invocation_args.extend(mesh_args);
    command_invocation("env", &invocation_args)
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
    if !cfg!(target_os = "windows") {
        return None;
    }

    let output = wsl_command_for_coordination()
        .args(["-e", "sh", "-c", "echo $HOME"])
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
    if !cfg!(target_os = "windows") {
        return None;
    }

    if !binary_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    if let Some(home) = resolve_wsl_home_for_coordination() {
        let candidate = format!("{home}/.local/bin/{binary_name}");
        let check = wsl_command_for_coordination()
            .args(["--", "test", "-x", &candidate])
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
        .args(["-e", "sh", "-c", &cmd])
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
    if !cfg!(target_os = "windows") {
        return None;
    }
    let distro = resolve_default_wsl_distro()?;
    let home = resolve_wsl_home_for_coordination()?;
    Some(windows_mesh_teams_dir_from_parts(&distro, &home))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_wsl_unix_path_from_stdout_handles_clean_output() {
        let stdout = b"/home/mstie\n";
        assert_eq!(
            parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/mstie".to_string())
        );
    }

    #[test]
    fn parse_wsl_unix_path_from_stdout_ignores_banner_noise() {
        let stdout = b"Welcome to Ubuntu 22.04.5 LTS\nThis message is shown once a day.\n/home/mstie/.local/bin/mesh\n";
        assert_eq!(
            parse_wsl_unix_path_from_stdout(stdout),
            Some("/home/mstie/.local/bin/mesh".to_string())
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
        let actual = windows_mesh_teams_dir_from_parts("Ubuntu", "/home/mstie");
        assert_eq!(
            actual.to_string_lossy(),
            r"\\wsl.localhost\Ubuntu\home\mstie\.claude\teams"
        );
    }
}
