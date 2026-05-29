use crate::Tick;

use super::{PartitionId, SchedulerError};

pub(super) fn remote_delivery_before_lookahead_error(
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    delivery_tick: Tick,
    min_remote_delay: Tick,
) -> Result<Option<SchedulerError>, SchedulerError> {
    let minimum_delivery_tick =
        source_tick
            .checked_add(min_remote_delay)
            .ok_or(SchedulerError::TickOverflow {
                now: source_tick,
                delay: min_remote_delay,
            })?;
    if delivery_tick < minimum_delivery_tick {
        Ok(Some(
            SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source,
                target,
                source_tick,
                delivery_tick,
                minimum_delivery_tick,
            },
        ))
    } else {
        Ok(None)
    }
}

pub(super) fn remote_event_delivery_key_values(
    target: PartitionId,
    delivery_tick: Tick,
    source: PartitionId,
    order: u64,
) -> (PartitionId, Tick, PartitionId, u64) {
    (target, delivery_tick, source, order)
}
