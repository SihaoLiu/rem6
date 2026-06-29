use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{
    CpuResponseRecord, DramMemoryAccessRecord, HarnessError, PartitionedDirectoryLineHarness,
    SubmitKind, TopologyCacheAgentConfig, TopologyDirectoryConfig, TopologyDirectoryHarnessConfig,
    TopologyDramMemoryConfig,
};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_fabric::{FabricLinkId, VirtualNetworkId};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    ResponseStatus,
};
use rem6_protocol_msi::MsiState;
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, FabricConnectionConfig, PortDirection,
    PortName, Topology, TopologyBuilder, TopologyError,
};
use rem6_transport::{MemoryTraceEvent, MemoryTraceKind, TransportEndpointId};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
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

fn endpoint(component_name: &str) -> TransportEndpointId {
    TransportEndpointId::new(component_name).unwrap()
}

fn fabric_link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn virtual_network(value: u16) -> VirtualNetworkId {
    VirtualNetworkId::new(value)
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn dram_target() -> MemoryTargetId {
    MemoryTargetId::new(0)
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn read(agent: u32, sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent, sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

fn topology() -> Topology {
    TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(1),
                clock(1),
            )
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
        .connect_with_latencies(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            3,
            5,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            4,
            6,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mem0"), port("requests")),
            7,
            11,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn dram_memory() -> DramMemoryController {
    let target = dram_target();
    let mut memory = DramMemoryController::new();
    memory
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            DramGeometry::new(4, 256, 64).unwrap(),
            DramTiming::new(3, 5, 7, 2, 4).unwrap(),
        ))
        .unwrap();
    memory
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(target, Address::new(0x1000), line_data())
        .unwrap();
    memory
}

fn harness_config() -> TopologyDirectoryHarnessConfig {
    TopologyDirectoryHarnessConfig::new(
        layout(),
        Address::new(0x1000),
        TopologyDirectoryConfig::new(component("dir0"), port("cache_side"), port("mem_side")),
        TopologyDramMemoryConfig::new(component("mem0"), port("requests"), dram_memory()),
        [
            TopologyCacheAgentConfig::new(agent(1), component("l1d0"), port("mem_side")),
            TopologyCacheAgentConfig::new(agent(2), component("l1d1"), port("mem_side")),
        ],
    )
}

#[test]
fn topology_directory_harness_reserves_declared_fabric_links() {
    let topology = TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(1),
                clock(1),
            )
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
        .connect_with_fabric_latencies(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            2,
            5,
            fabric_link("mesh_shared"),
            8,
        )
        .unwrap()
        .connect_with_fabric_latencies(
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            2,
            5,
            fabric_link("mesh_shared"),
            8,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mem0"), port("requests")),
            7,
            11,
        )
        .unwrap()
        .build()
        .unwrap();
    let mut harness =
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    harness
        .submit_cpu_request(agent(1), read(1, 10, 0x1004, 16))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 11, 0x1008, 16))
        .unwrap();

    harness.run_until_idle();

    let route_a = harness.route(agent(1)).unwrap();
    let route_b = harness.route(agent(2)).unwrap();
    let arrivals: Vec<_> = harness
        .trace()
        .into_iter()
        .filter(|event| event.kind() == MemoryTraceKind::RequestArrived)
        .filter(|event| event.route() == route_a || event.route() == route_b)
        .map(|event| (event.route(), event.tick(), event.request_id()))
        .collect();
    assert_eq!(
        arrivals,
        vec![
            (route_a, 10, request_id(1, 0)),
            (route_b, 18, request_id(2, 0)),
        ]
    );
}

