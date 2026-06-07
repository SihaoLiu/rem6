use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerError, Tick};
use rem6_memory::MemoryRequestId;
use rem6_traffic::{
    TrafficController, TrafficControllerEvent, TrafficControllerEventBatch, TrafficGeneratorError,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, ResponseDelivery, TargetOutcome,
    TransportError,
};

use super::{
    traffic_trace_replay_controller_runtime_control_event_context,
    traffic_trace_replay_controller_runtime_target_event_context, TrafficTraceReplayControlEvent,
    TrafficTraceReplayControlEventContext, TrafficTraceReplayControllerControlError,
    TrafficTraceReplayControllerRuntime, TrafficTraceReplayControllerTargetError,
    TrafficTraceReplaySidebandEvent, TrafficTraceReplayTargetEvent,
    TrafficTraceReplayTargetEventContext,
};

type ScheduledSideband = (Tick, TrafficTraceReplaySidebandEvent);
type SidebandSink = Arc<dyn Fn(Tick, TrafficTraceReplaySidebandEvent) + Send + Sync>;
type TargetRequestSink = Arc<dyn Fn(TrafficTraceReplayOrder, MemoryRequestId) + Send + Sync>;
type TargetSink = Arc<dyn Fn(TrafficTraceReplayOrder, &RequestDelivery) + Send + Sync>;
type TargetEventSink = Arc<
    dyn Fn(TrafficTraceReplayOrder, &RequestDelivery, &TrafficTraceReplayTargetEventContext)
        + Send
        + Sync,
>;
type TargetCompletionSink = Arc<
    dyn Fn(TrafficTraceReplayOrder, &RequestDelivery, &TrafficTraceReplayTargetEventContext)
        + Send
        + Sync,
>;
type ControlCompletionSink =
    Arc<dyn Fn(Tick, &TrafficTraceReplayControlEventContext) + Send + Sync>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TrafficTraceReplayOrder {
    tick: Tick,
    sequence: u64,
}

impl TrafficTraceReplayOrder {
    pub const fn new(tick: Tick, sequence: u64) -> Self {
        Self { tick, sequence }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

#[derive(Clone)]
pub struct TrafficTraceReplayControllerParallelExecutor {
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    controller: Arc<Mutex<TrafficController>>,
    errors: Arc<Mutex<TrafficTraceReplayControllerParallelErrors>>,
    retry_delay: Tick,
    sideband_sink: Option<SidebandSink>,
    target_request_sink: Option<TargetRequestSink>,
    target_sink: Option<TargetSink>,
    target_event_sink: Option<TargetEventSink>,
    target_completion_sink: Option<TargetCompletionSink>,
    control_completion_sink: Option<ControlCompletionSink>,
}

impl TrafficTraceReplayControllerParallelExecutor {
    pub fn new(controller: TrafficController) -> Self {
        Self {
            runtime: Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default())),
            controller: Arc::new(Mutex::new(controller)),
            errors: Arc::new(Mutex::new(
                TrafficTraceReplayControllerParallelErrors::default(),
            )),
            retry_delay: 0,
            sideband_sink: None,
            target_request_sink: None,
            target_sink: None,
            target_event_sink: None,
            target_completion_sink: None,
            control_completion_sink: None,
        }
    }

    pub const fn with_retry_delay(mut self, retry_delay: Tick) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    pub fn with_sideband_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(Tick, TrafficTraceReplaySidebandEvent) + Send + Sync + 'static,
    {
        self.sideband_sink = Some(Arc::new(sink));
        self
    }

    pub fn with_target_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(TrafficTraceReplayOrder, &RequestDelivery) + Send + Sync + 'static,
    {
        self.target_sink = Some(Arc::new(sink));
        self
    }

    pub fn with_target_event_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(TrafficTraceReplayOrder, &RequestDelivery, &TrafficTraceReplayTargetEventContext)
            + Send
            + Sync
            + 'static,
    {
        self.target_event_sink = Some(Arc::new(sink));
        self
    }

    pub fn with_target_completion_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(TrafficTraceReplayOrder, &RequestDelivery, &TrafficTraceReplayTargetEventContext)
            + Send
            + Sync
            + 'static,
    {
        self.target_completion_sink = Some(Arc::new(sink));
        self
    }

