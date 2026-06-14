use std::collections::BTreeMap;

use rem6_memory::MemoryRequestId;
use rem6_transport::{MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6MemoryTransportSummary {
    pub(crate) counters: Rem6MemoryTransportCounters,
    pub(crate) routes: Vec<Rem6MemoryTransportRouteSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6MemoryTransportRouteSummary {
    pub(crate) route: MemoryRouteId,
    pub(crate) source: String,
    pub(crate) counters: Rem6MemoryTransportCounters,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6MemoryTransportCounters {
    pub(crate) requests: u64,
    pub(crate) request_arrivals: u64,
    pub(crate) responses: u64,
    pub(crate) response_arrivals: u64,
    pub(crate) round_trip_ticks: u64,
    pub(crate) max_round_trip_ticks: u64,
}

pub(crate) fn memory_transport_summary(trace: &MemoryTrace) -> Rem6MemoryTransportSummary {
    let events = trace.snapshot();
    let mut request_sources: BTreeMap<(MemoryRouteId, MemoryRequestId), (u64, String)> =
        BTreeMap::new();
    let mut routes: BTreeMap<(MemoryRouteId, String), Rem6MemoryTransportRouteSummary> =
        BTreeMap::new();
    let mut summary = Rem6MemoryTransportSummary {
        counters: Rem6MemoryTransportCounters::default(),
        routes: Vec::new(),
    };

    for event in &events {
        match event.kind() {
            MemoryTraceKind::RequestSent => {
                summary.counters.requests += 1;
                let source = event.endpoint().as_str().to_string();
                route_summary(&mut routes, event.route(), &source)
                    .counters
                    .requests += 1;
                request_sources.insert(trace_key(event), (event.tick(), source));
            }
            MemoryTraceKind::RequestArrived => {
                summary.counters.request_arrivals += 1;
                if let Some((_, source)) = request_sources.get(&trace_key(event)) {
                    route_summary(&mut routes, event.route(), source)
                        .counters
                        .request_arrivals += 1;
                }
            }
            MemoryTraceKind::ResponseArrived => {
                summary.counters.response_arrivals += 1;
                let Some((sent_tick, source)) = request_sources.get(&trace_key(event)) else {
                    continue;
                };
                let route = route_summary(&mut routes, event.route(), source);
                route.counters.response_arrivals += 1;
                if event.endpoint().as_str() != source {
                    continue;
                }
                let latency = event.tick().saturating_sub(*sent_tick);
                summary.counters.responses += 1;
                summary.counters.round_trip_ticks =
                    summary.counters.round_trip_ticks.saturating_add(latency);
                summary.counters.max_round_trip_ticks =
                    summary.counters.max_round_trip_ticks.max(latency);
                route.counters.responses += 1;
                route.counters.round_trip_ticks =
                    route.counters.round_trip_ticks.saturating_add(latency);
                route.counters.max_round_trip_ticks =
                    route.counters.max_round_trip_ticks.max(latency);
            }
        }
    }

    summary.routes = routes.into_values().collect();
    summary
}

fn route_summary<'a>(
    routes: &'a mut BTreeMap<(MemoryRouteId, String), Rem6MemoryTransportRouteSummary>,
    route: MemoryRouteId,
    source: &str,
) -> &'a mut Rem6MemoryTransportRouteSummary {
    routes
        .entry((route, source.to_string()))
        .or_insert_with(|| Rem6MemoryTransportRouteSummary {
            route,
            source: source.to_string(),
            counters: Rem6MemoryTransportCounters::default(),
        })
}

fn trace_key(event: &MemoryTraceEvent) -> (MemoryRouteId, MemoryRequestId) {
    (event.route(), event.request_id())
}
