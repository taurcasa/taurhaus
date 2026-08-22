#![cfg(feature = "mesh-bridged-backend")]

//! Compile smoke test for coordination module visibility from crate root.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};

#[test]
fn coordination_modules_are_visible_from_crate_root() {
    let created_at = DateTime::parse_from_rfc3339("2026-03-01T21:00:00Z")
        .expect("valid timestamp")
        .with_timezone(&Utc);
    let _team = taurhaus_lib::coordination::domain::Team {
        name: "architecture-final".to_string(),
        description: None,
        created_at,
        schema_version: 1,
    };
    let _member = taurhaus_lib::coordination::domain::Member {
        name: "codex-reviewer".to_string(),
        role: taurhaus_lib::coordination::domain::MemberRole::Agent,
        role_id: None,
        role_name: None,
        focus_area: None,
        context_summary: None,
        behavior_summary: None,
        communication_style: None,
        runtime_compact_summary: None,
        instructions: None,
        behavioral_contract: None,
        quality_gates: None,
        handoff_expectations: None,
        definition_of_done: None,
        phase_scope: None,
        mode: None,
        inherits_from: None,
        required_artifacts: None,
        capabilities: None,
        model: None,
        reasoning_effort: None,
        project_path: PathBuf::from("/tmp/taurhaus"),
        cli_tool: taurhaus_lib::session_scanner::cli_tool::CliTool::Codex,
        extra: Default::default(),
    };
    let _event = taurhaus_lib::coordination::audit::AuditEvent::TeamCreated(
        taurhaus_lib::coordination::audit::TeamCreatedEvent {
            team_name: "architecture-final".to_string(),
            member_count: 1,
            created_at,
        },
    );
    let _orchestrator = taurhaus_lib::coordination::orchestrator::CoordinationOrchestrator::new(
        PathBuf::from("/tmp/taurhaus"),
        Arc::new(taurhaus_lib::coordination::backend::MeshBridgedBackend::default()),
    );
    let _selector = taurhaus_lib::coordination::backend::selector::BackendSelector::default()
        .select(taurhaus_lib::session_scanner::cli_tool::CliTool::Codex);
    let _launch_req = taurhaus_lib::coordination::requests::LaunchRequest {
        member: _member,
        team_name: "architecture-final".to_string(),
        pane_target: None,
        permissions: taurhaus_lib::coordination::requests::LaunchPermissions::Standard,
    };
    let _config_store = taurhaus_lib::coordination::stores::config::TeamConfigStore;
    let _runtime_store = taurhaus_lib::coordination::stores::runtime::MemberRuntimeStore;
    let _state = taurhaus_lib::coordination::health::state::HealthState::Healthy;
    let _error = taurhaus_lib::coordination::errors::CoordinationError::Validation(
        "placeholder".to_string(),
    );
}
