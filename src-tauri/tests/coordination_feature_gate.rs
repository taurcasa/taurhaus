//! Compile smoke test for coordination feature-gated type visibility.

#![cfg(feature = "mesh-bridged-backend")]

use std::path::PathBuf;
use std::time::Duration;

#[test]
fn coordination_core_types_are_importable() {
    let _event = taurhaus_lib::coordination::events::CoordinationEvent::TaskFileChanged {
        team_name: "architecture-final".to_string(),
    };
    let _action = taurhaus_lib::coordination::consumer::ConsumerAction::RefreshTaskState {
        team_name: "architecture-final".to_string(),
    };
    let _reconciler = taurhaus_lib::coordination::reconcile::Reconciler::new(
        PathBuf::from("/tmp/teams"),
        Duration::from_secs(30),
    );

    // Keep this assertion trivial; this test is for compile/link visibility.
    assert_eq!(2 + 2, 4);
}
