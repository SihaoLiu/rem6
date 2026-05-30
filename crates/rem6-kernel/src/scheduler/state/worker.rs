use crate::scheduler::PartitionId;
use crate::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParallelWorkerRecord {
    lane: usize,
    partition: PartitionId,
    start_tick: Tick,
    safe_until: Tick,
    next_tick: Option<Tick>,
    pending_events: usize,
}

impl ParallelWorkerRecord {
    pub const fn new(
        lane: usize,
        partition: PartitionId,
        start_tick: Tick,
        safe_until: Tick,
        next_tick: Option<Tick>,
        pending_events: usize,
    ) -> Self {
        Self {
            lane,
            partition,
            start_tick,
            safe_until,
            next_tick,
            pending_events,
        }
    }

    pub const fn lane(self) -> usize {
        self.lane
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn start_tick(self) -> Tick {
        self.start_tick
    }

    pub const fn safe_until(self) -> Tick {
        self.safe_until
    }

    pub const fn next_tick(self) -> Option<Tick> {
        self.next_tick
    }

    pub const fn pending_events(self) -> usize {
        self.pending_events
    }

    pub const fn duration_ticks(self) -> Tick {
        self.safe_until.saturating_sub(self.start_tick)
    }
}
