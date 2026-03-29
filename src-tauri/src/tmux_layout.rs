use std::path::Path;
use std::time::Duration;

pub const DEFAULT_SPLIT_MAX_PANES: usize = 4;
pub const LIST_WINDOWS_FORMAT: &str = "#{window_index}\t#{window_name}\t#{window_panes}";
pub const LIST_PANES_FORMAT: &str = "#{pane_id}\t#{pane_index}";
pub const TMUX_SESSION_READINESS_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxWindowRecord {
    pub index: String,
    pub name: String,
    pub pane_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxPaneRecord {
    pub pane_id: String,
    pub pane_index: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxLayoutPolicy {
    NewWindow,
    Split { max_panes: usize },
    PerProject,
}

impl TmuxLayoutPolicy {
    pub fn from_setting(raw: &str, split_max_panes: usize) -> Self {
        match raw {
            "split" => Self::Split {
                max_panes: split_max_panes,
            },
            "per_project" => Self::PerProject,
            _ => Self::NewWindow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TmuxLayoutAllocation {
    NewWindow {
        window_name: String,
    },
    SplitExisting {
        window_name: String,
        window_index: String,
    },
}

pub fn derive_window_name(project_path: &str, fallback: &str) -> String {
    Path::new(project_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub fn parse_window_records(raw: &str) -> Vec<TmuxWindowRecord> {
    raw.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let index = parts.next()?.trim();
            let name = parts.next()?.trim();
            let pane_count = parts.next()?.trim().parse::<usize>().ok()?;
            if index.is_empty() {
                return None;
            }
            Some(TmuxWindowRecord {
                index: index.to_string(),
                name: name.to_string(),
                pane_count,
            })
        })
        .collect()
}

pub fn parse_pane_records(raw: &str) -> Vec<TmuxPaneRecord> {
    raw.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let pane_id = parts.next()?.trim();
            let pane_index = parts.next()?.trim();
            if pane_id.is_empty() {
                return None;
            }
            Some(TmuxPaneRecord {
                pane_id: pane_id.to_string(),
                pane_index: pane_index.to_string(),
            })
        })
        .collect()
}

pub fn resolve_split_target_pane(
    tmux_session: &str,
    window_index: &str,
    panes: &[TmuxPaneRecord],
) -> Result<String, String> {
    panes
        .iter()
        .find_map(|pane| {
            let pane_id = pane.pane_id.trim();
            if pane_id.is_empty() {
                None
            } else {
                Some(pane_id.to_string())
            }
        })
        .ok_or_else(|| {
            format!("tmux window '{tmux_session}:{window_index}' has no panes available for split")
        })
}

pub fn wait_for_tmux_session_ready<F>(session_name: &str, verify_ready: F) -> Result<(), String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    wait_for_tmux_session_ready_with(session_name, verify_ready, std::thread::sleep)
}

fn wait_for_tmux_session_ready_with<F, S>(
    session_name: &str,
    mut verify_ready: F,
    mut sleep: S,
) -> Result<(), String>
where
    F: FnMut(&str) -> Result<(), String>,
    S: FnMut(Duration),
{
    let mut last_err = None;
    let mut attempts = 0usize;

    for retry_delay in std::iter::once(None).chain(
        TMUX_SESSION_READINESS_RETRY_DELAYS
            .iter()
            .copied()
            .map(Some),
    ) {
        if let Some(delay) = retry_delay {
            sleep(delay);
        }

        attempts += 1;
        match verify_ready(session_name) {
            Ok(()) => return Ok(()),
            Err(err) => last_err = Some(err),
        }
    }

    let last_err = last_err.unwrap_or_else(|| "unknown readiness failure".to_string());
    Err(format!(
        "tmux session '{session_name}' did not become ready after {attempts} attempts: {last_err}"
    ))
}

pub fn resolve_layout_allocation(
    policy: &TmuxLayoutPolicy,
    _tmux_session: &str,
    window_name: &str,
    windows: &[TmuxWindowRecord],
) -> TmuxLayoutAllocation {
    match policy {
        TmuxLayoutPolicy::NewWindow => TmuxLayoutAllocation::NewWindow {
            window_name: window_name.to_string(),
        },
        TmuxLayoutPolicy::Split { max_panes } => windows
            .iter()
            .find(|window| window.pane_count < *max_panes)
            .map(|window| TmuxLayoutAllocation::SplitExisting {
                window_name: window_name.to_string(),
                window_index: window.index.clone(),
            })
            .unwrap_or_else(|| TmuxLayoutAllocation::NewWindow {
                window_name: window_name.to_string(),
            }),
        TmuxLayoutPolicy::PerProject => windows
            .iter()
            .find(|window| window.name == window_name)
            .map(|window| TmuxLayoutAllocation::SplitExisting {
                window_name: window_name.to_string(),
                window_index: window.index.clone(),
            })
            .unwrap_or_else(|| TmuxLayoutAllocation::NewWindow {
                window_name: window_name.to_string(),
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_window_name_from_project_path() {
        assert_eq!(
            derive_window_name("/home/mstie/projects/taurhaus", "fallback"),
            "taurhaus"
        );
    }

    #[test]
    fn derives_window_name_uses_fallback_when_path_has_no_file_name() {
        assert_eq!(derive_window_name("/", "agent"), "agent");
    }

    #[test]
    fn parses_window_records_from_tmux_output() {
        let windows = parse_window_records("0\talpha\t1\n1\tmesh\t3\n");

        assert_eq!(
            windows,
            vec![
                TmuxWindowRecord {
                    index: "0".to_string(),
                    name: "alpha".to_string(),
                    pane_count: 1,
                },
                TmuxWindowRecord {
                    index: "1".to_string(),
                    name: "mesh".to_string(),
                    pane_count: 3,
                },
            ]
        );
    }

    #[test]
    fn parses_pane_records_from_tmux_output() {
        let panes = parse_pane_records("%7\t1\n%8\t2\n");

        assert_eq!(
            panes,
            vec![
                TmuxPaneRecord {
                    pane_id: "%7".to_string(),
                    pane_index: "1".to_string(),
                },
                TmuxPaneRecord {
                    pane_id: "%8".to_string(),
                    pane_index: "2".to_string(),
                },
            ]
        );
    }

    #[test]
    fn resolves_new_window_policy_without_inspecting_windows() {
        let allocation =
            resolve_layout_allocation(&TmuxLayoutPolicy::NewWindow, "taurhaus", "alpha", &[]);

        assert_eq!(
            allocation,
            TmuxLayoutAllocation::NewWindow {
                window_name: "alpha".to_string(),
            }
        );
    }

    #[test]
    fn split_policy_reuses_first_window_with_capacity() {
        let windows = vec![
            TmuxWindowRecord {
                index: "1".to_string(),
                name: "full".to_string(),
                pane_count: 4,
            },
            TmuxWindowRecord {
                index: "2".to_string(),
                name: "room".to_string(),
                pane_count: 2,
            },
        ];

        let allocation = resolve_layout_allocation(
            &TmuxLayoutPolicy::Split { max_panes: 4 },
            "taurhaus",
            "project-a",
            &windows,
        );

        assert_eq!(
            allocation,
            TmuxLayoutAllocation::SplitExisting {
                window_name: "project-a".to_string(),
                window_index: "2".to_string(),
            }
        );
    }

    #[test]
    fn split_policy_falls_back_to_new_window_when_all_windows_are_full() {
        let windows = vec![TmuxWindowRecord {
            index: "1".to_string(),
            name: "full".to_string(),
            pane_count: 4,
        }];

        let allocation = resolve_layout_allocation(
            &TmuxLayoutPolicy::Split { max_panes: 4 },
            "taurhaus",
            "project-a",
            &windows,
        );

        assert_eq!(
            allocation,
            TmuxLayoutAllocation::NewWindow {
                window_name: "project-a".to_string(),
            }
        );
    }

    #[test]
    fn per_project_policy_reuses_matching_window_name() {
        let windows = vec![
            TmuxWindowRecord {
                index: "3".to_string(),
                name: "project-a".to_string(),
                pane_count: 4,
            },
            TmuxWindowRecord {
                index: "4".to_string(),
                name: "project-b".to_string(),
                pane_count: 1,
            },
        ];

        let allocation = resolve_layout_allocation(
            &TmuxLayoutPolicy::PerProject,
            "taurhaus",
            "project-b",
            &windows,
        );

        assert_eq!(
            allocation,
            TmuxLayoutAllocation::SplitExisting {
                window_name: "project-b".to_string(),
                window_index: "4".to_string(),
            }
        );
    }

    #[test]
    fn per_project_policy_falls_back_to_new_window_when_project_is_absent() {
        let windows = vec![TmuxWindowRecord {
            index: "3".to_string(),
            name: "project-a".to_string(),
            pane_count: 1,
        }];

        let allocation = resolve_layout_allocation(
            &TmuxLayoutPolicy::PerProject,
            "taurhaus",
            "project-b",
            &windows,
        );

        assert_eq!(
            allocation,
            TmuxLayoutAllocation::NewWindow {
                window_name: "project-b".to_string(),
            }
        );
    }

    #[test]
    fn resolve_split_target_pane_uses_first_actual_pane_id_for_non_zero_base_index() {
        // Regression: split target used to assume `<session>:<window>.0`, which fails
        // when tmux is configured with `pane-base-index=1`.
        let panes = parse_pane_records("%7\t1\n%8\t2\n");

        let target =
            resolve_split_target_pane("taurhaus", "3", &panes).expect("pane target should exist");

        assert_eq!(target, "%7");
    }

    #[test]
    fn resolve_split_target_pane_uses_actual_pane_id_for_renumbered_window() {
        // Regression: renumbered panes may start at arbitrary indexes, so targeting `.0`
        // breaks even when the window is otherwise healthy.
        let panes = parse_pane_records("%11\t5\n%12\t6\n");

        let target =
            resolve_split_target_pane("taurhaus", "3", &panes).expect("pane target should exist");

        assert_eq!(target, "%11");
    }

    #[test]
    fn resolve_split_target_pane_reports_stale_window_target() {
        // Regression: stale window metadata used to fall through to tmux's cryptic
        // `can't find pane: 0` instead of surfacing that the selected window has no panes.
        let err = resolve_split_target_pane("taurhaus", "3", &[]).expect_err("missing panes");

        assert!(err.contains("taurhaus:3"));
        assert!(err.contains("no panes available for split"));
    }

    #[test]
    fn wait_for_tmux_session_ready_retries_until_session_becomes_usable() {
        let mut attempts = 0usize;
        let mut slept = Vec::new();

        wait_for_tmux_session_ready_with(
            "taurhaus",
            |_| {
                attempts += 1;
                if attempts < 3 {
                    Err(format!("session not ready yet (attempt {attempts})"))
                } else {
                    Ok(())
                }
            },
            |delay| slept.push(delay),
        )
        .expect("session should become ready");

        assert_eq!(attempts, 3);
        assert_eq!(
            slept,
            vec![Duration::from_millis(500), Duration::from_secs(1),]
        );
    }

    #[test]
    fn wait_for_tmux_session_ready_reports_last_failure_after_retries_exhaust() {
        let mut attempts = 0usize;
        let mut slept = Vec::new();

        let err = wait_for_tmux_session_ready_with(
            "taurhaus",
            |_| {
                attempts += 1;
                Err(format!("tmux still cold on attempt {attempts}"))
            },
            |delay| slept.push(delay),
        )
        .expect_err("session should remain unready");

        assert_eq!(attempts, 4);
        assert_eq!(slept, TMUX_SESSION_READINESS_RETRY_DELAYS);
        assert!(err.contains("did not become ready after 4 attempts"));
        assert!(err.contains("tmux still cold on attempt 4"));
    }
}
