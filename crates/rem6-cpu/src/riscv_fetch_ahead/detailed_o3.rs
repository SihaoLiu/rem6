use rem6_isa_riscv::{MemoryAccessKind, Register, RiscvInstruction};
use rem6_memory::{Address, MemoryRequestId};

use crate::{
    o3_runtime::{
        O3ProducerForwardedControlTarget, O3ProducerForwardedReturnDescendant,
        O3ProducerForwardedScalarChain,
    },
    riscv_branch_kind::riscv_branch_target_kind,
    riscv_execute::oldest_completed_fetch_at,
    riscv_live_retire_window::{
        completed_fetch_instruction_from_events, completed_fetch_instruction_starting_with,
        RiscvCompletedFetchInstruction,
    },
    riscv_o3_window_policy::{
        RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision,
        RiscvSequencedScalarIntegerYoungerDecision,
    },
    riscv_scalar_memory_window::independent_scalar_load_destination,
    BranchSpeculationId, BranchTargetKind, CpuFetchEvent, CpuFetchEventKind,
    ReturnAddressStackOperation, ReturnAddressStackOperationKind, RiscvCoreState,
};

use super::O3MemoryResultWindowRole;

#[path = "detailed_o3/data_access_result.rs"]
mod data_access_result;
#[path = "detailed_o3/data_access_result_effect_policy.rs"]
mod data_access_result_effect_policy;
#[path = "detailed_o3/data_access_result_pair_policy.rs"]
mod data_access_result_pair_policy;
#[path = "detailed_o3/data_access_result_translation.rs"]
mod data_access_result_translation;
#[path = "detailed_o3/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "detailed_o3/retained_data_access_result.rs"]
mod retained_data_access_result;

pub(super) use data_access_result::{
    data_access_result_fetch_ahead_authorization, data_access_result_window_candidate,
};
pub(super) use data_access_result_translation::{
    data_access_result_head_physical_probe, DataAccessResultHeadPhysicalProbe,
};
pub(super) use dependent_result_address::dependent_result_address_authorization;
#[cfg(test)]
pub(super) use dependent_result_address::DependentResultAddressAuthorizer;
pub(super) use retained_data_access_result::retained_data_access_result_window_candidate;

