use std::error::Error;
use std::fmt;

use crate::{PartitionId, ScheduledEventKind, Tick};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RestoreReplayEventKind {
    GlobalExit,
    SubsystemWake,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointRestoreScheduleError {
    EmptyWarmupEventSource,
    LiveEventBeforeRestoredTick {
        partition: PartitionId,
        restored_tick: Tick,
        requested_tick: Tick,
    },
    WarmupEventBeforeReplayClock {
        source: String,
        replay_now: Tick,
        requested_tick: Tick,
    },
    WarmupEventAfterRestoredTick {
        source: String,
        restored_tick: Tick,
        requested_tick: Tick,
    },
    WarmupFinishedAfterRestoredTick {
        restored_tick: Tick,
        final_tick: Tick,
    },
    WarmupAlreadyFinished {
        final_tick: Tick,
        source: String,
        requested_tick: Tick,
    },
}

impl fmt::Display for CheckpointRestoreScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWarmupEventSource => write!(formatter, "warmup event source is empty"),
            Self::LiveEventBeforeRestoredTick {
                partition,
                restored_tick,
                requested_tick,
            } => write!(
                formatter,
                "checkpoint restore live event for partition {} at tick {requested_tick} is before restored tick {restored_tick}",
                partition.index()
            ),
            Self::WarmupEventBeforeReplayClock {
                source,
                replay_now,
                requested_tick,
            } => write!(
                formatter,
                "checkpoint restore warmup event from {source} at tick {requested_tick} is before replay clock {replay_now}"
            ),
            Self::WarmupEventAfterRestoredTick {
                source,
                restored_tick,
                requested_tick,
            } => write!(
                formatter,
                "checkpoint restore warmup event from {source} at tick {requested_tick} is after restored tick {restored_tick}"
            ),
            Self::WarmupFinishedAfterRestoredTick {
                restored_tick,
                final_tick,
            } => write!(
                formatter,
                "checkpoint restore warmup finished at tick {final_tick}, after restored tick {restored_tick}"
            ),
            Self::WarmupAlreadyFinished {
                final_tick,
                source,
                requested_tick,
            } => write!(
                formatter,
                "checkpoint restore warmup already finished at tick {final_tick}; {source} requested warmup tick {requested_tick}"
            ),
        }
    }
}

impl Error for CheckpointRestoreScheduleError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointRestoreLiveEvent {
    partition: PartitionId,
    scheduled_tick: Tick,
    kind: ScheduledEventKind,
    restore_order: u64,
}

impl CheckpointRestoreLiveEvent {
    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn scheduled_tick(self) -> Tick {
        self.scheduled_tick
    }

    pub const fn kind(self) -> ScheduledEventKind {
        self.kind
    }

