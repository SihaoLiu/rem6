use std::error::Error;
use std::fmt;

use rem6_coherence::{ChiHarnessError, HarnessError, MesiHarnessError, MoesiHarnessError};
use rem6_kernel::Tick;
use rem6_memory::{
    Address, MemoryError, MemoryOperation, MemoryRequest, MemoryRequestId, MemoryTargetId,
};
use rem6_traffic::{
    TrafficTraceCacheEvent, TrafficTraceCacheKind, TrafficTraceResponseEvent,
    TrafficTraceResponseKind, TrafficTraceSyncEvent, TrafficTraceSyncKind,
};

use crate::RiscvDataCacheProtocol;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvDataCacheControllerError {
    Msi(HarnessError),
    Mesi(MesiHarnessError),
    Moesi(MoesiHarnessError),
    Chi(ChiHarnessError),
    MissingResponse { request: MemoryRequestId },
    Memory(MemoryError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvDataCacheControllerErrorSource {
    Request {
        request_id: MemoryRequestId,
    },
    TraceCache {
        sequence: u64,
        kind: TrafficTraceCacheKind,
        trace_packet_id: Option<u64>,
    },
    TraceSync {
        sequence: u64,
        kind: TrafficTraceSyncKind,
        kernel_sync: bool,
        invalidates_l1: bool,
        trace_packet_id: Option<u64>,
    },
    TraceResponse {
        sequence: u64,
        kind: TrafficTraceResponseKind,
        trace_packet_id: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataCacheControllerErrorRecord {
    tick: Tick,
    source: RiscvDataCacheControllerErrorSource,
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    address: Address,
    line: Address,
    operation: MemoryOperation,
    error: RiscvDataCacheControllerError,
}

impl RiscvDataCacheControllerError {
    pub const fn missing_response(request: MemoryRequestId) -> Self {
        Self::MissingResponse { request }
    }
}

impl fmt::Display for RiscvDataCacheControllerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Msi(error) => write!(formatter, "{error}"),
            Self::Mesi(error) => write!(formatter, "{error}"),
            Self::Moesi(error) => write!(formatter, "{error}"),
            Self::Chi(error) => write!(formatter, "{error}"),
            Self::MissingResponse { request } => write!(
                formatter,
                "data-cache controller did not record response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvDataCacheControllerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Msi(error) => Some(error),
            Self::Mesi(error) => Some(error),
            Self::Moesi(error) => Some(error),
            Self::Chi(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::MissingResponse { .. } => None,
        }
    }
}

impl From<HarnessError> for RiscvDataCacheControllerError {
    fn from(error: HarnessError) -> Self {
        Self::Msi(error)
    }
}

impl From<MesiHarnessError> for RiscvDataCacheControllerError {
    fn from(error: MesiHarnessError) -> Self {
        Self::Mesi(error)
    }
}

impl From<MoesiHarnessError> for RiscvDataCacheControllerError {
    fn from(error: MoesiHarnessError) -> Self {
        Self::Moesi(error)
    }
}

impl From<ChiHarnessError> for RiscvDataCacheControllerError {
    fn from(error: ChiHarnessError) -> Self {
        Self::Chi(error)
    }
}

impl From<MemoryError> for RiscvDataCacheControllerError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl RiscvDataCacheControllerErrorRecord {
    pub(crate) fn from_request(
        tick: Tick,
        request: &MemoryRequest,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        error: RiscvDataCacheControllerError,
    ) -> Self {
        Self {
            tick,
            source: RiscvDataCacheControllerErrorSource::Request {
                request_id: request.id(),
            },
            protocol,
            target,
            address: request.range().start(),
            line: request.line_address(),
            operation: request.operation(),
            error,
        }
    }

    pub(crate) fn from_trace_cache_event(
        tick: Tick,
        event: TrafficTraceCacheEvent,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        line: Address,
        error: RiscvDataCacheControllerError,
    ) -> Self {
        Self {
            tick,
            source: RiscvDataCacheControllerErrorSource::TraceCache {
                sequence: event.sequence(),
                kind: event.kind(),
                trace_packet_id: event.trace_packet_id(),
            },
            protocol,
            target,
            address: event.address(),
            line,
            operation: MemoryOperation::Invalidate,
            error,
        }
    }

    pub(crate) fn from_trace_response_event(
        tick: Tick,
        event: TrafficTraceResponseEvent,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        line: Address,
        error: RiscvDataCacheControllerError,
    ) -> Self {
        Self {
            tick,
            source: RiscvDataCacheControllerErrorSource::TraceResponse {
                sequence: event.sequence(),
                kind: event.kind(),
                trace_packet_id: event.trace_packet_id(),
            },
            protocol,
            target,
            address: event.address().unwrap_or(line),
            line,
            operation: trace_response_operation(event),
            error,
        }
    }

    pub(crate) fn from_trace_sync_event(
        tick: Tick,
        event: TrafficTraceSyncEvent,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        line: Address,
        error: RiscvDataCacheControllerError,
    ) -> Self {
        Self {
            tick,
            source: RiscvDataCacheControllerErrorSource::TraceSync {
                sequence: event.sequence(),
                kind: event.kind(),
                kernel_sync: event.kernel_sync(),
                invalidates_l1: event.invalidates_l1(),
                trace_packet_id: event.trace_packet_id(),
            },
            protocol,
            target,
            address: line,
            line,
            operation: MemoryOperation::Invalidate,
            error,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source(&self) -> RiscvDataCacheControllerErrorSource {
        self.source
    }

    pub const fn request_id(&self) -> Option<MemoryRequestId> {
        match self.source {
            RiscvDataCacheControllerErrorSource::Request { request_id } => Some(request_id),
            RiscvDataCacheControllerErrorSource::TraceCache { .. }
            | RiscvDataCacheControllerErrorSource::TraceSync { .. }
            | RiscvDataCacheControllerErrorSource::TraceResponse { .. } => None,
        }
    }

    pub const fn trace_sequence(&self) -> Option<u64> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceCache { sequence, .. }
            | RiscvDataCacheControllerErrorSource::TraceSync { sequence, .. }
            | RiscvDataCacheControllerErrorSource::TraceResponse { sequence, .. } => Some(sequence),
            RiscvDataCacheControllerErrorSource::Request { .. } => None,
        }
    }

    pub const fn trace_cache_kind(&self) -> Option<TrafficTraceCacheKind> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceCache { kind, .. } => Some(kind),
            RiscvDataCacheControllerErrorSource::Request { .. }
            | RiscvDataCacheControllerErrorSource::TraceSync { .. }
            | RiscvDataCacheControllerErrorSource::TraceResponse { .. } => None,
        }
    }

    pub const fn trace_response_kind(&self) -> Option<TrafficTraceResponseKind> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceResponse { kind, .. } => Some(kind),
            RiscvDataCacheControllerErrorSource::Request { .. }
            | RiscvDataCacheControllerErrorSource::TraceSync { .. }
            | RiscvDataCacheControllerErrorSource::TraceCache { .. } => None,
        }
    }

    pub const fn trace_sync_kind(&self) -> Option<TrafficTraceSyncKind> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceSync { kind, .. } => Some(kind),
            RiscvDataCacheControllerErrorSource::Request { .. }
            | RiscvDataCacheControllerErrorSource::TraceCache { .. }
            | RiscvDataCacheControllerErrorSource::TraceResponse { .. } => None,
        }
    }

    pub const fn trace_sync_kernel_sync(&self) -> Option<bool> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceSync { kernel_sync, .. } => Some(kernel_sync),
            RiscvDataCacheControllerErrorSource::Request { .. }
            | RiscvDataCacheControllerErrorSource::TraceCache { .. }
            | RiscvDataCacheControllerErrorSource::TraceResponse { .. } => None,
        }
    }

    pub const fn trace_sync_invalidates_l1(&self) -> Option<bool> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceSync { invalidates_l1, .. } => {
                Some(invalidates_l1)
            }
            RiscvDataCacheControllerErrorSource::Request { .. }
            | RiscvDataCacheControllerErrorSource::TraceCache { .. }
            | RiscvDataCacheControllerErrorSource::TraceResponse { .. } => None,
        }
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        match self.source {
            RiscvDataCacheControllerErrorSource::TraceCache {
                trace_packet_id, ..
            }
            | RiscvDataCacheControllerErrorSource::TraceSync {
                trace_packet_id, ..
            }
            | RiscvDataCacheControllerErrorSource::TraceResponse {
                trace_packet_id, ..
            } => trace_packet_id,
            RiscvDataCacheControllerErrorSource::Request { .. } => None,
        }
    }

    pub const fn protocol(&self) -> RiscvDataCacheProtocol {
        self.protocol
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn operation(&self) -> MemoryOperation {
        self.operation
    }

    pub const fn error(&self) -> &RiscvDataCacheControllerError {
        &self.error
    }
}

