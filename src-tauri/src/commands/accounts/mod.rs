//! Tool accounts, as the frontend sees them.
//!
//! Detection runs where the config dirs are: in-process on Linux and macOS, in
//! the WSL daemon on Windows. Three answers come back from that daemon and they
//! mean different things. A daemon that predates the additive
//! `list_accounts` method answers `UNKNOWN_METHOD`, and an empty list is
//! then the honest answer — the chooser and the chip stay hidden, and launches
//! keep rendering exactly as they did before this feature existed. A daemon
//! that is *gone* has answered nothing at all: that empty list is silence, it
//! is reported as degraded, and the frontend keeps showing the last accounts it
//! knew rather than pretending the subscriptions vanished.

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use tauri::State;

use crate::commands::lifecycle::IpcCommandSpan;
use crate::commands::projects::DbState;
use crate::daemon::protocol;
use crate::db::queries;
use crate::errors::{sanitize_error, AppError, CommandResultExt, IpcResult, SanitizeErr};
use crate::session_scanner::accounts::{self, Account};
use crate::session_scanner::cli_tool::{spec, CliTool};
use crate::session_scanner::launch_base::{self, ResolvedBase};
use crate::ProviderState;

/// Detection ran in this process.
pub(crate) const SOURCE_NATIVE: &str = "native";
/// Detection ran in the WSL daemon, where the config dirs are.
pub(crate) const SOURCE_DAEMON: &str = "daemon";

const UNKNOWN_METHOD: &str = "UNKNOWN_METHOD";

/// How long the daemon gets to say what a launch command means.
///
/// The daemon answers this one by running an interactive shell, so the request
/// has to outlive the resolution's own budget with room for the transport. A
/// request that expires first is indistinguishable from a daemon that cannot
/// resolve anything: the literal base comes back, the alias goes unseen, and
/// its selector overrides the account the operator chose — the defect this
/// resolution exists to fix.
const RESOLVE_LAUNCH_BASE_TIMEOUT: Duration =
    Duration::from_secs(launch_base::RESOLUTION_BUDGET.as_secs() + 4);
#[cfg(target_os = "windows")]
const ACCOUNT_DIRECTORY_CREATE_TIMEOUT: Duration = Duration::from_secs(15);

/// Detected accounts for one registry tool.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsResult {
    pub accounts: Vec<Account>,
    pub source: String,
    pub degraded: bool,
    pub error: Option<String>,
    /// Retained for wire compatibility and always empty: what the pane shell
    /// makes of the configured commands comes from the dedicated
    /// `resolve_launch_bases` command, never from this report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resolved_bases: Vec<ResolvedBase>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProjectRelationship {
    pub id: String,
    pub name: String,
    pub path: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountTeamRelationship {
    pub name: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub project_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRelationships {
    pub pinned_projects: Vec<AccountProjectRelationship>,
    pub last_used_projects: Vec<AccountProjectRelationship>,
    pub teams: Vec<AccountTeamRelationship>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRelationshipIndex {
    pub by_account: HashMap<String, AccountRelationships>,
}

/// One Settings-only base resolution, including the selector value already
/// classified by the backend's shared shell-word parser.
///
/// `ResolvedBase` itself stays unchanged because it also travels over the
/// app↔daemon protocol. This additive local IPC result keeps shell syntax out
/// of the frontend without changing that protocol.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLaunchBase {
    #[serde(flatten)]
    pub base: ResolvedBase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector_value: Option<String>,
    /// Which launch modes use this command. Distinct commands per mode mean
    /// the resolver can select different accounts per mode; ambient relevance
    /// must judge each mode, not the first selector found anywhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<String>,
}

/// The transcript that owns a project's history, and whether the lookup ran.
///
/// A resume that falls through an *unavailable* lookup is not choosing an
/// account; it is proceeding without the one fact that decides one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptLookup {
    pub transcript: Option<PathBuf>,
    pub unavailable: Option<String>,
}

/// One daemon answer, told apart by what it means.
pub(crate) enum DaemonAnswer<T> {
    Value(T),
    /// A daemon built before the method existed. Nothing is the right answer.
    Unsupported,
    /// Nothing answered: no daemon, a dropped connection, a timeout, or a
    /// payload this build cannot read.
    Unavailable(String),
}

#[tauri::command(async)]
pub fn list_accounts(
    provider: State<'_, ProviderState>,
    tool: CliTool,
) -> IpcResult<AccountsResult> {
    let span = IpcCommandSpan::start("list_accounts");
    let result =
        Ok::<_, String>(list_accounts_impl(provider.inner(), tool)).ipc_cmd("list_accounts");
    span.finish_result(&result);
    result
}

/// The accounts read path never asks an interactive shell what a launch means.
pub(crate) fn list_accounts_impl(provider: &ProviderState, tool: CliTool) -> AccountsResult {
    accounts_report(provider, tool)
}

