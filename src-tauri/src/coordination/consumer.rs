//! Event consumer that maps coordination events to orchestrator actions.

use std::path::PathBuf;
use std::sync::mpsc;

use crate::coordination::events::CoordinationEvent;

/// Action produced by the consumer for the orchestrator to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumerAction {
    RefreshTeamConfig { team_name: String },
    RefreshMemberRuntime { team_name: String, member_name: String },
    DeliverInboxMessage { team_name: String, member_name: String },
    RefreshTaskState { team_name: String },
}

/// Maps CoordinationEvents from a channel into ConsumerActions.
#[derive(Debug)]
pub struct EventConsumer {
    receiver: mpsc::Receiver<CoordinationEvent>,
    teams_dir: PathBuf,
}

impl EventConsumer {
    pub fn new(receiver: mpsc::Receiver<CoordinationEvent>, teams_dir: PathBuf) -> Self {
        Self { receiver, teams_dir }
    }

    pub fn teams_dir(&self) -> &PathBuf {
        &self.teams_dir
    }

    /// Blocking receive — waits for next event, maps to action.
    pub fn process_one(&self) -> Option<ConsumerAction> {
        self.receiver.recv().ok().map(Self::map_event)
    }

    /// Non-blocking try — returns None if channel empty.
    pub fn try_process_one(&self) -> Option<ConsumerAction> {
        self.receiver.try_recv().ok().map(Self::map_event)
    }

    /// Drain all available events without blocking.
    pub fn drain(&self) -> Vec<ConsumerAction> {
        let mut actions = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            actions.push(Self::map_event(event));
        }
        actions
    }

    fn map_event(event: CoordinationEvent) -> ConsumerAction {
        match event {
            CoordinationEvent::TeamConfigChanged { team_name } => {
                ConsumerAction::RefreshTeamConfig { team_name }
            }
            CoordinationEvent::MemberRuntimeChanged {
                team_name,
                member_name,
            } => ConsumerAction::RefreshMemberRuntime {
                team_name,
                member_name,
            },
            CoordinationEvent::InboxMessage {
                team_name,
                member_name,
            } => ConsumerAction::DeliverInboxMessage {
                team_name,
                member_name,
            },
            CoordinationEvent::TaskFileChanged { team_name } => {
                ConsumerAction::RefreshTaskState { team_name }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::mpsc;

    use super::*;
    use crate::coordination::events::CoordinationEvent;

    fn consumer_with_events(events: Vec<CoordinationEvent>) -> EventConsumer {
        let (tx, rx) = mpsc::channel();
        for event in events {
            tx.send(event).expect("send event");
        }
        drop(tx);
        EventConsumer::new(rx, PathBuf::from("/tmp/teams"))
    }

    #[test]
    fn team_config_event_maps_to_refresh_action() {
        let consumer = consumer_with_events(vec![CoordinationEvent::TeamConfigChanged {
            team_name: "arch".to_string(),
        }]);
        assert_eq!(
            consumer.try_process_one(),
            Some(ConsumerAction::RefreshTeamConfig {
                team_name: "arch".to_string()
            })
        );
    }

    #[test]
    fn runtime_event_maps_to_refresh_member_action() {
        let consumer = consumer_with_events(vec![CoordinationEvent::MemberRuntimeChanged {
            team_name: "arch".to_string(),
            member_name: "bob".to_string(),
        }]);
        assert_eq!(
            consumer.try_process_one(),
            Some(ConsumerAction::RefreshMemberRuntime {
                team_name: "arch".to_string(),
                member_name: "bob".to_string()
            })
        );
    }

    #[test]
    fn inbox_event_maps_to_deliver_action() {
        let consumer = consumer_with_events(vec![CoordinationEvent::InboxMessage {
            team_name: "arch".to_string(),
            member_name: "bob".to_string(),
        }]);
        assert_eq!(
            consumer.try_process_one(),
            Some(ConsumerAction::DeliverInboxMessage {
                team_name: "arch".to_string(),
                member_name: "bob".to_string()
            })
        );
    }

    #[test]
    fn empty_channel_returns_none() {
        let (_tx, rx) = mpsc::channel::<CoordinationEvent>();
        let consumer = EventConsumer::new(rx, PathBuf::from("/tmp/teams"));
        assert_eq!(consumer.try_process_one(), None);
    }

    #[test]
    fn drain_collects_all_available_actions() {
        let consumer = consumer_with_events(vec![
            CoordinationEvent::TeamConfigChanged {
                team_name: "a".to_string(),
            },
            CoordinationEvent::TaskFileChanged {
                team_name: "b".to_string(),
            },
        ]);
        let actions = consumer.drain();
        assert_eq!(actions.len(), 2);
    }
}