impl fmt::Display for RiscvDataCacheControllerErrorRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.source {
            RiscvDataCacheControllerErrorSource::Request { request_id } => write!(
                formatter,
                "data-cache controller {:?} failed request {} from agent {} at tick {} for address {:#x}: {}",
                self.protocol,
                request_id.sequence(),
                request_id.agent().get(),
                self.tick,
                self.address.get(),
                self.error
            ),
            RiscvDataCacheControllerErrorSource::TraceCache { sequence, kind, .. } => write!(
                formatter,
                "data-cache controller {:?} failed trace cache {:?} sequence {} at tick {} for address {:#x}: {}",
                self.protocol,
                kind,
                sequence,
                self.tick,
                self.address.get(),
                self.error
            ),
            RiscvDataCacheControllerErrorSource::TraceSync { sequence, kind, .. } => write!(
                formatter,
                "data-cache controller {:?} failed trace sync {:?} sequence {} at tick {} for address {:#x}: {}",
                self.protocol,
                kind,
                sequence,
                self.tick,
                self.address.get(),
                self.error
            ),
            RiscvDataCacheControllerErrorSource::TraceResponse { sequence, kind, .. } => write!(
                formatter,
                "data-cache controller {:?} failed trace response {:?} sequence {} at tick {} for address {:#x}: {}",
                self.protocol,
                kind,
                sequence,
                self.tick,
                self.address.get(),
                self.error
            ),
        }
    }
}

const fn trace_response_operation(event: TrafficTraceResponseEvent) -> MemoryOperation {
    match event.kind() {
        TrafficTraceResponseKind::CleanShared => MemoryOperation::CleanShared,
        TrafficTraceResponseKind::ReadWithInvalidate
        | TrafficTraceResponseKind::CleanInvalid
        | TrafficTraceResponseKind::Invalidate => MemoryOperation::Invalidate,
        _ => MemoryOperation::Invalidate,
    }
}
