use crate::commands::projects::DbState;
use crate::coordination::compact_hook::{
    ensure_codex_compact_hook_installed_at, remove_codex_compact_hook_at,
};
use crate::coordination::errors::CoordinationError;
#[cfg(test)]
use crate::models::CliCommandSettings;
use crate::models::{CliVersions, TerminalSettings};
use crate::provider::platform_paths::PlatformPaths;
use crate::session_scanner::cli_tool::{all, CliTool, CompactionDelivery, SessionRoot};

pub fn load_terminal_settings(db: &DbState) -> TerminalSettings {
    let conn = match db.0.lock() {
        Ok(conn) => conn,
        Err(e) => {
            tracing::warn!(error = %e, "Settings DB lock poisoned, using default terminal settings");
            return TerminalSettings::default();
        }
    };
    match crate::db::settings_queries::get_all_settings(&conn) {
        Ok(settings) => settings.terminal,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to load settings, using default terminal settings");
            TerminalSettings::default()
        }
    }
}

pub(crate) fn apply_managed_codex_launch_inputs(
    cli_commands: &mut crate::models::CliCommandSettings,
    has_managed_codex: bool,
    codex_bypass_hook_trust: bool,
) {
    let notify_supported = CliVersions::current().codex_notify_supported;
    let daemon_executable = PlatformPaths::daemon_binary_path();
    let codex_config_path = PlatformPaths::codex_dir().join("config.toml");
    let user_notify_configured = match codex_config_has_notify(&codex_config_path) {
        Ok(configured) => configured,
        Err(error) => {
            tracing::warn!(
                path = %codex_config_path.display(),
                error,
                "Could not inspect Codex config for an existing notifier"
            );
            false
        }
    };
    apply_managed_codex_launch_inputs_with_support(
        cli_commands,
        has_managed_codex,
        codex_bypass_hook_trust,
        notify_supported,
        user_notify_configured,
        &daemon_executable,
    );
    apply_managed_account_selector(cli_commands, has_managed_codex, PlatformPaths::codex_dir());
    if has_managed_codex && notify_supported && user_notify_configured {
        tracing::info!(
            path = %codex_config_path.display(),
            "Codex native notify preserved the user's config.toml notifier"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "found".to_string(),
            serde_json::Value::String("config.toml notify".to_string()),
        );
        fields.insert(
            "path".to_string(),
            serde_json::Value::String(codex_config_path.display().to_string()),
        );
        crate::commands::logging::emit_global(
            "info",
            "coordination",
            "launch.notify.ignored",
            Some("Preserved the user-configured Codex notifier".to_string()),
            fields,
        );
    } else if has_managed_codex
        && notify_supported
        && !codex_notify_executable_available(&daemon_executable)
    {
        tracing::warn!(
            path = %daemon_executable.display(),
            "Codex native notify skipped because the daemon executable is missing"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "path".to_string(),
            serde_json::Value::String(daemon_executable.display().to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "codex.notify.executable_missing",
            Some("Managed Codex notify requires the installed taurhaus daemon".to_string()),
            fields,
        );
    }
}

fn apply_managed_account_selector(
    cli_commands: &mut crate::models::CliCommandSettings,
    enabled: bool,
    dir: std::path::PathBuf,
) {
    let Some(selector) = all()
        .iter()
        .find(|entry| entry.capabilities.managed_home)
        .and_then(|entry| entry.capabilities.account_selector)
    else {
        return;
    };
    if enabled {
        cli_commands
            .account_selector_dirs
            .insert(selector.to_string(), dir);
    } else {
        cli_commands.account_selector_dirs.remove(selector);
    }
}

fn apply_managed_codex_launch_inputs_with_support(
    cli_commands: &mut crate::models::CliCommandSettings,
    has_managed_codex: bool,
    codex_bypass_hook_trust: bool,
    notify_supported: bool,
    user_notify_configured: bool,
    daemon_executable: &std::path::Path,
) {
    cli_commands.codex_bypass_hook_trust = codex_bypass_hook_trust;
    cli_commands.codex_notify_executable = (has_managed_codex
        && notify_supported
        && !user_notify_configured
        && codex_notify_executable_available(daemon_executable))
    .then(|| daemon_executable.to_path_buf());
}

fn codex_config_has_notify(path: &std::path::Path) -> Result<bool, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to read Codex config '{}': {error}",
                path.display()
            ));
        }
    };
    let config = raw
        .parse::<toml::Table>()
        .map_err(|error| format!("failed to parse Codex config '{}': {error}", path.display()))?;
    Ok(config.contains_key("notify"))
}

