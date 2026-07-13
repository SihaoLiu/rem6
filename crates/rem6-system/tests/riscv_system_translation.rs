use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTranslationFrontend,
    RiscvCluster, RiscvClusterTopologyConfig, RiscvCore, RiscvCoreDriveAction,
    RiscvCoreTopologyConfig, RiscvCoreTopologyDataTranslationConfig,
};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode};
use rem6_kernel::{ClockDomain, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore, TranslationAddressSpaceId, TranslationPageMap,
    TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig, TranslationTlbConfig,
    TranslationTlbStats,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvSystemRunDriver, RiscvSystemRunStopReason,
    RiscvTopologySystem, RiscvTrapEventPort, StopRequest, SystemHostController,
    SystemHostEventPort,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

const SBI_RFENCE_EXTENSION: u64 = 0x5246_4e43;
const SBI_RFENCE_REMOTE_SFENCE_VMA: u64 = 1;
const SBI_RFENCE_REMOTE_HFENCE_GVMA: u64 = 4;

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn topology_component(name: &str) -> ComponentId {
    ComponentId::new(name).unwrap()
}

fn topology_kind(name: &str) -> ComponentKind {
    ComponentKind::new(name).unwrap()
}

fn topology_port(name: &str) -> PortName {
    PortName::new(name).unwrap()
}

fn topology_endpoint(component: &str, port: &str) -> Endpoint {
    Endpoint::new(topology_component(component), topology_port(port))
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
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

fn lui(rd: u8, imm: u32) -> u32 {
    (imm << 12) | (u32::from(rd) << 7) | 0x37
}

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

struct CoreSpec<'a> {
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: u64,
    fetch_endpoint: &'a str,
    fetch_route: MemoryRouteId,
    data_endpoint: &'a str,
    data_route: MemoryRouteId,
}

fn translated_riscv_core(spec: CoreSpec<'_>) -> RiscvCore {
    translated_riscv_core_with_latency(spec, 0)
}

fn translated_riscv_core_with_latency(spec: CoreSpec<'_>, latency: u64) -> RiscvCore {
    let core = CpuCore::new(
        CpuResetState::new(
            CpuId::new(spec.cpu),
            PartitionId::new(spec.partition),
            AgentId::new(spec.agent),
            Address::new(spec.entry),
        ),
        CpuFetchConfig::new(
            endpoint(spec.fetch_endpoint),
            spec.fetch_route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap();

    RiscvCore::with_data_translation(
        core,
        CpuDataConfig::new(endpoint(spec.data_endpoint), spec.data_route, layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, latency).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    )
}

fn store_with_programs_and_data(
    programs: &[(u64, u32)],
    data_segments: &[(u64, Vec<u8>)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();

    let mut image = BootImage::new(Address::new(programs[0].0));
    for (entry, instruction) in programs {
        image = image
            .add_segment(Address::new(*entry), word(*instruction))
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

fn two_page_map(
    first_virtual_base: u64,
    first_physical_base: u64,
    second_virtual_base: u64,
    second_physical_base: u64,
) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    for (virtual_base, physical_base) in [
        (first_virtual_base, first_physical_base),
        (second_virtual_base, second_physical_base),
    ] {
        map.map(
            Address::new(virtual_base),
            Address::new(physical_base),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    }
    map
}

fn three_page_map(
    first_virtual_base: u64,
    first_physical_base: u64,
    second_virtual_base: u64,
    second_physical_base: u64,
    third_virtual_base: u64,
    third_physical_base: u64,
) -> TranslationPageMap {
    let mut map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    for (virtual_base, physical_base) in [
        (first_virtual_base, first_physical_base),
        (second_virtual_base, second_physical_base),
        (third_virtual_base, third_physical_base),
    ] {
        map.map(
            Address::new(virtual_base),
            Address::new(physical_base),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    }
    map
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

fn drive_one_translated_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    page_map: &TranslationPageMap,
) -> Option<RiscvCoreDriveAction> {
    for _ in 0..8 {
        let fetch_store = Arc::clone(&store);
        let data_store = Arc::clone(&store);
        let action = core
            .drive_next_action_with_data_translation(
                scheduler,
                transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                page_map,
                move |delivery, _context| memory_response(&fetch_store, &delivery),
                move |delivery, _context| memory_response(&data_store, &delivery),
            )
            .unwrap();
        if matches!(
            action,
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
        ) {
            scheduler.run_until_idle_conservative();
            continue;
        }
        return action;
    }
    panic!(
        "expected a non-pipeline translated action at pc {:?} with pipeline {:?}",
        core.pc(),
        core.in_order_pipeline_snapshot().in_flight()
    );
}

fn topology() -> Topology {
    TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                topology_component("cpu0"),
                topology_kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(topology_port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(topology_port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                topology_component("cpu1"),
                topology_kind("cpu"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(topology_port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(topology_port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                topology_component("mem0"),
                topology_kind("memory"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(topology_port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            topology_endpoint("cpu0", "ifetch"),
            topology_endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            topology_endpoint("cpu0", "dmem"),
            topology_endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            topology_endpoint("cpu1", "ifetch"),
            topology_endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            topology_endpoint("cpu1", "dmem"),
            topology_endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn translated_core_config(
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: u64,
) -> RiscvCoreTopologyConfig {
    let cpu_name = format!("cpu{cpu}");
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(entry),
        ),
        topology_endpoint(&cpu_name, "ifetch"),
        topology_endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data_translation(
        topology_endpoint(&cpu_name, "dmem"),
        topology_endpoint("mem0", "requests"),
        layout(),
        RiscvCoreTopologyDataTranslationConfig::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    )
}

#[test]
fn riscv_system_parallel_driver_supplies_page_map_to_translated_data_path() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(51);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_fetch = transport
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
    let cpu0_data = transport
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
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    let cluster = RiscvCluster::new([
        translated_riscv_core_with_latency(
            CoreSpec {
                cpu: 0,
                partition: 0,
                agent: 7,
                entry: 0x8000,
                fetch_endpoint: "cpu0.ifetch",
                fetch_route: cpu0_fetch,
                data_endpoint: "cpu0.dmem",
                data_route: cpu0_data,
            },
            2,
        ),
        translated_riscv_core_with_latency(
            CoreSpec {
                cpu: 1,
                partition: 1,
                agent: 8,
                entry: 0x8100,
                fetch_endpoint: "cpu1.ifetch",
                fetch_route: cpu1_fetch,
                data_endpoint: "cpu1.dmem",
                data_route: cpu1_data,
            },
            2,
        ),
    ])
    .unwrap();
    cluster
        .core(CpuId::new(0))
        .unwrap()
        .write_register(reg(2), 0x4000);
    cluster
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x4010);

    let page_map = single_page_map(0x4000, 0x9000);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8004, 0x0000_0073),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8104, 0x0010_0073),
        ],
        &[
            (0x9008, vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]),
            (0x9018, vec![0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f]),
        ],
    );
    let data_deliveries = Arc::new(Mutex::new(Vec::new()));
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
        .drive_until_host_stop_or_tick_limit_parallel_with_data_translation(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                let data_deliveries = Arc::clone(&data_deliveries);
                move |delivery, _context| {
                    data_deliveries
                        .lock()
                        .unwrap()
                        .push((delivery.request().id(), delivery.request().range().start()));
                    memory_response(&store, &delivery)
                }
            },
            200,
            |cpu| GuestEventId::new(140 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(140), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert!(run.turns().iter().any(|turn| {
        !turn.core_events().is_empty()
            && turn.core_events().iter().all(|event| {
                matches!(
                    event.action(),
                    RiscvCoreDriveAction::DataAccessIssued { .. }
                )
            })
    }));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(5)),
        0x1122_3344_5566_7788
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(5)),
        0x0f1e_2d3c_4b5a_6978
    );
    let mut data_deliveries = data_deliveries.lock().unwrap().clone();
    data_deliveries.sort();
    assert_eq!(
        data_deliveries,
        vec![
            (
                MemoryRequestId::new(AgentId::new(7), 1),
                Address::new(0x9008),
            ),
            (
                MemoryRequestId::new(AgentId::new(8), 1),
                Address::new(0x9018),
            ),
        ]
    );
}

#[test]
fn riscv_topology_config_builds_translated_parallel_data_cores() {
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology(),
        RiscvClusterTopologyConfig::new([
            translated_core_config(0, 0, 7, 0x8000),
            translated_core_config(1, 1, 8, 0x8100),
        ]),
        2,
    )
    .unwrap();
    assert_eq!(system.transport().route_count(), 4);
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(reg(2), 0x4000);
    system
        .cluster()
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x4010);

    let page_map = single_page_map(0x4000, 0x9000);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8004, 0x0000_0073),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8104, 0x0010_0073),
        ],
        &[
            (0x9008, vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]),
            (0x9018, vec![0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f]),
        ],
    );
    let data_deliveries = Arc::new(Mutex::new(Vec::new()));
    let source = GuestSourceId::new(52);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(PartitionId::new(3), 2, Arc::clone(&controller))
            .unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let (cluster, mut scheduler, transport) = system.execution_parts_mut();

    let run = driver
        .drive_until_host_stop_parallel_with_data_translation(
            cluster,
            &mut scheduler,
            transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context: &mut rem6_kernel::ParallelSchedulerContext<'_>| {
                    memory_response(&store, &delivery)
                }
            },
            |_cpu| {
                let store = Arc::clone(&store);
                let data_deliveries = Arc::clone(&data_deliveries);
                move |delivery, _context: &mut rem6_kernel::ParallelSchedulerContext<'_>| {
                    data_deliveries
                        .lock()
                        .unwrap()
                        .push((delivery.request().id(), delivery.request().range().start()));
                    memory_response(&store, &delivery)
                }
            },
            160,
            |cpu| GuestEventId::new(150 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(150), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(reg(5)),
        0x1122_3344_5566_7788
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(1))
            .unwrap()
            .read_register(reg(5)),
        0x0f1e_2d3c_4b5a_6978
    );
    let mut data_deliveries = data_deliveries.lock().unwrap().clone();
    data_deliveries.sort();
    assert_eq!(
        data_deliveries,
        vec![
            (
                MemoryRequestId::new(AgentId::new(7), 1),
                Address::new(0x9008),
            ),
            (
                MemoryRequestId::new(AgentId::new(8), 1),
                Address::new(0x9018),
            ),
        ]
    );
}

