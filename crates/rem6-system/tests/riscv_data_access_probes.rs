use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{ParallelSchedulerContext, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryResponse, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_stats::{
    CommMonitorConfig, CommMonitorStats, MemCheckerTransaction, MemCheckerWriteClusterSnapshot,
    MemFootprintAddressRange, MemFootprintProbeConfig, MemFootprintProbeSnapshot,
    MemProbePacketAccess, MemProbePacketKind, ProbePayload, StackDistProbeConfig,
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

fn store_conditional_failed_responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    move |delivery, _context| {
        if delivery.request().operation() == MemoryOperation::StoreConditional {
            return TargetOutcome::Respond(
                MemoryResponse::store_conditional_failed(delivery.request()).unwrap(),
            );
        }
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

fn comm_monitor_config() -> CommMonitorConfig {
    CommMonitorConfig::builder(100)
        .read_addr_mask(0xfff0)
        .write_addr_mask(0xfff0)
        .build()
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

#[test]
fn data_access_stats_without_comm_monitor_keeps_snapshot_absent() {
    let stats = RiscvDataAccessStats::with_stack_distance(stack_distance_config());

    assert!(stats
        .data_access_probe_snapshot()
        .communication_monitor()
        .is_none());
}

#[test]
fn system_run_data_access_stats_drive_comm_monitor_from_real_load_requests() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(43);
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
    let config = comm_monitor_config();
    let driver = RiscvSystemRunDriver::new(trap_port).with_data_access_stats(
        RiscvDataAccessStats::with_stack_distance(stack_distance_config())
            .with_comm_monitor(config.clone()),
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
            |cpu| GuestEventId::new(130 + u64::from(cpu.get())),
        )
        .unwrap();

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    let communication = data
        .communication_monitor()
        .expect("run should carry communication monitor evidence");
    assert_eq!(communication.config(), &config);
    assert!(communication.pending().is_empty());
    assert_eq!(
        communication.stats(),
        CommMonitorStats::new(2, 0, 0, 0, 0, 0, 0, 0)
    );
    let request_delta = data.probes().events()[1].tick() - data.probes().events()[0].tick();
    assert_eq!(communication.histograms().read_burst_lengths(), &[(8, 2)]);
    assert_eq!(
        communication.histograms().read_to_read_times(),
        &[(request_delta, 1)]
    );
    assert_eq!(
        communication.histograms().request_to_request_times(),
        &[(request_delta, 1)]
    );
    assert_eq!(
        communication.histograms().read_addresses(),
        &[(0x9800, 1), (0x9840, 1)]
    );
}

#[test]
fn system_run_data_access_stats_drive_comm_monitor_from_real_store_requests() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(44);
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
    core.write_register(reg(3), 0x1111_2222_3333_4444);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store_with_data(
        &[(0x8000, s_type(0x008, 3, 2, 0x3)), (0x8004, 0x0000_0073)],
        &[(0x9808, vec![0; 8])],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        rem6_stats::StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let config = comm_monitor_config();
    let driver = RiscvSystemRunDriver::new(trap_port).with_data_access_stats(
        RiscvDataAccessStats::with_stack_distance(stack_distance_config())
            .with_comm_monitor(config.clone()),
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
            |cpu| GuestEventId::new(140 + u64::from(cpu.get())),
        )
        .unwrap();

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    let communication = data
        .communication_monitor()
        .expect("run should carry communication monitor evidence");
    assert_eq!(communication.config(), &config);
    assert!(communication.pending().is_empty());
    assert_eq!(
        communication.stats(),
        CommMonitorStats::new(0, 1, 0, 8, 0, 8, 0, 0)
    );
    assert_eq!(communication.histograms().write_burst_lengths(), &[(8, 1)]);
    assert_eq!(communication.histograms().write_addresses(), &[(0x9800, 1)]);
    assert!(communication.histograms().write_latencies().is_empty());
    assert_eq!(data.probes().events().len(), 1);
}

#[test]
fn system_run_data_access_stats_drive_mem_checker_monitor_from_real_store_then_load() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(45);
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
    core.write_register(reg(3), 0xaa);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, s_type(0x008, 3, 2, 0x0)),
            (0x8004, i_type(0x008, 2, 0x0, 5, 0x03)),
            (0x8008, 0x0000_0073),
        ],
        &[(0x9808, vec![0])],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        rem6_stats::StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let comm_config = comm_monitor_config();
    let driver = RiscvSystemRunDriver::new(trap_port).with_data_access_stats(
        RiscvDataAccessStats::with_stack_distance(stack_distance_config())
            .with_comm_monitor(comm_config.clone())
            .with_mem_checker_monitor(),
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
            |cpu| GuestEventId::new(150 + u64::from(cpu.get())),
        )
        .unwrap();

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    assert!(data.response_point().is_some());
    let checker = data
        .mem_checker_monitor()
        .expect("run should carry memory checker monitor evidence");
    let communication = data
        .communication_monitor()
        .expect("run should carry communication monitor evidence");
    assert_eq!(data.probes().events().len(), 4);
    assert_eq!(communication.config(), &comm_config);
    assert!(communication.pending().is_empty());
    assert_eq!(
        communication.stats(),
        CommMonitorStats::new(1, 1, 1, 1, 1, 1, 0, 0)
    );
    assert!(checker.pending().is_empty());
    assert_eq!(checker.checker().next_serial(), 3);
    assert_eq!(checker.checker().bytes().len(), 1);

    let byte = &checker.checker().bytes()[0];
    assert_eq!(byte.address(), 0x9808);
    assert!(byte.outstanding_reads().is_empty());
    assert_eq!(byte.read_observations().len(), 2);
    assert_eq!(byte.read_observations()[1].serial(), 2);
    assert_eq!(byte.read_observations()[1].data(), 0xaa);
    assert_eq!(
        byte.write_clusters(),
        &[MemCheckerWriteClusterSnapshot::new(
            byte.write_clusters()[0].start_tick(),
            byte.write_clusters()[0].complete_tick(),
            byte.write_clusters()[0].complete_max_tick(),
            0,
            vec![MemCheckerTransaction::write(
                1,
                byte.write_clusters()[0].start_tick(),
                byte.write_clusters()[0].complete_tick(),
                0xaa
            )]
        )]
    );
}

