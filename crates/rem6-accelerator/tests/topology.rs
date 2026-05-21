use rem6_accelerator::{
    AcceleratorCommandId, AcceleratorDmaCompletion, AcceleratorDmaCopy, AcceleratorEngineConfig,
    AcceleratorEngineId, AcceleratorError, AcceleratorTopologyConfig, AcceleratorTopologyDevice,
};
use rem6_kernel::{ClockDomain, PartitionId};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, LineMemoryStore, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_topology::{
    ComponentId, ComponentKind, ComponentSpec, Endpoint, PortDirection, PortName, Topology,
    TopologyBuilder, TopologyError,
};
use rem6_transport::{
    MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, RequestDelivery, TargetOutcome,
};
use std::sync::{Arc, Mutex};

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

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn accelerator_topology() -> Topology {
    TopologyBuilder::new(3)
        .add_component(
            ComponentSpec::new(
                component("accelerator0"),
                kind("accelerator"),
                PartitionId::new(0),
                clock(1),
            )
            .add_port(port("dma"), PortDirection::Initiator)
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
            .add_port(port("accelerator_in"), PortDirection::Target)
            .unwrap()
            .add_port(port("memory_out"), PortDirection::Initiator)
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
            endpoint("accelerator0", "dma"),
            endpoint("mesh0", "accelerator_in"),
            2,
            4,
        )
        .unwrap()
        .connect_with_latencies(
            endpoint("mesh0", "memory_out"),
            endpoint("mem0", "requests"),
            3,
            5,
        )
        .unwrap()
        .build()
        .unwrap()
}

fn memory_store() -> Arc<Mutex<LineMemoryStore>> {
    let mut store = LineMemoryStore::new(line_layout());
    let mut source_line = vec![0; 64];
    source_line[16..20].copy_from_slice(&[0xa0, 0xb1, 0xc2, 0xd3]);
    store
        .insert_line(Address::new(0x1000), source_line)
        .unwrap();
    store
        .insert_line(Address::new(0x3000), vec![0; 64])
        .unwrap();
    Arc::new(Mutex::new(store))
}

#[test]
fn accelerator_topology_builds_dma_route_from_declared_endpoints() {
    let topology = accelerator_topology();
    let mut transport = MemoryTransport::new();
    let config = AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(12), PartitionId::new(0), 4).unwrap(),
        endpoint("accelerator0", "dma"),
        endpoint("mem0", "requests"),
    );

    let device =
        AcceleratorTopologyDevice::from_topology(&topology, &mut transport, config).unwrap();
    let route = transport.route(device.dma_route()).unwrap();

    assert_eq!(device.engine().id(), AcceleratorEngineId::new(12));
    assert_eq!(device.engine().partition(), PartitionId::new(0));
    assert_eq!(device.engine().lanes(), 4);
    assert_eq!(transport.route_count(), 1);
    assert_eq!(route.source().as_str(), "accelerator0.dma");
    assert_eq!(route.target().as_str(), "mem0.requests");
    assert_eq!(route.source_partition(), PartitionId::new(0));
    assert_eq!(route.target_partition(), PartitionId::new(2));
    assert_eq!(route.request_latency(), 5);
    assert_eq!(route.response_latency(), 9);
    assert_eq!(route.hops().len(), 2);
    assert_eq!(route.hops()[0].endpoint().as_str(), "mesh0.accelerator_in");
    assert_eq!(route.hops()[0].partition(), PartitionId::new(1));
    assert_eq!(route.hops()[1].endpoint().as_str(), "mem0.requests");
    assert_eq!(route.hops()[1].partition(), PartitionId::new(2));
}