/// Resolve the launch commands only for the Settings accounts surface.
#[tauri::command(async)]
pub fn resolve_launch_bases(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    tool: CliTool,
    force: Option<bool>,
) -> IpcResult<Vec<ResolvedLaunchBase>> {
    let span = IpcCommandSpan::start("resolve_launch_bases");
    let result = Ok::<_, String>(resolve_launch_bases_impl(
        db.inner(),
        provider.inner(),
        tool,
        force.unwrap_or(false),
    ))
    .ipc_cmd("resolve_launch_bases");
    span.finish_result(&result);
    result
}

pub(crate) fn resolve_launch_bases_impl(
    db: &DbState,
    provider: &ProviderState,
    tool: CliTool,
    force: bool,
) -> Vec<ResolvedLaunchBase> {
    if force && !cfg!(target_os = "windows") {
        launch_base::invalidate_base_command_cache();
    }
    let commands = crate::commands::terminal_settings::load_terminal_settings(db).cli_commands;
    let mut seen = std::collections::HashSet::new();
    let mut base_modes: Vec<(String, Vec<String>)> = Vec::new();
    for (mode, name) in [
        (protocol::LaunchMode::Fresh, "fresh"),
        (protocol::LaunchMode::Continue, "continue"),
        (protocol::LaunchMode::Resume, "resume"),
    ] {
        let base = crate::session_scanner::launch::base_command(&commands, tool, mode);
        if seen.insert(base.to_string()) {
            base_modes.push((base.to_string(), vec![name.to_string()]));
        } else if let Some((_, modes)) = base_modes.iter_mut().find(|(b, _)| *b == base) {
            modes.push(name.to_string());
        }
    }
    let bases: Vec<&str> = base_modes.iter().map(|(base, _)| base.as_str()).collect();
    resolve_bases_threading_force(&bases, force, |base, force| {
        resolve_launch_base_with_force_tracked(provider, tool, base, force)
    })
    .into_iter()
    .zip(base_modes.into_iter().map(|(_, modes)| modes))
    .map(|(base, modes)| {
        let mut resolved = resolved_launch_base(base, tool);
        resolved.modes = modes;
        resolved
    })
    .collect()
}

fn resolved_launch_base(base: ResolvedBase, tool: CliTool) -> ResolvedLaunchBase {
    let selector_value = spec(tool)
        .capabilities
        .account_selector
        .and_then(|selector| {
            crate::session_scanner::accounts::env_assignment_value(&base.command, selector)
        });
    ResolvedLaunchBase {
        base,
        selector_value,
        modes: Vec::new(),
    }
}

/// Resolve each base in order, carrying a forced invalidation forward until
/// one resolution actually consumed it.
///
/// A fail-soft literal answer — daemon absent, unreachable, or an
/// unsupported/unavailable reply — never consumed the force: the cache that
/// answers never heard it, so the next base must still carry it.
fn resolve_bases_threading_force(
    bases: &[&str],
    force: bool,
    mut resolve: impl FnMut(&str, bool) -> (ResolvedBase, bool),
) -> Vec<ResolvedBase> {
    let mut force_pending = force;
    bases
        .iter()
        .map(|base| {
            let (resolved, consumed) = resolve(base, force_pending);
            if consumed {
                force_pending = false;
            }
            resolved
        })
        .collect()
}

#[tauri::command(async)]
pub fn refresh_accounts_usage(
    provider: State<'_, ProviderState>,
    tool: CliTool,
) -> IpcResult<bool> {
    let span = IpcCommandSpan::start("refresh_accounts_usage");
    let result =
        refresh_accounts_usage_impl(provider.inner(), tool).ipc_cmd("refresh_accounts_usage");
    span.finish_result(&result);
    result
}

fn refresh_accounts_usage_impl(provider: &ProviderState, tool: CliTool) -> Result<bool, String> {
    if cfg!(target_os = "windows") {
        let Some(daemon) = provider.daemon.as_ref() else {
            return Ok(false);
        };
        let request = protocol::DaemonRequest::new(
            format!("refresh-usage-{tool}"),
            protocol::method::REFRESH_USAGE,
            protocol::ListAccountsParams { tool },
        );
        return Ok(daemon.send_status_request(&request).is_ok());
    }
    Ok(crate::daemon::usage_poller::refresh(tool))
}

/// Detected accounts, from whichever side of the WSL boundary can see them.
pub(crate) fn accounts_report(provider: &ProviderState, tool: CliTool) -> AccountsResult {
    if cfg!(target_os = "windows") {
        return daemon_accounts_report(provider, tool);
    }
    let mut detected = accounts::detect(tool);
    crate::daemon::usage_poller::attach_usage(tool, &mut detected);
    AccountsResult {
        accounts: detected,
        source: SOURCE_NATIVE.to_string(),
        degraded: false,
        error: None,
        resolved_bases: Vec::new(),
    }
}

#[tauri::command]
pub fn set_project_account(
    db: State<'_, DbState>,
    project_id: String,
    tool: CliTool,
    account_id: Option<String>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("set_project_account");
    let result = set_project_account_impl(db.inner(), &project_id, tool, account_id.as_deref())
        .ipc_cmd("set_project_account");
    span.finish_result(&result);
    result
}