#[test]
fn system_run_data_access_stats_do_not_feed_atomic_writes_to_mem_checker_monitor() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(46);
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
    core.write_register(reg(2), 0x9808);
    core.write_register(reg(3), 5);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, atomic_type(0x00, false, false, 3, 2, 0x3, 5)),
            (0x8004, 0x0000_0073),
        ],
        &[(0x9808, 7_u64.to_le_bytes().to_vec())],
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
            .with_mem_checker_monitor(),
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
            |cpu| GuestEventId::new(160 + u64::from(cpu.get())),
        )
        .unwrap();

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    let checker = data
        .mem_checker_monitor()
        .expect("run should carry memory checker monitor evidence");
    assert!(checker.pending().is_empty());
    assert!(checker.checker().bytes().is_empty());
}

#[test]
fn system_run_data_access_stats_labels_memory_side_store_conditional_failures() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(47);
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
    core.write_register(reg(2), 0x9808);
    core.write_register(reg(3), 0xaa);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, atomic_type(0x02, false, false, 0, 2, 0x3, 4)),
            (0x8004, atomic_type(0x03, false, false, 3, 2, 0x3, 5)),
            (0x8008, 0x0000_0073),
        ],
        &[(0x9808, 0_u64.to_le_bytes().to_vec())],
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
            .with_mem_checker_monitor(),
    );

    let run = driver
        .drive_until_host_stop_parallel(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| store_conditional_failed_responder(Arc::clone(&store)),
            40,
            |cpu| GuestEventId::new(170 + u64::from(cpu.get())),
        )
        .unwrap();

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    let failed_response = data
        .probes()
        .events()
        .iter()
        .filter_map(|event| match event.payload() {
            ProbePayload::MemoryPacket(packet)
                if packet.kind() == MemProbePacketKind::Response
                    && packet.access() == MemProbePacketAccess::Write =>
            {
                Some(packet)
            }
            _ => None,
        })
        .next()
        .expect("store-conditional failure should emit a write response probe");
    assert_eq!(failed_response.command(), 10);
}

#[test]
fn system_run_data_access_stats_filters_local_store_conditional_failures_from_mem_checker_monitor()
{
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(48);
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
    core.write_register(reg(2), 0x9808);
    core.write_register(reg(3), 0xaa);
    let cluster = RiscvCluster::new([core]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, atomic_type(0x03, false, false, 3, 2, 0x3, 5)),
            (0x8004, 0x0000_0073),
        ],
        &[(0x9808, 0_u64.to_le_bytes().to_vec())],
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
            .with_mem_checker_monitor(),
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
            |cpu| GuestEventId::new(180 + u64::from(cpu.get())),
        )
        .unwrap();

    let data = run
        .data_access_probes()
        .expect("run should carry data access probe evidence");
    let checker = data
        .mem_checker_monitor()
        .expect("run should carry memory checker monitor evidence");
    assert!(data.probes().events().is_empty());
    assert!(checker.pending().is_empty());
    assert!(checker.checker().bytes().is_empty());
}
