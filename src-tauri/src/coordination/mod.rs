#![cfg(feature = "mesh-bridged-backend")]

//! Coordination subsystem scaffolding.

pub mod audit;
pub mod backend;
pub mod delivery;
pub mod domain;
pub mod errors;
pub mod health;
pub mod orchestrator;
pub mod requests;
pub mod stores;
