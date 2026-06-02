use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCpuError,
    RiscvDataAccessEventKind, RiscvDataAccessTarget,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
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

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn core(route: MemoryRouteId, entry: u64) -> CpuCore {
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

#[test]
fn riscv_core_issues_parallel_mmio_load_and_updates_register_after_response() {
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
    core.write_register(reg(2), 0x1000);
    let store = loaded_store(0x8000, i_type(8, 2, 0x3, 5, 0x03));
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0x0f],
    )
    .unwrap();
    let mut bus = MmioBus::new();
    let route = MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap();
    bus.insert_device(
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
        route,
        Mutex::new(bank),
    )
    .unwrap();

    fetch_one_parallel(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    core.execute_next_completed_fetch().unwrap().unwrap();
    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(core.read_register(reg(5)), 0x0fed_cba9_8765_4321);
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(events[0].target(), RiscvDataAccessTarget::Mmio { route });
    assert_eq!(events[1].target(), RiscvDataAccessTarget::Mmio { route });
    assert_eq!(events[0].route(), None);
    assert_eq!(events[0].endpoint(), None);
    assert_eq!(
        events[1].data(),
        Some(&[0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0x0f][..])
    );
}

#[test]
fn riscv_core_rejects_mmio_atomic_before_scheduling_completion() {
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
    core.write_register(reg(2), 0x1000);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_store(0x8000, atomic_type(0x01, true, true, 6, 2, 0x3, 7));
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadWrite,
        vec![0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0x0f],
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();

    fetch_one_parallel(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    core.execute_next_completed_fetch().unwrap().unwrap();

    let error = core
        .issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap_err();
    assert!(matches!(
        error,
        RiscvCpuError::UnsupportedMmioAtomic {
            address,
            ..
        } if address == Address::new(0x1000)
    ));
    assert!(core.data_access_events().is_empty());
}
