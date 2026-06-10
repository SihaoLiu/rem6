use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{ParallelSchedulerContext, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::{
    MemFootprintAddressRange, MemFootprintProbeConfig, MemFootprintProbeSnapshot,
    StackDistProbeConfig,
};
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvDataAccessStats, RiscvSystemRunDriver,
    RiscvTrapEventPort, SystemHostController, SystemHostEventPort,
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

fn loaded_program_store_with_data(
    instructions: &[(u64, u32)],
    data_segments: &[(u64, Vec<u8>)],
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

    let mut image = BootImage::new(Address::new(instructions[0].0));
    for (address, instruction) in instructions {
        image = image
            .add_segment(Address::new(*address), word(*instruction))
            .unwrap();
    }
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

#[allow(clippy::too_many_arguments)]
fn riscv_data_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    data_endpoint: &str,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(endpoint(data_endpoint), data_route, layout()),
    )
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    move |delivery, _context| {
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
}

fn stack_distance_config() -> StackDistProbeConfig {
    StackDistProbeConfig::builder(16, 16).build().unwrap()
}

fn footprint_config() -> MemFootprintProbeConfig {
    MemFootprintProbeConfig::new(
        16,
        4096,
        vec![MemFootprintAddressRange::new(0x9800, 0x1000).unwrap()],
    )
    .unwrap()
}

#[test]
fn system_run_data_access_stats_drive_mem_footprint_from_real_loads() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(42);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
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
                endpoint("l1d"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_data_core(
        0,
        0,
        7,
        "cpu0.ifetch",
        fetch_route,
        "cpu0.dmem",
        data_route,
        0x8000,
    );
    core.write_register(reg(2), 0x9800);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, i_type(0x008, 2, 0x3, 5, 0x03)),
            (0x8004, i_type(0x040, 2, 0x3, 6, 0x03)),
            (0x8008, 0x0000_0073),
        ],
        &[
            (0x9808, 0x1111_2222_3333_4444_u64.to_le_bytes().to_vec()),
            (0x9840, 0x5555_6666_7777_8888_u64.to_le_bytes().to_vec()),
        ],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        rem6_stats::StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_data_access_stats(
        RiscvDataAccessStats::with_stack_distance(stack_distance_config())
            .with_mem_footprint(footprint_config()),
    );

    let run = driver
        .drive_until_host_stop_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            40,
            |cpu| GuestEventId::new(120 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(5)),
        0x1111_2222_3333_4444
    );
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(6)),
        0x5555_6666_7777_8888
    );

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    assert_eq!(
        data.memory_footprint(),
        Some(&MemFootprintProbeSnapshot::new(
            vec![0x9800, 0x9840],
            vec![0x9800, 0x9840],
            vec![0x9000],
            vec![0x9000],
        ))
    );
    assert_eq!(data.probes().events().len(), 2);
}

#[test]
fn data_access_stats_without_mem_footprint_keeps_snapshot_absent() {
    let stats = RiscvDataAccessStats::with_stack_distance(stack_distance_config());

    assert!(stats
        .data_access_probe_snapshot()
        .memory_footprint()
        .is_none());
}
