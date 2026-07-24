use super::*;

#[cfg(test)]
pub(in crate::o3_runtime) fn service_live_issue_queue_until_boundary_for_test(
    runtime: &mut O3RuntimeState,
    hart: &RiscvHartState,
    head: O3LiveIssueHeadReservation,
    earliest_tick: u64,
) -> Result<(), O3RuntimeError> {
    if !runtime
        .snapshot
        .reorder_buffer
        .iter()
        .any(|entry| entry.is_live_staged() && entry.sequence() == head.sequence())
        && !runtime.pending_data_address_has_producer_sequence(head.sequence())
    {
        return Ok(());
    }
    runtime.live_issue.request_service_at(earliest_tick);
    let mut tick = earliest_tick;
    let mut outcome = runtime.service_live_issue_scheduler_at(hart, tick)?;
    loop {
        if outcome.replay_boundary().is_some() {
            break;
        }
        let Some(next_tick) = outcome.next_service_tick() else {
            break;
        };
        if outcome.waits_for_pending_dependency() && next_tick > earliest_tick {
            break;
        }
        if runtime.pending_data_address_wake_tick() == Some(next_tick) {
            break;
        }
        tick = next_tick;
        outcome = runtime.service_live_issue_queue_at(hart, tick)?;
    }
    if runtime.live_issue_is_quiescent() {
        runtime.seal_live_issue_decision();
    }
    Ok(())
}
