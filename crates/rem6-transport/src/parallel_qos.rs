use std::collections::BTreeMap;

use rem6_fabric::{QosQueuedRequest, QosRequestId, QosRequestorId};
use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError, Tick};

use crate::{
    MemoryTraceEvent, MemoryTraceKind, MemoryTransport, PreparedParallelTransaction,
    RequestDelivery, TargetOutcome, TransportEndpointId, TransportError,
};

impl MemoryTransport {
    pub(crate) fn can_submit_direct_qos_parallel_batch(
        &self,
        transactions: &[PreparedParallelTransaction],
    ) -> bool {
        self.qos_arbiter.is_some()
            && transactions.len() > 1
            && transactions.iter().all(|transaction| {
                transaction
                    .route
                    .hops()
                    .iter()
                    .all(|hop| hop.request_fabric_path().is_none())
            })
    }

    pub(crate) fn submit_direct_qos_parallel_batch(
        &self,
        scheduler: &mut PartitionedScheduler,
        start_tick: Tick,
        transactions: Vec<PreparedParallelTransaction>,
    ) -> Result<Vec<PartitionEventId>, TransportError> {
        let mut events = vec![None; transactions.len()];
        let mut groups = BTreeMap::<
            (Tick, PartitionId, TransportEndpointId),
            Vec<(usize, PreparedParallelTransaction)>,
        >::new();
        for (index, transaction) in transactions.into_iter().enumerate() {
            let arrival_tick = start_tick
                .checked_add(transaction.route.request_latency())
                .ok_or(TransportError::Scheduler(SchedulerError::TickOverflow {
                    now: start_tick,
                    delay: transaction.route.request_latency(),
                }))?;
            groups
                .entry((
                    arrival_tick,
                    transaction.route.target_partition(),
                    transaction.route.target().clone(),
                ))
                .or_default()
                .push((index, transaction));
        }

        for ((arrival_tick, target_partition, _), group) in groups {
            for (index, transaction) in self.order_direct_qos_group(group) {
                let event = self.schedule_direct_parallel_target_event(
                    scheduler,
                    start_tick,
                    arrival_tick,
                    target_partition,
                    transaction,
                )?;
                events[index] = Some(event);
            }
        }

        Ok(events
            .into_iter()
            .map(|event| event.expect("every direct QoS transaction schedules one event"))
            .collect())
    }

    fn order_direct_qos_group(
        &self,
        group: Vec<(usize, PreparedParallelTransaction)>,
    ) -> Vec<(usize, PreparedParallelTransaction)> {
        let Some(arbiter) = &self.qos_arbiter else {
            return group;
        };
        let mut arbiter = arbiter.lock().expect("QoS arbiter lock");
        let mut pending = group;
        let mut ordered = Vec::with_capacity(pending.len());

        while !pending.is_empty() {
            let queue = pending
                .iter()
                .enumerate()
                .map(|(order, (_, transaction))| {
                    QosQueuedRequest::new(
                        QosRequestId::new(transaction.request.id().sequence()),
                        QosRequestorId::new(transaction.request.id().agent().get()),
                        transaction.qos_priority,
                        transaction.request.size().bytes(),
                        order as u64,
                    )
                    .expect("memory requests always have nonzero size")
                })
                .collect::<Vec<_>>();
            let grant = arbiter
                .grant(&queue)
                .expect("nonempty direct QoS queue must produce a grant");
            ordered.push(pending.remove(grant.queue_index()));
        }

        ordered
    }

    fn schedule_direct_parallel_target_event(
        &self,
        scheduler: &mut PartitionedScheduler,
        start_tick: Tick,
        arrival_tick: Tick,
        target_partition: PartitionId,
        transaction: PreparedParallelTransaction,
    ) -> Result<PartitionEventId, TransportError> {
        let PreparedParallelTransaction {
            route_id,
            route,
            request,
            trace,
            responder,
            response_sink,
            first_hop_delay: _,
            qos_priority: _,
        } = transaction;
        let request_id = request.id();
        let last_hop_index = route.hops().len() - 1;
        let fabric = self.fabric.clone();
        record_direct_request_trace(&trace, start_tick, route_id, &route, request_id)?;

        scheduler
            .schedule_parallel_at(target_partition, arrival_tick, move |context| {
                let hop = route.hops()[last_hop_index].clone();
                let delivery = RequestDelivery {
                    tick: context.now(),
                    route: route_id,
                    endpoint: hop.endpoint().clone(),
                    request,
                };

                match responder(delivery, context) {
                    TargetOutcome::Respond(response) => {
                        Self::schedule_parallel_response_hop(
                            context,
                            route_id,
                            route,
                            last_hop_index,
                            response,
                            trace,
                            fabric,
                            response_sink,
                        );
                    }
                    TargetOutcome::RespondAfter { delay, response } => {
                        context
                            .schedule_local_after(delay, move |context| {
                                Self::schedule_parallel_response_hop(
                                    context,
                                    route_id,
                                    route,
                                    last_hop_index,
                                    response,
                                    trace,
                                    fabric,
                                    response_sink,
                                );
                            })
                            .expect("validated target response delay");
                    }
                    TargetOutcome::NoResponse => {}
                }
            })
            .map_err(TransportError::Scheduler)
    }
}

fn record_direct_request_trace(
    trace: &crate::MemoryTrace,
    start_tick: Tick,
    route_id: crate::MemoryRouteId,
    route: &crate::MemoryRoute,
    request_id: rem6_memory::MemoryRequestId,
) -> Result<(), TransportError> {
    trace.record(MemoryTraceEvent::request(
        start_tick,
        route_id,
        route.source().clone(),
        MemoryTraceKind::RequestSent,
        request_id,
    ));

    let mut tick = start_tick;
    for hop in route.hops() {
        tick = tick
            .checked_add(hop.request_latency())
            .ok_or(TransportError::Scheduler(SchedulerError::TickOverflow {
                now: tick,
                delay: hop.request_latency(),
            }))?;
        trace.record(MemoryTraceEvent::request(
            tick,
            route_id,
            hop.endpoint().clone(),
            MemoryTraceKind::RequestArrived,
            request_id,
        ));
    }

    Ok(())
}
