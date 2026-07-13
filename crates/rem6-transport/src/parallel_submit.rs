use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, Tick};
use rem6_memory::MemoryRequest;

use crate::{
    MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    PreparedParallelTransaction, QosPriority, QosRequestorId, RequestDelivery, ResponseDelivery,
    TargetOutcome, TransportError,
};

impl MemoryTransport {
    pub(crate) fn schedule_prepared_parallel_batch(
        &self,
        scheduler: &mut PartitionedScheduler,
        start_tick: Tick,
        mut prepared: Vec<PreparedParallelTransaction>,
        first_hop_delays: Vec<Tick>,
    ) -> Result<Vec<PartitionEventId>, TransportError> {
        for (transaction, delay) in prepared.iter_mut().zip(first_hop_delays) {
            transaction.first_hop_delay = delay;
        }

        let mut events = Vec::with_capacity(prepared.len());
        for transaction in prepared {
            let PreparedParallelTransaction {
                route_id,
                route,
                request,
                trace,
                responder,
                response_sink,
                first_hop_delay,
                qos_requestor: _,
                qos_priority: _,
                response_qos,
            } = transaction;
            let source_partition = route.source_partition();
            let fabric = self.fabric.clone();
            let request_id = request.id();
            let event =
                scheduler.schedule_parallel_at(source_partition, start_tick, move |context| {
                    trace.record(MemoryTraceEvent::request(
                        context.now(),
                        route_id,
                        route.source().clone(),
                        MemoryTraceKind::RequestSent,
                        request_id,
                    ));

                    Self::schedule_parallel_request_hop_with_delay(
                        context,
                        route_id,
                        route,
                        0,
                        request,
                        trace,
                        fabric,
                        response_qos,
                        responder,
                        response_sink,
                        first_hop_delay,
                    );
                });
            match event {
                Ok(event) => events.push(event),
                Err(error) => {
                    for event in events.into_iter().rev() {
                        scheduler
                            .cancel_event(event)
                            .expect("events scheduled by this batch remain pending");
                    }
                    return Err(TransportError::Scheduler(error));
                }
            }
        }

        Ok(events)
    }

    pub fn submit_parallel<F, G>(
        &self,
        scheduler: &mut PartitionedScheduler,
        route_id: MemoryRouteId,
        request: MemoryRequest,
        trace: MemoryTrace,
        responder: F,
        response_sink: G,
    ) -> Result<PartitionEventId, TransportError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        self.submit_parallel_at(
            scheduler,
            scheduler.now(),
            route_id,
            request,
            trace,
            responder,
            response_sink,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_parallel_at<F, G>(
        &self,
        scheduler: &mut PartitionedScheduler,
        start_tick: Tick,
        route_id: MemoryRouteId,
        request: MemoryRequest,
        trace: MemoryTrace,
        responder: F,
        response_sink: G,
    ) -> Result<PartitionEventId, TransportError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let route = self
            .route(route_id)
            .cloned()
            .ok_or(TransportError::UnknownRoute { route: route_id })?;
        let source_partition = route.source_partition();
        self.validate_scheduler_route(scheduler, route_id, &route, start_tick)?;
        let fabric = self.fabric.clone();
        let response_qos = self.response_qos_context(
            QosRequestorId::new(request.id().agent().get()),
            QosPriority::new(0),
        );
        scheduler
            .schedule_parallel_at(source_partition, start_tick, move |context| {
                let request_id = request.id();
                trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    route.source().clone(),
                    MemoryTraceKind::RequestSent,
                    request_id,
                ));

                Self::schedule_parallel_request_hop(
                    context,
                    route_id,
                    route,
                    0,
                    request,
                    trace,
                    fabric,
                    response_qos,
                    responder,
                    response_sink,
                );
            })
            .map_err(TransportError::Scheduler)
    }
}
