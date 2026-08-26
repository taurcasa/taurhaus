//! taurhaus-daemon — WSL-side filesystem proxy for taurhaus.
//!
//! Runs inside WSL and provides fast native filesystem and git operations
//! for projects on Linux filesystems. The main taurhaus app connects to
//! this daemon via TCP localhost.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use taurhaus_lib::daemon::server::{DaemonConfig, DEFAULT_PORT};
use taurhaus_lib::logging::{install_global_sink, LogFileState};
use taurhaus_lib::provider::local::LocalProvider;
use taurhaus_lib::provider::platform_paths::PlatformPaths;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if maybe_run_compact_hook_mode() {
        return;
    }
    let args = match parse_args() {
        Ok(ParseOutcome::Run(args)) => args,
        Ok(ParseOutcome::ExitSuccess) => return,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    if let Some(data_dir) = args.data_dir.as_ref() {
        std::env::set_var("TAURHAUS_DATA_DIR", data_dir);
    }

    let filter = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    let log_state = LogFileState::new(PlatformPaths::log_path()).unwrap_or_else(|error| {
        tracing::error!(error = %error, "Failed to initialize daemon structured log sink");
        std::process::exit(1);
    });
    install_global_sink(&log_state);

    // Prevent auth-token desync:
    // if another daemon instance already owns the port, don't rotate token.
    // A failed duplicate start used to overwrite daemon.token before bind,
    // which broke clients talking to the still-running daemon.
    let bind_probe = match std::net::TcpListener::bind((args.bind_addr.as_str(), args.port)) {
        Ok(listener) => listener,
        Err(error) => {
            tracing::warn!(
                port = args.port,
                bind = %args.bind_addr,
                error = %error,
                "Port already in use; refusing to rotate daemon auth token"
            );
            std::process::exit(1);
        }
    };
    drop(bind_probe);

    let auth_token = match resolve_auth_token(args.no_auth) {
        Ok(token) => token,
        Err(message) => {
            tracing::error!("{message}");
            eprintln!("{message}");
            std::process::exit(1);
        }
    };

    tracing::info!(
        port = args.port,
        bind = %args.bind_addr,
        idle_timeout = ?args.idle_timeout_secs,
        data_root = %PlatformPaths::app_data_root().display(),
        auth = auth_token.is_some(),
        "taurhaus-daemon starting"
    );

    let config = DaemonConfig {
        port: args.port,
        bind_addr: args.bind_addr,
        idle_timeout_secs: args.idle_timeout_secs,
        auth_token,
    };

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_signal = shutdown.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        tracing::info!("Received shutdown signal");
        shutdown_signal.store(true, Ordering::Relaxed);
    }) {
        tracing::error!(error = %error, "Failed to set signal handler");
        std::process::exit(1);
    }

    if let Err(error) =
        taurhaus_lib::daemon::server::run(&config, shutdown, Arc::new(LocalProvider))
    {
        tracing::error!(error = %error, "Daemon server error");
        std::process::exit(1);
    }

    tracing::info!("taurhaus-daemon shut down cleanly");
}

fn maybe_run_compact_hook_mode() -> bool {
    let mode = std::env::args().nth(1);
    if !matches!(
        mode.as_deref(),
        Some("--compact-hook" | "--claude-compact-hook")
    ) {
        return false;
    }

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
        .init();
    let _log_state = LogFileState::new(PlatformPaths::log_path())
        .inspect(|state| {
            install_global_sink(state);
        })
        .map_err(|error| tracing::warn!(error = %error, "compact hook log sink unavailable"))
        .ok();
    let teams_dir = PlatformPaths::teams_dir();
    if let Err(error) = taurhaus_lib::coordination::compact_hook::run_compact_hook_cli(
        std::io::stdin(),
        std::io::stdout(),
        &teams_dir,
    ) {
        taurhaus_lib::coordination::compact_hook::emit_compact_hook_cli_failed(&error.to_string());
        tracing::warn!(error = %error, "compact hook bridge failed");
        println!("{{}}");
    }
    true
}

struct Args {
    port: u16,
    bind_addr: String,
    idle_timeout_secs: Option<u64>,
    data_dir: Option<std::path::PathBuf>,
    verbose: bool,
    no_auth: bool,
}

enum ParseOutcome {
    Run(Args),
    ExitSuccess,
}

fn parse_args() -> Result<ParseOutcome, String> {
    let raw: Vec<String> = std::env::args().collect();
    parse_args_from(&raw)
}

