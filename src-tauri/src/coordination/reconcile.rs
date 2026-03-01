//! Periodic reconcile safety net for coordination state drift.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::coordination::events::CoordinationEvent;

/// Re-reads on-disk state and emits synthetic events when drift is detected.
#[derive(Debug, Clone)]
pub struct Reconciler {
    teams_dir: PathBuf,
    interval: Duration,
    last_known: HashMap<String, HashSet<String>>,
}

impl Reconciler {
    pub fn new(teams_dir: PathBuf, interval: Duration) -> Self {
        Self {
            teams_dir,
            interval,
            last_known: HashMap::new(),
        }
    }

    /// Perform one reconcile pass and emit drift-recovery events.
    pub fn reconcile(&mut self) -> Vec<CoordinationEvent> {
        let current = scan_state(&self.teams_dir);
        let mut events = Vec::new();

        let mut all_teams = BTreeSet::new();
        all_teams.extend(self.last_known.keys().cloned());
        all_teams.extend(current.keys().cloned());

        for team_name in all_teams {
            let previous_members = self.last_known.get(&team_name);
            let current_members = current.get(&team_name);

            // Team appeared or disappeared.
            if previous_members.is_some() != current_members.is_some() {
                events.push(CoordinationEvent::TeamConfigChanged {
                    team_name: team_name.clone(),
                });
            }

            // Member runtime drift (appeared/removed runtime files).
            let empty = HashSet::new();
            let prev = previous_members.unwrap_or(&empty);
            let curr = current_members.unwrap_or(&empty);

            let mut changed_members = BTreeSet::new();
            for member in prev.symmetric_difference(curr) {
                changed_members.insert(member.clone());
            }

            for member_name in changed_members {
                events.push(CoordinationEvent::MemberRuntimeChanged {
                    team_name: team_name.clone(),
                    member_name,
                });
            }
        }

        self.last_known = current;
        events
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }
}

