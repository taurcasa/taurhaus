//! Recovery policy placeholder.

/// Placeholder health recovery policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct RecoveryPolicy {
    pub cooldown_secs: u64,
}
