use rem6_cache::MesiCacheControllerResultKind;
use rem6_coherence::{
    DramMemoryAccessRecord, MesiCpuResponseRecord, PartitionedMesiDirectoryLineHarness,
    TopologyCacheAgentConfig, TopologyDirectoryConfig, TopologyDirectoryHarnessConfig,
    TopologyDramMemoryConfig,
};
use rem6_dram::{DramControllerConfig, DramGeometry, DramMemoryController, DramTiming};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
    ResponseStatus,
};
use rem6_protocol_mesi::MesiState;
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder,
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

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
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
            AccessSize::new(0x8000).unwrap(),
        )
        .unwrap();
    memory
        .insert_line(target, Address::new(0x3000), line_data())
        .unwrap();
    memory
}

fn harness_config() -> TopologyDirectoryHarnessConfig {
    TopologyDirectoryHarnessConfig::new(
        layout(),
        Address::new(0x3000),
        TopologyDirectoryConfig::new(component("dir0"), port("cache_side"), port("mem_side")),
        TopologyDramMemoryConfig::new(component("mem0"), port("requests"), dram_memory()),
        [
            TopologyCacheAgentConfig::new(agent(1), component("l1d0"), port("mem_side")),
            TopologyCacheAgentConfig::new(agent(2), component("l1d1"), port("mem_side")),
        ],
    )
}

fn intermediate_topology() -> Topology {
    TopologyBuilder::new(6)
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
        .unwrap()
}

#[test]
fn topology_mesi_harness_builds_intermediate_dram_path_for_parallel_reader() {
    let topology = intermediate_topology();
    let mut harness =
        PartitionedMesiDirectoryLineHarness::new_with_topology(&topology, harness_config())
            .unwrap();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3004, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.final_tick(), 52);
    assert_eq!(run.cpu_response_count(), 1);
    assert_eq!(run.directory_decision_count(), 1);
    assert_eq!(run.dram_access_count(), 1);
    assert_eq!(run.protocol_activity_count(), 3);
    assert_eq!(run.summary().executed_events(), run.executed_events());
    assert_eq!(run.executed_events(), run.dispatch_count());
    assert_eq!(run.profile().dispatch_count(), run.dispatch_count());
    assert_eq!(run.profile().epoch_count(), run.epoch_count());
    assert!(run.has_parallel_work());
    assert!(run.has_directory_activity());
    assert!(run.has_dram_activity());
    assert_eq!(harness.cache_state(agent(1)).unwrap(), MesiState::Exclusive);
    assert_eq!(
        harness.cpu_responses(),
        vec![MesiCpuResponseRecord::new(
            52,
            MesiCacheControllerResultKind::Fill,
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
            12,
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
