#[cfg(feature = "mesh-bridged-backend")]
use chrono::Utc;

#[cfg(feature = "mesh-bridged-backend")]
fn main() {
    let days = match std::env::args().nth(1) {
        Some(value) => match value.parse::<u32>() {
            Ok(days) if days > 0 => days,
            _ => {
                eprintln!("DAYS must be a positive integer");
                std::process::exit(2);
            }
        },
        None => 30,
    };
    match taurhaus_lib::coordination::routing_report::render_routing_report(
        &taurhaus_lib::provider::platform_paths::PlatformPaths::teams_dir(),
        days,
        Utc::now(),
    ) {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("routing report failed: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "mesh-bridged-backend"))]
fn main() {
    eprintln!("routing-report requires the mesh-bridged-backend feature");
    std::process::exit(1);
}
