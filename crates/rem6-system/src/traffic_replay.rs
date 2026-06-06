use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{SchedulerContext, Tick};
use rem6_memory::MemoryRequestId;
use rem6_traffic::{
    TrafficController, TrafficControllerEvent, TrafficControllerEventBatch, TrafficGeneratorError,
    TrafficTraceCacheEvent, TrafficTraceControlFailureRecord, TrafficTraceDiagnosticEvent,
    TrafficTraceHtmEvent, TrafficTraceMemoryFailureRecord, TrafficTraceReplayAction,
    TrafficTraceReplayActionQueue, TrafficTraceTlbEvent,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

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
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayTargetRuntime {
    action_queue: TrafficTraceReplayActionQueue,
    memory_failures: Vec<TrafficTraceReplayScheduledMemoryFailure>,
    request_ticks: BTreeMap<MemoryRequestId, Tick>,
}

impl TrafficTraceReplayTargetRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), rem6_traffic::TrafficGeneratorError> {
        if let Some(request) = batch
            .request()
            .filter(|request| request.request().requires_response())
        {
            self.request_ticks
                .insert(request.request().id(), request.tick());
        }
        for event in batch.events() {
            let TrafficControllerEvent::TraceReplayAction(action) = event else {
                continue;
            };
            if matches!(
                action,
                TrafficTraceReplayAction::MemoryResponse { .. }
                    | TrafficTraceReplayAction::MemoryFailure { .. }
            ) {
                self.action_queue.record_action(action.clone())?;
            }
        }
        Ok(())
    }

    pub fn target_event(
        &mut self,
        delivery: &RequestDelivery,
    ) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
        let event = traffic_trace_replay_target_event(&mut self.action_queue, delivery)?;
        self.request_ticks.remove(&delivery.request().id());
        Ok(event)
    }

    pub fn record_memory_failure(&mut self, tick: Tick, record: TrafficTraceMemoryFailureRecord) {
        self.memory_failures
            .push(TrafficTraceReplayScheduledMemoryFailure::new(tick, record));
    }

    pub fn memory_failures(&self) -> &[TrafficTraceReplayScheduledMemoryFailure] {
        &self.memory_failures
    }

    pub fn is_empty(&self) -> bool {
        self.action_queue.is_empty()
    }

    pub fn request_tick(&self, request: MemoryRequestId) -> Option<Tick> {
        self.request_ticks.get(&request).copied()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplaySidebandRuntime {
    pending_events: VecDeque<TrafficTraceReplaySidebandEvent>,
    sideband_events: Vec<TrafficTraceReplayScheduledSidebandEvent>,
}

impl TrafficTraceReplaySidebandRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), TrafficGeneratorError> {
        for event in batch.events() {
            match event {
                TrafficControllerEvent::TraceTlb(tlb) => {
                    self.pending_events
                        .push_back(TrafficTraceReplaySidebandEvent::Tlb(*tlb));
                }
                TrafficControllerEvent::TraceCache(cache) => {
                    self.pending_events
                        .push_back(TrafficTraceReplaySidebandEvent::Cache(*cache));
                }
                TrafficControllerEvent::TraceDiagnostic(diagnostic) => {
                    self.pending_events
                        .push_back(TrafficTraceReplaySidebandEvent::Diagnostic(*diagnostic));
                }
                TrafficControllerEvent::TraceHtm(htm) if !htm.requires_response() => {
                    self.pending_events
                        .push_back(TrafficTraceReplaySidebandEvent::Htm(*htm));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn next_sideband_event(
        &mut self,
        delivery_tick: Tick,
    ) -> Option<TrafficTraceReplaySidebandCompletion> {
        let event = self.pending_events.front().copied()?;
        let delay = sideband_delay(delivery_tick, event.tick());
        self.pending_events
            .pop_front()
            .expect("validated trace replay sideband event remains queued");
        Some(TrafficTraceReplaySidebandCompletion { delay, event })
    }

    pub fn record_sideband_event(&mut self, tick: Tick, event: TrafficTraceReplaySidebandEvent) {
        self.sideband_events
            .push(TrafficTraceReplayScheduledSidebandEvent::new(tick, event));
    }

    pub fn sideband_events(&self) -> &[TrafficTraceReplayScheduledSidebandEvent] {
        &self.sideband_events
    }

    pub fn is_empty(&self) -> bool {
        self.pending_events.is_empty()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayControlRuntime {
    action_queue: TrafficTraceReplayActionQueue,
    control_acks: Vec<TrafficTraceReplayScheduledControlAck>,
    control_failures: Vec<TrafficTraceReplayScheduledControlFailure>,
    source_ticks: VecDeque<Tick>,
}

impl TrafficTraceReplayControlRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), rem6_traffic::TrafficGeneratorError> {
        for event in batch.events() {
            match event {
                TrafficControllerEvent::TraceSync(sync) if sync.requires_response() => {
                    self.source_ticks.push_back(sync.tick());
                }
                TrafficControllerEvent::TraceHtm(htm) if htm.requires_response() => {
                    self.source_ticks.push_back(htm.tick());
                }
                TrafficControllerEvent::TraceReplayAction(action)
                    if matches!(
                        action,
                        TrafficTraceReplayAction::ControlAck { .. }
                            | TrafficTraceReplayAction::ControlFailure { .. }
                    ) =>
                {
                    self.action_queue.record_action(action.clone())?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn control_event(
        &mut self,
        delivery_tick: Tick,
    ) -> Result<TrafficTraceReplayControlEvent, TrafficTraceReplayControlError> {
        let action = self
            .action_queue
            .peek_action()
            .ok_or(TrafficTraceReplayControlError::ActionQueueEmpty { delivery_tick })?;
        let event = match action {
            TrafficTraceReplayAction::ControlAck { tick } => {
                let delay = control_ack_delay(delivery_tick, *tick)?;
                TrafficTraceReplayControlEvent::ControlAck {
                    delay,
                    trace_tick: *tick,
                }
            }
            TrafficTraceReplayAction::ControlFailure { tick, failure } => {
                let delay = control_failure_delay(delivery_tick, *tick)?;
                TrafficTraceReplayControlEvent::ControlFailure {
                    delay,
                    record: TrafficTraceControlFailureRecord::new(*tick, *failure),
                }
            }
            action => {
                return Err(TrafficTraceReplayControlError::UnexpectedAction {
                    delivery_tick,
                    action: action.clone(),
                });
            }
        };

        self.action_queue
            .pop_action()
            .expect("validated trace replay control action remains queued");
        self.source_ticks.pop_front();
        Ok(event)
    }

    pub fn record_control_ack(&mut self, tick: Tick, trace_tick: Tick) {
        self.control_acks
            .push(TrafficTraceReplayScheduledControlAck::new(tick, trace_tick));
    }

    pub fn record_control_failure(&mut self, tick: Tick, record: TrafficTraceControlFailureRecord) {
        self.control_failures
            .push(TrafficTraceReplayScheduledControlFailure::new(tick, record));
    }

    pub fn control_acks(&self) -> &[TrafficTraceReplayScheduledControlAck] {
        &self.control_acks
    }

    pub fn control_failures(&self) -> &[TrafficTraceReplayScheduledControlFailure] {
        &self.control_failures
    }

    pub fn is_empty(&self) -> bool {
        self.action_queue.is_empty() && self.source_ticks.is_empty()
    }

    fn source_tick(&self) -> Option<Tick> {
        self.source_ticks.front().copied()
    }

    fn has_control_action(&self) -> bool {
        !self.action_queue.is_empty()
    }

    fn clear_control_sources(&mut self) {
        self.source_ticks.clear();
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayControllerRuntime {
    target: TrafficTraceReplayTargetRuntime,
    control: TrafficTraceReplayControlRuntime,
    sideband: TrafficTraceReplaySidebandRuntime,
}

impl TrafficTraceReplayControllerRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), TrafficGeneratorError> {
        self.target.record_batch(batch)?;
        self.control.record_batch(batch)?;
        self.sideband.record_batch(batch)?;
        Ok(())
    }

    pub fn memory_failures(&self) -> &[TrafficTraceReplayScheduledMemoryFailure] {
        self.target.memory_failures()
    }

    pub fn control_acks(&self) -> &[TrafficTraceReplayScheduledControlAck] {
        self.control.control_acks()
    }

    pub fn control_failures(&self) -> &[TrafficTraceReplayScheduledControlFailure] {
        self.control.control_failures()
    }

    pub fn sideband_events(&self) -> &[TrafficTraceReplayScheduledSidebandEvent] {
        self.sideband.sideband_events()
    }

    pub fn is_empty(&self) -> bool {
        self.target.is_empty() && self.control.is_empty() && self.sideband.is_empty()
    }

    fn target_request_tick(&self, request: MemoryRequestId) -> Option<Tick> {
        self.target.request_tick(request)
    }

    fn target_event(
        &mut self,
        delivery: &RequestDelivery,
    ) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
        self.target.target_event(delivery)
    }

    fn has_target_action(&self) -> bool {
        !self.target.is_empty()
    }

    fn record_memory_failure(&mut self, tick: Tick, record: TrafficTraceMemoryFailureRecord) {
        self.target.record_memory_failure(tick, record);
    }

    fn control_source_tick(&self) -> Option<Tick> {
        self.control.source_tick()
    }

    fn control_event(
        &mut self,
        delivery_tick: Tick,
    ) -> Result<TrafficTraceReplayControlEvent, TrafficTraceReplayControlError> {
        self.control.control_event(delivery_tick)
    }

    fn has_control_action(&self) -> bool {
        self.control.has_control_action()
    }

    fn clear_control_sources(&mut self) {
        self.control.clear_control_sources();
    }

    fn record_control_ack(&mut self, tick: Tick, trace_tick: Tick) {
        self.control.record_control_ack(tick, trace_tick);
    }

    fn record_control_failure(&mut self, tick: Tick, record: TrafficTraceControlFailureRecord) {
        self.control.record_control_failure(tick, record);
    }

    fn next_sideband_event(
        &mut self,
        delivery_tick: Tick,
    ) -> Option<TrafficTraceReplaySidebandCompletion> {
        self.sideband.next_sideband_event(delivery_tick)
    }

    fn record_sideband_event(&mut self, tick: Tick, event: TrafficTraceReplaySidebandEvent) {
        self.sideband.record_sideband_event(tick, event);
    }
}

pub fn traffic_trace_replay_runtime_target_outcome(
    runtime: Arc<Mutex<TrafficTraceReplayTargetRuntime>>,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
) -> Result<TargetOutcome, TrafficTraceReplayTargetError> {
    let event = runtime
        .lock()
        .expect("trace replay target runtime lock")
        .target_event(delivery)?;
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => Ok(outcome),
        TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay target runtime lock")
                        .record_memory_failure(context.now(), record);
                })
                .expect("validated trace replay failure delay");
            Ok(TargetOutcome::NoResponse)
        }
    }
}

pub fn traffic_trace_replay_runtime_control_completion(
    runtime: Arc<Mutex<TrafficTraceReplayControlRuntime>>,
    delivery_tick: Tick,
    context: &mut SchedulerContext<'_>,
) -> Result<(), TrafficTraceReplayControlError> {
    let event = runtime
        .lock()
        .expect("trace replay control runtime lock")
        .control_event(delivery_tick)?;
    match event {
        TrafficTraceReplayControlEvent::ControlAck { delay, trace_tick } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay control runtime lock")
                        .record_control_ack(context.now(), trace_tick);
                })
                .expect("validated trace replay control ack delay");
        }
        TrafficTraceReplayControlEvent::ControlFailure { delay, record } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay control runtime lock")
                        .record_control_failure(context.now(), record);
                })
                .expect("validated trace replay control failure delay");
        }
    }
    Ok(())
}

