//! Daemon event normalization into typed coordination events.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{SendError, Sender};

/// Typed coordination event produced from filesystem changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinationEvent {
    TeamConfigChanged { team_name: String },
    MemberRuntimeChanged { team_name: String, member_name: String },
    InboxMessage { team_name: String, member_name: String },
    TaskFileChanged { team_name: String },
}

impl CoordinationEvent {
    /// Stable key for event dedup/coalescing.
    pub fn coalescing_key(&self) -> String {
        match self {
            Self::TeamConfigChanged { team_name } => format!("team_config:{team_name}"),
            Self::MemberRuntimeChanged {
                team_name,
                member_name,
            } => format!("member_runtime:{team_name}:{member_name}"),
            Self::InboxMessage {
                team_name,
                member_name,
            } => format!("inbox:{team_name}:{member_name}"),
            Self::TaskFileChanged { team_name } => format!("task_file:{team_name}"),
        }
    }
}

/// Normalizes raw watcher paths and emits typed coordination events.
#[derive(Debug)]
pub struct EventProducer {
    teams_dir: PathBuf,
    sender: Sender<CoordinationEvent>,
}

impl EventProducer {
    pub fn new(teams_dir: PathBuf, sender: Sender<CoordinationEvent>) -> Self {
        Self { teams_dir, sender }
    }

    /// Classify a raw filesystem path into a typed coordination event.
    pub fn classify(&self, path: &Path) -> Option<CoordinationEvent> {
        let rel = path.strip_prefix(&self.teams_dir).ok()?;
        let mut parts = rel.iter();

        let team_name = parts.next()?.to_str()?.to_string();
        let second = parts.next()?.to_str()?;

        // {team}/config.json
        if second == "config.json" && parts.next().is_none() {
            return Some(CoordinationEvent::TeamConfigChanged { team_name });
        }

        // {team}/runtime/{member}.json
        if second == "runtime" {
            let file_name = parts.next()?.to_str()?;
            if parts.next().is_none() {
                if let Some(member_name) = json_stem(file_name) {
                    return Some(CoordinationEvent::MemberRuntimeChanged {
                        team_name,
                        member_name: member_name.to_string(),
                    });
                }
            }
            return None;
        }

        // {team}/inbox-{member}.json
        if let Some(member) = second
            .strip_prefix("inbox-")
            .and_then(|rest| rest.strip_suffix(".json"))
        {
            if !member.is_empty() && parts.next().is_none() {
                return Some(CoordinationEvent::InboxMessage {
                    team_name,
                    member_name: member.to_string(),
                });
            }
        }

        // {team}/tasks/*.json
        if second == "tasks" {
            let file_name = parts.next()?.to_str()?;
            if parts.next().is_none() && json_stem(file_name).is_some() {
                return Some(CoordinationEvent::TaskFileChanged { team_name });
            }
        }

        None
    }

    /// Normalize and send a path-derived event into the channel.
    pub fn produce(&self, path: &Path) -> Result<(), SendError<CoordinationEvent>> {
        if let Some(event) = self.classify(path) {
            self.sender.send(event)?;
        }
        Ok(())
    }
}

fn json_stem(file_name: &str) -> Option<&str> {
    file_name.strip_suffix(".json").filter(|stem| !stem.is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn producer_with_temp_channel() -> (EventProducer, mpsc::Receiver<CoordinationEvent>) {
        let (tx, rx) = mpsc::channel();
        (
            EventProducer::new(PathBuf::from("/tmp/teams"), tx),
            rx,
        )
    }

    #[test]
    fn config_change_produces_team_config_event() {
        let (producer, _rx) = producer_with_temp_channel();
        let path = Path::new("/tmp/teams/architecture-final/config.json");
        let event = producer.classify(path);

        assert_eq!(
            event,
            Some(CoordinationEvent::TeamConfigChanged {
                team_name: "architecture-final".to_string()
            })
        );
    }

    #[test]
    fn runtime_change_produces_member_runtime_event() {
        let (producer, _rx) = producer_with_temp_channel();
        let path = Path::new("/tmp/teams/architecture-final/runtime/codex-reviewer.json");
        let event = producer.classify(path);

        assert_eq!(
            event,
            Some(CoordinationEvent::MemberRuntimeChanged {
                team_name: "architecture-final".to_string(),
                member_name: "codex-reviewer".to_string()
            })
        );
    }

    #[test]
    fn inbox_write_produces_inbox_message_event() {
        let (producer, _rx) = producer_with_temp_channel();
        let path = Path::new("/tmp/teams/architecture-final/inbox-codex-reviewer.json");
        let event = producer.classify(path);

        assert_eq!(
            event,
            Some(CoordinationEvent::InboxMessage {
                team_name: "architecture-final".to_string(),
                member_name: "codex-reviewer".to_string()
            })
        );
    }

    #[test]
    fn task_change_produces_task_file_event() {
        let (producer, _rx) = producer_with_temp_channel();
        let path = Path::new("/tmp/teams/architecture-final/tasks/task-001.json");
        let event = producer.classify(path);

        assert_eq!(
            event,
            Some(CoordinationEvent::TaskFileChanged {
                team_name: "architecture-final".to_string()
            })
        );
    }

    #[test]
    fn unknown_path_produces_no_event() {
        let (producer, _rx) = producer_with_temp_channel();
        let path = Path::new("/tmp/teams/architecture-final/notes/readme.md");
        let event = producer.classify(path);

        assert_eq!(event, None);
    }
}
