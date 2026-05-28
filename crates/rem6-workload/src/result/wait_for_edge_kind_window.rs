use rem6_kernel::{Tick, WaitForEdgeKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadWaitForEdgeKindWindow {
    kind: WaitForEdgeKind,
    edge_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl WorkloadWaitForEdgeKindWindow {
    pub const fn new(
        kind: WaitForEdgeKind,
        edge_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Self {
        let stored_first_tick = if first_tick <= last_tick {
            first_tick
        } else {
            last_tick
        };
        let stored_last_tick = if first_tick <= last_tick {
            last_tick
        } else {
            first_tick
        };
        Self {
            kind,
            edge_count,
            first_tick: stored_first_tick,
            last_tick: stored_last_tick,
        }
    }

    pub const fn kind(&self) -> WaitForEdgeKind {
        self.kind
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub const fn is_empty(&self) -> bool {
        self.edge_count == 0
    }

    pub(crate) fn merge(&mut self, other: Self) {
        debug_assert_eq!(self.kind, other.kind);
        self.edge_count = self.edge_count.saturating_add(other.edge_count);
        self.first_tick = self.first_tick.min(other.first_tick);
        self.last_tick = self.last_tick.max(other.last_tick);
    }
}