#[tauri::command(async)]
pub fn list_account_relationships(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    tool: CliTool,
) -> IpcResult<AccountRelationshipIndex> {
    let span = IpcCommandSpan::start("list_account_relationships");
    let registry_home = crate::provider::platform_paths::PlatformPaths::tool_home(tool);
    let report = accounts_report(provider.inner(), tool);
    let registry_home_account = registry_home_account_id(&report.accounts, &registry_home);
    let result = account_relationships_impl(
        db.inner(),
        &crate::provider::platform_paths::PlatformPaths::teams_dir(),
        tool,
        registry_home_account.as_deref(),
        &report.accounts,
    )
    .ipc_cmd("list_account_relationships");
    span.finish_result(&result);
    result
}

pub(crate) fn account_relationships_impl(
    db: &DbState,
    teams_dir: &Path,
    tool: CliTool,
    registry_home_account_id: Option<&str>,
    detected_accounts: &[Account],
) -> Result<AccountRelationshipIndex, String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT p.id, p.name, p.path, a.account_id, a.origin, a.updated_at
             FROM project_tool_accounts a
             JOIN projects p ON p.id = a.project_id
             WHERE a.tool = ?1
             ORDER BY p.name COLLATE NOCASE, p.id",
        )
        .sanitize_err()?;
    let rows = statement
        .query_map([tool.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .sanitize_err()?;

    let mut index = AccountRelationshipIndex::default();
    for row in rows {
        let (id, name, path, account_id, origin, updated_at) = row.sanitize_err()?;
        let project = AccountProjectRelationship {
            id,
            name,
            path,
            updated_at,
        };
        let relationships = index.by_account.entry(account_id).or_default();
        match origin.as_str() {
            "pinned" => relationships.pinned_projects.push(project),
            "last_used" => relationships.last_used_projects.push(project),
            _ => {}
        }
    }

    // A team names a project by its path, and a project it names may never
    // have remembered an account: the registered projects are the map, not
    // the ones this tool already has a row for.
    let projects_by_path = queries::list_projects(&conn)
        .sanitize_err()?
        .into_iter()
        .map(|project| {
            (
                crate::provider::path::normalize_project_path(&project.path),
                (project.id, project.name),
            )
        })
        .collect::<HashMap<_, _>>();
    for (account_id, teams) in scan_team_account_relationships(
        teams_dir,
        tool,
        &projects_by_path,
        registry_home_account_id,
        detected_accounts,
    ) {
        index
            .by_account
            .entry(account_id)
            .or_default()
            .teams
            .extend(teams);
    }
    Ok(index)
}

fn registry_home_account_id(accounts: &[Account], registry_home: &Path) -> Option<String> {
    let registry_home =
        crate::provider::path::normalize_project_path(&registry_home.to_string_lossy());
    accounts
        .iter()
        .find(|account| {
            crate::provider::path::normalize_project_path(&account.dir.to_string_lossy())
                == registry_home
        })
        .map(|account| account.id.clone())
}

/// The account a team member is actually on, rather than the one its config
/// asks for.
///
/// The launch authority (`managed_member_account`) applies a requested account
/// only while detection says it is logged in, and otherwise runs the member on
/// the registry home; a running member's `MemberRuntimeRecord.launch_account`
/// records which of those actually happened. Listing a team under an account
/// whose launch it would never get puts the hub's switch action on a
/// relationship that does not exist. Detection that has no opinion about an
/// account is missing evidence, not proof, so the configured id still stands.
#[cfg(feature = "mesh-bridged-backend")]
fn member_launch_account_id(
    teams_dir: &Path,
    team_name: &str,
    member: &crate::coordination::domain::Member,
    default_account_id: Option<&str>,
    detected_accounts: &[Account],
) -> Option<String> {
    if let Ok(runtime) =
        crate::coordination::stores::MemberRuntimeStore::load(teams_dir, team_name, &member.name)
    {
        if runtime.health != crate::coordination::domain::HealthState::SessionDead {
            if let Some(launched) = runtime.launch_account.account_id {
                return Some(launched);
            }
        }
    }
    let requested = member.account_id.as_deref()?;
    let signed_out = detected_accounts
        .iter()
        .any(|account| account.id == requested && !account.identity.logged_in);
    if signed_out {
        return default_account_id.map(str::to_string);
    }
    Some(requested.to_string())
}

#[cfg(feature = "mesh-bridged-backend")]
fn scan_team_account_relationships(
    teams_dir: &Path,
    tool: CliTool,
    projects_by_path: &HashMap<String, (String, String)>,
    default_account_id: Option<&str>,
    detected_accounts: &[Account],
) -> HashMap<String, Vec<AccountTeamRelationship>> {
    use crate::coordination::stores::TeamConfigStore;

    let Ok(team_names) = TeamConfigStore::list(teams_dir) else {
        return HashMap::new();
    };
    let mut by_account = HashMap::<String, Vec<AccountTeamRelationship>>::new();
    for team_name in team_names {
        let Ok(config) = TeamConfigStore::load(teams_dir, &team_name) else {
            continue;
        };
        for member in config
            .members
            .iter()
            .filter(|member| member.cli_tool == tool)
        {
            let account_id = member_launch_account_id(
                teams_dir,
                &team_name,
                member,
                default_account_id,
                detected_accounts,
            )
            .or_else(|| default_account_id.map(str::to_string));
            let Some(account_id) = account_id else {
                continue;
            };
            let account_teams = by_account.entry(account_id).or_default();
            if account_teams.iter().any(|team| team.name == config.name) {
                continue;
            }
            let project_path = member.project_path.to_string_lossy().into_owned();
            let project = projects_by_path.get(&crate::provider::path::normalize_project_path(
                &project_path,
            ));
            account_teams.push(AccountTeamRelationship {
                name: config.name.clone(),
                project_id: project.map(|(id, _)| id.clone()),
                project_name: project.map(|(_, name)| name.clone()),
                project_path: Some(project_path),
            });
        }
    }
    for teams in by_account.values_mut() {
        teams.sort_by(|left, right| left.name.cmp(&right.name));
    }
    by_account
}

#[cfg(not(feature = "mesh-bridged-backend"))]
fn scan_team_account_relationships(
    _teams_dir: &Path,
    _tool: CliTool,
    _projects_by_path: &HashMap<String, (String, String)>,
    _default_account_id: Option<&str>,
    _detected_accounts: &[Account],
) -> HashMap<String, Vec<AccountTeamRelationship>> {
    HashMap::new()
}

#[tauri::command]
pub fn set_global_default_account(
    db: State<'_, DbState>,
    tool: CliTool,
    account_id: Option<String>,
) -> IpcResult<()> {
    let span = IpcCommandSpan::start("set_global_default_account");
    let result = set_global_default_account_impl(db.inner(), tool, account_id.as_deref())
        .ipc_cmd("set_global_default_account");
    span.finish_result(&result);
    result
}

pub(crate) fn set_global_default_account_impl(
    db: &DbState,
    tool: CliTool,
    account_id: Option<&str>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|error| error.to_string())?;
    let mut settings = crate::db::settings_queries::get_all_settings(&conn).sanitize_err()?;
    match account_id.filter(|value| !value.trim().is_empty()) {
        Some(account_id) => {
            settings
                .terminal
                .default_account_ids
                .insert(tool.to_string(), account_id.to_string());
        }
        None => {
            settings
                .terminal
                .default_account_ids
                .remove(&tool.to_string());
        }
    }
    crate::db::settings_queries::save_settings(&conn, &settings).sanitize_err()
}