#[cfg(not(target_os = "windows"))]
fn codex_notify_executable_available(path: &std::path::Path) -> bool {
    path.is_file()
}

#[cfg(target_os = "windows")]
fn codex_notify_executable_available(path: &std::path::Path) -> bool {
    let Some(distro) = crate::coordination::mesh_cli::resolve_wsl_distro_for_coordination(None)
    else {
        return false;
    };
    let Some(linux_path) = path.to_str() else {
        return false;
    };
    std::path::Path::new(&crate::provider::path::linux_to_wsl_unc(
        linux_path, &distro,
    ))
    .is_file()
}

#[cfg(test)]
pub fn load_cli_commands(db: &DbState) -> CliCommandSettings {
    load_terminal_settings(db).cli_commands
}

pub(crate) fn reconcile_codex_hook_at(
    codex_home: &std::path::Path,
    has_managed_codex: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    reconcile_codex_hook_at_with_support(
        codex_home,
        has_managed_codex,
        CliVersions::current().codex_compaction_hooks_support(),
        taurhaus_exe,
    )
}

pub(crate) fn reconcile_codex_hook_at_with_support(
    codex_home: &std::path::Path,
    has_managed_codex: bool,
    hooks_supported: Option<bool>,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    match (has_managed_codex, hooks_supported) {
        (true, Some(true)) => ensure_codex_compact_hook_installed_at(codex_home, taurhaus_exe),
        (_, Some(_)) => remove_codex_compact_hook_at(codex_home),
        (_, None) => Ok(false),
    }
}

pub(crate) fn reconcile_codex_hook(has_managed_codex: bool) -> Result<bool, CoordinationError> {
    let hooks_support = CliVersions::current().codex_compaction_hooks_support();
    let executable = compact_hook_executable()?;
    let changed =
        reconcile_codex_hook_at(&PlatformPaths::codex_dir(), has_managed_codex, &executable)?;
    if has_managed_codex && hooks_support == Some(false) {
        log_codex_hook_unsupported_once();
    } else if has_managed_codex && hooks_support.is_none() {
        tracing::warn!(
            "Codex compact hook reconciliation skipped because the CLI version could not be resolved"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("codex".to_string()),
        );
        fields.insert(
            "minimum_version".to_string(),
            serde_json::Value::String("0.147.0".to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "compaction.codex_hook.version_unknown",
            Some(
                "Left the Codex compact hook unchanged because the CLI version was unavailable"
                    .to_string(),
            ),
            fields,
        );
    }
    if changed {
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("codex".to_string()),
        );
        fields.insert(
            "installed".to_string(),
            serde_json::Value::Bool(
                crate::coordination::compact_hook::codex_compact_hook_is_installed(),
            ),
        );
        fields.insert("changed".to_string(), serde_json::Value::Bool(true));
        crate::commands::logging::emit_global(
            "info",
            "coordination",
            "compaction.codex_hook.reconciled",
            Some("Reconciled the managed Codex compact hook".to_string()),
            fields,
        );
    }
    Ok(changed)
}

/// One line per run: startup and every managed launch reconcile the same
/// unsupported installation, and repeats do not add operational information.
fn log_codex_hook_unsupported_once() {
    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        tracing::warn!(
            codex_version = ?CliVersions::current().codex,
            "Codex compact hook skipped because the installed CLI predates 0.147"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("codex".to_string()),
        );
        fields.insert(
            "version".to_string(),
            CliVersions::current()
                .codex
                .clone()
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        );
        fields.insert(
            "minimum_version".to_string(),
            serde_json::Value::String("0.147.0".to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "compaction.codex_hook.unsupported",
            Some("Codex compact hook requires CLI version 0.147.0 or newer".to_string()),
            fields,
        );
    });
}

pub(crate) fn reconcile_agy_hooks(enabled: bool) -> Result<bool, CoordinationError> {
    reconcile_agy_hooks_at(
        &PlatformPaths::agy_dir(),
        enabled,
        CliVersions::current().agy_hooks_support(),
        &PlatformPaths::daemon_binary_path(),
    )
}

