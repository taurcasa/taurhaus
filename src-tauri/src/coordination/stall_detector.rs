//! Stall detector core service.
//!
//! Provides per-member in-memory state, configurable thresholds, polling,
//! signal collection, trigger history, and stage transitions.

#[path = "stall_detector/delivery.rs"]
mod delivery;
#[path = "stall_detector/history.rs"]
mod history;
#[path = "stall_detector/paths.rs"]
mod paths;
#[path = "stall_detector/service.rs"]
mod service;
#[path = "stall_detector/signal_sources.rs"]
mod signal_sources;
#[path = "stall_detector/signals.rs"]
mod signals;
#[path = "stall_detector/transitions.rs"]
mod transitions;
#[path = "stall_detector/types.rs"]
mod types;

#[allow(unused_imports)]
pub use self::service::StallDetectorService;
#[allow(unused_imports)]
pub use self::types::{
    MemberSignalContext, MemberStallState, MeshMemberStatus, NudgeCountWindow, SignalSnapshot,
    SignalStrength, StageTransition, StallDetectorConfig, StallSignalSnapshot, StallStage,
    StallSuppressionReason, StallSuppressionSnapshot, StallTriggerRecord, StallTriggerStage,
    StallWeeklyMetrics,
};