pub(crate) fn account_directory_plan(default_dir: &Path, label: &str) -> Result<PathBuf, String> {
    let slug = label
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else if character == ' ' || character == '_' || character == '-' {
                '-'
            } else {
                '\0'
            }
        })
        .collect::<String>();
    if slug.contains('\0') {
        return Err(
            "Account names may contain only letters, numbers, spaces, hyphens, and underscores"
                .to_string(),
        );
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        return Err("Enter an account name".to_string());
    }
    let base = default_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The registry account directory has no file name".to_string())?;
    let parent = default_dir
        .parent()
        .ok_or_else(|| "The registry account directory has no parent".to_string())?;
    let parent = parent.to_string_lossy().replace('\\', "/");
    let separator = if parent.ends_with('/') { "" } else { "/" };
    Ok(PathBuf::from(format!("{parent}{separator}{base}-{slug}")))
}

#[tauri::command(async)]
pub fn account_directory_host_path(
    provider: State<'_, ProviderState>,
    path: String,
) -> IpcResult<String> {
    let span = IpcCommandSpan::start("account_directory_host_path");
    let distro = if cfg!(target_os = "windows") {
        provider.wsl_distro.as_deref()
    } else {
        Some("native")
    };
    let result =
        account_directory_host_path_impl(&path, distro).ipc_cmd("account_directory_host_path");
    span.finish_result(&result);
    result
}

fn account_directory_host_path_impl(
    path: &str,
    wsl_distro: Option<&str>,
) -> Result<String, String> {
    if !path.starts_with('/') || wsl_distro == Some("native") {
        return Ok(path.to_string());
    }
    let distro = wsl_distro
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Cannot reveal a WSL account directory without a distro".to_string())?;
    Ok(crate::provider::path::to_windows(path, distro))
}

pub(crate) fn account_login_command(tool: CliTool, config_dir: &Path) -> Result<String, String> {
    let tool_spec = spec(tool);
    let selector = tool_spec.capabilities.account_selector.ok_or_else(|| {
        format!(
            "{} does not support selectable account directories",
            tool_spec.label
        )
    })?;
    let login = tool_spec.account_login_command.ok_or_else(|| {
        format!(
            "{} does not declare an account login command",
            tool_spec.label
        )
    })?;
    Ok(format!(
        "{selector}={} {login}",
        crate::session_scanner::launch::shell_escape(&config_dir.to_string_lossy())
    ))
}

#[tauri::command(async)]
pub fn prepare_account_directory(tool: CliTool, label: String) -> IpcResult<String> {
    let span = IpcCommandSpan::start("prepare_account_directory");
    let result = prepare_account_directory_impl(tool, &label).ipc_cmd("prepare_account_directory");
    span.finish_result(&result);
    result
}