fn assert_remote_tlb_fence_flushes_translated_data_tlb(
    function: u64,
    source_id: u32,
    event_base: u64,
) {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(source_id);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_fetch = transport
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
    let cpu0_data = transport
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
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = translated_riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route: cpu0_fetch,
        data_endpoint: "cpu0.dmem",
        data_route: cpu0_data,
    });
    let core1 = translated_riscv_core(CoreSpec {
        cpu: 1,
        partition: 1,
        agent: 8,
        entry: 0x8100,
        fetch_endpoint: "cpu1.ifetch",
        fetch_route: cpu1_fetch,
        data_endpoint: "cpu1.dmem",
        data_route: cpu1_data,
    });
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.write_register(reg(2), 0x4010);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let page_map = single_page_map(0x4000, 0x9000);
    let (rfence_hi, rfence_lo) = lui_addi_parts(SBI_RFENCE_EXTENSION);
    let store = store_with_programs_and_data(
        &[
            (0x8000, lui(17, rfence_hi)),
            (0x8004, i_type(rfence_lo, 17, 0x0, 17, 0x13)),
            (0x8008, i_type(function as i32, 0, 0x0, 16, 0x13)),
            (0x800c, i_type(2, 0, 0x0, 10, 0x13)),
            (0x8010, i_type(0, 0, 0x0, 11, 0x13)),
            (0x8014, i_type(0, 0, 0x0, 12, 0x13)),
            (0x8018, i_type(0, 0, 0x0, 13, 0x13)),
            (0x801c, 0x0000_0073),
            (0x8020, i_type(0, 10, 0x0, 6, 0x13)),
            (0x8024, i_type(0, 11, 0x0, 7, 0x13)),
            (0x8028, 0x0010_0073),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8104, 0x0000_006f),
        ],
        &[(0x9018, vec![0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10])],
    );
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core1.read_register(reg(5)), 0x1032_5476_98ba_dcfe);
    assert_eq!(
        core1.data_translation_tlb_stats(),
        Some(TranslationTlbStats::new(0, 1, 0, 1, 0))
    );
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_sbi_firmware();

    let run = driver
        .drive_until_host_stop_parallel_with_data_translation(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            160,
            |cpu| GuestEventId::new(event_base + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(event_base),
        source,
        1,
    );
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core0.read_register(reg(7)), 0);
    assert_eq!(core0.data_translation_tlb_entry_count(), Some(0));
    assert_eq!(
        core1.data_translation_tlb_stats(),
        Some(TranslationTlbStats::new(0, 1, 0, 1, 0))
    );
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(0));
}