pub fn traffic_trace_replay_runtime_sideband_events(
    runtime: Arc<Mutex<TrafficTraceReplaySidebandRuntime>>,
    delivery_tick: Tick,
    context: &mut SchedulerContext<'_>,
) -> usize {
    let mut scheduled = 0;
    loop {
        let Some(completion) = runtime
            .lock()
            .expect("trace replay sideband runtime lock")
            .next_sideband_event(delivery_tick)
        else {
            return scheduled;
        };
        scheduled += 1;
        let replay = Arc::clone(&runtime);
        context
            .schedule_local_after(completion.delay, move |context| {
                replay
                    .lock()
                    .expect("trace replay sideband runtime lock")
                    .record_sideband_event(context.now(), completion.event);
            })
            .expect("validated trace replay sideband delay");
    }
}

pub fn traffic_trace_replay_controller_target_outcome(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    controller: Arc<Mutex<TrafficController>>,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
    retry_delay: Tick,
) -> Result<TargetOutcome, TrafficTraceReplayControllerTargetError> {
    if !delivery.request().requires_response() {
        return Ok(TargetOutcome::NoResponse);
    }

    let controller_tick = runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .target_request_tick(delivery.request().id())
        .unwrap_or_else(|| delivery.tick());
    loop {
        match traffic_trace_replay_controller_runtime_target_outcome(
            Arc::clone(&runtime),
            delivery,
            context,
        ) {
            Ok(outcome) => return Ok(outcome),
            Err(TrafficTraceReplayTargetError::ActionQueueEmpty { .. }) => {}
            Err(error) => return Err(error.into()),
        }

        let batch = controller
            .lock()
            .expect("traffic controller lock")
            .next_event(controller_tick, retry_delay)?;
        let Some(batch) = batch else {
            return Err(
                TrafficTraceReplayControllerTargetError::ReplayActionMissing {
                    request: delivery.request().id(),
                },
            );
        };

        let trace_exited = batch.trace_exit().is_some();
        let target_action_available = {
            let mut runtime = runtime
                .lock()
                .expect("trace replay controller runtime lock");
            runtime.record_batch(&batch)?;
            runtime.has_target_action()
        };
        if trace_exited && !target_action_available {
            return Err(
                TrafficTraceReplayControllerTargetError::ReplayActionMissing {
                    request: delivery.request().id(),
                },
            );
        }
    }
}