/// Reconcile the Antigravity activity hooks against the setting and the CLI
/// version gate. An unresolved version is not proof of an unsupported CLI, so
/// it leaves whatever is installed alone instead of disabling a live session's
/// idle edge.
pub(crate) fn reconcile_agy_hooks_at(
    agy_root: &std::path::Path,
    enabled: bool,
    hooks_support: Option<bool>,
    daemon_executable: &std::path::Path,
) -> Result<bool, CoordinationError> {
    if enabled && hooks_support != Some(true) {
        log_agy_hooks_gate_once(hooks_support);
    }
    match (enabled, hooks_support) {
        (true, Some(true)) => {
            crate::coordination::agy_hooks_installer::ensure_agy_hooks_installed_at(
                agy_root,
                daemon_executable,
            )
        }
        (true, None) => Ok(false),
        _ => crate::coordination::agy_hooks_installer::remove_agy_hooks_at(agy_root),
    }
}

/// One line per run: the gate is re-evaluated on every startup and every
/// settings save, and none of those repeats carry new information.
fn log_agy_hooks_gate_once(hooks_support: Option<bool>) {
    static LOGGED: std::sync::Once = std::sync::Once::new();
    LOGGED.call_once(|| {
        let (reason, message) = match hooks_support {
            Some(false) => (
                "unsupported_version",
                "Antigravity activity hooks require agy 1.1.10 or newer",
            ),
            _ => (
                "version_unknown",
                "Left the Antigravity activity hooks unchanged because the agy version was unavailable",
            ),
        };
        tracing::warn!(
            reason,
            agy_version = ?CliVersions::current().agy,
            minimum_version = "1.1.10",
            "Antigravity activity hooks are gated on the CLI version"
        );
        let mut fields = serde_json::Map::new();
        fields.insert(
            "tool".to_string(),
            serde_json::Value::String("agy".to_string()),
        );
        fields.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
        fields.insert(
            "minimum_version".to_string(),
            serde_json::Value::String("1.1.10".to_string()),
        );
        crate::commands::logging::emit_global(
            "warn",
            "coordination",
            "agy.hooks.degraded",
            Some(message.to_string()),
            fields,
        );
    });
}

/// Reconcile the one global grok hook against the current roster.
///
/// grok registers hooks per home, not per session, so the hook has to appear as
/// soon as the first managed grok member exists and go away once the last one
/// does — every roster mutation calls this, not just startup and a Settings
/// save. A discovery failure is reported rather than answered with "no members":
/// neither an unreadable teams directory nor one team's unreadable config is
/// proof the last grok member is gone, and uninstalling on either would
/// silently disable reinjection for a live session.
pub(crate) fn reconcile_grok_hooks_for_roots(
    teams_roots: &[std::path::PathBuf],
    enabled: bool,
) -> Result<bool, CoordinationError> {
    reconcile_grok_hooks_for_roots_at(
        teams_roots,
        &PlatformPaths::grok_dir(),
        enabled,
        &compact_hook_executable()?,
    )
}

pub(crate) fn reconcile_grok_hooks_for_roots_at(
    teams_roots: &[std::path::PathBuf],
    grok_home: &std::path::Path,
    enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let mut has_managed_grok = false;
    for teams_dir in teams_roots {
        has_managed_grok |= crate::coordination::compact_hook::any_managed_grok_member(teams_dir)?;
    }
    reconcile_grok_hooks_at(grok_home, enabled, has_managed_grok, taurhaus_exe)
}

pub(crate) fn reconcile_grok_hooks_for_roster_at(
    teams_dir: &std::path::Path,
    grok_home: &std::path::Path,
    enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let has_managed_grok = crate::coordination::compact_hook::any_managed_grok_member(teams_dir)?;
    reconcile_grok_hooks_at(grok_home, enabled, has_managed_grok, taurhaus_exe)
}

pub(crate) fn reconcile_grok_hooks_at(
    grok_home: &std::path::Path,
    enabled: bool,
    has_managed_grok: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    if enabled && has_managed_grok {
        crate::coordination::compact_hook::ensure_grok_compact_hook_installed_at(
            grok_home,
            taurhaus_exe,
        )
    } else {
        crate::coordination::compact_hook::remove_grok_compact_hook_at(grok_home)
    }
}

