//! Store layer for coordination data.

pub mod active_project;
pub mod compaction;
pub mod config;
pub mod inbox;
pub mod lock;
pub mod operational;
pub mod runtime;

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
    OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
    OperationalWorkingSetSnapshot,
};
pub use runtime::{MemberRuntimeRecord, MemberRuntimeStore};
