use std::path::Path;

pub const DEFAULT_SPLIT_MAX_PANES: usize = 4;
pub const LIST_WINDOWS_FORMAT: &str = "#{window_index}\t#{window_name}\t#{window_panes}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TmuxWindowRecord {
    pub index: String,
    pub name: String,
    pub pane_count: usize,
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
        target_pane: String,
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

pub fn resolve_layout_allocation(
    policy: &TmuxLayoutPolicy,
    tmux_session: &str,
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
                target_pane: target_pane_for_window(tmux_session, &window.index),
            })
            .unwrap_or_else(|| TmuxLayoutAllocation::NewWindow {
                window_name: window_name.to_string(),
            }),
        TmuxLayoutPolicy::PerProject => windows
            .iter()
            .find(|window| window.name == window_name)
            .map(|window| TmuxLayoutAllocation::SplitExisting {
                window_name: window_name.to_string(),
                target_pane: target_pane_for_window(tmux_session, &window.index),
            })
            .unwrap_or_else(|| TmuxLayoutAllocation::NewWindow {
                window_name: window_name.to_string(),
            }),
    }
}

fn target_pane_for_window(tmux_session: &str, window_index: &str) -> String {
    format!("{tmux_session}:{window_index}.0")
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
                target_pane: "taurhaus:2.0".to_string(),
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
                target_pane: "taurhaus:4.0".to_string(),
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
}