fn resolved_managed_home(
    cli_commands: &crate::models::CliCommandSettings,
    cli_tool: CliTool,
    account_id: Option<&str>,
) -> Option<std::path::PathBuf> {
    let accounts = cli_commands.managed_accounts.get(&cli_tool);
    let selected = account_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|requested| {
            accounts
                .into_iter()
                .flatten()
                .find(|account| account.id == requested && account.logged_in)
        });
    if let Some(selected) = selected {
        return Some(selected.dir.clone());
    }
    let default_detected = accounts
        .into_iter()
        .flatten()
        .find(|account| account.is_default && account.logged_in)
        .map(|account| account.dir.clone());
    let selector = crate::session_scanner::cli_tool::spec(cli_tool)
        .capabilities
        .account_selector;
    default_detected.or_else(|| {
        selector.and_then(|selector| cli_commands.account_selector_dirs.get(selector).cloned())
    })
}

/// What one launched tool's hook reconciliation has to know: the account homes
/// a member still launches from, and whether some member could not be resolved
/// to one at all. An unresolved member suppresses removal for that tool, the
/// same conservative answer `managed_home_needed_after_switch` gives — a
/// momentary detection gap is missing evidence, not proof that an installed
/// hook is obsolete.
#[derive(Default)]
struct ManagedHookHomes {
    needed: std::collections::BTreeSet<std::path::PathBuf>,
    unresolved_member: bool,
}

impl ManagedHookHomes {
    fn record(&mut self, home: Option<std::path::PathBuf>) {
        match home {
            Some(home) => {
                self.needed.insert(home);
            }
            None => self.unresolved_member = true,
        }
    }
}

fn hook_reconciled_tool(cli_tool: CliTool) -> bool {
    let capabilities = crate::session_scanner::cli_tool::spec(cli_tool).capabilities;
    capabilities.compaction_hook && capabilities.session_root != SessionRoot::AppManagedClaudeDir
}

/// Every account home taurhaus may have written this tool's hook into: the
/// detected accounts plus the selector directory `resolved_managed_home` falls
/// back to. Reconciliation walks all of them so a home the roster left behind
/// gets the hook taken away, not just the homes that still need it.
fn known_managed_homes(
    cli_commands: &crate::models::CliCommandSettings,
    cli_tool: CliTool,
) -> std::collections::BTreeSet<std::path::PathBuf> {
    let mut homes = cli_commands
        .managed_accounts
        .get(&cli_tool)
        .into_iter()
        .flatten()
        .map(|account| account.dir.clone())
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(dir) = crate::session_scanner::cli_tool::spec(cli_tool)
        .capabilities
        .account_selector
        .and_then(|selector| cli_commands.account_selector_dirs.get(selector))
    {
        homes.insert(dir.clone());
    }
    homes
}

/// The account home a live runtime record says this member actually launched
/// from. `MemberRuntimeRecord.launch_account` is the launch authority: config
/// plus current detection only describes where the *next* launch would go, and
/// re-deriving a running member from them reassigns it the moment its account
/// stops reporting `logged_in`. `Some(None)` means a live record names an
/// account detection cannot place — missing evidence, which suppresses removal
/// exactly like an unresolvable configured member.
fn live_launch_home(
    teams_dir: &std::path::Path,
    team_name: &str,
    member: &crate::coordination::domain::Member,
    cli_commands: &crate::models::CliCommandSettings,
) -> Option<Option<std::path::PathBuf>> {
    let runtime =
        crate::coordination::stores::MemberRuntimeStore::load(teams_dir, team_name, &member.name)
            .ok()?;
    if runtime.health == crate::coordination::domain::HealthState::SessionDead {
        return None;
    }
    // A LIVE session is always the authority over config. When its launch
    // account cannot be named (a fallback launch recorded account_id: None),
    // the session is running on SOME home we cannot place — report it as
    // live-unresolved so hook removal is suppressed for this tool, instead of
    // falling back to the configured account and letting reconciliation
    // remove the hook of the home actually in use.
    let Some(account_id) = runtime.launch_account.account_id else {
        return Some(None);
    };
    Some(
        cli_commands
            .managed_accounts
            .get(&member.cli_tool)
            .into_iter()
            .flatten()
            .find(|account| account.id == account_id)
            .map(|account| account.dir.clone()),
    )
}

