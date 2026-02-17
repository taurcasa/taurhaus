//! taurhaus-daemon — WSL-side filesystem proxy for taurhaus.
//!
//! Runs inside WSL and provides fast native filesystem and git operations
//! for projects on Linux filesystems. The main taurhaus app connects to
//! this daemon via TCP localhost.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use taurhaus_lib::daemon::server::{DaemonConfig, DEFAULT_PORT};

fn main() {
    let args = parse_args();

    let filter = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    tracing::info!(
        port = args.port,
        bind = %args.bind_addr,
        idle_timeout = ?args.idle_timeout_secs,
        "taurhaus-daemon starting"
    );

    let config = DaemonConfig {
        port: args.port,
        bind_addr: args.bind_addr,
        idle_timeout_secs: args.idle_timeout_secs,
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
}

fn parse_args() -> Args {
    let mut args = Args {
        port: DEFAULT_PORT,
        bind_addr: "127.0.0.1".to_string(),
        idle_timeout_secs: Some(600),
        verbose: false,
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
                args.bind_addr = raw
                    .get(i)
                    .cloned()
                    .expect("--bind requires an address");
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
    eprintln!("      --idle-timeout <SECS>  Auto-shutdown after N idle seconds (default: 600, 0=disable)");
    eprintln!("  -v, --verbose              Enable debug logging");
    eprintln!("  -h, --help                 Show this help");
}
