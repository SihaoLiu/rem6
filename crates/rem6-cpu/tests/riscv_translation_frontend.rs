use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    decode_sv39_pte_read_response, CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState,
    CpuTranslationFrontend, CpuTranslationOutcome, CpuTranslationRequest, RiscvCore,
    RiscvCoreDriveAction, RiscvCpuError, RiscvDataAccessEventKind, RiscvLoadReservation,
    RiscvSv39PageTableResolver, RiscvSv39PteReadRequestError, RiscvSv39PteReadResponseError,
};
use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvPmaAccessKind, RiscvPmaError, RiscvPmpAccessKind,
    RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpError, RiscvPrivilegeMode, RiscvStatusWord,
    RiscvSv39AccessContext, RiscvSv39AccessKind, RiscvSv39PageFault, RiscvSv39PageTableLevel,
    RiscvSv39Pte, RiscvTrapKind,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryResponse, MemoryResponseSnapshot, MemoryTargetId, PartitionedMemoryStore, ResponseStatus,
    TranslationAddressSpaceId, TranslationFaultKind, TranslationPageMap,
    TranslationPageMappingScope, TranslationPagePermissions, TranslationPageSize,
    TranslationQueueConfig, TranslationRequestId, TranslationTlbConfig, TranslationTlbStats,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn translation_id(sequence: u64) -> TranslationRequestId {
    TranslationRequestId::new(AgentId::new(17), sequence)
}

fn memory_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(19), sequence)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    ((imm >> 5) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn atomic_type(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn csr_type(csr: u32, rs1_or_zimm: u8, funct3: u32, rd: u8) -> u32 {
    (csr << 20) | (u32::from(rs1_or_zimm) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
}

fn sfence_vma_type(rs1: u8, rs2: u8) -> u32 {
    (0x09 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | 0x73
}

fn locked_tor_without_permissions() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor).with_locked(true)
}

fn tor_with_all_permissions() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
        .with_read(true)
        .with_write(true)
        .with_execute(true)
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn data_read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(99), sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn read_store_bytes(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    size: u64,
    sequence: u64,
) -> Vec<u8> {
    store
        .lock()
        .unwrap()
        .respond(&data_read(address, size, sequence))
        .unwrap()
        .response()
        .unwrap()
        .data()
        .unwrap()
        .to_vec()
}

fn core(route: rem6_transport::MemoryRouteId, entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn translated_data_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    translated_data_core_with_latency(fetch_route, data_route, entry, 0)
}

fn translated_data_core_with_latency(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    entry: u64,
    latency: u64,
) -> RiscvCore {
    RiscvCore::with_data_translation(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, latency).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    )
}

#[test]
fn riscv_core_reports_zero_flushed_entries_for_translation_frontend_without_tlb() {
    let core = RiscvCore::with_data_translation(
        core(MemoryRouteId::new(6), 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), MemoryRouteId::new(7), layout()),
        CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap()),
    );

    assert_eq!(core.data_translation_tlb_stats(), None);
    assert_eq!(core.flush_data_translation_tlb(), Some(0));
    assert_eq!(core.data_translation_tlb_stats(), None);
}

fn single_page_map(virtual_base: u64, physical_base: u64) -> TranslationPageMap {
    scoped_single_page_map(
        virtual_base,
        physical_base,
        TranslationPageMappingScope::NonGlobal,
    )
}

fn scoped_single_page_map(
    virtual_base: u64,
    physical_base: u64,
    scope: TranslationPageMappingScope,
) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map_with_scope(
        Address::new(virtual_base),
        Address::new(physical_base),
        1,
        TranslationPagePermissions::read_write_execute(),
        scope,
    )
    .unwrap();
    map
}