pub fn traffic_trace_replay_controller_control_completion(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    controller: Arc<Mutex<TrafficController>>,
    delivery_tick: Tick,
    context: &mut SchedulerContext<'_>,
    retry_delay: Tick,
) -> Result<(), TrafficTraceReplayControllerControlError> {
    let controller_tick = runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .control_source_tick()
        .unwrap_or(delivery_tick);
    loop {
        match traffic_trace_replay_controller_runtime_control_completion(
            Arc::clone(&runtime),
            delivery_tick,
            context,
        ) {
            Ok(()) => return Ok(()),
            Err(TrafficTraceReplayControlError::ActionQueueEmpty { .. }) => {}
            Err(error) => return Err(error.into()),
        }

        let batch = controller
            .lock()
            .expect("traffic controller lock")
            .next_event(controller_tick, retry_delay)?;
        let Some(batch) = batch else {
            runtime
                .lock()
                .expect("trace replay controller runtime lock")
                .clear_control_sources();
            return Err(
                TrafficTraceReplayControllerControlError::ReplayActionMissing { delivery_tick },
            );
        };

        let trace_exited = batch.trace_exit().is_some();
        let control_action_available = {
            let mut runtime = runtime
                .lock()
                .expect("trace replay controller runtime lock");
            runtime.record_batch(&batch)?;
            runtime.has_control_action()
        };
        if trace_exited && !control_action_available {
            runtime
                .lock()
                .expect("trace replay controller runtime lock")
                .clear_control_sources();
            return Err(
                TrafficTraceReplayControllerControlError::ReplayActionMissing { delivery_tick },
            );
        }
    }
}