#[test]
fn riscv_sbi_remote_sfence_vma_flushes_translated_data_tlb() {
    assert_remote_tlb_fence_flushes_translated_data_tlb(SBI_RFENCE_REMOTE_SFENCE_VMA, 54, 170);
}

#[test]
fn riscv_sbi_remote_hfence_gvma_flushes_translated_data_tlb() {
    assert_remote_tlb_fence_flushes_translated_data_tlb(SBI_RFENCE_REMOTE_HFENCE_GVMA, 57, 200);
}

#[test]
fn riscv_sbi_remote_sfence_vma_asid_preserves_other_address_spaces() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(55);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_fetch = transport
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
    let cpu0_data = transport
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
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = translated_riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route: cpu0_fetch,
        data_endpoint: "cpu0.dmem",
        data_route: cpu0_data,
    });
    let core1 = translated_riscv_core(CoreSpec {
        cpu: 1,
        partition: 1,
        agent: 8,
        entry: 0x8100,
        fetch_endpoint: "cpu1.ifetch",
        fetch_route: cpu1_fetch,
        data_endpoint: "cpu1.dmem",
        data_route: cpu1_data,
    });
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.write_register(reg(2), 0x4010);
    core1.set_data_translation_address_space(TranslationAddressSpaceId::new(11));
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let page_map = single_page_map(0x4000, 0x9000);
    let (rfence_hi, rfence_lo) = lui_addi_parts(SBI_RFENCE_EXTENSION);
    let store = store_with_programs_and_data(
        &[
            (0x8000, lui(17, rfence_hi)),
            (0x8004, i_type(rfence_lo, 17, 0x0, 17, 0x13)),
            (0x8008, i_type(2, 0, 0x0, 16, 0x13)),
            (0x800c, i_type(2, 0, 0x0, 10, 0x13)),
            (0x8010, i_type(0, 0, 0x0, 11, 0x13)),
            (0x8014, i_type(0, 0, 0x0, 12, 0x13)),
            (0x8018, i_type(0, 0, 0x0, 13, 0x13)),
            (0x801c, i_type(11, 0, 0x0, 14, 0x13)),
            (0x8020, 0x0000_0073),
            (0x8024, i_type(0, 10, 0x0, 6, 0x13)),
            (0x8028, i_type(0, 11, 0x0, 7, 0x13)),
            (0x802c, 0x0010_0073),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8104, i_type(8, 2, 0x3, 6, 0x03)),
            (0x8108, 0x0000_006f),
        ],
        &[(0x9018, vec![0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10])],
    );
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core1.read_register(reg(5)), 0x1032_5476_98ba_dcfe);

    core1.set_data_translation_address_space(TranslationAddressSpaceId::new(12));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core1.read_register(reg(6)), 0x1032_5476_98ba_dcfe);
    assert_eq!(
        core1.data_translation_tlb_stats(),
        Some(TranslationTlbStats::new(0, 2, 0, 2, 0))
    );
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(2));

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_sbi_firmware();

    let run = driver
        .drive_until_host_stop_parallel_with_data_translation(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            160,
            |cpu| GuestEventId::new(180 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(180), source, 1);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core0.read_register(reg(7)), 0);
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::new(11),
            Address::new(0x4000)
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::new(12),
            Address::new(0x4000)
        ),
        Some(true)
    );
}

