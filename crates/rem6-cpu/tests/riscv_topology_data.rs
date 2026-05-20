use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, CpuTopologyError, RiscvCluster,
    RiscvClusterError, RiscvClusterTopologyConfig, RiscvCore, RiscvCoreTopologyConfig,
    RiscvDataAccessEventKind, RiscvDataAccessTarget,
};
use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, Register};
use rem6_kernel::{ClockDomain, PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId, MemoryResponse,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, TargetOutcome,
    TransportEndpointId,
};

fn endpoint_id(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn component(name: &str) -> ComponentId {
    ComponentId::new(name).unwrap()
}

fn kind(name: &str) -> ComponentKind {
    ComponentKind::new(name).unwrap()
}

fn port(name: &str) -> PortName {
    PortName::new(name).unwrap()
}

fn endpoint(component_name: &str, port_name: &str) -> Endpoint {
    Endpoint::new(component(component_name), port(port_name))
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
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

fn data_topology() -> Topology {
    TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("icache0"),
                kind("l1_instruction_cache"),
                PartitionId::new(3),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mesh0"),
                kind("mesh_router"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("cpu_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_out"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("icache0", "requests"),
            2,
            2,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mesh0", "cpu_in"), 2, 4)
        .unwrap()
        .connect_with_latencies(
            endpoint("mesh0", "mem_out"),
            endpoint("mem0", "requests"),
            3,
            5,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn multicore_topology() -> Topology {
    TopologyBuilder::new(6)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("cpu1"),
                kind("cpu"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("ifetch"), PortDirection::Initiator)
            .unwrap()
            .add_port(port("dmem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mesh0"),
                kind("mesh_router"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("cpu0_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("cpu1_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_out"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(3),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("icache0"),
                kind("l1_instruction_cache"),
                PartitionId::new(4),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("icache1"),
                kind("l1_instruction_cache"),
                PartitionId::new(5),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("icache0", "requests"),
            2,
            2,
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu1", "ifetch"),
            endpoint("icache1", "requests"),
            3,
            3,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("mesh0", "cpu0_in"), 4, 5)
        .unwrap()
        .connect_with_latencies(endpoint("cpu1", "dmem"), endpoint("mesh0", "cpu1_in"), 6, 7)
        .unwrap()
        .connect_with_latencies(
            endpoint("mesh0", "mem_out"),
            endpoint("mem0", "requests"),
            8,
            9,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn topology_core_config(
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: u64,
) -> RiscvCoreTopologyConfig {
    let cpu_name = format!("cpu{cpu}");
    let icache_name = format!("icache{cpu}");
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(cpu),
            PartitionId::new(partition),
            AgentId::new(agent),
            Address::new(entry),
        ),
        endpoint(&cpu_name, "ifetch"),
        endpoint(&icache_name, "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint(&cpu_name, "dmem"),
        endpoint("mem0", "requests"),
        layout(),
    )
}

#[test]
fn riscv_core_from_topology_registers_fetch_and_data_routes() {
    let topology = data_topology();
    let mut transport = MemoryTransport::new();

    let core = RiscvCore::from_topology(
        &topology,
        &mut transport,
        RiscvCoreTopologyConfig::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            endpoint("cpu0", "ifetch"),
            endpoint("icache0", "requests"),
            layout(),
            AccessSize::new(4).unwrap(),
        )
        .with_data(
            endpoint("cpu0", "dmem"),
            endpoint("mem0", "requests"),
            layout(),
        ),
    )
    .unwrap();

    assert_eq!(transport.route_count(), 2);
    assert_eq!(core.fetch_endpoint().as_str(), "cpu0.ifetch");
    assert_eq!(core.data_endpoint().unwrap().as_str(), "cpu0.dmem");
    assert_eq!(core.fetch_route().get(), 0);
    assert_eq!(core.data_route().unwrap().get(), 1);
    assert_eq!(core.partition(), PartitionId::new(0));
    assert_eq!(core.agent(), AgentId::new(7));
    assert_eq!(core.pc(), Address::new(0x8000));
}

#[test]
fn riscv_cluster_from_topology_registers_multicore_fetch_and_data_routes() {
    let topology = multicore_topology();
    let mut transport = MemoryTransport::new();

    let cluster = RiscvCluster::from_topology(
        &topology,
        &mut transport,
        RiscvClusterTopologyConfig::new([
            topology_core_config(0, 0, 7, 0x8000),
            topology_core_config(1, 1, 8, 0x9000),
        ]),
    )
    .unwrap();

    assert_eq!(cluster.core_count(), 2);
    assert_eq!(cluster.core_ids(), vec![CpuId::new(0), CpuId::new(1)]);
    assert_eq!(transport.route_count(), 4);

    let core0 = cluster.core(CpuId::new(0)).unwrap();
    assert_eq!(core0.fetch_endpoint().as_str(), "cpu0.ifetch");
    assert_eq!(core0.data_endpoint().unwrap().as_str(), "cpu0.dmem");
    assert_eq!(core0.fetch_route().get(), 0);
    assert_eq!(core0.data_route().unwrap().get(), 1);

    let core1 = cluster.core(CpuId::new(1)).unwrap();
    assert_eq!(core1.fetch_endpoint().as_str(), "cpu1.ifetch");
    assert_eq!(core1.data_endpoint().unwrap().as_str(), "cpu1.dmem");
    assert_eq!(core1.fetch_route().get(), 2);
    assert_eq!(core1.data_route().unwrap().get(), 3);
}

#[test]
fn riscv_cluster_from_topology_rejects_duplicates_before_routes_are_registered() {
    let topology = multicore_topology();
    let mut transport = MemoryTransport::new();

    let error = RiscvCluster::from_topology(
        &topology,
        &mut transport,
        RiscvClusterTopologyConfig::new([
            topology_core_config(0, 0, 7, 0x8000),
            topology_core_config(0, 0, 8, 0x9000),
        ]),
    )
    .unwrap_err();

    assert_eq!(
        error,
        CpuTopologyError::Cluster(RiscvClusterError::DuplicateCpu { cpu: CpuId::new(0) })
    );
    assert_eq!(transport.route_count(), 0);
}

#[test]
fn riscv_core_parallel_data_load_uses_topology_built_route() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint_id("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint_id("icache.requests"),
                PartitionId::new(3),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let topology = data_topology();
    let data_route = transport
        .add_topology_route(
            &topology,
            endpoint("cpu0", "dmem"),
            endpoint("mem0", "requests"),
        )
        .unwrap();
    let core = RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint_id("cpu0.ifetch"),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(endpoint_id("cpu0.dmem"), data_route, layout()),
    );

    core.write_register(reg(2), 0x9000);
    core.issue_next_fetch_parallel(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, context| {
            assert_eq!(context.partition(), PartitionId::new(3));
            assert_eq!(delivery.endpoint().as_str(), "icache.requests");
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(word(i_type(8, 2, 0x3, 5, 0x03))),
                )
                .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(core.read_register(reg(5)), 0);

    let data_trace = MemoryTrace::new();
    core.issue_next_data_access_parallel(
        &mut scheduler,
        &transport,
        data_trace.clone(),
        move |delivery, context| {
            assert_eq!(delivery.tick(), 11);
            assert_eq!(context.partition(), PartitionId::new(2));
            assert_eq!(delivery.endpoint().as_str(), "mem0.requests");
            assert_eq!(delivery.request().range().start(), Address::new(0x9008));
            assert_eq!(delivery.request().operation(), MemoryOperation::ReadShared);
            TargetOutcome::Respond(
                MemoryResponse::completed(
                    delivery.request(),
                    Some(vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]),
                )
                .unwrap(),
            )
        },
    )
    .unwrap()
    .unwrap();
    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.final_tick(), 20);
    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    assert_eq!(
        data_trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                6,
                data_route,
                endpoint_id("cpu0.dmem"),
                MemoryTraceKind::RequestSent,
                MemoryRequestId::new(AgentId::new(7), 1),
            ),
            MemoryTraceEvent::request(
                8,
                data_route,
                endpoint_id("mesh0.cpu_in"),
                MemoryTraceKind::RequestArrived,
                MemoryRequestId::new(AgentId::new(7), 1),
            ),
            MemoryTraceEvent::request(
                11,
                data_route,
                endpoint_id("mem0.requests"),
                MemoryTraceKind::RequestArrived,
                MemoryRequestId::new(AgentId::new(7), 1),
            ),
            MemoryTraceEvent::response(
                16,
                data_route,
                endpoint_id("mesh0.cpu_in"),
                MemoryRequestId::new(AgentId::new(7), 1),
                rem6_memory::ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                20,
                data_route,
                endpoint_id("cpu0.dmem"),
                MemoryRequestId::new(AgentId::new(7), 1),
                rem6_memory::ResponseStatus::Completed,
            ),
        ]
    );

    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(
        events[0].target(),
        RiscvDataAccessTarget::Memory {
            route: data_route,
            endpoint: endpoint_id("cpu0.dmem"),
        }
    );
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::Load {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            signed: true,
        }
    );
    assert_eq!(events[0].operation(), MemoryOperation::ReadShared);
    assert_eq!(
        events[1].data(),
        Some(&[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11][..])
    );
}