pub(super) enum DetailedFetchAheadCandidate {
    NotApplicable,
    Blocked,
    Ready(Address),
    ReadyProducerForwardedScalar {
        pc: Address,
        scalar_chain: O3ProducerForwardedScalarChain,
    },
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
    DataAccessResultWindow {
        next_pc: Option<Address>,
        authorizations: Vec<(MemoryRequestId, super::O3MemoryResultWindowAuthorization)>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequiredRasConsumer {
    Pop,
    PopThenPush { pushed_address: Address },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PredictedControlTargetAuthority {
    Normal,
    ProducerForwarded(O3ProducerForwardedControlTarget),
    ProducerForwardedReturn(O3ProducerForwardedReturnDescendant),
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
    Mmio,
    Blocked,
}

pub(super) enum ScalarMemoryFetchAheadHead {
    Untranslated,
    CachedTranslatedLoad { destination: Register },
}

pub(super) fn allows_detailed_data_access_head_fetch_ahead(
    state: &RiscvCoreState,
    fetch_request: rem6_memory::MemoryRequestId,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    translated: TranslatedMemoryFetchAhead,
) -> bool {
    state.live_retire_gate.detailed_policy_enabled()
        && (scalar_memory_fetch_ahead_head(state, fetch_request, instruction, translated).is_some()
            || data_access_result_fetch_ahead_authorization(
                state,
                fetch_request,
                instruction,
                instruction_bytes,
                translated,
            )
            .is_some())
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
    if translated == TranslatedMemoryFetchAhead::Blocked {
        return None;
    }
    if state.data_translation.is_none() {
        if translated == TranslatedMemoryFetchAhead::Mmio {
            return None;
        }
        return state
            .cacheable_scalar_memory_instruction_range(instruction)
            .is_some()
            .then_some(ScalarMemoryFetchAheadHead::Untranslated);
    }
    if translated != TranslatedMemoryFetchAhead::CachedMemory {
        return None;
    }
    let destination = independent_scalar_load_destination(instruction, [])?;
    state
        .cacheable_cached_translated_scalar_load(instruction, fetch_request)
        .then_some(ScalarMemoryFetchAheadHead::CachedTranslatedLoad { destination })
}

pub(super) fn data_access_has_younger_fetch(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    memory_request: rem6_memory::MemoryRequestId,
    younger_pc: Address,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    translated: TranslatedMemoryFetchAhead,
) -> bool {
    allows_detailed_data_access_head_fetch_ahead(
        state,
        memory_request,
        instruction,
        instruction_bytes,
        translated,
    ) && has_live_younger_fetch_at(state, fetch_events, memory_request, younger_pc)
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

pub(super) fn data_access_waits_for_younger_fetch(
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
    let Some(current) =
        completed_fetch_instruction_starting_with(&state.executed_fetches, fetch_events, memory)
    else {
        return false;
    };
    if state.can_overlap_detailed_scalar_memory_instruction(current.decoded().instruction()) {
        return false;
    }

    allows_detailed_data_access_head_fetch_ahead(
        state,
        current.first_consumed_request(),
        current.decoded().instruction(),
        current.decoded().bytes(),
        translated,
    ) && super::has_pending_younger_fetch(
        state,
        fetch_events,
        current.last_consumed_request().sequence(),
    )
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
    if translated == TranslatedMemoryFetchAhead::Blocked {
        return DetailedFetchAheadCandidate::Blocked;
    }
    if let Some(candidate) = producer_forwarded_control_fetch_candidate(state, fetch_events) {
        return candidate;
    }
    if let Some(candidate) = producer_forwarded_return_fetch_candidate(state, fetch_events) {
        return candidate;
    }
    if let Some(candidate) =
        producer_forwarded_scalar_continuation_fetch_candidate(state, fetch_events)
    {
        return candidate;
    }
    if let Some(candidate) =
        retained_producer_forwarded_scalar_return_fetch_candidate(state, fetch_events)
    {
        return candidate;
    }
    if let Some(candidate) =
        retained_data_access_result_window_candidate(state, fetch_events, translated)
    {
        return candidate;
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
    if state
        .memory_result_window_authorizations
        .get(&current.first_consumed_request())
        .is_some_and(|authorization| authorization.role().is_younger())
    {
        return DetailedFetchAheadCandidate::Blocked;
    }
    let scalar_memory_head = scalar_memory_fetch_ahead_head(
        state,
        current.last_consumed_request(),
        current.decoded().instruction(),
        translated,
    );
    if matches!(
        scalar_memory_head,
        Some(ScalarMemoryFetchAheadHead::Untranslated)
    ) {
        if let Some(authorization) = data_access_result_fetch_ahead_authorization(
            state,
            current.first_consumed_request(),
            current.decoded().instruction(),
            current.decoded().bytes(),
            translated,
        ) {
            let candidate = data_access_result_window_candidate(
                state,
                fetch_events,
                &current,
                authorization,
                translated,
            );
            if matches!(
                &candidate,
                DetailedFetchAheadCandidate::DataAccessResultWindow { authorizations, .. }
                    if authorizations.iter().any(|(_, authorization)| {
                        matches!(
                            authorization.role(),
                            super::O3MemoryResultWindowRole::YoungerBufferedEffect
                                | super::O3MemoryResultWindowRole::YoungerDependentRead
                        )
                    })
            ) {
                return candidate;
            }
        }
    }
    match scalar_memory_head {
        Some(ScalarMemoryFetchAheadHead::Untranslated) => {
            return scalar_memory_window_candidate(state, fetch_events, &current);
        }
        Some(ScalarMemoryFetchAheadHead::CachedTranslatedLoad { destination }) => {
            if let Some(authorization) = data_access_result_fetch_ahead_authorization(
                state,
                current.first_consumed_request(),
                current.decoded().instruction(),
                current.decoded().bytes(),
                translated,
            ) {
                let candidate = data_access_result_window_candidate(
                    state,
                    fetch_events,
                    &current,
                    authorization,
                    translated,
                );
                if matches!(
                    &candidate,
                    DetailedFetchAheadCandidate::DataAccessResultWindow {
                        authorizations,
                        ..
                    } if authorizations.len() == 2
                        && authorizations.iter().any(|(_, authorization)| {
                            authorization.role()
                                == super::O3MemoryResultWindowRole::YoungerRead
                        })
                ) {
                    return candidate;
                }
            }
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
    if let Some(authorization) = data_access_result_fetch_ahead_authorization(
        state,
        current.first_consumed_request(),
        current.decoded().instruction(),
        current.decoded().bytes(),
        translated,
    ) {
        return data_access_result_window_candidate(
            state,
            fetch_events,
            &current,
            authorization,
            translated,
        );
    }
    let Some(window) = RiscvScalarIntegerLiveWindow::from_fu_head(current.decoded().instruction())
    else {
        return DetailedFetchAheadCandidate::NotApplicable;
    };
    scalar_integer_fu_window_candidate(state, fetch_events, &current, window)
}

fn producer_forwarded_control_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Option<DetailedFetchAheadCandidate> {
    let forwarded = state.o3_runtime.producer_forwarded_control_target()?;
    let target_authority = PredictedControlTargetAuthority::ProducerForwarded(forwarded);
    Some(
        match recorded_predicted_pc(
            state,
            forwarded.fetch_request(),
            forwarded.sequential_pc(),
            &target_authority,
        ) {
            RecordedPredictedPc::Ready(target) if target == forwarded.target() => {
                match completed_window_instruction_or_candidate(
                    state,
                    fetch_events,
                    forwarded.last_fetch_request(),
                    target,
                ) {
                    Ok(_) => DetailedFetchAheadCandidate::Blocked,
                    Err(candidate) => candidate,
                }
            }
            RecordedPredictedPc::Missing
                if state.branch_speculations.len() < state.branch_lookahead =>
            {
                DetailedFetchAheadCandidate::ReadyPredictedControl {
                    request: forwarded.fetch_request(),
                    pc: forwarded.pc(),
                    sequential_pc: forwarded.sequential_pc(),
                    instruction: forwarded.instruction(),
                    target_authority,
                }
            }
            RecordedPredictedPc::Ready(_)
            | RecordedPredictedPc::Missing
            | RecordedPredictedPc::Invalid => DetailedFetchAheadCandidate::Blocked,
        },
    )
}

fn producer_forwarded_return_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Option<DetailedFetchAheadCandidate> {
    let descendant = state.o3_runtime.producer_forwarded_return_descendant()?;
    let target_authority =
        PredictedControlTargetAuthority::ProducerForwardedReturn(descendant.clone());
    Some(
        match recorded_predicted_pc(
            state,
            descendant.fetch_request(),
            descendant.sequential_pc(),
            &target_authority,
        ) {
            RecordedPredictedPc::Ready(target) if target == descendant.target() => {
                match completed_window_instruction_or_candidate(
                    state,
                    fetch_events,
                    descendant.last_fetch_request(),
                    target,
                ) {
                    Ok(_) => DetailedFetchAheadCandidate::Blocked,
                    Err(candidate) => candidate,
                }
            }
            RecordedPredictedPc::Missing
                if state.branch_speculations.len() < state.branch_lookahead =>
            {
                DetailedFetchAheadCandidate::ReadyPredictedControl {
                    request: descendant.fetch_request(),
                    pc: descendant.pc(),
                    sequential_pc: descendant.sequential_pc(),
                    instruction: descendant.instruction(),
                    target_authority,
                }
            }
            RecordedPredictedPc::Ready(_)
            | RecordedPredictedPc::Missing
            | RecordedPredictedPc::Invalid => DetailedFetchAheadCandidate::Blocked,
        },
    )
}

fn producer_forwarded_scalar_continuation_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Option<DetailedFetchAheadCandidate> {
    let scalar_chain = state.o3_runtime.producer_forwarded_scalar_chain()?;
    let parent = scalar_chain.parent();
    let scalar = scalar_chain.last()?;
    let target_authority = PredictedControlTargetAuthority::ProducerForwarded(parent);
    if recorded_predicted_pc(
        state,
        parent.fetch_request(),
        parent.sequential_pc(),
        &target_authority,
    ) != RecordedPredictedPc::Ready(parent.target())
        || state.branch_speculations.len() >= state.branch_lookahead
        || unconsumed_ras_required_target(
            state,
            parent.fetch_request().sequence(),
            parent.sequential_pc(),
            RequiredRasConsumer::Pop,
        ) != Some(parent.sequential_pc())
    {
        return Some(DetailedFetchAheadCandidate::Blocked);
    }
    Some(
        match completed_window_instruction_or_candidate(
            state,
            fetch_events,
            scalar.last_fetch_request(),
            scalar.sequential_pc(),
        ) {
            Ok(_) => DetailedFetchAheadCandidate::Blocked,
            Err(DetailedFetchAheadCandidate::Ready(pc)) => {
                DetailedFetchAheadCandidate::ReadyProducerForwardedScalar { pc, scalar_chain }
            }
            Err(candidate) => candidate,
        },
    )
}

fn retained_producer_forwarded_scalar_return_fetch_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Option<DetailedFetchAheadCandidate> {
    let continuation = state.producer_forwarded_scalar_continuation.as_ref()?;
    if !continuation.matches_parent_ras(state) {
        return Some(DetailedFetchAheadCandidate::Blocked);
    }
    let parent = continuation.parent();
    let retained_executed_fetches = std::collections::BTreeSet::new();
    let scalar_chain = match completed_fetch_instruction_from_events(
        &retained_executed_fetches,
        fetch_events,
        parent.last_fetch_request(),
        parent.target(),
    ) {
        Some(fetched) => parent.fetched_scalar_chain(
            fetched.decoded().instruction(),
            fetched.decoded().bytes(),
            fetched.consumed_requests(),
        )?,
        None => {
            return Some(
                match completed_window_instruction_or_candidate(
                    state,
                    fetch_events,
                    parent.last_fetch_request(),
                    parent.target(),
                ) {
                    Ok(_) => DetailedFetchAheadCandidate::Blocked,
                    Err(candidate) => candidate,
                },
            );
        }
    };
    if !continuation.retains_scalar_chain(state, &scalar_chain)
        || state.branch_speculations.len() >= state.branch_lookahead
    {
        return Some(DetailedFetchAheadCandidate::Blocked);
    }
    let scalar = scalar_chain.last()?;
    let returned = match completed_window_instruction_or_candidate(
        state,
        fetch_events,
        scalar.last_fetch_request(),
        scalar.sequential_pc(),
    ) {
        Ok(returned) => returned,
        Err(DetailedFetchAheadCandidate::Ready(pc)) => {
            return Some(DetailedFetchAheadCandidate::ReadyProducerForwardedScalar {
                pc,
                scalar_chain,
            });
        }
        Err(candidate) => return Some(candidate),
    };
    let descendant = scalar_chain.retained_return_descendant(
        returned.decoded().instruction(),
        returned.decoded().bytes(),
        returned.consumed_requests(),
    )?;
    let target_authority =
        PredictedControlTargetAuthority::ProducerForwardedReturn(descendant.clone());
    Some(
        match recorded_predicted_pc(
            state,
            descendant.fetch_request(),
            descendant.sequential_pc(),
            &target_authority,
        ) {
            RecordedPredictedPc::Missing
                if state.branch_speculations.len() < state.branch_lookahead =>
            {
                DetailedFetchAheadCandidate::ReadyPredictedControl {
                    request: descendant.fetch_request(),
                    pc: descendant.pc(),
                    sequential_pc: descendant.sequential_pc(),
                    instruction: descendant.instruction(),
                    target_authority,
                }
            }
            RecordedPredictedPc::Ready(target) if target == descendant.target() => {
                DetailedFetchAheadCandidate::Blocked
            }
            RecordedPredictedPc::Ready(_)
            | RecordedPredictedPc::Missing
            | RecordedPredictedPc::Invalid => DetailedFetchAheadCandidate::Blocked,
        },
    )
}

pub(crate) fn retained_parent_resolution_preserves_fetch_path(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    request: MemoryRequestId,
    pc: Address,
    instruction: RiscvInstruction,
    target: Address,
    current_fetch_pc: Address,
) -> bool {
    let Some(continuation) = state.producer_forwarded_scalar_continuation.as_ref() else {
        return false;
    };
    let parent = continuation.parent();
    if request != parent.fetch_request()
        || pc != parent.pc()
        || instruction != parent.instruction()
        || target != parent.target()
    {
        return false;
    }
    let retained_executed_fetches = std::collections::BTreeSet::new();
    let Some(fetched) = completed_fetch_instruction_from_events(
        &retained_executed_fetches,
        fetch_events,
        parent.last_fetch_request(),
        parent.target(),
    ) else {
        return false;
    };
    let Some(scalar_chain) = parent.fetched_scalar_chain(
        fetched.decoded().instruction(),
        fetched.decoded().bytes(),
        fetched.consumed_requests(),
    ) else {
        return false;
    };
    let Some(scalar) = scalar_chain.last() else {
        return false;
    };
    let Some(returned) = completed_fetch_instruction_from_events(
        &retained_executed_fetches,
        fetch_events,
        scalar.last_fetch_request(),
        scalar.sequential_pc(),
    ) else {
        return false;
    };
    let Some(descendant) = scalar_chain.retained_return_descendant(
        returned.decoded().instruction(),
        returned.decoded().bytes(),
        returned.consumed_requests(),
    ) else {
        return false;
    };
    let landing_sequential_pc = completed_fetch_instruction_from_events(
        &retained_executed_fetches,
        fetch_events,
        descendant.last_fetch_request(),
        descendant.target(),
    )
    .map(|landing| {
        Address::new(
            landing
                .pc()
                .get()
                .wrapping_add(u64::from(landing.decoded().bytes())),
        )
    });
    continuation.matches_scalar_identity(&scalar_chain)
        && continuation.matches_return_identity(&descendant)
        && (current_fetch_pc == descendant.target()
            || landing_sequential_pc == Some(current_fetch_pc))
        && (continuation.matches_parent_ras(state)
            || continuation.recorded_return_target(
                state,
                &descendant,
                descendant.fetch_request().sequence(),
            ) == Some(descendant.target()))
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

pub(super) fn ready_translated_scalar_load_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    fetch_request: MemoryRequestId,
) -> DetailedFetchAheadCandidate {
    let Some(translated) = state.ready_translated_data.get(&fetch_request) else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    if state
        .memory_result_window_authorizations
        .get(&fetch_request)
        .is_some_and(|authorization| authorization.role() == O3MemoryResultWindowRole::Head)
    {
        if let Some(candidate) = retained_data_access_result_window_candidate(
            state,
            fetch_events,
            TranslatedMemoryFetchAhead::CachedMemory,
        ) {
            return candidate;
        }
    }
    let MemoryAccessKind::Load { rd, .. } = &translated.access else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    let Some(window) = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
        [*rd],
        1,
        state.o3_runtime.scalar_memory_window_limit(),
    ) else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    scalar_integer_window_candidate_from(
        state,
        fetch_events,
        fetch_request,
        Address::new(state.hart.pc()),
        window,
        Vec::new(),
    )
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
                    &target_authority,
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
    let scalar_memory_window_limit = state.o3_runtime.scalar_memory_window_limit();
    let scalar_live_window_limit = state.o3_runtime.scalar_live_window_limit();
    let Some(window) = state.o3_runtime.scalar_memory_fetch_window_state() else {
        return DetailedFetchAheadCandidate::Blocked;
    };
    if window.rows() >= scalar_memory_window_limit || window.rows() >= scalar_live_window_limit {
        return DetailedFetchAheadCandidate::Blocked;
    }
    let mut destinations = window.load_destinations().to_vec();
    if !admit_scalar_memory_prefix_instruction(
        state,
        current.decoded().instruction(),
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
        if window_rows >= scalar_live_window_limit {
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
            if window_rows >= scalar_memory_window_limit || window_rows >= scalar_live_window_limit
            {
                return DetailedFetchAheadCandidate::Blocked;
            }
            if !admit_scalar_memory_prefix_instruction(state, instruction, &mut destinations) {
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

        let Some(mut alu_window) =
            RiscvScalarIntegerLiveWindow::from_untranslated_scalar_memory_prefix(
                destinations.iter().copied(),
                window_rows,
                scalar_live_window_limit,
            )
        else {
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
                    &target_authority,
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
    destinations: &mut Vec<Register>,
) -> bool {
    if state
        .cacheable_scalar_memory_instruction_range(instruction)
        .is_none()
    {
        return false;
    }
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
        RiscvInstruction::Store { .. } if destinations.is_empty() => true,
        _ => false,
    }
}

pub(crate) fn recorded_predicted_pc(
    state: &RiscvCoreState,
    request: MemoryRequestId,
    sequential_pc: Address,
    target_authority: &PredictedControlTargetAuthority,
) -> RecordedPredictedPc {
    match target_authority {
        PredictedControlTargetAuthority::ProducerForwarded(forwarded)
            if request != forwarded.fetch_request()
                || sequential_pc != forwarded.sequential_pc() =>
        {
            return RecordedPredictedPc::Invalid;
        }
        PredictedControlTargetAuthority::ProducerForwardedReturn(descendant)
            if request != descendant.fetch_request()
                || sequential_pc != descendant.sequential_pc() =>
        {
            return RecordedPredictedPc::Invalid;
        }
        _ => {}
    }
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
        PredictedControlTargetAuthority::ProducerForwarded(forwarded) => {
            let live_head = state
                .o3_runtime
                .retained_producer_forwarded_control_target()
                == Some(*forwarded);
            let retired_head = state
                .o3_runtime
                .producer_forwarded_control_target_after_head_retire()
                == Some(*forwarded);
            let scalar_head = state
                .o3_runtime
                .producer_forwarded_scalar_chain()
                .is_some_and(|scalar_chain| scalar_chain.parent() == *forwarded);
            if (!live_head && !retired_head && !scalar_head)
                || !producer_forwarded_control_speculation_matches(
                    state,
                    request.sequence(),
                    *speculation,
                    *forwarded,
                )
                || pending.pc() != forwarded.pc()
                || !pending.predicted_taken()
                || pending.target() != Some(forwarded.target())
            {
                RecordedPredictedPc::Invalid
            } else {
                RecordedPredictedPc::Ready(forwarded.target())
            }
        }
        PredictedControlTargetAuthority::ProducerForwardedReturn(descendant) => {
            let live_descendant = state
                .o3_runtime
                .producer_forwarded_return_descendant()
                .as_ref()
                == Some(descendant);
            let retained_descendant = state
                .producer_forwarded_scalar_continuation
                .as_ref()
                .is_some_and(|continuation| continuation.matches_return_identity(descendant));
            if !live_descendant && !retained_descendant {
                return RecordedPredictedPc::Invalid;
            }
            let parent = descendant.parent();
            let target = recorded_ras_required_target(
                state,
                parent.fetch_request().sequence(),
                parent.sequential_pc(),
                RequiredRasConsumer::Pop,
                request.sequence(),
            )
            .or_else(|| {
                state
                    .producer_forwarded_scalar_continuation
                    .as_ref()
                    .and_then(|continuation| {
                        continuation.recorded_return_target(state, descendant, request.sequence())
                    })
            });
            let Some(target) = target else {
                return RecordedPredictedPc::Invalid;
            };
            if pending.predicted_taken() && pending.target() == Some(target) {
                RecordedPredictedPc::Ready(target)
            } else {
                RecordedPredictedPc::Invalid
            }
        }
        PredictedControlTargetAuthority::RasRequired {
            push_sequence,
            pushed_address,
            consumer,
        } => {
            let Some(target) = recorded_ras_required_target(
                state,
                *push_sequence,
                *pushed_address,
                *consumer,
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

fn producer_forwarded_control_speculation_matches(
    state: &RiscvCoreState,
    sequence: u64,
    speculation: BranchSpeculationId,
    forwarded: O3ProducerForwardedControlTarget,
) -> bool {
    let kind = riscv_branch_target_kind(forwarded.instruction());
    if !state
        .o3_runtime
        .recorded_producer_forwarded_control_speculation_matches(sequence, speculation)
        || state.branch_speculation_kinds.get(&sequence) != Some(&kind)
    {
        return false;
    }
    match super::return_address_stack_action(forwarded.instruction(), forwarded.sequential_pc()) {
        None => !state
            .return_address_stack_operations
            .contains_key(&sequence),
        Some(super::ReturnAddressStackAction::Push(pushed_address)) => {
            let Some(operation_id) = state.return_address_stack_operations.get(&sequence) else {
                return false;
            };
            let Some(operation) = state
                .return_address_stack
                .pending_operations()
                .iter()
                .find(|operation| operation.id() == *operation_id)
            else {
                return false;
            };
            ras_required_producer_matches(
                kind,
                operation,
                state.return_address_stack.config().entries(),
                pushed_address,
                RequiredRasConsumer::Pop,
            )
        }
        Some(
            super::ReturnAddressStackAction::Pop | super::ReturnAddressStackAction::PopThenPush(_),
        ) => false,
    }
}

fn ras_required_producer_matches(
    producer_kind: BranchTargetKind,
    operation: &ReturnAddressStackOperation,
    entries: usize,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
) -> bool {
    if entries == 0
        || operation.stack_before().len() > entries
        || operation.pushed_address() != Some(pushed_address)
    {
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
        (BranchTargetKind::Return, ReturnAddressStackOperationKind::PopThenPush, _) => {
            let Some(predicted_return) = expected_after.pop() else {
                return false;
            };
            if operation.predicted_return() != Some(predicted_return) {
                return false;
            }
        }
        _ => return false,
    }
    if expected_after.len() == entries {
        expected_after.remove(0);
    }
    expected_after.push(pushed_address);
    operation.stack_after() == expected_after
}

pub(crate) fn unconsumed_ras_required_target(
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

pub(super) fn unconsumed_producer_forwarded_return_target(
    state: &RiscvCoreState,
    fetch_pc: Address,
    instruction: RiscvInstruction,
    descendant: &O3ProducerForwardedReturnDescendant,
) -> Option<Address> {
    if fetch_pc != descendant.pc() || instruction != descendant.instruction() {
        return None;
    }
    let parent = descendant.parent();
    unconsumed_ras_required_target(
        state,
        parent.fetch_request().sequence(),
        parent.sequential_pc(),
        RequiredRasConsumer::Pop,
    )
    .or_else(|| {
        state
            .producer_forwarded_scalar_continuation
            .as_ref()
            .and_then(|continuation| {
                continuation.unconsumed_return_target(state, fetch_pc, instruction, descendant)
            })
    })
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
