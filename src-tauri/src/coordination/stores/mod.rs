//! Store layer for coordination data.

pub mod account_switch;
pub mod active_project;
pub mod compaction;
pub mod config;
pub mod inbox;
pub mod lock;
pub mod mesh_task;
pub mod operational;
pub mod runtime;
pub mod team_roots;

#[allow(unused_imports)]
pub use account_switch::AccountSwitchManifestStore;
#[allow(unused_imports)]
pub use active_project::ActiveProjectTeamStore;
#[allow(unused_imports)]
pub use compaction::{
    emit_compaction_delivery_event, record_delivery_at, CompactionDeliveryResult,
    MemberCompactionState, MemberCompactionStore,
};
pub use config::{DiscoveredTeam, TeamConfig, TeamConfigStore};
#[allow(unused_imports)]
pub use inbox::{MeshInboxMessage, MeshInboxStore, OPERATOR_SENDER_NAME};
#[allow(unused_imports)]
pub use operational::{
    read_snapshot, write_snapshot, OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
    OperationalContextSnapshotStore, OperationalOwnershipSnapshot,
    OperationalSnapshotCommitOutcome, OperationalTaskSnapshot, OperationalWorkingSetSnapshot,
};
pub use runtime::{
    EffortResumeFailure, MemberRuntimeRecord, MemberRuntimeSnapshot, MemberRuntimeStore,
    RuntimeCommitOutcome,
};
pub use team_roots::TeamRootRegistry;