fn loaded_store_with_data(
    entry: u64,
    instruction: u32,
    data_address: u64,
    data: Vec<u8>,
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
        .unwrap()
        .add_segment(Address::new(data_address), data)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_program_store(
    entry: u64,
    instructions: &[u32],
    data_segments: &[(u64, Vec<u8>)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();

    let mut instruction_bytes = Vec::new();
    for instruction in instructions {
        instruction_bytes.extend(word(*instruction));
    }
    let mut image = BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), instruction_bytes)
        .unwrap();
    for (address, data) in data_segments {
        image = image
            .add_segment(Address::new(*address), data.clone())
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn data_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    MemoryRouteId,
    MemoryRouteId,
) {
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

    (scheduler, transport, fetch_route, data_route)
}

const SV39_PTE_V: u64 = 1 << 0;
const SV39_PTE_R: u64 = 1 << 1;
const SV39_PTE_W: u64 = 1 << 2;
const SV39_PTE_X: u64 = 1 << 3;
const SV39_PTE_A: u64 = 1 << 6;
const SV39_PTE_D: u64 = 1 << 7;

fn fetch_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    core.issue_next_fetch(scheduler, transport, trace, move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    })
    .unwrap();
    scheduler.run_until_idle_conservative();
}

fn fetch_one_parallel(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    core.issue_next_fetch_parallel(scheduler, transport, trace, move |delivery, context| {
        assert_eq!(context.partition(), PartitionId::new(1));
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    })
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
}

fn drive_one_translated_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    page_map: &TranslationPageMap,
) -> Option<RiscvCoreDriveAction> {
    let fetch_store = store.clone();
    let data_store = store;
    core.drive_next_action_with_data_translation(
        scheduler,
        transport,
        MemoryTrace::new(),
        MemoryTrace::new(),
        page_map,
        move |delivery, _context| {
            let response = fetch_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
        move |delivery, _context| {
            let response = data_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
}

#[test]
fn riscv_core_translated_driver_waits_for_data_translation_before_next_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core_with_latency(fetch_route, data_route, 0x8000, 1);
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(8, 2, 0x3, 5, 0x03), i_type(4, 0, 0x0, 6, 0x13)],
        &[(0x9008, vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe])],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        None
    );
    assert!(core.has_pending_data_access());
    assert!(!core.has_pending_fetch());

    scheduler
        .schedule_after(core.partition(), 1, |_context| {})
        .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0xfedc_ba98_7654_3210);
    let data_events = core.data_access_events();
    assert_eq!(data_events[0].physical_address(), Address::new(0x9008));

    assert!(matches!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_data_translation_fault_enters_guest_load_page_fault_trap() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    core.set_machine_exception_delegation(1 << 13);
    core.set_supervisor_trap_vector(0xa001);
    core.set_status(RiscvStatusWord::new(0).with_sie(true));
    let page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    let store = loaded_program_store(0x8000, &[i_type(8, 2, 0x3, 5, 0x03)], &[]);

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    let action = drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map)
        .expect("translation fault should be reported as a trap event");
    let RiscvCoreDriveAction::InstructionExecuted(event) = action else {
        panic!("translation fault should execute a trap entry, got {action:?}");
    };
    assert_eq!(event.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        event.execution().trap().map(|trap| trap.kind()),
        Some(RiscvTrapKind::LoadPageFault { address: 0x4008 })
    );
    assert!(!event.counts_as_retired_instruction());
    assert!(event.in_order_pipeline_cycle().is_none());
    assert_eq!(core.in_order_pipeline_snapshot().cycle(), 0);
    assert!(core.in_order_pipeline_snapshot().in_flight().is_empty());
    assert_eq!(core.pending_trap(), event.execution().trap().copied());
    assert_eq!(core.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(core.supervisor_exception_pc(), 0x8000);
    assert_eq!(core.supervisor_trap_cause(), 13);
    assert_eq!(core.supervisor_trap_value(), 0x4008);
    assert_eq!(core.pc(), Address::new(0xa000));
    assert_eq!(core.read_register(reg(5)), 0);
    assert!(!core.has_pending_data_access());
    let execution_events = core.execution_events();
    assert_eq!(execution_events.len(), 1);
    assert_eq!(
        execution_events[0]
            .execution()
            .trap()
            .map(|trap| trap.kind()),
        Some(RiscvTrapKind::LoadPageFault { address: 0x4008 })
    );
}

