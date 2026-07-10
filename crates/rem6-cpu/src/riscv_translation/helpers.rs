use rem6_isa_riscv::{
    AtomicMemoryOp, MemoryAccessKind, RiscvSv39AccessKind, RiscvSv39PageFault, RiscvTrapKind,
};
use rem6_memory::{
    AccessSize, Address, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryRequestId,
    TranslationFaultKind, TranslationRequestId, TranslationResolution,
};

use crate::riscv_cross_line::supports_cross_line_data_access;
use crate::riscv_data_issue::{store_byte_mask, store_bytes, vector_store_request_payload};
use crate::riscv_translation_state::{
    DataTranslationCompletion, PendingDataTranslation, TranslatedDataAccess,
};
use crate::{
    riscv_checker, CpuDataConfig, CpuTranslatedMemoryOperation, CpuTranslatedMemoryRequest,
    CpuTranslationFaultRecord, CpuTranslationOutcome, CpuTranslationRequest, RiscvCoreState,
    RiscvCpuError, RiscvCpuExecutionEvent, RiscvHartRunState,
};

pub(super) fn supports_translated_cross_line_data_access(
    access: &MemoryAccessKind,
    virtual_address: Address,
    physical_address: Address,
    size: AccessSize,
    line_layout: CacheLineLayout,
) -> bool {
    const RISCV_BASE_PAGE_BYTES: u64 = 4096;

    if !supports_cross_line_data_access(access, physical_address, size, line_layout) {
        return false;
    }
    let page_offset = virtual_address.get() & (RISCV_BASE_PAGE_BYTES - 1);
    page_offset
        .checked_add(size.bytes())
        .is_some_and(|end| end <= RISCV_BASE_PAGE_BYTES)
}

pub(super) fn wake_suspended_hart_on_pending_interrupt(state: &mut RiscvCoreState, pending: u64) {
    if pending != 0
        && matches!(
            state.run_state,
            RiscvHartRunState::Suspended | RiscvHartRunState::SuspendPending
        )
    {
        state.run_state = RiscvHartRunState::Started;
        state.run_state_explicit = true;
    }
}

