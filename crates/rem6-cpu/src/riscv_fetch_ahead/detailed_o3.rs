use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_memory::{Address, AddressRange, MemoryRequestId};

use crate::{
    riscv_execute::oldest_completed_fetch_at,
    riscv_live_retire_window::{
        completed_fetch_instruction_from_events, completed_fetch_instruction_starting_with,
        RiscvCompletedFetchInstruction,
    },
    riscv_o3_window_policy::{RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision},
    riscv_scalar_memory_window::{
        independent_scalar_load_destination, store_range_extends_overlap_prefix,
    },
    CpuFetchEvent, CpuFetchEventKind, RiscvCoreState,
};

pub(super) enum DetailedFetchAheadCandidate {
    NotApplicable,
    Blocked,
    Ready(Address),
    ReadyPredictedControl {
        request: MemoryRequestId,
        pc: Address,
        sequential_pc: Address,
        instruction: RiscvInstruction,
        target_authority: PredictedControlTargetAuthority,
    },
    ReadyCachedTranslatedLoad {
        pc: Address,
        fetch_request: MemoryRequestId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PredictedControlTargetAuthority {
    Normal,
    RasRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TranslatedMemoryFetchAhead {
    Disabled,
    CachedMemory,
}

pub(super) enum ScalarMemoryFetchAheadHead {
    Untranslated,
    CachedTranslatedLoad { destination: Register },
}

pub(super) fn allows_detailed_memory_head_fetch_ahead(
    state: &RiscvCoreState,
    fetch_request: rem6_memory::MemoryRequestId,
    instruction: RiscvInstruction,
    translated: TranslatedMemoryFetchAhead,
) -> bool {
    scalar_memory_fetch_ahead_head(state, fetch_request, instruction, translated).is_some()
}

pub(super) fn scalar_memory_fetch_ahead_head(
    state: &RiscvCoreState,
    fetch_request: rem6_memory::MemoryRequestId,
    instruction: RiscvInstruction,
    translated: TranslatedMemoryFetchAhead,
) -> Option<ScalarMemoryFetchAheadHead> {
    if !state.live_retire_gate.detailed_policy_enabled() {
        return None;
    }
    if state.data_translation.is_none() {
        return state
            .cacheable_scalar_memory_instruction_range(instruction)
            .is_some()
            .then_some(ScalarMemoryFetchAheadHead::Untranslated);
    }
    if translated == TranslatedMemoryFetchAhead::Disabled {
        return None;
    }
    let destination = independent_scalar_load_destination(instruction, [])?;
    state
        .cacheable_cached_translated_scalar_load(instruction, fetch_request)
        .then_some(ScalarMemoryFetchAheadHead::CachedTranslatedLoad { destination })
}

pub(super) fn scalar_memory_has_younger_fetch(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    memory_request: rem6_memory::MemoryRequestId,
    younger_pc: Address,
    instruction: RiscvInstruction,
    translated: TranslatedMemoryFetchAhead,
) -> bool {
    allows_detailed_memory_head_fetch_ahead(state, memory_request, instruction, translated)
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
    translated: TranslatedMemoryFetchAhead,
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

    allows_detailed_memory_head_fetch_ahead(
        state,
        memory.request_id(),
        decoded.instruction(),
        translated,
    ) && super::has_pending_younger_fetch(state, fetch_events, memory.request_id().sequence())
}

pub(super) fn cached_translated_scalar_load_head_physical_range(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Option<(MemoryRequestId, AddressRange)> {
    let architectural = Address::new(state.hart.pc());
    let memory = fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.pc() == architectural
                && !state.executed_fetches.contains(&event.request_id())
        })
        .min_by_key(|event| event.request_id().sequence())?;
    let current =
        completed_fetch_instruction_starting_with(&state.executed_fetches, fetch_events, memory)?;
    let fetch_request = current.last_consumed_request();
    state
        .cached_translated_scalar_load_physical_range(
            current.decoded().instruction(),
            fetch_request,
        )
        .map(|range| (fetch_request, range))
}

pub(super) fn additional_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    completed: &[&CpuFetchEvent],
    translated: TranslatedMemoryFetchAhead,
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
    let scalar_memory_head = scalar_memory_fetch_ahead_head(
        state,
        current.last_consumed_request(),
        current.decoded().instruction(),
        translated,
    );
    match scalar_memory_head {
        Some(ScalarMemoryFetchAheadHead::Untranslated) => {
            return scalar_memory_window_candidate(state, fetch_events, &current);
        }
        Some(ScalarMemoryFetchAheadHead::CachedTranslatedLoad { destination }) => {
            let candidate =
                translated_scalar_load_window_candidate(state, fetch_events, &current, destination);
            return match candidate {
                DetailedFetchAheadCandidate::Ready(pc) => {
                    DetailedFetchAheadCandidate::ReadyCachedTranslatedLoad {
                        pc,
                        fetch_request: current.last_consumed_request(),
                    }
                }
                candidate => candidate,
            };
        }
        None => {}
    }
    let Some(window) = RiscvScalarIntegerLiveWindow::from_fu_head(current.decoded().instruction())
    else {
        return DetailedFetchAheadCandidate::NotApplicable;
    };
    scalar_integer_fu_window_candidate(state, fetch_events, &current, window)
}

fn translated_scalar_load_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current: &RiscvCompletedFetchInstruction,
    destination: Register,
) -> DetailedFetchAheadCandidate {
    let Some(window) = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
        [destination],
        1,
        state.o3_runtime.scalar_memory_window_limit(),
    ) else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    scalar_integer_fu_window_candidate(state, fetch_events, current, window)
}