fn assert_remote_tlb_fence_range_flushes_each_overlapping_page(
    function: u64,
    source_id: u32,
    event_base: u64,
    range_start_page: u32,
    range_size_pages: u32,
) {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(source_id);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_fetch = transport
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
    let cpu0_data = transport
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
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = translated_riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route: cpu0_fetch,
        data_endpoint: "cpu0.dmem",
        data_route: cpu0_data,
    });
    let core1 = translated_riscv_core(CoreSpec {
        cpu: 1,
        partition: 1,
        agent: 8,
        entry: 0x8100,
        fetch_endpoint: "cpu1.ifetch",
        fetch_route: cpu1_fetch,
        data_endpoint: "cpu1.dmem",
        data_route: cpu1_data,
    });
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.write_register(reg(2), 0x4010);
    core1.write_register(reg(3), 0x5010);
    core1.write_register(reg(4), 0x7010);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let page_map = three_page_map(0x4000, 0x9000, 0x5000, 0xa000, 0x7000, 0xb000);
    let (rfence_hi, rfence_lo) = lui_addi_parts(SBI_RFENCE_EXTENSION);
    let store = store_with_programs_and_data(
        &[
            (0x8000, lui(17, rfence_hi)),
            (0x8004, i_type(rfence_lo, 17, 0x0, 17, 0x13)),
            (0x8008, i_type(function as i32, 0, 0x0, 16, 0x13)),
            (0x800c, i_type(2, 0, 0x0, 10, 0x13)),
            (0x8010, i_type(0, 0, 0x0, 11, 0x13)),
            (0x8014, lui(12, range_start_page)),
            (0x8018, lui(13, range_size_pages)),
            (0x801c, 0x0000_0073),
            (0x8020, i_type(0, 10, 0x0, 6, 0x13)),
            (0x8024, i_type(0, 11, 0x0, 7, 0x13)),
            (0x8028, 0x0010_0073),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8104, i_type(8, 3, 0x3, 6, 0x03)),
            (0x8108, i_type(8, 4, 0x3, 9, 0x03)),
            (0x810c, 0x0000_006f),
        ],
        &[
            (0x9018, vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]),
            (0xa018, vec![0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x10]),
            (0xb018, vec![0x08, 0x06, 0x04, 0x02, 0x18, 0x16, 0x14, 0x12]),
        ],
    );
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core1.read_register(reg(5)), 0x8877_6655_4433_2211);

    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core1.read_register(reg(6)), 0x10ff_eedd_ccbb_aa99);

    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_translated_action(
            &core1,
            Arc::clone(&store),
            &mut scheduler,
            &transport,
            &page_map
        ),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert_eq!(core1.read_register(reg(9)), 0x1214_1618_0204_0608);
    assert_eq!(
        core1.data_translation_tlb_stats(),
        Some(TranslationTlbStats::new(0, 3, 0, 3, 0))
    );
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(3));

    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_sbi_firmware();

    let run = driver
        .drive_until_host_stop_parallel_with_data_translation(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            160,
            |cpu| GuestEventId::new(event_base + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(event_base),
        source,
        1,
    );
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core0.read_register(reg(7)), 0);
    assert_eq!(core1.data_translation_tlb_entry_count(), Some(1));
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x4000)
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x5000)
        ),
        Some(false)
    );
    assert_eq!(
        core1.data_translation_tlb_contains_entry(
            TranslationAddressSpaceId::global(),
            Address::new(0x7000)
        ),
        Some(true)
    );
}

