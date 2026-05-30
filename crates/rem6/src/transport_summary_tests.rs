use super::*;

fn endpoint(value: &str) -> TransportEndpointId {
    TransportEndpointId::new(value).unwrap()
}

#[test]
fn memory_transport_summary_counts_intermediate_route_response_arrivals() {
    let route = MemoryRouteId::new(7);
    let request = MemoryRequestId::new(AgentId::new(3), 11);
    let trace = MemoryTrace::from_events(vec![
        MemoryTraceEvent::request(
            0,
            route,
            endpoint("cpu0.ifetch"),
            MemoryTraceKind::RequestSent,
            request,
        ),
        MemoryTraceEvent::request(
            2,
            route,
            endpoint("noc.router0"),
            MemoryTraceKind::RequestArrived,
            request,
        ),
        MemoryTraceEvent::request(
            7,
            route,
            endpoint("memory.port0"),
            MemoryTraceKind::RequestArrived,
            request,
        ),
        MemoryTraceEvent::response(
            14,
            route,
            endpoint("noc.router0"),
            request,
            rem6_memory::ResponseStatus::Completed,
        ),
        MemoryTraceEvent::response(
            17,
            route,
            endpoint("cpu0.ifetch"),
            request,
            rem6_memory::ResponseStatus::Completed,
        ),
    ]);

    let summary = memory_transport_summary(&trace);

    assert_eq!(summary.counters.requests, 1);
    assert_eq!(summary.counters.request_arrivals, 2);
    assert_eq!(summary.counters.response_arrivals, 2);
    assert_eq!(summary.counters.responses, 1);
    assert_eq!(summary.counters.round_trip_ticks, 17);
    assert_eq!(summary.routes.len(), 1);
    let route = &summary.routes[0];
    assert_eq!(route.source, "cpu0.ifetch");
    assert_eq!(route.counters.requests, 1);
    assert_eq!(route.counters.request_arrivals, 2);
    assert_eq!(route.counters.response_arrivals, 2);
    assert_eq!(route.counters.responses, 1);
    assert_eq!(route.counters.round_trip_ticks, 17);
}
