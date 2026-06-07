use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, Tick};
use rem6_memory::{MemoryRequestId, ResponseStatus};
use rem6_traffic::{
    TrafficController, TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceCacheKind,
    TrafficTraceDiagnosticKind, TrafficTraceHtmKind, TrafficTraceTlbKind,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport, RequestDelivery,
    ResponseDelivery, TargetOutcome,
};
use rem6_workload::{WorkloadRouteId, WorkloadTopology, WorkloadTrafficTraceReplaySummary};

use crate::{
    RiscvCluster, TrafficTraceReplayControllerParallelErrors,
    TrafficTraceReplayControllerParallelExecutor, TrafficTraceReplayControllerRuntime,
    TrafficTraceReplayOrder, TrafficTraceReplayScheduledSidebandEvent,
    TrafficTraceReplaySidebandEvent, TrafficTraceReplayTargetEvent,
    TrafficTraceReplayTargetEventContext,
};

use super::data_cache_backend::WorkloadDataCacheBackend;
use super::RiscvWorkloadReplayError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTrafficTraceReplay {
    controller: TrafficController,
    route: WorkloadRouteId,
    control_partition: PartitionId,
    retry_delay: Tick,
}

impl RiscvWorkloadTrafficTraceReplay {
    pub const fn new(
        controller: TrafficController,
        route: WorkloadRouteId,
        control_partition: PartitionId,
    ) -> Self {
        Self {
            controller,
            route,
            control_partition,
            retry_delay: 0,
        }
    }

    pub const fn with_retry_delay(mut self, retry_delay: Tick) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    pub const fn controller(&self) -> &TrafficController {
        &self.controller
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn control_partition(&self) -> PartitionId {
        self.control_partition
    }

    pub const fn retry_delay(&self) -> Tick {
        self.retry_delay
    }
}

pub(super) struct RiscvWorkloadScheduledTrafficTraceReplay {
    route: WorkloadRouteId,
    scheduled_count: usize,
    trace: MemoryTrace,
    response_deliveries: Arc<Mutex<Vec<ResponseDelivery>>>,
    executor: TrafficTraceReplayControllerParallelExecutor,
}

impl RiscvWorkloadScheduledTrafficTraceReplay {
    pub(super) fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub(super) fn errors(&self) -> TrafficTraceReplayControllerParallelErrors {
        self.executor.errors()
    }

    pub(super) fn summary(&self) -> WorkloadTrafficTraceReplaySummary {
        let runtime = self
            .executor
            .runtime()
            .lock()
            .expect("traffic trace replay runtime lock")
            .clone();
        let sideband_counts = traffic_trace_replay_sideband_counts(runtime.sideband_events());
        WorkloadTrafficTraceReplaySummary::new(self.route.clone(), self.scheduled_count)
            .with_response_delivery_count(
                self.response_deliveries
                    .lock()
                    .expect("traffic trace replay response lock")
                    .len(),
            )
            .with_memory_trace_event_count(self.trace.snapshot().len())
            .with_memory_failure_count(runtime.memory_failures().len())
            .with_control_ack_count(runtime.control_acks().len())
            .with_control_failure_count(runtime.control_failures().len())
            .with_sideband_event_count(runtime.sideband_events().len())
            .with_tlb_sync_event_count(sideband_counts.tlb_sync)
            .with_cache_flush_event_count(sideband_counts.cache_flush)
            .with_diagnostic_print_event_count(sideband_counts.diagnostic_print)
            .with_htm_abort_event_count(sideband_counts.htm_abort)
    }