#[test]
fn riscv_core_data_translation_fault_enters_guest_store_page_fault_trap() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    core.write_register(reg(3), 0x1122_3344_5566_7788);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    core.set_machine_exception_delegation(1 << 15);
    core.set_supervisor_trap_vector(0xa000);
    let page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    let store = loaded_program_store(0x8000, &[s_type(8, 3, 2, 0x3, 0x23)], &[]);

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    let action = drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map)
        .expect("store translation fault should be reported as a trap event");
    let RiscvCoreDriveAction::InstructionExecuted(event) = action else {
        panic!("store translation fault should execute a trap entry, got {action:?}");
    };
    assert_eq!(
        event.execution().trap().map(|trap| trap.kind()),
        Some(RiscvTrapKind::StorePageFault { address: 0x4008 })
    );
    assert!(!event.counts_as_retired_instruction());
    assert_eq!(core.privilege_mode(), RiscvPrivilegeMode::Supervisor);
    assert_eq!(core.supervisor_exception_pc(), 0x8000);
    assert_eq!(core.supervisor_trap_cause(), 15);
    assert_eq!(core.supervisor_trap_value(), 0x4008);
    assert_eq!(core.pc(), Address::new(0xa000));
    let execution_events = core.execution_events();
    assert_eq!(execution_events.len(), 1);
    assert_eq!(
        execution_events[0]
            .execution()
            .trap()
            .map(|trap| trap.kind()),
        Some(RiscvTrapKind::StorePageFault { address: 0x4008 })
    );
}

#[test]
fn riscv_core_plain_data_issue_rejects_configured_translation_frontend() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("plain issue must not bypass translation"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataTranslationPageMapRequired { fetch }
            if fetch == execution.fetch().request_id()
    ));
}

#[test]
fn riscv_core_pmp_rejects_locked_translated_data_load_by_physical_address() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    core.write_pmp_addr(0, 0x8800 >> 2).unwrap();
    core.write_pmp_config(0, tor_with_all_permissions())
        .unwrap();
    core.write_pmp_addr(1, 0xa000 >> 2).unwrap();
    core.write_pmp_config(1, locked_tor_without_permissions())
        .unwrap();
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_translated_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| {
                panic!("PMP-denied translated data load must not issue to memory")
            },
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmpAccess {
            fetch,
            error: RiscvPmpError::AccessDenied {
                address: 0x9008,
                size: 8,
                kind: RiscvPmpAccessKind::Read,
                privilege: RiscvPrivilegeMode::Machine,
                matched_entry: Some(1),
            },
        } if fetch == execution.fetch().request_id()
    ));
    assert!(core.data_access_events().is_empty());
    assert!(core.has_pending_data_access());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_pma_rejects_misaligned_translated_data_load_by_physical_address() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4001);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x3, 5, 0x03),
        0x9001,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_translated_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| {
                panic!("PMA-denied translated data load must not issue to memory")
            },
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmaAccess {
            fetch,
            error: RiscvPmaError::MisalignedDataAccess {
                address: 0x9001,
                size: 8,
                kind: RiscvPmaAccessKind::Read,
            },
        } if fetch == execution.fetch().request_id()
    ));
    assert!(core.data_access_events().is_empty());
    assert!(core.has_pending_data_access());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_issues_translated_load_through_data_tlb() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();

    let delivered_addresses = Arc::new(Mutex::new(Vec::new()));
    let observed_addresses = delivered_addresses.clone();
    core.issue_next_translated_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        &page_map,
        move |delivery, _context| {
            observed_addresses
                .lock()
                .unwrap()
                .push(delivery.request().range().start());
            let response = store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(5)), 0x0123_4567_89ab_cdef);
    assert_eq!(
        *delivered_addresses.lock().unwrap(),
        vec![Address::new(0x9008)]
    );
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(0, 1, 0, 1, 0)
    );
    let events = core.data_access_events();
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::Load {
            rd: reg(5),
            address: 0x4008,
            width: MemoryWidth::Doubleword,
            signed: true,
        }
    );
    assert_eq!(events[0].physical_address(), Address::new(0x9008));
    assert_eq!(events[1].physical_address(), Address::new(0x9008));
}

