use rem6_memory::{Address, MemoryRequest};

use crate::TrafficGeneratorError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficRequestKind {
    Read,
    Write,
    Maintenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficRequestEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficRequestKind,
    address: Address,
    request: MemoryRequest,
}

impl TrafficRequestEvent {
    pub(crate) fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficRequestKind,
        address: Address,
        request: MemoryRequest,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            request,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn kind(&self) -> TrafficRequestKind {
        self.kind
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn request(&self) -> &MemoryRequest {
        &self.request
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrafficGeneratorSummary {
    packet_count: u64,
    read_count: u64,
    write_count: u64,
    bytes_read: u64,
    bytes_written: u64,
    first_tick: Option<u64>,
    last_tick: Option<u64>,
}

impl TrafficGeneratorSummary {
    pub(crate) fn record(
        &mut self,
        tick: u64,
        kind: TrafficRequestKind,
        bytes: u64,
    ) -> Result<(), TrafficGeneratorError> {
        self.packet_count = checked_counter_add("summary.packet_count", self.packet_count, 1)?;
        self.first_tick = Some(self.first_tick.map_or(tick, |first| first.min(tick)));
        self.last_tick = Some(self.last_tick.map_or(tick, |last| last.max(tick)));

        match kind {
            TrafficRequestKind::Read => {
                self.read_count = checked_counter_add("summary.read_count", self.read_count, 1)?;
                self.bytes_read =
                    checked_counter_add("summary.bytes_read", self.bytes_read, bytes)?;
            }
            TrafficRequestKind::Write => {
                self.write_count = checked_counter_add("summary.write_count", self.write_count, 1)?;
                self.bytes_written =
                    checked_counter_add("summary.bytes_written", self.bytes_written, bytes)?;
            }
            TrafficRequestKind::Maintenance => {}
        }

        Ok(())
    }

    pub const fn packet_count(self) -> u64 {
        self.packet_count
    }

    pub const fn read_count(self) -> u64 {
        self.read_count
    }

    pub const fn write_count(self) -> u64 {
        self.write_count
    }

    pub const fn bytes_read(self) -> u64 {
        self.bytes_read
    }

    pub const fn bytes_written(self) -> u64 {
        self.bytes_written
    }

    pub const fn first_tick(self) -> Option<u64> {
        self.first_tick
    }

    pub const fn last_tick(self) -> Option<u64> {
        self.last_tick
    }
}

pub(crate) fn checked_counter_add(
    counter: &'static str,
    value: u64,
    increment: u64,
) -> Result<u64, TrafficGeneratorError> {
    value
        .checked_add(increment)
        .ok_or(TrafficGeneratorError::CounterOverflow {
            counter,
            value,
            increment,
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TrafficRng {
    state: u64,
}

impl TrafficRng {
    const MULTIPLIER: u64 = 6364136223846793005;
    const INCREMENT: u64 = 1442695040888963407;

    pub(crate) const fn new(state: u64) -> Self {
        Self { state }
    }

    pub(crate) const fn state(&self) -> u64 {
        self.state
    }

    pub(crate) fn next_inclusive(&mut self, min: u64, max: u64) -> u64 {
        let value = self.peek_inclusive(min, max);
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        value
    }

    fn peek_inclusive(&self, min: u64, max: u64) -> u64 {
        if min == max {
            return min;
        }

        let width = max - min;
        match width.checked_add(1) {
            Some(span) => min + (self.state % span),
            None => self.state,
        }
    }
}