#[test]
fn topology_directory_harness_reserves_fabric_for_dram_fill_response() {
    let topology = TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(1),
                clock(1),
            )
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
        .connect_with_fabric_latencies(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            2,
            5,
            fabric_link("mesh_fill"),
            8,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            4,
            6,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mem0"), port("requests")),
            7,
            11,
        )
        .unwrap()
        .build()
        .unwrap();
    let mut harness =
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    let submit = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 6);
    assert_eq!(run.final_tick(), 49);
    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            49,
            CacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );
    assert_eq!(
        harness.dram_memory_accesses(),
        vec![DramMemoryAccessRecord::new(
            17,
            dram_target(),
            request_id(1, 0),
            0,
            4,
            false,
            25,
        )]
    );

    let cache_route = harness.route(agent(1)).unwrap();
    let memory_route = harness.memory_route().unwrap();
    assert_eq!(
        harness.trace(),
        vec![
            MemoryTraceEvent::request(
                0,
                cache_route,
                endpoint("l1d0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                10,
                cache_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                10,
                memory_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                17,
                memory_route,
                endpoint("mem0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::response(
                36,
                memory_route,
                endpoint("dir0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                49,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn topology_directory_harness_uses_declared_fabric_virtual_networks() {
    let topology = TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(1),
                clock(1),
            )
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
        .connect_with_fabric_config(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            2,
            2,
            FabricConnectionConfig::new(fabric_link("mesh_shared"), 2)
                .with_virtual_networks(virtual_network(0), virtual_network(1)),
        )
        .unwrap()
        .connect_with_fabric_config(
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            2,
            2,
            FabricConnectionConfig::new(fabric_link("mesh_shared"), 2)
                .with_virtual_networks(virtual_network(0), virtual_network(1)),
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mem0"), port("requests")),
            7,
            11,
        )
        .unwrap()
        .build()
        .unwrap();
    let mut harness =
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    harness
        .submit_cpu_request(agent(1), read(1, 20, 0x1004, 4))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 21, 0x1008, 4))
        .unwrap();

    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 130);
    assert_eq!(
        harness.cpu_responses(),
        vec![
            CpuResponseRecord::new(
                98,
                CacheControllerResultKind::Fill,
                request_id(1, 20),
                ResponseStatus::Completed,
                Some(vec![4, 5, 6, 7]),
            ),
            CpuResponseRecord::new(
                130,
                CacheControllerResultKind::Fill,
                request_id(2, 21),
                ResponseStatus::Completed,
                Some(vec![8, 9, 10, 11]),
            ),
        ]
    );
}

#[test]
fn topology_directory_harness_uses_virtual_networks_after_fixed_hops() {
    let topology = TopologyBuilder::new(5)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mesh_r0"),
                kind("mesh_router"),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(port("cache_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("dir_out"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("dir0"),
                kind("directory"),
                PartitionId::new(3),
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
                PartitionId::new(4),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("mesh_r0"), port("cache_in")),
            1,
            1,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("mesh_r0"), port("cache_in")),
            1,
            1,
        )
        .unwrap()
        .connect_with_fabric_config(
            Endpoint::new(component("mesh_r0"), port("dir_out")),
            Endpoint::new(component("dir0"), port("cache_side")),
            2,
            2,
            FabricConnectionConfig::new(fabric_link("mesh_shared"), 2)
                .with_virtual_networks(virtual_network(0), virtual_network(1)),
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mem0"), port("requests")),
            7,
            11,
        )
        .unwrap()
        .build()
        .unwrap();
    let mut harness =
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    harness
        .submit_cpu_request(agent(1), read(1, 30, 0x1004, 4))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 31, 0x1008, 4))
        .unwrap();

    let run = harness.run_until_idle();
    assert_eq!(run.final_tick(), 132);
    assert_eq!(
        harness.cpu_responses(),
        vec![
            CpuResponseRecord::new(
                100,
                CacheControllerResultKind::Fill,
                request_id(1, 30),
                ResponseStatus::Completed,
                Some(vec![4, 5, 6, 7]),
            ),
            CpuResponseRecord::new(
                132,
                CacheControllerResultKind::Fill,
                request_id(2, 31),
                ResponseStatus::Completed,
                Some(vec![8, 9, 10, 11]),
            ),
        ]
    );
}

