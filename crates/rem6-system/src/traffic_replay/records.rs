use rem6_kernel::Tick;
use rem6_memory::MemoryRequestId;
use rem6_traffic::{
    TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceCacheEvent,
    TrafficTraceControlFailureRecord, TrafficTraceDiagnosticEvent, TrafficTraceHtmEvent,
    TrafficTraceMemoryFailureRecord, TrafficTraceMemoryWriteCompletionRecord,
    TrafficTraceReplayAction, TrafficTraceTlbEvent,
};

pub(super) fn traffic_trace_replay_batch_is_sideband_only(
    batch: &TrafficControllerEventBatch,
) -> bool {
    batch.events().iter().all(|event| match event {
        TrafficControllerEvent::TraceTlb(_)
        | TrafficControllerEvent::TraceCache(_)
        | TrafficControllerEvent::TraceDiagnostic(_) => true,
        TrafficControllerEvent::TraceHtm(htm) => !htm.requires_response(),
        _ => false,
    })
}

pub(super) fn traffic_trace_replay_batch_has_memory_write_completion(
    batch: &TrafficControllerEventBatch,
    request: MemoryRequestId,
) -> bool {
    batch.events().iter().any(|event| {
        matches!(
            event,
            TrafficControllerEvent::TraceReplayAction(
                TrafficTraceReplayAction::MemoryWriteCompletion {
                    request: action_request,
                    ..
                }
            ) if *action_request == request
        )
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplayScheduledMemoryFailure {
    tick: Tick,
    record: TrafficTraceMemoryFailureRecord,
}

impl TrafficTraceReplayScheduledMemoryFailure {
    pub const fn new(tick: Tick, record: TrafficTraceMemoryFailureRecord) -> Self {
        Self { tick, record }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn record(self) -> TrafficTraceMemoryFailureRecord {
        self.record
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplayScheduledMemoryWriteCompletion {
    tick: Tick,
    record: TrafficTraceMemoryWriteCompletionRecord,
}

impl TrafficTraceReplayScheduledMemoryWriteCompletion {
    pub const fn new(tick: Tick, record: TrafficTraceMemoryWriteCompletionRecord) -> Self {
        Self { tick, record }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn record(self) -> TrafficTraceMemoryWriteCompletionRecord {
        self.record
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplayScheduledControlAck {
    tick: Tick,
    trace_tick: Tick,
}

impl TrafficTraceReplayScheduledControlAck {
    pub const fn new(tick: Tick, trace_tick: Tick) -> Self {
        Self { tick, trace_tick }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(self) -> Tick {
        self.trace_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplayScheduledControlFailure {
    tick: Tick,
    record: TrafficTraceControlFailureRecord,
}

impl TrafficTraceReplayScheduledControlFailure {
    pub const fn new(tick: Tick, record: TrafficTraceControlFailureRecord) -> Self {
        Self { tick, record }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn record(self) -> TrafficTraceControlFailureRecord {
        self.record
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplayScheduledSidebandEvent {
    tick: Tick,
    event: TrafficTraceReplaySidebandEvent,
}

impl TrafficTraceReplayScheduledSidebandEvent {
    pub const fn new(tick: Tick, event: TrafficTraceReplaySidebandEvent) -> Self {
        Self { tick, event }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn event(self) -> TrafficTraceReplaySidebandEvent {
        self.event
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplaySidebandEvent {
    Tlb(TrafficTraceTlbEvent),
    Cache(TrafficTraceCacheEvent),
    Diagnostic(TrafficTraceDiagnosticEvent),
    Htm(TrafficTraceHtmEvent),
}

impl TrafficTraceReplaySidebandEvent {
    pub const fn tick(self) -> Tick {
        match self {
            Self::Tlb(event) => event.tick(),
            Self::Cache(event) => event.tick(),
            Self::Diagnostic(event) => event.tick(),
            Self::Htm(event) => event.tick(),
        }
    }

    pub const fn sequence(self) -> u64 {
        match self {
            Self::Tlb(event) => event.sequence(),
            Self::Cache(event) => event.sequence(),
            Self::Diagnostic(event) => event.sequence(),
            Self::Htm(event) => event.sequence(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplaySidebandCompletion {
    delay: Tick,
    event: TrafficTraceReplaySidebandEvent,
}

impl TrafficTraceReplaySidebandCompletion {
    pub(super) const fn new(delay: Tick, event: TrafficTraceReplaySidebandEvent) -> Self {
        Self { delay, event }
    }

    pub const fn delay(self) -> Tick {
        self.delay
    }

    pub const fn event(self) -> TrafficTraceReplaySidebandEvent {
        self.event
    }
}
