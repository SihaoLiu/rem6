use std::sync::Mutex;

use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryRequestId, MemoryResponse,
    TranslationAccessKind, TranslationAddressSpaceId, TranslationPageMap,
    TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig, TranslationRequest,
    TranslationRequestId, TranslationTlbConfig,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

use super::super::*;
use crate::{
    riscv_execution_mode_handoff::{RiscvO3LiveDataHandoffCapture, RiscvO3LiveDataHandoffTarget},
    riscv_fetch_ahead::O3MemoryResultWindowRoute,
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState,
    CpuTranslationFrontend, RiscvCore, RiscvCoreDriveAction,
};

#[path = "translated_mmio_result_pair/lifecycle.rs"]
mod lifecycle;

const HEAD_PC: u64 = 0x8000;
const YOUNGER_PC: u64 = 0x8004;
const HEAD_VIRTUAL_ADDRESS: u64 = 0x4000;
const YOUNGER_VIRTUAL_ADDRESS: u64 = 0x5000;
const HEAD_PHYSICAL_ADDRESS: u64 = 0x9000;
const MMIO_PHYSICAL_ADDRESS: u64 = 0xa000;
const HEAD_VALUE: u64 = 0x11;
const MMIO_VALUE: u64 = 0x33;
const OLD_YOUNGER_VALUE: u64 = 0x77;

#[test]
fn translated_memory_mmio_pair_records_independent_targets_and_request_ids() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    let (memory_request, mmio_request) = fixture.issue_pair(false);

    assert_ne!(memory_request, mmio_request);
    let handoff = fixture.core.capture_o3_live_data_handoff().unwrap();
    assert_eq!(handoff.entries().len(), 2);
    assert!(matches!(
        handoff.entries()[0].target(),
        RiscvO3LiveDataHandoffTarget::Memory { .. }
    ));
    assert!(matches!(
        handoff.entries()[1].target(),
        RiscvO3LiveDataHandoffTarget::Mmio { .. }
    ));
    assert_eq!(handoff.entries()[0].data_request(), memory_request);
    assert_eq!(handoff.entries()[1].data_request(), mmio_request);
}

#[test]
fn memory_and_mmio_completions_cannot_cross_complete_pair_rows() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    let (memory_request, mmio_request) = fixture.issue_pair(true);

    fixture.run_until_one_request_remains();
    let state = fixture.core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.contains_key(&memory_request));
    assert!(!state.outstanding_data.contains_key(&mmio_request));
    let snapshot = state.o3_runtime.snapshot();
    assert!(!snapshot.load_store_queue()[0].is_completed());
    assert!(snapshot.load_store_queue()[1].is_completed());
}

#[test]
fn younger_mmio_completion_before_memory_keeps_architecture_unpublished() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.issue_pair(true);

    fixture.run_until_one_request_remains();
    assert_eq!(fixture.core.read_register(reg(11)), 0);
    assert_eq!(fixture.core.read_register(reg(12)), OLD_YOUNGER_VALUE);
    assert!(fixture
        .core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_none());

    fixture.scheduler.run_until_idle_parallel().unwrap();
    let older = fixture
        .core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("older memory result retires first");
    assert_eq!(older.fetch_pc(), Address::new(HEAD_PC));
    assert_eq!(fixture.core.read_register(reg(11)), HEAD_VALUE);
    assert_eq!(fixture.core.read_register(reg(12)), OLD_YOUNGER_VALUE);
    let younger = fixture
        .core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("younger MMIO result retires second");
    assert_eq!(younger.fetch_pc(), Address::new(YOUNGER_PC));
    assert_eq!(fixture.core.read_register(reg(12)), MMIO_VALUE);
}

#[test]
fn translated_memory_mmio_pair_handoff_preserves_both_typed_targets() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.issue_pair(false);

    let handoff = fixture
        .core
        .capture_o3_live_data_handoff()
        .expect("mixed translated pair is transferable");
    assert_eq!(handoff.entries().len(), 2);
    assert!(matches!(
        handoff.entries()[0].target(),
        RiscvO3LiveDataHandoffTarget::Memory { .. }
    ));
    assert!(matches!(
        handoff.entries()[1].target(),
        RiscvO3LiveDataHandoffTarget::Mmio { .. }
    ));
}

