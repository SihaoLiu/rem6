use rem6_boot::BootImage;
use rem6_coherence::{
    PartitionedMesiDirectoryLineHarness, TopologyCacheAgentConfig, TopologyDirectoryConfig,
    TopologyDirectoryHarnessConfig, TopologyDramMemoryConfig,
};
use rem6_cpu::{CpuId, CpuResetState, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_isa_riscv::Register;
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId};
use rem6_protocol_mesi::MesiState;
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, RiscvSystemRunStopReason, RiscvTopologyDramConfig,
    RiscvTopologyHostConfig, RiscvTopologySystem, StopRequest,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
};
use rem6_transport::{MemoryTrace, MemoryTraceKind, TransportEndpointId};

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

fn transport_endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
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

fn coherent_data_topology() -> Topology {
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
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("cpu_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("dir0"),
                kind("directory"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("cache_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_side"), PortDirection::Initiator)
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
                component("mem1"),
                kind("dram"),
                PartitionId::new(4),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("cpu0", "ifetch"),
            endpoint("mem0", "requests"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(endpoint("cpu0", "dmem"), endpoint("l1d0", "cpu_side"), 2, 2)
        .unwrap()
        .connect_with_latencies(
            endpoint("l1d0", "mem_side"),
            endpoint("dir0", "cache_side"),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("dir0", "mem_side"),
            endpoint("mem1", "requests"),
            4,
            5,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn core_config() -> RiscvCoreTopologyConfig {
    RiscvCoreTopologyConfig::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            agent(7),
            Address::new(0x8000),
        ),
        endpoint("cpu0", "ifetch"),
        endpoint("mem0", "requests"),
        layout(),
        AccessSize::new(4).unwrap(),
    )
    .with_data(
        endpoint("cpu0", "dmem"),
        endpoint("l1d0", "cpu_side"),
        layout(),
    )
}

fn code_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), word(i_type(8, 2, 0x3, 5, 0x03)))
        .unwrap()
        .add_segment(Address::new(0x8004), word(0x0000_0073))
        .unwrap()
}

fn code_dram_config() -> RiscvTopologyDramConfig {
    RiscvTopologyDramConfig::new(
        MemoryTargetId::new(0),
        layout(),
        DramGeometry::new(2, 64, 16).unwrap(),
        DramTiming::new(5, 7, 11, 3, 2).unwrap(),
    )
    .add_region(Address::new(0x8000), AccessSize::new(0x1000).unwrap())
}

fn data_dram_memory() -> DramMemoryController {
    let target = MemoryTargetId::new(7);
    let mut memory = DramMemoryController::new();
    memory
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            DramGeometry::new(4, 64, 16).unwrap(),
            DramTiming::new(3, 5, 9, 2, 2).unwrap(),
        ))
        .unwrap();
    memory
        .map_region(
            target,
            Address::new(0x3000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(
            target,
            Address::new(0x3000),
            vec![
                0, 1, 2, 3, 4, 5, 6, 7, 0x89, 0x67, 0x45, 0x23, 0x01, 0xef, 0xcd, 0xab,
            ],
        )
        .unwrap();
    memory
}

fn mesi_data_harness(topology: &Topology) -> PartitionedMesiDirectoryLineHarness {
    PartitionedMesiDirectoryLineHarness::new_with_topology(
        topology,
        TopologyDirectoryHarnessConfig::new(
            layout(),
            Address::new(0x3000),
            TopologyDirectoryConfig::new(component("dir0"), port("cache_side"), port("mem_side")),
            TopologyDramMemoryConfig::new(component("mem1"), port("requests"), data_dram_memory()),
            [TopologyCacheAgentConfig::new(
                agent(7),
                component("l1d0"),
                port("mem_side"),
            )],
        ),
    )
    .unwrap()
}

#[test]
fn topology_system_routes_cpu_data_load_through_mesi_cache_backend() {
    let topology = coherent_data_topology();
    let source = GuestSourceId::new(101);
    let system = RiscvTopologySystem::with_min_remote_delay(
        topology.clone(),
        RiscvClusterTopologyConfig::new([core_config()]),
        2,
    )
    .unwrap()
    .with_boot_image_dram_memory(code_dram_config(), &code_image())
    .unwrap()
    .with_mesi_data_cache(mesi_data_harness(&topology))
    .unwrap()
    .with_host_controller(
        RiscvTopologyHostConfig::new(PartitionId::new(5), 2, source),
        StatsRegistry::new(),
    )
    .unwrap();
    system
        .cluster()
        .core(CpuId::new(0))
        .unwrap()
        .write_register(Register::new(2).unwrap(), 0x3000);

    let data_trace = MemoryTrace::new();
    let run = system
        .drive_attached_until_host_stop_parallel(
            MemoryTrace::new(),
            data_trace.clone(),
            40,
            |cpu: CpuId| GuestEventId::new(1010 + u64::from(cpu.get())),
        )
        .unwrap();

    assert_eq!(
        run.stop_reason(),
        RiscvSystemRunStopReason::HostStop(StopRequest::new(
            run.final_tick().unwrap(),
            GuestEventId::new(1010),
            source,
            0,
        )),
    );
    assert_eq!(
        system
            .cluster()
            .core(CpuId::new(0))
            .unwrap()
            .read_register(Register::new(5).unwrap()),
        0xabcd_ef01_2345_6789,
    );
    assert_eq!(
        data_trace
            .snapshot()
            .iter()
            .map(|event| (event.endpoint().clone(), event.kind()))
            .collect::<Vec<_>>(),
        vec![
            (
                transport_endpoint("cpu0.dmem"),
                MemoryTraceKind::RequestSent,
            ),
            (
                transport_endpoint("l1d0.cpu_side"),
                MemoryTraceKind::RequestArrived,
            ),
            (
                transport_endpoint("cpu0.dmem"),
                MemoryTraceKind::ResponseArrived,
            ),
        ],
    );
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(0))
            .unwrap()
            .profile()
            .read_count(),
        2
    );
    assert_eq!(
        run.dram_target_activity(MemoryTargetId::new(7))
            .unwrap()
            .profile()
            .read_count(),
        1
    );
    assert_eq!(run.active_dram_target_count(), 2);

    let cache = system.mesi_data_cache().unwrap();
    let harness = cache.lock().unwrap();
    assert_eq!(harness.cache_state(agent(7)).unwrap(), MesiState::Exclusive);
    assert_eq!(harness.dram_memory_accesses().len(), 1);
    assert!(harness
        .trace()
        .iter()
        .any(|event: &rem6_transport::MemoryTraceEvent| {
            event.endpoint() == &transport_endpoint("mem1")
        }));
    drop(harness);

    let cache_runs = system.mesi_data_cache_runs();
    assert_eq!(cache_runs.len(), 1);
    assert_eq!(cache_runs[0].dram_access_count(), 1);
    assert_eq!(cache_runs[0].active_dram_target_count(), 1);
}