fn roster_member_hook_home(
    teams_dir: &std::path::Path,
    team_name: &str,
    member: &crate::coordination::domain::Member,
    cli_commands: &crate::models::CliCommandSettings,
) -> Option<std::path::PathBuf> {
    match live_launch_home(teams_dir, team_name, member, cli_commands) {
        Some(live) => live,
        None => resolved_managed_home(cli_commands, member.cli_tool, member.account_id.as_deref()),
    }
}

fn collect_managed_hook_homes_for_launch(
    teams_dir: &std::path::Path,
    launch_members: &[(CliTool, Option<String>)],
    cli_commands: &crate::models::CliCommandSettings,
) -> Result<std::collections::HashMap<CliTool, ManagedHookHomes>, CoordinationError> {
    let launched_tools = launch_members
        .iter()
        .map(|(tool, _)| *tool)
        .collect::<std::collections::HashSet<_>>();
    let mut homes = std::collections::HashMap::<CliTool, ManagedHookHomes>::new();
    for (tool, account_id) in launch_members {
        if hook_reconciled_tool(*tool) {
            homes
                .entry(*tool)
                .or_default()
                .record(resolved_managed_home(
                    cli_commands,
                    *tool,
                    account_id.as_deref(),
                ));
        }
    }
    for team_name in crate::coordination::stores::TeamConfigStore::list(teams_dir)? {
        let config = crate::coordination::stores::TeamConfigStore::load(teams_dir, &team_name)?;
        for member in config.members {
            if !launched_tools.contains(&member.cli_tool) {
                continue;
            }
            if hook_reconciled_tool(member.cli_tool) {
                homes
                    .entry(member.cli_tool)
                    .or_default()
                    .record(roster_member_hook_home(
                        teams_dir,
                        &team_name,
                        &member,
                        cli_commands,
                    ));
            }
        }
    }
    Ok(homes)
}

pub(crate) fn reconcile_managed_account_hooks_for_launch(
    teams_dir: &std::path::Path,
    launch_members: &[(CliTool, Option<String>)],
    cli_commands: &crate::models::CliCommandSettings,
) -> bool {
    match reconcile_managed_account_hooks_for_launch_at(
        teams_dir,
        launch_members,
        cli_commands,
        CliVersions::current().codex_compaction_hooks_support(),
        cli_commands.grok_hooks_enabled.unwrap_or(true),
        &match compact_hook_executable() {
            Ok(executable) => executable,
            Err(error) => {
                log_managed_account_hook_degraded(
                    &error,
                    "Managed launch continued without compact-hook trust",
                );
                return false;
            }
        },
    ) {
        Ok(trusted) => trusted,
        Err(error) => {
            log_managed_account_hook_degraded(
                &error,
                "Managed launch continued without compact-hook trust",
            );
            false
        }
    }
}

/// Every hook-capable tool's account homes, reconciled against the whole
/// roster instead of one launch.
///
/// The launch reconciler is the only other place that sees account-scoped
/// homes, and a launch is exactly what never happens again once the last
/// member using one is gone: disbanding the only team on `~/.codex-work` left
/// the taurhaus hook in that home forever. Removal only — installing stays
/// with the launch that knows which account it is about to use.
pub(crate) fn reconcile_managed_account_hooks_for_roots(
    teams_roots: &[std::path::PathBuf],
    grok_enabled: bool,
) -> bool {
    let tools = crate::session_scanner::cli_tool::all()
        .iter()
        .map(|spec| spec.tool)
        .filter(|tool| hook_reconciled_tool(*tool))
        .collect::<Vec<_>>();
    let mut cli_commands = crate::models::CliCommandSettings::default();
    crate::commands::accounts::apply_team_account_selector_dirs(
        &mut cli_commands,
        tools.iter().copied(),
    );
    crate::commands::accounts::apply_team_managed_accounts(&mut cli_commands, tools);
    let executable = match compact_hook_executable() {
        Ok(executable) => executable,
        Err(error) => {
            log_managed_account_hook_degraded(&error, "Account homes were left unreconciled");
            return false;
        }
    };
    match reconcile_managed_account_hooks_for_roots_at(
        teams_roots,
        &cli_commands,
        CliVersions::current().codex_compaction_hooks_support(),
        grok_enabled,
        &executable,
    ) {
        Ok(changed) => changed,
        Err(error) => {
            log_managed_account_hook_degraded(&error, "Account homes were left unreconciled");
            false
        }
    }
}

