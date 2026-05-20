use std::sync::{Arc, Mutex};

use rem6_fabric::{FabricLinkId, FabricModel, FabricPath, FabricPathHop};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind,
    MemoryTransport, RequestDelivery, ResponseDelivery, TargetOutcome, TransportEndpointId,
    TransportError, TransportLatency,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn fabric_link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn fabric_path(name: &str, latency: u64, bandwidth_bytes_per_tick: u64) -> FabricPath {
    FabricPath::new([
        FabricPathHop::new(fabric_link(name), latency, bandwidth_bytes_per_tick).unwrap(),
    ])
    .unwrap()
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
fn transport_reserves_shared_fabric_links_for_request_hops() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::with_fabric(FabricModel::new());
    let trace = MemoryTrace::new();

    let memory = endpoint("memory0");
    let link = fabric_path("mesh_x0", 2, 8);
    let route_a = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("core0"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(memory.clone(), PartitionId::new(1), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(link.clone()),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let route_b = transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("core1"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(memory.clone(), PartitionId::new(1), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(link),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let req_a = request(50, 0x6000, 16);
    let req_b = request(51, 0x7000, 16);
    let deliveries = Arc::new(Mutex::new(Vec::new()));

    let delivered_a = Arc::clone(&deliveries);
    transport
        .submit(
            &mut scheduler,
            route_a,
            req_a.clone(),
            trace.clone(),
            move |delivery, _context| {
                delivered_a.lock().unwrap().push((
                    delivery.route(),
                    delivery.tick(),
                    delivery.request().id(),
                ));
                TargetOutcome::NoResponse
            },
            |_| panic!("request-only transfer must not deliver a response"),
        )
        .unwrap();
    let delivered_b = Arc::clone(&deliveries);
    transport
        .submit(
            &mut scheduler,
            route_b,
            req_b.clone(),
            trace.clone(),
            move |delivery, _context| {
                delivered_b.lock().unwrap().push((
                    delivery.route(),
                    delivery.tick(),
                    delivery.request().id(),
                ));
                TargetOutcome::NoResponse
            },
            |_| panic!("request-only transfer must not deliver a response"),
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 4);
    assert_eq!(
        *deliveries.lock().unwrap(),
        vec![(route_a, 4, req_a.id()), (route_b, 6, req_b.id()),]
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route_a,
                endpoint("core0"),
                MemoryTraceKind::RequestSent,
                req_a.id(),
            ),
            MemoryTraceEvent::request(
                0,
                route_b,
                endpoint("core1"),
                MemoryTraceKind::RequestSent,
                req_b.id(),
            ),
            MemoryTraceEvent::request(
                4,
                route_a,
                memory.clone(),
                MemoryTraceKind::RequestArrived,
                req_a.id(),
            ),
            MemoryTraceEvent::request(
                6,
                route_b,
                memory,
                MemoryTraceKind::RequestArrived,
                req_b.id(),
            ),
        ]
    );
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
            move |delivery: RequestDelivery, _context| {
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
fn transport_routes_request_and_response_across_path_hops() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));

    let core = endpoint("core0");
    let router = endpoint("mesh_r0");
    let memory = endpoint("memory0");
    let route_spec = MemoryRoute::new_path(
        core.clone(),
        PartitionId::new(0),
        [
            MemoryRouteHop::new(router.clone(), PartitionId::new(1), 2, 3).unwrap(),
            MemoryRouteHop::new(memory.clone(), PartitionId::new(2), 5, 7).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(route_spec.source(), &core);
    assert_eq!(route_spec.target(), &memory);
    assert_eq!(route_spec.request_latency(), 7);
    assert_eq!(route_spec.response_latency(), 10);
    assert_eq!(route_spec.hops().len(), 2);

    let route = transport.add_route(route_spec).unwrap();
    let req = request(11, 0x1800, 4);
    let response_log = Arc::clone(&responses);
    let expected_memory = memory.clone();
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery: RequestDelivery, _context| {
                assert_eq!(delivery.tick(), 7);
                assert_eq!(delivery.endpoint(), &expected_memory);
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(vec![0x10, 0x20, 0x30, 0x40]),
                    )
                    .unwrap(),
                )
            },
            move |delivery: ResponseDelivery| {
                response_log
                    .lock()
                    .unwrap()
                    .push((delivery.tick(), delivery.endpoint().clone()));
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 5);
    assert_eq!(summary.final_tick(), 18);
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
            MemoryTraceEvent::request(
                2,
                route,
                router.clone(),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
            MemoryTraceEvent::request(7, route, memory, MemoryTraceKind::RequestArrived, req.id()),
            MemoryTraceEvent::response(14, route, router, req.id(), ResponseStatus::Completed),
            MemoryTraceEvent::response(
                17,
                route,
                core.clone(),
                req.id(),
                ResponseStatus::Completed
            ),
        ]
    );

    assert_eq!(*responses.lock().unwrap(), vec![(17, core)]);
}

