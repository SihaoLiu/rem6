use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::MemoryRequestId;
use rem6_traffic::{
    TrafficController, TrafficControllerEvent, TrafficControllerEventBatch, TrafficGeneratorError,
    TrafficTraceCacheEvent, TrafficTraceControlFailureRecord, TrafficTraceDiagnosticEvent,
    TrafficTraceErrorEvent, TrafficTraceHtmEvent, TrafficTraceMemoryFailureRecord,
    TrafficTraceReplayAction, TrafficTraceReplayActionQueue, TrafficTraceReplaySource,
    TrafficTraceResponseEvent, TrafficTraceSyncEvent, TrafficTraceTlbEvent,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

mod parallel_executor;

pub use parallel_executor::{
    TrafficTraceReplayControllerParallelErrors, TrafficTraceReplayControllerParallelExecutor,
    TrafficTraceReplayControllerParallelSubmitError, TrafficTraceReplayOrder,
};

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

    pub const fn sequence(self) -> u64 {
        match self {
            Self::Tlb(event) => event.sequence(),
            Self::Cache(event) => event.sequence(),
            Self::Diagnostic(event) => event.sequence(),
            Self::Htm(event) => event.sequence(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayTargetRuntime {
    actions: VecDeque<TrafficTraceReplayTargetAction>,
    memory_failures: Vec<TrafficTraceReplayScheduledMemoryFailure>,
    request_ticks: BTreeMap<MemoryRequestId, Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrafficTraceReplayTargetAction {
    action: TrafficTraceReplayAction,
    request: Option<MemoryRequestId>,
    trace_response: Option<TrafficTraceResponseEvent>,
    trace_error: Option<TrafficTraceErrorEvent>,
}

impl TrafficTraceReplayTargetAction {
    fn new(
        action: TrafficTraceReplayAction,
        request: Option<MemoryRequestId>,
        trace_response: Option<TrafficTraceResponseEvent>,
        trace_error: Option<TrafficTraceErrorEvent>,
    ) -> Self {
        Self {
            action,
            request,
            trace_response,
            trace_error,
        }
    }

    fn matches_request(&self, request: MemoryRequestId) -> bool {
        self.request
            .is_none_or(|action_request| action_request == request)
    }

    fn trace_response(&self) -> Option<TrafficTraceResponseEvent> {
        self.trace_response
    }

    fn trace_error(&self) -> Option<TrafficTraceErrorEvent> {
        self.trace_error
    }

    fn trace_response_data(&self) -> Option<Vec<u8>> {
        match &self.action {
            TrafficTraceReplayAction::MemoryResponse { trace_data, .. } => trace_data.clone(),
            _ => None,
        }
    }
}

impl TrafficTraceReplayTargetRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), rem6_traffic::TrafficGeneratorError> {
        let mut matched_request = None;
        let mut matched_response = None;
        let mut matched_error = None;
        if let Some(request) = batch
            .request()
            .filter(|request| request.request().requires_response())
        {
            self.request_ticks
                .insert(request.request().id(), request.tick());
        }
        for event in batch.events() {
            match event {
                TrafficControllerEvent::TraceResponseMatch(response) => {
                    matched_request = replay_source_request(response.source());
                    matched_response = Some(response.response());
                    matched_error = None;
                }
                TrafficControllerEvent::TraceErrorMatch(error) => {
                    matched_request = replay_source_request(error.source());
                    matched_response = None;
                    matched_error = Some(error.error());
                }
                TrafficControllerEvent::TraceReplayAction(action)
                    if matches!(
                        action,
                        TrafficTraceReplayAction::MemoryResponse { .. }
                            | TrafficTraceReplayAction::MemoryFailure { .. }
                    ) =>
                {
                    self.actions.push_back(TrafficTraceReplayTargetAction::new(
                        action.clone(),
                        matched_request.take(),
                        matched_response.take(),
                        matched_error.take(),
                    ));
                }
                TrafficControllerEvent::TraceReplayAction(_) => {
                    matched_request = None;
                    matched_response = None;
                    matched_error = None;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn target_event(
        &mut self,
        delivery: &RequestDelivery,
    ) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
        self.target_event_context(delivery)
            .map(TrafficTraceReplayTargetEventContext::into_event)
    }

    pub fn target_event_context(
        &mut self,
        delivery: &RequestDelivery,
    ) -> Result<TrafficTraceReplayTargetEventContext, TrafficTraceReplayTargetError> {
        let request = delivery.request().id();
        let action_index = self
            .target_action_index(request)
            .ok_or(TrafficTraceReplayTargetError::ActionQueueEmpty { request })?;
        let action = &self.actions[action_index];
        let event = target_event_for_action(&action.action, delivery)?;
        let trace_response = action.trace_response();
        let trace_error = action.trace_error();
        let trace_response_data = action.trace_response_data();
        self.actions
            .remove(action_index)
            .expect("validated trace replay target action remains queued");
        self.request_ticks.remove(&delivery.request().id());
        Ok(
            TrafficTraceReplayTargetEventContext::with_trace_response_data(
                event,
                trace_response,
                trace_error,
                trace_response_data,
            ),
        )
    }

    pub fn record_memory_failure(&mut self, tick: Tick, record: TrafficTraceMemoryFailureRecord) {
        self.memory_failures
            .push(TrafficTraceReplayScheduledMemoryFailure::new(tick, record));
    }

    pub fn memory_failures(&self) -> &[TrafficTraceReplayScheduledMemoryFailure] {
        &self.memory_failures
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty() && self.request_ticks.is_empty()
    }

    fn has_replay_action(&self, request: MemoryRequestId) -> bool {
        self.target_action_index(request).is_some()
    }

    pub fn request_tick(&self, request: MemoryRequestId) -> Option<Tick> {
        self.request_ticks.get(&request).copied()
    }

    fn drop_request(&mut self, request: MemoryRequestId) {
        self.request_ticks.remove(&request);
    }

    fn target_action_index(&self, request: MemoryRequestId) -> Option<usize> {
        self.actions
            .iter()
            .position(|action| action.matches_request(request))
    }
}

fn replay_source_request(source: &TrafficTraceReplaySource) -> Option<MemoryRequestId> {
    match source {
        TrafficTraceReplaySource::Memory(request) => Some(request.request().id()),
        TrafficTraceReplaySource::Sync(_) | TrafficTraceReplaySource::Htm(_) => None,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrafficTraceReplayControlSource {
    Sync(TrafficTraceSyncEvent),
    Htm(TrafficTraceHtmEvent),
}

impl TrafficTraceReplayControlSource {
    const fn tick(self) -> Tick {
        match self {
            Self::Sync(event) => event.tick(),
            Self::Htm(event) => event.tick(),
        }
    }

    fn from_replay_source(source: &TrafficTraceReplaySource) -> Option<Self> {
        match source {
            TrafficTraceReplaySource::Sync(event) => Some(Self::Sync(*event)),
            TrafficTraceReplaySource::Htm(event) => Some(Self::Htm(*event)),
            TrafficTraceReplaySource::Memory(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrafficTraceReplayControlAction {
    action: TrafficTraceReplayAction,
    source: Option<TrafficTraceReplayControlSource>,
}

impl TrafficTraceReplayControlAction {
    fn new(
        action: TrafficTraceReplayAction,
        source: Option<TrafficTraceReplayControlSource>,
    ) -> Self {
        Self { action, source }
    }

    fn matches_source(&self, source: Option<TrafficTraceReplayControlSource>) -> bool {
        match (source, self.source) {
            (Some(source), Some(action_source)) => source == action_source,
            (None, Some(_)) => false,
            (_, None) => true,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayControlRuntime {
    actions: VecDeque<TrafficTraceReplayControlAction>,
    control_acks: Vec<TrafficTraceReplayScheduledControlAck>,
    control_failures: Vec<TrafficTraceReplayScheduledControlFailure>,
    sources: VecDeque<TrafficTraceReplayControlSource>,
}

impl TrafficTraceReplayControlRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), rem6_traffic::TrafficGeneratorError> {
        let mut matched_source = None;
        for event in batch.events() {
            match event {
                TrafficControllerEvent::TraceSync(sync) if sync.requires_response() => {
                    self.sources
                        .push_back(TrafficTraceReplayControlSource::Sync(*sync));
                }
                TrafficControllerEvent::TraceHtm(htm) if htm.requires_response() => {
                    self.sources
                        .push_back(TrafficTraceReplayControlSource::Htm(*htm));
                }
                TrafficControllerEvent::TraceResponseMatch(response) => {
                    matched_source =
                        TrafficTraceReplayControlSource::from_replay_source(response.source());
                }
                TrafficControllerEvent::TraceErrorMatch(error) => {
                    matched_source =
                        TrafficTraceReplayControlSource::from_replay_source(error.source());
                }
                TrafficControllerEvent::TraceReplayAction(action)
                    if matches!(
                        action,
                        TrafficTraceReplayAction::ControlAck { .. }
                            | TrafficTraceReplayAction::ControlFailure { .. }
                    ) =>
                {
                    self.actions.push_back(TrafficTraceReplayControlAction::new(
                        action.clone(),
                        matched_source.take(),
                    ));
                }
                TrafficControllerEvent::TraceReplayAction(_) => {
                    matched_source = None;
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
        self.control_event_context(delivery_tick)
            .map(TrafficTraceReplayControlEventContext::into_event)
    }

    pub fn control_event_context(
        &mut self,
        delivery_tick: Tick,
    ) -> Result<TrafficTraceReplayControlEventContext, TrafficTraceReplayControlError> {
        let action_index = self
            .control_action_index()
            .ok_or(TrafficTraceReplayControlError::ActionQueueEmpty { delivery_tick })?;
        let source = self.source();
        let action = &self
            .actions
            .get(action_index)
            .expect("validated trace replay control action remains queued")
            .action;
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

        self.actions
            .remove(action_index)
            .expect("validated trace replay control action remains queued");
        if source.is_some() {
            self.sources.pop_front();
        }
        Ok(TrafficTraceReplayControlEventContext::new(event, source))
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
        self.actions.is_empty() && self.sources.is_empty()
    }

    fn source_tick(&self) -> Option<Tick> {
        self.source().map(TrafficTraceReplayControlSource::tick)
    }

    fn source(&self) -> Option<TrafficTraceReplayControlSource> {
        self.sources.front().copied()
    }

    fn has_control_action(&self) -> bool {
        self.control_action_index().is_some()
    }

    fn control_action_index(&self) -> Option<usize> {
        let source = self.source();
        self.actions
            .iter()
            .position(|action| action.matches_source(source))
    }

    fn drop_current_source(&mut self) {
        let Some(source) = self.sources.pop_front() else {
            return;
        };
        while self.sources.front().copied() == Some(source) {
            self.sources.pop_front();
        }
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

    fn target_event_context(
        &mut self,
        delivery: &RequestDelivery,
    ) -> Result<TrafficTraceReplayTargetEventContext, TrafficTraceReplayTargetError> {
        self.target.target_event_context(delivery)
    }

    fn has_target_action(&self, request: MemoryRequestId) -> bool {
        self.target.has_replay_action(request)
    }

    fn drop_target_request(&mut self, request: MemoryRequestId) {
        self.target.drop_request(request);
    }

    fn record_memory_failure(&mut self, tick: Tick, record: TrafficTraceMemoryFailureRecord) {
        self.target.record_memory_failure(tick, record);
    }

    fn control_source_tick(&self) -> Option<Tick> {
        self.control.source_tick()
    }

    fn control_source(&self) -> Option<TrafficTraceReplayControlSource> {
        self.control.source()
    }

    fn control_event(
        &mut self,
        delivery_tick: Tick,
    ) -> Result<TrafficTraceReplayControlEvent, TrafficTraceReplayControlError> {
        self.control.control_event(delivery_tick)
    }

    fn control_event_context(
        &mut self,
        delivery_tick: Tick,
    ) -> Result<TrafficTraceReplayControlEventContext, TrafficTraceReplayControlError> {
        self.control.control_event_context(delivery_tick)
    }

    fn has_control_action(&self) -> bool {
        self.control.has_control_action()
    }

    fn drop_current_control_source(&mut self) {
        self.control.drop_current_source();
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
    let Some(event) = traffic_trace_replay_controller_target_event(
        Arc::clone(&runtime),
        controller,
        delivery,
        context,
        retry_delay,
    )?
    else {
        return Ok(TargetOutcome::NoResponse);
    };

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

pub fn traffic_trace_replay_controller_runtime_target_outcome_parallel(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery: &RequestDelivery,
    context: &mut ParallelSchedulerContext<'_>,
) -> Result<TargetOutcome, TrafficTraceReplayControllerTargetError> {
    if !delivery.request().requires_response() {
        return Ok(TargetOutcome::NoResponse);
    }

    let event =
        traffic_trace_replay_controller_runtime_target_event(Arc::clone(&runtime), delivery)?;
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

pub fn traffic_trace_replay_controller_target_event(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    controller: Arc<Mutex<TrafficController>>,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
    retry_delay: Tick,
) -> Result<Option<TrafficTraceReplayTargetEvent>, TrafficTraceReplayControllerTargetError> {
    if !delivery.request().requires_response() {
        return Ok(None);
    }

    let controller_tick = runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .target_request_tick(delivery.request().id())
        .unwrap_or_else(|| delivery.tick());
    loop {
        match traffic_trace_replay_controller_runtime_target_event(Arc::clone(&runtime), delivery) {
            Ok(event) => return Ok(Some(event)),
            Err(TrafficTraceReplayTargetError::ActionQueueEmpty { .. }) => {}
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
                .drop_target_request(delivery.request().id());
            return Err(
                TrafficTraceReplayControllerTargetError::ReplayActionMissing {
                    request: delivery.request().id(),
                },
            );
        };

        let trace_exited = batch.trace_exit().is_some();
        let repeated_request = batch
            .request()
            .is_some_and(|request| request.request().id() == delivery.request().id());
        let target_action_available = {
            let mut runtime = runtime
                .lock()
                .expect("trace replay controller runtime lock");
            runtime.record_batch(&batch)?;
            runtime.has_target_action(delivery.request().id())
        };
        traffic_trace_replay_controller_runtime_sideband_events(
            Arc::clone(&runtime),
            context.now(),
            context,
        );
        if (trace_exited || repeated_request) && !target_action_available {
            runtime
                .lock()
                .expect("trace replay controller runtime lock")
                .drop_target_request(delivery.request().id());
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
    let event = traffic_trace_replay_controller_control_event(
        Arc::clone(&runtime),
        controller,
        delivery_tick,
        context,
        retry_delay,
    )?;
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

pub fn traffic_trace_replay_controller_runtime_control_completion_parallel(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery_tick: Tick,
    context: &mut ParallelSchedulerContext<'_>,
) -> Result<(), TrafficTraceReplayControllerControlError> {
    let event =
        traffic_trace_replay_controller_runtime_control_event(Arc::clone(&runtime), delivery_tick)?;
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

pub fn traffic_trace_replay_controller_control_event(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    controller: Arc<Mutex<TrafficController>>,
    delivery_tick: Tick,
    context: &mut SchedulerContext<'_>,
    retry_delay: Tick,
) -> Result<TrafficTraceReplayControlEvent, TrafficTraceReplayControllerControlError> {
    let controller_tick = runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .control_source_tick()
        .unwrap_or(delivery_tick);
    loop {
        match traffic_trace_replay_controller_runtime_control_event(
            Arc::clone(&runtime),
            delivery_tick,
        ) {
            Ok(event) => return Ok(event),
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
                .drop_current_control_source();
            return Err(
                TrafficTraceReplayControllerControlError::ReplayActionMissing { delivery_tick },
            );
        };

        let trace_exited = batch.trace_exit().is_some();
        let repeated_source = runtime
            .lock()
            .expect("trace replay controller runtime lock")
            .control_source()
            .is_some_and(|source| batch_has_control_source(&batch, source));
        let control_action_available = {
            let mut runtime = runtime
                .lock()
                .expect("trace replay controller runtime lock");
            runtime.record_batch(&batch)?;
            runtime.has_control_action()
        };
        traffic_trace_replay_controller_runtime_sideband_events(
            Arc::clone(&runtime),
            context.now(),
            context,
        );
        if (trace_exited || repeated_source) && !control_action_available {
            runtime
                .lock()
                .expect("trace replay controller runtime lock")
                .drop_current_control_source();
            return Err(
                TrafficTraceReplayControllerControlError::ReplayActionMissing { delivery_tick },
            );
        }
    }
}

fn batch_has_control_source(
    batch: &TrafficControllerEventBatch,
    source: TrafficTraceReplayControlSource,
) -> bool {
    batch.events().iter().any(|event| match (event, source) {
        (
            TrafficControllerEvent::TraceSync(event),
            TrafficTraceReplayControlSource::Sync(source),
        ) => *event == source,
        (TrafficControllerEvent::TraceHtm(event), TrafficTraceReplayControlSource::Htm(source)) => {
            *event == source
        }
        _ => false,
    })
}

fn traffic_trace_replay_controller_runtime_target_event(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery: &RequestDelivery,
) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
    runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .target_event(delivery)
}

pub(super) fn traffic_trace_replay_controller_runtime_target_event_context(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery: &RequestDelivery,
) -> Result<TrafficTraceReplayTargetEventContext, TrafficTraceReplayTargetError> {
    runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .target_event_context(delivery)
}

fn traffic_trace_replay_controller_runtime_control_event(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery_tick: Tick,
) -> Result<TrafficTraceReplayControlEvent, TrafficTraceReplayControlError> {
    runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .control_event(delivery_tick)
}

pub(super) fn traffic_trace_replay_controller_runtime_control_event_context(
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    delivery_tick: Tick,
) -> Result<TrafficTraceReplayControlEventContext, TrafficTraceReplayControlError> {
    runtime
        .lock()
        .expect("trace replay controller runtime lock")
        .control_event_context(delivery_tick)
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
pub struct TrafficTraceReplayControlEventContext {
    event: TrafficTraceReplayControlEvent,
    source: Option<TrafficTraceReplayControlSource>,
}

impl TrafficTraceReplayControlEventContext {
    const fn new(
        event: TrafficTraceReplayControlEvent,
        source: Option<TrafficTraceReplayControlSource>,
    ) -> Self {
        Self { event, source }
    }

    pub const fn event(&self) -> &TrafficTraceReplayControlEvent {
        &self.event
    }

    pub const fn trace_htm(&self) -> Option<TrafficTraceHtmEvent> {
        match self.source {
            Some(TrafficTraceReplayControlSource::Htm(event)) => Some(event),
            _ => None,
        }
    }

    pub const fn trace_sync(&self) -> Option<TrafficTraceSyncEvent> {
        match self.source {
            Some(TrafficTraceReplayControlSource::Sync(event)) => Some(event),
            _ => None,
        }
    }

    pub fn into_event(self) -> TrafficTraceReplayControlEvent {
        self.event
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
pub struct TrafficTraceReplayTargetEventContext {
    event: TrafficTraceReplayTargetEvent,
    trace_response: Option<TrafficTraceResponseEvent>,
    trace_error: Option<TrafficTraceErrorEvent>,
    trace_response_data: Option<Vec<u8>>,
}

impl TrafficTraceReplayTargetEventContext {
    pub fn new(
        event: TrafficTraceReplayTargetEvent,
        trace_response: Option<TrafficTraceResponseEvent>,
        trace_error: Option<TrafficTraceErrorEvent>,
    ) -> Self {
        Self::with_trace_response_data(event, trace_response, trace_error, None)
    }

    pub fn with_trace_response_data(
        event: TrafficTraceReplayTargetEvent,
        trace_response: Option<TrafficTraceResponseEvent>,
        trace_error: Option<TrafficTraceErrorEvent>,
        trace_response_data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            event,
            trace_response,
            trace_error,
            trace_response_data,
        }
    }

    pub const fn event(&self) -> &TrafficTraceReplayTargetEvent {
        &self.event
    }

    pub const fn trace_response(&self) -> Option<TrafficTraceResponseEvent> {
        self.trace_response
    }

    pub const fn trace_error(&self) -> Option<TrafficTraceErrorEvent> {
        self.trace_error
    }

    pub fn trace_response_data(&self) -> Option<&[u8]> {
        self.trace_response_data.as_deref()
    }

    pub fn into_event(self) -> TrafficTraceReplayTargetEvent {
        self.event
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
    if !matches!(action, TrafficTraceReplayAction::MemoryResponse { .. }) {
        return Err(TrafficTraceReplayTargetError::UnexpectedAction {
            request,
            action: action.clone(),
        });
    }
    let event = target_event_for_action(action, delivery)?;
    let TrafficTraceReplayTargetEvent::MemoryResponse(outcome) = event else {
        unreachable!("validated trace replay target outcome is a memory response")
    };
    pop_validated_memory_response(queue);
    Ok(outcome)
}

pub fn traffic_trace_replay_target_event(
    queue: &mut TrafficTraceReplayActionQueue,
    delivery: &RequestDelivery,
) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
    let request = delivery.request().id();
    let action = queue
        .peek_action()
        .ok_or(TrafficTraceReplayTargetError::ActionQueueEmpty { request })?;
    let event = target_event_for_action(action, delivery)?;
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(_) => {
            pop_validated_memory_response(queue);
        }
        TrafficTraceReplayTargetEvent::MemoryFailure { .. } => {
            pop_validated_memory_failure(queue);
        }
    }
    Ok(event)
}

fn target_event_for_action(
    action: &TrafficTraceReplayAction,
    delivery: &RequestDelivery,
) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
    let request = delivery.request().id();
    match action {
        TrafficTraceReplayAction::MemoryResponse { tick, response, .. } => {
            let response_request = response.request_id();
            if response_request != request {
                return Err(TrafficTraceReplayTargetError::RequestMismatch {
                    request,
                    response: response_request,
                });
            }
            let delay = memory_response_delay(request, delivery.tick(), *tick)?;
            Ok(TrafficTraceReplayTargetEvent::MemoryResponse(
                memory_response_outcome(delay, response.clone()),
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
                record: TrafficTraceMemoryFailureRecord::new(*tick, *failure),
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