    pub const fn restore_order(self) -> u64 {
        self.restore_order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointRestoreWarmupEvent {
    source: String,
    replay_now: Tick,
    scheduled_tick: Tick,
    kind: RestoreReplayEventKind,
    restore_order: u64,
}

impl CheckpointRestoreWarmupEvent {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub const fn replay_now(&self) -> Tick {
        self.replay_now
    }

    pub const fn scheduled_tick(&self) -> Tick {
        self.scheduled_tick
    }

    pub const fn kind(&self) -> RestoreReplayEventKind {
        self.kind
    }

    pub const fn restore_order(&self) -> u64 {
        self.restore_order
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckpointRestoreWarmupSummary {
    restored_tick: Tick,
    warmup_final_tick: Tick,
    warmup_event_count: usize,
    live_event_count: usize,
}

impl CheckpointRestoreWarmupSummary {
    pub const fn restored_tick(self) -> Tick {
        self.restored_tick
    }

    pub const fn warmup_final_tick(self) -> Tick {
        self.warmup_final_tick
    }

    pub const fn warmup_slack_ticks(self) -> Tick {
        self.restored_tick - self.warmup_final_tick
    }

    pub const fn warmup_event_count(self) -> usize {
        self.warmup_event_count
    }

    pub const fn live_event_count(self) -> usize {
        self.live_event_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointRestoreEventPlan {
    restored_tick: Tick,
    next_restore_order: u64,
    warmup_final_tick: Option<Tick>,
    warmup_events: Vec<CheckpointRestoreWarmupEvent>,
    live_events: Vec<CheckpointRestoreLiveEvent>,
}

impl CheckpointRestoreEventPlan {
    pub fn new(restored_tick: Tick) -> Self {
        Self {
            restored_tick,
            next_restore_order: 0,
            warmup_final_tick: None,
            warmup_events: Vec::new(),
            live_events: Vec::new(),
        }
    }

    pub const fn restored_tick(&self) -> Tick {
        self.restored_tick
    }

    pub fn warmup_events(&self) -> &[CheckpointRestoreWarmupEvent] {
        &self.warmup_events
    }

    pub const fn warmup_final_tick(&self) -> Option<Tick> {
        self.warmup_final_tick
    }

    pub const fn warmup_event_count(&self) -> usize {
        self.warmup_events.len()
    }

    pub fn warmup_events_for_replay(&self) -> Vec<CheckpointRestoreWarmupEvent> {
        let mut events = self.warmup_events.clone();
        events.sort_by_key(|event| (event.replay_now, event.scheduled_tick, event.restore_order));
        events
    }

    pub fn live_events(&self) -> &[CheckpointRestoreLiveEvent] {
        &self.live_events
    }

    pub const fn live_event_count(&self) -> usize {
        self.live_events.len()
    }

    pub fn live_events_for_scheduler(&self) -> Vec<CheckpointRestoreLiveEvent> {
        let mut events = self.live_events.clone();
        events.sort_by_key(|event| (event.scheduled_tick, event.partition, event.restore_order));
        events
    }

    pub fn record_warmup_event(
        &mut self,
        source: impl Into<String>,
        replay_now: Tick,
        scheduled_tick: Tick,
        kind: RestoreReplayEventKind,
    ) -> Result<&CheckpointRestoreWarmupEvent, CheckpointRestoreScheduleError> {
        let source = source.into();
        if source.is_empty() {
            return Err(CheckpointRestoreScheduleError::EmptyWarmupEventSource);
        }
        if let Some(final_tick) = self.warmup_final_tick {
            return Err(CheckpointRestoreScheduleError::WarmupAlreadyFinished {
                final_tick,
                source,
                requested_tick: scheduled_tick,
            });
        }
        if scheduled_tick < replay_now {
            return Err(
                CheckpointRestoreScheduleError::WarmupEventBeforeReplayClock {
                    source,
                    replay_now,
                    requested_tick: scheduled_tick,
                },
            );
        }
        if scheduled_tick > self.restored_tick {
            return Err(
                CheckpointRestoreScheduleError::WarmupEventAfterRestoredTick {
                    source,
                    restored_tick: self.restored_tick,
                    requested_tick: scheduled_tick,
                },
            );
        }

        let restore_order = self.next_restore_order();
        self.warmup_events.push(CheckpointRestoreWarmupEvent {
            source,
            replay_now,
            scheduled_tick,
            kind,
            restore_order,
        });
        Ok(self
            .warmup_events
            .last()
            .expect("just-pushed warmup event is present"))
    }

    pub fn stage_live_event(
        &mut self,
        partition: PartitionId,
        scheduled_tick: Tick,
        kind: ScheduledEventKind,
    ) -> Result<CheckpointRestoreLiveEvent, CheckpointRestoreScheduleError> {
        if scheduled_tick < self.restored_tick {
            return Err(
                CheckpointRestoreScheduleError::LiveEventBeforeRestoredTick {
                    partition,
                    restored_tick: self.restored_tick,
                    requested_tick: scheduled_tick,
                },
            );
        }

        let event = CheckpointRestoreLiveEvent {
            partition,
            scheduled_tick,
            kind,
            restore_order: self.next_restore_order(),
        };
        self.live_events.push(event);
        Ok(event)
    }

    pub fn finish_warmup(
        &mut self,
        final_tick: Tick,
    ) -> Result<CheckpointRestoreWarmupSummary, CheckpointRestoreScheduleError> {
        if let Some(existing_final_tick) = self.warmup_final_tick {
            return Err(CheckpointRestoreScheduleError::WarmupAlreadyFinished {
                final_tick: existing_final_tick,
                source: "finish".to_string(),
                requested_tick: final_tick,
            });
        }
        if final_tick > self.restored_tick {
            return Err(
                CheckpointRestoreScheduleError::WarmupFinishedAfterRestoredTick {
                    restored_tick: self.restored_tick,
                    final_tick,
                },
            );
        }

        self.warmup_final_tick = Some(final_tick);
        Ok(CheckpointRestoreWarmupSummary {
            restored_tick: self.restored_tick,
            warmup_final_tick: final_tick,
            warmup_event_count: self.warmup_events.len(),
            live_event_count: self.live_events.len(),
        })
    }

    fn next_restore_order(&mut self) -> u64 {
        let order = self.next_restore_order;
        self.next_restore_order += 1;
        order
    }
}
