use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::Address;

use crate::{
    riscv_execute::oldest_completed_fetch_at,
    riscv_live_retire_window::{
        completed_fetch_instruction_from_events, completed_fetch_instruction_starting_with,
        RiscvCompletedFetchInstruction, ScalarIntegerFuLiveWindow, ScalarIntegerFuYoungerBoundary,
    },
    riscv_scalar_memory_window::independent_scalar_load_destination,
    CpuFetchEvent, CpuFetchEventKind, RiscvCoreState,
};

pub(super) enum DetailedFetchAheadCandidate {
    NotApplicable,
    Blocked,
    Ready(Address),
}

pub(super) fn allows_scalar_memory_fetch_ahead(
    state: &RiscvCoreState,
    instruction: RiscvInstruction,
) -> bool {
    state.live_retire_gate.detailed_policy_enabled()
        && state.data_translation.is_none()
        && state
            .cacheable_scalar_memory_instruction_range(instruction)
            .is_some()
}

pub(super) fn scalar_memory_has_younger_fetch(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    memory_request: rem6_memory::MemoryRequestId,
    younger_pc: Address,
    instruction: RiscvInstruction,
) -> bool {
    allows_scalar_memory_fetch_ahead(state, instruction)
        && has_live_younger_fetch_at(state, fetch_events, memory_request, younger_pc)
}

fn has_live_younger_fetch_at(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    memory_request: rem6_memory::MemoryRequestId,
    pc: Address,
) -> bool {
    fetch_events.iter().any(|event| {
        event.pc() == pc
            && event.request_id().agent() == memory_request.agent()
            && event.request_id().sequence() > memory_request.sequence()
            && !state.executed_fetches.contains(&event.request_id())
            && match event.kind() {
                CpuFetchEventKind::Completed => true,
                CpuFetchEventKind::Issued => {
                    !super::fetch_request_has_response(fetch_events, event)
                }
                CpuFetchEventKind::Retry | CpuFetchEventKind::Failed => false,
            }
    })
}

pub(super) fn scalar_memory_waits_for_younger_fetch(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> bool {
    let architectural = Address::new(state.hart.pc());
    let Some(memory) = fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.pc() == architectural
                && !state.executed_fetches.contains(&event.request_id())
        })
        .min_by_key(|event| event.request_id().sequence())
    else {
        return false;
    };
    let Some([a, b, c, d]) = memory.data() else {
        return false;
    };
    let Ok(decoded) = RiscvInstruction::decode_with_length(u32::from_le_bytes([*a, *b, *c, *d]))
    else {
        return false;
    };
    if state.can_overlap_detailed_scalar_memory_instruction(decoded.instruction()) {
        return false;
    }

    allows_scalar_memory_fetch_ahead(state, decoded.instruction())
        && super::has_pending_younger_fetch(state, fetch_events, memory.request_id().sequence())
}

pub(super) fn additional_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    completed: &[&CpuFetchEvent],
) -> DetailedFetchAheadCandidate {
    if !state.live_retire_gate.detailed_policy_enabled() {
        return DetailedFetchAheadCandidate::NotApplicable;
    }
    let architectural = Address::new(state.hart.pc());
    let Some(current) = completed
        .iter()
        .copied()
        .find(|event| event.pc() == architectural)
    else {
        return DetailedFetchAheadCandidate::NotApplicable;
    };
    let Some(current) =
        completed_fetch_instruction_starting_with(&state.executed_fetches, fetch_events, current)
    else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    let scalar_memory_window =
        matches!(
            current.decoded().instruction(),
            RiscvInstruction::Load { .. } | RiscvInstruction::Store { .. }
        ) && allows_scalar_memory_fetch_ahead(state, current.decoded().instruction());
    if scalar_memory_window {
        return scalar_memory_window_candidate(state, fetch_events, &current);
    }
    let Some(window) = ScalarIntegerFuLiveWindow::new(current.decoded().instruction()) else {
        return DetailedFetchAheadCandidate::NotApplicable;
    };
    scalar_integer_fu_window_candidate(state, fetch_events, &current, window)
}