fn collect_managed_hook_homes_for_roster(
    teams_dir: &std::path::Path,
    cli_commands: &crate::models::CliCommandSettings,
) -> Result<std::collections::HashMap<CliTool, ManagedHookHomes>, CoordinationError> {
    // Every hook-capable tool gets an entry even when no member runs it: a
    // roster with no Codex member left is the case whose homes must be swept.
    let mut homes = crate::session_scanner::cli_tool::all()
        .iter()
        .map(|spec| spec.tool)
        .filter(|tool| hook_reconciled_tool(*tool))
        .map(|tool| (tool, ManagedHookHomes::default()))
        .collect::<std::collections::HashMap<_, _>>();
    for team_name in crate::coordination::stores::TeamConfigStore::list(teams_dir)? {
        let config = crate::coordination::stores::TeamConfigStore::load(teams_dir, &team_name)?;
        for member in config.members {
            if !hook_reconciled_tool(member.cli_tool) {
                continue;
            }
            homes
                .entry(member.cli_tool)
                .or_default()
                .record(roster_member_hook_home(
                    teams_dir,
                    &team_name,
                    &member,
                    cli_commands,
                ));
        }
    }
    Ok(homes)
}

fn collect_managed_hook_homes_for_roots(
    teams_roots: &[std::path::PathBuf],
    cli_commands: &crate::models::CliCommandSettings,
) -> Result<std::collections::HashMap<CliTool, ManagedHookHomes>, CoordinationError> {
    let mut combined = crate::session_scanner::cli_tool::all()
        .iter()
        .map(|spec| spec.tool)
        .filter(|tool| hook_reconciled_tool(*tool))
        .map(|tool| (tool, ManagedHookHomes::default()))
        .collect::<std::collections::HashMap<_, _>>();
    for teams_dir in teams_roots {
        for (tool, homes) in collect_managed_hook_homes_for_roster(teams_dir, cli_commands)? {
            let aggregate = combined.entry(tool).or_default();
            aggregate.needed.extend(homes.needed);
            aggregate.unresolved_member |= homes.unresolved_member;
        }
    }
    Ok(combined)
}

fn reconcile_managed_account_hooks_for_roster_at(
    teams_dir: &std::path::Path,
    cli_commands: &crate::models::CliCommandSettings,
    codex_hooks_supported: Option<bool>,
    grok_enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let homes = collect_managed_hook_homes_for_roster(teams_dir, cli_commands)?;
    reconcile_unused_managed_hook_homes(
        homes,
        cli_commands,
        codex_hooks_supported,
        grok_enabled,
        taurhaus_exe,
    )
}

fn reconcile_unused_managed_hook_homes(
    homes: std::collections::HashMap<CliTool, ManagedHookHomes>,
    cli_commands: &crate::models::CliCommandSettings,
    codex_hooks_supported: Option<bool>,
    grok_enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let mut changed = false;
    for (tool, tool_homes) in &homes {
        if tool_homes.unresolved_member {
            continue;
        }
        let delivery = crate::session_scanner::cli_tool::spec(*tool)
            .capabilities
            .compaction_delivery;
        for home in known_managed_homes(cli_commands, *tool)
            .iter()
            .filter(|home| !tool_homes.needed.contains(*home))
        {
            changed |= match delivery {
                CompactionDelivery::HookStdout => reconcile_codex_hook_at_with_support(
                    home,
                    false,
                    codex_hooks_supported,
                    taurhaus_exe,
                )?,
                CompactionDelivery::MeshInbox => {
                    reconcile_grok_hooks_at(home, grok_enabled, false, taurhaus_exe)?
                }
            };
        }
    }
    Ok(changed)
}

fn reconcile_managed_account_hooks_for_roots_at(
    teams_roots: &[std::path::PathBuf],
    cli_commands: &crate::models::CliCommandSettings,
    codex_hooks_supported: Option<bool>,
    grok_enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let homes = collect_managed_hook_homes_for_roots(teams_roots, cli_commands)?;
    reconcile_unused_managed_hook_homes(
        homes,
        cli_commands,
        codex_hooks_supported,
        grok_enabled,
        taurhaus_exe,
    )
}

