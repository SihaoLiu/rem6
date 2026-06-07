use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, Tick};
use rem6_memory::MemoryRequest;

use crate::{
    MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    RequestDelivery, ResponseDelivery, TargetOutcome, TransportError,
};

impl MemoryTransport {
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
                    responder,
                    response_sink,
                );
            })
            .map_err(TransportError::Scheduler)
    }
}