    pub(super) fn into_outcome(self) -> RiscvWorkloadTrafficTraceReplayOutcome {
        RiscvWorkloadTrafficTraceReplayOutcome {
            route: self.route,
            scheduled_count: self.scheduled_count,
            runtime: self
                .executor
                .runtime()
                .lock()
                .expect("traffic trace replay runtime lock")
                .clone(),
            errors: self.executor.errors(),
            response_deliveries: self
                .response_deliveries
                .lock()
                .expect("traffic trace replay response lock")
                .clone(),
            memory_trace_events: self.trace.snapshot(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TrafficTraceReplaySidebandCounts {
    tlb_sync: usize,
    cache_flush: usize,
    diagnostic_print: usize,
    htm_abort: usize,
}

fn traffic_trace_replay_sideband_counts(
    events: &[TrafficTraceReplayScheduledSidebandEvent],
) -> TrafficTraceReplaySidebandCounts {
    events.iter().fold(
        TrafficTraceReplaySidebandCounts::default(),
        |mut counts, event| {
            match event.event() {
                TrafficTraceReplaySidebandEvent::Tlb(event) => match event.kind() {
                    TrafficTraceTlbKind::ExternalSync => counts.tlb_sync += 1,
                },
                TrafficTraceReplaySidebandEvent::Cache(event) => match event.kind() {
                    TrafficTraceCacheKind::Flush => counts.cache_flush += 1,
                },
                TrafficTraceReplaySidebandEvent::Diagnostic(event) => match event.kind() {
                    TrafficTraceDiagnosticKind::Print => counts.diagnostic_print += 1,
                },
                TrafficTraceReplaySidebandEvent::Htm(event) => match event.kind() {
                    TrafficTraceHtmKind::Request => {}
                    TrafficTraceHtmKind::Abort => counts.htm_abort += 1,
                },
            }
            counts
        },
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTrafficTraceReplayOutcome {
    route: WorkloadRouteId,
    scheduled_count: usize,
    runtime: TrafficTraceReplayControllerRuntime,
    errors: TrafficTraceReplayControllerParallelErrors,
    response_deliveries: Vec<ResponseDelivery>,
    memory_trace_events: Vec<MemoryTraceEvent>,
}

impl RiscvWorkloadTrafficTraceReplayOutcome {
    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    pub const fn runtime(&self) -> &TrafficTraceReplayControllerRuntime {
        &self.runtime
    }

    pub const fn errors(&self) -> &TrafficTraceReplayControllerParallelErrors {
        &self.errors
    }

    pub fn response_deliveries(&self) -> &[ResponseDelivery] {
        &self.response_deliveries
    }

    pub fn memory_trace_events(&self) -> &[MemoryTraceEvent] {
        &self.memory_trace_events
    }
}

pub(super) fn schedule_traffic_trace_replays(
    replays: &[RiscvWorkloadTrafficTraceReplay],
    topology: &WorkloadTopology,
    route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    cluster: &RiscvCluster,
) -> Result<Vec<RiscvWorkloadScheduledTrafficTraceReplay>, RiscvWorkloadReplayError> {
    let mut scheduled_replays = Vec::new();
    for replay in replays {
        let route = route_map.get(replay.route()).copied().ok_or_else(|| {
            RiscvWorkloadReplayError::MissingRoute {
                route: replay.route().clone(),
            }
        })?;
        let trace = MemoryTrace::new();
        let response_deliveries = Arc::new(Mutex::new(Vec::new()));
        let mut controller = replay.controller().clone();
        let start_batch = controller
            .start(scheduler.now())
            .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
        let data_cache_consumer =
            trace_data_cache_consumer(replay.route(), route, topology, data_cache, cluster);
        let executor =
            traffic_trace_replay_executor(controller, replay.retry_delay(), data_cache_consumer);

        let mut scheduled_count = schedule_workload_traffic_trace_batch(
            &executor,
            &start_batch,
            scheduler,
            transport,
            route,
            trace.clone(),
            replay.control_partition(),
            Arc::clone(&response_deliveries),
        )?;
        let response_log = Arc::clone(&response_deliveries);
        scheduled_count += executor
            .schedule_controller_parallel(
                scheduler,
                transport,
                route,
                trace.clone(),
                replay.control_partition(),
                move |delivery| {
                    response_log
                        .lock()
                        .expect("traffic trace replay response lock")
                        .push(delivery);
                },
            )
            .map_err(RiscvWorkloadReplayError::TrafficTraceReplay)?;
        scheduled_replays.push(RiscvWorkloadScheduledTrafficTraceReplay {
            route: replay.route().clone(),
            scheduled_count,
            trace,
            response_deliveries,
            executor,
        });
    }
    Ok(scheduled_replays)
}

fn traffic_trace_replay_executor(
    controller: TrafficController,
    retry_delay: Tick,
    data_cache: Option<WorkloadTraceDataCacheConsumer>,
) -> TrafficTraceReplayControllerParallelExecutor {
    let executor =
        TrafficTraceReplayControllerParallelExecutor::new(controller).with_retry_delay(retry_delay);
    let Some(data_cache) = data_cache else {
        return executor;
    };

    executor
        .with_target_request_sink({
            let data_cache = data_cache.clone();
            move |order, request| {
                data_cache.register_request(order, request);
            }
        })
        .with_target_event_sink({
            let data_cache = data_cache.clone();
            move |order, delivery, event_context| {
                data_cache.register_target_event(order, delivery.tick(), event_context);
            }
        })
        .with_target_completion_sink({
            let data_cache = data_cache.clone();
            move |order, delivery, event_context| {
                data_cache.complete_target_event(order, delivery, event_context);
            }
        })
        .with_sideband_sink(move |tick, event| {
            data_cache.record_sideband(tick, event);
        })
}

fn trace_data_cache_consumer(
    route: &WorkloadRouteId,
    memory_route: MemoryRouteId,
    topology: &WorkloadTopology,
    data_cache: &Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    cluster: &RiscvCluster,
) -> Option<WorkloadTraceDataCacheConsumer> {
    if !trace_route_uses_data_cache(route, topology)
        && !trace_route_uses_data_translation(route, topology)
    {
        return None;
    }
    Some(WorkloadTraceDataCacheConsumer::new(
        memory_route,
        data_cache.clone(),
        cluster.clone(),
    ))
}

fn trace_route_uses_data_cache(route: &WorkloadRouteId, topology: &WorkloadTopology) -> bool {
    if topology.riscv_data_cache().is_none() {
        return false;
    }
    topology
        .riscv_cores()
        .iter()
        .filter_map(|core| core.data_route())
        .any(|data_route| data_route == route)
        || topology
            .gpu_dma_copies()
            .iter()
            .any(|copy| copy.route() == route)
        || topology
            .accelerator_dma_copies()
            .iter()
            .any(|copy| copy.route() == route)
}

fn trace_route_uses_data_translation(route: &WorkloadRouteId, topology: &WorkloadTopology) -> bool {
    topology.riscv_cores().iter().any(|core| {
        core.data_translation().is_some()
            && core
                .data_route()
                .is_some_and(|data_route| data_route == route)
    })
}

#[derive(Clone)]
struct WorkloadTraceDataCacheConsumer {
    inner: Arc<Mutex<WorkloadTraceDataCacheConsumerInner>>,
}

impl WorkloadTraceDataCacheConsumer {
    fn new(
        route: MemoryRouteId,
        data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
        cluster: RiscvCluster,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(WorkloadTraceDataCacheConsumerInner {
                route,
                data_cache,
                cluster,
                pending_requests: BTreeSet::new(),
                pending_sidebands: Vec::new(),
            })),
        }
    }

    fn register_request(&self, order: TrafficTraceReplayOrder, _request: MemoryRequestId) {
        self.inner
            .lock()
            .expect("workload trace data cache consumer lock")
            .pending_requests
            .insert(order);
    }

    fn register_target_event(
        &self,
        request_order: TrafficTraceReplayOrder,
        now: Tick,
        event_context: &TrafficTraceReplayTargetEventContext,
    ) {
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner.replace_pending_request_order(
            request_order,
            target_event_order(request_order, event_context),
            now,
        );
    }

    fn complete_target_event(
        &self,
        request_order: TrafficTraceReplayOrder,
        delivery: &RequestDelivery,
        event_context: &TrafficTraceReplayTargetEventContext,
    ) {
        let completion_order = target_event_order(request_order, event_context);
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner.apply_sidebands_before(completion_order);
        if matches!(
            event_context.event(),
            TrafficTraceReplayTargetEvent::MemoryResponse(_)
        ) {
            let data_cache_response_status = target_event_response_status(event_context.event());
            if data_cache_response_status.is_some()
                && data_cache_response_status != Some(ResponseStatus::StoreConditionalFailed)
            {
                if let Some(data_cache) = inner.data_cache.as_ref() {
                    data_cache
                        .lock()
                        .expect("workload data cache lock")
                        .respond(delivery);
                }
            }
            if let Some(response) = event_context.trace_response() {
                inner.apply_trace_response(response);
            }
        }
        inner.pending_requests.remove(&completion_order);
        inner.apply_ready_sidebands(completion_order.tick().max(delivery.tick()));
    }

    fn record_sideband(&self, tick: Tick, event: TrafficTraceReplaySidebandEvent) {
        let mut inner = self
            .inner
            .lock()
            .expect("workload trace data cache consumer lock");
        inner
            .pending_sidebands
            .push(WorkloadTraceDataCacheSideband {
                tick,
                order: TrafficTraceReplayOrder::new(event.tick(), event.sequence()),
                event,
            });
        inner.apply_ready_sidebands(tick);
    }
}

struct WorkloadTraceDataCacheConsumerInner {
    route: MemoryRouteId,
    data_cache: Option<Arc<Mutex<WorkloadDataCacheBackend>>>,
    cluster: RiscvCluster,
    pending_requests: BTreeSet<TrafficTraceReplayOrder>,
    pending_sidebands: Vec<WorkloadTraceDataCacheSideband>,
}

impl WorkloadTraceDataCacheConsumerInner {
    fn replace_pending_request_order(
        &mut self,
        request_order: TrafficTraceReplayOrder,
        completion_order: TrafficTraceReplayOrder,
        now: Tick,
    ) {
        if self.pending_requests.remove(&request_order) {
            self.pending_requests.insert(completion_order);
        }
        self.apply_ready_sidebands(now);
    }

    fn apply_sidebands_before(&mut self, request_order: TrafficTraceReplayOrder) {
        while let Some(index) = self.next_sideband_index(|sideband| sideband.order <= request_order)
        {
            let sideband = self.pending_sidebands.remove(index);
            self.apply_sideband(sideband.tick, sideband.event);
        }
    }

    fn apply_ready_sidebands(&mut self, now: Tick) {
        while let Some(index) = self.next_sideband_index(|sideband| sideband.tick <= now) {
            let sideband = self.pending_sidebands.remove(index);
            self.apply_sideband(sideband.tick, sideband.event);
        }
    }

    fn next_sideband_index(
        &self,
        ready: impl Fn(&WorkloadTraceDataCacheSideband) -> bool,
    ) -> Option<usize> {
        self.pending_sidebands
            .iter()
            .enumerate()
            .filter(|(_, sideband)| ready(sideband) && !self.is_blocked(sideband.order))
            .min_by_key(|(_, sideband)| sideband.order)
            .map(|(index, _)| index)
    }

    fn is_blocked(&self, order: TrafficTraceReplayOrder) -> bool {
        self.pending_requests
            .iter()
            .next()
            .is_some_and(|request_order| *request_order < order)
    }

    fn apply_sideband(&mut self, tick: Tick, event: TrafficTraceReplaySidebandEvent) {
        match event {
            TrafficTraceReplaySidebandEvent::Cache(cache) => {
                if let Some(data_cache) = self.data_cache.as_ref() {
                    data_cache
                        .lock()
                        .expect("workload data cache lock")
                        .apply_trace_cache_event(cache);
                }
            }
            TrafficTraceReplaySidebandEvent::Diagnostic(diagnostic) => {
                if let Some(data_cache) = self.data_cache.as_ref() {
                    data_cache
                        .lock()
                        .expect("workload data cache lock")
                        .apply_trace_diagnostic_event(tick, diagnostic);
                }
            }
            TrafficTraceReplaySidebandEvent::Tlb(tlb) => {
                if matches!(tlb.kind(), TrafficTraceTlbKind::ExternalSync) {
                    self.cluster
                        .flush_data_translation_tlbs_for_data_route(self.route);
                }
            }
            TrafficTraceReplaySidebandEvent::Htm(_) => {}
        }
    }

    fn apply_trace_response(&mut self, event: rem6_traffic::TrafficTraceResponseEvent) {
        if let Some(data_cache) = self.data_cache.as_ref() {
            data_cache
                .lock()
                .expect("workload data cache lock")
                .apply_trace_response_event(event);
        }
    }
}

struct WorkloadTraceDataCacheSideband {
    tick: Tick,
    order: TrafficTraceReplayOrder,
    event: TrafficTraceReplaySidebandEvent,
}

fn target_event_order(
    request_order: TrafficTraceReplayOrder,
    event_context: &TrafficTraceReplayTargetEventContext,
) -> TrafficTraceReplayOrder {
    if let Some(response) = event_context.trace_response() {
        return TrafficTraceReplayOrder::new(response.tick(), response.sequence());
    }
    if let Some(error) = event_context.trace_error() {
        return TrafficTraceReplayOrder::new(error.tick(), error.sequence());
    }
    request_order
}

fn target_event_response_status(event: &TrafficTraceReplayTargetEvent) -> Option<ResponseStatus> {
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => {
            target_outcome_response_status(outcome)
        }
        TrafficTraceReplayTargetEvent::MemoryFailure { .. } => None,
    }
}

fn target_outcome_response_status(outcome: &TargetOutcome) -> Option<ResponseStatus> {
    match outcome {
        TargetOutcome::Respond(response) | TargetOutcome::RespondAfter { response, .. } => {
            Some(response.status())
        }
        TargetOutcome::NoResponse => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn schedule_workload_traffic_trace_batch(
    executor: &TrafficTraceReplayControllerParallelExecutor,
    batch: &TrafficControllerEventBatch,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    route: MemoryRouteId,
    trace: MemoryTrace,
    control_partition: PartitionId,
    response_deliveries: Arc<Mutex<Vec<ResponseDelivery>>>,
) -> Result<usize, RiscvWorkloadReplayError> {
    if batch.is_empty() {
        return Ok(0);
    }

    if batch.request().is_some() {
        return executor
            .submit_batch_request_parallel(
                batch,
                scheduler,
                transport,
                route,
                trace,
                move |delivery| {
                    response_deliveries
                        .lock()
                        .expect("traffic trace replay response lock")
                        .push(delivery);
                },
            )
            .map(usize::from)
            .map_err(RiscvWorkloadReplayError::TrafficTraceReplay);
    }

    if workload_traffic_trace_batch_requires_control_response(batch) {
        return executor
            .schedule_batch_control_parallel(batch, scheduler, control_partition)
            .map(usize::from)
            .map_err(RiscvWorkloadReplayError::TrafficTraceReplay);
    }

    executor
        .record_batch_parallel(
            batch,
            scheduler,
            control_partition,
            workload_traffic_trace_batch_replay_tick(batch),
        )
        .map_err(RiscvWorkloadReplayError::TrafficTraceReplay)
}

fn workload_traffic_trace_batch_requires_control_response(
    batch: &TrafficControllerEventBatch,
) -> bool {
    batch
        .trace_sync()
        .is_some_and(|sync| sync.requires_response())
        || batch.trace_htm().is_some_and(|htm| htm.requires_response())
}

fn workload_traffic_trace_batch_replay_tick(batch: &TrafficControllerEventBatch) -> Tick {
    batch
        .events()
        .iter()
        .map(workload_traffic_trace_event_tick)
        .min()
        .unwrap_or(0)
}

fn workload_traffic_trace_event_tick(event: &TrafficControllerEvent) -> Tick {
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