#[test]
fn transport_path_allows_local_hops_below_remote_lookahead() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 4).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));

    let core = endpoint("core0");
    let crossbar = endpoint("xbar0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new_path(
                core.clone(),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(crossbar.clone(), PartitionId::new(0), 1, 1).unwrap(),
                    MemoryRouteHop::new(memory.clone(), PartitionId::new(1), 4, 5).unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap();

    let req = request(12, 0x1900, 4);
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            |delivery, _context| {
                assert_eq!(delivery.tick(), 5);
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(vec![1, 3, 5, 7])).unwrap(),
                )
            },
            move |delivery| {
                response_log
                    .lock()
                    .unwrap()
                    .push((delivery.tick(), delivery.endpoint().clone()));
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

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
            MemoryTraceEvent::request(
                1,
                route,
                crossbar.clone(),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
            MemoryTraceEvent::request(5, route, memory, MemoryTraceKind::RequestArrived, req.id()),
            MemoryTraceEvent::response(10, route, crossbar, req.id(), ResponseStatus::Completed),
            MemoryTraceEvent::response(
                11,
                route,
                core.clone(),
                req.id(),
                ResponseStatus::Completed
            ),
        ]
    );
    assert_eq!(*responses.lock().unwrap(), vec![(11, core)]);
}

#[test]
fn transport_path_omits_response_hops_for_no_response_transactions() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();

    let cache = endpoint("l1d0");
    let router = endpoint("mesh_r0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new_path(
                cache.clone(),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(router.clone(), PartitionId::new(1), 2, 3).unwrap(),
                    MemoryRouteHop::new(memory.clone(), PartitionId::new(2), 4, 5).unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let req = MemoryRequest::writeback_dirty(
        MemoryRequestId::new(AgentId::new(7), 13),
        Address::new(0x2000),
        vec![0xaa; 64],
        line_layout(),
    )
    .unwrap();

    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            |delivery, _context| {
                assert_eq!(delivery.tick(), 6);
                assert!(!delivery.request().requires_response());
                TargetOutcome::NoResponse
            },
            |_| panic!("response sink must not run for no response path transactions"),
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 3);
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(0, route, cache, MemoryTraceKind::RequestSent, req.id()),
            MemoryTraceEvent::request(2, route, router, MemoryTraceKind::RequestArrived, req.id()),
            MemoryTraceEvent::request(6, route, memory, MemoryTraceKind::RequestArrived, req.id()),
        ]
    );
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
            |delivery, _context| {
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
            |delivery, _context| {
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
            |delivery, _context| {
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
fn transport_responder_can_schedule_followup_remote_work() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 1).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let followups = Arc::new(Mutex::new(Vec::new()));
    let responses = Arc::new(Mutex::new(Vec::new()));

    let core = endpoint("core0");
    let directory = endpoint("dir0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                directory.clone(),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(35, 0x4800, 4);

    let followup_log = Arc::clone(&followups);
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, context| {
                assert_eq!(delivery.tick(), 2);
                assert_eq!(context.now(), 2);
                assert_eq!(context.partition(), PartitionId::new(1));
                let request_id = delivery.request().id();
                let followup_log = Arc::clone(&followup_log);
                context
                    .schedule_remote_after(PartitionId::new(2), 4, move |context| {
                        followup_log.lock().unwrap().push((
                            context.partition(),
                            context.now(),
                            request_id,
                        ));
                    })
                    .unwrap();
                TargetOutcome::Respond(
                    MemoryResponse::completed(delivery.request(), Some(vec![1, 2, 3, 4])).unwrap(),
                )
            },
            move |delivery| {
                response_log
                    .lock()
                    .unwrap()
                    .push((delivery.tick(), delivery.response().request_id()));
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.executed_events(), 4);
    assert_eq!(summary.final_tick(), 6);
    assert_eq!(
        *responses.lock().unwrap(),
        vec![(5, MemoryRequestId::new(AgentId::new(1), 35))]
    );
    assert_eq!(
        *followups.lock().unwrap(),
        vec![(
            PartitionId::new(2),
            6,
            MemoryRequestId::new(AgentId::new(1), 35),
        )]
    );
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
            MemoryTraceEvent::request(
                2,
                route,
                directory,
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
            MemoryTraceEvent::response(5, route, core, req.id(), ResponseStatus::Completed),
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
        TransportError::LatencyBelowLookahead {
            route: short_route,
            latency: TransportLatency::Request,
            delay: 2,
            minimum: 3,
        }
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
        TransportError::LatencyBelowLookahead {
            route: path_route,
            latency: TransportLatency::Request,
            delay: 2,
            minimum: 3,
        }
    );
}
