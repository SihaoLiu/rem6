use rem6_fabric::{FabricLinkId, VirtualNetworkId};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, FabricConnectionConfig, PortDirection,
    PortName, Topology, TopologyBuilder,
};
use rem6_transport::{MemoryRoute, MemoryTransport, TopologyRouteError};

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

fn fabric(link: &str, request_vn: u16, response_vn: u16) -> FabricConnectionConfig {
    FabricConnectionConfig::new(FabricLinkId::new(link).unwrap(), 16).with_virtual_networks(
        VirtualNetworkId::new(request_vn),
        VirtualNetworkId::new(response_vn),
    )
}

fn topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("cpu0"),
                kind("cpu"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("dmem"), PortDirection::Initiator)
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
        .connect_with_fabric_config(
            endpoint("cpu0", "dmem"),
            endpoint("mesh0", "cpu_in"),
            2,
            4,
            fabric("cpu_mesh", 1, 2),
        )
        .unwrap()
        .connect_with_fabric_config(
            endpoint("mesh0", "mem_out"),
            endpoint("mem0", "requests"),
            3,
            5,
            fabric("mesh_mem", 3, 4),
        )
        .unwrap()
        .build()
        .unwrap()
}

#[test]
fn memory_route_can_be_built_from_topology_endpoint_path() {
    let topology = topology();

    let route = MemoryRoute::from_topology(
        &topology,
        endpoint("cpu0", "dmem"),
        endpoint("mem0", "requests"),
    )
    .unwrap();

    assert_eq!(route.source().as_str(), "cpu0.dmem");
    assert_eq!(route.target().as_str(), "mem0.requests");
    assert_eq!(route.source_partition(), PartitionId::new(0));
    assert_eq!(route.target_partition(), PartitionId::new(2));
    assert_eq!(route.request_latency(), 5);
    assert_eq!(route.response_latency(), 9);
    assert_eq!(route.request_virtual_network(), VirtualNetworkId::new(1));
    assert_eq!(route.response_virtual_network(), VirtualNetworkId::new(2));
    assert_eq!(route.hops().len(), 2);
    assert_eq!(route.hops()[0].endpoint().as_str(), "mesh0.cpu_in");
    assert_eq!(route.hops()[0].partition(), PartitionId::new(1));
    assert_eq!(route.hops()[0].request_latency(), 2);
    assert_eq!(route.hops()[0].response_latency(), 4);
    assert!(route.hops()[0].request_fabric_path().is_some());
    assert!(route.hops()[0].response_fabric_path().is_some());
    assert_eq!(route.hops()[1].endpoint().as_str(), "mem0.requests");
    assert_eq!(route.hops()[1].partition(), PartitionId::new(2));
    assert_eq!(route.hops()[1].request_latency(), 3);
    assert_eq!(route.hops()[1].response_latency(), 5);
    assert!(route.hops()[1].request_fabric_path().is_some());
    assert!(route.hops()[1].response_fabric_path().is_some());
}

#[test]
fn memory_transport_reports_missing_topology_connection_without_route_mutation() {
    let topology = topology();
    let mut transport = MemoryTransport::new();

    let error = transport
        .add_topology_route(
            &topology,
            endpoint("mem0", "requests"),
            endpoint("cpu0", "dmem"),
        )
        .unwrap_err();

    assert_eq!(
        error,
        TopologyRouteError::MissingTopologyConnection {
            from: endpoint("mem0", "requests"),
            to: endpoint("cpu0", "dmem"),
        }
    );
    assert_eq!(transport.route_count(), 0);
}
