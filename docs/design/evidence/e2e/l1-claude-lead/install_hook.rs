//! Harness adapter: invoke the production installer, never synthesize a hook/card.
use std::path::PathBuf;
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    assert_eq!(args.len(), 2, "usage: install-hook SCRATCH_TEAMS SCRATCH_DAEMON");
    let home = PathBuf::from(std::env::var_os("CLAUDE_CONFIG_DIR").expect("scratch selector"));
    assert_eq!(args[0], home.join("teams"));
    let changed = taurhaus_lib::coordination::compact_hook::ensure_compact_hook_installed(&args[0], &args[1])
        .expect("production Claude hook installation");
    println!("production_compact_hook_installed changed={changed}");
}