fn traffic_trace_replay_controller_runtime_target_outcome(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
) -> Result<TargetOutcome, TrafficTraceReplayTargetError> {
    let event = runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .target_event(delivery)?;
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => Ok(outcome),
        TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay controller runtime lock")
                        .record_memory_failure(context.now(), record);
                })
                .expect("validated trace replay failure delay");
            Ok(TargetOutcome::NoResponse)
        }
    }
}

fn traffic_trace_replay_controller_runtime_control_completion(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery_tick: Tick,
    context: &mut SchedulerContext<'_>,
) -> Result<(), TrafficTraceReplayControlError> {
    let event = runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .control_event(delivery_tick)?;
    match event {
        TrafficTraceReplayControlEvent::ControlAck { delay, trace_tick } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay controller runtime lock")
                        .record_control_ack(context.now(), trace_tick);
                })
                .expect("validated trace replay control ack delay");
        }
        TrafficTraceReplayControlEvent::ControlFailure { delay, record } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay controller runtime lock")
                        .record_control_failure(context.now(), record);
                })
                .expect("validated trace replay control failure delay");
        }
    }
    Ok(())
}

pub fn traffic_trace_replay_controller_runtime_sideband_events(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery_tick: Tick,
    context: &mut SchedulerContext<'_>,
) -> usize {
    let mut scheduled = 0;
    loop {
        let Some(completion) = runtime
            .lock()
            .expect("trace replay controller runtime lock")
            .next_sideband_event(delivery_tick)
        else {
            return scheduled;
        };
        scheduled += 1;
        let replay = Arc::clone(&runtime);
        context
            .schedule_local_after(completion.delay, move |context| {
                replay
                    .lock()
                    .expect("trace replay controller runtime lock")
                    .record_sideband_event(context.now(), completion.event);
            })
            .expect("validated trace replay sideband delay");
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplaySidebandCompletion {
    delay: Tick,
    event: TrafficTraceReplaySidebandEvent,
}

impl TrafficTraceReplaySidebandCompletion {
    pub const fn delay(self) -> Tick {
        self.delay
    }

    pub const fn event(self) -> TrafficTraceReplaySidebandEvent {
        self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayControllerTargetError {
    Target(TrafficTraceReplayTargetError),
    Generator(TrafficGeneratorError),
    ReplayActionMissing { request: MemoryRequestId },
}

impl fmt::Display for TrafficTraceReplayControllerTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(formatter, "{error}"),
            Self::Generator(error) => write!(formatter, "{error}"),
            Self::ReplayActionMissing { request } => {
                write!(
                    formatter,
                    "trace replay controller has no response or failure action for {request:?}"
                )
            }
        }
    }
}

impl Error for TrafficTraceReplayControllerTargetError {}

impl From<TrafficTraceReplayTargetError> for TrafficTraceReplayControllerTargetError {
    fn from(error: TrafficTraceReplayTargetError) -> Self {
        Self::Target(error)
    }
}

impl From<TrafficGeneratorError> for TrafficTraceReplayControllerTargetError {
    fn from(error: TrafficGeneratorError) -> Self {
        Self::Generator(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayControllerControlError {
    Control(TrafficTraceReplayControlError),
    Generator(TrafficGeneratorError),
    ReplayActionMissing { delivery_tick: Tick },
}

impl fmt::Display for TrafficTraceReplayControllerControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(error) => write!(formatter, "{error}"),
            Self::Generator(error) => write!(formatter, "{error}"),
            Self::ReplayActionMissing { delivery_tick } => {
                write!(
                    formatter,
                    "trace replay controller has no control acknowledgement or failure action for delivery tick {delivery_tick}"
                )
            }
        }
    }
}

impl Error for TrafficTraceReplayControllerControlError {}

impl From<TrafficTraceReplayControlError> for TrafficTraceReplayControllerControlError {
    fn from(error: TrafficTraceReplayControlError) -> Self {
        Self::Control(error)
    }
}

impl From<TrafficGeneratorError> for TrafficTraceReplayControllerControlError {
    fn from(error: TrafficGeneratorError) -> Self {
        Self::Generator(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayControlEvent {
    ControlAck {
        delay: Tick,
        trace_tick: Tick,
    },
    ControlFailure {
        delay: Tick,
        record: TrafficTraceControlFailureRecord,
    },
}

impl TrafficTraceReplayControlEvent {
    pub const fn control_delay(&self) -> Tick {
        match self {
            Self::ControlAck { delay, .. } | Self::ControlFailure { delay, .. } => *delay,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayControlError {
    ActionQueueEmpty {
        delivery_tick: Tick,
    },
    UnexpectedAction {
        delivery_tick: Tick,
        action: TrafficTraceReplayAction,
    },
    AckBeforeDelivery {
        delivery_tick: Tick,
        ack_tick: Tick,
    },
    FailureBeforeDelivery {
        delivery_tick: Tick,
        failure_tick: Tick,
    },
}

impl fmt::Display for TrafficTraceReplayControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActionQueueEmpty { delivery_tick } => {
                write!(
                    formatter,
                    "trace replay control action queue is empty for delivery tick {delivery_tick}"
                )
            }
            Self::UnexpectedAction {
                delivery_tick,
                action,
            } => {
                write!(
                    formatter,
                    "trace replay action {action:?} cannot answer control delivery tick {delivery_tick}"
                )
            }
            Self::AckBeforeDelivery {
                delivery_tick,
                ack_tick,
            } => {
                write!(
                    formatter,
                    "trace replay control acknowledgement at tick {ack_tick} precedes delivery tick {delivery_tick}"
                )
            }
            Self::FailureBeforeDelivery {
                delivery_tick,
                failure_tick,
            } => {
                write!(
                    formatter,
                    "trace replay control failure at tick {failure_tick} precedes delivery tick {delivery_tick}"
                )
            }
        }
    }
}

impl Error for TrafficTraceReplayControlError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayTargetEvent {
    MemoryResponse(TargetOutcome),
    MemoryFailure {
        delay: Tick,
        record: TrafficTraceMemoryFailureRecord,
    },
}

impl TrafficTraceReplayTargetEvent {
    pub fn target_delay(&self) -> Tick {
        match self {
            Self::MemoryResponse(outcome) => match outcome {
                TargetOutcome::Respond(_) | TargetOutcome::NoResponse => 0,
                TargetOutcome::RespondAfter { delay, .. } => *delay,
            },
            Self::MemoryFailure { delay, .. } => *delay,
        }
    }

    pub fn into_target_outcome(self) -> TargetOutcome {
        match self {
            Self::MemoryResponse(outcome) => outcome,
            Self::MemoryFailure { .. } => TargetOutcome::NoResponse,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayTargetError {
    ActionQueueEmpty {
        request: MemoryRequestId,
    },
    UnexpectedAction {
        request: MemoryRequestId,
        action: TrafficTraceReplayAction,
    },
    RequestMismatch {
        request: MemoryRequestId,
        response: MemoryRequestId,
    },
    FailureRequestMismatch {
        request: MemoryRequestId,
        failure: MemoryRequestId,
    },
    ResponseBeforeRequest {
        request: MemoryRequestId,
        delivery_tick: Tick,
        response_tick: Tick,
    },
    FailureBeforeRequest {
        request: MemoryRequestId,
        delivery_tick: Tick,
        failure_tick: Tick,
    },
}

impl fmt::Display for TrafficTraceReplayTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActionQueueEmpty { request } => {
                write!(
                    formatter,
                    "trace replay action queue is empty for {request:?}"
                )
            }
            Self::UnexpectedAction { request, action } => {
                write!(
                    formatter,
                    "trace replay action {action:?} cannot answer memory request {request:?}"
                )
            }
            Self::RequestMismatch { request, response } => {
                write!(
                    formatter,
                    "trace replay response {response:?} does not match memory request {request:?}"
                )
            }
            Self::FailureRequestMismatch { request, failure } => {
                write!(
                    formatter,
                    "trace replay failure {failure:?} does not match memory request {request:?}"
                )
            }
            Self::ResponseBeforeRequest {
                request,
                delivery_tick,
                response_tick,
            } => {
                write!(
                    formatter,
                    "trace replay response for {request:?} at tick {response_tick} precedes delivery tick {delivery_tick}"
                )
            }
            Self::FailureBeforeRequest {
                request,
                delivery_tick,
                failure_tick,
            } => {
                write!(
                    formatter,
                    "trace replay failure for {request:?} at tick {failure_tick} precedes delivery tick {delivery_tick}"
                )
            }
        }
    }
}

impl Error for TrafficTraceReplayTargetError {}

pub fn traffic_trace_replay_target_outcome(
    queue: &mut TrafficTraceReplayActionQueue,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, TrafficTraceReplayTargetError> {
    let request = delivery.request().id();
    let action = queue
        .peek_action()
        .ok_or(TrafficTraceReplayTargetError::ActionQueueEmpty { request })?;
    let (tick, response_request) = match action {
        TrafficTraceReplayAction::MemoryResponse { tick, response } => {
            (*tick, response.request_id())
        }
        action => {
            return Err(TrafficTraceReplayTargetError::UnexpectedAction {
                request,
                action: action.clone(),
            });
        }
    };

    if response_request != request {
        return Err(TrafficTraceReplayTargetError::RequestMismatch {
            request,
            response: response_request,
        });
    }

    let delay = memory_response_delay(request, delivery.tick(), tick)?;
    let response = pop_validated_memory_response(queue);
    Ok(memory_response_outcome(delay, response))
}

pub fn traffic_trace_replay_target_event(
    queue: &mut TrafficTraceReplayActionQueue,
    delivery: &RequestDelivery,
) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
    let request = delivery.request().id();
    let action = queue
        .peek_action()
        .ok_or(TrafficTraceReplayTargetError::ActionQueueEmpty { request })?;
    match action {
        TrafficTraceReplayAction::MemoryResponse { tick, response } => {
            let response_request = response.request_id();
            if response_request != request {
                return Err(TrafficTraceReplayTargetError::RequestMismatch {
                    request,
                    response: response_request,
                });
            }
            let delay = memory_response_delay(request, delivery.tick(), *tick)?;
            let response = pop_validated_memory_response(queue);
            Ok(TrafficTraceReplayTargetEvent::MemoryResponse(
                memory_response_outcome(delay, response),
            ))
        }
        TrafficTraceReplayAction::MemoryFailure { tick, failure } => {
            let failure_request = failure.request_id();
            if failure_request != request {
                return Err(TrafficTraceReplayTargetError::FailureRequestMismatch {
                    request,
                    failure: failure_request,
                });
            }
            let delay = memory_failure_delay(request, delivery.tick(), *tick)?;
            Ok(TrafficTraceReplayTargetEvent::MemoryFailure {
                delay,
                record: pop_validated_memory_failure(queue),
            })
        }
        action => Err(TrafficTraceReplayTargetError::UnexpectedAction {
            request,
            action: action.clone(),
        }),
    }
}

fn memory_response_delay(
    request: MemoryRequestId,
    delivery_tick: Tick,
    response_tick: Tick,
) -> Result<Tick, TrafficTraceReplayTargetError> {
    response_tick.checked_sub(delivery_tick).ok_or(
        TrafficTraceReplayTargetError::ResponseBeforeRequest {
            request,
            delivery_tick,
            response_tick,
        },
    )
}

fn memory_failure_delay(
    request: MemoryRequestId,
    delivery_tick: Tick,
    failure_tick: Tick,
) -> Result<Tick, TrafficTraceReplayTargetError> {
    failure_tick.checked_sub(delivery_tick).ok_or(
        TrafficTraceReplayTargetError::FailureBeforeRequest {
            request,
            delivery_tick,
            failure_tick,
        },
    )
}

fn control_ack_delay(
    delivery_tick: Tick,
    ack_tick: Tick,
) -> Result<Tick, TrafficTraceReplayControlError> {
    ack_tick
        .checked_sub(delivery_tick)
        .ok_or(TrafficTraceReplayControlError::AckBeforeDelivery {
            delivery_tick,
            ack_tick,
        })
}

fn control_failure_delay(
    delivery_tick: Tick,
    failure_tick: Tick,
) -> Result<Tick, TrafficTraceReplayControlError> {
    failure_tick.checked_sub(delivery_tick).ok_or(
        TrafficTraceReplayControlError::FailureBeforeDelivery {
            delivery_tick,
            failure_tick,
        },
    )
}

fn sideband_delay(delivery_tick: Tick, event_tick: Tick) -> Tick {
    event_tick.saturating_sub(delivery_tick)
}

fn memory_response_outcome(delay: Tick, response: rem6_memory::MemoryResponse) -> TargetOutcome {
    if delay == 0 {
        TargetOutcome::Respond(response)
    } else {
        TargetOutcome::RespondAfter { delay, response }
    }
}

fn pop_validated_memory_response(
    queue: &mut TrafficTraceReplayActionQueue,
) -> rem6_memory::MemoryResponse {
    match queue
        .pop_action()
        .expect("validated trace replay action remains queued")
    {
        TrafficTraceReplayAction::MemoryResponse { response, .. } => response,
        _ => unreachable!("validated trace replay action is a memory response"),
    }
}

fn pop_validated_memory_failure(
    queue: &mut TrafficTraceReplayActionQueue,
) -> TrafficTraceMemoryFailureRecord {
    match queue
        .pop_action()
        .expect("validated trace replay action remains queued")
    {
        TrafficTraceReplayAction::MemoryFailure { tick, failure } => {
            TrafficTraceMemoryFailureRecord::new(tick, failure)
        }
        _ => unreachable!("validated trace replay action is a memory failure"),
    }
}
