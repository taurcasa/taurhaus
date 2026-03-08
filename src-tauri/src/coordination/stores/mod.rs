//! Store layer for coordination data.

pub mod compaction;
pub mod config;
pub mod lock;
pub mod operational;
pub mod runtime;

#[allow(unused_imports)]
pub use compaction::{
    emit_compaction_delivery_event, emit_compaction_detected_event, is_already_handled,
    is_stale_compaction, record_delivery, CompactionDeliveryResult, MemberCompactionState,
    MemberCompactionStore, COMPACTION_FRESHNESS_WINDOW_SECS,
};
pub use config::{DiscoveredTeam, TeamConfig, TeamConfigStore};
#[allow(unused_imports)]
pub use operational::{
    read_snapshot, write_snapshot, OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
    OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
    OperationalWorkingSetSnapshot,
};
pub use runtime::{MemberRuntimeRecord, MemberRuntimeStore};
