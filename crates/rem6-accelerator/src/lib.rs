use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError,
    Tick,
};
use rem6_memory::{Address, ByteMask, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, ResponseDelivery, TargetOutcome,
    TransportError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceleratorEngineId(u32);

impl AcceleratorEngineId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceleratorCommandId(u64);

impl AcceleratorCommandId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorCommandKind {
    GpuKernel { workgroups: u32 },
    NpuInference { tiles: u32 },
    DmaCopy { bytes: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCommand {
    id: AcceleratorCommandId,
    kind: AcceleratorCommandKind,
    execution_latency: Tick,
}

impl AcceleratorCommand {
    pub fn new(
        id: AcceleratorCommandId,
        kind: AcceleratorCommandKind,
        execution_latency: Tick,
    ) -> Result<Self, AcceleratorError> {
        if execution_latency == 0 {
            return Err(AcceleratorError::ZeroExecutionLatency { command: id });
        }

        Ok(Self {
            id,
            kind,
            execution_latency,
        })
    }

    pub const fn id(&self) -> AcceleratorCommandId {
        self.id
    }

    pub const fn kind(&self) -> &AcceleratorCommandKind {
        &self.kind
    }

    pub const fn execution_latency(&self) -> Tick {
        self.execution_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorEngineConfig {
    id: AcceleratorEngineId,
    partition: PartitionId,
    lanes: u32,
}

impl AcceleratorEngineConfig {
    pub fn new(
        id: AcceleratorEngineId,
        partition: PartitionId,
        lanes: u32,
    ) -> Result<Self, AcceleratorError> {
        if lanes == 0 {
            return Err(AcceleratorError::ZeroLanes { engine: id });
        }

        Ok(Self {
            id,
            partition,
            lanes,
        })
    }

    pub const fn id(&self) -> AcceleratorEngineId {
        self.id
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn lanes(&self) -> u32 {
        self.lanes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorError {
    ZeroLanes {
        engine: AcceleratorEngineId,
    },
    ZeroExecutionLatency {
        command: AcceleratorCommandId,
    },
    DmaReadRequiresData {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
    TickOverflow {
        now: Tick,
        delay: Tick,
    },
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Transport(TransportError),
}

impl fmt::Display for AcceleratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLanes { engine } => {
                write!(
                    formatter,
                    "accelerator engine {} needs at least one lane",
                    engine.get()
                )
            }
            Self::ZeroExecutionLatency { command } => write!(
                formatter,
                "accelerator command {} needs positive execution latency",
                command.get()
            ),
            Self::DmaReadRequiresData { command, request } => write!(
                formatter,
                "accelerator command {} DMA read request {} from agent {} must return data",
                command.get(),
                request.sequence(),
                request.agent().get(),
            ),
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for AcceleratorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorTraceEvent {
    tick: Tick,
    kind: AcceleratorTraceKind,
}

impl AcceleratorTraceEvent {
    pub const fn new(tick: Tick, kind: AcceleratorTraceKind) -> Self {
        Self { tick, kind }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> &AcceleratorTraceKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorTraceKind {
    Submitted {
        command: AcceleratorCommandId,
        source: PartitionId,
        target: PartitionId,
    },
    Started {
        command: AcceleratorCommandId,
        lane: u32,
        complete_at: Tick,
    },
    Completed {
        command: AcceleratorCommandId,
        lane: u32,
    },
    DmaReadIssued {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
    DmaReadCompleted {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
        bytes: u64,
    },
    DmaWriteIssued {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
    DmaWriteCompleted {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCompletion {
    command: AcceleratorCommandId,
    kind: AcceleratorCommandKind,
    lane: u32,
    started_at: Tick,
    completed_at: Tick,
}

impl AcceleratorCompletion {
    pub const fn new(
        command: AcceleratorCommandId,
        kind: AcceleratorCommandKind,
        lane: u32,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            command,
            kind,
            lane,
            started_at,
            completed_at,
        }
    }

    pub const fn command(&self) -> AcceleratorCommandId {
        self.command
    }

    pub const fn kind(&self) -> &AcceleratorCommandKind {
        &self.kind
    }

    pub const fn lane(&self) -> u32 {
        self.lane
    }

    pub const fn started_at(&self) -> Tick {
        self.started_at
    }

    pub const fn completed_at(&self) -> Tick {
        self.completed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorDmaCopy {
    command: AcceleratorCommandId,
    read_route: MemoryRouteId,
    read_request: MemoryRequest,
    write_route: MemoryRouteId,
    write_request: MemoryRequestId,
    destination: Address,
}

impl AcceleratorDmaCopy {
    pub fn new(
        command: AcceleratorCommandId,
        read_route: MemoryRouteId,
        read_request: MemoryRequest,
        write_route: MemoryRouteId,
        write_request: MemoryRequestId,
        destination: Address,
    ) -> Result<Self, AcceleratorError> {
        if !read_request.returns_data() {
            return Err(AcceleratorError::DmaReadRequiresData {
                command,
                request: read_request.id(),
            });
        }

        Ok(Self {
            command,
            read_route,
            read_request,
            write_route,
            write_request,
            destination,
        })
    }

    pub const fn command(&self) -> AcceleratorCommandId {
        self.command
    }

    pub const fn read_route(&self) -> MemoryRouteId {
        self.read_route
    }

    pub const fn read_request(&self) -> &MemoryRequest {
        &self.read_request
    }

    pub const fn write_route(&self) -> MemoryRouteId {
        self.write_route
    }

    pub const fn write_request(&self) -> MemoryRequestId {
        self.write_request
    }

    pub const fn destination(&self) -> Address {
        self.destination
    }

    fn make_write_request(&self, data: Vec<u8>) -> Result<MemoryRequest, AcceleratorError> {
        MemoryRequest::write(
            self.write_request,
            self.destination,
            self.read_request.size(),
            data,
            ByteMask::full(self.read_request.size()).map_err(AcceleratorError::Memory)?,
            self.read_request.line_layout(),
        )
        .map_err(AcceleratorError::Memory)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorPendingDmaWrite {
    copy: AcceleratorDmaCopy,
    data: Vec<u8>,
    read_completed_at: Tick,
}

impl AcceleratorPendingDmaWrite {
    pub fn new(copy: AcceleratorDmaCopy, data: Vec<u8>, read_completed_at: Tick) -> Self {
        Self {
            copy,
            data,
            read_completed_at,
        }
    }

    pub const fn copy(&self) -> &AcceleratorDmaCopy {
        &self.copy
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub const fn read_completed_at(&self) -> Tick {
        self.read_completed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorDmaCompletion {
    command: AcceleratorCommandId,
    read_request: MemoryRequestId,
    write_request: MemoryRequestId,
    read_completed_at: Tick,
    write_completed_at: Tick,
}

impl AcceleratorDmaCompletion {
    pub const fn new(
        command: AcceleratorCommandId,
        read_request: MemoryRequestId,
        write_request: MemoryRequestId,
        read_completed_at: Tick,
        write_completed_at: Tick,
    ) -> Self {
        Self {
            command,
            read_request,
            write_request,
            read_completed_at,
            write_completed_at,
        }
    }

    pub const fn command(&self) -> AcceleratorCommandId {
        self.command
    }

    pub const fn read_request(&self) -> MemoryRequestId {
        self.read_request
    }

    pub const fn write_request(&self) -> MemoryRequestId {
        self.write_request
    }

    pub const fn read_completed_at(&self) -> Tick {
        self.read_completed_at
    }

    pub const fn write_completed_at(&self) -> Tick {
        self.write_completed_at
    }
}

#[derive(Clone)]
pub struct AcceleratorEngine {
    config: AcceleratorEngineConfig,
    state: Arc<Mutex<AcceleratorEngineState>>,
}

impl AcceleratorEngine {
    pub fn new(config: AcceleratorEngineConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(AcceleratorEngineState::new(config.lanes()))),
            config,
        }
    }

    pub const fn id(&self) -> AcceleratorEngineId {
        self.config.id()
    }

    pub const fn partition(&self) -> PartitionId {
        self.config.partition()
    }

    pub const fn lanes(&self) -> u32 {
        self.config.lanes()
    }

    pub fn submit_from_partition(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: PartitionId,
        submission_latency: Tick,
        command: AcceleratorCommand,
    ) -> Result<PartitionEventId, AcceleratorError> {
        let target = self.partition();
        scheduler
            .partition_now(source)
            .map_err(AcceleratorError::Scheduler)?;
        scheduler
            .partition_now(target)
            .map_err(AcceleratorError::Scheduler)?;
        validate_submission_latency(scheduler, source, target, submission_latency)?;

        let source_tick = scheduler.now();
        let engine = self.clone();
        scheduler
            .schedule_parallel_at(source, source_tick, move |context| {
                engine.record(AcceleratorTraceEvent::new(
                    context.now(),
                    AcceleratorTraceKind::Submitted {
                        command: command.id(),
                        source,
                        target,
                    },
                ));
                let target_engine = engine.clone();
                context
                    .schedule_remote_after(target, submission_latency, move |context| {
                        target_engine.accept_on_partition(context, command);
                    })
                    .expect("accelerator submission latency was validated");
            })
            .map_err(AcceleratorError::Scheduler)
    }

    pub fn trace(&self) -> Vec<AcceleratorTraceEvent> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .trace
            .clone()
    }

    pub fn completed(&self) -> Vec<AcceleratorCompletion> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .completed
            .clone()
    }

    pub fn pending_dma_writes(&self) -> Vec<AcceleratorPendingDmaWrite> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .pending_dma_writes
            .clone()
    }

    pub fn dma_completions(&self) -> Vec<AcceleratorDmaCompletion> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .dma_completions
            .clone()
    }

    pub fn submit_dma_copy_read<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, AcceleratorError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let command = copy.command();
        let request = copy.read_request().id();
        let read_request = copy.read_request().clone();
        let sink_engine = self.clone();
        let sink_copy = copy.clone();
        let event = transport
            .submit_parallel(
                scheduler,
                copy.read_route(),
                read_request,
                trace,
                responder,
                move |delivery| sink_engine.accept_dma_read_response(sink_copy, delivery),
            )
            .map_err(AcceleratorError::Transport)?;
        self.record(AcceleratorTraceEvent::new(
            scheduler.now(),
            AcceleratorTraceKind::DmaReadIssued { command, request },
        ));
        Ok(event)
    }

    pub fn issue_next_dma_write<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PartitionEventId>, AcceleratorError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(pending) = self.pop_pending_dma_write() else {
            return Ok(None);
        };
        let write_request = match pending.copy.make_write_request(pending.data.clone()) {
            Ok(request) => request,
            Err(error) => {
                self.push_pending_dma_write(pending);
                return Err(error);
            }
        };
        let command = pending.copy.command();
        let request = write_request.id();
        let sink_engine = self.clone();
        let completion = AcceleratorDmaCompletion::new(
            command,
            pending.copy.read_request().id(),
            write_request.id(),
            pending.read_completed_at(),
            0,
        );
        let route = pending.copy.write_route();
        let event = match transport.submit_parallel(
            scheduler,
            route,
            write_request,
            trace,
            responder,
            move |delivery| sink_engine.accept_dma_write_response(completion, delivery),
        ) {
            Ok(event) => event,
            Err(error) => {
                self.push_pending_dma_write(pending);
                return Err(AcceleratorError::Transport(error));
            }
        };
        self.record(AcceleratorTraceEvent::new(
            scheduler.now(),
            AcceleratorTraceKind::DmaWriteIssued { command, request },
        ));
        Ok(Some(event))
    }

    fn accept_on_partition(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        command: AcceleratorCommand,
    ) {
        let reservation = self.reserve(context.now(), command.execution_latency());
        if reservation.started_at == context.now() {
            self.start_now(context, command, reservation);
            return;
        }

        let engine = self.clone();
        context
            .schedule_local_after(reservation.started_at - context.now(), move |context| {
                engine.start_now(context, command, reservation);
            })
            .expect("accelerator queued start tick was reserved");
    }

    fn start_now(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        command: AcceleratorCommand,
        reservation: AcceleratorReservation,
    ) {
        self.record(AcceleratorTraceEvent::new(
            context.now(),
            AcceleratorTraceKind::Started {
                command: command.id(),
                lane: reservation.lane,
                complete_at: reservation.completed_at,
            },
        ));

        let engine = self.clone();
        context
            .schedule_local_after(command.execution_latency(), move |context| {
                engine.complete(context.now(), command, reservation);
            })
            .expect("accelerator command execution latency was validated");
    }

    fn complete(
        &self,
        tick: Tick,
        command: AcceleratorCommand,
        reservation: AcceleratorReservation,
    ) {
        let mut state = self.state.lock().expect("accelerator state lock");
        state.trace.push(AcceleratorTraceEvent::new(
            tick,
            AcceleratorTraceKind::Completed {
                command: command.id(),
                lane: reservation.lane,
            },
        ));
        state.completed.push(AcceleratorCompletion::new(
            command.id(),
            command.kind().clone(),
            reservation.lane,
            reservation.started_at,
            tick,
        ));
    }

    fn reserve(&self, now: Tick, execution_latency: Tick) -> AcceleratorReservation {
        let mut state = self.state.lock().expect("accelerator state lock");
        let (lane, available_at) = state
            .lane_busy_until
            .iter()
            .copied()
            .enumerate()
            .min_by_key(|(lane, tick)| (*tick, *lane))
            .expect("accelerator engine has at least one lane");
        let started_at = now.max(available_at);
        let completed_at = started_at
            .checked_add(execution_latency)
            .expect("accelerator command completion tick fits");
        state.lane_busy_until[lane] = completed_at;

        AcceleratorReservation {
            lane: lane as u32,
            started_at,
            completed_at,
        }
    }

    fn record(&self, event: AcceleratorTraceEvent) {
        self.state
            .lock()
            .expect("accelerator state lock")
            .trace
            .push(event);
    }

    fn accept_dma_read_response(&self, copy: AcceleratorDmaCopy, delivery: ResponseDelivery) {
        if let Some(data) = response_data(delivery.response()) {
            let bytes = data.len() as u64;
            let mut state = self.state.lock().expect("accelerator state lock");
            state.trace.push(AcceleratorTraceEvent::new(
                delivery.tick(),
                AcceleratorTraceKind::DmaReadCompleted {
                    command: copy.command(),
                    request: copy.read_request().id(),
                    bytes,
                },
            ));
            state
                .pending_dma_writes
                .push(AcceleratorPendingDmaWrite::new(copy, data, delivery.tick()));
        }
    }

    fn accept_dma_write_response(
        &self,
        mut completion: AcceleratorDmaCompletion,
        delivery: ResponseDelivery,
    ) {
        completion.write_completed_at = delivery.tick();
        let mut state = self.state.lock().expect("accelerator state lock");
        state.trace.push(AcceleratorTraceEvent::new(
            delivery.tick(),
            AcceleratorTraceKind::DmaWriteCompleted {
                command: completion.command(),
                request: completion.write_request(),
            },
        ));
        state.dma_completions.push(completion);
    }

    fn pop_pending_dma_write(&self) -> Option<AcceleratorPendingDmaWrite> {
        let mut state = self.state.lock().expect("accelerator state lock");
        if state.pending_dma_writes.is_empty() {
            None
        } else {
            Some(state.pending_dma_writes.remove(0))
        }
    }

    fn push_pending_dma_write(&self, pending: AcceleratorPendingDmaWrite) {
        self.state
            .lock()
            .expect("accelerator state lock")
            .pending_dma_writes
            .insert(0, pending);
    }
}

impl fmt::Debug for AcceleratorEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceleratorEngine")
            .field("id", &self.id())
            .field("partition", &self.partition())
            .field("lanes", &self.lanes())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceleratorReservation {
    lane: u32,
    started_at: Tick,
    completed_at: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcceleratorEngineState {
    lane_busy_until: Vec<Tick>,
    trace: Vec<AcceleratorTraceEvent>,
    completed: Vec<AcceleratorCompletion>,
    pending_dma_writes: Vec<AcceleratorPendingDmaWrite>,
    dma_completions: Vec<AcceleratorDmaCompletion>,
}

impl AcceleratorEngineState {
    fn new(lanes: u32) -> Self {
        Self {
            lane_busy_until: vec![0; lanes as usize],
            trace: Vec::new(),
            completed: Vec::new(),
            pending_dma_writes: Vec::new(),
            dma_completions: Vec::new(),
        }
    }
}

fn response_data(response: &MemoryResponse) -> Option<Vec<u8>> {
    response.data().map(<[u8]>::to_vec)
}

fn validate_submission_latency(
    scheduler: &PartitionedScheduler,
    source: PartitionId,
    target: PartitionId,
    delay: Tick,
) -> Result<(), AcceleratorError> {
    if source != target && delay == 0 {
        return Err(AcceleratorError::Scheduler(
            SchedulerError::ZeroDelayRemoteMessage { source, target },
        ));
    }
    if source != target && delay < scheduler.min_remote_delay() {
        return Err(AcceleratorError::Scheduler(
            SchedulerError::RemoteDelayBelowLookahead {
                source,
                target,
                delay,
                minimum: scheduler.min_remote_delay(),
            },
        ));
    }
    scheduler
        .now()
        .checked_add(delay)
        .ok_or(AcceleratorError::TickOverflow {
            now: scheduler.now(),
            delay,
        })?;

    Ok(())
}
