use std::collections::{BTreeMap, BTreeSet};

use rem6_memory::{MemoryRequestId, ResponseStatus};
use rem6_transport::{MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind};

use crate::formatting::json_escape;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6MemoryTraceRecord {
    pub(crate) channel: &'static str,
    pub(crate) tick: u64,
    pub(crate) kind: &'static str,
    pub(crate) route: u64,
    endpoint: String,
    pub(crate) request_agent: u32,
    pub(crate) request: u64,
    pub(crate) response_status: Option<&'static str>,
    pub(crate) response_latency_ticks: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6MemoryTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MemoryTraceStatSummary {
    records: u64,
    requests: BTreeSet<(u32, u64)>,
    routes: BTreeSet<u64>,
    request_agents: BTreeSet<u32>,
    events: BTreeMap<&'static str, u64>,
    response_status: BTreeMap<&'static str, u64>,
    response_latency_ticks: u64,
    max_response_latency_ticks: u64,
}

impl Rem6MemoryTraceRecord {
    pub(crate) fn to_json(&self) -> String {
        let response_status = self
            .response_status
            .map(|status| format!("\"{status}\""))
            .unwrap_or_else(|| "null".to_string());
        let response_latency_ticks = self
            .response_latency_ticks
            .map(|ticks| ticks.to_string())
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"channel\":\"{}\",\"tick\":{},\"kind\":\"{}\",\"route\":{},\"endpoint\":\"{}\",\"request_agent\":{},\"request\":{},\"response_status\":{},\"response_latency_ticks\":{}}}",
            self.channel,
            self.tick,
            self.kind,
            self.route,
            json_escape(&self.endpoint),
            self.request_agent,
            self.request,
            response_status,
            response_latency_ticks,
        )
    }
}

