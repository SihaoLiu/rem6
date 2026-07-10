use rem6_memory::Address;

use crate::{
    o3_fu_latency::o3_fu_latency_class,
    o3_runtime::{o3_scalar_integer_destination, o3_speculative_scalar_alu_operands},
    o3_runtime_trace::O3RuntimeFuLatencyClass,
    riscv_execute::oldest_completed_fetch_at,
    riscv_live_retire_window::{
        completed_fetch_instruction_from_events, completed_fetch_instruction_starting_with,
    },
    CpuFetchEvent, RiscvCoreState,
};

pub(super) enum ThirdFetchCandidate {
    NotApplicable,
    Blocked,
    Ready(Address),
}

pub(super) fn third_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    completed: &[&CpuFetchEvent],
) -> ThirdFetchCandidate {
    if !state.live_retire_gate.detailed_policy_enabled() {
        return ThirdFetchCandidate::NotApplicable;
    }
    let architectural = Address::new(state.hart.pc());
    let Some(current) = completed
        .iter()
        .copied()
        .find(|event| event.pc() == architectural)
    else {
        return ThirdFetchCandidate::NotApplicable;
    };
    let Some(current) =
        completed_fetch_instruction_starting_with(&state.executed_fetches, fetch_events, current)
    else {
        return ThirdFetchCandidate::Blocked;
    };
    if !matches!(
        o3_fu_latency_class(current.decoded().instruction()),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    ) {
        return ThirdFetchCandidate::NotApplicable;
    }
    let younger_pc = Address::new(
        current
            .pc()
            .get()
            .wrapping_add(u64::from(current.decoded().bytes())),
    );
    let current_request = current.last_consumed_request();
    let has_younger_prefix = oldest_completed_fetch_at(
        &state.executed_fetches,
        fetch_events,
        current_request,
        younger_pc,
    )
    .is_some();
    let Some(younger) = completed_fetch_instruction_from_events(
        &state.executed_fetches,
        fetch_events,
        current_request,
        younger_pc,
    ) else {
        return if has_younger_prefix {
            ThirdFetchCandidate::Blocked
        } else {
            ThirdFetchCandidate::NotApplicable
        };
    };
    let Some((_destination, sources)) =
        o3_speculative_scalar_alu_operands(younger.decoded().instruction())
    else {
        return ThirdFetchCandidate::Blocked;
    };
    let current_destination = o3_scalar_integer_destination(current.decoded().instruction());
    if current_destination.is_some_and(|destination| {
        !destination.is_zero() && sources.iter().any(|source| *source == destination)
    }) {
        return ThirdFetchCandidate::Blocked;
    }
    let third_pc = Address::new(
        younger
            .pc()
            .get()
            .wrapping_add(u64::from(younger.decoded().bytes())),
    );
    if oldest_completed_fetch_at(
        &state.executed_fetches,
        fetch_events,
        younger.last_consumed_request(),
        third_pc,
    )
    .is_some()
    {
        return ThirdFetchCandidate::Blocked;
    }
    ThirdFetchCandidate::Ready(third_pc)
}