    pub fn with_target_request_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(TrafficTraceReplayOrder, MemoryRequestId) + Send + Sync + 'static,
    {
        self.target_request_sink = Some(Arc::new(sink));
        self
    }

    pub fn with_control_completion_sink<F>(mut self, sink: F) -> Self
    where
        F: Fn(Tick, &TrafficTraceReplayControlEventContext) + Send + Sync + 'static,
    {
        self.control_completion_sink = Some(Arc::new(sink));
        self
    }

    pub fn runtime(&self) -> Arc<Mutex<TrafficTraceReplayControllerRuntime>> {
        Arc::clone(&self.runtime)
    }

    pub fn errors(&self) -> TrafficTraceReplayControllerParallelErrors {
        self.errors
            .lock()
            .expect("trace replay controller parallel error lock")
            .clone()
    }

    pub fn schedule_controller_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        route: MemoryRouteId,
        trace: MemoryTrace,
        control_partition: PartitionId,
        response_sink: F,
    ) -> Result<usize, TrafficTraceReplayControllerParallelSubmitError>
    where
        F: Fn(ResponseDelivery) + Send + Sync + 'static,
    {
        let response_sink = Arc::new(response_sink);
        let mut scheduled = 0;
        loop {
            let checkpoint = self
                .controller
                .lock()
                .expect("traffic controller lock")
                .snapshot();
            let batch_tick = self.controller_batch_tick(scheduler.now());
            let batch = match self.next_controller_batch(batch_tick) {
                Ok(batch) => batch,
                Err(error) => {
                    self.restore_controller(checkpoint)?;
                    return Err(error.into());
                }
            };
            let Some(batch) = batch else {
                return Ok(scheduled);
            };

            if batch.is_empty() {
                return Ok(scheduled);
            }
            let trace_exited = batch.trace_exit().is_some();
            let batch_scheduled = match if batch.request().is_some() {
                let sink = Arc::clone(&response_sink);
                self.submit_batch_request_parallel(
                    &batch,
                    scheduler,
                    transport,
                    route,
                    trace.clone(),
                    move |delivery| (*sink)(delivery),
                )
                .map(usize::from)
            } else if batch_requires_control_response(&batch) {
                self.schedule_batch_control_parallel(&batch, scheduler, control_partition)
                    .map(usize::from)
            } else {
                self.record_batch_parallel(
                    &batch,
                    scheduler,
                    control_partition,
                    batch_replay_tick(&batch),
                )
            } {
                Ok(scheduled) => scheduled,
                Err(error) => {
                    self.restore_controller(checkpoint)?;
                    return Err(error);
                }
            };
            scheduled += batch_scheduled;
            if trace_exited {
                return Ok(scheduled);
            }
        }
    }

    fn next_controller_batch(
        &self,
        tick: Tick,
    ) -> Result<Option<TrafficControllerEventBatch>, TrafficGeneratorError> {
        self.controller
            .lock()
            .expect("traffic controller lock")
            .next_event(tick, self.retry_delay)
    }

    fn controller_batch_tick(&self, scheduler_tick: Tick) -> Tick {
        self.runtime
            .lock()
            .expect("trace replay controller runtime lock")
            .control_source_tick()
            .unwrap_or(scheduler_tick)
    }

    fn restore_controller(
        &self,
        checkpoint: rem6_traffic::TrafficControllerSnapshot,
    ) -> Result<(), TrafficGeneratorError> {
        *self.controller.lock().expect("traffic controller lock") =
            TrafficController::restore(checkpoint)?;
        Ok(())
    }

    pub fn record_batch_parallel(
        &self,
        batch: &TrafficControllerEventBatch,
        scheduler: &mut PartitionedScheduler,
        partition: PartitionId,
        delivery_tick: Tick,
    ) -> Result<usize, TrafficTraceReplayControllerParallelSubmitError> {
        let staged = self.staged_runtime(batch)?;
        let (staged, sidebands) =
            prepare_sidebands_from_runtime(staged, scheduler, partition, delivery_tick)?;
        let scheduled = sidebands.len();
        self.schedule_prepared_sidebands(scheduler, partition, sidebands)?;
        self.commit_runtime(staged);
        Ok(scheduled)
    }

    pub fn submit_batch_request_parallel<G>(
        &self,
        batch: &TrafficControllerEventBatch,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        route: MemoryRouteId,
        trace: MemoryTrace,
        response_sink: G,
    ) -> Result<bool, TrafficTraceReplayControllerParallelSubmitError>
    where
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let Some(request_event) = batch.request() else {
            return Ok(false);
        };
        let request_tick = request_event.tick();
        let request_order = TrafficTraceReplayOrder::new(request_tick, request_event.sequence());
        let request = request_event.request().clone();
        let request_id = request.id();
        let staged = self.staged_runtime(batch)?;
        let (staged, sidebands) = prepare_sidebands_from_runtime(
            staged,
            scheduler,
            route_source_partition(transport, route)?,
            request_tick,
        )?;

        let runtime = Arc::clone(&self.runtime);
        let errors = Arc::clone(&self.errors);
        let target_sink = self.target_sink.clone();
        let target_event_sink = self.target_event_sink.clone();
        let target_completion_sink = self.target_completion_sink.clone();
        transport.submit_parallel_at(
            scheduler,
            request_tick,
            route,
            request,
            trace,
            move |delivery, context| {
                if !delivery.request().requires_response() {
                    let event_context = TrafficTraceReplayTargetEventContext::new(
                        TrafficTraceReplayTargetEvent::MemoryResponse(TargetOutcome::NoResponse),
                        None,
                        None,
                    );
                    if let Some(target_sink) = &target_sink {
                        target_sink(request_order, &delivery);
                    }
                    if let Some(target_event_sink) = &target_event_sink {
                        target_event_sink(request_order, &delivery, &event_context);
                    }
                    if let Some(target_completion_sink) = target_completion_sink.clone() {
                        target_completion_sink(request_order, &delivery, &event_context);
                    }
                    return TargetOutcome::NoResponse;
                }
                match traffic_trace_replay_controller_runtime_target_event_context(
                    Arc::clone(&runtime),
                    &delivery,
                ) {
                    Ok(event_context) => {
                        if let Some(target_sink) = &target_sink {
                            target_sink(request_order, &delivery);
                        }
                        if let Some(target_event_sink) = &target_event_sink {
                            target_event_sink(request_order, &delivery, &event_context);
                        }
                        if let Some(target_completion_sink) = target_completion_sink.clone() {
                            let completion_delivery = delivery.clone();
                            let completion_context = event_context.clone();
                            let delay = event_context.event().target_delay();
                            context
                                .schedule_local_after(delay, move |_context| {
                                    target_completion_sink(
                                        request_order,
                                        &completion_delivery,
                                        &completion_context,
                                    );
                                })
                                .expect("validated trace replay target completion delay");
                        }
                        target_outcome_for_event(
                            event_context.into_event(),
                            Arc::clone(&runtime),
                            context,
                        )
                    }
                    Err(error) => {
                        errors
                            .lock()
                            .expect("trace replay controller parallel error lock")
                            .record_target(error.into());
                        TargetOutcome::NoResponse
                    }
                }
            },
            response_sink,
        )?;
        if request_event.request().requires_response() {
            if let Some(target_request_sink) = &self.target_request_sink {
                target_request_sink(request_order, request_id);
            }
        }

        self.schedule_prepared_sidebands(
            scheduler,
            route_source_partition(transport, route)?,
            sidebands,
        )?;
        self.commit_runtime(staged);
        Ok(true)
    }

    pub fn schedule_batch_control_parallel(
        &self,
        batch: &TrafficControllerEventBatch,
        scheduler: &mut PartitionedScheduler,
        partition: PartitionId,
    ) -> Result<bool, TrafficTraceReplayControllerParallelSubmitError> {
        let delivery_tick = batch
            .trace_sync()
            .filter(|sync| sync.requires_response())
            .map(|sync| sync.tick())
            .or_else(|| {
                batch
                    .trace_htm()
                    .filter(|htm| htm.requires_response())
                    .map(|htm| htm.tick())
            });
        let Some(delivery_tick) = delivery_tick else {
            return self
                .record_batch_parallel(batch, scheduler, partition, batch_replay_tick(batch))
                .map(|scheduled| scheduled != 0);
        };

        let staged = self.staged_runtime(batch)?;
        let (staged, sidebands) =
            prepare_sidebands_from_runtime(staged, scheduler, partition, delivery_tick)?;
        let runtime = Arc::clone(&self.runtime);
        let errors = Arc::clone(&self.errors);
        let control_completion_sink = self.control_completion_sink.clone();
        scheduler.schedule_parallel_at(partition, delivery_tick, move |context| {
            let event_context = match traffic_trace_replay_controller_runtime_control_event_context(
                Arc::clone(&runtime),
                context.now(),
            ) {
                Ok(event_context) => event_context,
                Err(error) => {
                    errors
                        .lock()
                        .expect("trace replay controller parallel error lock")
                        .record_control(error.into());
                    return;
                }
            };
            match event_context.event() {
                TrafficTraceReplayControlEvent::ControlAck { delay, trace_tick } => {
                    let delay = *delay;
                    let trace_tick = *trace_tick;
                    let runtime = Arc::clone(&runtime);
                    let event_context = event_context.clone();
                    let control_completion_sink = control_completion_sink.clone();
                    context
                        .schedule_local_after(delay, move |context| {
                            runtime
                                .lock()
                                .expect("trace replay controller runtime lock")
                                .record_control_ack(context.now(), trace_tick);
                            if let Some(control_completion_sink) = &control_completion_sink {
                                control_completion_sink(context.now(), &event_context);
                            }
                        })
                        .expect("validated trace replay control ack delay");
                }
                TrafficTraceReplayControlEvent::ControlFailure { delay, record } => {
                    let delay = *delay;
                    let record = *record;
                    let runtime = Arc::clone(&runtime);
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
        })?;

        self.schedule_prepared_sidebands(scheduler, partition, sidebands)?;
        self.commit_runtime(staged);
        Ok(true)
    }

    fn staged_runtime(
        &self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<TrafficTraceReplayControllerRuntime, TrafficGeneratorError> {
        let mut staged = self
            .runtime
            .lock()
            .expect("trace replay controller runtime lock")
            .clone();
        staged.record_batch(batch)?;
        Ok(staged)
    }

    fn commit_runtime(&self, staged: TrafficTraceReplayControllerRuntime) {
        *self
            .runtime
            .lock()
            .expect("trace replay controller runtime lock") = staged;
    }

    fn schedule_prepared_sidebands(
        &self,
        scheduler: &mut PartitionedScheduler,
        partition: PartitionId,
        sidebands: Vec<ScheduledSideband>,
    ) -> Result<(), SchedulerError> {
        for (tick, event) in sidebands {
            let runtime = Arc::clone(&self.runtime);
            let sideband_sink = self.sideband_sink.clone();
            scheduler.schedule_parallel_at(partition, tick, move |context| {
                if let Some(sideband_sink) = &sideband_sink {
                    sideband_sink(context.now(), event);
                }
                runtime
                    .lock()
                    .expect("trace replay controller runtime lock")
                    .record_sideband_event(context.now(), event);
            })?;
        }
        Ok(())
    }
}

fn route_source_partition(
    transport: &MemoryTransport,
    route: MemoryRouteId,
) -> Result<PartitionId, TransportError> {
    transport
        .route(route)
        .map(|route| route.source_partition())
        .ok_or(TransportError::UnknownRoute { route })
}

fn prepare_sidebands_from_runtime(
    mut staged: TrafficTraceReplayControllerRuntime,
    scheduler: &PartitionedScheduler,
    partition: PartitionId,
    delivery_tick: Tick,
) -> Result<(TrafficTraceReplayControllerRuntime, Vec<ScheduledSideband>), SchedulerError> {
    let mut sidebands = Vec::new();
    while let Some(completion) = staged.next_sideband_event(delivery_tick) {
        let tick =
            delivery_tick
                .checked_add(completion.delay())
                .ok_or(SchedulerError::TickOverflow {
                    now: delivery_tick,
                    delay: completion.delay(),
                })?;
        sidebands.push((tick, completion.event()));
    }

    if sidebands.is_empty() {
        return Ok((staged, sidebands));
    }

    let partition_now = scheduler.partition_now(partition)?;
    for (tick, _) in &sidebands {
        if *tick < partition_now {
            return Err(SchedulerError::InThePast {
                partition,
                now: partition_now,
                requested: *tick,
            });
        }
    }
    Ok((staged, sidebands))
}

fn target_outcome_for_event(
    event: TrafficTraceReplayTargetEvent,
    runtime: Arc<Mutex<TrafficTraceReplayControllerRuntime>>,
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
) -> TargetOutcome {
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => outcome,
        TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay controller runtime lock")
                        .record_memory_failure(context.now(), record);
                })
                .expect("validated trace replay failure delay");
            TargetOutcome::NoResponse
        }
    }
}

