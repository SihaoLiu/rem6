use rem6_isa_riscv::MemoryWidth;
use rem6_memory::{
    AddressRange, TranslationAccessKind, TranslationAddressSpaceId, TranslationRequest,
    TranslationRequestId,
};

use crate::riscv_data_issue::{access_address, access_size, masked_vector_memory_request_span};
use crate::riscv_fetch_ahead::{O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole};

use super::data_access_result::data_access_result_fetch_ahead_shape;
use super::*;

pub(super) fn translated_younger_result_authorization(
    state: &RiscvCoreState,
    instruction: &RiscvCompletedFetchInstruction,
) -> Option<O3MemoryResultWindowAuthorization> {
    if state.data_translation.is_none()
        || !state.live_retire_gate.detailed_policy_enabled()
        || instruction.decoded().bytes() != 4
        || state.o3_runtime.scalar_memory_window_limit() <= 1
    {
        return None;
    }
    let decoded = instruction.decoded().instruction();
    let RiscvInstruction::Load {
        width: MemoryWidth::Doubleword,
        ..
    } = decoded
    else {
        return None;
    };
    let fetch_request = instruction.first_consumed_request();
    let rd = independent_scalar_load_destination(
        decoded,
        state
            .memory_result_window_authorizations
            .iter()
            .filter(|(request, _)| **request != fetch_request)
            .filter_map(|(_, authorization)| authorization.integer_destination()),
    )?;
    let mut hart = state.hart.clone();
    let execution = hart.execute(decoded).ok()?;
    let access = execution.memory_access()?.clone();
    let base_size = access_size(&access).ok()?;
    let base_address = Address::new(access_address(&access));
    let request_span = masked_vector_memory_request_span(&access, base_address, base_size).ok()?;
    let virtual_range = AddressRange::new(request_span.address, request_span.size).ok()?;
    let authorization = O3MemoryResultWindowAuthorization::translated_unbound(
        Some(rd),
        virtual_range,
        O3MemoryResultWindowRole::YoungerRead,
    );
    if let Some(existing) = state
        .memory_result_window_authorizations
        .get(&fetch_request)
        .copied()
    {
        return (existing == authorization).then_some(existing);
    }
    (state.memory_result_window_authorizations.len() < 2).then_some(authorization)
}

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

pub(super) struct DataAccessResultHeadProbe {
    pub(super) access: MemoryAccessKind,
    pub(super) request: TranslationRequest,
    pub(super) virtual_range: AddressRange,
    pub(super) request_byte_offset: usize,
}

pub(super) fn data_access_result_head_probe(
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
pub(super) enum DataAccessResultTranslationProbe {
    Direct,
    Unknown,
    Ready(Address),
    Blocked,
}

pub(super) fn data_access_result_translation_probe(
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
