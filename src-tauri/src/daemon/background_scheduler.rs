//! Daemon-owned self-heal and pending-effort scheduler.

use std::sync::{Arc, Mutex};

use crate::daemon::protocol::{
    CoordinationPutLaunchSettingsParams, CoordinationPutLaunchSettingsResult,
};

/// Process-local launch settings pushed by the paired app.
///
/// The daemon deliberately does not invent defaults or persist this snapshot.
#[derive(Debug, Clone, Default)]
pub(crate) struct LaunchSettingsStore {
    current: Arc<Mutex<Option<CoordinationPutLaunchSettingsParams>>>,
}

impl LaunchSettingsStore {
    pub(crate) fn put(
        &self,
        incoming: CoordinationPutLaunchSettingsParams,
    ) -> CoordinationPutLaunchSettingsResult {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if current
            .as_ref()
            .is_some_and(|snapshot| snapshot.version > incoming.version)
        {
            return CoordinationPutLaunchSettingsResult {
                accepted: false,
                version: current.as_ref().map_or(0, |snapshot| snapshot.version),
            };
        }

        let version = incoming.version;
        *current = Some(incoming);
        CoordinationPutLaunchSettingsResult {
            accepted: true,
            version,
        }
    }

    pub(crate) fn get(&self) -> Option<CoordinationPutLaunchSettingsParams> {
        self.current
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::LaunchSettingsStore;
    use crate::daemon::protocol::CoordinationPutLaunchSettingsParams;

    fn settings(version: u64, base: &str) -> CoordinationPutLaunchSettingsParams {
        let mut cli_commands = crate::models::CliCommandSettings::default();
        cli_commands.claude.resume = base.to_string();
        CoordinationPutLaunchSettingsParams {
            version,
            cli_commands,
            tmux_layout: "new_window".to_string(),
        }
    }

    #[test]
    fn launch_settings_snapshot_is_highest_version_wins() {
        let store = LaunchSettingsStore::default();

        let first = store.put(settings(7, "claude2 --resume"));
        let stale = store.put(settings(6, "claude --resume"));
        let snapshot = store.get().expect("newest snapshot retained");

        assert!(first.accepted);
        assert!(!stale.accepted);
        assert_eq!(stale.version, 7);
        assert_eq!(snapshot.version, 7);
        assert_eq!(snapshot.cli_commands.claude.resume, "claude2 --resume");
    }
}
