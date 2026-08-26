use crate::daemon_api::protocol;
use crate::ProviderState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeSnapshotFreshness {
    Fresh,
    Cached,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RuntimeSnapshotOutcome {
    pub snapshot: Option<protocol::RuntimeSessionSnapshotResult>,
    pub freshness: RuntimeSnapshotFreshness,
}

pub(crate) fn daemon_runtime_session_snapshot(
    provider: &ProviderState,
) -> Result<RuntimeSnapshotOutcome, String> {
    let Some(ref daemon) = provider.daemon else {
        return Ok(RuntimeSnapshotOutcome {
            snapshot: None,
            freshness: RuntimeSnapshotFreshness::Unavailable,
        });
    };

    if !daemon.is_connected() && !daemon.try_reconnect() {
        let cached = crate::session_snapshot_cache::load();
        return Ok(RuntimeSnapshotOutcome {
            freshness: if cached.is_some() {
                RuntimeSnapshotFreshness::Cached
            } else {
                RuntimeSnapshotFreshness::Unavailable
            },
            snapshot: cached,
        });
    }

    match request_daemon_runtime_session_snapshot(daemon) {
        Ok(Some(snapshot)) => {
            crate::session_snapshot_cache::store(&snapshot);
            Ok(RuntimeSnapshotOutcome {
                snapshot: Some(snapshot),
                freshness: RuntimeSnapshotFreshness::Fresh,
            })
        }
        Ok(None) => {
            let cached = crate::session_snapshot_cache::load();
            Ok(RuntimeSnapshotOutcome {
                freshness: if cached.is_some() {
                    RuntimeSnapshotFreshness::Cached
                } else {
                    RuntimeSnapshotFreshness::Unavailable
                },
                snapshot: cached,
            })
        }
        Err(error) => {
            if taurhaus_lib::daemon_api::is_busy_transport_error(&error) {
                tracing::debug!(
                    error = %error,
                    "Daemon runtime session snapshot skipped because the shared daemon connection is busy"
                );
                let cached = crate::session_snapshot_cache::load();
                return Ok(RuntimeSnapshotOutcome {
                    freshness: if cached.is_some() {
                        RuntimeSnapshotFreshness::Cached
                    } else {
                        RuntimeSnapshotFreshness::Unavailable
                    },
                    snapshot: cached,
                });
            }

            tracing::warn!(
                error = %error,
                "Failed to reach daemon for runtime session snapshot; attempting inline reconnect"
            );
            if daemon.try_reconnect() {
                match request_daemon_runtime_session_snapshot(daemon) {
                    Ok(Some(snapshot)) => {
                        crate::session_snapshot_cache::store(&snapshot);
                        Ok(RuntimeSnapshotOutcome {
                            snapshot: Some(snapshot),
                            freshness: RuntimeSnapshotFreshness::Fresh,
                        })
                    }
                    Ok(None) => {
                        let cached = crate::session_snapshot_cache::load();
                        Ok(RuntimeSnapshotOutcome {
                            freshness: if cached.is_some() {
                                RuntimeSnapshotFreshness::Cached
                            } else {
                                RuntimeSnapshotFreshness::Unavailable
                            },
                            snapshot: cached,
                        })
                    }
                    Err(retry_error) => {
                        tracing::warn!(
                            error = %retry_error,
                            "Inline reconnect succeeded but runtime session snapshot retry failed"
                        );
                        let cached = crate::session_snapshot_cache::load();
                        Ok(RuntimeSnapshotOutcome {
                            freshness: if cached.is_some() {
                                RuntimeSnapshotFreshness::Cached
                            } else {
                                RuntimeSnapshotFreshness::Unavailable
                            },
                            snapshot: cached,
                        })
                    }
                }
            } else {
                let cached = crate::session_snapshot_cache::load();
                Ok(RuntimeSnapshotOutcome {
                    freshness: if cached.is_some() {
                        RuntimeSnapshotFreshness::Cached
                    } else {
                        RuntimeSnapshotFreshness::Unavailable
                    },
                    snapshot: cached,
                })
            }
        }
    }
}

pub(crate) fn request_daemon_runtime_session_snapshot(
    daemon: &crate::provider::daemon_client::DaemonProvider,
) -> Result<Option<protocol::RuntimeSessionSnapshotResult>, String> {
    let request = protocol::DaemonRequest::new(
        "runtime-session-snapshot",
        protocol::method::GET_RUNTIME_SESSION_SNAPSHOT,
        serde_json::Value::Null,
    );
    match daemon.send_status_request(&request) {
        Ok(response) if response.is_ok() => {
            decode_daemon_runtime_session_snapshot(response.result).map(Some)
        }
        Ok(response) => {
            tracing::warn!(
                error = ?response.error,
                "Daemon returned error for runtime session snapshot"
            );
            Ok(None)
        }
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn decode_daemon_runtime_session_snapshot(
    payload: Option<serde_json::Value>,
) -> Result<protocol::RuntimeSessionSnapshotResult, String> {
    match payload {
        Some(value) => serde_json::from_value(value).map_err(|error| {
            tracing::warn!(
                error = %error,
                "Failed to deserialize runtime session snapshot from daemon"
            );
            format!("Runtime session snapshot decode error: {error}")
        }),
        None => Ok(protocol::RuntimeSessionSnapshotResult {
            version: 0,
            display_sessions: Vec::new(),
            runtime_sessions: Vec::new(),
            focus: None,
            foreground_project_path: None,
            degraded: false,
            degraded_revision: 0,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::daemon_client::DaemonProvider;
    use crate::provider::local::LocalProvider;
    use crate::test_support::StubDaemon;

    // Regression: 07ab6c5 deleted the hook -> file -> inotify focus chain, so the
    // daemon's versioned snapshot became the app's only focus transport, and
    // a0f3545 then gated the protocol on the health monitor's recovery path
    // alone. This retained startup fallback reconnects inline through
    // `try_reconnect`, which counted a reachable socket as success — so a daemon
    // that predates hub-owned focus (started by hand, or arriving late) was
    // adopted here and served snapshots whose omitted focus fields decode as
    // `None`, with nothing left to drive the foreground indicator.
    #[test]
    fn the_inline_reconnect_refuses_a_daemon_that_predates_hub_owned_focus() {
        let _guard = crate::test_support::acquire_heavy_test_guard();
        crate::session_snapshot_cache::clear();

        let stub = StubDaemon::start(
            crate::daemon::protocol::PROTOCOL_VERSION - 1,
            serde_json::json!({
                "version": 12,
                "display_sessions": [],
                "runtime_sessions": [],
                "foreground_project_path": null,
            }),
        );
        let provider = ProviderState {
            local: LocalProvider,
            daemon: Some(DaemonProvider::new_disconnected(stub.addr())),
            wsl_distro: None,
        };

        let outcome = daemon_runtime_session_snapshot(&provider).expect("snapshot call");

        assert_eq!(
            outcome.freshness,
            RuntimeSnapshotFreshness::Unavailable,
            "an outdated daemon must not supply the runtime snapshot"
        );
        assert_eq!(outcome.snapshot, None);
        assert!(
            !provider.daemon.as_ref().expect("daemon").is_connected(),
            "the mismatched daemon must be dropped, not left connected for the next call"
        );
    }

    #[test]
    fn the_inline_reconnect_adopts_a_daemon_speaking_this_protocol() {
        let _guard = crate::test_support::acquire_heavy_test_guard();
        crate::session_snapshot_cache::clear();

        let stub = StubDaemon::start(
            crate::daemon::protocol::PROTOCOL_VERSION,
            serde_json::json!({
                "version": 12,
                "display_sessions": [],
                "runtime_sessions": [],
                "foreground_project_path": "/home/dev/taurhaus",
            }),
        );
        let provider = ProviderState {
            local: LocalProvider,
            daemon: Some(DaemonProvider::new_disconnected(stub.addr())),
            wsl_distro: None,
        };

        let outcome = daemon_runtime_session_snapshot(&provider).expect("snapshot call");

        assert_eq!(outcome.freshness, RuntimeSnapshotFreshness::Fresh);
        assert_eq!(
            outcome
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.foreground_project_path.as_deref()),
            Some("/home/dev/taurhaus")
        );
    }
}