#[test]
fn riscv_core_issues_translated_llsc_requests_through_data_tlb() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            atomic_type(0x02, true, false, 0, 2, 0x3, 5),
            atomic_type(0x03, false, true, 6, 2, 0x3, 7),
        ],
        &[(0x9008, vec![0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90])],
    );
    let delivered_operations = Arc::new(Mutex::new(Vec::new()));

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    let observed_operations = delivered_operations.clone();
    let data_store = store.clone();
    core.issue_next_translated_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        &page_map,
        move |delivery, _context| {
            observed_operations
                .lock()
                .unwrap()
                .push(delivery.request().operation());
            assert_eq!(delivery.request().range().start(), Address::new(0x9008));
            let response = data_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(5)), 0x90ab_cdef_1234_5678);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(8).unwrap()
        ))
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    let observed_operations = delivered_operations.clone();
    let data_store = store.clone();
    core.issue_next_translated_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        &page_map,
        move |delivery, _context| {
            observed_operations
                .lock()
                .unwrap()
                .push(delivery.request().operation());
            assert_eq!(delivery.request().range().start(), Address::new(0x9008));
            let response = data_store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 91),
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    assert_eq!(
        *delivered_operations.lock().unwrap(),
        vec![
            MemoryOperation::LoadLocked,
            MemoryOperation::StoreConditional
        ]
    );
    let events = core.data_access_events();
    assert_eq!(events[0].operation(), MemoryOperation::LoadLocked);
    assert_eq!(events[0].physical_address(), Address::new(0x9008));
    assert_eq!(events[2].operation(), MemoryOperation::StoreConditional);
    assert_eq!(events[2].physical_address(), Address::new(0x9008));
}

#[test]
fn riscv_core_issues_parallel_translated_load_through_data_tlb() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one_parallel(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();

    core.issue_next_translated_data_access_parallel(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        &page_map,
        move |delivery, context| {
            assert_eq!(context.partition(), PartitionId::new(1));
            assert_eq!(delivery.request().range().start(), Address::new(0x9008));
            let response = store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(core.read_register(reg(5)), 0x0123_4567_89ab_cdef);
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(events[0].physical_address(), Address::new(0x9008));
    assert_eq!(events[1].physical_address(), Address::new(0x9008));
}

#[test]
fn riscv_core_sfence_vma_all_scope_flushes_data_translation_tlb() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(8, 2, 0x3, 5, 0x03),
            sfence_vma_type(0, 0),
            i_type(8, 2, 0x3, 6, 0x03),
        ],
        &[(0x9008, vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01])],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0x0123_4567_89ab_cdef);
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(0, 1, 0, 1, 0)
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(6)), 0x0123_4567_89ab_cdef);
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(0, 2, 0, 2, 0)
    );
}

#[test]
fn riscv_core_sfence_vma_address_space_page_preserves_global_tlb_entry() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    core.write_register(reg(3), 7);
    core.set_data_translation_address_space(TranslationAddressSpaceId::new(7));
    let page_map = scoped_single_page_map(0x4000, 0x9000, TranslationPageMappingScope::Global);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(8, 2, 0x3, 5, 0x03),
            sfence_vma_type(2, 3),
            i_type(8, 2, 0x3, 6, 0x03),
        ],
        &[(0x9008, vec![0xba, 0xdc, 0xfe, 0x10, 0x32, 0x54, 0x76, 0x98])],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0x9876_5432_10fe_dcba);
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(0, 1, 0, 1, 0)
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(6)), 0x9876_5432_10fe_dcba);
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(1, 1, 0, 1, 0)
    );
}

