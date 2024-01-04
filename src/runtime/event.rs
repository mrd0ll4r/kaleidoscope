use alloy::event::{ButtonEvent, Event, EventKind};
use serde::{Deserialize, Serialize};

/// Filters for programs to use to describe which events they are interested in.
#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub(crate) struct EventFilter {
    pub strategy: EventFilterStrategy,
    pub entries: Vec<EventFilterEntry>,
}

impl EventFilter {
    fn match_inner(entry: &EventFilterEntry, e: &Event) -> bool {
        match entry {
            EventFilterEntry::Any => true,
            EventFilterEntry::Kind { kind } => match &e.inner {
                Ok(inner) => match kind {
                    EventFilterKind::Update => match inner {
                        EventKind::Update { .. } => true,
                        _ => false,
                    },
                    EventFilterKind::Button { filter } => match inner {
                        EventKind::Button(e) => match filter {
                            ButtonEventFilter::Down => match e {
                                ButtonEvent::Down => true,
                                _ => false,
                            },
                            ButtonEventFilter::Up => match e {
                                ButtonEvent::Up => true,
                                _ => false,
                            },
                            ButtonEventFilter::Clicked => match e {
                                ButtonEvent::Clicked { .. } => true,
                                _ => false,
                            },
                            ButtonEventFilter::LongPress => match e {
                                ButtonEvent::LongPress { .. } => true,
                                _ => false,
                            },
                        },
                        _ => false,
                    },
                },
                Err(_) => false,
            },
        }
    }

    pub fn matches(&self, e: &Event) -> bool {
        match &self.strategy {
            EventFilterStrategy::Any => {
                self.entries.iter().any(|entry| Self::match_inner(entry, e))
            }
            EventFilterStrategy::All => {
                self.entries.iter().all(|entry| Self::match_inner(entry, e))
            }
        }
    }
}

/// Event filtering strategy.
/// Applied to a set of event filters.
#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub(crate) enum EventFilterStrategy {
    Any,
    All,
}

/// A filter for events addressed to a specific address.
#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
#[serde(tag = "type")]
pub(crate) enum EventFilterEntry {
    Any,
    Kind { kind: EventFilterKind },
}

/// Specification for events to be filtered in for a specific address.
#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
#[serde(tag = "type")]
pub(crate) enum EventFilterKind {
    Update,
    Button { filter: ButtonEventFilter },
}

/// Filter specification for button events.
#[derive(Serialize, Deserialize, Debug, Clone, Ord, PartialOrd, PartialEq, Eq)]
pub(crate) enum ButtonEventFilter {
    Down,
    Up,
    Clicked,
    LongPress,
}