fn scalar_integer_fu_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current: &RiscvCompletedFetchInstruction,
    mut window: ScalarIntegerFuLiveWindow,
) -> DetailedFetchAheadCandidate {
    let mut previous_request = current.last_consumed_request();
    let mut next_pc = Address::new(
        current
            .pc()
            .get()
            .wrapping_add(u64::from(current.decoded().bytes())),
    );
    while !window.is_full() {
        let has_completed_prefix = oldest_completed_fetch_at(
            &state.executed_fetches,
            fetch_events,
            previous_request,
            next_pc,
        )
        .is_some();
        let Some(younger) = completed_fetch_instruction_from_events(
            &state.executed_fetches,
            fetch_events,
            previous_request,
            next_pc,
        ) else {
            return if has_completed_prefix
                || has_live_younger_fetch_at(state, fetch_events, previous_request, next_pc)
            {
                DetailedFetchAheadCandidate::Blocked
            } else {
                DetailedFetchAheadCandidate::Ready(next_pc)
            };
        };
        match window.classify_younger(younger.decoded().instruction()) {
            ScalarIntegerFuYoungerBoundary::Continue => {}
            ScalarIntegerFuYoungerBoundary::StopAfter | ScalarIntegerFuYoungerBoundary::Reject => {
                return DetailedFetchAheadCandidate::Blocked;
            }
        }

        previous_request = younger.last_consumed_request();
        next_pc = Address::new(
            younger
                .pc()
                .get()
                .wrapping_add(u64::from(younger.decoded().bytes())),
        );
    }

    DetailedFetchAheadCandidate::Blocked
}

fn scalar_memory_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current: &RiscvCompletedFetchInstruction,
) -> DetailedFetchAheadCandidate {
    let limit = state.o3_runtime.scalar_memory_window_limit();
    let Some(window) = state.o3_runtime.scalar_memory_fetch_window_state() else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    let mut destinations = window.load_destinations().to_vec();
    match current.decoded().instruction() {
        instruction @ RiscvInstruction::Load { .. } => {
            let Some(current_destination) =
                independent_scalar_load_destination(instruction, destinations.iter().copied())
            else {
                return DetailedFetchAheadCandidate::Blocked;
            };
            destinations.push(current_destination);
        }
        RiscvInstruction::Store { .. } if window.rows() == 0 => {}
        _ => return DetailedFetchAheadCandidate::Blocked,
    }
    let mut previous_request = current.last_consumed_request();
    let mut next_pc = Address::new(
        current
            .pc()
            .get()
            .wrapping_add(u64::from(current.decoded().bytes())),
    );
    let mut window_rows = window.rows().saturating_add(1);

    loop {
        if window_rows >= limit {
            return DetailedFetchAheadCandidate::Blocked;
        }
        let has_completed_prefix = oldest_completed_fetch_at(
            &state.executed_fetches,
            fetch_events,
            previous_request,
            next_pc,
        )
        .is_some();
        let Some(next) = completed_fetch_instruction_from_events(
            &state.executed_fetches,
            fetch_events,
            previous_request,
            next_pc,
        ) else {
            if has_completed_prefix
                || has_live_younger_fetch_at(state, fetch_events, previous_request, next_pc)
            {
                return DetailedFetchAheadCandidate::Blocked;
            }
            return DetailedFetchAheadCandidate::Ready(next_pc);
        };
        let Some(destination) = independent_scalar_load_destination(
            next.decoded().instruction(),
            destinations.iter().copied(),
        ) else {
            return DetailedFetchAheadCandidate::Blocked;
        };
        if state
            .cacheable_scalar_memory_instruction_range(next.decoded().instruction())
            .is_none()
        {
            return DetailedFetchAheadCandidate::Blocked;
        }
        destinations.push(destination);
        window_rows += 1;
        previous_request = next.last_consumed_request();
        next_pc = Address::new(
            next.pc()
                .get()
                .wrapping_add(u64::from(next.decoded().bytes())),
        );
    }
}
