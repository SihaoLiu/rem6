use std::error::Error;
use std::fmt;

use rem6_isa_riscv::{
    walk_sv39_page_table_with_context, RiscvCounterEnableCsr, RiscvCounterInhibitCsr,
    RiscvEnvironmentConfigCsr, RiscvMachineTrapCsr, RiscvPrivilegeMode, RiscvStatusWord,
    RiscvSv39AccessContext, RiscvSv39PageFault, RiscvSv39PageTableLevel, RiscvSv39Pte,
    RiscvSv39VirtualAddress, RiscvSv39WalkAdvance as IsaSv39WalkAdvance, RiscvSv39WalkState,
    RiscvSystemEvent, RiscvVectorFixedPointState,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, SchedulerContext, Tick,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryError, MemoryRequest,
    MemoryRequestId, MemoryResponse, ResponseStatus, TranslationAddressSpaceId, TranslationFault,
    TranslationPageMap, TranslationRequestId, TranslationResolution, TranslationTlbStats,
};
use rem6_mmio::{MmioBus, MmioError};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
    TransportError,
};

use crate::riscv_translation_state::DataTranslationCompletion;
pub(crate) use crate::riscv_translation_state::{PendingDataTranslation, TranslatedDataAccess};

use crate::riscv_data_issue::{
    access_address, access_size, masked_vector_memory_request_span, mmio_request,
    OutstandingDataAccess, PreparedDataParallelAccess,
};
use crate::{
    riscv_checker, riscv_data_access, CpuDataConfig, CpuTranslationFrontend,
    CpuTranslationFrontendError, CpuTranslationOutcome, CpuTranslationRequest, RiscvCore,
    RiscvCoreDriveAction, RiscvCoreState, RiscvCpuError, RiscvCpuExecutionEvent,
    RiscvDataAccessTarget,
};

mod csr;
mod helpers;

use csr::{
    read_counter_enable_csr, read_counter_inhibit_csr, read_environment_config_csr,
    read_machine_trap_csr, write_counter_enable_csr, write_counter_inhibit_csr,
    write_environment_config_csr, write_machine_trap_csr,
};
use helpers::{
    cpu_translation_outcome_from_resolution, cpu_translation_request,
    ready_translated_fetch_request, record_data_translation_fault_state,
    supports_translated_cross_line_data_access, sv39_access_kind, sv39_translation_fault_kind,
    translated_data_from_outcome, wake_suspended_hart_on_pending_interrupt,
};

const RISCV_SV39_PTE_ACCESS_BYTES: u64 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSv39PageTableResolver {
    root_table_ppn: u64,
}

impl RiscvSv39PageTableResolver {
    pub const fn new(root_table_ppn: u64) -> Self {
        Self { root_table_ppn }
    }

    pub const fn root_table_ppn(self) -> u64 {
        self.root_table_ppn
    }

    pub fn complete_ready<F>(
        &self,
        frontend: &mut CpuTranslationFrontend,
        tick: u64,
        mut read_pte: F,
    ) -> Result<Vec<RiscvSv39TranslationResult>, CpuTranslationFrontendError>
    where
        F: FnMut(Address) -> Result<RiscvSv39Pte, RiscvSv39PageFault>,
    {
        frontend.complete_ready_with_cpu_resolver(tick, |request| {
            let result = self.resolve(request, &mut read_pte);
            (result.resolution().clone(), result)
        })
    }