pub(super) fn cpu_translation_request(
    translation_id: TranslationRequestId,
    memory_request_id: MemoryRequestId,
    data: &CpuDataConfig,
    access: &MemoryAccessKind,
    address: Address,
    size: AccessSize,
    request_byte_offset: usize,
) -> Result<CpuTranslationRequest, RiscvCpuError> {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. } => CpuTranslationRequest::load(
            translation_id,
            memory_request_id,
            data.route(),
            data.endpoint().clone(),
            address,
            size,
        ),
        MemoryAccessKind::LoadReserved { .. } => CpuTranslationRequest::load_locked(
            translation_id,
            memory_request_id,
            data.route(),
            data.endpoint().clone(),
            address,
            size,
        ),
        MemoryAccessKind::Store { value, .. } | MemoryAccessKind::FloatStore { value, .. } => {
            CpuTranslationRequest::store(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                address,
                size,
                store_bytes(*value, size),
                ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
            )
        }
        MemoryAccessKind::VectorStoreUnitStride {
            data: bytes,
            byte_mask,
            ..
        } => {
            let (bytes, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                bytes,
                byte_mask.as_deref(),
            )?;
            CpuTranslationRequest::store(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                address,
                size,
                bytes,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
        }
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            data: bytes,
            byte_mask,
            ..
        } => {
            let (bytes, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                bytes,
                byte_mask.as_deref(),
            )?;
            CpuTranslationRequest::store(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                address,
                size,
                bytes,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
        }
        MemoryAccessKind::VectorStoreStrided {
            data: bytes,
            byte_mask,
            ..
        } => {
            let (bytes, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                bytes,
                Some(byte_mask.as_slice()),
            )?;
            CpuTranslationRequest::store(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                address,
                size,
                bytes,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
        }
        MemoryAccessKind::VectorStoreIndexed {
            data: bytes,
            byte_mask,
            ..
        } => {
            let (bytes, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                bytes,
                Some(byte_mask.as_slice()),
            )?;
            CpuTranslationRequest::store(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                address,
                size,
                bytes,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
        }
        MemoryAccessKind::StoreConditional { value, .. } => {
            CpuTranslationRequest::store_conditional(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                address,
                size,
                store_bytes(*value, size),
                ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
            )
        }
        MemoryAccessKind::AtomicMemory { value, op, .. } => CpuTranslationRequest::atomic_with_op(
            translation_id,
            memory_request_id,
            data.route(),
            data.endpoint().clone(),
            address,
            size,
            match op {
                AtomicMemoryOp::Swap => MemoryAtomicOp::Swap,
                AtomicMemoryOp::Add => MemoryAtomicOp::Add,
                AtomicMemoryOp::Xor => MemoryAtomicOp::Xor,
                AtomicMemoryOp::Or => MemoryAtomicOp::Or,
                AtomicMemoryOp::And => MemoryAtomicOp::And,
                AtomicMemoryOp::MinSigned => MemoryAtomicOp::MinSigned,
                AtomicMemoryOp::MaxSigned => MemoryAtomicOp::MaxSigned,
                AtomicMemoryOp::MinUnsigned => MemoryAtomicOp::MinUnsigned,
                AtomicMemoryOp::MaxUnsigned => MemoryAtomicOp::MaxUnsigned,
            },
            store_bytes(*value, size),
            ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
        ),
    }
    .map_err(RiscvCpuError::DataTranslation)
}

pub(super) fn ready_translated_fetch_request(state: &RiscvCoreState) -> Option<MemoryRequestId> {
    state.events.iter().find_map(|event| {
        let fetch_request = event.fetch().request_id();
        if state.issued_data_for_fetches.contains(&fetch_request) {
            return None;
        }
        state
            .ready_translated_data
            .contains_key(&fetch_request)
            .then_some(fetch_request)
    })
}

pub(super) fn translated_data_from_outcome(
    pending: PendingDataTranslation,
    outcome: CpuTranslationOutcome,
) -> DataTranslationCompletion {
    match outcome {
        CpuTranslationOutcome::Mapped(mapped) => {
            debug_assert_eq!(mapped.memory_request_id(), pending.request_id);
            debug_assert_eq!(mapped.size(), pending.size);
            DataTranslationCompletion::Access(TranslatedDataAccess {
                request_id: mapped.memory_request_id(),
                fetch_request: pending.fetch_request,
                access: pending.access,
                virtual_address: pending.virtual_address,
                size: mapped.size(),
                physical_address: mapped.physical_address(),
                request_byte_offset: pending.request_byte_offset,
            })
        }
        CpuTranslationOutcome::Fault(fault) => DataTranslationCompletion::Fault {
            fetch_request: pending.fetch_request,
            fault,
        },
    }
}

pub(super) fn record_data_translation_fault_state(
    state: &mut RiscvCoreState,
    fetch_request: MemoryRequestId,
    fault: CpuTranslationFaultRecord,
) -> Result<Address, RiscvCpuError> {
    let original_index = state
        .events
        .iter()
        .position(|event| event.fetch().request_id() == fetch_request)
        .ok_or_else(|| RiscvCpuError::DataTranslationFault {
            fetch: fetch_request,
            fault: fault.fault().clone(),
        })?;
    let original = state.events[original_index].clone();
    state
        .o3_runtime
        .abort_deferred_scalar_memory_execution(fetch_request);
    let trap_kind = data_translation_fault_trap_kind(&fault);
    let execution = state.hart.enter_synchronous_trap(
        original.instruction(),
        original.execution().instruction_bytes(),
        original.fetch_pc().get(),
        trap_kind,
    );
    let event = RiscvCpuExecutionEvent::with_retired_instruction_counting(
        original.fetch().clone(),
        original.instruction(),
        execution,
        None,
        false,
    );
    state.pending_trap = event.execution().trap().copied();
    state.pending_trap_event = Some(event.clone());
    state.issued_data_for_fetches.insert(fetch_request);
    state.ready_translated_data.remove(&fetch_request);
    state.events[original_index] = event;
    riscv_checker::sync_checker_hart(state);
    Ok(Address::new(state.hart.pc()))
}

fn data_translation_fault_trap_kind(fault: &CpuTranslationFaultRecord) -> RiscvTrapKind {
    let address = fault.fault().virtual_address().get();
    match fault.operation() {
        CpuTranslatedMemoryOperation::InstructionFetch => {
            RiscvTrapKind::InstructionPageFault { address }
        }
        CpuTranslatedMemoryOperation::Read | CpuTranslatedMemoryOperation::LoadLocked => {
            RiscvTrapKind::LoadPageFault { address }
        }
        CpuTranslatedMemoryOperation::Write
        | CpuTranslatedMemoryOperation::StoreConditional
        | CpuTranslatedMemoryOperation::Atomic => RiscvTrapKind::StorePageFault { address },
    }
}

pub(super) fn sv39_access_kind(operation: CpuTranslatedMemoryOperation) -> RiscvSv39AccessKind {
    match operation {
        CpuTranslatedMemoryOperation::InstructionFetch => RiscvSv39AccessKind::InstructionFetch,
        CpuTranslatedMemoryOperation::Read | CpuTranslatedMemoryOperation::LoadLocked => {
            RiscvSv39AccessKind::Load
        }
        CpuTranslatedMemoryOperation::Write => RiscvSv39AccessKind::Store,
        CpuTranslatedMemoryOperation::StoreConditional | CpuTranslatedMemoryOperation::Atomic => {
            RiscvSv39AccessKind::Atomic
        }
    }
}

pub(super) fn sv39_translation_fault_kind(fault: &RiscvSv39PageFault) -> TranslationFaultKind {
    match fault {
        RiscvSv39PageFault::PermissionDenied { .. } => TranslationFaultKind::PermissionFault,
        _ => TranslationFaultKind::PageFault,
    }
}

pub(super) fn cpu_translation_outcome_from_resolution(
    request: CpuTranslationRequest,
    resolution: TranslationResolution,
) -> CpuTranslationOutcome {
    match resolution {
        TranslationResolution::Mapped(physical_address) => CpuTranslationOutcome::Mapped(
            CpuTranslatedMemoryRequest::new(request, physical_address),
        ),
        TranslationResolution::Fault(fault) => {
            CpuTranslationOutcome::Fault(CpuTranslationFaultRecord::new(
                request.translation_id(),
                request.memory_request_id(),
                request.route(),
                request.endpoint().clone(),
                request.virtual_address(),
                request.size(),
                request.operation(),
                fault,
            ))
        }
    }
}