#[test]
fn riscv_sbi_remote_sfence_vma_range_flushes_each_overlapping_page() {
    assert_remote_tlb_fence_range_flushes_each_overlapping_page(
        SBI_RFENCE_REMOTE_SFENCE_VMA,
        56,
        190,
        4,
        2,
    );
}

#[test]
fn riscv_sbi_remote_hfence_gvma_range_flushes_each_overlapping_physical_page() {
    assert_remote_tlb_fence_range_flushes_each_overlapping_page(
        SBI_RFENCE_REMOTE_HFENCE_GVMA,
        58,
        210,
        9,
        2,
    );
}

#[test]
fn riscv_system_parallel_driver_routes_translated_mmio_and_memory_data() {
    assert_riscv_system_parallel_driver_routes_translated_mmio_and_memory_data(false);
}

#[test]
fn riscv_system_tick_limited_parallel_driver_routes_translated_mmio_and_memory_data() {
    assert_riscv_system_parallel_driver_routes_translated_mmio_and_memory_data(true);
}

fn assert_riscv_system_parallel_driver_routes_translated_mmio_and_memory_data(tick_limited: bool) {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(53);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_fetch = transport
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
    let cpu0_data = transport
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
    let cpu1_fetch = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu1_data = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.dmem"),
                PartitionId::new(1),
                endpoint("l1d1"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    let cluster = RiscvCluster::new([
        translated_riscv_core_with_latency(
            CoreSpec {
                cpu: 0,
                partition: 0,
                agent: 7,
                entry: 0x8000,
                fetch_endpoint: "cpu0.ifetch",
                fetch_route: cpu0_fetch,
                data_endpoint: "cpu0.dmem",
                data_route: cpu0_data,
            },
            2,
        ),
        translated_riscv_core_with_latency(
            CoreSpec {
                cpu: 1,
                partition: 1,
                agent: 8,
                entry: 0x8100,
                fetch_endpoint: "cpu1.ifetch",
                fetch_route: cpu1_fetch,
                data_endpoint: "cpu1.dmem",
                data_route: cpu1_data,
            },
            2,
        ),
    ])
    .unwrap();
    cluster
        .core(CpuId::new(0))
        .unwrap()
        .write_register(reg(2), 0x4000);
    cluster
        .core(CpuId::new(1))
        .unwrap()
        .write_register(reg(2), 0x5010);

    let page_map = two_page_map(0x4000, 0x1000, 0x5000, 0x9000);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8004, i_type(8, 2, 0x3, 6, 0x03)),
            (0x8008, 0x0000_0073),
            (0x8100, i_type(8, 2, 0x3, 5, 0x03)),
            (0x8104, i_type(1, 0, 0x0, 6, 0x13)),
            (0x8108, i_type(1, 6, 0x0, 6, 0x13)),
            (0x810c, i_type(1, 6, 0x0, 6, 0x13)),
            (0x8110, i_type(0, 0, 0x0, 0, 0x13)),
            (0x8114, 0x0010_0073),
        ],
        &[(0x9018, vec![0x78, 0x69, 0x5a, 0x4b, 0x3c, 0x2d, 0x1e, 0x0f])],
    );
    let mut bank =
        MmioRegisterBank::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe],
    )
    .unwrap();
    let mmio_route = MmioRoute::new(PartitionId::new(0), PartitionId::new(2), 2, 2).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x100).unwrap()).unwrap(),
        mmio_route,
        Mutex::new(bank),
    )
    .unwrap();

    let memory_deliveries = Arc::new(Mutex::new(Vec::new()));
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port);

    let run = if tick_limited {
        driver.drive_until_host_stop_or_tick_limit_parallel_with_mmio_and_data_translation(
            &cluster,
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                let memory_deliveries = Arc::clone(&memory_deliveries);
                move |delivery, _context| {
                    memory_deliveries
                        .lock()
                        .unwrap()
                        .push((delivery.request().id(), delivery.request().range().start()));
                    memory_response(&store, &delivery)
                }
            },
            200,
            |cpu| GuestEventId::new(160 + u64::from(cpu.get())),
        )
    } else {
        driver.drive_until_host_stop_parallel_with_mmio_and_data_translation(
            &cluster,
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = Arc::clone(&store);
                let memory_deliveries = Arc::clone(&memory_deliveries);
                move |delivery, _context| {
                    memory_deliveries
                        .lock()
                        .unwrap()
                        .push((delivery.request().id(), delivery.request().range().start()));
                    memory_response(&store, &delivery)
                }
            },
            80,
            |cpu| GuestEventId::new(160 + u64::from(cpu.get())),
        )
    }
    .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(160), source, 0);
    assert_eq!(run.stop_reason(), RiscvSystemRunStopReason::HostStop(stop));
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(5)),
        0xfedc_ba98_7654_3210
    );
    assert_eq!(
        cluster.core(CpuId::new(0)).unwrap().read_register(reg(6)),
        0xfedc_ba98_7654_3210,
        "cpu0 events={:?} tlb={:?}",
        cluster.core(CpuId::new(0)).unwrap().data_access_events(),
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .data_translation_tlb_stats()
    );
    assert_eq!(
        cluster.core(CpuId::new(1)).unwrap().read_register(reg(5)),
        0x0f1e_2d3c_4b5a_6978
    );
    assert_eq!(
        *memory_deliveries.lock().unwrap(),
        vec![(
            MemoryRequestId::new(AgentId::new(8), 1),
            Address::new(0x9018),
        )]
    );

    let cpu0_events = cluster.core(CpuId::new(0)).unwrap().data_access_events();
    let cpu1_events = cluster.core(CpuId::new(1)).unwrap().data_access_events();
    assert_eq!(cpu0_events.len(), 4);
    assert!(cpu0_events.iter().all(|event| {
        matches!(
            event.target(),
            rem6_cpu::RiscvDataAccessTarget::Mmio { route } if route == mmio_route
        ) && event.physical_address() == Address::new(0x1008)
    }));
    assert_eq!(
        cluster
            .core(CpuId::new(0))
            .unwrap()
            .data_translation_tlb_stats(),
        Some(TranslationTlbStats::new(1, 1, 0, 1, 0))
    );
    assert!(matches!(
        cpu1_events[0].target(),
        rem6_cpu::RiscvDataAccessTarget::Memory { route, .. } if route == cpu1_data
    ));
    assert_eq!(cpu1_events[0].physical_address(), Address::new(0x9018));
}
