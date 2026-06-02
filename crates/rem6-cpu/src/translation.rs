use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_isa_riscv::{RiscvPrivilegeMode, RiscvSv39AccessContext};
use rem6_memory::{
    AccessSize, Address, AddressRange, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryError,
    MemoryOperation, MemoryRequest, MemoryRequestId, TranslationAccessKind,
    TranslationAddressSpaceId, TranslationCompletion, TranslationError, TranslationFault,
    TranslationPageMap, TranslationQueue, TranslationQueueConfig, TranslationQueueSnapshot,
    TranslationRequest, TranslationRequestId, TranslationResolution, TranslationSegment,
    TranslationSegmentedResolution, TranslationTlb, TranslationTlbConfig, TranslationTlbSnapshot,
};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuTranslatedMemoryOperation {
    InstructionFetch,
    Read,
    Write,
    Atomic,
}

impl CpuTranslatedMemoryOperation {
    const fn translation_access_kind(self) -> TranslationAccessKind {
        match self {
            Self::InstructionFetch => TranslationAccessKind::InstructionFetch,
            Self::Read => TranslationAccessKind::Load,
            Self::Write => TranslationAccessKind::Store,
            Self::Atomic => TranslationAccessKind::Atomic,
        }
    }

    pub const fn memory_operation(self) -> MemoryOperation {
        match self {
            Self::InstructionFetch => MemoryOperation::InstructionFetch,
            Self::Read => MemoryOperation::ReadShared,
            Self::Write => MemoryOperation::Write,
            Self::Atomic => MemoryOperation::Atomic,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTranslationRequest {
    address_space: TranslationAddressSpaceId,
    translation_id: TranslationRequestId,
    memory_request_id: MemoryRequestId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    virtual_address: Address,
    size: AccessSize,
    operation: CpuTranslatedMemoryOperation,
    sv39_access_context: RiscvSv39AccessContext,
    write_data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
    atomic_op: Option<MemoryAtomicOp>,
}

impl CpuTranslationRequest {
    pub fn fetch(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
    ) -> Result<Self, CpuTranslationFrontendError> {
        Self::new(
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            CpuTranslatedMemoryOperation::InstructionFetch,
            None,
            None,
            None,
        )
    }

    pub fn load(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
    ) -> Result<Self, CpuTranslationFrontendError> {
        Self::new(
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            CpuTranslatedMemoryOperation::Read,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
        write_data: Vec<u8>,
        byte_mask: ByteMask,
    ) -> Result<Self, CpuTranslationFrontendError> {
        Self::new(
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            CpuTranslatedMemoryOperation::Write,
            Some(write_data),
            Some(byte_mask),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn atomic(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
        write_data: Vec<u8>,
        byte_mask: ByteMask,
    ) -> Result<Self, CpuTranslationFrontendError> {
        Self::atomic_with_op(
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            MemoryAtomicOp::Swap,
            write_data,
            byte_mask,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn atomic_with_op(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
        op: MemoryAtomicOp,
        write_data: Vec<u8>,
        byte_mask: ByteMask,
    ) -> Result<Self, CpuTranslationFrontendError> {
        Self::new(
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            CpuTranslatedMemoryOperation::Atomic,
            Some(write_data),
            Some(byte_mask),
            Some(op),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
        operation: CpuTranslatedMemoryOperation,
        write_data: Option<Vec<u8>>,
        byte_mask: Option<ByteMask>,
        atomic_op: Option<MemoryAtomicOp>,
    ) -> Result<Self, CpuTranslationFrontendError> {
        AddressRange::new(virtual_address, size).map_err(CpuTranslationFrontendError::Memory)?;
        match operation {
            CpuTranslatedMemoryOperation::Write | CpuTranslatedMemoryOperation::Atomic => {
                let data =
                    write_data
                        .as_ref()
                        .ok_or(CpuTranslationFrontendError::MissingWriteData {
                            request: memory_request_id,
                        })?;
                if data.len() as u64 != size.bytes() {
                    return Err(CpuTranslationFrontendError::PayloadSizeMismatch {
                        request: memory_request_id,
                        expected: size,
                        actual: data.len() as u64,
                    });
                }
                let mask =
                    byte_mask
                        .as_ref()
                        .ok_or(CpuTranslationFrontendError::MissingByteMask {
                            request: memory_request_id,
                        })?;
                if mask.len() != size.bytes() {
                    return Err(CpuTranslationFrontendError::ByteMaskSizeMismatch {
                        request: memory_request_id,
                        expected: size,
                        actual: mask.len(),
                    });
                }
            }
            CpuTranslatedMemoryOperation::InstructionFetch | CpuTranslatedMemoryOperation::Read => {
                if write_data.is_some() {
                    return Err(CpuTranslationFrontendError::UnexpectedWriteData {
                        request: memory_request_id,
                    });
                }
                if byte_mask.is_some() {
                    return Err(CpuTranslationFrontendError::UnexpectedByteMask {
                        request: memory_request_id,
                    });
                }
            }
        }
        match (operation, atomic_op) {
            (CpuTranslatedMemoryOperation::Atomic, Some(_)) => {}
            (CpuTranslatedMemoryOperation::Atomic, None) => {
                return Err(CpuTranslationFrontendError::Memory(
                    MemoryError::MissingAtomicOp {
                        request: memory_request_id,
                    },
                ));
            }
            (_, Some(_)) => {
                return Err(CpuTranslationFrontendError::Memory(
                    MemoryError::UnexpectedAtomicOp {
                        request: memory_request_id,
                    },
                ));
            }
            (_, None) => {}
        }

        Ok(Self {
            address_space: TranslationAddressSpaceId::global(),
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            operation,
            sv39_access_context: RiscvSv39AccessContext::new(RiscvPrivilegeMode::Machine),
            write_data,
            byte_mask,
            atomic_op,
        })
    }

    pub const fn address_space(&self) -> TranslationAddressSpaceId {
        self.address_space
    }

    pub fn in_address_space(mut self, address_space: TranslationAddressSpaceId) -> Self {
        self.address_space = address_space;
        self
    }

    pub const fn with_sv39_access_context(mut self, context: RiscvSv39AccessContext) -> Self {
        self.sv39_access_context = context;
        self
    }

    pub const fn sv39_access_context(&self) -> RiscvSv39AccessContext {
        self.sv39_access_context
    }

    pub const fn translation_id(&self) -> TranslationRequestId {
        self.translation_id
    }

    pub const fn memory_request_id(&self) -> MemoryRequestId {
        self.memory_request_id
    }

    pub const fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn virtual_address(&self) -> Address {
        self.virtual_address
    }

    pub const fn size(&self) -> AccessSize {
        self.size
    }

    pub const fn operation(&self) -> CpuTranslatedMemoryOperation {
        self.operation
    }

    pub fn translation_request(&self) -> Result<TranslationRequest, TranslationError> {
        TranslationRequest::new(
            self.translation_id,
            self.virtual_address,
            self.size,
            self.operation.translation_access_kind(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTranslatedMemoryRequest {
    request: CpuTranslationRequest,
    physical_address: Address,
}

impl CpuTranslatedMemoryRequest {
    pub fn new(request: CpuTranslationRequest, physical_address: Address) -> Self {
        Self {
            request,
            physical_address,
        }
    }

    pub const fn translation_id(&self) -> TranslationRequestId {
        self.request.translation_id()
    }

    pub const fn memory_request_id(&self) -> MemoryRequestId {
        self.request.memory_request_id()
    }

    pub const fn route(&self) -> MemoryRouteId {
        self.request.route()
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        self.request.endpoint()
    }

    pub const fn virtual_address(&self) -> Address {
        self.request.virtual_address()
    }

    pub const fn physical_address(&self) -> Address {
        self.physical_address
    }

    pub const fn size(&self) -> AccessSize {
        self.request.size()
    }

    pub const fn operation(&self) -> &CpuTranslatedMemoryOperation {
        &self.request.operation
    }

    pub fn memory_request(
        &self,
        line_layout: CacheLineLayout,
    ) -> Result<MemoryRequest, CpuTranslationFrontendError> {
        match self.request.operation {
            CpuTranslatedMemoryOperation::InstructionFetch => MemoryRequest::instruction_fetch(
                self.request.memory_request_id,
                self.physical_address,
                self.request.size,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
            CpuTranslatedMemoryOperation::Read => MemoryRequest::read_shared(
                self.request.memory_request_id,
                self.physical_address,
                self.request.size,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
            CpuTranslatedMemoryOperation::Write => MemoryRequest::write(
                self.request.memory_request_id,
                self.physical_address,
                self.request.size,
                self.request.write_data.clone().ok_or(
                    CpuTranslationFrontendError::MissingWriteData {
                        request: self.request.memory_request_id,
                    },
                )?,
                self.request.byte_mask.clone().ok_or(
                    CpuTranslationFrontendError::MissingByteMask {
                        request: self.request.memory_request_id,
                    },
                )?,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
            CpuTranslatedMemoryOperation::Atomic => MemoryRequest::atomic_with_op(
                self.request.memory_request_id,
                self.physical_address,
                self.request.size,
                self.request
                    .atomic_op
                    .ok_or(CpuTranslationFrontendError::Memory(
                        MemoryError::MissingAtomicOp {
                            request: self.request.memory_request_id,
                        },
                    ))?,
                self.request.write_data.clone().ok_or(
                    CpuTranslationFrontendError::MissingWriteData {
                        request: self.request.memory_request_id,
                    },
                )?,
                self.request.byte_mask.clone().ok_or(
                    CpuTranslationFrontendError::MissingByteMask {
                        request: self.request.memory_request_id,
                    },
                )?,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTranslatedMemorySegment {
    translation_id: TranslationRequestId,
    memory_request_id: MemoryRequestId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    virtual_address: Address,
    physical_address: Address,
    size: AccessSize,
    operation: CpuTranslatedMemoryOperation,
    write_data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
    atomic_op: Option<MemoryAtomicOp>,
}

impl CpuTranslatedMemorySegment {
    fn new(
        request: &CpuTranslationRequest,
        segment: &TranslationSegment,
    ) -> Result<Self, CpuTranslationFrontendError> {
        let range = segment_slice_range(request, segment.virtual_start(), segment.size())?;
        let write_data = request
            .write_data
            .as_ref()
            .map(|data| data[range.clone()].to_vec());
        let byte_mask = request
            .byte_mask
            .as_ref()
            .map(|mask| ByteMask::from_bits(mask.bits()[range].to_vec()))
            .transpose()
            .map_err(CpuTranslationFrontendError::Memory)?;

        Ok(Self {
            translation_id: request.translation_id(),
            memory_request_id: request.memory_request_id(),
            route: request.route(),
            endpoint: request.endpoint().clone(),
            virtual_address: segment.virtual_start(),
            physical_address: segment.physical_start(),
            size: segment.size(),
            operation: request.operation(),
            write_data,
            byte_mask,
            atomic_op: request.atomic_op,
        })
    }

    pub const fn translation_id(&self) -> TranslationRequestId {
        self.translation_id
    }

    pub const fn memory_request_id(&self) -> MemoryRequestId {
        self.memory_request_id
    }

    pub const fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn virtual_address(&self) -> Address {
        self.virtual_address
    }

    pub const fn physical_address(&self) -> Address {
        self.physical_address
    }

    pub const fn size(&self) -> AccessSize {
        self.size
    }

    pub const fn operation(&self) -> CpuTranslatedMemoryOperation {
        self.operation
    }

    pub fn write_data(&self) -> Option<&[u8]> {
        self.write_data.as_deref()
    }

    pub const fn byte_mask(&self) -> Option<&ByteMask> {
        self.byte_mask.as_ref()
    }

    pub fn memory_request_with_id(
        &self,
        request_id: MemoryRequestId,
        line_layout: CacheLineLayout,
    ) -> Result<MemoryRequest, CpuTranslationFrontendError> {
        match self.operation {
            CpuTranslatedMemoryOperation::InstructionFetch => MemoryRequest::instruction_fetch(
                request_id,
                self.physical_address,
                self.size,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
            CpuTranslatedMemoryOperation::Read => MemoryRequest::read_shared(
                request_id,
                self.physical_address,
                self.size,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
            CpuTranslatedMemoryOperation::Write => MemoryRequest::write(
                request_id,
                self.physical_address,
                self.size,
                self.write_data
                    .clone()
                    .ok_or(CpuTranslationFrontendError::MissingWriteData {
                        request: self.memory_request_id,
                    })?,
                self.byte_mask
                    .clone()
                    .ok_or(CpuTranslationFrontendError::MissingByteMask {
                        request: self.memory_request_id,
                    })?,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
            CpuTranslatedMemoryOperation::Atomic => MemoryRequest::atomic_with_op(
                request_id,
                self.physical_address,
                self.size,
                self.atomic_op.ok_or(CpuTranslationFrontendError::Memory(
                    MemoryError::MissingAtomicOp {
                        request: self.memory_request_id,
                    },
                ))?,
                self.write_data
                    .clone()
                    .ok_or(CpuTranslationFrontendError::MissingWriteData {
                        request: self.memory_request_id,
                    })?,
                self.byte_mask
                    .clone()
                    .ok_or(CpuTranslationFrontendError::MissingByteMask {
                        request: self.memory_request_id,
                    })?,
                line_layout,
            )
            .map_err(CpuTranslationFrontendError::Memory),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTranslationFaultRecord {
    translation_id: TranslationRequestId,
    memory_request_id: MemoryRequestId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    virtual_address: Address,
    size: AccessSize,
    operation: CpuTranslatedMemoryOperation,
    fault: TranslationFault,
}

impl CpuTranslationFaultRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        translation_id: TranslationRequestId,
        memory_request_id: MemoryRequestId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        virtual_address: Address,
        size: AccessSize,
        operation: CpuTranslatedMemoryOperation,
        fault: TranslationFault,
    ) -> Self {
        Self {
            translation_id,
            memory_request_id,
            route,
            endpoint,
            virtual_address,
            size,
            operation,
            fault,
        }
    }

    pub const fn translation_id(&self) -> TranslationRequestId {
        self.translation_id
    }

    pub const fn memory_request_id(&self) -> MemoryRequestId {
        self.memory_request_id
    }

    pub const fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn virtual_address(&self) -> Address {
        self.virtual_address
    }

    pub const fn size(&self) -> AccessSize {
        self.size
    }

    pub const fn operation(&self) -> CpuTranslatedMemoryOperation {
        self.operation
    }

    pub const fn fault(&self) -> &TranslationFault {
        &self.fault
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuTranslationOutcome {
    Mapped(CpuTranslatedMemoryRequest),
    Fault(CpuTranslationFaultRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuSegmentedTranslationOutcome {
    Mapped(Vec<CpuTranslatedMemorySegment>),
    Fault(CpuTranslationFaultRecord),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTranslationFrontendSnapshot {
    queue: TranslationQueueSnapshot,
    pending: Vec<CpuTranslationRequest>,
    tlb: Option<TranslationTlbSnapshot>,
}

impl CpuTranslationFrontendSnapshot {
    pub fn new(queue: TranslationQueueSnapshot, pending: Vec<CpuTranslationRequest>) -> Self {
        Self {
            queue,
            pending,
            tlb: None,
        }
    }

    pub fn new_with_tlb(
        queue: TranslationQueueSnapshot,
        pending: Vec<CpuTranslationRequest>,
        tlb: TranslationTlbSnapshot,
    ) -> Self {
        Self {
            queue,
            pending,
            tlb: Some(tlb),
        }
    }

    pub const fn queue(&self) -> &TranslationQueueSnapshot {
        &self.queue
    }

    pub fn pending(&self) -> &[CpuTranslationRequest] {
        &self.pending
    }

    pub const fn tlb(&self) -> Option<&TranslationTlbSnapshot> {
        self.tlb.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTranslationFrontend {
    queue: TranslationQueue,
    pending: BTreeMap<TranslationRequestId, CpuTranslationRequest>,
    tlb: Option<TranslationTlb>,
}

impl CpuTranslationFrontend {
    pub fn new(config: TranslationQueueConfig) -> Self {
        Self {
            queue: TranslationQueue::new(config),
            pending: BTreeMap::new(),
            tlb: None,
        }
    }

    pub fn with_tlb(config: TranslationQueueConfig, tlb_config: TranslationTlbConfig) -> Self {
        Self {
            queue: TranslationQueue::new(config),
            pending: BTreeMap::new(),
            tlb: Some(TranslationTlb::new(tlb_config)),
        }
    }

    pub fn from_snapshot(
        snapshot: &CpuTranslationFrontendSnapshot,
    ) -> Result<Self, CpuTranslationFrontendError> {
        let queue = TranslationQueue::from_snapshot(snapshot.queue())
            .map_err(CpuTranslationFrontendError::Translation)?;
        let mut pending = BTreeMap::new();
        for request in snapshot.pending() {
            if pending
                .insert(request.translation_id(), request.clone())
                .is_some()
            {
                return Err(CpuTranslationFrontendError::DuplicatePendingMetadata {
                    request: request.translation_id(),
                });
            }
        }

        let queue_ids: BTreeSet<_> = queue.pending_request_ids().into_iter().collect();
        let pending_ids: BTreeSet<_> = pending.keys().copied().collect();
        if let Some(request) = queue_ids.difference(&pending_ids).next() {
            return Err(CpuTranslationFrontendError::SnapshotMissingPending { request: *request });
        }
        if let Some(request) = pending_ids.difference(&queue_ids).next() {
            return Err(CpuTranslationFrontendError::SnapshotOrphanPending { request: *request });
        }
        let tlb = snapshot
            .tlb()
            .map(TranslationTlb::from_snapshot)
            .transpose()
            .map_err(CpuTranslationFrontendError::Translation)?;

        Ok(Self {
            queue,
            pending,
            tlb,
        })
    }

    pub fn restore(
        &mut self,
        snapshot: &CpuTranslationFrontendSnapshot,
    ) -> Result<(), CpuTranslationFrontendError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub const fn tlb(&self) -> Option<&TranslationTlb> {
        self.tlb.as_ref()
    }

    pub fn tlb_mut(&mut self) -> Option<&mut TranslationTlb> {
        self.tlb.as_mut()
    }

    pub fn enqueue(
        &mut self,
        issue_tick: u64,
        request: CpuTranslationRequest,
    ) -> Result<(), CpuTranslationFrontendError> {
        let translation = request
            .translation_request()
            .map_err(CpuTranslationFrontendError::Translation)?;
        self.enqueue_translation(issue_tick, request, translation)
    }

    pub fn enqueue_or_translate_cached(
        &mut self,
        issue_tick: u64,
        request: CpuTranslationRequest,
    ) -> Result<Option<CpuTranslationOutcome>, CpuTranslationFrontendError> {
        let translation = request
            .translation_request()
            .map_err(CpuTranslationFrontendError::Translation)?;
        if let Some(tlb) = &mut self.tlb {
            let cached = tlb
                .lookup_cached_in_address_space(request.address_space(), &translation)
                .map_err(CpuTranslationFrontendError::Translation)?;
            if let Some(lookup) = cached {
                return Ok(Some(Self::outcome_from_resolution(
                    request,
                    lookup.resolution().clone(),
                )));
            }
        }

        self.enqueue_translation(issue_tick, request, translation)?;
        Ok(None)
    }

    fn enqueue_translation(
        &mut self,
        issue_tick: u64,
        request: CpuTranslationRequest,
        translation: TranslationRequest,
    ) -> Result<(), CpuTranslationFrontendError> {
        self.queue
            .enqueue(issue_tick, translation)
            .map_err(CpuTranslationFrontendError::Translation)?;
        self.pending.insert(request.translation_id(), request);
        Ok(())
    }

    pub fn pending_request_ids(&self) -> Vec<TranslationRequestId> {
        self.queue.pending_request_ids()
    }

    pub fn ready_request_ids(&self, tick: u64) -> Vec<TranslationRequestId> {
        self.queue.ready_request_ids(tick)
    }

    pub fn ready_cpu_requests(&self, tick: u64) -> Vec<CpuTranslationRequest> {
        self.queue
            .ready_request_ids(tick)
            .into_iter()
            .map(|request| {
                self.pending
                    .get(&request)
                    .expect("translation queue ready request has matching CPU metadata")
                    .clone()
            })
            .collect()
    }

    pub fn complete(
        &mut self,
        request: TranslationRequestId,
        resolution: TranslationResolution,
    ) -> Result<CpuTranslationOutcome, CpuTranslationFrontendError> {
        let completion = self
            .queue
            .complete(request, resolution)
            .map_err(CpuTranslationFrontendError::Translation)?;
        Ok(self.complete_translation(completion))
    }

    pub fn complete_ready<F>(&mut self, tick: u64, resolver: F) -> Vec<CpuTranslationOutcome>
    where
        F: FnMut(&TranslationRequest) -> TranslationResolution,
    {
        self.queue
            .complete_ready(tick, resolver)
            .into_iter()
            .map(|completion| self.complete_translation(completion))
            .collect()
    }

    pub(crate) fn complete_ready_with_cpu_resolver<F, T>(
        &mut self,
        tick: u64,
        mut resolver: F,
    ) -> Result<Vec<T>, CpuTranslationFrontendError>
    where
        F: FnMut(CpuTranslationRequest) -> (TranslationResolution, T),
    {
        let ready = self.queue.ready_request_ids(tick);
        let mut outcomes = Vec::with_capacity(ready.len());
        for request_id in ready {
            let pending = self
                .pending
                .get(&request_id)
                .expect("translation queue ready request has matching CPU metadata")
                .clone();
            let (resolution, outcome) = resolver(pending);
            let completion = self
                .queue
                .complete(request_id, resolution)
                .map_err(CpuTranslationFrontendError::Translation)?;
            self.take_completed_request(&completion);
            outcomes.push(outcome);
        }

        Ok(outcomes)
    }

    pub fn complete_ready_with_tlb_page_map(
        &mut self,
        tick: u64,
        page_map: &TranslationPageMap,
    ) -> Result<Vec<CpuTranslationOutcome>, CpuTranslationFrontendError> {
        let ready = self.queue.ready_request_ids(tick);
        let mut outcomes = Vec::with_capacity(ready.len());
        for request_id in ready {
            let pending = self
                .pending
                .get(&request_id)
                .expect("translation queue ready request has matching CPU metadata")
                .clone();
            let translation = pending
                .translation_request()
                .map_err(CpuTranslationFrontendError::Translation)?;
            let resolution = if let Some(tlb) = &mut self.tlb {
                tlb.fill_from_page_map_in_address_space(
                    pending.address_space(),
                    &translation,
                    page_map,
                )
                .map_err(CpuTranslationFrontendError::Translation)?
            } else {
                page_map.translate(&translation)
            };
            outcomes.push(self.complete(request_id, resolution)?);
        }

        Ok(outcomes)
    }

    pub fn complete_ready_segmented_with_page_map(
        &mut self,
        tick: u64,
        page_map: &TranslationPageMap,
    ) -> Result<Vec<CpuSegmentedTranslationOutcome>, CpuTranslationFrontendError> {
        let ready = self.queue.ready_request_ids(tick);
        let mut outcomes = Vec::with_capacity(ready.len());
        for request_id in ready {
            let pending = self
                .pending
                .get(&request_id)
                .expect("translation queue ready request has matching CPU metadata")
                .clone();
            let translation = pending
                .translation_request()
                .map_err(CpuTranslationFrontendError::Translation)?;
            let resolution = page_map.translate_segments(&translation);
            outcomes.push(self.complete_segmented(request_id, resolution)?);
        }

        Ok(outcomes)
    }

    pub fn complete_ready_segmented_with_tlb_page_map(
        &mut self,
        tick: u64,
        page_map: &TranslationPageMap,
    ) -> Result<Vec<CpuSegmentedTranslationOutcome>, CpuTranslationFrontendError> {
        let ready = self.queue.ready_request_ids(tick);
        let mut outcomes = Vec::with_capacity(ready.len());
        for request_id in ready {
            let pending = self
                .pending
                .get(&request_id)
                .expect("translation queue ready request has matching CPU metadata")
                .clone();
            let translation = pending
                .translation_request()
                .map_err(CpuTranslationFrontendError::Translation)?;
            let resolution = if let Some(tlb) = &mut self.tlb {
                tlb.fill_segments_from_page_map_in_address_space(
                    pending.address_space(),
                    &translation,
                    page_map,
                )
                .map_err(CpuTranslationFrontendError::Translation)?
            } else {
                page_map.translate_segments(&translation)
            };
            outcomes.push(self.complete_segmented(request_id, resolution)?);
        }

        Ok(outcomes)
    }

    pub fn snapshot(&self) -> CpuTranslationFrontendSnapshot {
        let queue = self.queue.snapshot();
        let pending = self
            .pending
            .values()
            .cloned()
            .collect::<Vec<CpuTranslationRequest>>();
        if let Some(tlb) = &self.tlb {
            CpuTranslationFrontendSnapshot::new_with_tlb(queue, pending, tlb.snapshot())
        } else {
            CpuTranslationFrontendSnapshot::new(queue, pending)
        }
    }

    fn complete_translation(&mut self, completion: TranslationCompletion) -> CpuTranslationOutcome {
        let request = self.take_completed_request(&completion);
        Self::outcome_from_resolution(request, completion.resolution().clone())
    }

    fn complete_segmented(
        &mut self,
        request_id: TranslationRequestId,
        resolution: TranslationSegmentedResolution,
    ) -> Result<CpuSegmentedTranslationOutcome, CpuTranslationFrontendError> {
        let queue_resolution = segmented_queue_resolution(&resolution);
        let completion = self
            .queue
            .complete(request_id, queue_resolution)
            .map_err(CpuTranslationFrontendError::Translation)?;
        let request = self.take_completed_request(&completion);
        Self::segmented_outcome_from_resolution(request, resolution)
    }

    fn take_completed_request(
        &mut self,
        completion: &TranslationCompletion,
    ) -> CpuTranslationRequest {
        self.pending
            .remove(&completion.request().id())
            .expect("translation queue completion has matching CPU metadata")
    }

    fn outcome_from_resolution(
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

    fn segmented_outcome_from_resolution(
        request: CpuTranslationRequest,
        resolution: TranslationSegmentedResolution,
    ) -> Result<CpuSegmentedTranslationOutcome, CpuTranslationFrontendError> {
        match resolution {
            TranslationSegmentedResolution::Mapped(segments) => segments
                .iter()
                .map(|segment| CpuTranslatedMemorySegment::new(&request, segment))
                .collect::<Result<Vec<_>, _>>()
                .map(CpuSegmentedTranslationOutcome::Mapped),
            TranslationSegmentedResolution::Fault(fault) => Ok(
                CpuSegmentedTranslationOutcome::Fault(CpuTranslationFaultRecord::new(
                    request.translation_id(),
                    request.memory_request_id(),
                    request.route(),
                    request.endpoint().clone(),
                    request.virtual_address(),
                    request.size(),
                    request.operation(),
                    fault,
                )),
            ),
        }
    }
}

fn segmented_queue_resolution(
    resolution: &TranslationSegmentedResolution,
) -> TranslationResolution {
    match resolution {
        TranslationSegmentedResolution::Mapped(segments) => TranslationResolution::mapped(
            segments
                .first()
                .expect("mapped segmented translation has at least one segment")
                .physical_start(),
        ),
        TranslationSegmentedResolution::Fault(fault) => TranslationResolution::fault(fault.clone()),
    }
}

fn segment_slice_range(
    request: &CpuTranslationRequest,
    virtual_start: Address,
    size: AccessSize,
) -> Result<std::ops::Range<usize>, CpuTranslationFrontendError> {
    let offset = virtual_start
        .get()
        .checked_sub(request.virtual_address().get())
        .expect("translation segment starts within CPU request");
    let start = usize::try_from(offset).map_err(|_| {
        CpuTranslationFrontendError::Memory(MemoryError::AccessSizeTooLarge {
            size: request.size(),
        })
    })?;
    let bytes = usize::try_from(size.bytes()).map_err(|_| {
        CpuTranslationFrontendError::Memory(MemoryError::AccessSizeTooLarge { size })
    })?;
    let end = start
        .checked_add(bytes)
        .expect("translation segment end fits host usize");
    Ok(start..end)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuTranslationFrontendError {
    Translation(TranslationError),
    Memory(MemoryError),
    MissingWriteData {
        request: MemoryRequestId,
    },
    UnexpectedWriteData {
        request: MemoryRequestId,
    },
    MissingByteMask {
        request: MemoryRequestId,
    },
    UnexpectedByteMask {
        request: MemoryRequestId,
    },
    PayloadSizeMismatch {
        request: MemoryRequestId,
        expected: AccessSize,
        actual: u64,
    },
    ByteMaskSizeMismatch {
        request: MemoryRequestId,
        expected: AccessSize,
        actual: u64,
    },
    DuplicatePendingMetadata {
        request: TranslationRequestId,
    },
    SnapshotMissingPending {
        request: TranslationRequestId,
    },
    SnapshotOrphanPending {
        request: TranslationRequestId,
    },
}

impl fmt::Display for CpuTranslationFrontendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Translation(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::MissingWriteData { request } => write!(
                formatter,
                "CPU memory request {} from agent {} is missing translated write data",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedWriteData { request } => write!(
                formatter,
                "CPU memory request {} from agent {} must not carry translated write data",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingByteMask { request } => write!(
                formatter,
                "CPU memory request {} from agent {} is missing translated byte mask",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedByteMask { request } => write!(
                formatter,
                "CPU memory request {} from agent {} must not carry translated byte mask",
                request.sequence(),
                request.agent().get()
            ),
            Self::PayloadSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU memory request {} from agent {} translated payload has {actual} bytes but expects {}",
                request.sequence(),
                request.agent().get(),
                expected.bytes()
            ),
            Self::ByteMaskSizeMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU memory request {} from agent {} translated byte mask has {actual} bytes but expects {}",
                request.sequence(),
                request.agent().get(),
                expected.bytes()
            ),
            Self::DuplicatePendingMetadata { request } => write!(
                formatter,
                "CPU translation metadata for request {} from agent {} is duplicated",
                request.sequence(),
                request.agent().get()
            ),
            Self::SnapshotMissingPending { request } => write!(
                formatter,
                "CPU translation snapshot is missing metadata for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::SnapshotOrphanPending { request } => write!(
                formatter,
                "CPU translation snapshot has orphan metadata for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
        }
    }
}

impl Error for CpuTranslationFrontendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Translation(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}
