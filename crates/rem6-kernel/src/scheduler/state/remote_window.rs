use crate::scheduler::PartitionId;
use crate::Tick;

use super::{ParallelEpochPlan, PartitionFrontier, ReadyPartition};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParallelRemoteDeliveryWindow {
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    target_now: Tick,
    minimum_delivery_tick: Option<Tick>,
    first_accepted_delivery_tick: Option<Tick>,
    horizon: Tick,
}

impl ParallelRemoteDeliveryWindow {
    pub const fn new(
        source: PartitionId,
        target: PartitionId,
        source_tick: Tick,
        target_now: Tick,
        minimum_delivery_tick: Option<Tick>,
        horizon: Tick,
    ) -> Self {
        let first_accepted_delivery_tick = match minimum_delivery_tick {
            Some(tick) if tick < target_now => Some(target_now),
            Some(tick) => Some(tick),
            None => None,
        };
        Self {
            source,
            target,
            source_tick,
            target_now,
            minimum_delivery_tick,
            first_accepted_delivery_tick,
            horizon,
        }
    }

    pub const fn source(self) -> PartitionId {
        self.source
    }

    pub const fn target(self) -> PartitionId {
        self.target
    }

    pub const fn source_tick(self) -> Tick {
        self.source_tick
    }

    pub const fn target_now(self) -> Tick {
        self.target_now
    }

    pub const fn minimum_delivery_tick(self) -> Option<Tick> {
        self.minimum_delivery_tick
    }

    pub const fn first_accepted_delivery_tick(self) -> Option<Tick> {
        self.first_accepted_delivery_tick
    }

    pub const fn horizon(self) -> Tick {
        self.horizon
    }

    pub const fn can_deliver_in_epoch(self) -> bool {
        match self.first_accepted_delivery_tick {
            Some(tick) => tick <= self.horizon,
            None => false,
        }
    }
}

impl ParallelEpochPlan {
    pub fn remote_delivery_windows(&self) -> &[ParallelRemoteDeliveryWindow] {
        &self.remote_delivery_windows
    }

    pub fn remote_delivery_window(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> Option<ParallelRemoteDeliveryWindow> {
        self.remote_delivery_windows
            .iter()
            .copied()
            .find(|window| window.source() == source && window.target() == target)
    }

    pub fn remote_delivery_windows_for_source(
        &self,
        source: PartitionId,
    ) -> Vec<ParallelRemoteDeliveryWindow> {
        self.remote_delivery_windows
            .iter()
            .copied()
            .filter(|window| window.source() == source)
            .collect()
    }
}

pub(super) fn collect_parallel_remote_delivery_windows(
    horizon: Tick,
    ready_partitions: &[ReadyPartition],
    frontiers: &[PartitionFrontier],
) -> Vec<ParallelRemoteDeliveryWindow> {
    let mut windows = Vec::new();
    for ready in ready_partitions {
        let Some(source_frontier) = frontiers
            .iter()
            .find(|frontier| frontier.partition() == ready.partition)
        else {
            continue;
        };
        let min_remote_delay = source_frontier
            .safe_until()
            .saturating_sub(source_frontier.now());
        for target_frontier in frontiers {
            if target_frontier.partition() == ready.partition {
                continue;
            }
            windows.push(ParallelRemoteDeliveryWindow::new(
                ready.partition,
                target_frontier.partition(),
                ready.next_tick,
                target_frontier.now(),
                ready.next_tick.checked_add(min_remote_delay),
                horizon,
            ));
        }
    }
    windows
}