fn scan_state(teams_dir: &Path) -> HashMap<String, HashSet<String>> {
    let mut state = HashMap::<String, HashSet<String>>::new();

    let entries = match fs::read_dir(teams_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return state,
        Err(_) => return state,
    };

    for team_entry in entries.flatten() {
        let Ok(file_type) = team_entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let Some(team_name) = team_entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let runtime_dir = team_entry.path().join("runtime");

        let mut members = HashSet::new();
        if let Ok(runtime_entries) = fs::read_dir(runtime_dir) {
            for runtime_entry in runtime_entries.flatten() {
                let Ok(ft) = runtime_entry.file_type() else {
                    continue;
                };
                if !ft.is_file() {
                    continue;
                }
                let path = runtime_entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                if let Some(member_name) = path.file_stem().and_then(|stem| stem.to_str()) {
                    if !member_name.is_empty() {
                        members.insert(member_name.to_string());
                    }
                }
            }
        }

        state.insert(team_name, members);
    }

    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_runtime_file(teams_dir: &Path, team: &str, member: &str) {
        let runtime_dir = teams_dir.join(team).join("runtime");
        fs::create_dir_all(&runtime_dir).expect("create runtime dir");
        fs::write(runtime_dir.join(format!("{member}.json")), "{}").expect("write runtime file");
    }

    #[test]
    fn new_team_detected_on_reconcile() {
        let tmp = TempDir::new().expect("tempdir");
        let mut reconciler = Reconciler::new(tmp.path().to_path_buf(), Duration::from_secs(30));

        assert!(reconciler.reconcile().is_empty());

        fs::create_dir_all(tmp.path().join("architecture-final")).expect("create team dir");
        let events = reconciler.reconcile();

        assert_eq!(
            events,
            vec![CoordinationEvent::TeamConfigChanged {
                team_name: "architecture-final".to_string()
            }]
        );
    }

    #[test]
    fn removed_team_detected_on_reconcile() {
        let tmp = TempDir::new().expect("tempdir");
        let mut reconciler = Reconciler::new(tmp.path().to_path_buf(), Duration::from_secs(30));

        fs::create_dir_all(tmp.path().join("architecture-final")).expect("create team dir");
        let _ = reconciler.reconcile(); // establish baseline with team present

        fs::remove_dir_all(tmp.path().join("architecture-final")).expect("remove team dir");
        let events = reconciler.reconcile();

        assert_eq!(
            events,
            vec![CoordinationEvent::TeamConfigChanged {
                team_name: "architecture-final".to_string()
            }]
        );
    }

    #[test]
    fn new_member_detected_on_reconcile() {
        let tmp = TempDir::new().expect("tempdir");
        let mut reconciler = Reconciler::new(tmp.path().to_path_buf(), Duration::from_secs(30));

        fs::create_dir_all(tmp.path().join("architecture-final")).expect("create team dir");
        let _ = reconciler.reconcile(); // baseline

        write_runtime_file(tmp.path(), "architecture-final", "codex-reviewer");
        let events = reconciler.reconcile();

        assert_eq!(
            events,
            vec![CoordinationEvent::MemberRuntimeChanged {
                team_name: "architecture-final".to_string(),
                member_name: "codex-reviewer".to_string()
            }]
        );
    }

    #[test]
    fn no_drift_produces_no_events() {
        let tmp = TempDir::new().expect("tempdir");
        let mut reconciler = Reconciler::new(tmp.path().to_path_buf(), Duration::from_secs(30));

        fs::create_dir_all(tmp.path().join("architecture-final")).expect("create team dir");
        write_runtime_file(tmp.path(), "architecture-final", "codex-reviewer");

        let _ = reconciler.reconcile(); // baseline snapshot
        let events = reconciler.reconcile(); // no filesystem changes
        assert!(events.is_empty());
    }

    #[test]
    fn member_removal_detected_on_reconcile() {
        let tmp = TempDir::new().expect("tempdir");
        let mut reconciler = Reconciler::new(tmp.path().to_path_buf(), Duration::from_secs(30));

        fs::create_dir_all(tmp.path().join("architecture-final")).expect("create team dir");
        write_runtime_file(tmp.path(), "architecture-final", "alice");
        write_runtime_file(tmp.path(), "architecture-final", "bob");
        let _ = reconciler.reconcile();

        fs::remove_file(
            tmp.path()
                .join("architecture-final")
                .join("runtime")
                .join("alice.json"),
        )
        .expect("remove member runtime");

        let events = reconciler.reconcile();
        assert_eq!(
            events,
            vec![CoordinationEvent::MemberRuntimeChanged {
                team_name: "architecture-final".to_string(),
                member_name: "alice".to_string(),
            }]
        );
    }

    #[test]
    fn reconcile_handles_simultaneous_adds_and_removes_across_teams() {
        let tmp = TempDir::new().expect("tempdir");
        let mut reconciler = Reconciler::new(tmp.path().to_path_buf(), Duration::from_secs(30));

        fs::create_dir_all(tmp.path().join("team-a")).expect("create team a");
        fs::create_dir_all(tmp.path().join("team-b")).expect("create team b");
        write_runtime_file(tmp.path(), "team-a", "old-a");
        write_runtime_file(tmp.path(), "team-b", "old-b");
        let _ = reconciler.reconcile();

        fs::remove_file(tmp.path().join("team-a").join("runtime").join("old-a.json"))
            .expect("remove old-a");
        write_runtime_file(tmp.path(), "team-a", "new-a");
        fs::remove_file(tmp.path().join("team-b").join("runtime").join("old-b.json"))
            .expect("remove old-b");
        write_runtime_file(tmp.path(), "team-b", "new-b");

        let mut events = reconciler.reconcile();
        events.sort_by_key(|event| match event {
            CoordinationEvent::MemberRuntimeChanged {
                team_name,
                member_name,
            } => format!("{team_name}:{member_name}"),
            CoordinationEvent::TeamConfigChanged { team_name } => format!("{team_name}:team"),
            CoordinationEvent::InboxMessage {
                team_name,
                member_name,
            } => format!("{team_name}:{member_name}"),
            CoordinationEvent::TaskFileChanged { team_name } => format!("{team_name}:task"),
        });

        assert_eq!(
            events,
            vec![
                CoordinationEvent::MemberRuntimeChanged {
                    team_name: "team-a".to_string(),
                    member_name: "new-a".to_string()
                },
                CoordinationEvent::MemberRuntimeChanged {
                    team_name: "team-a".to_string(),
                    member_name: "old-a".to_string()
                },
                CoordinationEvent::MemberRuntimeChanged {
                    team_name: "team-b".to_string(),
                    member_name: "new-b".to_string()
                },
                CoordinationEvent::MemberRuntimeChanged {
                    team_name: "team-b".to_string(),
                    member_name: "old-b".to_string()
                },
            ]
        );
    }
}