impl Rem6MemoryTraceStat {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn unit(&self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

impl MemoryTraceStatSummary {
    fn add_record(&mut self, record: &Rem6MemoryTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.requests.insert((record.request_agent, record.request));
        self.routes.insert(record.route);
        self.request_agents.insert(record.request_agent);
        self.events
            .entry(record.kind)
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        if let Some(status) = record.response_status {
            self.response_status
                .entry(status)
                .and_modify(|count| *count = count.saturating_add(1))
                .or_insert(1);
        }
        if let Some(latency_ticks) = record.response_latency_ticks {
            self.response_latency_ticks = self.response_latency_ticks.saturating_add(latency_ticks);
            self.max_response_latency_ticks = self.max_response_latency_ticks.max(latency_ticks);
        }
    }

    fn push_stats(&self, stats: &mut Vec<Rem6MemoryTraceStat>, prefix: &str) {
        push_memory_trace_count_stats(
            stats,
            prefix,
            &[
                ("records", self.records),
                ("requests", self.requests.len() as u64),
                ("routes", self.routes.len() as u64),
                ("request_agents", self.request_agents.len() as u64),
            ],
        );
        push_memory_trace_stats(
            stats,
            prefix,
            &[
                (
                    "response_latency_ticks",
                    "Tick",
                    self.response_latency_ticks,
                ),
                (
                    "max_response_latency_ticks",
                    "Tick",
                    self.max_response_latency_ticks,
                ),
            ],
        );
        for (kind, value) in &self.events {
            stats.push(Rem6MemoryTraceStat {
                path: format!("{prefix}.events.{kind}"),
                unit: "Count",
                value: *value,
            });
        }
        for (status, value) in &self.response_status {
            stats.push(Rem6MemoryTraceStat {
                path: format!("{prefix}.response_status.{status}"),
                unit: "Count",
                value: *value,
            });
        }
    }
}

pub(crate) fn memory_trace_channel_matches(
    record: &Rem6MemoryTraceRecord,
    channel: Option<&str>,
) -> bool {
    channel.map_or(true, |expected| record.channel == expected)
}

pub(crate) fn memory_trace_stats(
    records: &[Rem6MemoryTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6MemoryTraceStat> {
    let mut channels = BTreeMap::<String, MemoryTraceStatSummary>::new();
    let mut routes = BTreeMap::<(String, u64, String), MemoryTraceStatSummary>::new();
    let mut request_agents = BTreeMap::<(String, u32), MemoryTraceStatSummary>::new();
    for record in records {
        let channel = record.channel.to_string();
        channels
            .entry(channel.clone())
            .or_default()
            .add_record(record);
        routes
            .entry((channel.clone(), record.route, record.endpoint.clone()))
            .or_default()
            .add_record(record);
        request_agents
            .entry((channel, record.request_agent))
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (channel, summary) in channels {
        let prefix = format!("channel.{}", stat_path_segment(&channel));
        summary.push_stats(&mut stats, &prefix);
    }
    for ((channel, route, endpoint), summary) in routes {
        let prefix = format!(
            "channel.{}.route{route}.endpoint.{}",
            stat_path_segment(&channel),
            stat_path_segment(&endpoint)
        );
        summary.push_stats(&mut stats, &prefix);
    }
    for ((channel, request_agent), summary) in request_agents {
        let prefix = format!(
            "channel.{}.request_agent.agent{request_agent}",
            stat_path_segment(&channel)
        );
        summary.push_stats(&mut stats, &prefix);
    }
    stats
}

fn push_memory_trace_stats(
    stats: &mut Vec<Rem6MemoryTraceStat>,
    prefix: &str,
    entries: &[(&'static str, &'static str, u64)],
) {
    for (suffix, unit, value) in entries {
        stats.push(Rem6MemoryTraceStat {
            path: format!("{prefix}.{suffix}"),
            unit,
            value: *value,
        });
    }
}

fn push_memory_trace_count_stats(
    stats: &mut Vec<Rem6MemoryTraceStat>,
    prefix: &str,
    entries: &[(&'static str, u64)],
) {
    for (suffix, value) in entries {
        stats.push(Rem6MemoryTraceStat {
            path: format!("{prefix}.{suffix}"),
            unit: "Count",
            value: *value,
        });
    }
}

pub(crate) fn memory_trace_records(
    fetch_memory_trace: &MemoryTrace,
    data_memory_trace: &MemoryTrace,
) -> Vec<Rem6MemoryTraceRecord> {
    let mut records = Vec::new();
    records.extend(memory_trace_channel_records("fetch", fetch_memory_trace));
    records.extend(memory_trace_channel_records("data", data_memory_trace));
    records.sort_by_key(|record| {
        (
            record.tick,
            record.channel,
            record.route,
            record.request_agent,
            record.request,
            record.kind,
        )
    });
    records
}

fn memory_trace_channel_records(
    channel: &'static str,
    trace: &MemoryTrace,
) -> Vec<Rem6MemoryTraceRecord> {
    let mut request_sent_ticks = BTreeMap::<(MemoryRouteId, MemoryRequestId), u64>::new();
    trace
        .snapshot()
        .into_iter()
        .map(|event| {
            let latency = memory_trace_response_latency(&mut request_sent_ticks, &event);
            memory_trace_record(channel, event, latency)
        })
        .collect()
}

fn memory_trace_response_latency(
    request_sent_ticks: &mut BTreeMap<(MemoryRouteId, MemoryRequestId), u64>,
    event: &MemoryTraceEvent,
) -> Option<u64> {
    let key = (event.route(), event.request_id());
    match event.kind() {
        MemoryTraceKind::RequestSent => {
            request_sent_ticks.insert(key, event.tick());
            None
        }
        MemoryTraceKind::RequestArrived => None,
        MemoryTraceKind::ResponseArrived => request_sent_ticks
            .get(&key)
            .map(|sent_tick| event.tick().saturating_sub(*sent_tick)),
    }
}

fn memory_trace_record(
    channel: &'static str,
    event: MemoryTraceEvent,
    response_latency_ticks: Option<u64>,
) -> Rem6MemoryTraceRecord {
    let request = event.request_id();
    Rem6MemoryTraceRecord {
        channel,
        tick: event.tick(),
        kind: memory_trace_kind(event.kind()),
        route: event.route().get(),
        endpoint: event.endpoint().as_str().to_string(),
        request_agent: request.agent().get(),
        request: request.sequence(),
        response_status: event.response_status().map(response_status_name),
        response_latency_ticks,
    }
}

const fn memory_trace_kind(kind: MemoryTraceKind) -> &'static str {
    match kind {
        MemoryTraceKind::RequestSent => "request_sent",
        MemoryTraceKind::RequestArrived => "request_arrived",
        MemoryTraceKind::ResponseArrived => "response_arrived",
    }
}

const fn response_status_name(status: ResponseStatus) -> &'static str {
    match status {
        ResponseStatus::Completed => "completed",
        ResponseStatus::Retry => "retry",
        ResponseStatus::StoreConditionalFailed => "store_conditional_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rem6_memory::{AgentId, MemoryRequestId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    fn endpoint(name: &str) -> TransportEndpointId {
        TransportEndpointId::new(name).unwrap()
    }

    #[test]
    fn memory_trace_response_latency_matches_route_and_request() {
        let request = MemoryRequestId::new(AgentId::new(2), 9);
        let route_a = MemoryRouteId::new(1);
        let route_b = MemoryRouteId::new(2);
        let source = endpoint("cpu0.data");
        let fetch_trace = MemoryTrace::from_events(vec![
            MemoryTraceEvent::request(
                1,
                route_a,
                source.clone(),
                MemoryTraceKind::RequestSent,
                request,
            ),
            MemoryTraceEvent::request(
                5,
                route_b,
                source.clone(),
                MemoryTraceKind::RequestSent,
                request,
            ),
            MemoryTraceEvent::response(
                8,
                route_b,
                source.clone(),
                request,
                ResponseStatus::Completed,
            ),
            MemoryTraceEvent::response(10, route_a, source, request, ResponseStatus::Completed),
        ]);
        let data_trace = MemoryTrace::new();

        let records = memory_trace_records(&fetch_trace, &data_trace);
        let latencies = records
            .iter()
            .filter(|record| record.kind == "response_arrived")
            .map(|record| (record.route, record.response_latency_ticks.unwrap()))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(latencies.get(&route_a.get()), Some(&9));
        assert_eq!(latencies.get(&route_b.get()), Some(&3));
    }
}
