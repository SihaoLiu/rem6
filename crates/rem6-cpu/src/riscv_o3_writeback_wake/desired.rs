use rem6_kernel::Tick;

use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvO3WritebackWakeDemand {
    pub(super) desired_tick: Option<Tick>,
    pub(super) allow_current: bool,
}

pub(super) fn desired_o3_writeback_wake(
    state: &RiscvCoreState,
    now: Tick,
    translated_result_pair: Option<Tick>,
) -> RiscvO3WritebackWakeDemand {
    let memory_result = state
        .o3_runtime
        .earliest_unpublished_memory_result_writeback_tick();
    let pending_address = state.o3_runtime.pending_data_address_wake_tick();
    let live_issue = state
        .o3_runtime
        .live_issue_service_tick()
        .map(|tick| tick.max(now));
    let live_gate_ready_tick = state.live_retire_gate.pending_ready_tick();
    let restored_live_gate = state
        .live_retire_gate
        .owned_scheduler_wakes()
        .is_empty()
        .then_some(live_gate_ready_tick)
        .flatten();
    let forwarded_control = state
        .o3_runtime
        .producer_forwarded_control_target()
        .filter(|forwarded| {
            !state
                .branch_speculations
                .contains_key(&forwarded.fetch_request().sequence())
        })
        .map(|forwarded| forwarded.ready_tick().max(now));
    let translated_result_retry = state.translated_result_pair_retry_wake_tick(now);
    let desired_tick = [
        memory_result,
        pending_address,
        live_issue,
        restored_live_gate,
        forwarded_control,
        translated_result_pair,
        translated_result_retry,
    ]
    .into_iter()
    .flatten()
    .min();
    let allow_current = [
        pending_address,
        live_issue,
        restored_live_gate,
        forwarded_control,
    ]
    .into_iter()
    .flatten()
    .any(|tick| tick == now);
    RiscvO3WritebackWakeDemand {
        desired_tick,
        allow_current,
    }
}