fn log_managed_account_hook_degraded(error: &CoordinationError, message: &str) {
    tracing::warn!(error = %error, "managed account hook reconciliation degraded");
    let mut fields = serde_json::Map::new();
    fields.insert(
        "error.message".to_string(),
        serde_json::Value::String(crate::errors::sanitize_error(&error.to_string())),
    );
    crate::commands::logging::emit_global(
        "warn",
        "coordination",
        "compaction.codex_hook.degraded",
        Some(message.to_string()),
        fields,
    );
}

fn reconcile_managed_account_hooks_for_launch_at(
    teams_dir: &std::path::Path,
    launch_members: &[(CliTool, Option<String>)],
    cli_commands: &crate::models::CliCommandSettings,
    codex_hooks_supported: Option<bool>,
    grok_enabled: bool,
    taurhaus_exe: &std::path::Path,
) -> Result<bool, CoordinationError> {
    let homes = collect_managed_hook_homes_for_launch(teams_dir, launch_members, cli_commands)?;
    for (tool, tool_homes) in &homes {
        let delivery = crate::session_scanner::cli_tool::spec(*tool)
            .capabilities
            .compaction_delivery;
        let reconcile = |home: &std::path::Path, needed: bool| match delivery {
            CompactionDelivery::HookStdout => reconcile_codex_hook_at_with_support(
                home,
                needed,
                codex_hooks_supported,
                taurhaus_exe,
            ),
            CompactionDelivery::MeshInbox => {
                reconcile_grok_hooks_at(home, grok_enabled, needed, taurhaus_exe)
            }
        };
        for home in &tool_homes.needed {
            reconcile(home, true)?;
        }
        // The launch is the only reconciler that sees account-scoped homes, so
        // it owns removal there too: startup and the roster reconcilers only
        // ever visit the tool's default home.
        if tool_homes.unresolved_member {
            continue;
        }
        for home in known_managed_homes(cli_commands, *tool)
            .iter()
            .filter(|home| !tool_homes.needed.contains(*home))
        {
            reconcile(home, false)?;
        }
    }
    let codex_launch_homes = launch_members
        .iter()
        .filter(|(tool, _)| {
            crate::session_scanner::cli_tool::spec(*tool)
                .capabilities
                .hook_trust
        })
        .filter_map(|(tool, account_id)| {
            resolved_managed_home(cli_commands, *tool, account_id.as_deref())
        })
        .collect::<std::collections::BTreeSet<_>>();
    Ok(!codex_launch_homes.is_empty()
        && codex_launch_homes.iter().all(|home| {
            crate::coordination::compact_hook::codex_compact_hook_is_installed_at(home)
        }))
}

fn managed_home_needed_after_switch(
    teams_dir: &std::path::Path,
    switching_team: &str,
    cli_tool: CliTool,
    home: &std::path::Path,
    accounts: &[crate::models::ManagedLaunchAccount],
) -> Result<bool, CoordinationError> {
    let default_home = accounts
        .iter()
        .find(|account| account.is_default && account.logged_in)
        .map(|account| account.dir.as_path());
    for team_name in crate::coordination::stores::TeamConfigStore::list(teams_dir)? {
        if team_name == switching_team {
            continue;
        }
        let config = crate::coordination::stores::TeamConfigStore::load(teams_dir, &team_name)?;
        for member in config
            .members
            .iter()
            .filter(|member| member.cli_tool == cli_tool)
        {
            let resolved = member
                .account_id
                .as_deref()
                .and_then(|account_id| {
                    accounts
                        .iter()
                        .find(|account| account.id == account_id && account.logged_in)
                })
                .map(|account| account.dir.as_path())
                .or(default_home);
            match resolved {
                Some(resolved) if resolved == home => return Ok(true),
                Some(_) => {}
                None => return Ok(true),
            }
        }
    }
    Ok(false)
}

/// Move a managed member's account-scoped compaction hook before its team is
/// relaunched. Installing the target first keeps the previous session covered
/// if writing the new home fails.
/// Which half of the switch-hook move to perform. Installing the target
/// before teardown keeps the previous session covered; removing the previous
/// homes must wait until every old session has stopped and the new config is
/// committed, or a failed teardown leaves a running pane with no hook.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountSwitchHookPhase {
    InstallTarget,
    RemovePrevious,
}