fn parse_args_from(raw: &[String]) -> Result<ParseOutcome, String> {
    let mut args = Args {
        port: DEFAULT_PORT,
        bind_addr: "127.0.0.1".to_string(),
        idle_timeout_secs: Some(600),
        data_dir: None,
        verbose: false,
        no_auth: false,
    };

    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "--port" | "-p" => {
                let value = require_flag_value(raw, &mut i, "--port")?;
                args.port = value
                    .parse()
                    .map_err(|_| "--port requires a valid port number".to_string())?;
            }
            "--bind" | "-b" => {
                args.bind_addr = require_flag_value(raw, &mut i, "--bind")?;
            }
            "--idle-timeout" => {
                let value = require_flag_value(raw, &mut i, "--idle-timeout")?;
                let secs: u64 = value
                    .parse()
                    .map_err(|_| "--idle-timeout requires seconds".to_string())?;
                args.idle_timeout_secs = if secs == 0 { None } else { Some(secs) };
            }
            "--data-dir" => {
                let value = require_flag_value(raw, &mut i, "--data-dir")?;
                if value.trim().is_empty() {
                    return Err("--data-dir requires a non-empty path".to_string());
                }
                args.data_dir = Some(std::path::PathBuf::from(value));
            }
            "--verbose" | "-v" => {
                args.verbose = true;
            }
            "--no-auth" => {
                #[cfg(not(debug_assertions))]
                {
                    return Err("--no-auth is only supported in debug builds".to_string());
                }
                #[cfg(debug_assertions)]
                {
                    args.no_auth = true;
                }
            }
            "--version" | "-V" => {
                println!("taurhaus-daemon {VERSION}");
                return Ok(ParseOutcome::ExitSuccess);
            }
            "--help" | "-h" => {
                print_help();
                return Ok(ParseOutcome::ExitSuccess);
            }
            other => {
                return Err(format!(
                    "Unknown argument: {other}\nRun with --help for usage information"
                ));
            }
        }
        i += 1;
    }

    Ok(ParseOutcome::Run(args))
}

fn require_flag_value(raw: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    raw.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn print_help() {
    eprintln!("taurhaus-daemon — WSL-side filesystem proxy for taurhaus");
    eprintln!();
    eprintln!("Runs inside WSL to provide fast native filesystem and git");
    eprintln!("operations for projects on Linux filesystems. The taurhaus");
    eprintln!("app connects to this daemon via TCP localhost.");
    eprintln!();
    eprintln!("Usage: taurhaus-daemon [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -p, --port <PORT>          TCP port to listen on (default: {DEFAULT_PORT})");
    eprintln!("  -b, --bind <ADDR>          Bind address (default: 127.0.0.1)");
    eprintln!(
        "      --idle-timeout <SECS>  Auto-shutdown after N idle seconds (default: 600, 0=disable)"
    );
    eprintln!("      --data-dir <PATH>     App data root for daemon state and identity");
    eprintln!("      --no-auth              Disable auth token (debug builds only)");
    eprintln!("  -v, --verbose              Enable debug logging");
    eprintln!("  -V, --version              Print version and exit");
    eprintln!("  -h, --help                 Show this help");
}

fn resolve_auth_token(no_auth: bool) -> Result<Option<String>, String> {
    resolve_auth_token_with(
        no_auth,
        taurhaus_lib::daemon::auth::token_path,
        taurhaus_lib::daemon::auth::generate_and_write_token,
    )
}

fn resolve_auth_token_with<P, G>(
    no_auth: bool,
    token_path_fn: P,
    generate_token_fn: G,
) -> Result<Option<String>, String>
where
    P: FnOnce() -> Option<std::path::PathBuf>,
    G: FnOnce(&std::path::Path) -> std::io::Result<String>,
{
    if no_auth {
        tracing::warn!("Authentication disabled via --no-auth");
        return Ok(None);
    }

    let path = token_path_fn()
        .ok_or_else(|| "Could not determine data dir for daemon auth token".to_string())?;
    let token = generate_token_fn(&path).map_err(|error| {
        format!(
            "Failed to write daemon auth token at {}: {}",
            path.display(),
            error
        )
    })?;
    tracing::info!(path = %path.display(), "Auth token written");
    Ok(Some(token))
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{parse_args_from, resolve_auth_token_with, ParseOutcome};

    #[test]
    fn data_dir_and_port_flags_are_parsed() {
        let raw = [
            "taurhaus-daemon".to_string(),
            "--data-dir".to_string(),
            "/tmp/taurhaus-data".to_string(),
            "--port".to_string(),
            "17299".to_string(),
        ];

        let ParseOutcome::Run(args) = parse_args_from(&raw).expect("parse args") else {
            panic!("expected daemon run args");
        };
        assert_eq!(args.port, 17299);
        assert_eq!(
            args.data_dir.as_deref(),
            Some(std::path::Path::new("/tmp/taurhaus-data"))
        );
    }

    #[test]
    fn refuses_to_start_without_auth_unless_no_auth_flag() {
        let failed = resolve_auth_token_with(false, || None, |_| Ok("ignored".to_string()));
        assert!(failed.is_err());

        let insecure = resolve_auth_token_with(
            true,
            || None,
            |_| Err(io::Error::other("should not be called")),
        );
        assert_eq!(insecure.ok(), Some(None));
    }

    #[test]
    fn auth_setup_fails_when_token_write_fails() {
        let failed = resolve_auth_token_with(
            false,
            || Some(std::path::PathBuf::from("/tmp/daemon.token")),
            |_| Err(io::Error::other("disk full")),
        );
        assert!(failed.is_err());
    }
}
