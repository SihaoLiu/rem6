use rem6_cache::ChiCacheControllerResultKind;
use rem6_coherence::{
    ChiCpuResponseRecord, ChiHarnessError, LineBackingStore, PartitionedChiDirectoryLineHarness,
    TopologyCacheAgentConfig, TopologyChiDirectoryHarnessConfig, TopologyDirectoryConfig,
};
use rem6_fabric::FabricLinkId;
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_chi::ChiState;
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder, TopologyError,
};
use rem6_transport::MemoryTraceKind;

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

fn fabric_link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn line_data() -> Vec<u8> {
    (0..64).collect()
}

fn backing() -> LineBackingStore {
    LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap()
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

fn write(agent: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::write(
        request_id(agent, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn harness_config() -> TopologyChiDirectoryHarnessConfig {
    TopologyChiDirectoryHarnessConfig::new(
        layout(),
        Address::new(0x3000),
        backing(),
        TopologyDirectoryConfig::new(component("dir0"), port("cache_side"), port("unused_mem")),
        [
            TopologyCacheAgentConfig::new(agent(1), component("l1d0"), port("mem_side")),
            TopologyCacheAgentConfig::new(agent(2), component("l1d1"), port("mem_side")),
        ],
    )
}

fn intermediate_topology() -> Topology {
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
            .add_port(port("unused_mem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                component("l1d1"),
                kind("l1_cache"),
                PartitionId::new(3),
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
            Endpoint::new(component("l1d1"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            4,
            6,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn fabric_topology() -> Topology {
    TopologyBuilder::new(2)
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
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("cache_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("unused_mem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .connect_with_fabric_latencies(
            Endpoint::new(component("l1d0"), port("mem_side")),
            Endpoint::new(component("dir0"), port("cache_side")),
            3,
            5,
            fabric_link("chi_fill"),
            16,
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn topology_chi_harness_builds_intermediate_cache_directory_path() {
    let topology = intermediate_topology();
    let mut harness =
        PartitionedChiDirectoryLineHarness::new_with_topology(&topology, harness_config()).unwrap();

    harness
        .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3002, vec![0xaa, 0xbb]))
        .unwrap();
    let first = harness.run_until_idle_parallel().unwrap();
    assert_eq!(first.final_tick(), 17);
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        ChiState::UniqueDirty
    );

    harness
        .submit_cpu_request_parallel(agent(2), read(2, 0, 0x3000, 4))
        .unwrap();
    let second = harness.run_until_idle_parallel().unwrap();
    assert_eq!(second.final_tick(), 31);
    assert_eq!(
        harness.cpu_responses().last(),
        Some(&ChiCpuResponseRecord::new(
            31,
            ChiCacheControllerResultKind::Fill,
            request_id(2, 0),
            ResponseStatus::Completed,
            Some(vec![0, 1, 0xaa, 0xbb]),
        ))
    );
    assert_eq!(
        harness.cache_state(agent(1)).unwrap(),
        ChiState::SharedClean
    );
    assert_eq!(
        harness.cache_state(agent(2)).unwrap(),
        ChiState::SharedClean
    );

    let agent1_route = harness.route(agent(1)).unwrap();
    let agent2_route = harness.route(agent(2)).unwrap();
    assert_eq!(
        harness
            .trace()
            .iter()
            .map(|event| {
                (
                    event.tick(),
                    event.route(),
                    event.endpoint().as_str().to_owned(),
                    event.kind(),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                0,
                agent1_route,
                "l1d0".to_owned(),
                MemoryTraceKind::RequestSent,
            ),
            (
                2,
                agent1_route,
                "mesh_r0".to_owned(),
                MemoryTraceKind::RequestArrived,
            ),
            (
                7,
                agent1_route,
                "dir0".to_owned(),
                MemoryTraceKind::RequestArrived,
            ),
            (
                14,
                agent1_route,
                "mesh_r0".to_owned(),
                MemoryTraceKind::ResponseArrived,
            ),
            (
                17,
                agent1_route,
                "l1d0".to_owned(),
                MemoryTraceKind::ResponseArrived,
            ),
            (
                17,
                agent2_route,
                "l1d1".to_owned(),
                MemoryTraceKind::RequestSent,
            ),
            (
                21,
                agent2_route,
                "dir0".to_owned(),
                MemoryTraceKind::RequestArrived,
            ),
            (
                31,
                agent2_route,
                "l1d1".to_owned(),
                MemoryTraceKind::ResponseArrived,
            ),
        ]
    );
}

#[test]
fn topology_chi_harness_attaches_fabric_for_cache_directory_path() {
    let topology = fabric_topology();
    let mut config = harness_config();
    config.set_caches([TopologyCacheAgentConfig::new(
        agent(1),
        component("l1d0"),
        port("mem_side"),
    )]);
    let mut harness =
        PartitionedChiDirectoryLineHarness::new_with_topology(&topology, config).unwrap();

    harness
        .submit_cpu_request_parallel(agent(1), read(1, 0, 0x3004, 4))
        .unwrap();
    let run = harness.run_until_idle_parallel().unwrap();

    assert_eq!(run.final_tick(), 12);
    assert_eq!(
        harness.cpu_responses(),
        vec![ChiCpuResponseRecord::new(
            12,
            ChiCacheControllerResultKind::Fill,
            request_id(1, 0),
            ResponseStatus::Completed,
            Some(vec![4, 5, 6, 7]),
        )]
    );
}

#[test]
fn topology_chi_harness_rejects_unknown_cache_component() {
    let topology = fabric_topology();
    let mut config = harness_config();
    config.set_caches([TopologyCacheAgentConfig::new(
        agent(1),
        component("missing_l1"),
        port("mem_side"),
    )]);

    let error = match PartitionedChiDirectoryLineHarness::new_with_topology(&topology, config) {
        Ok(_) => panic!("unknown cache component must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        ChiHarnessError::Topology(TopologyError::UnknownComponent { component: missing })
            if missing == component("missing_l1")
    ));
}

#[test]
fn topology_chi_harness_rejects_missing_cache_directory_connection() {
    let topology = TopologyBuilder::new(2)
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
                PartitionId::new(1),
                clock(1),
            )
            .add_port(port("cache_side"), PortDirection::Target)
            .unwrap()
            .add_port(port("unused_mem"), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
    let mut config = harness_config();
    config.set_caches([TopologyCacheAgentConfig::new(
        agent(1),
        component("l1d0"),
        port("mem_side"),
    )]);

    let error = match PartitionedChiDirectoryLineHarness::new_with_topology(&topology, config) {
        Ok(_) => panic!("missing cache-directory connection must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        ChiHarnessError::MissingTopologyConnection { from, to }
            if from == Endpoint::new(component("l1d0"), port("mem_side"))
                && to == Endpoint::new(component("dir0"), port("cache_side"))
    ));
}
