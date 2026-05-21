use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError,
    Tick,
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
    ZeroLanes { engine: AcceleratorEngineId },
    ZeroExecutionLatency { command: AcceleratorCommandId },
    TickOverflow { now: Tick, delay: Tick },
    Scheduler(SchedulerError),
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
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for AcceleratorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
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
}

impl AcceleratorEngineState {
    fn new(lanes: u32) -> Self {
        Self {
            lane_busy_until: vec![0; lanes as usize],
            trace: Vec::new(),
            completed: Vec::new(),
        }
    }
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
