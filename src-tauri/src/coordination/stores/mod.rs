//! Store layer for coordination data.

pub mod active_project;
pub mod compaction;
pub mod compaction_signal;
pub mod config;
pub mod inbox;
pub mod lock;
pub mod operational;
pub mod runtime;

#[allow(unused_imports)]
pub use active_project::ActiveProjectTeamStore;
#[allow(unused_imports)]
pub use compaction::{
    emit_compaction_delivery_event, emit_compaction_detected_event, is_already_handled,
    is_stale_compaction, record_delivery_at, CompactionDeliveryResult, MemberCompactionState,
    MemberCompactionStore, COMPACTION_FRESHNESS_WINDOW_SECS,
};
#[allow(unused_imports)]
pub use compaction_signal::{
    append_signal, inspect_signal_log_at, read_signal_items_from_offset, read_signals_from_offset,
    signal_log_path_for_team, CompactionSignalKind, CompactionSignalLog,
    CompactionSignalLogInspection, CompactionSignalReadItem, CompactionSignalRecord,
};
pub use config::{DiscoveredTeam, TeamConfig, TeamConfigStore};
#[allow(unused_imports)]
pub use inbox::{MeshInboxMessage, MeshInboxStore};
#[allow(unused_imports)]
pub use operational::{
    read_snapshot, write_snapshot, OperationalAssignmentFooterSnapshot, OperationalContextSnapshot,
    OperationalContextSnapshotStore, OperationalOwnershipSnapshot, OperationalTaskSnapshot,
    OperationalWorkingSetSnapshot,
};
pub use runtime::{MemberRuntimeRecord, MemberRuntimeStore};
