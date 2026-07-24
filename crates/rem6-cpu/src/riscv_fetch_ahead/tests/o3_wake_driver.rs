use super::*;
use rem6_kernel::PartitionedScheduler;

pub(super) fn fire_requested_o3_writeback_wakes(core: &RiscvCore) -> Vec<u64> {
    let mut now = 0;
    let mut fired = Vec::new();
    for _ in 0..16 {
        let Some(tick) = core.requested_o3_writeback_wake_tick(now) else {
            return fired;
        };
        let mut scheduler = PartitionedScheduler::new(core.partition().index() + 1).unwrap();
        let wake_core = core.clone();
        let event = scheduler
            .schedule_at(core.partition(), tick, move |context| {
                wake_core.mark_o3_writeback_wake_fired(context.now());
            })
            .unwrap();
        core.mark_o3_writeback_wake_scheduled(
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(event).unwrap(),
        );
        assert_eq!(core.owned_o3_writeback_wakes().len(), 1);
        scheduler.run_until_idle_conservative();
        assert!(core.owned_o3_writeback_wakes().is_empty());
        now = tick;
        fired.push(tick);
    }
    panic!("O3 writeback wake test helper did not quiesce");
}

pub(super) fn record_prepared_fetch_ahead_speculation_and_fire_o3_wakes(
    core: &RiscvCore,
    prepared: Option<PreparedRiscvFetchAheadSpeculation>,
) -> Vec<u64> {
    core.record_prepared_fetch_ahead_speculation(prepared);
    fire_requested_o3_writeback_wakes(core)
}

pub(super) fn next_fetch_ahead_before_retire_after_o3_wake(
    core: &RiscvCore,
) -> Option<RiscvFetchAheadDecision> {
    core.next_fetch_ahead_before_retire().or_else(|| {
        fire_requested_o3_writeback_wakes(core);
        core.next_fetch_ahead_before_retire()
    })
}

pub(super) fn next_pending_data_fetch_ahead_after_o3_wake(
    core: &RiscvCore,
    pending_data_blocks_new_work: bool,
) -> Option<RiscvFetchAheadDecision> {
    core.next_pending_data_fetch_ahead(pending_data_blocks_new_work)
        .or_else(|| {
            fire_requested_o3_writeback_wakes(core);
            core.next_pending_data_fetch_ahead(pending_data_blocks_new_work)
        })
}
