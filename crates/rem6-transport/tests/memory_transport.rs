use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    RequestDelivery, ResponseDelivery, TargetOutcome, TransportEndpointId, TransportError,
    TransportLatency,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request(sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(1), sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        line_layout(),
    )
    .unwrap()
}

#[test]
fn transport_routes_request_and_response_across_scheduler_partitions() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));

    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                memory.clone(),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let req = request(10, 0x1000, 4);
    let response_log = Arc::clone(&responses);
    let expected_memory = memory.clone();
    let expected_req = req.clone();
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery: RequestDelivery| {
                assert_eq!(delivery.tick(), 3);
                assert_eq!(delivery.route(), route);
                assert_eq!(delivery.endpoint(), &expected_memory);
                assert_eq!(delivery.request(), &expected_req);
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(vec![0xde, 0xad, 0xbe, 0xef]),
                    )
                    .unwrap(),
                )
            },
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.route(),
                    delivery.endpoint().clone(),
                    delivery.response().clone(),
                ));
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(summary.final_tick(), 8);
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req.id()
            ),
            MemoryTraceEvent::request(3, route, memory, MemoryTraceKind::RequestArrived, req.id()),
            MemoryTraceEvent::response(8, route, core.clone(), req.id(), ResponseStatus::Completed),
        ]
    );

    let received = responses.lock().unwrap();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].0, 8);
    assert_eq!(received[0].1, route);
    assert_eq!(received[0].2, core);
    assert_eq!(received[0].3.data(), Some(&[0xde, 0xad, 0xbe, 0xef][..]));
}

#[test]
fn transport_preserves_scheduler_order_for_same_tick_targets() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 3).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();

    let core = endpoint("core0");
    let cache_a = endpoint("l1a");
    let cache_b = endpoint("l1b");
    let route_a = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                cache_a.clone(),
                PartitionId::new(1),
                4,
                4,
            )
            .unwrap(),
        )
        .unwrap();
    let route_b = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                cache_b.clone(),
                PartitionId::new(2),
                4,
                4,
            )
            .unwrap(),
        )
        .unwrap();

    let req_a = request(20, 0x2000, 1);
    let req_b = request(21, 0x3000, 1);
    transport
        .submit(
            &mut scheduler,
            route_a,
            req_a.clone(),
            trace.clone(),
            |delivery| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(vec![0xa0])).unwrap(),
                )
            },
            |_| {},
        )
        .unwrap();
    transport
        .submit(
            &mut scheduler,
            route_b,
            req_b.clone(),
            trace.clone(),
            |delivery| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(vec![0xb0])).unwrap(),
                )
            },
            |_| {},
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route_a,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req_a.id()
            ),
            MemoryTraceEvent::request(
                0,
                route_b,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req_b.id()
            ),
            MemoryTraceEvent::request(
                4,
                route_a,
                cache_a,
                MemoryTraceKind::RequestArrived,
                req_a.id()
            ),
            MemoryTraceEvent::request(
                4,
                route_b,
                cache_b,
                MemoryTraceKind::RequestArrived,
                req_b.id()
            ),
            MemoryTraceEvent::response(
                8,
                route_a,
                core.clone(),
                req_a.id(),
                ResponseStatus::Completed
            ),
            MemoryTraceEvent::response(8, route_b, core, req_b.id(), ResponseStatus::Completed),
        ]
    );
}

#[test]
fn transport_allows_no_response_transactions() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();

    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                memory.clone(),
                PartitionId::new(1),
                2,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let req = MemoryRequest::writeback_dirty(
        MemoryRequestId::new(AgentId::new(2), 30),
        Address::new(0x4000),
        vec![0x55; 64],
        line_layout(),
    )
    .unwrap();

    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            |delivery| {
                assert!(!delivery.request().requires_response());
                TargetOutcome::NoResponse
            },
            |_| panic!("response sink must not run for no response transactions"),
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 2);
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(0, route, core, MemoryTraceKind::RequestSent, req.id()),
            MemoryTraceEvent::request(2, route, memory, MemoryTraceKind::RequestArrived, req.id()),
        ]
    );
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
            |_| TargetOutcome::NoResponse,
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
            |_| TargetOutcome::NoResponse,
            |_| {},
        )
        .unwrap_err();
    assert_eq!(
        error,
        TransportError::LatencyBelowLookahead {
            route: short_route,
            latency: TransportLatency::Request,
            delay: 2,
            minimum: 3,
        }
    );
}
