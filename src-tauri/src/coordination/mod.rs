//! Coordination subsystem scaffolding.

pub mod activity_export;
pub mod activity_schema;
pub mod audit;
pub mod backend;
pub mod claude_hooks;
pub mod compaction_events;
pub mod compaction_processor;
pub mod consumer;
pub mod delivery;
pub mod domain;
pub mod errors;
pub mod events;
pub mod health;
pub mod mesh_cli;
pub mod operational_context;
pub mod orchestrator;
pub mod pipelines;
pub mod reconcile;
pub mod reinjection;
pub mod requests;
pub mod roster;
pub mod runtime;
pub mod stall_detector;
pub mod state;
pub mod stores;
pub mod validation;
