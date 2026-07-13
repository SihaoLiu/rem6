use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvStoreConditionalProgressConfig,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvSystemRunDriver, RiscvSystemRunStopReason,
    RiscvTrapEventPort, StopRequest, SystemHostController, SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
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

fn system_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    MemoryRouteId,
    MemoryRouteId,
) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(2),
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
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    (scheduler, transport, fetch_route, data_route)
}

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            fetch_route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();
    RiscvCore::with_data_and_store_conditional_progress_config(
        core,
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
        RiscvStoreConditionalProgressConfig::new(3).unwrap(),
    )
}

fn program_store(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
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

    let mut image = BootImage::new(Address::new(entry));
    for (index, instruction) in instructions.iter().enumerate() {
        image = image
            .add_segment(Address::new(entry + (index as u64 * 4)), word(*instruction))
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn memory_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    let response = store
        .lock()
        .unwrap()
        .respond(delivery.request())
        .unwrap()
        .response()
        .cloned()
        .unwrap();
    TargetOutcome::Respond(response)
}

#[test]
fn system_parallel_run_preserves_riscv_store_conditional_failure_diagnostics() {
    let (mut scheduler, transport, fetch_route, data_route) = system_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x1020_3040_5060_7080);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = program_store(
        0x8000,
        &[
            atomic_type(0x03, false, false, 6, 2, 0x3, 7),
            atomic_type(0x03, false, false, 6, 2, 0x3, 7),
            atomic_type(0x03, false, false, 6, 2, 0x3, 7),
            0x0000_0073,
        ],
    );
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(41);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);

    let run = driver
        .drive_until_host_stop_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                |_delivery, _context| {
                    panic!("failed store conditional should not issue a memory transaction")
                }
            },
            80,
            |cpu| GuestEventId::new(400 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(400), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(run.store_conditional_failure_diagnostic_count(), 1);
    assert_eq!(
        run.store_conditional_failure_diagnostic_count_for_cpu(CpuId::new(0)),
        1,
    );
    let diagnostics = run.store_conditional_failure_diagnostics();
    assert!(run.has_store_conditional_failure_diagnostics());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].cpu(), CpuId::new(0));
    assert_eq!(diagnostics[0].address(), Address::new(0x9008));
    assert_eq!(diagnostics[0].size(), AccessSize::new(8).unwrap());
    assert_eq!(diagnostics[0].failure_count(), 3);
    assert_eq!(diagnostics[0].diagnostic_threshold(), 3);
}
