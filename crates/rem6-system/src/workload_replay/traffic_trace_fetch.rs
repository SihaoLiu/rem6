use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvCore};
use rem6_kernel::{ParallelSchedulerContext, Tick};
use rem6_memory::{MemoryRequestId, MemoryResponse, ResponseStatus};
use rem6_traffic::{TrafficController, TrafficControllerEventBatch};
use rem6_transport::{MemoryRouteId, RequestDelivery, TargetOutcome};
use rem6_workload::{WorkloadRouteId, WorkloadTopology};

use crate::traffic_replay::traffic_trace_replay_controller_runtime_target_event_context;
use crate::{
    TrafficTraceReplayControllerParallelExecutor, TrafficTraceReplayControllerTargetError,
    TrafficTraceReplayOrder, TrafficTraceReplayTargetEvent,
};

use super::memory_backend::{memory_response, WorkloadMemoryBackend};
use super::traffic_trace::WorkloadTraceDataCacheConsumer;
use super::RiscvWorkloadReplayError;

#[derive(Clone, Default)]
pub(super) struct RiscvWorkloadTrafficTraceFetchRequests {
    orders: Arc<Mutex<BTreeMap<MemoryRequestId, TrafficTraceReplayOrder>>>,
}

impl RiscvWorkloadTrafficTraceFetchRequests {
    pub(super) fn record(&self, order: TrafficTraceReplayOrder, request: MemoryRequestId) {
        self.orders
            .lock()
            .expect("workload fetch trace request lock")
            .insert(request, order);
    }

    fn order_for(&self, request: MemoryRequestId) -> Option<TrafficTraceReplayOrder> {
        self.orders
            .lock()
            .expect("workload fetch trace request lock")
            .get(&request)
            .copied()
    }
}

#[derive(Clone)]
pub(super) struct RiscvWorkloadTrafficTraceFetchBinding {
    memory_route: MemoryRouteId,
    requests: RiscvWorkloadTrafficTraceFetchRequests,
    executor: TrafficTraceReplayControllerParallelExecutor,
    consumer: WorkloadTraceDataCacheConsumer,
}

impl RiscvWorkloadTrafficTraceFetchBinding {
    pub(super) fn new(
        memory_route: MemoryRouteId,
        requests: RiscvWorkloadTrafficTraceFetchRequests,
        executor: TrafficTraceReplayControllerParallelExecutor,
        consumer: WorkloadTraceDataCacheConsumer,
    ) -> Self {
        Self {
            memory_route,
            requests,
            executor,
            consumer,
        }
    }

    pub(super) const fn memory_route(&self) -> MemoryRouteId {
        self.memory_route
    }

    pub(super) fn executor(&self) -> &TrafficTraceReplayControllerParallelExecutor {
        &self.executor
    }

