use serde::Serialize;

pub(crate) const ACTIVITY_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MemberActivitySnapshot {
    pub version: u32,
    pub observed_at: String,
    pub stall_recent_activity: bool,
    pub stall_no_output: bool,
    pub stall_no_active_process: bool,
    pub active_non_shell_process: bool,
    pub recent_io: bool,
    pub pane_alive: bool,
    pub pane_foreign: bool,
    pub last_output_age_secs: Option<u64>,
    pub activity_confidence: SnapshotActivityConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotActivityConfidence {
    Active,
    LikelyWorking,
    Uncertain,
    Idle,
    Dead,
}
