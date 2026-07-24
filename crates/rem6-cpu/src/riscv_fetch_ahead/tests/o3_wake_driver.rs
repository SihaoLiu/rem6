use super::*;

pub(super) fn fire_requested_o3_writeback_wakes(core: &RiscvCore) -> Vec<u64> {
    let mut now = 0;
    let mut fired = Vec::new();
    for _ in 0..16 {
        let Some(tick) = core.requested_o3_writeback_wake_tick(now) else {
            return fired;
        };
        core.mark_o3_writeback_wake_fired(tick);
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
