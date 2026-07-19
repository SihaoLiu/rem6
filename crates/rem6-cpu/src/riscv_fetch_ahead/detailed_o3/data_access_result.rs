use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMemoryInstruction, VectorRegister,
};
use rem6_memory::{
    Address, AddressRange, MemoryRequestId, TranslationAccessKind, TranslationAddressSpaceId,
    TranslationRequest, TranslationRequestId,
};

use crate::{
    riscv_data_issue::{access_address, access_size, masked_vector_memory_request_span},
    riscv_live_retire_window::{
        completed_fetch_instruction_starting_with, RiscvCompletedFetchInstruction,
    },
    riscv_o3_window_policy::{RiscvScalarIntegerLiveWindow, RiscvScalarIntegerYoungerDecision},
    CpuFetchEvent, CpuFetchEventKind, RiscvCoreState,
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
    DetailedFetchAheadCandidate, TranslatedMemoryFetchAhead,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::riscv_fetch_ahead) enum DataAccessResultHeadPhysicalProbe {
    Memory,
    Ready {
        fetch_request: MemoryRequestId,
        access: MemoryAccessKind,
        range: AddressRange,
        request_byte_offset: usize,
    },
    Blocked,
}

pub(in crate::riscv_fetch_ahead) fn data_access_result_head_physical_probe(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> DataAccessResultHeadPhysicalProbe {
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
        return DataAccessResultHeadPhysicalProbe::Memory;
    };
    let Some(current) =
        completed_fetch_instruction_starting_with(&state.executed_fetches, fetch_events, memory)
    else {
        return DataAccessResultHeadPhysicalProbe::Memory;
    };
    let fetch_request = current.first_consumed_request();
    let instruction = current.decoded().instruction();
    if data_access_result_fetch_ahead_shape(state, instruction).is_none() {
        return DataAccessResultHeadPhysicalProbe::Memory;
    }
    let Some(probe) = data_access_result_head_probe(state, fetch_request, instruction) else {
        return DataAccessResultHeadPhysicalProbe::Blocked;
    };
    let range = match data_access_result_translation_probe(state, &probe) {
        DataAccessResultTranslationProbe::Direct => probe.virtual_range,
        DataAccessResultTranslationProbe::Unknown => {
            return DataAccessResultHeadPhysicalProbe::Memory;
        }
        DataAccessResultTranslationProbe::Ready(physical_address) => {
            let Ok(range) = AddressRange::new(physical_address, probe.virtual_range.size()) else {
                return DataAccessResultHeadPhysicalProbe::Blocked;
            };
            range
        }
        DataAccessResultTranslationProbe::Blocked => {
            return DataAccessResultHeadPhysicalProbe::Blocked;
        }
    };
    DataAccessResultHeadPhysicalProbe::Ready {
        fetch_request,
        access: probe.access,
        range,
        request_byte_offset: probe.request_byte_offset,
    }
}

fn data_access_result_fetch_ahead_shape(
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
    let physical_range = if state.data_translation.is_none() {
        probe.virtual_range
    } else {
        if !matches!(
            translated,
            TranslatedMemoryFetchAhead::CachedMemory | TranslatedMemoryFetchAhead::Mmio
        ) {
            return None;
        }
        let DataAccessResultTranslationProbe::Ready(physical_address) =
            data_access_result_translation_probe(state, &probe)
        else {
            return None;
        };
        AddressRange::new(physical_address, probe.virtual_range.size()).ok()?
    };
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
    Some(O3MemoryResultWindowAuthorization::new(
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
    let mut authorizations = vec![(current.first_consumed_request(), head_authorization)];
    let mut window = RiscvScalarIntegerLiveWindow::from_memory_result(
        head_authorization.integer_destination(),
        row_limit,
    );
    let mut result_rows = 1;
    let mut scalar_started = false;
    let mut previous_request = current.last_consumed_request();
    let mut next_pc = Address::new(
        current
            .pc()
            .get()
            .wrapping_add(u64::from(current.decoded().bytes())),
    );

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
                return if result_rows == 2 {
                    DetailedFetchAheadCandidate::DataAccessResultWindow {
                        next_pc: None,
                        authorizations,
                    }
                } else {
                    candidate
                };
            }
        };

        if !scalar_started && result_rows == 1 {
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
                    O3MemoryResultWindowRole::Head => false,
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
                next_pc = Address::new(
                    younger
                        .pc()
                        .get()
                        .wrapping_add(u64::from(younger.decoded().bytes())),
                );
                continue;
            }
        }

        match window.classify_younger(younger.decoded().instruction()) {
            RiscvScalarIntegerYoungerDecision::AdmitContinue => {
                scalar_started = true;
                previous_request = younger.last_consumed_request();
                next_pc = Address::new(
                    younger
                        .pc()
                        .get()
                        .wrapping_add(u64::from(younger.decoded().bytes())),
                );
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
                return if result_rows == 2 {
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

struct DataAccessResultHeadProbe {
    access: MemoryAccessKind,
    request: TranslationRequest,
    virtual_range: AddressRange,
    request_byte_offset: usize,
}

fn data_access_result_head_probe(
    state: &RiscvCoreState,
    fetch_request: MemoryRequestId,
    instruction: RiscvInstruction,
) -> Option<DataAccessResultHeadProbe> {
    let mut hart = state.hart.clone();
    let execution = hart.execute(instruction).ok()?;
    let access = execution.memory_access()?.clone();
    let translation_access = match &access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride {
            fault_only_first: false,
            ..
        }
        | MemoryAccessKind::LoadReserved { .. } => TranslationAccessKind::Load,
        MemoryAccessKind::AtomicMemory { .. } => TranslationAccessKind::Atomic,
        _ => return None,
    };
    let base_size = access_size(&access).ok()?;
    let base_address = Address::new(access_address(&access));
    let request_span = masked_vector_memory_request_span(&access, base_address, base_size).ok()?;
    let virtual_range = AddressRange::new(request_span.address, request_span.size).ok()?;
    let request = TranslationRequest::new(
        TranslationRequestId::new(fetch_request.agent(), fetch_request.sequence()),
        request_span.address,
        request_span.size,
        translation_access,
    )
    .ok()?;
    Some(DataAccessResultHeadProbe {
        access,
        request,
        virtual_range,
        request_byte_offset: request_span.byte_offset,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DataAccessResultTranslationProbe {
    Direct,
    Unknown,
    Ready(Address),
    Blocked,
}

fn data_access_result_translation_probe(
    state: &RiscvCoreState,
    probe: &DataAccessResultHeadProbe,
) -> DataAccessResultTranslationProbe {
    let Some(frontend) = state.data_translation.as_ref() else {
        return DataAccessResultTranslationProbe::Direct;
    };
    let Some(tlb) = frontend.tlb() else {
        return DataAccessResultTranslationProbe::Unknown;
    };
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    match tlb.peek_cached_in_address_space(address_space, &probe.request) {
        Ok(Some(lookup)) if lookup.fault().is_some() => DataAccessResultTranslationProbe::Blocked,
        Ok(Some(lookup)) => lookup
            .physical_address()
            .map(DataAccessResultTranslationProbe::Ready)
            .unwrap_or(DataAccessResultTranslationProbe::Blocked),
        Ok(None) => DataAccessResultTranslationProbe::Unknown,
        Err(_) => DataAccessResultTranslationProbe::Blocked,
    }
}
