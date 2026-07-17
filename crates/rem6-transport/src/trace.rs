use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::Tick;
use rem6_memory::{MemoryRequest, MemoryRequestId, MemoryResponse, ResponseStatus};

use crate::{MemoryRouteId, TransportEndpointId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestDelivery {
    pub(crate) tick: Tick,
    pub(crate) route: MemoryRouteId,
    pub(crate) endpoint: TransportEndpointId,
    pub(crate) request: MemoryRequest,
}

impl RequestDelivery {
    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn request(&self) -> &MemoryRequest {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseDelivery {
    pub(crate) tick: Tick,
    pub(crate) route: MemoryRouteId,
    pub(crate) endpoint: TransportEndpointId,
    pub(crate) response: MemoryResponse,
}

impl ResponseDelivery {
    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn response(&self) -> &MemoryResponse {
        &self.response
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TargetOutcome {
    Respond(MemoryResponse),
    RespondAfter {
        delay: Tick,
        response: MemoryResponse,
    },
    NoResponse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetBatchOutcome {
    request: MemoryRequestId,
    outcome: TargetOutcome,
}

impl TargetBatchOutcome {
    pub const fn new(request: MemoryRequestId, outcome: TargetOutcome) -> Self {
        Self { request, outcome }
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn outcome(&self) -> &TargetOutcome {
        &self.outcome
    }

    pub fn into_outcome(self) -> TargetOutcome {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTraceKind {
    RequestSent,
    RequestArrived,
    ResponseArrived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTraceEvent {
    tick: Tick,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    kind: MemoryTraceKind,
    request: MemoryRequestId,
    response_status: Option<ResponseStatus>,
}

impl MemoryTraceEvent {
    pub fn request(
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        kind: MemoryTraceKind,
        request: MemoryRequestId,
    ) -> Self {
        Self {
            tick,
            route,
            endpoint,
            kind,
            request,
            response_status: None,
        }
    }

    pub fn response(
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        request: MemoryRequestId,
        response_status: ResponseStatus,
    ) -> Self {
        Self {
            tick,
            route,
            endpoint,
            kind: MemoryTraceKind::ResponseArrived,
            request,
            response_status: Some(response_status),
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn kind(&self) -> MemoryTraceKind {
        self.kind
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.request
    }

    pub fn response_status(&self) -> Option<ResponseStatus> {
        self.response_status
    }
}

#[derive(Clone, Default)]
pub struct MemoryTrace {
    events: Arc<Mutex<Vec<MemoryTraceEvent>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryTraceSnapshotError;

impl fmt::Display for MemoryTraceSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "memory trace lock poisoned")
    }
}

impl Error for MemoryTraceSnapshotError {}

impl MemoryTrace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_events(events: Vec<MemoryTraceEvent>) -> Self {
        Self {
            events: Arc::new(Mutex::new(events)),
        }
    }

    pub fn record(&self, event: MemoryTraceEvent) {
        self.events.lock().expect("memory trace lock").push(event);
    }

    pub fn snapshot(&self) -> Vec<MemoryTraceEvent> {
        self.try_snapshot().expect("memory trace lock")
    }

    pub fn try_snapshot(&self) -> Result<Vec<MemoryTraceEvent>, MemoryTraceSnapshotError> {
        self.events
            .lock()
            .map(|events| events.clone())
            .map_err(|_| MemoryTraceSnapshotError)
    }

    pub fn len(&self) -> usize {
        self.events.lock().expect("memory trace lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_snapshot_reports_a_poisoned_trace_lock() {
        let trace = MemoryTrace::new();
        let poison = trace.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = poison.events.lock().unwrap();
            panic!("poison memory trace");
        });

        assert!(trace.try_snapshot().is_err());
    }
}