#[test]
fn topology_directory_harness_builds_cache_directory_dram_path() {
    let topology = topology();
    let mut harness =
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    let submit = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 6);
    assert_eq!(run.final_tick(), 34);
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MsiState::Shared);
    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            34,
            CacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );
    assert_eq!(
        harness.dram_memory_accesses(),
        vec![DramMemoryAccessRecord::new(
            10,
            dram_target(),
            request_id(1, 0),
            0,
            4,
            false,
            18,
        )]
    );

    let cache_route = harness.route(agent(1)).unwrap();
    let memory_route = harness.memory_route().unwrap();
    assert_eq!(
        harness.trace(),
        vec![
            MemoryTraceEvent::request(
                0,
                cache_route,
                endpoint("l1d0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                3,
                cache_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                3,
                memory_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                10,
                memory_route,
                endpoint("mem0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::response(
                29,
                memory_route,
                endpoint("dir0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                34,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn topology_directory_harness_builds_routes_through_intermediate_components() {
    let topology = TopologyBuilder::new(6)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mesh_r0"),
                kind("mesh_router"),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("cache_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("dir_out"), PortDirection::Initiator)
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
                component("mesh_r1"),
                kind("mesh_router"),
                PartitionId::new(3),
                clock(1),
            )
            .add_port(port("dir_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("mem_out"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("mem0"),
                kind("dram"),
                PartitionId::new(4),
                clock(1),
            )
            .add_port(port("requests"), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(5),
                clock(1),
            )
            .add_port(port("mem_side"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("mesh_r0"), port("cache_in")),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("mesh_r0"), port("dir_out")),
            Endpoint::new(component("dir0"), port("cache_side")),
            5,
            7,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mesh_r1"), port("dir_in")),
            4,
            6,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("mesh_r1"), port("mem_out")),
            Endpoint::new(component("mem0"), port("requests")),
            8,
            9,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            4,
            6,
        )
        .unwrap()
        .build()
        .unwrap();
    let mut harness =
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    let submit = harness
        .submit_cpu_request(agent(1), read(1, 0, 0x1004, 4))
        .unwrap();
    assert_eq!(submit.kind(), SubmitKind::ScheduledMiss);

    let run = harness.run_until_idle();
    assert_eq!(run.executed_events(), 10);
    assert_eq!(run.final_tick(), 52);
    assert_eq!(
        harness.cpu_responses(),
        vec![CpuResponseRecord::new(
            52,
            CacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );
    assert_eq!(
        harness.dram_memory_accesses(),
        vec![DramMemoryAccessRecord::new(
            19,
            dram_target(),
            request_id(1, 0),
            0,
            4,
            false,
            27,
        )]
    );

    let cache_route = harness.route(agent(1)).unwrap();
    let memory_route = harness.memory_route().unwrap();
    assert_eq!(
        harness.trace(),
        vec![
            MemoryTraceEvent::request(
                0,
                cache_route,
                endpoint("l1d0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                2,
                cache_route,
                endpoint("mesh_r0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                7,
                cache_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                7,
                memory_route,
                endpoint("dir0"),
                MemoryTraceKind::RequestSent,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                11,
                memory_route,
                endpoint("mesh_r1"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::request(
                19,
                memory_route,
                endpoint("mem0"),
                MemoryTraceKind::RequestArrived,
                request_id(1, 0),
            ),
            MemoryTraceEvent::response(
                36,
                memory_route,
                endpoint("mesh_r1"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                42,
                memory_route,
                endpoint("dir0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                49,
                cache_route,
                endpoint("mesh_r0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                52,
                cache_route,
                endpoint("l1d0"),
                request_id(1, 0),
                ResponseStatus::Completed,
            ),
        ]
    );
}

#[test]
fn topology_directory_harness_rejects_missing_component() {
    let mut config = harness_config();
    config.set_directory(TopologyDirectoryConfig::new(
        component("missing_dir"),
        port("cache_side"),
        port("mem_side"),
    ));

    assert_eq!(
        PartitionedDirectoryLineHarness::new_with_topology(&topology(), config).err(),
        Some(HarnessError::Topology(TopologyError::UnknownComponent {
            component: component("missing_dir")
        }))
    );
}

#[test]
fn topology_directory_harness_rejects_missing_connection() {
    let topology = TopologyBuilder::new(4)
        .add_component(
            ComponentSpec::new(
                component("l1d0"),
                kind("l1_cache"),
                PartitionId::new(0),
                clock(1),
            )
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
        .connect(
            Endpoint::new(component("dir0"), port("mem_side")),
            Endpoint::new(component("mem0"), port("requests")),
            7,
        )
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(
        PartitionedDirectoryLineHarness::new_with_topology(&topology, harness_config()).err(),
        Some(HarnessError::MissingTopologyConnection {
            from: Endpoint::new(component("l1d0"), port("mem_side")),
            to: Endpoint::new(component("dir0"), port("cache_side")),
        })
    );
}
