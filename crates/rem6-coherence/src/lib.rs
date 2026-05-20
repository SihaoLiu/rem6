use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cache::{CacheControllerError, CacheControllerResultKind, MsiCacheController};
use rem6_kernel::{ConservativeRunSummary, PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    Address, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryResponse, ResponseStatus,
};
use rem6_protocol_msi::MsiState;
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport, TargetOutcome,
    TransportEndpointId, TransportError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitKind {
    ImmediateHit,
    ScheduledMiss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitResult {
    kind: SubmitKind,
    cache_result: CacheControllerResultKind,
}

impl SubmitResult {
    fn new(kind: SubmitKind, cache_result: CacheControllerResultKind) -> Self {
        Self { kind, cache_result }
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> CacheControllerResultKind {
        self.cache_result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuResponseRecord {
    tick: u64,
    cache_result: CacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl CpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: CacheControllerResultKind,
        request: MemoryRequestId,
        status: ResponseStatus,
        data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            tick,
            cache_result,
            request,
            status,
            data,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn cache_result(&self) -> CacheControllerResultKind {
        self.cache_result
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HarnessError {
    LineBusy { state: MsiState },
    WrongLine { expected: Address, actual: Address },
    LineDataSizeMismatch { expected: u64, actual: u64 },
    Cache(CacheControllerError),
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Transport(TransportError),
}

impl fmt::Display for HarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineBusy { state } => write!(formatter, "cache line is busy in {state:?}"),
            Self::WrongLine { expected, actual } => write!(
                formatter,
                "request for line {:#x} reached backing line {:#x}",
                actual.get(),
                expected.get()
            ),
            Self::LineDataSizeMismatch { expected, actual } => write!(
                formatter,
                "line data has {actual} bytes but line expects {expected}"
            ),
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for HarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineBackingStore {
    layout: CacheLineLayout,
    line_address: Address,
    data: Vec<u8>,
}

impl LineBackingStore {
    pub fn new(
        layout: CacheLineLayout,
        line_address: Address,
        data: Vec<u8>,
    ) -> Result<Self, HarnessError> {
        let line_address = layout.line_address(line_address);
        if data.len() as u64 != layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: layout.bytes(),
                actual: data.len() as u64,
            });
        }

        Ok(Self {
            layout,
            line_address,
            data,
        })
    }

    pub fn line_address(&self) -> Address {
        self.line_address
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn respond(&mut self, request: &MemoryRequest) -> Result<MemoryResponse, HarnessError> {
        self.check_line(request)?;
        match request.operation() {
            MemoryOperation::ReadShared | MemoryOperation::ReadUnique => {
                MemoryResponse::completed(request, Some(self.data.clone()))
                    .map_err(HarnessError::Memory)
            }
            MemoryOperation::Upgrade => {
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::Write | MemoryOperation::Atomic => {
                self.apply_write(request)?;
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::WritebackClean | MemoryOperation::WritebackDirty => {
                self.replace_line(request)?;
                Ok(MemoryResponse::retry(request))
            }
            _ => MemoryResponse::completed(request, None).map_err(HarnessError::Memory),
        }
    }

    fn check_line(&self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let actual = request.line_address();
        if actual != self.line_address {
            return Err(HarnessError::WrongLine {
                expected: self.line_address,
                actual,
            });
        }

        Ok(())
    }

    fn apply_write(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let offset = request.line_offset() as usize;
        let payload =
            request
                .data()
                .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                    request: request.id(),
                }))?;
        let mask = request.byte_mask();
        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                self.data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn replace_line(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let data = request
            .data()
            .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                request: request.id(),
            }))?;
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = data.to_vec();
        Ok(())
    }
}

pub struct CoherentLineHarness {
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    route: MemoryRouteId,
    cache: Arc<Mutex<MsiCacheController>>,
    backing: Arc<Mutex<LineBackingStore>>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
}

impl CoherentLineHarness {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache_agent: rem6_memory::AgentId,
        layout: CacheLineLayout,
        line_address: Address,
        cache_partition: PartitionId,
        memory_partition: PartitionId,
        cache_endpoint: TransportEndpointId,
        memory_endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
        backing: LineBackingStore,
    ) -> Result<Self, HarnessError> {
        let partitions = cache_partition
            .index()
            .max(memory_partition.index())
            .checked_add(1)
            .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let scheduler = PartitionedScheduler::with_min_remote_delay(partitions, 1)
            .map_err(HarnessError::Scheduler)?;
        let mut transport = MemoryTransport::new();
        let route = transport
            .add_route(
                MemoryRoute::new(
                    cache_endpoint,
                    cache_partition,
                    memory_endpoint,
                    memory_partition,
                    request_latency,
                    response_latency,
                )
                .map_err(HarnessError::Transport)?,
            )
            .map_err(HarnessError::Transport)?;

        Ok(Self {
            scheduler,
            transport,
            route,
            cache: Arc::new(Mutex::new(MsiCacheController::new(
                cache_agent,
                layout,
                line_address,
            ))),
            backing: Arc::new(Mutex::new(backing)),
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn submit_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        let result = self
            .cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
            .map_err(map_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(self.scheduler.now(), cache_result, response);
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let backing = Arc::clone(&self.backing);
        let cache = Arc::clone(&self.cache);
        let responses = Arc::clone(&self.cpu_responses);
        self.transport
            .submit(
                &mut self.scheduler,
                self.route,
                downstream,
                self.trace.clone(),
                move |delivery| {
                    let response = backing
                        .lock()
                        .expect("backing lock")
                        .respond(delivery.request())
                        .expect("backing store response");
                    TargetOutcome::Respond(response)
                },
                move |delivery| {
                    let result = cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(delivery.response().clone())
                        .expect("cache fill");
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        responses
                            .lock()
                            .expect("response lock")
                            .push(response_record(delivery.tick(), result.kind(), response));
                    }
                },
            )
            .map_err(HarnessError::Transport)?;

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result))
    }

    pub fn run_until_idle(&mut self) -> ConservativeRunSummary {
        self.scheduler.run_until_idle_conservative()
    }

    pub fn cache_state(&self) -> MsiState {
        self.cache.lock().expect("cache lock").state()
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.snapshot()
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.lock().expect("response lock").clone()
    }

    pub fn backing_data(&self) -> Vec<u8> {
        self.backing.lock().expect("backing lock").data().to_vec()
    }

    fn record_cpu_response(
        &self,
        tick: u64,
        cache_result: CacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .lock()
            .expect("response lock")
            .push(response_record(tick, cache_result, response));
    }
}

fn response_record(
    tick: u64,
    cache_result: CacheControllerResultKind,
    response: &MemoryResponse,
) -> CpuResponseRecord {
    CpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_cache_error(error: CacheControllerError) -> HarnessError {
    match error {
        CacheControllerError::LineBusy { state } => HarnessError::LineBusy { state },
        error => HarnessError::Cache(error),
    }
}
