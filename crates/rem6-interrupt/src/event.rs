use rem6_kernel::{PartitionId, Tick};

use crate::{InterruptLineId, InterruptRoute, InterruptSourceId, InterruptTargetId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptEventKind {
    Assert,
    Deassert,
    Claim,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptDelivery {
    tick: Tick,
    source_partition: PartitionId,
    route: InterruptRoute,
    source: InterruptSourceId,
    kind: InterruptEventKind,
}

impl InterruptDelivery {
    pub const fn new(
        tick: Tick,
        source_partition: PartitionId,
        route: InterruptRoute,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Self {
        Self {
            tick,
            source_partition,
            route,
            source,
            kind,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn route(&self) -> InterruptRoute {
        self.route
    }

    pub const fn line(&self) -> InterruptLineId {
        self.route.line()
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.route.target()
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.route.target_partition()
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn kind(&self) -> InterruptEventKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptEvent {
    tick: Tick,
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    source: InterruptSourceId,
    kind: InterruptEventKind,
}

impl InterruptEvent {
    pub const fn new(
        tick: Tick,
        line: InterruptLineId,
        target: InterruptTargetId,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Self {
        Self::routed(
            tick,
            line,
            target,
            PartitionId::new(target.get()),
            source,
            kind,
        )
    }

    pub const fn routed(
        tick: Tick,
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Self {
        Self {
            tick,
            line,
            target,
            target_partition,
            source,
            kind,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn line(&self) -> InterruptLineId {
        self.line
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn kind(&self) -> InterruptEventKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingInterrupt {
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    source: InterruptSourceId,
    asserted_tick: Tick,
}

impl PendingInterrupt {
    pub const fn new(
        line: InterruptLineId,
        target: InterruptTargetId,
        source: InterruptSourceId,
        asserted_tick: Tick,
    ) -> Self {
        Self::routed(
            line,
            target,
            PartitionId::new(target.get()),
            source,
            asserted_tick,
        )
    }

    pub const fn routed(
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source: InterruptSourceId,
        asserted_tick: Tick,
    ) -> Self {
        Self {
            line,
            target,
            target_partition,
            source,
            asserted_tick,
        }
    }

    pub const fn line(&self) -> InterruptLineId {
        self.line
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn asserted_tick(&self) -> Tick {
        self.asserted_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptClaim {
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    source: InterruptSourceId,
    asserted_tick: Tick,
    claimed_tick: Tick,
}

impl InterruptClaim {
    pub const fn new(
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source: InterruptSourceId,
        asserted_tick: Tick,
        claimed_tick: Tick,
    ) -> Self {
        Self {
            line,
            target,
            target_partition,
            source,
            asserted_tick,
            claimed_tick,
        }
    }

    pub const fn line(self) -> InterruptLineId {
        self.line
    }

    pub const fn target(self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(self) -> PartitionId {
        self.target_partition
    }

    pub const fn source(self) -> InterruptSourceId {
        self.source
    }

    pub const fn asserted_tick(self) -> Tick {
        self.asserted_tick
    }

    pub const fn claimed_tick(self) -> Tick {
        self.claimed_tick
    }
}
