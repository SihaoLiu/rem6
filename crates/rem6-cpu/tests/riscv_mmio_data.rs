use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    RiscvCore, RiscvCpuError,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
    TranslationTlbConfig,
};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioError, MmioRegisterBank, MmioRequest, MmioRequestId, MmioRoute,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | 0x23
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
    fetch_route: rem6_transport::MemoryRouteId,
    data_route: rem6_transport::MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data_translation(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    )
}

fn single_page_map(virtual_base: u64, physical_base: u64) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    map.map(
        Address::new(virtual_base),
        Address::new(physical_base),
        1,
        TranslationPagePermissions::read_write_execute(),
    )
    .unwrap();
    map
}

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn fetch_one_parallel(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_fetch_parallel(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _| {
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
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
}

#[test]
fn riscv_core_rejects_parallel_mmio_response_below_lookahead_before_worker_dispatch() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
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
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.write_register(Register::new(2).unwrap(), 0x1000);
    let store = loaded_store(0x8000, i_type(8, 2, 0x3, 5, 0x03));

    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0; 8],
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        rem6_memory::AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap())
            .unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 1).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();

    fetch_one_parallel(&core, store, &mut scheduler, &transport);
    core.execute_next_completed_fetch().unwrap().unwrap();

    let issue_tick = scheduler.now();
    let error = core
        .issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap_err();

    assert_eq!(
        error,
        RiscvCpuError::Mmio(MmioError::Scheduler(
            SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: PartitionId::new(1),
                target: PartitionId::new(0),
                source_tick: issue_tick + 2,
                delivery_tick: issue_tick + 3,
                minimum_delivery_tick: issue_tick + 4,
            }
        ))
    );
    assert!(core.data_access_events().is_empty());
    assert!(core.has_unissued_data_access());
}

#[test]
fn riscv_core_rejects_parallel_translated_mmio_response_below_lookahead_before_worker_dispatch() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
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
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(Register::new(2).unwrap(), 0x2000);
    let store = loaded_store(0x8000, i_type(8, 2, 0x3, 5, 0x03));
    let page_map = single_page_map(0x2000, 0x1000);

    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0; 8],
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        rem6_memory::AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap())
            .unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 1).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();

    fetch_one_parallel(&core, store, &mut scheduler, &transport);
    core.execute_next_completed_fetch().unwrap().unwrap();

    let issue_tick = scheduler.now();
    let error = core
        .issue_next_translated_mmio_data_access_parallel(&mut scheduler, &bus, &page_map)
        .unwrap_err();

    assert_eq!(
        error,
        RiscvCpuError::Mmio(MmioError::Scheduler(
            SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source: PartitionId::new(1),
                target: PartitionId::new(0),
                source_tick: issue_tick + 2,
                delivery_tick: issue_tick + 3,
                minimum_delivery_tick: issue_tick + 4,
            }
        ))
    );
    assert!(core.data_access_events().is_empty());
    assert!(core.has_pending_data_access());
}

#[test]
fn riscv_core_leaves_unmapped_translated_atomic_for_memory_after_mmio_probe() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
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
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(Register::new(2).unwrap(), 0x2000);
    core.write_register(Register::new(6).unwrap(), 1);
    let store = loaded_store(0x8000, atomic_type(0x00, false, false, 6, 2, 0x3, 7));
    let page_map = single_page_map(0x2000, 0x9000);

    let bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        rem6_memory::AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap())
            .unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();

    fetch_one_parallel(&core, store, &mut scheduler, &transport);
    core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(
        core.issue_next_translated_mmio_data_access_parallel(&mut scheduler, &bus, &page_map)
            .unwrap(),
        None
    );
    assert!(core.has_pending_data_access());
}

#[test]
fn redirect_cancels_scheduled_mmio_store_before_device_delivery() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = RiscvCore::with_data(
        core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    );
    core.write_register(Register::new(2).unwrap(), 0x1000);
    core.write_register(Register::new(6).unwrap(), 0x5a);
    let store = loaded_store(0x8000, s_type(8, 6, 2, 0x3));
    let (bus, bank) = writable_mmio_bus();

    fetch_one_parallel(&core, store, &mut scheduler, &transport);
    core.execute_next_completed_fetch().unwrap().unwrap();
    let source_event = core
        .issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .expect("MMIO store schedules before redirect");
    let source_tick = scheduler
        .pending_event_snapshot(source_event)
        .expect("MMIO source event is pending")
        .tick();
    assert_eq!(source_tick, scheduler.now());
    let source_epoch = scheduler.run_next_epoch_parallel().unwrap();
    assert_eq!(source_epoch.executed_events(), 1);
    assert_eq!(read_mmio_u64(&bank), 0);
    core.redirect_pc(Address::new(0x9000));
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(read_mmio_u64(&bank), 0);
}

#[test]
fn redirect_cancels_scheduled_translated_mmio_store_before_device_delivery() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.write_register(Register::new(2).unwrap(), 0x2000);
    core.write_register(Register::new(6).unwrap(), 0x5a);
    let store = loaded_store(0x8000, s_type(8, 6, 2, 0x3));
    let page_map = single_page_map(0x2000, 0x1000);
    let (bus, bank) = writable_mmio_bus();

    fetch_one_parallel(&core, store, &mut scheduler, &transport);
    core.execute_next_completed_fetch().unwrap().unwrap();
    let source_event = core
        .issue_next_translated_mmio_data_access_parallel(&mut scheduler, &bus, &page_map)
        .unwrap()
        .expect("translated MMIO store schedules before redirect");
    let source_tick = scheduler
        .pending_event_snapshot(source_event)
        .expect("translated MMIO source event is pending")
        .tick();
    assert_eq!(source_tick, scheduler.now());
    let source_epoch = scheduler.run_next_epoch_parallel().unwrap();
    assert_eq!(source_epoch.executed_events(), 1);
    assert_eq!(read_mmio_u64(&bank), 0);
    core.redirect_pc(Address::new(0x9000));
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(read_mmio_u64(&bank), 0);
}

fn data_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    rem6_transport::MemoryRouteId,
    rem6_transport::MemoryRouteId,
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

fn writable_mmio_bus() -> (MmioBus, Arc<Mutex<MmioRegisterBank>>) {
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadWrite,
        vec![0; 8],
    )
    .unwrap();
    let bank = Arc::new(Mutex::new(bank));
    let mut bus = MmioBus::new();
    bus.insert_device(
        rem6_memory::AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap())
            .unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 3, 2).unwrap(),
        Arc::clone(&bank),
    )
    .unwrap();
    (bus, bank)
}

fn read_mmio_u64(bank: &Arc<Mutex<MmioRegisterBank>>) -> u64 {
    let response = bank
        .lock()
        .unwrap()
        .respond(
            &MmioRequest::read(
                MmioRequestId::new(99),
                Address::new(0x1008),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    u64::from_le_bytes(response.data().unwrap().try_into().unwrap())
}
