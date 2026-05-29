use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome,
    TransportEndpointId, TransportError, TransportLatency,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request(id: u64, address: u64, size: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(1), id),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        line_layout(),
    )
    .unwrap()
}

#[test]
fn transport_rejects_invalid_routes_and_submissions() {
    assert_eq!(
        TransportEndpointId::new("").unwrap_err(),
        TransportError::EmptyEndpoint
    );
    assert_eq!(
        MemoryRoute::new(
            endpoint("core0"),
            PartitionId::new(0),
            endpoint("memory0"),
            PartitionId::new(1),
            0,
            1,
        )
        .unwrap_err(),
        TransportError::ZeroRouteLatency {
            latency: TransportLatency::Request
        }
    );
    assert_eq!(
        MemoryRoute::new_path(
            endpoint("core0"),
            PartitionId::new(0),
            Vec::<MemoryRouteHop>::new(),
        )
        .unwrap_err(),
        TransportError::EmptyRoutePath
    );
    assert_eq!(
        MemoryRouteHop::new(endpoint("mesh_r0"), PartitionId::new(1), 1, 0).unwrap_err(),
        TransportError::ZeroRouteLatency {
            latency: TransportLatency::Response
        }
    );

    let mut transport = MemoryTransport::new();
    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = MemoryRoute::new(
        core.clone(),
        PartitionId::new(0),
        memory.clone(),
        PartitionId::new(1),
        4,
        4,
    )
    .unwrap();
    transport.add_route(route.clone()).unwrap();
    assert_eq!(
        transport.add_route(route).unwrap_err(),
        TransportError::DuplicateRoute {
            source: core.clone(),
            target: memory.clone()
        }
    );

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 3).unwrap();
    let error = transport
        .submit(
            &mut scheduler,
            MemoryRouteId::new(99),
            request(40, 0x5000, 1),
            MemoryTrace::new(),
            |_, _| TargetOutcome::NoResponse,
            |_| {},
        )
        .unwrap_err();
    assert_eq!(
        error,
        TransportError::UnknownRoute {
            route: MemoryRouteId::new(99)
        }
    );

    let short_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core1"),
                PartitionId::new(0),
                endpoint("memory1"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let error = transport
        .submit(
            &mut scheduler,
            short_route,
            request(41, 0x5008, 1),
            MemoryTrace::new(),
            |_, _| TargetOutcome::NoResponse,
            |_| {},
        )
        .unwrap_err();
    assert_eq!(
        error,
        TransportError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: PartitionId::new(0),
            target: PartitionId::new(1),
            source_tick: 0,
            delivery_tick: 2,
            minimum_delivery_tick: 3,
        })
    );

    let path_route = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("core2"),
                PartitionId::new(0),
                [MemoryRouteHop::new(endpoint("router2"), PartitionId::new(1), 2, 3).unwrap()],
            )
            .unwrap(),
        )
        .unwrap();
    let error = transport
        .submit(
            &mut scheduler,
            path_route,
            request(42, 0x5010, 1),
            MemoryTrace::new(),
            |_, _| TargetOutcome::NoResponse,
            |_| {},
        )
        .unwrap_err();
    assert_eq!(
        error,
        TransportError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: PartitionId::new(0),
            target: PartitionId::new(1),
            source_tick: 0,
            delivery_tick: 2,
            minimum_delivery_tick: 3,
        })
    );

    let short_response_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core3"),
                PartitionId::new(0),
                endpoint("memory3"),
                PartitionId::new(1),
                3,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let error = transport
        .submit(
            &mut scheduler,
            short_response_route,
            request(43, 0x5018, 1),
            MemoryTrace::new(),
            |delivery, _| {
                TargetOutcome::Respond(MemoryResponse::completed(delivery.request(), None).unwrap())
            },
            |_| {},
        )
        .unwrap_err();
    assert_eq!(
        error,
        TransportError::Scheduler(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source: PartitionId::new(1),
            target: PartitionId::new(0),
            source_tick: 3,
            delivery_tick: 5,
            minimum_delivery_tick: 6,
        })
    );
}
