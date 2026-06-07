use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, Tick};
use rem6_traffic::{TrafficController, TrafficControllerEvent, TrafficControllerEventBatch};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport, ResponseDelivery,
};
use rem6_workload::{WorkloadRouteId, WorkloadTrafficTraceReplaySummary};

use crate::{
    TrafficTraceReplayControllerParallelErrors, TrafficTraceReplayControllerParallelExecutor,
    TrafficTraceReplayControllerRuntime,
};

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
    route_map: &BTreeMap<WorkloadRouteId, MemoryRouteId>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
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
        let executor = TrafficTraceReplayControllerParallelExecutor::new(controller)
            .with_retry_delay(replay.retry_delay());

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
