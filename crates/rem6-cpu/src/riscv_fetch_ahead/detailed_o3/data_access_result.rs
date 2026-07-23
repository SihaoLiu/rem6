use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMemoryInstruction, VectorRegister,
};
use rem6_memory::{Address, AddressRange, MemoryRequestId};

use crate::{
    riscv_live_retire_window::RiscvCompletedFetchInstruction,
    riscv_o3_window_policy::{RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision},
    CpuFetchEvent, RiscvCoreState,
};

use super::super::{
    O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole, O3MemoryResultWindowRoute,
};
use super::{
    completed_window_instruction_or_candidate,
    data_access_result_effect_policy::{
        data_access_result_younger_authorization, result_head_allows_younger_effect,
    },
    data_access_result_pair_policy::result_head_allows_younger_read,
    data_access_result_translation::{
        data_access_result_head_probe, data_access_result_translation_probe,
        DataAccessResultTranslationProbe,
    },
    dependent_result_address::DependentResultAddressAuthorizer,
    dependent_result_address_authorization, DetailedFetchAheadCandidate,
    TranslatedMemoryFetchAhead,
};

pub(super) fn data_access_result_fetch_ahead_shape(
    state: &RiscvCoreState,
    instruction: RiscvInstruction,
) -> Option<Option<Register>> {
    let integer_destination = match instruction {
        RiscvInstruction::Load { rd, .. } if !rd.is_zero() => Some(rd),
        RiscvInstruction::FloatLoad { .. } => None,
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride {
            vd,
            width: MemoryWidth::Doubleword,
            mask,
            ..
        }) => {
            let config = state.hart.vector_config();
            if config.vill()
                || config.vl() == 0
                || config.vtype() & 0x7 != 0
                || config.element_width_bytes() != Some(MemoryWidth::Doubleword.bytes())
                || config.register_group_registers() != Some(1)
                || (mask == RiscvVectorMaskMode::Masked && vd.index() == 0)
            {
                return None;
            }
            if mask == RiscvVectorMaskMode::Masked {
                let mask_register = state
                    .hart
                    .read_vector(VectorRegister::new(0).expect("v0 is a valid vector register"));
                let any_active = (0..config.vl() as usize)
                    .any(|lane| mask_register[lane / 8] & (1_u8 << (lane % 8)) != 0);
                if !any_active {
                    return None;
                }
            }
            None
        }
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStrideFaultOnly {
            ..
        }) => return None,
        RiscvInstruction::LoadReserved { rd, .. } | RiscvInstruction::AtomicMemory { rd, .. }
            if !rd.is_zero() =>
        {
            Some(rd)
        }
        _ => return None,
    };
    Some(integer_destination)
}
pub(in crate::riscv_fetch_ahead) fn data_access_result_fetch_ahead_authorization(
    state: &RiscvCoreState,
    fetch_request: MemoryRequestId,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    translated: TranslatedMemoryFetchAhead,
) -> Option<O3MemoryResultWindowAuthorization> {
    data_access_result_authorization(
        state,
        fetch_request,
        instruction,
        instruction_bytes,
        translated,
        O3MemoryResultWindowRole::Head,
    )
}
pub(in crate::riscv_fetch_ahead) fn data_access_result_authorization(
    state: &RiscvCoreState,
    fetch_request: MemoryRequestId,
    instruction: RiscvInstruction,
    instruction_bytes: u8,
    translated: TranslatedMemoryFetchAhead,
    role: O3MemoryResultWindowRole,
) -> Option<O3MemoryResultWindowAuthorization> {
    if !state.live_retire_gate.detailed_policy_enabled()
        || instruction_bytes != 4
        || state.o3_runtime.scalar_memory_window_limit() <= 1
        || translated == TranslatedMemoryFetchAhead::Blocked
        || (translated == TranslatedMemoryFetchAhead::Mmio && state.data_translation.is_some())
    {
        return None;
    }
    let integer_destination = data_access_result_fetch_ahead_shape(state, instruction)?;
    let probe = data_access_result_head_probe(state, fetch_request, instruction)?;
    if translated == TranslatedMemoryFetchAhead::Mmio
        && !matches!(&probe.access, MemoryAccessKind::Load { rd, .. } if !rd.is_zero())
    {
        return None;
    }
    if state.data_translation.is_some() {
        if translated != TranslatedMemoryFetchAhead::CachedMemory {
            return None;
        }
        let DataAccessResultTranslationProbe::Ready(physical_address) =
            data_access_result_translation_probe(state, &probe)
        else {
            return None;
        };
        let physical_range =
            AddressRange::new(physical_address, probe.virtual_range.size()).ok()?;
        if state
            .pma
            .is_uncacheable(physical_range.start().get(), physical_range.size().bytes())
            .ok()?
        {
            return None;
        }
        return Some(O3MemoryResultWindowAuthorization::translated_unbound(
            integer_destination,
            probe.virtual_range,
            role,
        ));
    }
    let physical_range = probe.virtual_range;
    if translated != TranslatedMemoryFetchAhead::Mmio
        && state
            .pma
            .is_uncacheable(physical_range.start().get(), physical_range.size().bytes())
            .ok()?
    {
        return None;
    }
    let route = if translated == TranslatedMemoryFetchAhead::Mmio {
        O3MemoryResultWindowRoute::Mmio
    } else {
        O3MemoryResultWindowRoute::Memory
    };
    Some(O3MemoryResultWindowAuthorization::resolved(
        integer_destination,
        route,
        physical_range,
        role,
    ))
}
pub(in crate::riscv_fetch_ahead) fn data_access_result_window_candidate(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    current: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    translated: TranslatedMemoryFetchAhead,
) -> DetailedFetchAheadCandidate {
    if head_authorization.role() != O3MemoryResultWindowRole::Head {
        return DetailedFetchAheadCandidate::Blocked;
    }
    let row_limit = state.o3_runtime.scalar_memory_window_limit();
    let mut authorizer =
        DependentResultAddressAuthorizer::from_head(state, current, head_authorization, row_limit);
    let mut authorizations = vec![(current.first_consumed_request(), head_authorization)];
    let mut window = RiscvScalarIntegerLiveWindow::from_memory_result(
        head_authorization.integer_destination(),
        row_limit,
    );
    let (mut result_rows, mut dependent_rows, mut scalar_started) = (1, 0, false);
    let mut previous_request = current.last_consumed_request();
    let mut next_pc = completed_instruction_sequential_pc(current);
    while !window.is_full() {
        let younger = match completed_window_instruction_or_candidate(
            state,
            fetch_events,
            previous_request,
            next_pc,
        ) {
            Ok(younger) => younger,
            Err(DetailedFetchAheadCandidate::Ready(pc)) => {
                return DetailedFetchAheadCandidate::DataAccessResultWindow {
                    next_pc: Some(pc),
                    authorizations,
                };
            }
            Err(candidate) => {
                return if result_rows > 1 {
                    DetailedFetchAheadCandidate::DataAccessResultWindow {
                        next_pc: None,
                        authorizations,
                    }
                } else {
                    candidate
                };
            }
        };
        if !scalar_started && result_rows == 1 + dependent_rows {
            if let Some(authorizer) = authorizer.as_mut() {
                if let Some(authorization) = authorizer.try_authorize_next(&younger) {
                    debug_assert!(
                        authorizer.dependent_rows() != 1
                            || dependent_result_address_authorization(
                                state,
                                current,
                                &younger,
                                head_authorization,
                                row_limit,
                            ) == Some(authorization)
                    );
                    authorizations.push((younger.first_consumed_request(), authorization));
                    dependent_rows = authorizer.dependent_rows();
                    result_rows = 1 + dependent_rows;
                    window = RiscvScalarIntegerLiveWindow::from_memory_results(
                        authorizer.result_destinations().iter().copied(),
                        result_rows,
                        row_limit,
                    )
                    .expect("authorized result rows fit the configured live window");
                    previous_request = younger.last_consumed_request();
                    next_pc = completed_instruction_sequential_pc(&younger);
                    continue;
                }
            }
            if dependent_rows == 0 && result_rows == 1 {
                if let Some(younger_authorization) =
                    data_access_result_younger_authorization(state, &younger, translated)
                {
                    if !younger_authorization.role().is_younger() {
                        return DetailedFetchAheadCandidate::Blocked;
                    }
                    let allowed = match younger_authorization.role() {
                        O3MemoryResultWindowRole::YoungerRead => result_head_allows_younger_read(
                            current,
                            &younger,
                            head_authorization,
                            younger_authorization,
                        ),
                        O3MemoryResultWindowRole::YoungerBufferedEffect => {
                            result_head_allows_younger_effect(
                                current,
                                &younger,
                                head_authorization,
                                younger_authorization,
                            )
                        }
                        O3MemoryResultWindowRole::Head
                        | O3MemoryResultWindowRole::YoungerDependentRead => false,
                    };
                    if !allowed {
                        return DetailedFetchAheadCandidate::Blocked;
                    }
                    authorizations.push((younger.first_consumed_request(), younger_authorization));
                    result_rows = 2;
                    window = RiscvScalarIntegerLiveWindow::from_memory_results(
                        authorizations
                            .iter()
                            .filter_map(|(_, authorization)| authorization.integer_destination()),
                        result_rows,
                        row_limit,
                    )
                    .expect("authorized result rows fit the configured live window");
                    previous_request = younger.last_consumed_request();
                    next_pc = completed_instruction_sequential_pc(&younger);
                    continue;
                }
            }
        }
        let mut decision = window.classify_younger(younger.decoded().instruction());
        if dependent_rows != 0
            && decision == RiscvScalarIntegerYoungerDecision::AdmitStop
            && !window.is_full()
        {
            decision = RiscvScalarIntegerYoungerDecision::AdmitContinue;
        }
        match decision {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => {
                scalar_started = true;
                previous_request = younger.last_consumed_request();
                next_pc = completed_instruction_sequential_pc(&younger);
            }
            RiscvScalarIntegerYoungerDecision::AdmitStop => {
                return DetailedFetchAheadCandidate::DataAccessResultWindow {
                    next_pc: None,
                    authorizations,
                };
            }
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
            | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            | RiscvScalarIntegerYoungerDecision::Reject => {
                return if result_rows > 1 {
                    DetailedFetchAheadCandidate::DataAccessResultWindow {
                        next_pc: None,
                        authorizations,
                    }
                } else {
                    DetailedFetchAheadCandidate::Blocked
                };
            }
        }
    }
    DetailedFetchAheadCandidate::DataAccessResultWindow {
        next_pc: None,
        authorizations,
    }
}
fn completed_instruction_sequential_pc(instruction: &RiscvCompletedFetchInstruction) -> Address {
    let bytes = u64::from(instruction.decoded().bytes());
    Address::new(instruction.pc().get().wrapping_add(bytes))
}