fn batch_requires_control_response(batch: &TrafficControllerEventBatch) -> bool {
    batch
        .trace_sync()
        .is_some_and(|sync| sync.requires_response())
        || batch.trace_htm().is_some_and(|htm| htm.requires_response())
}

fn batch_replay_tick(batch: &TrafficControllerEventBatch) -> Tick {
    batch
        .events()
        .iter()
        .map(event_replay_tick)
        .min()
        .unwrap_or(0)
}

fn event_replay_tick(event: &TrafficControllerEvent) -> Tick {
    match event {
        TrafficControllerEvent::Request(request) => request.tick(),
        TrafficControllerEvent::Transition(transition) => transition.tick(),
        TrafficControllerEvent::Exit(exit) => exit.tick(),
        TrafficControllerEvent::TraceExit(_) => Tick::MAX,
        TrafficControllerEvent::TraceSync(sync) => sync.tick(),
        TrafficControllerEvent::TraceTlb(tlb) => tlb.tick(),
        TrafficControllerEvent::TraceCache(cache) => cache.tick(),
        TrafficControllerEvent::TraceHtm(htm) => htm.tick(),
        TrafficControllerEvent::TraceDiagnostic(diagnostic) => diagnostic.tick(),
        TrafficControllerEvent::TraceResponse(response) => response.tick(),
        TrafficControllerEvent::TraceError(error) => error.tick(),
        TrafficControllerEvent::TraceResponseMatch(response) => response.response().tick(),
        TrafficControllerEvent::TraceErrorMatch(error) => error.error().tick(),
        TrafficControllerEvent::TraceReplayAction(action) => action.tick(),
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayControllerParallelErrors {
    target: Vec<TrafficTraceReplayControllerTargetError>,
    control: Vec<TrafficTraceReplayControllerControlError>,
}

impl TrafficTraceReplayControllerParallelErrors {
    pub fn target(&self) -> &[TrafficTraceReplayControllerTargetError] {
        &self.target
    }

    pub fn control(&self) -> &[TrafficTraceReplayControllerControlError] {
        &self.control
    }

    pub fn is_empty(&self) -> bool {
        self.target.is_empty() && self.control.is_empty()
    }

    fn record_target(&mut self, error: TrafficTraceReplayControllerTargetError) {
        self.target.push(error);
    }

    fn record_control(&mut self, error: TrafficTraceReplayControllerControlError) {
        self.control.push(error);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayControllerParallelSubmitError {
    Generator(TrafficGeneratorError),
    Scheduler(SchedulerError),
    Transport(TransportError),
}

impl fmt::Display for TrafficTraceReplayControllerParallelSubmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generator(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TrafficTraceReplayControllerParallelSubmitError {}

impl From<TrafficGeneratorError> for TrafficTraceReplayControllerParallelSubmitError {
    fn from(error: TrafficGeneratorError) -> Self {
        Self::Generator(error)
    }
}

impl From<SchedulerError> for TrafficTraceReplayControllerParallelSubmitError {
    fn from(error: SchedulerError) -> Self {
        Self::Scheduler(error)
    }
}

impl From<TransportError> for TrafficTraceReplayControllerParallelSubmitError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

#[cfg(test)]
mod tests {
    use rem6_memory::{AgentId, CacheLineLayout};
    use rem6_traffic::{
        TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
        TrafficStateGeneratorSnapshot, TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec,
        TrafficTrace, TrafficTraceConfig, TrafficTraceGenerator, TrafficTransition,
        TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
    };

    use super::*;

    const TICK_FREQUENCY: u64 = 1_000;

    #[test]
    fn schedule_controller_parallel_restores_after_next_event_errors() {
        let mut controller = transition_overflow_controller();
        assert!(controller.start(u64::MAX).unwrap().is_empty());
        let executor = TrafficTraceReplayControllerParallelExecutor::new(controller);
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        scheduler
            .schedule_at(PartitionId::new(0), u64::MAX, |_| {})
            .unwrap();
        scheduler.run_until_idle();
        assert_eq!(scheduler.now(), u64::MAX);

        let err = executor
            .schedule_controller_parallel(
                &mut scheduler,
                &MemoryTransport::new(),
                MemoryRouteId::new(0),
                MemoryTrace::new(),
                PartitionId::new(0),
                |_| {},
            )
            .unwrap_err();
        assert!(matches!(
            err,
            TrafficTraceReplayControllerParallelSubmitError::Generator(_)
        ));

        let snapshot = executor
            .controller
            .lock()
            .expect("traffic controller lock")
            .snapshot();
        let state = snapshot
            .generators()
            .iter()
            .find(|entry| entry.id() == TrafficStateId::new(0))
            .expect("trace state snapshot");
        let TrafficStateGeneratorSnapshot::Trace(trace) = state.generator() else {
            panic!("expected trace generator snapshot");
        };
        assert!(trace.active());
    }

    fn transition_overflow_controller() -> TrafficController {
        let trace = TrafficTrace::from_gem5_packet_trace(
            &empty_gem5_packet_trace(TICK_FREQUENCY),
            TICK_FREQUENCY,
        )
        .unwrap();
        let trace_config = TrafficTraceConfig::new(
            AgentId::new(7),
            CacheLineLayout::new(64).unwrap(),
            99,
            trace,
        )
        .unwrap();
        let graph = TrafficStateGraphConfig::new(
            vec![
                TrafficStateSpec::new(TrafficStateId::new(0), u64::MAX),
                TrafficStateSpec::new(TrafficStateId::new(1), 1),
            ],
            TrafficStateId::new(0),
            vec![
                TrafficTransition::new(
                    TrafficStateId::new(0),
                    TrafficStateId::new(1),
                    TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
                        .unwrap(),
                ),
                TrafficTransition::new(
                    TrafficStateId::new(1),
                    TrafficStateId::new(1),
                    TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
                        .unwrap(),
                ),
            ],
        )
        .unwrap();
        let config = TrafficControllerConfig::new(
            graph,
            vec![
                TrafficControllerState::new(
                    TrafficStateId::new(0),
                    TrafficStateGenerator::Trace(TrafficTraceGenerator::new(trace_config)),
                ),
                TrafficControllerState::new(
                    TrafficStateId::new(1),
                    TrafficStateGenerator::Trace(TrafficTraceGenerator::new(
                        TrafficTraceConfig::new(
                            AgentId::new(7),
                            CacheLineLayout::new(64).unwrap(),
                            99,
                            TrafficTrace::from_gem5_packet_trace(
                                &empty_gem5_packet_trace(TICK_FREQUENCY),
                                TICK_FREQUENCY,
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                    )),
                ),
            ],
        )
        .unwrap();
        TrafficController::new(config)
    }

    fn empty_gem5_packet_trace(tick_frequency: u64) -> Vec<u8> {
        let mut bytes = vec![0x67, 0x65, 0x6d, 0x35];
        let mut header = Vec::new();
        append_key(&mut header, 3, 0);
        append_varint(&mut header, tick_frequency);
        append_record(&mut bytes, &header);
        bytes
    }

    fn append_record(bytes: &mut Vec<u8>, message: &[u8]) {
        append_varint(bytes, message.len() as u64);
        bytes.extend_from_slice(message);
    }

    fn append_key(bytes: &mut Vec<u8>, field: u32, wire_type: u8) {
        append_varint(bytes, (u64::from(field) << 3) | u64::from(wire_type));
    }

    fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
        while value >= 0x80 {
            bytes.push((value as u8) | 0x80);
            value >>= 7;
        }
        bytes.push(value as u8);
    }
}