    fn fetch_response(
        &self,
        core: &RiscvCore,
        memory: &WorkloadMemoryBackend,
        delivery: RequestDelivery,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Result<Option<TargetOutcome>, TrafficTraceReplayControllerTargetError> {
        if !delivery.request().requires_response() {
            return Ok(Some(TargetOutcome::NoResponse));
        }

        let request_order = match self.requests.order_for(delivery.request().id()) {
            Some(order) => order,
            None => return Ok(None),
        };
        let runtime = self.executor.runtime();
        let event_context = traffic_trace_replay_controller_runtime_target_event_context(
            runtime.clone(),
            &delivery,
        )
        .map_err(TrafficTraceReplayControllerTargetError::Target)?;
        self.consumer
            .register_target_event(request_order, delivery.tick(), &event_context);
        let delay = event_context.event().target_delay();
        match event_context.event().clone() {
            TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => {
                let fetch_outcome = fetch_response_outcome(memory, &delivery, outcome);
                let consumer = self.consumer.clone();
                let completion_delivery = delivery.clone();
                let completion_context = event_context.clone();
                context
                    .schedule_local_after(delay, move |_context| {
                        consumer.complete_target_event(
                            request_order,
                            &completion_delivery,
                            &completion_context,
                        );
                    })
                    .expect("validated trace replay target completion delay");
                Ok(Some(fetch_outcome))
            }
            TrafficTraceReplayTargetEvent::MemoryFailure { record, .. } => {
                let request_id = record.failure().request_id();
                let route = delivery.route();
                let endpoint = delivery.endpoint().clone();
                let fetch_core = core.clone();
                let consumer = self.consumer.clone();
                let completion_delivery = delivery.clone();
                let completion_context = event_context.clone();
                context
                    .schedule_local_after(delay, move |context| {
                        let tick = context.now();
                        runtime
                            .lock()
                            .expect("trace replay controller runtime lock")
                            .record_memory_failure(tick, record);
                        fetch_core.record_fetch_failure(request_id, tick, route, endpoint);
                        consumer.complete_target_event(
                            request_order,
                            &completion_delivery,
                            &completion_context,
                        );
                    })
                    .expect("validated trace replay failure delay");
                Ok(Some(TargetOutcome::NoResponse))
            }
        }
    }
}

pub(super) fn traffic_trace_fetch_responder(
    cluster: &RiscvCluster,
    bindings: &[RiscvWorkloadTrafficTraceFetchBinding],
    memory: WorkloadMemoryBackend,
    cpu: CpuId,
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    let core = cluster.core(cpu).ok();
    let binding = core.as_ref().and_then(|core| {
        bindings
            .iter()
            .find(|binding| binding.memory_route() == core.fetch_route())
            .cloned()
    });
    move |delivery, context| {
        if let (Some(binding), Some(core)) = (binding.as_ref(), core.as_ref()) {
            match binding.fetch_response(core, &memory, delivery.clone(), context) {
                Ok(Some(outcome)) => return outcome,
                Ok(None) => return TargetOutcome::NoResponse,
                Err(error) => {
                    binding.executor().record_target_error(error);
                    return TargetOutcome::NoResponse;
                }
            }
        }
        memory_response(&memory, &delivery)
    }
}

fn fetch_response_outcome(
    memory: &WorkloadMemoryBackend,
    delivery: &RequestDelivery,
    outcome: TargetOutcome,
) -> TargetOutcome {
    match outcome {
        TargetOutcome::Respond(response) => {
            TargetOutcome::Respond(fetch_response_data(memory, delivery, response))
        }
        TargetOutcome::RespondAfter { delay, response } => TargetOutcome::RespondAfter {
            delay,
            response: fetch_response_data(memory, delivery, response),
        },
        TargetOutcome::NoResponse => TargetOutcome::NoResponse,
    }
}

fn fetch_response_data(
    memory: &WorkloadMemoryBackend,
    delivery: &RequestDelivery,
    response: MemoryResponse,
) -> MemoryResponse {
    if response.status() != ResponseStatus::Completed || !delivery.request().returns_data() {
        return response;
    }

    let request = delivery.request();
    let data = match memory.response_data(request) {
        Ok(data) => data,
        Err(_) => return response,
    };
    if data.len() as u64 != request.size().bytes() {
        return response;
    }
    MemoryResponse::completed(request, Some(data)).unwrap_or(response)
}

pub(super) fn trace_batch_riscv_fetch_request_order(
    route: &WorkloadRouteId,
    batch: &TrafficControllerEventBatch,
    topology: &WorkloadTopology,
) -> Option<TrafficTraceReplayOrder> {
    let request = batch.request()?;
    topology
        .riscv_cores()
        .iter()
        .any(|core| core.fetch_route() == route && request.request().range().contains(core.entry()))
        .then(|| TrafficTraceReplayOrder::new(request.tick(), request.sequence()))
}

pub(super) fn trace_controller_riscv_fetch_request_order(
    route: &WorkloadRouteId,
    controller: &TrafficController,
    tick: Tick,
    retry_delay: Tick,
    topology: &WorkloadTopology,
) -> Result<Option<TrafficTraceReplayOrder>, RiscvWorkloadReplayError> {
    let mut controller = controller.clone();
    loop {
        let Some(batch) = controller
            .next_event(tick, retry_delay)
            .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?
        else {
            return Ok(None);
        };
        if let Some(order) = trace_batch_riscv_fetch_request_order(route, &batch, topology) {
            return Ok(Some(order));
        }
        if batch.request().is_some() || batch.trace_exit().is_some() || batch.exit().is_some() {
            return Ok(None);
        }
        if batch.is_empty() {
            return Ok(None);
        }
    }
}