fn scalar_integer_fu_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current: &RiscvCompletedFetchInstruction,
    window: RiscvScalarIntegerLiveWindow,
) -> DetailedFetchAheadCandidate {
    let previous_request = current.last_consumed_request();
    let next_pc = Address::new(
        current
            .pc()
            .get()
            .wrapping_add(u64::from(current.decoded().bytes())),
    );
    scalar_integer_window_candidate_from(state, fetch_events, previous_request, next_pc, window)
}

fn scalar_integer_window_candidate_from(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    mut previous_request: rem6_memory::MemoryRequestId,
    mut next_pc: Address,
    mut window: RiscvScalarIntegerLiveWindow,
) -> DetailedFetchAheadCandidate {
    while !window.is_full() {
        let younger = match completed_window_instruction_or_candidate(
            state,
            fetch_events,
            previous_request,
            next_pc,
        ) {
            Ok(younger) => younger,
            Err(candidate) => return candidate,
        };
        let decision = window.classify_younger(younger.decoded().instruction());
        match decision {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => {}
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
                let target_authority = if matches!(
                    decision,
                    RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
                ) {
                    PredictedControlTargetAuthority::RasRequired
                } else {
                    PredictedControlTargetAuthority::Normal
                };
                let prediction_request = younger.first_consumed_request();
                previous_request = younger.last_consumed_request();
                let sequential_pc = Address::new(
                    younger
                        .pc()
                        .get()
                        .wrapping_add(u64::from(younger.decoded().bytes())),
                );
                next_pc = match recorded_predicted_pc(state, prediction_request, sequential_pc) {
                    Some(predicted_pc) => predicted_pc,
                    None if state.branch_speculations.len() < state.branch_lookahead => {
                        return DetailedFetchAheadCandidate::ReadyPredictedControl {
                            request: prediction_request,
                            pc: younger.pc(),
                            sequential_pc,
                            instruction: younger.decoded().instruction(),
                            target_authority,
                        };
                    }
                    None => return DetailedFetchAheadCandidate::Blocked,
                };
                continue;
            }
            RiscvScalarIntegerYoungerDecision::AdmitStop
            | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
            | RiscvScalarIntegerYoungerDecision::Reject => {
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
    let mut store_ranges = window.store_ranges();
    let mut destinations = window.load_destinations().to_vec();
    if !admit_scalar_memory_prefix_instruction(
        state,
        current.decoded().instruction(),
        &mut store_ranges,
        &mut destinations,
    ) {
        return DetailedFetchAheadCandidate::Blocked;
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
        let next = match completed_window_instruction_or_candidate(
            state,
            fetch_events,
            previous_request,
            next_pc,
        ) {
            Ok(next) => next,
            Err(candidate) => return candidate,
        };
        let instruction = next.decoded().instruction();
        if matches!(
            instruction,
            RiscvInstruction::Load { .. } | RiscvInstruction::Store { .. }
        ) {
            if !admit_scalar_memory_prefix_instruction(
                state,
                instruction,
                &mut store_ranges,
                &mut destinations,
            ) {
                return DetailedFetchAheadCandidate::Blocked;
            }
            window_rows += 1;
            previous_request = next.last_consumed_request();
            next_pc = Address::new(
                next.pc()
                    .get()
                    .wrapping_add(u64::from(next.decoded().bytes())),
            );
            continue;
        }

        let Some(mut alu_window) = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            destinations.iter().copied(),
            window_rows,
            limit,
        ) else {
            return DetailedFetchAheadCandidate::Blocked;
        };
        let decision = alu_window.classify_younger(next.decoded().instruction());
        match decision {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => {
                let previous_request = next.last_consumed_request();
                let next_pc = Address::new(
                    next.pc()
                        .get()
                        .wrapping_add(u64::from(next.decoded().bytes())),
                );
                return scalar_integer_window_candidate_from(
                    state,
                    fetch_events,
                    previous_request,
                    next_pc,
                    alu_window,
                );
            }
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
                let target_authority = if matches!(
                    decision,
                    RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
                ) {
                    PredictedControlTargetAuthority::RasRequired
                } else {
                    PredictedControlTargetAuthority::Normal
                };
                let prediction_request = next.first_consumed_request();
                let previous_request = next.last_consumed_request();
                let sequential_pc = Address::new(
                    next.pc()
                        .get()
                        .wrapping_add(u64::from(next.decoded().bytes())),
                );
                let next_pc = match recorded_predicted_pc(state, prediction_request, sequential_pc)
                {
                    Some(predicted_pc) => predicted_pc,
                    None if state.branch_speculations.len() < state.branch_lookahead => {
                        return DetailedFetchAheadCandidate::ReadyPredictedControl {
                            request: prediction_request,
                            pc: next.pc(),
                            sequential_pc,
                            instruction: next.decoded().instruction(),
                            target_authority,
                        };
                    }
                    None => return DetailedFetchAheadCandidate::Blocked,
                };
                return scalar_integer_window_candidate_from(
                    state,
                    fetch_events,
                    previous_request,
                    next_pc,
                    alu_window,
                );
            }
            RiscvScalarIntegerYoungerDecision::AdmitStop
            | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
            | RiscvScalarIntegerYoungerDecision::Reject => {
                return DetailedFetchAheadCandidate::Blocked;
            }
        }
    }
}

