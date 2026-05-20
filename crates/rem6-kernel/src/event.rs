use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fmt;

use crate::clock::{ClockDomain, ClockError, Cycles};
use crate::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EventId(u64);

impl EventId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScheduleError {
    InThePast { now: Tick, requested: Tick },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InThePast { now, requested } => {
                write!(
                    formatter,
                    "cannot schedule event at tick {requested}; current tick is {now}"
                )
            }
        }
    }
}

impl Error for ScheduleError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockScheduleError {
    Clock(ClockError),
    Schedule(ScheduleError),
}

impl fmt::Display for ClockScheduleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clock(error) => write!(formatter, "{error}"),
            Self::Schedule(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ClockScheduleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clock(error) => Some(error),
            Self::Schedule(error) => Some(error),
        }
    }
}

type EventCallback = Box<dyn FnOnce(Tick) + Send + 'static>;

pub struct EventQueue {
    now: Tick,
    next_id: u64,
    next_order: u64,
    pending: BinaryHeap<ScheduledEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            now: 0,
            next_id: 0,
            next_order: 0,
            pending: BinaryHeap::new(),
        }
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn schedule_at<F>(&mut self, tick: Tick, callback: F) -> Result<EventId, ScheduleError>
    where
        F: FnOnce(Tick) + Send + 'static,
    {
        if tick < self.now {
            return Err(ScheduleError::InThePast {
                now: self.now,
                requested: tick,
            });
        }

        let id = EventId(self.next_id);
        self.next_id += 1;

        let order = self.next_order;
        self.next_order += 1;

        self.pending.push(ScheduledEvent {
            tick,
            order,
            id,
            callback: Some(Box::new(callback)),
        });

        Ok(id)
    }

    pub fn schedule_after<F>(&mut self, delay: Tick, callback: F) -> Result<EventId, ScheduleError>
    where
        F: FnOnce(Tick) + Send + 'static,
    {
        self.schedule_at(self.now + delay, callback)
    }

    pub fn schedule_at_clock_edge<F>(
        &mut self,
        domain: ClockDomain,
        cycles: Cycles,
        callback: F,
    ) -> Result<EventId, ClockScheduleError>
    where
        F: FnOnce(Tick) + Send + 'static,
    {
        let tick = domain
            .clock_edge(self.now, cycles)
            .map_err(ClockScheduleError::Clock)?;
        self.schedule_at(tick, callback)
            .map_err(ClockScheduleError::Schedule)
    }

    pub fn run_until_empty(&mut self) {
        while let Some(mut event) = self.pending.pop() {
            self.now = event.tick;
            let callback = event.callback.take().expect("event callback is present");
            callback(self.now);
        }
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

struct ScheduledEvent {
    tick: Tick,
    order: u64,
    id: EventId,
    callback: Option<EventCallback>,
}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.tick == other.tick && self.order == other.order && self.id == other.id
    }
}

impl Eq for ScheduledEvent {}

impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .tick
            .cmp(&self.tick)
            .then_with(|| other.order.cmp(&self.order))
            .then_with(|| other.id.0.cmp(&self.id.0))
    }
}