#[test]
fn riscv_core_data_translation_uses_current_address_space() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    core.set_data_translation_address_space(TranslationAddressSpaceId::new(11));
    let map_one = single_page_map(0x4000, 0x9000);
    let map_two = single_page_map(0x4000, 0xa000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(8, 2, 0x3, 5, 0x03), i_type(8, 2, 0x3, 6, 0x03)],
        &[
            (0x9008, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]),
            (0xa008, vec![0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x10]),
        ],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &map_one),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &map_one),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &map_one),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0x8877_6655_4433_2211);

    core.set_data_translation_address_space(TranslationAddressSpaceId::new(12));
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &map_two),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &map_two),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &map_two),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(6)), 0x10ff_eedd_ccbb_aa99);
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(0, 2, 0, 2, 0)
    );
}

#[test]
fn riscv_core_data_translation_request_uses_hart_sv39_context() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core_with_latency(fetch_route, data_route, 0x8000, 20);
    let context = RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor)
        .with_mxr(true)
        .with_sum(true);
    core.set_sv39_access_context(context);
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(8, 2, 0x3, 5, 0x03)],
        &[(0x9008, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88])],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map),
        None
    );

    let ready = core.ready_data_translation_requests(u64::MAX);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].sv39_access_context(), context);
}

#[test]
fn riscv_core_data_translation_request_uses_mprv_data_context() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core_with_latency(fetch_route, data_route, 0x8000, 20);
    core.set_privilege_mode(RiscvPrivilegeMode::Machine);
    core.set_status(
        RiscvStatusWord::new(0)
            .with_mprv(true)
            .with_mpp(RiscvPrivilegeMode::Supervisor)
            .with_mxr(true)
            .with_sum(true),
    );
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(8, 2, 0x3, 5, 0x03)],
        &[(0x9008, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88])],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert_eq!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &page_map),
        None
    );

    let ready = core.ready_data_translation_requests(u64::MAX);
    assert_eq!(ready.len(), 1);
    assert_eq!(
        ready[0].sv39_access_context(),
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor)
            .with_mxr(true)
            .with_sum(true)
    );
}

#[test]
fn riscv_core_satp_asid_selects_data_translation_address_space() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x4000);
    core.write_register(reg(3), (8_u64 << 60) | (42_u64 << 44));
    let global_map = single_page_map(0x4000, 0xa000);
    let satp_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(8, 2, 0x3, 5, 0x03),
            csr_type(0x180, 3, 0x1, 0),
            i_type(8, 2, 0x3, 6, 0x03),
        ],
        &[
            (0x9008, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]),
            (0xa008, vec![0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x10]),
        ],
    );

    assert!(matches!(
        drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &global_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &global_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            &global_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0x10ff_eedd_ccbb_aa99);

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &satp_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &satp_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &satp_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &satp_map),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(&core, store, &mut scheduler, &transport, &satp_map),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(6)), 0x8877_6655_4433_2211);
    assert_eq!(
        core.data_translation_tlb_stats().unwrap(),
        TranslationTlbStats::new(0, 2, 0, 2, 0)
    );
}

#[test]
fn riscv_sv39_page_table_resolver_maps_load_and_tracks_walk_addresses() {
    let resolver = RiscvSv39PageTableResolver::new(0x100);
    let virtual_address = (0x012_u64 << 30) | (0x034_u64 << 21) | (0x056_u64 << 12) | 0x789;
    let level1_ppn = 0x200;
    let level0_ppn = 0x300;
    let leaf_ppn = 0x45678;
    let level2_pte_address = Address::new((0x100_u64 << 12) + (0x012 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x034 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x056 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(81),
        memory_id(91),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();

    let resolved = resolver.resolve(request, |pte_address| {
        Ok(match pte_address {
            address if address == level2_pte_address => {
                RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V)
            }
            address if address == level1_pte_address => {
                RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V)
            }
            address if address == level0_pte_address => RiscvSv39Pte::new(
                (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
            ),
            _ => RiscvSv39Pte::new(0),
        })
    });

    let CpuTranslationOutcome::Mapped(mapped) = resolved.outcome() else {
        panic!("Sv39 translation should map");
    };
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | 0x789)
    );
    assert_eq!(mapped.virtual_address(), Address::new(virtual_address));
    assert_eq!(mapped.size(), AccessSize::new(8).unwrap());
    assert_eq!(
        resolved.pte_addresses(),
        &[level2_pte_address, level1_pte_address, level0_pte_address]
    );
    assert_eq!(resolved.leaf_level(), Some(RiscvSv39PageTableLevel::Level0));
    assert_eq!(resolved.page_fault(), None);
}