fn admit_scalar_memory_prefix_instruction(
    state: &RiscvCoreState,
    instruction: RiscvInstruction,
    store_ranges: &mut Vec<AddressRange>,
    destinations: &mut Vec<Register>,
) -> bool {
    let Some(range) = state.cacheable_scalar_memory_instruction_range(instruction) else {
        return false;
    };
    match instruction {
        instruction @ RiscvInstruction::Load { .. } => {
            let Some(destination) =
                independent_scalar_load_destination(instruction, destinations.iter().copied())
            else {
                return false;
            };
            destinations.push(destination);
            true
        }
        RiscvInstruction::Store { .. }
            if destinations.is_empty()
                && (store_ranges.is_empty()
                    || store_range_extends_overlap_prefix(store_ranges.iter().copied(), range)) =>
        {
            store_ranges.push(range);
            true
        }
        _ => false,
    }
}

pub(crate) fn recorded_predicted_pc(
    state: &RiscvCoreState,
    request: MemoryRequestId,
    sequential_pc: Address,
) -> Option<Address> {
    let speculation = state.branch_speculations.get(&request.sequence())?;
    let pending = state.branch_predictor.pending_speculation(*speculation)?;
    if pending.predicted_taken() {
        pending.target()
    } else {
        Some(sequential_pc)
    }
}

fn completed_window_instruction_or_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    previous_request: rem6_memory::MemoryRequestId,
    pc: Address,
) -> Result<RiscvCompletedFetchInstruction, DetailedFetchAheadCandidate> {
    let has_completed_prefix =
        oldest_completed_fetch_at(&state.executed_fetches, fetch_events, previous_request, pc)
            .is_some();
    completed_fetch_instruction_from_events(
        &state.executed_fetches,
        fetch_events,
        previous_request,
        pc,
    )
    .ok_or_else(|| {
        if has_completed_prefix
            || has_live_younger_fetch_at(state, fetch_events, previous_request, pc)
        {
            DetailedFetchAheadCandidate::Blocked
        } else {
            DetailedFetchAheadCandidate::Ready(pc)
        }
    })
}
