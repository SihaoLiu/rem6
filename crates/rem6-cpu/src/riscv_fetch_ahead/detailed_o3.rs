use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_memory::{Address, AddressRange, MemoryRequestId};

use crate::{
    riscv_execute::oldest_completed_fetch_at,
    riscv_live_retire_window::{
        completed_fetch_instruction_from_events, completed_fetch_instruction_starting_with,
        RiscvCompletedFetchInstruction,
    },
    riscv_o3_window_policy::{
        RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision,
        RiscvSequencedScalarIntegerYoungerDecision,
    },
    riscv_scalar_memory_window::{
        independent_scalar_load_destination, store_range_extends_overlap_prefix,
    },
    BranchTargetKind, CpuFetchEvent, CpuFetchEventKind, ReturnAddressStackOperation,
    ReturnAddressStackOperationKind, RiscvCoreState,
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
pub(crate) enum RequiredRasConsumer {
    Pop,
    PopThenPush { pushed_address: Address },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PredictedControlTargetAuthority {
    Normal,
    RasRequired {
        push_sequence: u64,
        pushed_address: Address,
        consumer: RequiredRasConsumer,
    },
}

pub(crate) fn predicted_control_target_authority(
    instruction: RiscvInstruction,
    sequential_pc: Address,
    classification: RiscvSequencedScalarIntegerYoungerDecision,
    sequenced_return_addresses: &[(u64, Address)],
) -> Option<PredictedControlTargetAuthority> {
    match classification.decision() {
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl => {
            return Some(PredictedControlTargetAuthority::Normal);
        }
        RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {}
        RiscvScalarIntegerYoungerDecision::AdmitContinue
        | RiscvScalarIntegerYoungerDecision::AdmitStop
        | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        | RiscvScalarIntegerYoungerDecision::Reject => return None,
    }
    let push_sequence = classification.ras_push_sequence()?;
    let pushed_address = sequenced_return_addresses
        .iter()
        .rev()
        .find_map(|(sequence, address)| (*sequence == push_sequence).then_some(*address))?;
    let consumer = match super::return_address_stack_action(instruction, sequential_pc)? {
        super::ReturnAddressStackAction::Pop => RequiredRasConsumer::Pop,
        super::ReturnAddressStackAction::PopThenPush(pushed_address) => {
            RequiredRasConsumer::PopThenPush { pushed_address }
        }
        super::ReturnAddressStackAction::Push(_) => return None,
    };
    Some(PredictedControlTargetAuthority::RasRequired {
        push_sequence,
        pushed_address,
        consumer,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecordedPredictedPc {
    Missing,
    Invalid,
    Ready(Address),
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
    scalar_integer_window_candidate_from(
        state,
        fetch_events,
        previous_request,
        next_pc,
        window,
        Vec::new(),
    )
}

fn scalar_integer_window_candidate_from(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    mut previous_request: rem6_memory::MemoryRequestId,
    mut next_pc: Address,
    mut window: RiscvScalarIntegerLiveWindow,
    mut sequenced_return_addresses: Vec<(u64, Address)>,
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
        let prediction_request = younger.first_consumed_request();
        let sequential_pc = Address::new(
            younger
                .pc()
                .get()
                .wrapping_add(u64::from(younger.decoded().bytes())),
        );
        sequenced_return_addresses.push((prediction_request.sequence(), sequential_pc));
        let classification = window.classify_sequenced_younger(
            younger.decoded().instruction(),
            prediction_request.sequence(),
        );
        let decision = classification.decision();
        match decision {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => {}
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
                let Some(target_authority) = predicted_control_target_authority(
                    younger.decoded().instruction(),
                    sequential_pc,
                    classification,
                    &sequenced_return_addresses,
                ) else {
                    return DetailedFetchAheadCandidate::Blocked;
                };
                previous_request = younger.last_consumed_request();
                next_pc = match recorded_predicted_pc(
                    state,
                    prediction_request,
                    sequential_pc,
                    target_authority,
                ) {
                    RecordedPredictedPc::Ready(predicted_pc) => predicted_pc,
                    RecordedPredictedPc::Missing
                        if state.branch_speculations.len() < state.branch_lookahead =>
                    {
                        return DetailedFetchAheadCandidate::ReadyPredictedControl {
                            request: prediction_request,
                            pc: younger.pc(),
                            sequential_pc,
                            instruction: younger.decoded().instruction(),
                            target_authority,
                        };
                    }
                    RecordedPredictedPc::Missing | RecordedPredictedPc::Invalid => {
                        return DetailedFetchAheadCandidate::Blocked;
                    }
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
        next_pc = sequential_pc;
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
        let prediction_request = next.first_consumed_request();
        let sequential_pc = Address::new(
            next.pc()
                .get()
                .wrapping_add(u64::from(next.decoded().bytes())),
        );
        let sequenced_return_addresses = vec![(prediction_request.sequence(), sequential_pc)];
        let classification = alu_window.classify_sequenced_younger(
            next.decoded().instruction(),
            prediction_request.sequence(),
        );
        let decision = classification.decision();
        match decision {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => {
                let previous_request = next.last_consumed_request();
                return scalar_integer_window_candidate_from(
                    state,
                    fetch_events,
                    previous_request,
                    sequential_pc,
                    alu_window,
                    sequenced_return_addresses,
                );
            }
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
                let Some(target_authority) = predicted_control_target_authority(
                    next.decoded().instruction(),
                    sequential_pc,
                    classification,
                    &sequenced_return_addresses,
                ) else {
                    return DetailedFetchAheadCandidate::Blocked;
                };
                let previous_request = next.last_consumed_request();
                let next_pc = match recorded_predicted_pc(
                    state,
                    prediction_request,
                    sequential_pc,
                    target_authority,
                ) {
                    RecordedPredictedPc::Ready(predicted_pc) => predicted_pc,
                    RecordedPredictedPc::Missing
                        if state.branch_speculations.len() < state.branch_lookahead =>
                    {
                        return DetailedFetchAheadCandidate::ReadyPredictedControl {
                            request: prediction_request,
                            pc: next.pc(),
                            sequential_pc,
                            instruction: next.decoded().instruction(),
                            target_authority,
                        };
                    }
                    RecordedPredictedPc::Missing | RecordedPredictedPc::Invalid => {
                        return DetailedFetchAheadCandidate::Blocked;
                    }
                };
                return scalar_integer_window_candidate_from(
                    state,
                    fetch_events,
                    previous_request,
                    next_pc,
                    alu_window,
                    sequenced_return_addresses,
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
    target_authority: PredictedControlTargetAuthority,
) -> RecordedPredictedPc {
    let Some(speculation) = state.branch_speculations.get(&request.sequence()) else {
        return RecordedPredictedPc::Missing;
    };
    let Some(pending) = state.branch_predictor.pending_speculation(*speculation) else {
        return RecordedPredictedPc::Invalid;
    };
    match target_authority {
        PredictedControlTargetAuthority::Normal => {
            if pending.predicted_taken() {
                pending
                    .target()
                    .map_or(RecordedPredictedPc::Invalid, RecordedPredictedPc::Ready)
            } else {
                RecordedPredictedPc::Ready(sequential_pc)
            }
        }
        PredictedControlTargetAuthority::RasRequired {
            push_sequence,
            pushed_address,
            consumer,
        } => {
            let Some(target) = recorded_ras_required_target(
                state,
                push_sequence,
                pushed_address,
                consumer,
                request.sequence(),
            ) else {
                return RecordedPredictedPc::Invalid;
            };
            if pending.predicted_taken() && pending.target() == Some(target) {
                RecordedPredictedPc::Ready(target)
            } else {
                RecordedPredictedPc::Invalid
            }
        }
    }
}

fn ras_required_producer_matches(
    producer_kind: BranchTargetKind,
    operation: &ReturnAddressStackOperation,
    entries: usize,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
) -> bool {
    if operation.pushed_address() != Some(pushed_address) {
        return false;
    }
    let mut expected_after = operation.stack_before().to_vec();
    match (producer_kind, operation.kind(), consumer) {
        (
            BranchTargetKind::CallDirect | BranchTargetKind::CallIndirect,
            ReturnAddressStackOperationKind::Push,
            _,
        ) => {
            if operation.predicted_return().is_some() {
                return false;
            }
        }
        (
            BranchTargetKind::Return,
            ReturnAddressStackOperationKind::PopThenPush,
            RequiredRasConsumer::Pop,
        ) => {
            let Some(predicted_return) = expected_after.pop() else {
                return false;
            };
            if operation.predicted_return() != Some(predicted_return) {
                return false;
            }
        }
        _ => return false,
    }
    if entries == 0 || expected_after.len() > entries {
        return false;
    }
    if expected_after.len() == entries {
        expected_after.remove(0);
    }
    expected_after.push(pushed_address);
    operation.stack_after() == expected_after
}

pub(super) fn unconsumed_ras_required_target(
    state: &RiscvCoreState,
    push_sequence: u64,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
) -> Option<Address> {
    let producer_kind = *state.branch_speculation_kinds.get(&push_sequence)?;
    let operation_id = state.return_address_stack_operations.get(&push_sequence)?;
    let operation = state.return_address_stack.pending_operations().last()?;
    if operation.id() != *operation_id
        || !ras_required_producer_matches(
            producer_kind,
            operation,
            state.return_address_stack.config().entries(),
            pushed_address,
            consumer,
        )
        || operation.stack_after() != state.return_address_stack.stack_entries()
    {
        return None;
    }
    (state.return_address_stack.top() == Some(pushed_address)).then_some(pushed_address)
}

fn recorded_ras_required_target(
    state: &RiscvCoreState,
    push_sequence: u64,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
    return_sequence: u64,
) -> Option<Address> {
    let producer_kind = *state.branch_speculation_kinds.get(&push_sequence)?;
    if state.branch_speculation_kinds.get(&return_sequence) != Some(&BranchTargetKind::Return) {
        return None;
    }
    let producer_id = state.return_address_stack_operations.get(&push_sequence)?;
    let consumer_id = state
        .return_address_stack_operations
        .get(&return_sequence)?;
    let operations = state.return_address_stack.pending_operations();
    let producer_index = operations
        .iter()
        .position(|operation| operation.id() == *producer_id)?;
    let consumer_index = operations
        .iter()
        .position(|operation| operation.id() == *consumer_id)?;
    if consumer_index != producer_index + 1 {
        return None;
    }
    let producer = &operations[producer_index];
    let consumer_operation = &operations[consumer_index];
    if !ras_required_producer_matches(
        producer_kind,
        producer,
        state.return_address_stack.config().entries(),
        pushed_address,
        consumer,
    ) || producer.stack_after() != consumer_operation.stack_before()
    {
        return None;
    }
    let mut expected_after = consumer_operation.stack_before().to_vec();
    let consumed_address = expected_after.pop()?;
    if consumed_address != pushed_address
        || consumer_operation.predicted_return() != Some(consumed_address)
    {
        return None;
    }
    match consumer {
        RequiredRasConsumer::Pop => {
            if consumer_operation.kind() != ReturnAddressStackOperationKind::Pop
                || consumer_operation.pushed_address().is_some()
            {
                return None;
            }
        }
        RequiredRasConsumer::PopThenPush { pushed_address } => {
            if consumer_operation.kind() != ReturnAddressStackOperationKind::PopThenPush
                || consumer_operation.pushed_address() != Some(pushed_address)
            {
                return None;
            }
            expected_after.push(pushed_address);
        }
    }
    if consumer_operation.stack_after() != expected_after {
        return None;
    }
    Some(consumed_address)
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
