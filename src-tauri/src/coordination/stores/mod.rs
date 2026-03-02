//! Store layer for coordination data.

pub mod config;
pub mod lock;
pub mod runtime;

pub use config::{DiscoveredTeam, TeamConfig, TeamConfigStore, TeamDiscovery};
pub use runtime::{MemberRuntimeRecord, MemberRuntimeStore};