#[test]
fn translated_result_pair_prebind_state_rejects_handoff() {
    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.authorize_and_execute_head();
    fixture
        .core
        .advance_next_data_translation(fixture.scheduler.now(), &fixture.page_map)
        .unwrap();

    assert_eq!(
        fixture.core.capture_o3_live_data_handoff_status(),
        RiscvO3LiveDataHandoffCapture::Rejected
    );

    let mut fixture = TranslatedMemoryMmioPairFixture::new();
    fixture.authorize_and_execute_head();
    fixture
        .core
        .issue_next_translated_data_access_parallel(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            &fixture.page_map,
            |_, _| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("translated memory head issues");
    assert_eq!(
        fixture.core.capture_o3_live_data_handoff_status(),
        RiscvO3LiveDataHandoffCapture::Rejected
    );
}

struct TranslatedMemoryMmioPairFixture {
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    bus: MmioBus,
    page_map: TranslationPageMap,
    core: RiscvCore,
}

impl TranslatedMemoryMmioPairFixture {
    fn new() -> Self {
        Self::with_delays(0, 2)
    }

    fn with_translation_latency(latency: u64) -> Self {
        Self::with_delays(latency, 2)
    }

    fn with_mmio_delay(delay: u64) -> Self {
        Self::with_delays(0, delay)
    }

    fn with_delays(translation_latency: u64, mmio_delay: u64) -> Self {
        let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
        let mut transport = MemoryTransport::new();
        let fetch_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu0.ifetch"),
                    PartitionId::new(0),
                    endpoint("l1i0"),
                    PartitionId::new(1),
                    2,
                    3,
                )
                .unwrap(),
            )
            .unwrap();
        let data_route = transport
            .add_route(
                MemoryRoute::new(
                    endpoint("cpu0.dmem"),
                    PartitionId::new(0),
                    endpoint("l1d0"),
                    PartitionId::new(1),
                    2,
                    3,
                )
                .unwrap(),
            )
            .unwrap();
        let core = RiscvCore::with_data_translation(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(0),
                    PartitionId::new(0),
                    AgentId::new(7),
                    Address::new(HEAD_PC),
                ),
                CpuFetchConfig::new(
                    endpoint("cpu0.ifetch"),
                    fetch_route,
                    CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
            CpuDataConfig::new(
                endpoint("cpu0.dmem"),
                data_route,
                CacheLineLayout::new(16).unwrap(),
            ),
            CpuTranslationFrontend::with_tlb(
                TranslationQueueConfig::new(4, translation_latency).unwrap(),
                TranslationTlbConfig::new(4).unwrap(),
            ),
        );
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_scalar_memory_depth(4);
        core.set_o3_issue_width(2);
        core.set_o3_memory_issue_width(2);
        core.write_register(reg(2), HEAD_VIRTUAL_ADDRESS);
        core.write_register(reg(3), YOUNGER_VIRTUAL_ADDRESS);
        core.write_register(reg(12), OLD_YOUNGER_VALUE);
        core.core
            .state
            .lock()
            .expect("cpu core lock")
            .events
            .extend([
                completed_fetch(0, HEAD_PC, ld(11, 2)),
                completed_fetch(1, YOUNGER_PC, ld(12, 3)),
            ]);

        let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
        page_map
            .map(
                Address::new(HEAD_VIRTUAL_ADDRESS),
                Address::new(HEAD_PHYSICAL_ADDRESS),
                1,
                TranslationPagePermissions::read_write_execute(),
            )
            .unwrap();
        page_map
            .map(
                Address::new(YOUNGER_VIRTUAL_ADDRESS),
                Address::new(MMIO_PHYSICAL_ADDRESS),
                1,
                TranslationPagePermissions::read_write_execute(),
            )
            .unwrap();
        install_cached_translation(&core, &page_map, HEAD_VIRTUAL_ADDRESS);

        let mut bank = MmioRegisterBank::new(
            Address::new(MMIO_PHYSICAL_ADDRESS),
            AccessSize::new(0x100).unwrap(),
        )
        .unwrap();
        bank.insert_register(
            0,
            AccessSize::new(8).unwrap(),
            MmioAccess::ReadOnly,
            MMIO_VALUE.to_le_bytes().to_vec(),
        )
        .unwrap();
        let mut bus = MmioBus::new();
        bus.insert_device(
            AddressRange::new(
                Address::new(MMIO_PHYSICAL_ADDRESS),
                AccessSize::new(0x100).unwrap(),
            )
            .unwrap(),
            MmioRoute::new(
                PartitionId::new(0),
                PartitionId::new(1),
                mmio_delay,
                mmio_delay,
            )
            .unwrap(),
            Mutex::new(bank),
        )
        .unwrap();

        Self {
            scheduler,
            transport,
            bus,
            page_map,
            core,
        }
    }

    fn authorize_and_execute_head(&mut self) {
        assert_eq!(
            self.core
                .next_cached_translated_memory_fetch_ahead_before_retire()
                .map(|decision| decision.pc()),
            Some(Address::new(0x8008))
        );
        let head = self
            .core
            .execute_next_completed_fetch()
            .unwrap()
            .or_else(|| self.core.execute_next_completed_fetch().unwrap())
            .expect("translated memory head executes");
        assert_eq!(head.fetch_pc(), Address::new(HEAD_PC));
    }

    fn issue_pair(&mut self, complete_memory: bool) -> (MemoryRequestId, MemoryRequestId) {
        self.authorize_and_execute_head();
        self.core
            .issue_next_translated_data_access_parallel(
                &mut self.scheduler,
                &self.transport,
                MemoryTrace::new(),
                &self.page_map,
                move |delivery, _context| {
                    if complete_memory {
                        TargetOutcome::RespondAfter {
                            delay: 20,
                            response: MemoryResponse::completed(
                                delivery.request(),
                                Some(HEAD_VALUE.to_le_bytes().to_vec()),
                            )
                            .unwrap(),
                        }
                    } else {
                        TargetOutcome::NoResponse
                    }
                },
            )
            .unwrap()
            .expect("translated memory head issues");
        let memory_request = *self
            .core
            .state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .keys()
            .next()
            .unwrap();

        let action = self
            .core
            .drive_next_action_with_data_translation(
                &mut self.scheduler,
                &self.transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                &self.page_map,
                |_delivery, _context| TargetOutcome::NoResponse,
                |_delivery, _context| TargetOutcome::NoResponse,
            )
            .unwrap();
        assert!(matches!(
            action,
            Some(RiscvCoreDriveAction::InstructionExecuted(event))
                if event.fetch_pc() == Address::new(YOUNGER_PC)
        ));
        self.core
            .issue_next_translated_mmio_data_access_parallel(
                &mut self.scheduler,
                &self.bus,
                &self.page_map,
            )
            .unwrap()
            .expect("translated MMIO younger issues");

        let state = self.core.state.lock().expect("riscv core lock");
        assert_eq!(state.outstanding_data.len(), 2);
        let mmio_request = state
            .outstanding_data
            .keys()
            .copied()
            .find(|request| *request != memory_request)
            .unwrap();
        (memory_request, mmio_request)
    }

    fn run_until_one_request_remains(&mut self) {
        while self
            .core
            .state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .len()
            == 2
        {
            self.scheduler.run_next_epoch_parallel().unwrap();
        }
        assert_eq!(
            self.core
                .state
                .lock()
                .expect("riscv core lock")
                .outstanding_data
                .len(),
            1
        );
    }
}