fn prepare_account_directory_impl(tool: CliTool, label: &str) -> Result<String, String> {
    let default_dir = crate::provider::platform_paths::PlatformPaths::tool_home(tool);
    let launch_default = crate::provider::path::to_linux(&default_dir.to_string_lossy())
        .map(PathBuf::from)
        .unwrap_or(default_dir);
    let target = account_directory_plan(&launch_default, label)?;

    #[cfg(target_os = "windows")]
    {
        let dir = crate::session_scanner::launch::shell_escape(&target.to_string_lossy());
        let marker =
            crate::session_scanner::launch::shell_escape(&accounts::pending_account_marker(label));
        let file = accounts::PENDING_ACCOUNT_FILENAME;
        let script = format!("mkdir -p -- {dir} && printf %s {marker} > {dir}/{file}");
        let mut command = crate::daemon::launcher::wsl_command();
        command.args(["-e", "sh", "-c", script.as_str()]);
        let output = crate::process_utils::run_command_with_timeout(
            &mut command,
            ACCOUNT_DIRECTORY_CREATE_TIMEOUT,
            "create WSL account directory",
        )
        .map_err(|error| format!("Failed to create the account directory: {error}"))?;
        if !output.status.success() {
            return Err("Failed to create the account directory in WSL".to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    create_account_directory(&target, label)?;

    Ok(target.to_string_lossy().into_owned())
}

/// Create the directory a sign-in will run in, and say who asked for it.
///
/// The marker is what makes an abandoned sign-in recoverable: detection has no
/// identity to read until the tool writes one, so without it the prepared
/// directory is invisible and the promised signed-out row never appears.
#[cfg(not(target_os = "windows"))]
fn create_account_directory(target: &Path, label: &str) -> Result<(), String> {
    std::fs::create_dir_all(target)
        .map_err(|error| format!("Failed to create the account directory: {error}"))?;
    std::fs::write(
        target.join(accounts::PENDING_ACCOUNT_FILENAME),
        accounts::pending_account_marker(label),
    )
    .map_err(|error| format!("Failed to record the prepared account: {error}"))
}

#[tauri::command(async)]
pub fn launch_account_login(
    db: State<'_, DbState>,
    provider: State<'_, ProviderState>,
    project_id: Option<String>,
    tool: CliTool,
    config_dir: String,
) -> IpcResult<protocol::LaunchSessionResult> {
    let span = IpcCommandSpan::start("launch_account_login");
    let result = launch_account_login_impl(
        db.inner(),
        provider.inner(),
        project_id.as_deref(),
        tool,
        Path::new(&config_dir),
    )
    .ipc_cmd("launch_account_login");
    span.finish_result(&result);
    result
}

fn launch_account_login_impl(
    db: &DbState,
    provider: &ProviderState,
    project_id: Option<&str>,
    tool: CliTool,
    config_dir: &Path,
) -> Result<protocol::LaunchSessionResult, String> {
    validate_account_login_dir(tool, config_dir)?;
    let command = account_login_command(tool, config_dir)?;
    let terminal_settings = crate::commands::terminal_settings::load_terminal_settings(db);
    let project_path = match project_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(project_id) => {
            let path = {
                let conn = db.0.lock().map_err(|error| error.to_string())?;
                queries::get_project(&conn, project_id)
                    .sanitize_err()?
                    .map(|project| project.path)
                    .ok_or_else(|| "Project not found".to_string())?
            };
            crate::provider::path::to_linux(&path).unwrap_or(path)
        }
        None => account_login_working_dir(config_dir)?,
    };
    let (session, window, pane) =
        crate::session_scanner::control::launch_command_in_tmux_with_layout(
            &project_path,
            &terminal_settings.tmux_layout,
            &command,
        )?;
    let _ = crate::terminal::handle_terminal(crate::terminal::TerminalIntent::EnsureOpen {
        distro: provider.wsl_distro.clone(),
        tmux_session: session.clone(),
        emulator: terminal_settings.emulator,
        custom_command: terminal_settings.custom_command,
    });
    Ok(protocol::LaunchSessionResult {
        tmux_session: Some(session),
        tmux_window: window,
        tmux_pane: pane,
        ..Default::default()
    })
}

/// Where a sign-in runs when no project names a working directory.
///
/// Account management is app-global — a user with an empty project list still
/// has accounts to add — so the login falls back to the directory that holds
/// this tool's account directories. It is the one place the sign-in is
/// guaranteed to be able to enter, on every platform the launch can reach.
fn account_login_working_dir(config_dir: &Path) -> Result<String, String> {
    config_dir
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .filter(|parent| !parent.is_empty())
        .ok_or_else(|| "The account directory has no parent to sign in from".to_string())
}

fn validate_account_login_dir(tool: CliTool, config_dir: &Path) -> Result<(), String> {
    let default_dir = crate::provider::platform_paths::PlatformPaths::tool_home(tool);
    let default_dir = crate::provider::path::to_linux(&default_dir.to_string_lossy())
        .map(PathBuf::from)
        .unwrap_or(default_dir);
    validate_account_login_dir_against(&default_dir, config_dir)
}

fn validate_account_login_dir_against(default_dir: &Path, config_dir: &Path) -> Result<(), String> {
    let expected_parent = default_dir
        .parent()
        .ok_or_else(|| "The registry account directory has no parent".to_string())?;
    let expected_name = default_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The registry account directory has no file name".to_string())?;
    let actual_parent = config_dir
        .parent()
        .ok_or_else(|| "The account directory has no parent".to_string())?;
    let actual_name = config_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "The account directory has no file name".to_string())?;
    if actual_parent != expected_parent
        || (actual_name != expected_name && !actual_name.starts_with(&format!("{expected_name}-")))
    {
        return Err(
            "The account directory must be the registry home or one of its named siblings"
                .to_string(),
        );
    }
    Ok(())
}

pub(crate) fn set_project_account_impl(
    db: &DbState,
    project_id: &str,
    tool: CliTool,
    account_id: Option<&str>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let changed = queries::set_project_account(&conn, project_id, &tool.to_string(), account_id)
        .sanitize_err()?;
    if !changed {
        return Err("Project not found".to_string());
    }
    Ok(())
}

/// The newest transcript for one tool and project, read where that tool runs.
pub(crate) fn project_transcript(
    provider: &ProviderState,
    tool: CliTool,
    project_path: &str,
) -> TranscriptLookup {
    if cfg!(target_os = "windows") {
        return daemon_project_transcript_lookup(provider, tool, project_path);
    }
    TranscriptLookup {
        transcript: accounts::newest_project_transcript(
            tool,
            &accounts::transcript_dirs(tool),
            project_path,
        ),
        unavailable: None,
    }
}

/// What the shell that will run this launch makes of its base command.
///
/// Where the config dirs are, the aliases are: in-process on Linux and macOS,
/// in the WSL daemon on Windows. Every failure — no daemon, an older daemon, a
/// shell that never answered — leaves the base exactly as configured.
pub(crate) fn resolve_launch_base(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
) -> ResolvedBase {
    resolve_launch_base_with_force(provider, tool, base, false)
}

/// Resolve the shell bases a managed team launch can render, then carry them
/// across the commands-to-coordination boundary on the existing settings
/// payload. Resolution stays here because coordination may not import commands.
pub(crate) fn apply_team_launch_base_resolutions(
    provider: &ProviderState,
    commands: &mut crate::models::CliCommandSettings,
    tools: impl IntoIterator<Item = CliTool>,
) {
    let tools = tools.into_iter().collect::<Vec<_>>();
    apply_team_account_selector_dirs(commands, tools.iter().copied());
    let probe = (!cfg!(target_os = "windows"))
        .then(crate::session_scanner::launch_base::ShellAliasProbe::for_pane);
    apply_team_launch_base_resolutions_with(commands, tools, |base, tool| {
        resolve_launch_base_with_force_and_probe_tracked(
            provider,
            tool,
            base,
            false,
            probe.as_ref(),
        )
    });
}

pub(crate) fn apply_team_account_selector_dirs(
    commands: &mut crate::models::CliCommandSettings,
    tools: impl IntoIterator<Item = CliTool>,
) {
    apply_team_account_selector_dirs_with(commands, tools, |tool| {
        crate::provider::platform_paths::PlatformPaths::tool_home(tool)
    });
}

/// Carry a credential-free detection snapshot into managed launch rendering.
/// The member keeps only an account id; its machine-local directory is looked
/// up again for every operation that can start a pane.
pub(crate) fn apply_team_managed_accounts(
    commands: &mut crate::models::CliCommandSettings,
    tools: impl IntoIterator<Item = CliTool>,
) {
    for tool in tools {
        if crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .account_selector
            .is_none()
        {
            continue;
        }
        let accounts = accounts::detect(tool)
            .into_iter()
            .map(|account| crate::models::ManagedLaunchAccount {
                id: account.id,
                label: account.identity.label,
                dir: account.dir,
                logged_in: account.identity.logged_in,
                is_default: account.is_default,
            })
            .collect();
        commands.managed_accounts.insert(tool, accounts);
    }
}

pub(crate) fn apply_team_account_selector_dirs_with(
    commands: &mut crate::models::CliCommandSettings,
    tools: impl IntoIterator<Item = CliTool>,
    mut tool_home: impl FnMut(CliTool) -> std::path::PathBuf,
) {
    for tool in tools {
        if let Some(selector) = crate::session_scanner::cli_tool::spec(tool)
            .capabilities
            .account_selector
        {
            commands
                .account_selector_dirs
                .entry(selector.to_string())
                .or_insert_with(|| tool_home(tool));
        }
    }
}

pub(crate) fn apply_team_launch_base_resolutions_with(
    commands: &mut crate::models::CliCommandSettings,
    tools: impl IntoIterator<Item = CliTool>,
    mut resolve: impl FnMut(&str, CliTool) -> (ResolvedBase, bool),
) {
    let mut seen = std::collections::HashSet::new();
    let mut requested = Vec::new();
    for tool in tools.into_iter().filter(|tool| seen.insert(*tool)) {
        for mode in [protocol::LaunchMode::Fresh, protocol::LaunchMode::Resume] {
            requested.push((
                tool,
                mode,
                crate::session_scanner::launch::base_command(commands, tool, mode).to_string(),
            ));
        }
    }

    for (tool, mode, base) in requested {
        let (resolved, answered) = resolve(&base, tool);
        if answered {
            commands.resolved_bases.insert((tool, mode), resolved);
        }
    }
}

fn resolve_launch_base_with_force(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
) -> ResolvedBase {
    resolve_launch_base_with_force_tracked(provider, tool, base, force).0
}

/// Resolves one base and says whether a forced invalidation was consumed —
/// which it never is by a fail-soft literal answer.
fn resolve_launch_base_with_force_tracked(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
) -> (ResolvedBase, bool) {
    let probe = (!cfg!(target_os = "windows"))
        .then(crate::session_scanner::launch_base::ShellAliasProbe::for_pane);
    resolve_launch_base_with_force_and_probe_tracked(provider, tool, base, force, probe.as_ref())
}

fn resolve_launch_base_with_force_and_probe_tracked(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
    probe: Option<&crate::session_scanner::launch_base::ShellAliasProbe>,
) -> (ResolvedBase, bool) {
    #[cfg(test)]
    if let Some(resolved) = test_resolution_probe(base) {
        return (resolved, true);
    }
    if cfg!(target_os = "windows") {
        return daemon_resolve_launch_base_tracked(provider, tool, base, force);
    }
    (
        launch_base::resolve_base_command_cached(
            base,
            tool,
            probe.expect("non-Windows launch resolution has a pane-shell probe"),
        ),
        true,
    )
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct TestResolutionProbe {
    delay: Duration,
    calls: usize,
}

#[cfg(test)]
thread_local! {
    static TEST_RESOLUTION_PROBE: std::cell::RefCell<Option<TestResolutionProbe>> = const {
        std::cell::RefCell::new(None)
    };
}

#[cfg(test)]
pub(crate) struct TestResolutionProbeGuard;

#[cfg(test)]
impl TestResolutionProbeGuard {
    pub(crate) fn calls(&self) -> usize {
        TEST_RESOLUTION_PROBE.with(|probe| probe.borrow().map_or(0, |probe| probe.calls))
    }
}

#[cfg(test)]
impl Drop for TestResolutionProbeGuard {
    fn drop(&mut self) {
        TEST_RESOLUTION_PROBE.with(|probe| *probe.borrow_mut() = None);
    }
}

/// Install a counting, delayed stand-in for daemon/shell resolution on this test thread.
#[cfg(test)]
pub(crate) fn install_test_resolution_probe(delay: Duration) -> TestResolutionProbeGuard {
    TEST_RESOLUTION_PROBE.with(|probe| {
        *probe.borrow_mut() = Some(TestResolutionProbe { delay, calls: 0 });
    });
    TestResolutionProbeGuard
}

#[cfg(test)]
fn test_resolution_probe(base: &str) -> Option<ResolvedBase> {
    let delay = TEST_RESOLUTION_PROBE.with(|probe| {
        let mut probe = probe.borrow_mut();
        let probe = probe.as_mut()?;
        probe.calls += 1;
        Some(probe.delay)
    })?;
    std::thread::sleep(delay);
    Some(literal_base(base))
}

fn daemon_resolve_launch_base_tracked(
    provider: &ProviderState,
    tool: CliTool,
    base: &str,
    force: bool,
) -> (ResolvedBase, bool) {
    let Some(daemon) = provider.daemon.as_ref() else {
        return (literal_base(base), false);
    };
    if !daemon.is_connected() {
        if !daemon.try_reconnect() {
            return (literal_base(base), false);
        }
        #[cfg(feature = "mesh-bridged-backend")]
        if let Err(error) =
            crate::commands::settings::repush_cached_launch_settings_to_daemon(daemon)
        {
            tracing::warn!(error = %error, "Failed to repush launch settings after launch-base reconnect");
        }
    }

    let request = protocol::DaemonRequest::new(
        format!("resolve-launch-base-{tool}"),
        protocol::method::RESOLVE_LAUNCH_BASE,
        protocol::ResolveLaunchBaseParams {
            tool,
            base: base.to_string(),
            force,
        },
    );
    let answer = daemon_answer(
        daemon.send_status_request_within(&request, RESOLVE_LAUNCH_BASE_TIMEOUT),
        "the resolved launch base",
    );
    let answered = matches!(answer, DaemonAnswer::Value(_));
    (resolved_base_from(answer, base), answered)
}

fn resolved_base_from(answer: DaemonAnswer<ResolvedBase>, base: &str) -> ResolvedBase {
    match answer {
        DaemonAnswer::Value(resolved) => resolved,
        DaemonAnswer::Unsupported | DaemonAnswer::Unavailable(_) => literal_base(base),
    }
}

/// The base command as configured: no expansion, nothing claimed about it.
fn literal_base(base: &str) -> ResolvedBase {
    ResolvedBase {
        command: base.to_string(),
        expansions: Vec::new(),
        opaque_head: None,
    }
}

fn daemon_accounts_report(provider: &ProviderState, tool: CliTool) -> AccountsResult {
    daemon_accounts_report_from(daemon_accounts(provider, tool))
}

fn daemon_accounts_report_from(answer: DaemonAnswer<protocol::AccountsResult>) -> AccountsResult {
    let (accounts, degraded, error) = match answer {
        DaemonAnswer::Value(result) => (result.accounts, result.degraded, result.error),
        DaemonAnswer::Unsupported => (Vec::new(), false, None),
        DaemonAnswer::Unavailable(error) => (Vec::new(), true, Some(error)),
    };
    AccountsResult {
        accounts,
        source: SOURCE_DAEMON.to_string(),
        degraded,
        error,
        resolved_bases: Vec::new(),
    }
}

fn daemon_project_transcript_lookup(
    provider: &ProviderState,
    tool: CliTool,
    project_path: &str,
) -> TranscriptLookup {
    let Some(daemon) = provider.daemon.as_ref() else {
        return TranscriptLookup {
            transcript: None,
            unavailable: Some("The WSL daemon is not running".to_string()),
        };
    };
    if !daemon.is_connected() {
        if !daemon.try_reconnect() {
            return TranscriptLookup {
                transcript: None,
                unavailable: Some("The WSL daemon is not reachable".to_string()),
            };
        }
        #[cfg(feature = "mesh-bridged-backend")]
        if let Err(error) =
            crate::commands::settings::repush_cached_launch_settings_to_daemon(daemon)
        {
            tracing::warn!(error = %error, "Failed to repush launch settings after transcript reconnect");
        }
    }

    let request = protocol::DaemonRequest::new(
        format!("project-transcript-{tool}"),
        protocol::method::PROJECT_TRANSCRIPT,
        protocol::ProjectTranscriptParams {
            tool,
            project: project_path.to_string(),
        },
    );
    transcript_lookup_from(daemon_answer(
        daemon.send_status_request(&request),
        "Project transcript",
    ))
}

fn transcript_lookup_from(
    answer: DaemonAnswer<protocol::ProjectTranscriptResult>,
) -> TranscriptLookup {
    match answer {
        DaemonAnswer::Value(result) => TranscriptLookup {
            transcript: result.transcript.map(PathBuf::from),
            unavailable: None,
        },
        // A daemon that cannot resolve transcripts never could: the resume
        // keeps the project's own choice, exactly as it did before.
        DaemonAnswer::Unsupported => TranscriptLookup::default(),
        DaemonAnswer::Unavailable(error) => TranscriptLookup {
            transcript: None,
            unavailable: Some(error),
        },
    }
}

fn daemon_accounts(
    provider: &ProviderState,
    tool: CliTool,
) -> DaemonAnswer<protocol::AccountsResult> {
    let Some(daemon) = provider.daemon.as_ref() else {
        return DaemonAnswer::Unavailable("The WSL daemon is not running".to_string());
    };
    if !daemon.is_connected() {
        if !daemon.try_reconnect() {
            return DaemonAnswer::Unavailable("The WSL daemon is not reachable".to_string());
        }
        #[cfg(feature = "mesh-bridged-backend")]
        if let Err(error) =
            crate::commands::settings::repush_cached_launch_settings_to_daemon(daemon)
        {
            tracing::warn!(error = %error, "Failed to repush launch settings after accounts reconnect");
        }
    }

    let request = protocol::DaemonRequest::new(
        format!("list-accounts-{tool}"),
        protocol::method::LIST_ACCOUNTS,
        protocol::ListAccountsParams { tool },
    );
    daemon_answer(daemon.send_status_request(&request), "tool accounts")
}

/// Read one daemon response for what it means, not just for what it carries.
fn daemon_answer<T: DeserializeOwned>(
    outcome: Result<protocol::DaemonResponse, AppError>,
    what: &str,
) -> DaemonAnswer<T> {
    match outcome {
        Ok(response) if response.is_ok() => match response.result {
            Some(value) => match serde_json::from_value::<T>(value) {
                Ok(decoded) => DaemonAnswer::Value(decoded),
                Err(error) => {
                    tracing::warn!(error = %error, what, "Failed to decode daemon answer");
                    DaemonAnswer::Unavailable(sanitize_error(&format!(
                        "The daemon sent {what} this build cannot read: {error}"
                    )))
                }
            },
            None => DaemonAnswer::Unavailable(format!("The daemon returned no {what}")),
        },
        Ok(response) => {
            let error = response.error.unwrap_or(protocol::DaemonError {
                code: "DAEMON_ERROR".to_string(),
                message: format!("The daemon could not report {what}"),
            });
            if error.code == UNKNOWN_METHOD {
                tracing::debug!(
                    what,
                    "Daemon predates this method; treating the empty answer as honest"
                );
                return DaemonAnswer::Unsupported;
            }
            tracing::warn!(code = %error.code, message = %error.message, what, "Daemon error");
            DaemonAnswer::Unavailable(sanitize_error(&error.message))
        }
        Err(error) => {
            tracing::warn!(error = %error, what, "Daemon unreachable");
            DaemonAnswer::Unavailable(sanitize_error(&error.to_string()))
        }
    }
}