    pub fn resolve<F>(
        &self,
        request: CpuTranslationRequest,
        mut read_pte: F,
    ) -> RiscvSv39TranslationResult
    where
        F: FnMut(Address) -> Result<RiscvSv39Pte, RiscvSv39PageFault>,
    {
        let mut pte_addresses = Vec::with_capacity(3);
        let virtual_address = match RiscvSv39VirtualAddress::new(request.virtual_address().get()) {
            Ok(address) => address,
            Err(fault) => {
                return RiscvSv39TranslationResult::fault(request, fault, pte_addresses);
            }
        };
        let access = sv39_access_kind(request.operation());
        let walk = walk_sv39_page_table_with_context(
            self.root_table_ppn,
            virtual_address,
            access,
            request.sv39_access_context(),
            |pte_address| {
                let pte_address = Address::new(pte_address);
                pte_addresses.push(pte_address);
                read_pte(pte_address)
            },
        );

        match walk {
            Ok(walk) => {
                debug_assert_eq!(
                    pte_addresses,
                    walk.pte_addresses()
                        .iter()
                        .copied()
                        .map(Address::new)
                        .collect::<Vec<_>>()
                );
                RiscvSv39TranslationResult::mapped(
                    request,
                    Address::new(walk.physical_address()),
                    pte_addresses,
                    walk.leaf_level(),
                )
            }
            Err(fault) => RiscvSv39TranslationResult::fault(request, fault, pte_addresses),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSv39TranslationResult {
    outcome: CpuTranslationOutcome,
    resolution: TranslationResolution,
    pte_addresses: Vec<Address>,
    leaf_level: Option<RiscvSv39PageTableLevel>,
    page_fault: Option<RiscvSv39PageFault>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39PteReadRequestError {
    RequestSequenceOverflow {
        first: MemoryRequestId,
        index: usize,
    },
    Memory(MemoryError),
}

impl fmt::Display for RiscvSv39PteReadRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestSequenceOverflow { first, index } => write!(
                formatter,
                "PTE read request id sequence starting at {} from agent {} overflows at index {index}",
                first.sequence(),
                first.agent().get()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvSv39PteReadRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<MemoryError> for RiscvSv39PteReadRequestError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39PteReadResponseError {
    UnexpectedRequest {
        expected: MemoryRequestId,
        actual: MemoryRequestId,
    },
    Retry {
        request: MemoryRequestId,
    },
    MissingData {
        request: MemoryRequestId,
    },
    InvalidDataLength {
        request: MemoryRequestId,
        actual: usize,
    },
}

impl fmt::Display for RiscvSv39PteReadResponseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedRequest { expected, actual } => write!(
                formatter,
                "PTE read response request {} from agent {} does not match expected request {} from agent {}",
                actual.sequence(),
                actual.agent().get(),
                expected.sequence(),
                expected.agent().get()
            ),
            Self::Retry { request } => write!(
                formatter,
                "PTE read request {} from agent {} returned retry",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingData { request } => write!(
                formatter,
                "PTE read response for request {} from agent {} has no data",
                request.sequence(),
                request.agent().get()
            ),
            Self::InvalidDataLength { request, actual } => write!(
                formatter,
                "PTE read response for request {} from agent {} has {actual} bytes instead of {RISCV_SV39_PTE_ACCESS_BYTES}",
                request.sequence(),
                request.agent().get()
            ),
        }
    }
}

impl Error for RiscvSv39PteReadResponseError {}

pub fn decode_sv39_pte_read_response(
    expected: &MemoryRequest,
    response: &MemoryResponse,
) -> Result<RiscvSv39Pte, RiscvSv39PteReadResponseError> {
    if response.request_id() != expected.id() {
        return Err(RiscvSv39PteReadResponseError::UnexpectedRequest {
            expected: expected.id(),
            actual: response.request_id(),
        });
    }
    if response.status() != ResponseStatus::Completed {
        return Err(RiscvSv39PteReadResponseError::Retry {
            request: expected.id(),
        });
    }

    let data = response
        .data()
        .ok_or(RiscvSv39PteReadResponseError::MissingData {
            request: expected.id(),
        })?;
    let bytes: [u8; RISCV_SV39_PTE_ACCESS_BYTES as usize] =
        data.try_into()
            .map_err(|_| RiscvSv39PteReadResponseError::InvalidDataLength {
                request: expected.id(),
                actual: data.len(),
            })?;
    Ok(RiscvSv39Pte::new(u64::from_le_bytes(bytes)))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39MemoryWalkError {
    PteReadRequest(RiscvSv39PteReadRequestError),
    PteReadResponse(RiscvSv39PteReadResponseError),
}

impl fmt::Display for RiscvSv39MemoryWalkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PteReadRequest(error) => write!(formatter, "{error}"),
            Self::PteReadResponse(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvSv39MemoryWalkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::PteReadRequest(error) => Some(error),
            Self::PteReadResponse(error) => Some(error),
        }
    }
}

impl From<RiscvSv39PteReadRequestError> for RiscvSv39MemoryWalkError {
    fn from(error: RiscvSv39PteReadRequestError) -> Self {
        Self::PteReadRequest(error)
    }
}

impl From<RiscvSv39PteReadResponseError> for RiscvSv39MemoryWalkError {
    fn from(error: RiscvSv39PteReadResponseError) -> Self {
        Self::PteReadResponse(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSv39MemoryWalk {
    request: CpuTranslationRequest,
    state: RiscvSv39WalkState,
    first_pte_request: MemoryRequestId,
    line_layout: CacheLineLayout,
    pte_request: MemoryRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSv39MemoryWalkAdvance {
    ReadPte(RiscvSv39MemoryWalk),
    Complete(RiscvSv39TranslationResult),
}

impl RiscvSv39MemoryWalk {
    pub fn start(
        request: CpuTranslationRequest,
        root_table_ppn: u64,
        first_pte_request: MemoryRequestId,
        line_layout: CacheLineLayout,
    ) -> Result<RiscvSv39MemoryWalkAdvance, RiscvSv39MemoryWalkError> {
        let virtual_address = match RiscvSv39VirtualAddress::new(request.virtual_address().get()) {
            Ok(address) => address,
            Err(fault) => {
                return Ok(RiscvSv39MemoryWalkAdvance::Complete(
                    RiscvSv39TranslationResult::fault(request, fault, Vec::new()),
                ));
            }
        };
        let access = sv39_access_kind(request.operation());
        let state = match RiscvSv39WalkState::new_with_context(
            root_table_ppn,
            virtual_address,
            access,
            request.sv39_access_context(),
        ) {
            Ok(state) => state,
            Err(fault) => {
                return Ok(RiscvSv39MemoryWalkAdvance::Complete(
                    RiscvSv39TranslationResult::fault(request, fault, Vec::new()),
                ));
            }
        };
        Self::from_state(request, state, first_pte_request, line_layout)
            .map(RiscvSv39MemoryWalkAdvance::ReadPte)
    }

    fn from_state(
        request: CpuTranslationRequest,
        state: RiscvSv39WalkState,
        first_pte_request: MemoryRequestId,
        line_layout: CacheLineLayout,
    ) -> Result<Self, RiscvSv39MemoryWalkError> {
        let index = state.pte_addresses().len() - 1;
        let pte_request = sv39_pte_read_request(
            first_pte_request,
            index,
            Address::new(state.pending_pte_address()),
            line_layout,
        )?;
        Ok(Self {
            request,
            state,
            first_pte_request,
            line_layout,
            pte_request,
        })
    }

    pub const fn translation_request(&self) -> &CpuTranslationRequest {
        &self.request
    }

    pub const fn pte_request(&self) -> &MemoryRequest {
        &self.pte_request
    }

    pub fn pte_addresses(&self) -> Vec<Address> {
        sv39_pte_addresses(self.state.pte_addresses())
    }

    pub fn advance(
        self,
        response: &MemoryResponse,
    ) -> Result<RiscvSv39MemoryWalkAdvance, RiscvSv39MemoryWalkError> {
        let pte = decode_sv39_pte_read_response(&self.pte_request, response)?;
        let fault_pte_addresses = sv39_pte_addresses(self.state.pte_addresses());
        match self.state.advance(pte) {
            Ok(IsaSv39WalkAdvance::ReadPte(state)) => Self::from_state(
                self.request,
                state,
                self.first_pte_request,
                self.line_layout,
            )
            .map(RiscvSv39MemoryWalkAdvance::ReadPte),
            Ok(IsaSv39WalkAdvance::Complete(walk)) => Ok(RiscvSv39MemoryWalkAdvance::Complete(
                RiscvSv39TranslationResult::mapped(
                    self.request,
                    Address::new(walk.physical_address()),
                    sv39_pte_addresses(walk.pte_addresses()),
                    walk.leaf_level(),
                ),
            )),
            Err(fault) => Ok(RiscvSv39MemoryWalkAdvance::Complete(
                RiscvSv39TranslationResult::fault(self.request, fault, fault_pte_addresses),
            )),
        }
    }
}

fn sv39_pte_read_request(
    first_request: MemoryRequestId,
    index: usize,
    address: Address,
    line_layout: CacheLineLayout,
) -> Result<MemoryRequest, RiscvSv39PteReadRequestError> {
    let offset = u64::try_from(index).map_err(|_| {
        RiscvSv39PteReadRequestError::RequestSequenceOverflow {
            first: first_request,
            index,
        }
    })?;
    let sequence = first_request.sequence().checked_add(offset).ok_or(
        RiscvSv39PteReadRequestError::RequestSequenceOverflow {
            first: first_request,
            index,
        },
    )?;
    let id = MemoryRequestId::new(first_request.agent(), sequence);
    let pte_size = AccessSize::new(RISCV_SV39_PTE_ACCESS_BYTES)?;
    MemoryRequest::read_shared(id, address, pte_size, line_layout).map_err(Into::into)
}

fn sv39_pte_addresses(addresses: &[u64]) -> Vec<Address> {
    addresses.iter().copied().map(Address::new).collect()
}

impl RiscvSv39TranslationResult {
    fn mapped(
        request: CpuTranslationRequest,
        physical_address: Address,
        pte_addresses: Vec<Address>,
        leaf_level: RiscvSv39PageTableLevel,
    ) -> Self {
        let resolution = TranslationResolution::mapped(physical_address);
        Self {
            outcome: cpu_translation_outcome_from_resolution(request, resolution.clone()),
            resolution,
            pte_addresses,
            leaf_level: Some(leaf_level),
            page_fault: None,
        }
    }

    fn fault(
        request: CpuTranslationRequest,
        fault: RiscvSv39PageFault,
        pte_addresses: Vec<Address>,
    ) -> Self {
        let translation_fault = TranslationFault::new(
            request.virtual_address(),
            sv39_translation_fault_kind(&fault),
        );
        let resolution = TranslationResolution::fault(translation_fault);
        Self {
            outcome: cpu_translation_outcome_from_resolution(request, resolution.clone()),
            resolution,
            pte_addresses,
            leaf_level: None,
            page_fault: Some(fault),
        }
    }

    pub const fn outcome(&self) -> &CpuTranslationOutcome {
        &self.outcome
    }

    pub fn into_outcome(self) -> CpuTranslationOutcome {
        self.outcome
    }

    pub const fn resolution(&self) -> &TranslationResolution {
        &self.resolution
    }

    pub fn pte_addresses(&self) -> &[Address] {
        &self.pte_addresses
    }

    pub fn pte_read_requests(
        &self,
        first_request: MemoryRequestId,
        line_layout: CacheLineLayout,
    ) -> Result<Vec<MemoryRequest>, RiscvSv39PteReadRequestError> {
        self.pte_addresses
            .iter()
            .enumerate()
            .map(|(index, address)| {
                sv39_pte_read_request(first_request, index, *address, line_layout)
            })
            .collect()
    }

    pub const fn leaf_level(&self) -> Option<RiscvSv39PageTableLevel> {
        self.leaf_level
    }

    pub const fn page_fault(&self) -> Option<&RiscvSv39PageFault> {
        self.page_fault.as_ref()
    }
}

impl RiscvCoreState {
    pub(super) fn apply_riscv_system_event(&mut self, system_event: Option<&RiscvSystemEvent>) {
        let Some(RiscvSystemEvent::SfenceVma {
            virtual_address,
            address_space,
            ..
        }) = system_event
        else {
            return;
        };
        let address_space = match address_space {
            Some(value) => {
                let Ok(value) = u16::try_from(*value) else {
                    return;
                };
                Some(TranslationAddressSpaceId::new(value))
            }
            None => None,
        };

        self.flush_data_translation_tlb_scope(virtual_address.map(Address::new), address_space);
    }

    pub(super) fn flush_data_translation_tlb_scope(
        &mut self,
        virtual_address: Option<Address>,
        address_space: Option<TranslationAddressSpaceId>,
    ) -> Option<usize> {
        let frontend = self.data_translation.as_mut()?;
        let Some(tlb) = frontend.tlb_mut() else {
            return Some(0);
        };

        Some(match (virtual_address, address_space) {
            (None, None) => tlb.flush_all(),
            (None, Some(address_space)) => tlb.flush_non_global_address_space(address_space),
            (Some(virtual_address), None) => tlb.demap_page_all_address_spaces(virtual_address),
            (Some(virtual_address), Some(address_space)) => {
                tlb.demap_non_global_page(address_space, virtual_address)
            }
        })
    }

    pub(super) fn flush_data_translation_tlb_range(
        &mut self,
        virtual_range: Option<AddressRange>,
        address_space: Option<TranslationAddressSpaceId>,
    ) -> Option<usize> {
        let frontend = self.data_translation.as_mut()?;
        let Some(tlb) = frontend.tlb_mut() else {
            return Some(0);
        };

        Some(match (virtual_range, address_space) {
            (None, None) => tlb.flush_all(),
            (None, Some(address_space)) => tlb.flush_non_global_address_space(address_space),
            (Some(virtual_range), None) => tlb.demap_range_all_address_spaces(virtual_range),
            (Some(virtual_range), Some(address_space)) => {
                tlb.demap_non_global_range(address_space, virtual_range)
            }
        })
    }

    pub(super) fn flush_data_translation_tlb_physical_range(
        &mut self,
        physical_range: Option<AddressRange>,
    ) -> Option<usize> {
        let frontend = self.data_translation.as_mut()?;
        let Some(tlb) = frontend.tlb_mut() else {
            return Some(0);
        };

        Some(match physical_range {
            None => tlb.flush_all(),
            Some(physical_range) => tlb.demap_physical_range_all_address_spaces(physical_range),
        })
    }

    pub(super) fn next_unissued_data_access(
        &self,
    ) -> Option<(MemoryRequestId, rem6_isa_riscv::MemoryAccessKind)> {
        self.events.iter().find_map(|event| {
            let fetch_request = event.fetch().request_id();
            if self.issued_data_for_fetches.contains(&fetch_request) {
                return None;
            }
            if self
                .pending_data_translations
                .values()
                .any(|pending| pending.fetch_request() == fetch_request)
            {
                return None;
            }
            if self.ready_translated_data.contains_key(&fetch_request) {
                return None;
            }
            event
                .execution()
                .memory_access()
                .map(|access| (fetch_request, access.clone()))
        })
    }
}

impl RiscvCore {
    pub fn privilege_mode(&self) -> RiscvPrivilegeMode {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .privilege_mode()
    }

    pub fn machine_interrupt_pending(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .machine_interrupt_pending()
    }

    pub fn machine_interrupt_enable(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .machine_interrupt_enable()
    }

    pub fn set_machine_interrupt_enable(&self, enable: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_machine_interrupt_enable(enable);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_machine_interrupt_pending(&self, pending: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_machine_interrupt_pending(pending);
        wake_suspended_hart_on_pending_interrupt(&mut state, pending);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_machine_interrupt_pending_bits(&self, bits: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        let pending = state.hart.machine_interrupt_pending() | bits;
        state.hart.set_machine_interrupt_pending(pending);
        wake_suspended_hart_on_pending_interrupt(&mut state, pending);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn clear_machine_interrupt_pending_bits(&self, bits: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        let pending = state.hart.machine_interrupt_pending() & !bits;
        state.hart.set_machine_interrupt_pending(pending);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn supervisor_exception_pc(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .supervisor_exception_pc()
    }

    pub fn status(&self) -> RiscvStatusWord {
        self.state.lock().expect("riscv core lock").hart.status()
    }

    pub fn float_status(&self) -> rem6_isa_riscv::RiscvFloatStatus {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .float_status()
    }

    pub fn vector_fixed_point(&self) -> RiscvVectorFixedPointState {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .vector_fixed_point()
    }

    pub fn supervisor_trap_vector(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .supervisor_trap_vector()
    }

    pub fn supervisor_scratch(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .supervisor_scratch()
    }

    pub fn supervisor_trap_cause(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .supervisor_trap_cause()
    }

    pub fn supervisor_trap_value(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .supervisor_trap_value()
    }

    pub fn environment_config_csr(&self, csr: RiscvEnvironmentConfigCsr) -> u64 {
        let state = self.state.lock().expect("riscv core lock");
        read_environment_config_csr(&state.hart, csr)
    }

    pub fn set_environment_config_csr(&self, csr: RiscvEnvironmentConfigCsr, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        write_environment_config_csr(&mut state.hart, csr, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn counter_enable_csr(&self, csr: RiscvCounterEnableCsr) -> u64 {
        let state = self.state.lock().expect("riscv core lock");
        read_counter_enable_csr(&state.hart, csr)
    }

    pub fn set_counter_enable_csr(&self, csr: RiscvCounterEnableCsr, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        write_counter_enable_csr(&mut state.hart, csr, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn counter_inhibit_csr(&self, csr: RiscvCounterInhibitCsr) -> u64 {
        let state = self.state.lock().expect("riscv core lock");
        read_counter_inhibit_csr(&state.hart, csr)
    }

    pub fn set_counter_inhibit_csr(&self, csr: RiscvCounterInhibitCsr, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        write_counter_inhibit_csr(&mut state.hart, csr, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn machine_exception_pc(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .machine_exception_pc()
    }

    pub fn machine_trap_cause(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .machine_trap_cause()
    }

    pub fn machine_trap_value(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .machine_trap_value()
    }

    pub fn machine_trap_csr(&self, csr: RiscvMachineTrapCsr) -> u64 {
        let state = self.state.lock().expect("riscv core lock");
        read_machine_trap_csr(&state.hart, csr)
    }

    pub fn set_machine_trap_csr(&self, csr: RiscvMachineTrapCsr, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        write_machine_trap_csr(&mut state.hart, csr, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_machine_exception_delegation(&self, delegation: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_machine_exception_delegation(delegation);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_supervisor_trap_vector(&self, vector: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_supervisor_trap_vector(vector);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_supervisor_scratch(&self, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_supervisor_scratch(value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_supervisor_exception_pc(&self, pc: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_supervisor_exception_pc(pc);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_supervisor_trap_cause(&self, cause: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_supervisor_trap_cause(cause);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_supervisor_trap_value(&self, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_supervisor_trap_value(value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_machine_trap_vector(&self, vector: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_machine_trap_vector(vector);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub(crate) fn take_pending_trap_event(&self) -> Option<RiscvCpuExecutionEvent> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pending_trap_event
            .take()
    }

    pub fn with_data_translation(
        core: crate::CpuCore,
        data: CpuDataConfig,
        data_translation: CpuTranslationFrontend,
    ) -> Self {
        let core = Self::with_data(core, data);
        core.state.lock().expect("riscv core lock").data_translation = Some(data_translation);
        core
    }

    pub fn data_translation_address_space(&self) -> TranslationAddressSpaceId {
        TranslationAddressSpaceId::new(
            self.state
                .lock()
                .expect("riscv core lock")
                .hart
                .translation_address_space(),
        )
    }

    pub fn translation_satp(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .translation_satp()
    }

    pub fn set_translation_satp(&self, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_translation_satp(value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_data_translation_address_space(&self, address_space: TranslationAddressSpaceId) {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .hart
            .set_translation_address_space(address_space.get());
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_sv39_access_context(&self, context: RiscvSv39AccessContext) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_sv39_access_context(context);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_privilege_mode(&self, privilege: RiscvPrivilegeMode) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_privilege_mode(privilege);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_status(&self, status: RiscvStatusWord) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_status(status);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_float_status(&self, status: rem6_isa_riscv::RiscvFloatStatus) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_float_status(status);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn set_vector_fixed_point(&self, state: RiscvVectorFixedPointState) {
        let mut core_state = self.state.lock().expect("riscv core lock");
        core_state.hart.set_vector_fixed_point(state);
        riscv_checker::sync_checker_hart(&mut core_state);
    }

    pub fn ready_data_translation_requests(&self, tick: Tick) -> Vec<CpuTranslationRequest> {
        let state = self.state.lock().expect("riscv core lock");
        state
            .data_translation
            .as_ref()
            .map_or_else(Vec::new, |frontend| frontend.ready_cpu_requests(tick))
    }

    pub fn data_translation_tlb_stats(&self) -> Option<TranslationTlbStats> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_translation
            .as_ref()
            .and_then(|frontend| frontend.tlb().map(|tlb| tlb.stats()))
    }

    pub fn data_translation_tlb_entry_count(&self) -> Option<usize> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_translation
            .as_ref()
            .and_then(|frontend| frontend.tlb().map(|tlb| tlb.entry_count()))
    }

    pub fn data_translation_tlb_contains_entry(
        &self,
        address_space: TranslationAddressSpaceId,
        virtual_page: Address,
    ) -> Option<bool> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_translation
            .as_ref()
            .and_then(|frontend| {
                frontend
                    .tlb()
                    .map(|tlb| tlb.contains_entry(address_space, virtual_page))
            })
    }

    pub fn flush_data_translation_tlb(&self) -> Option<usize> {
        let mut state = self.state.lock().expect("riscv core lock");
        state.flush_data_translation_tlb_scope(None, None)
    }

    pub fn flush_data_translation_tlb_scope(
        &self,
        virtual_address: Option<Address>,
        address_space: Option<TranslationAddressSpaceId>,
    ) -> Option<usize> {
        let mut state = self.state.lock().expect("riscv core lock");
        state.flush_data_translation_tlb_scope(virtual_address, address_space)
    }

    pub fn flush_data_translation_tlb_range(
        &self,
        virtual_range: Option<AddressRange>,
        address_space: Option<TranslationAddressSpaceId>,
    ) -> Option<usize> {
        let mut state = self.state.lock().expect("riscv core lock");
        state.flush_data_translation_tlb_range(virtual_range, address_space)
    }

    pub fn flush_data_translation_tlb_physical_range(
        &self,
        physical_range: Option<AddressRange>,
    ) -> Option<usize> {
        let mut state = self.state.lock().expect("riscv core lock");
        state.flush_data_translation_tlb_physical_range(physical_range)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_next_action_with_data_translation<F, D>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        if !self.is_hart_started() {
            return Ok(None);
        }
        if let Some(event) = self.take_pending_trap_event() {
            return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                event,
            ))));
        }
        if self.has_outstanding_data_request() {
            return Ok(None);
        }
        if self.has_pending_trap() {
            return Ok(None);
        }
        if self.core.has_pending_fetch() {
            if self.has_pending_data_access() {
                return Ok(None);
            }
            if !self.can_retire_completed_fetch_while_fetch_pending()? {
                self.record_in_order_fetch_wait_stall_cycle()?;
                return Ok(None);
            }
            if let Some(event) = self.execute_next_completed_fetch_serial(scheduler)? {
                return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                    event,
                ))));
            }
            if self.live_retire_gate_blocks_new_work() {
                return Ok(None);
            }
            self.record_in_order_fetch_wait_stall_cycle()?;
            return Ok(None);
        }

        if let Some(decision) = self.next_fetch_ahead_before_retire() {
            let fetch_ahead = self.prepare_fetch_ahead_speculation(&decision)?;
            self.set_fetch_ahead_pc(decision.pc());
            let event = self.issue_next_fetch_with_prepared_fetch_ahead(
                scheduler,
                transport,
                fetch_trace,
                fetch_responder,
                fetch_ahead,
            )?;
            return Ok(Some(RiscvCoreDriveAction::FetchIssued { event }));
        }

        if let Some(event) = self.execute_next_completed_fetch_serial(scheduler)? {
            return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                event,
            ))));
        }
        if self.live_retire_gate_blocks_new_work() {
            return Ok(None);
        }

        let had_unissued_data = self.has_unissued_data_access();
        if let Some(event) = self.issue_next_translated_data_access(
            scheduler,
            transport,
            data_trace,
            page_map,
            data_responder,
        )? {
            return Ok(Some(RiscvCoreDriveAction::DataAccessIssued { event }));
        }
        if let Some(event) = self.take_pending_trap_event() {
            return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                event,
            ))));
        }
        if had_unissued_data || self.has_pending_data_access() {
            return Ok(None);
        }

        let event = self.issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)?;
        Ok(Some(RiscvCoreDriveAction::FetchIssued { event }))
    }

    pub fn issue_next_translated_data_access<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        page_map: &TranslationPageMap,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let Some(issue) =
            self.prepare_next_translated_data_access(scheduler.now(), transport, page_map)?
        else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return self
                .schedule_store_conditional_failure(scheduler, issue)
                .map(Some);
        }
        let request = self.apply_pma_data_request_attributes(
            issue.fetch_request,
            issue.physical_address,
            issue.size,
            issue.memory_request()?,
        )?;

        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                issue.memory_route(),
                request,
                trace,
                responder,
                move |delivery| core.record_data_response(delivery),
            )
            .map_err(RiscvCpuError::Transport)?;

        self.record_data_issue(issue);
        Ok(Some(event))
    }

    pub fn issue_next_translated_data_access_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        page_map: &TranslationPageMap,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(prepared) = self.prepare_translated_data_parallel_access(
            scheduler.now(),
            transport,
            trace,
            page_map,
            responder,
        )?
        else {
            return Ok(None);
        };

        match prepared {
            PreparedDataParallelAccess::Transaction { issue, transaction } => {
                let event = transport
                    .submit_parallel_batch(scheduler, [transaction])
                    .map_err(RiscvCpuError::Transport)?
                    .into_iter()
                    .next()
                    .expect("single translated data transaction returns one event");

                self.record_data_issue(issue);
                Ok(Some(event))
            }
            PreparedDataParallelAccess::ConditionalFailed { issue } => self
                .schedule_store_conditional_failure_parallel(scheduler, issue)
                .map(Some),
        }
    }

    pub fn issue_next_translated_mmio_data_access_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        bus: &MmioBus,
        page_map: &TranslationPageMap,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError> {
        let Some(issue) =
            self.prepare_next_translated_mmio_data_access(scheduler, bus, page_map)?
        else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return self
                .schedule_store_conditional_failure_parallel(scheduler, issue)
                .map(Some);
        }
        let request = issue.mmio_request()?;
        let bus = bus.clone();
        let core = self.clone();
        let request_id = issue.request_id;
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), move |context| {
                bus.submit_parallel(context, request, move |completion| {
                    core.record_mmio_completion(request_id, completion);
                })
                .expect("validated translated parallel MMIO data access submission");
            })
            .map_err(RiscvCpuError::Scheduler)?;

        self.record_data_issue(issue);
        Ok(Some(event))
    }

    pub(crate) fn prepare_translated_data_parallel_access<F>(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        page_map: &TranslationPageMap,
        responder: F,
    ) -> Result<Option<PreparedDataParallelAccess>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(issue) = self.prepare_next_translated_data_access(tick, transport, page_map)?
        else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return Ok(Some(PreparedDataParallelAccess::ConditionalFailed {
                issue,
            }));
        }
        let request = self.apply_pma_data_request_attributes(
            issue.fetch_request,
            issue.physical_address,
            issue.size,
            issue.memory_request()?,
        )?;
        let core = self.clone();
        let transaction = ParallelMemoryTransaction::new(
            issue.memory_route(),
            request,
            trace,
            responder,
            move |delivery| core.record_data_response(delivery),
        );

        Ok(Some(PreparedDataParallelAccess::Transaction {
            issue,
            transaction,
        }))
    }