fn install_cached_translation(
    core: &RiscvCore,
    page_map: &TranslationPageMap,
    virtual_address: u64,
) {
    let request = TranslationRequest::new(
        TranslationRequestId::new(AgentId::new(7), 99),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
        TranslationAccessKind::Load,
    )
    .unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    state
        .data_translation
        .as_mut()
        .unwrap()
        .tlb_mut()
        .unwrap()
        .translate_in_address_space(address_space, &request, page_map)
        .unwrap();
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

#[test]
fn translation_preserves_and_binds_each_result_pair_authorization_once() {
    let core = translated_pair_core();
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );

    let spans = [
        (request(0), 0x4000, 0x9000, 11),
        (request(1), 0x5000, 0xa000, 12),
    ];
    let mut state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    for (fetch_request, virtual_address, physical_address, rd) in spans {
        let size = AccessSize::new(8).unwrap();
        assert!(state.translated_result_authorization_is_pending(
            fetch_request,
            Address::new(virtual_address),
            size,
        ));
        let translated = TranslatedDataAccess {
            request_id: MemoryRequestId::new(AgentId::new(7), fetch_request.sequence() + 20),
            fetch_request,
            access: rem6_isa_riscv::MemoryAccessKind::Load {
                rd: Register::new(rd).unwrap(),
                address: virtual_address,
                width: rem6_isa_riscv::MemoryWidth::Doubleword,
                signed: false,
            },
            virtual_address: Address::new(virtual_address),
            size,
            physical_address: Address::new(physical_address),
            request_byte_offset: 0,
        };
        assert!(state.bind_translated_result_range(&translated));
        assert!(state.bind_translated_result_range(&translated));
        assert!(!state.translated_result_authorization_is_pending(
            fetch_request,
            Address::new(virtual_address),
            size,
        ));
        assert!(
            state.bind_translated_result_target(fetch_request, O3MemoryResultWindowRoute::Memory,)
        );
        assert!(
            state.bind_translated_result_target(fetch_request, O3MemoryResultWindowRoute::Memory,)
        );
        let authorization = state
            .memory_result_window_authorizations
            .get(&fetch_request)
            .copied()
            .unwrap();
        assert!(authorization.matches_bound_target(
            O3MemoryResultWindowRoute::Memory,
            Address::new(physical_address),
            size,
        ));
    }
}

fn translated_pair_core() -> RiscvCore {
    let core = RiscvCore::with_data_translation(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(
            endpoint("cpu0.dmem"),
            MemoryRouteId::new(1),
            CacheLineLayout::new(16).unwrap(),
        ),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x4000);
    core.write_register(Register::new(3).unwrap(), 0x5000);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch(0, 0x8000, ld(11, 2)),
            completed_fetch(1, 0x8004, ld(12, 3)),
        ]);

    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(0x4000),
            Address::new(0x9000),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    let fill = TranslationRequest::new(
        TranslationRequestId::new(AgentId::new(7), 99),
        Address::new(0x4000),
        AccessSize::new(8).unwrap(),
        TranslationAccessKind::Load,
    )
    .unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    let address_space = TranslationAddressSpaceId::new(state.hart.translation_address_space());
    state
        .data_translation
        .as_mut()
        .unwrap()
        .tlb_mut()
        .unwrap()
        .translate_in_address_space(address_space, &fill, &page_map)
        .unwrap();
    drop(state);
    core
}

fn completed_fetch(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            4,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn ld(rd: u8, rs1: u8) -> u32 {
    (u32::from(rs1) << 15) | (0b011 << 12) | (u32::from(rd) << 7) | 0x03
}
