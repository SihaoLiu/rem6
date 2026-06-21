use super::json_escape;

use crate::{
    Rem6MemoryTransportCounters, Rem6MemoryTransportRouteSummary, Rem6MemoryTransportSummary,
};

pub(super) fn empty_transport_json() -> String {
    format!(
        "{{\"fetch\":{},\"data\":{}}}",
        empty_transport_scope_json(),
        empty_transport_scope_json()
    )
}

fn empty_transport_scope_json() -> String {
    "{\"requests\":0,\"request_arrivals\":0,\"responses\":0,\"response_arrivals\":0,\"round_trip_ticks\":0,\"max_round_trip_ticks\":0,\"routes\":[]}".to_string()
}

impl Rem6MemoryTransportSummary {
    pub(crate) fn to_json(&self) -> String {
        let routes = self
            .routes
            .iter()
            .map(Rem6MemoryTransportRouteSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{{},\"routes\":[{}]}}",
            self.counters.json_fields(),
            routes
        )
    }
}

impl Rem6MemoryTransportRouteSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"route\":{},\"source\":\"{}\",{}}}",
            self.route.get(),
            json_escape(&self.source),
            self.counters.json_fields()
        )
    }
}

impl Rem6MemoryTransportCounters {
    fn json_fields(&self) -> String {
        format!(
            "\"requests\":{},\"request_arrivals\":{},\"responses\":{},\"response_arrivals\":{},\"round_trip_ticks\":{},\"max_round_trip_ticks\":{}",
            self.requests,
            self.request_arrivals,
            self.responses,
            self.response_arrivals,
            self.round_trip_ticks,
            self.max_round_trip_ticks,
        )
    }
}