    fn prepare_next_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        page_map: &TranslationPageMap,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        self.complete_ready_data_translations_with_page_map(tick, page_map)?;
        let mut issue = self.prepare_ready_translated_data_access(tick, transport)?;
        if issue.is_none() && self.enqueue_next_data_translation(tick)? {
            self.complete_ready_data_translations_with_page_map(tick, page_map)?;
            issue = self.prepare_ready_translated_data_access(tick, transport)?;
        }

        Ok(issue)
    }

    fn prepare_next_translated_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
        page_map: &TranslationPageMap,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let tick = scheduler.now();
        self.complete_ready_data_translations_with_page_map(tick, page_map)?;
        let mut issue = self.prepare_ready_translated_mmio_data_access(scheduler, bus)?;
        if issue.is_none() && self.enqueue_next_data_translation(tick)? {
            self.complete_ready_data_translations_with_page_map(tick, page_map)?;
            issue = self.prepare_ready_translated_mmio_data_access(scheduler, bus)?;
        }

        Ok(issue)
    }

    fn enqueue_next_data_translation(&self, tick: Tick) -> Result<bool, RiscvCpuError> {
        let Some((fetch_request, access)) = self.next_unissued_data_access() else {
            return Ok(false);
        };
        let base_address = Address::new(access_address(&access));
        let base_size = access_size(&access)?;
        let request_span = masked_vector_memory_request_span(&access, base_address, base_size)?;
        let (data, address_space, access_context) = {
            let state = self.state.lock().expect("riscv core lock");
            (
                state.data.clone().ok_or(RiscvCpuError::MissingDataConfig {
                    fetch: fetch_request,
                })?,
                TranslationAddressSpaceId::new(state.hart.translation_address_space()),
                state.hart.data_sv39_access_context(),
            )
        };
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        let translation_id = TranslationRequestId::new(self.core.agent(), request_id.sequence());
        let pending = PendingDataTranslation {
            request_id,
            fetch_request,
            access: access.clone(),
            virtual_address: request_span.address,
            size: request_span.size,
            request_byte_offset: request_span.byte_offset,
        };
        let request = cpu_translation_request(
            translation_id,
            request_id,
            &data,
            &access,
            request_span.address,
            request_span.size,
            request_span.byte_offset,
        )?
        .in_address_space(address_space)
        .with_sv39_access_context(access_context);

        let mut state = self.state.lock().expect("riscv core lock");
        let frontend =
            state
                .data_translation
                .as_mut()
                .ok_or(RiscvCpuError::MissingDataTranslationConfig {
                    fetch: fetch_request,
                })?;
        match frontend
            .enqueue_or_translate_cached(tick, request)
            .map_err(RiscvCpuError::DataTranslation)?
        {
            Some(outcome) => match translated_data_from_outcome(pending, outcome) {
                DataTranslationCompletion::Access(translated) => {
                    state
                        .ready_translated_data
                        .insert(translated.fetch_request, translated);
                }
                DataTranslationCompletion::Fault {
                    fetch_request,
                    fault,
                } => {
                    let next_pc =
                        record_data_translation_fault_state(&mut state, fetch_request, fault)?;
                    self.core.set_pc(next_pc);
                }
            },
            None => {
                state
                    .pending_data_translations
                    .insert(translation_id, pending);
            }
        }

        Ok(true)
    }

    fn complete_ready_data_translations_with_page_map(
        &self,
        tick: Tick,
        page_map: &TranslationPageMap,
    ) -> Result<(), RiscvCpuError> {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(frontend) = state.data_translation.as_mut() else {
            return Ok(());
        };
        let outcomes = frontend
            .complete_ready_with_tlb_page_map(tick, page_map)
            .map_err(RiscvCpuError::DataTranslation)?;

        for outcome in outcomes {
            let translation_id = match &outcome {
                CpuTranslationOutcome::Mapped(mapped) => mapped.translation_id(),
                CpuTranslationOutcome::Fault(fault) => fault.translation_id(),
            };
            let pending = state
                .pending_data_translations
                .remove(&translation_id)
                .expect("ready data translation has matching RISC-V metadata");
            match translated_data_from_outcome(pending, outcome) {
                DataTranslationCompletion::Access(translated) => {
                    state
                        .ready_translated_data
                        .insert(translated.fetch_request, translated);
                }
                DataTranslationCompletion::Fault {
                    fetch_request,
                    fault,
                } => {
                    let next_pc =
                        record_data_translation_fault_state(&mut state, fetch_request, fault)?;
                    self.core.set_pc(next_pc);
                }
            }
        }

        Ok(())
    }

    fn prepare_ready_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let translated = {
            let state = self.state.lock().expect("riscv core lock");
            let Some(fetch_request) = ready_translated_fetch_request(&state) else {
                return Ok(None);
            };
            state
                .ready_translated_data
                .get(&fetch_request)
                .expect("selected ready data translation exists")
                .clone()
        };

        let issue = self.prepare_translated_data_access(tick, transport, translated)?;
        {
            let mut state = self.state.lock().expect("riscv core lock");
            state
                .ready_translated_data
                .remove(&issue.fetch_request)
                .expect("selected ready data translation exists");
        }
        Ok(Some(issue))
    }

    fn prepare_ready_translated_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let tick = scheduler.now();
        let translated = {
            let state = self.state.lock().expect("riscv core lock");
            let Some(fetch_request) = ready_translated_fetch_request(&state) else {
                return Ok(None);
            };
            state
                .ready_translated_data
                .get(&fetch_request)
                .expect("selected ready data translation exists")
                .clone()
        };

        self.check_pmp_data_access(
            translated.fetch_request,
            &translated.access,
            translated.size,
            translated.physical_address,
        )?;
        self.check_pma_data_access(
            translated.fetch_request,
            &translated.access,
            translated.size,
            translated.physical_address,
            translated.request_byte_offset,
        )?;
        let request = mmio_request(
            translated.request_id,
            &translated.access,
            translated.size,
            translated.physical_address,
            translated.request_byte_offset,
        )?;
        let route = match bus.route_for(self.core.partition(), &request) {
            Ok(route) => route,
            Err(MmioError::UnmappedAddress { .. }) => return Ok(None),
            Err(error) => return Err(RiscvCpuError::Mmio(error)),
        };
        if route.source_partition() != self.core.partition() {
            return Err(RiscvCpuError::MmioRoutePartitionMismatch {
                expected: self.core.partition(),
                actual: route.source_partition(),
            });
        }
        riscv_data_access::validate_parallel_mmio_route(
            route,
            tick,
            scheduler.min_remote_delay(),
            scheduler.partition_count(),
        )
        .map_err(|error| RiscvCpuError::Mmio(MmioError::Scheduler(error)))?;

        {
            let mut state = self.state.lock().expect("riscv core lock");
            state
                .ready_translated_data
                .remove(&translated.fetch_request)
                .expect("selected ready data translation exists");
        }

        Ok(Some(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Mmio { route },
            request_id: translated.request_id,
            fetch_request: translated.fetch_request,
            access: translated.access,
            size: translated.size,
            physical_address: translated.physical_address,
            request_byte_offset: translated.request_byte_offset,
            line_layout: None,
        }))
    }

    fn prepare_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        translated: TranslatedDataAccess,
    ) -> Result<OutstandingDataAccess, RiscvCpuError> {
        let data = self
            .state
            .lock()
            .expect("riscv core lock")
            .data
            .clone()
            .ok_or(RiscvCpuError::MissingDataConfig {
                fetch: translated.fetch_request,
            })?;
        let route = transport
            .route(data.route())
            .ok_or(RiscvCpuError::Transport(TransportError::UnknownRoute {
                route: data.route(),
            }))?;
        if route.source_partition() != self.core.partition() {
            return Err(RiscvCpuError::DataRoutePartitionMismatch {
                route: data.route(),
                expected: self.core.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source() != data.endpoint() {
            return Err(RiscvCpuError::DataRouteEndpointMismatch {
                route: data.route(),
                expected: data.endpoint().clone(),
                actual: route.source().clone(),
            });
        }

        self.check_pmp_data_access(
            translated.fetch_request,
            &translated.access,
            translated.size,
            translated.physical_address,
        )?;
        self.check_pma_data_access(
            translated.fetch_request,
            &translated.access,
            translated.size,
            translated.physical_address,
            translated.request_byte_offset,
        )?;
        let line_layout = data
            .line_layout_for_access(translated.physical_address, translated.size)
            .map_err(RiscvCpuError::Memory)?;
        let line_offset = line_layout.line_offset(translated.physical_address);
        if line_offset + translated.size.bytes() > line_layout.bytes()
            && !supports_translated_cross_line_data_access(
                &translated.access,
                translated.virtual_address,
                translated.physical_address,
                translated.size,
                line_layout,
            )
        {
            return Err(RiscvCpuError::DataAccessCrossesLine {
                address: translated.physical_address,
                size: translated.size,
                line_size: line_layout.bytes(),
            });
        }

        Ok(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Memory {
                route: data.route(),
                endpoint: data.endpoint().clone(),
            },
            request_id: translated.request_id,
            fetch_request: translated.fetch_request,
            access: translated.access,
            size: translated.size,
            physical_address: translated.physical_address,
            request_byte_offset: translated.request_byte_offset,
            line_layout: Some(line_layout),
        })
    }
}

#[cfg(test)]
#[path = "riscv_translation_tests.rs"]
mod tests;