/// Everything a switch-hook phase needs, bundled so both phases share one
/// call shape.
pub(crate) struct AccountSwitchHookRequest<'a> {
    pub teams_dir: &'a std::path::Path,
    pub team_name: &'a str,
    pub cli_tool: CliTool,
    pub target_home: &'a std::path::Path,
    pub previous_homes: &'a [std::path::PathBuf],
    pub accounts: &'a [crate::models::ManagedLaunchAccount],
    pub grok_enabled: bool,
}

pub(crate) fn reconcile_account_switch_hooks(
    request: &AccountSwitchHookRequest<'_>,
    phase: AccountSwitchHookPhase,
) -> Result<bool, CoordinationError> {
    let capabilities = crate::session_scanner::cli_tool::spec(request.cli_tool).capabilities;
    if !capabilities.compaction_hook
        || capabilities.session_root == SessionRoot::AppManagedClaudeDir
    {
        return Ok(false);
    }
    reconcile_account_switch_hooks_at(
        AccountSwitchHookContext {
            teams_dir: request.teams_dir,
            team_name: request.team_name,
            cli_tool: request.cli_tool,
            delivery: capabilities.compaction_delivery,
            accounts: request.accounts,
            codex_hooks_supported: CliVersions::current().codex_compaction_hooks_support(),
            grok_enabled: request.grok_enabled,
            taurhaus_exe: &compact_hook_executable()?,
        },
        request.target_home,
        request.previous_homes,
        phase,
    )
}

#[derive(Clone, Copy)]
struct AccountSwitchHookContext<'a> {
    teams_dir: &'a std::path::Path,
    team_name: &'a str,
    cli_tool: CliTool,
    delivery: CompactionDelivery,
    accounts: &'a [crate::models::ManagedLaunchAccount],
    codex_hooks_supported: Option<bool>,
    grok_enabled: bool,
    taurhaus_exe: &'a std::path::Path,
}

/// Test helper mirroring the production sequencing: install the target, then
/// remove the previous homes — the two phases the switch runs around teardown.
#[cfg(test)]
fn reconcile_account_switch_hooks_at_both_phases(
    context: AccountSwitchHookContext<'_>,
    target_home: &std::path::Path,
    previous_homes: &[std::path::PathBuf],
) -> Result<bool, CoordinationError> {
    let installed = reconcile_account_switch_hooks_at(
        context,
        target_home,
        previous_homes,
        AccountSwitchHookPhase::InstallTarget,
    )?;
    let removed = reconcile_account_switch_hooks_at(
        context,
        target_home,
        previous_homes,
        AccountSwitchHookPhase::RemovePrevious,
    )?;
    Ok(installed || removed)
}

fn reconcile_account_switch_hooks_at(
    context: AccountSwitchHookContext<'_>,
    target_home: &std::path::Path,
    previous_homes: &[std::path::PathBuf],
    phase: AccountSwitchHookPhase,
) -> Result<bool, CoordinationError> {
    let mut changed = false;
    if phase == AccountSwitchHookPhase::InstallTarget {
        changed = match context.delivery {
            CompactionDelivery::HookStdout => reconcile_codex_hook_at_with_support(
                target_home,
                true,
                context.codex_hooks_supported,
                context.taurhaus_exe,
            )?,
            CompactionDelivery::MeshInbox => reconcile_grok_hooks_at(
                target_home,
                context.grok_enabled,
                true,
                context.taurhaus_exe,
            )?,
        };
        return Ok(changed);
    }
    for previous_home in previous_homes {
        if previous_home == target_home {
            continue;
        }
        let keep_installed = managed_home_needed_after_switch(
            context.teams_dir,
            context.team_name,
            context.cli_tool,
            previous_home,
            context.accounts,
        )?;
        changed |= match context.delivery {
            CompactionDelivery::HookStdout => reconcile_codex_hook_at_with_support(
                previous_home,
                keep_installed,
                context.codex_hooks_supported,
                context.taurhaus_exe,
            )?,
            CompactionDelivery::MeshInbox => reconcile_grok_hooks_at(
                previous_home,
                context.grok_enabled,
                keep_installed,
                context.taurhaus_exe,
            )?,
        };
    }
    Ok(changed)
}

fn compact_hook_executable() -> Result<std::path::PathBuf, CoordinationError> {
    Ok(PlatformPaths::daemon_binary_path())
}

#[cfg(test)]
#[path = "terminal_settings/tests.rs"]
mod tests;
