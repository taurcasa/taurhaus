//! Coordination subsystem scaffolding.

pub mod activity_export;
pub mod activity_schema;
pub mod agy_hooks_installer;
pub mod audit;
pub mod backend;
pub mod compact_hook;
pub mod compaction_events;
pub mod consumer;
pub mod delivery;
pub mod domain;
pub mod errors;
pub mod events;
pub mod health;
pub mod journal;
pub mod member_activation;
pub mod mesh_cli;
pub mod operational_context;
pub mod orchestrator;
pub mod pipelines;
pub mod reconcile;
pub mod recovery_card;
pub mod recovery_delivery;
pub mod reinjection;
pub mod requests;
pub mod roster;
pub mod routing_report;
pub mod runtime;
pub mod state;
pub mod stores;
pub mod task_deadline;
mod task_deadline_pass;
pub mod task_effort;
pub mod validation;

#[cfg(all(test, target_os = "linux"))]
pub(crate) mod mesh_contract_fixture;

#[cfg(target_os = "linux")]
pub(crate) mod hosted;
#[cfg(target_os = "linux")]
pub(crate) mod hosted_process;

pub(crate) mod initialize_guard;