#[test]
fn accelerator_topology_device_uses_declared_route_for_dma_copy() {
    let topology = accelerator_topology();
    let mut transport = MemoryTransport::new();
    let device = AcceleratorTopologyDevice::from_topology(
        &topology,
        &mut transport,
        AcceleratorTopologyConfig::new(
            AcceleratorEngineConfig::new(AcceleratorEngineId::new(21), PartitionId::new(0), 2)
                .unwrap(),
            endpoint("accelerator0", "dma"),
            endpoint("mem0", "requests"),
        ),
    )
    .unwrap();
    let mut scheduler = rem6_kernel::PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let trace = MemoryTrace::new();
    let store = memory_store();
    let route = device.dma_route();
    let read_request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(21), 1),
        Address::new(0x1010),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let copy = AcceleratorDmaCopy::new(
        AcceleratorCommandId::new(99),
        route,
        read_request.clone(),
        route,
        MemoryRequestId::new(AgentId::new(21), 2),
        Address::new(0x3008),
    )
    .unwrap();

    let read_store = Arc::clone(&store);
    device
        .engine()
        .submit_dma_copy_read(
            &mut scheduler,
            &transport,
            copy,
            trace.clone(),
            move |delivery: RequestDelivery, _context| {
                let response = read_store
                    .lock()
                    .unwrap()
                    .respond(delivery.request())
                    .unwrap()
                    .unwrap();
                TargetOutcome::Respond(response)
            },
        )
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    let write_store = Arc::clone(&store);
    device
        .engine()
        .issue_next_dma_write(
            &mut scheduler,
            &transport,
            trace.clone(),
            move |delivery: RequestDelivery, _context| {
                let response = write_store
                    .lock()
                    .unwrap()
                    .respond(delivery.request())
                    .unwrap()
                    .unwrap();
                TargetOutcome::Respond(response)
            },
        )
        .unwrap()
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    let destination = store
        .lock()
        .unwrap()
        .line_data(Address::new(0x3000))
        .unwrap();
    assert_eq!(&destination[8..12], &[0xa0, 0xb1, 0xc2, 0xd3]);
    assert_eq!(
        device.engine().dma_completions(),
        vec![AcceleratorDmaCompletion::new(
            AcceleratorCommandId::new(99),
            read_request.id(),
            MemoryRequestId::new(AgentId::new(21), 2),
            14,
            28,
        )]
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                rem6_transport::TransportEndpointId::new("accelerator0.dma").unwrap(),
                MemoryTraceKind::RequestSent,
                read_request.id(),
            ),
            MemoryTraceEvent::request(
                2,
                route,
                rem6_transport::TransportEndpointId::new("mesh0.accelerator_in").unwrap(),
                MemoryTraceKind::RequestArrived,
                read_request.id(),
            ),
            MemoryTraceEvent::request(
                5,
                route,
                rem6_transport::TransportEndpointId::new("mem0.requests").unwrap(),
                MemoryTraceKind::RequestArrived,
                read_request.id(),
            ),
            MemoryTraceEvent::response(
                10,
                route,
                rem6_transport::TransportEndpointId::new("mesh0.accelerator_in").unwrap(),
                read_request.id(),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                14,
                route,
                rem6_transport::TransportEndpointId::new("accelerator0.dma").unwrap(),
                read_request.id(),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::request(
                14,
                route,
                rem6_transport::TransportEndpointId::new("accelerator0.dma").unwrap(),
                MemoryTraceKind::RequestSent,
                MemoryRequestId::new(AgentId::new(21), 2),
            ),
            MemoryTraceEvent::request(
                16,
                route,
                rem6_transport::TransportEndpointId::new("mesh0.accelerator_in").unwrap(),
                MemoryTraceKind::RequestArrived,
                MemoryRequestId::new(AgentId::new(21), 2),
            ),
            MemoryTraceEvent::request(
                19,
                route,
                rem6_transport::TransportEndpointId::new("mem0.requests").unwrap(),
                MemoryTraceKind::RequestArrived,
                MemoryRequestId::new(AgentId::new(21), 2),
            ),
            MemoryTraceEvent::response(
                24,
                route,
                rem6_transport::TransportEndpointId::new("mesh0.accelerator_in").unwrap(),
                MemoryRequestId::new(AgentId::new(21), 2),
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(
                28,
                route,
                rem6_transport::TransportEndpointId::new("accelerator0.dma").unwrap(),
                MemoryRequestId::new(AgentId::new(21), 2),
                ResponseStatus::Completed,
            ),
        ],
    );
}

#[test]
fn accelerator_topology_rejects_source_partition_mismatch_without_route_mutation() {
    let topology = accelerator_topology();
    let mut transport = MemoryTransport::new();
    let config = AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(13), PartitionId::new(2), 2).unwrap(),
        endpoint("accelerator0", "dma"),
        endpoint("mem0", "requests"),
    );

    let error =
        AcceleratorTopologyDevice::from_topology(&topology, &mut transport, config).unwrap_err();

    assert_eq!(
        error,
        AcceleratorError::SourcePartitionMismatch {
            endpoint: endpoint("accelerator0", "dma"),
            expected: PartitionId::new(2),
            actual: PartitionId::new(0),
        },
    );
    assert_eq!(transport.route_count(), 0);
}

#[test]
fn accelerator_topology_rejects_unknown_source_component_without_route_mutation() {
    let topology = accelerator_topology();
    let mut transport = MemoryTransport::new();
    let config = AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(14), PartitionId::new(0), 2).unwrap(),
        endpoint("missing_accelerator", "dma"),
        endpoint("mem0", "requests"),
    );

    let error =
        AcceleratorTopologyDevice::from_topology(&topology, &mut transport, config).unwrap_err();

    assert_eq!(
        error,
        AcceleratorError::Topology(TopologyError::UnknownComponent {
            component: component("missing_accelerator"),
        }),
    );
    assert_eq!(transport.route_count(), 0);
}

#[test]
fn accelerator_topology_rejects_unknown_source_port_without_route_mutation() {
    let topology = accelerator_topology();
    let mut transport = MemoryTransport::new();
    let config = AcceleratorTopologyConfig::new(
        AcceleratorEngineConfig::new(AcceleratorEngineId::new(15), PartitionId::new(0), 2).unwrap(),
        endpoint("accelerator0", "missing_dma"),
        endpoint("mem0", "requests"),
    );

    let error =
        AcceleratorTopologyDevice::from_topology(&topology, &mut transport, config).unwrap_err();

    assert_eq!(
        error,
        AcceleratorError::Topology(TopologyError::UnknownPort {
            component: component("accelerator0"),
            port: port("missing_dma"),
        }),
    );
    assert_eq!(transport.route_count(), 0);
}
