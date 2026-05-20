use rem6_kernel::{ClockDomain, PartitionId};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, TopologyBuilder,
    TopologyError,
};

fn clock(period: u64) -> ClockDomain {
    ClockDomain::new(period).unwrap()
}

#[test]
fn topology_builds_valid_component_graph_with_stable_endpoints() {
    let core = ComponentId::new("core0").unwrap();
    let l1 = ComponentId::new("l1i0").unwrap();
    let memory = ComponentId::new("dram0").unwrap();
    let requests = PortName::new("requests").unwrap();
    let cpu_side = PortName::new("cpu_side").unwrap();
    let mem_side = PortName::new("mem_side").unwrap();

    let topology = TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                core.clone(),
                ComponentKind::new("timing_cpu").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(requests.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                l1.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(1),
                clock(2),
            )
            .add_port(cpu_side.clone(), PortDirection::Target)
            .unwrap()
            .add_port(mem_side.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                memory.clone(),
                ComponentKind::new("dram").unwrap(),
                PartitionId::new(2),
                clock(4),
            )
            .add_port(requests.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect(
            Endpoint::new(core.clone(), requests.clone()),
            Endpoint::new(l1.clone(), cpu_side.clone()),
            3,
        )
        .unwrap()
        .connect(
            Endpoint::new(l1.clone(), mem_side.clone()),
            Endpoint::new(memory.clone(), requests.clone()),
            7,
        )
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(topology.partition_count(), 3);
    assert_eq!(topology.component_count(), 3);
    assert_eq!(topology.connection_count(), 2);

    let core_spec = topology.component(&core).unwrap();
    assert_eq!(core_spec.kind().as_str(), "timing_cpu");
    assert_eq!(core_spec.partition(), PartitionId::new(0));
    assert_eq!(core_spec.clock_domain().period(), 1);
    assert_eq!(
        core_spec.port_direction(&requests),
        Some(PortDirection::Initiator)
    );

    assert_eq!(
        topology.components_in_partition(PartitionId::new(1)),
        std::slice::from_ref(&l1)
    );
    assert_eq!(
        topology
            .connection_between(&Endpoint::new(core, requests), &Endpoint::new(l1, cpu_side))
            .unwrap()
            .latency(),
        3
    );
}

#[test]
fn topology_records_asymmetric_connection_latencies() {
    let cache = ComponentId::new("l1d0").unwrap();
    let directory = ComponentId::new("dir0").unwrap();
    let mem_side = PortName::new("mem_side").unwrap();
    let cache_side = PortName::new("cache_side").unwrap();

    let topology = TopologyBuilder::new(2)
        .add_component(
            ComponentSpec::new(
                cache.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(mem_side.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                directory.clone(),
                ComponentKind::new("directory").unwrap(),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(cache_side.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(cache.clone(), mem_side.clone()),
            Endpoint::new(directory.clone(), cache_side.clone()),
            3,
            11,
        )
        .unwrap()
        .build()
        .unwrap();

    let connection = topology
        .connection_between(
            &Endpoint::new(cache, mem_side),
            &Endpoint::new(directory, cache_side),
        )
        .unwrap();
    assert_eq!(connection.request_latency(), 3);
    assert_eq!(connection.response_latency(), 11);
}

#[test]
fn topology_finds_lowest_latency_component_path() {
    let cache = ComponentId::new("l1d0").unwrap();
    let router = ComponentId::new("mesh_r0").unwrap();
    let directory = ComponentId::new("dir0").unwrap();
    let mem_side = PortName::new("mem_side").unwrap();
    let ingress = PortName::new("ingress").unwrap();
    let egress = PortName::new("egress").unwrap();
    let cache_side = PortName::new("cache_side").unwrap();

    let topology = TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                cache.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(mem_side.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                router.clone(),
                ComponentKind::new("mesh_router").unwrap(),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(ingress.clone(), PortDirection::Target)
            .unwrap()
            .add_port(egress.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                directory.clone(),
                ComponentKind::new("directory").unwrap(),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(cache_side.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(cache.clone(), mem_side.clone()),
            Endpoint::new(directory.clone(), cache_side.clone()),
            20,
            21,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(cache.clone(), mem_side.clone()),
            Endpoint::new(router.clone(), ingress.clone()),
            3,
            5,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(router.clone(), egress.clone()),
            Endpoint::new(directory.clone(), cache_side.clone()),
            7,
            11,
        )
        .unwrap()
        .build()
        .unwrap();

    let path = topology
        .find_component_path(&cache, &directory)
        .expect("component path");

    assert_eq!(path.source(), &cache);
    assert_eq!(path.target(), &directory);
    assert_eq!(path.request_latency(), 10);
    assert_eq!(path.response_latency(), 16);
    assert_eq!(path.hops().len(), 2);
    assert_eq!(
        path.hops()[0].from(),
        &Endpoint::new(cache.clone(), mem_side)
    );
    assert_eq!(path.hops()[0].to(), &Endpoint::new(router.clone(), ingress));
    assert_eq!(path.hops()[0].request_latency(), 3);
    assert_eq!(path.hops()[0].response_latency(), 5);
    assert_eq!(path.hops()[1].from(), &Endpoint::new(router, egress));
    assert_eq!(path.hops()[1].to(), &Endpoint::new(directory, cache_side));
    assert_eq!(path.hops()[1].request_latency(), 7);
    assert_eq!(path.hops()[1].response_latency(), 11);
}

#[test]
fn topology_finds_endpoint_path_through_intermediate_components() {
    let cache = ComponentId::new("l1d0").unwrap();
    let router = ComponentId::new("mesh_r0").unwrap();
    let directory = ComponentId::new("dir0").unwrap();
    let mem_side = PortName::new("mem_side").unwrap();
    let ingress = PortName::new("ingress").unwrap();
    let egress = PortName::new("egress").unwrap();
    let cache_side = PortName::new("cache_side").unwrap();

    let from = Endpoint::new(cache.clone(), mem_side.clone());
    let to = Endpoint::new(directory.clone(), cache_side.clone());
    let topology = TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                cache.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(mem_side.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                router.clone(),
                ComponentKind::new("mesh_router").unwrap(),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(ingress.clone(), PortDirection::Target)
            .unwrap()
            .add_port(egress.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                directory.clone(),
                ComponentKind::new("directory").unwrap(),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(cache_side.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(
            from.clone(),
            Endpoint::new(router.clone(), ingress.clone()),
            2,
            3,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(router.clone(), egress.clone()),
            to.clone(),
            5,
            7,
        )
        .unwrap()
        .build()
        .unwrap();

    let path = topology
        .find_endpoint_path(&from, &to)
        .expect("endpoint path");

    assert_eq!(path.source(), &cache);
    assert_eq!(path.target(), &directory);
    assert_eq!(path.request_latency(), 7);
    assert_eq!(path.response_latency(), 10);
    assert_eq!(path.hops().len(), 2);
    assert_eq!(path.hops()[0].from(), &from);
    assert_eq!(path.hops()[1].to(), &to);
}

#[test]
fn topology_endpoint_path_respects_boundary_ports() {
    let cache = ComponentId::new("l1d0").unwrap();
    let router = ComponentId::new("mesh_r0").unwrap();
    let directory = ComponentId::new("dir0").unwrap();
    let mem_side = PortName::new("mem_side").unwrap();
    let debug_side = PortName::new("debug_side").unwrap();
    let ingress = PortName::new("ingress").unwrap();
    let egress = PortName::new("egress").unwrap();
    let cache_side = PortName::new("cache_side").unwrap();

    let requested_from = Endpoint::new(cache.clone(), mem_side.clone());
    let debug_from = Endpoint::new(cache.clone(), debug_side.clone());
    let to = Endpoint::new(directory.clone(), cache_side.clone());
    let topology = TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                cache.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(mem_side.clone(), PortDirection::Initiator)
            .unwrap()
            .add_port(debug_side.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                router.clone(),
                ComponentKind::new("mesh_router").unwrap(),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(ingress.clone(), PortDirection::Target)
            .unwrap()
            .add_port(egress.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                directory.clone(),
                ComponentKind::new("directory").unwrap(),
                PartitionId::new(2),
                clock(1),
            )
            .add_port(cache_side.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect_with_latencies(debug_from.clone(), to.clone(), 1, 1)
        .unwrap()
        .connect_with_latencies(
            requested_from.clone(),
            Endpoint::new(router.clone(), ingress.clone()),
            4,
            6,
        )
        .unwrap()
        .connect_with_latencies(
            Endpoint::new(router.clone(), egress.clone()),
            to.clone(),
            8,
            10,
        )
        .unwrap()
        .build()
        .unwrap();

    let path = topology
        .find_endpoint_path(&requested_from, &to)
        .expect("endpoint path");

    assert_eq!(path.request_latency(), 12);
    assert_eq!(path.response_latency(), 16);
    assert_eq!(path.hops().len(), 2);
    assert_eq!(path.hops()[0].from(), &requested_from);
    assert_eq!(path.hops()[1].to(), &to);
}

#[test]
fn topology_reports_absent_component_path() {
    let cache = ComponentId::new("l1d0").unwrap();
    let directory = ComponentId::new("dir0").unwrap();
    let mem_side = PortName::new("mem_side").unwrap();
    let cache_side = PortName::new("cache_side").unwrap();

    let topology = TopologyBuilder::new(2)
        .add_component(
            ComponentSpec::new(
                cache.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(mem_side.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                directory.clone(),
                ComponentKind::new("directory").unwrap(),
                PartitionId::new(1),
                clock(1),
            )
            .add_port(cache_side.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .connect(
            Endpoint::new(cache.clone(), mem_side),
            Endpoint::new(directory.clone(), cache_side),
            3,
        )
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(topology.find_component_path(&directory, &cache), None);
}

#[test]
fn topology_rejects_duplicate_components_and_ports() {
    let core = ComponentId::new("core0").unwrap();
    let requests = PortName::new("requests").unwrap();

    let duplicate_port = ComponentSpec::new(
        core.clone(),
        ComponentKind::new("timing_cpu").unwrap(),
        PartitionId::new(0),
        clock(1),
    )
    .add_port(requests.clone(), PortDirection::Initiator)
    .unwrap()
    .add_port(requests.clone(), PortDirection::Target)
    .unwrap_err();

    assert_eq!(
        duplicate_port,
        TopologyError::DuplicatePort {
            component: core.clone(),
            port: requests
        }
    );

    let component = ComponentSpec::new(
        core.clone(),
        ComponentKind::new("timing_cpu").unwrap(),
        PartitionId::new(0),
        clock(1),
    );
    let duplicate_component = TopologyBuilder::new(1)
        .add_component(component.clone())
        .unwrap()
        .add_component(component)
        .unwrap_err();

    assert_eq!(
        duplicate_component,
        TopologyError::DuplicateComponent { component: core }
    );
}

#[test]
fn topology_rejects_unknown_connection_endpoints() {
    let core = ComponentId::new("core0").unwrap();
    let cache = ComponentId::new("l1d0").unwrap();
    let requests = PortName::new("requests").unwrap();
    let missing = PortName::new("missing").unwrap();

    let unknown_component = TopologyBuilder::new(1)
        .add_component(
            ComponentSpec::new(
                core.clone(),
                ComponentKind::new("timing_cpu").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(requests.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .connect(
            Endpoint::new(core.clone(), requests.clone()),
            Endpoint::new(cache.clone(), requests.clone()),
            1,
        )
        .unwrap_err();

    assert_eq!(
        unknown_component,
        TopologyError::UnknownComponent { component: cache }
    );

    let unknown_port = TopologyBuilder::new(1)
        .add_component(
            ComponentSpec::new(
                core.clone(),
                ComponentKind::new("timing_cpu").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(requests.clone(), PortDirection::Initiator)
            .unwrap(),
        )
        .unwrap()
        .connect(
            Endpoint::new(core.clone(), requests),
            Endpoint::new(core.clone(), missing.clone()),
            1,
        )
        .unwrap_err();

    assert_eq!(
        unknown_port,
        TopologyError::UnknownPort {
            component: core,
            port: missing
        }
    );
}

#[test]
fn topology_rejects_direction_mismatches_and_zero_latency() {
    let core = ComponentId::new("core0").unwrap();
    let cache = ComponentId::new("l1d0").unwrap();
    let requests = PortName::new("requests").unwrap();
    let cpu_side = PortName::new("cpu_side").unwrap();

    let builder = TopologyBuilder::new(2)
        .add_component(
            ComponentSpec::new(
                core.clone(),
                ComponentKind::new("timing_cpu").unwrap(),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(requests.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap()
        .add_component(
            ComponentSpec::new(
                cache.clone(),
                ComponentKind::new("l1_cache").unwrap(),
                PartitionId::new(1),
                clock(2),
            )
            .add_port(cpu_side.clone(), PortDirection::Target)
            .unwrap(),
        )
        .unwrap();

    let direction = builder
        .clone()
        .connect(
            Endpoint::new(core.clone(), requests.clone()),
            Endpoint::new(cache.clone(), cpu_side.clone()),
            1,
        )
        .unwrap_err();

    assert_eq!(
        direction,
        TopologyError::InvalidConnectionDirection {
            from: Endpoint::new(core.clone(), requests.clone()),
            from_direction: PortDirection::Target,
            to: Endpoint::new(cache.clone(), cpu_side.clone()),
            to_direction: PortDirection::Target,
        }
    );

    let zero_latency = builder
        .connect(
            Endpoint::new(core.clone(), requests),
            Endpoint::new(cache.clone(), cpu_side),
            0,
        )
        .unwrap_err();

    assert_eq!(zero_latency, TopologyError::ZeroConnectionLatency);
}

#[test]
fn topology_rejects_partitions_outside_declared_range() {
    let core = ComponentId::new("core0").unwrap();

    let error = TopologyBuilder::new(2)
        .add_component(ComponentSpec::new(
            core.clone(),
            ComponentKind::new("timing_cpu").unwrap(),
            PartitionId::new(2),
            clock(1),
        ))
        .unwrap_err();

    assert_eq!(
        error,
        TopologyError::PartitionOutOfRange {
            component: core,
            partition: PartitionId::new(2),
            partitions: 2
        }
    );
}

#[test]
fn topology_rejects_empty_identifiers() {
    assert_eq!(
        ComponentId::new("").unwrap_err(),
        TopologyError::EmptyIdentifier
    );
    assert_eq!(
        ComponentKind::new("").unwrap_err(),
        TopologyError::EmptyIdentifier
    );
    assert_eq!(
        PortName::new("").unwrap_err(),
        TopologyError::EmptyIdentifier
    );
}