#[test]
fn riscv_sv39_translation_result_builds_pte_read_requests() {
    let resolver = RiscvSv39PageTableResolver::new(0x130);
    let virtual_address = (0x015_u64 << 30) | (0x037_u64 << 21) | (0x059_u64 << 12) | 0xabc;
    let level1_ppn = 0x230;
    let level0_ppn = 0x330;
    let leaf_ppn = 0x75678;
    let level2_pte_address = Address::new((0x130_u64 << 12) + (0x015 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x037 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x059 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(84),
        memory_id(94),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();

    let resolved = resolver.resolve(request, |pte_address| {
        Ok(match pte_address {
            address if address == level2_pte_address => {
                RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V)
            }
            address if address == level1_pte_address => {
                RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V)
            }
            address if address == level0_pte_address => RiscvSv39Pte::new(
                (leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_W | SV39_PTE_A | SV39_PTE_D,
            ),
            _ => RiscvSv39Pte::new(0),
        })
    });
    let first_id = MemoryRequestId::new(AgentId::new(29), 700);
    let line_layout = layout();

    let pte_requests = resolved
        .pte_read_requests(first_id, line_layout)
        .expect("PTE read request planning should succeed");

    assert_eq!(pte_requests.len(), 3);
    for (index, (request, expected_address)) in pte_requests
        .iter()
        .zip([level2_pte_address, level1_pte_address, level0_pte_address])
        .enumerate()
    {
        assert_eq!(
            request.id(),
            MemoryRequestId::new(first_id.agent(), first_id.sequence() + index as u64)
        );
        assert_eq!(request.operation(), MemoryOperation::ReadShared);
        assert_eq!(request.range().start(), expected_address);
        assert_eq!(request.size(), AccessSize::new(8).unwrap());
        assert_eq!(request.line_layout(), line_layout);
        assert!(request.data().is_none());
    }
    assert_eq!(
        resolved
            .pte_read_requests(
                MemoryRequestId::new(first_id.agent(), u64::MAX - 1),
                line_layout
            )
            .unwrap_err(),
        RiscvSv39PteReadRequestError::RequestSequenceOverflow {
            first: MemoryRequestId::new(first_id.agent(), u64::MAX - 1),
            index: 2,
        }
    );
}

#[test]
fn riscv_sv39_pte_read_response_decoder_returns_typed_pte() {
    let request = MemoryRequest::read_shared(
        memory_id(95),
        Address::new(0x130_02a8),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap();
    let raw_pte = (0x8_7654_u64 << 10) | SV39_PTE_V | SV39_PTE_R | SV39_PTE_A;
    let response = MemoryResponse::completed(&request, Some(raw_pte.to_le_bytes().to_vec()))
        .expect("PTE response data should match request width");

    assert_eq!(
        decode_sv39_pte_read_response(&request, &response).unwrap(),
        RiscvSv39Pte::new(raw_pte)
    );
}

#[test]
fn riscv_sv39_pte_read_response_decoder_rejects_invalid_responses() {
    let request = MemoryRequest::read_shared(
        memory_id(96),
        Address::new(0x130_02b0),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap();
    let raw_pte = (0x8_7655_u64 << 10) | SV39_PTE_V | SV39_PTE_X | SV39_PTE_A;
    let mismatched = MemoryResponseSnapshot::new(
        memory_id(97),
        ResponseStatus::Completed,
        Some(raw_pte.to_le_bytes().to_vec()),
    )
    .and_then(|snapshot| MemoryResponse::from_snapshot(&snapshot))
    .unwrap();
    assert_eq!(
        decode_sv39_pte_read_response(&request, &mismatched).unwrap_err(),
        RiscvSv39PteReadResponseError::UnexpectedRequest {
            expected: request.id(),
            actual: mismatched.request_id(),
        }
    );

    let retry = MemoryResponse::retry(&request);
    assert_eq!(
        decode_sv39_pte_read_response(&request, &retry).unwrap_err(),
        RiscvSv39PteReadResponseError::Retry {
            request: request.id(),
        }
    );

    let missing = MemoryResponseSnapshot::new(request.id(), ResponseStatus::Completed, None)
        .and_then(|snapshot| MemoryResponse::from_snapshot(&snapshot))
        .unwrap();
    assert_eq!(
        decode_sv39_pte_read_response(&request, &missing).unwrap_err(),
        RiscvSv39PteReadResponseError::MissingData {
            request: request.id(),
        }
    );

    let short = MemoryResponseSnapshot::new(
        request.id(),
        ResponseStatus::Completed,
        Some(vec![0xaa, 0xbb, 0xcc, 0xdd]),
    )
    .and_then(|snapshot| MemoryResponse::from_snapshot(&snapshot))
    .unwrap();
    assert_eq!(
        decode_sv39_pte_read_response(&request, &short).unwrap_err(),
        RiscvSv39PteReadResponseError::InvalidDataLength {
            request: request.id(),
            actual: 4,
        }
    );
}

#[test]
fn riscv_sv39_page_table_resolver_reports_permission_fault_and_tracks_walk_addresses() {
    let resolver = RiscvSv39PageTableResolver::new(0x110);
    let virtual_address = (0x013_u64 << 30) | (0x035_u64 << 21) | (0x057_u64 << 12) | 0x89a;
    let level1_ppn = 0x210;
    let level0_ppn = 0x310;
    let leaf_ppn = 0x55678;
    let level2_pte_address = Address::new((0x110_u64 << 12) + (0x013 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x035 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x057 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(82),
        memory_id(92),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();

    let resolved = resolver.resolve(request, |pte_address| {
        Ok(match pte_address {
            address if address == level2_pte_address => {
                RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V)
            }
            address if address == level1_pte_address => {
                RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V)
            }
            address if address == level0_pte_address => {
                RiscvSv39Pte::new((leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_X | SV39_PTE_A)
            }
            _ => RiscvSv39Pte::new(0),
        })
    });

    let CpuTranslationOutcome::Fault(fault) = resolved.outcome() else {
        panic!("Sv39 translation should fault");
    };
    assert_eq!(
        fault.fault().virtual_address(),
        Address::new(virtual_address)
    );
    assert_eq!(fault.fault().kind(), TranslationFaultKind::PermissionFault);
    assert_eq!(
        resolved.pte_addresses(),
        &[level2_pte_address, level1_pte_address, level0_pte_address]
    );
    assert_eq!(resolved.leaf_level(), None);
    assert_eq!(
        resolved.page_fault(),
        Some(&RiscvSv39PageFault::PermissionDenied {
            access: RiscvSv39AccessKind::Load
        })
    );
}

#[test]
fn riscv_sv39_page_table_resolver_uses_request_access_context() {
    let resolver = RiscvSv39PageTableResolver::new(0x115);
    let virtual_address = (0x013_u64 << 30) | (0x035_u64 << 21) | (0x057_u64 << 12) | 0x89a;
    let level1_ppn = 0x215;
    let level0_ppn = 0x315;
    let leaf_ppn = 0x85678;
    let level2_pte_address = Address::new((0x115_u64 << 12) + (0x013 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x035 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x057 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(182),
        memory_id(192),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap()
    .with_sv39_access_context(
        RiscvSv39AccessContext::new(RiscvPrivilegeMode::Supervisor).with_mxr(true),
    );

    let resolved = resolver.resolve(request, |pte_address| {
        Ok(match pte_address {
            address if address == level2_pte_address => {
                RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V)
            }
            address if address == level1_pte_address => {
                RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V)
            }
            address if address == level0_pte_address => {
                RiscvSv39Pte::new((leaf_ppn << 10) | SV39_PTE_V | SV39_PTE_X | SV39_PTE_A)
            }
            _ => RiscvSv39Pte::new(0),
        })
    });

    let CpuTranslationOutcome::Mapped(mapped) = resolved.outcome() else {
        panic!("Sv39 translation should map when request MXR allows load from execute leaf");
    };
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | 0x89a)
    );
    assert_eq!(resolved.leaf_level(), Some(RiscvSv39PageTableLevel::Level0));
    assert_eq!(resolved.page_fault(), None);
}

#[test]
fn riscv_sv39_page_table_resolver_completes_ready_frontend_translations() {
    let resolver = RiscvSv39PageTableResolver::new(0x120);
    let mut frontend = CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 2).unwrap());
    let virtual_address = (0x014_u64 << 30) | (0x036_u64 << 21) | (0x058_u64 << 12) | 0x9ab;
    let level1_ppn = 0x220;
    let level0_ppn = 0x320;
    let leaf_ppn = 0x65678;
    let level2_pte_address = Address::new((0x120_u64 << 12) + (0x014 * 8));
    let level1_pte_address = Address::new((level1_ppn << 12) + (0x036 * 8));
    let level0_pte_address = Address::new((level0_ppn << 12) + (0x058 * 8));
    let request = CpuTranslationRequest::load(
        translation_id(83),
        memory_id(93),
        MemoryRouteId::new(6),
        endpoint("cpu0.dmem"),
        Address::new(virtual_address),
        AccessSize::new(8).unwrap(),
    )
    .unwrap();

    frontend.enqueue(40, request).unwrap();
    assert!(resolver
        .complete_ready(&mut frontend, 41, |_pte_address| {
            panic!("translation should not be ready before its latency")
        })
        .unwrap()
        .is_empty());
    assert_eq!(frontend.pending_count(), 1);

    let resolved = resolver
        .complete_ready(&mut frontend, 42, |pte_address| {
            Ok(match pte_address {
                address if address == level2_pte_address => {
                    RiscvSv39Pte::new((level1_ppn << 10) | SV39_PTE_V)
                }
                address if address == level1_pte_address => {
                    RiscvSv39Pte::new((level0_ppn << 10) | SV39_PTE_V)
                }
                address if address == level0_pte_address => RiscvSv39Pte::new(
                    (leaf_ppn << 10)
                        | SV39_PTE_V
                        | SV39_PTE_R
                        | SV39_PTE_W
                        | SV39_PTE_A
                        | SV39_PTE_D,
                ),
                _ => RiscvSv39Pte::new(0),
            })
        })
        .unwrap();

    assert_eq!(resolved.len(), 1);
    let CpuTranslationOutcome::Mapped(mapped) = resolved[0].outcome() else {
        panic!("ready Sv39 translation should map");
    };
    assert_eq!(
        mapped.physical_address(),
        Address::new((leaf_ppn << 12) | 0x9ab)
    );
    assert_eq!(mapped.translation_id(), translation_id(83));
    assert_eq!(
        resolved[0].pte_addresses(),
        &[level2_pte_address, level1_pte_address, level0_pte_address]
    );
    assert_eq!(
        resolved[0].leaf_level(),
        Some(RiscvSv39PageTableLevel::Level0)
    );
    assert!(frontend.is_empty());
}

#[test]
fn riscv_core_issues_ready_translated_data_after_translation_latency() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core_with_latency(fetch_route, data_route, 0x8000, 1);
    core.write_register(reg(2), 0x4000);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    assert!(core
        .issue_next_translated_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            &page_map,
            |_delivery, _context| panic!("translation miss should not issue data memory"),
        )
        .unwrap()
        .is_none());

    scheduler
        .schedule_after(core.partition(), 1, |_context| {})
        .unwrap();
    scheduler.run_until_idle_conservative();

    core.issue_next_translated_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        &page_map,
        move |delivery, _context| {
            assert_eq!(delivery.request().range().start(), Address::new(0x9008));
            let response = store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(5)), 0xfedc_ba98_7654_3210);
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
}
