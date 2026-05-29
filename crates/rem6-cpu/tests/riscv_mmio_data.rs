use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCpuError};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioError, MmioRegisterBank, MmioRoute};
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
    core.write_register(rem6_isa_riscv::Register::new(2).unwrap(), 0x1000);
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
