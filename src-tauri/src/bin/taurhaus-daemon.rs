//! taurhaus-daemon — WSL-side filesystem proxy for taurhaus.
//!
//! Runs inside WSL and provides fast native filesystem and git operations
//! for projects on Linux filesystems. The main taurhaus app connects to
//! this daemon via TCP localhost.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use taurhaus_lib::daemon::server::{DaemonConfig, DEFAULT_PORT};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let args = parse_args();

    let filter = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    // Prevent auth-token desync:
    // if another daemon instance already owns the port, don't rotate token.
    // A failed duplicate start used to overwrite daemon.token before bind,
    // which broke clients talking to the still-running daemon.
    let bind_probe = match std::net::TcpListener::bind((args.bind_addr.as_str(), args.port)) {
        Ok(listener) => listener,
        Err(e) => {
            tracing::warn!(
                port = args.port,
                bind = %args.bind_addr,
                error = %e,
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

    // Handle Ctrl+C / SIGTERM
    let shutdown_signal = shutdown.clone();
    ctrlc::set_handler(move || {
        tracing::info!("Received shutdown signal");
        shutdown_signal.store(true, Ordering::Relaxed);
    })
    .expect("Failed to set signal handler");

    if let Err(e) = taurhaus_lib::daemon::server::run(&config, shutdown) {
        tracing::error!(error = %e, "Daemon server error");
        std::process::exit(1);
    }

    tracing::info!("taurhaus-daemon shut down cleanly");
}

struct Args {
    port: u16,
    bind_addr: String,
    idle_timeout_secs: Option<u64>,
    verbose: bool,
    no_auth: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        port: DEFAULT_PORT,
        bind_addr: "127.0.0.1".to_string(),
        idle_timeout_secs: Some(600),
        verbose: false,
        no_auth: false,
    };

    let raw: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < raw.len() {
        match raw[i].as_str() {
            "--port" | "-p" => {
                i += 1;
                args.port = raw
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .expect("--port requires a valid port number");
            }
            "--bind" | "-b" => {
                i += 1;
                args.bind_addr = raw.get(i).cloned().expect("--bind requires an address");
            }
            "--idle-timeout" => {
                i += 1;
                let secs: u64 = raw
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .expect("--idle-timeout requires seconds");
                args.idle_timeout_secs = if secs == 0 { None } else { Some(secs) };
            }
            "--verbose" | "-v" => {
                args.verbose = true;
            }
            "--no-auth" => {
                #[cfg(not(debug_assertions))]
                {
                    eprintln!("--no-auth is only supported in debug builds");
                    std::process::exit(1);
                }
                #[cfg(debug_assertions)]
                {
                    args.no_auth = true;
                }
            }
            "--version" | "-V" => {
                println!("taurhaus-daemon {VERSION}");
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Run with --help for usage information");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    args
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

    use super::resolve_auth_token_with;

    #[test]
    fn refuses_to_start_without_auth_unless_no_auth_flag() {
        let failed = resolve_auth_token_with(false, || None, |_| Ok("ignored".to_string()));
        assert!(failed.is_err());

        let insecure = resolve_auth_token_with(
            true,
            || None,
            |_| Err(io::Error::other("should not be called")),
        )
        .expect("--no-auth should permit insecure mode");
        assert_eq!(insecure, None);
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
