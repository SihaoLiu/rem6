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
                let hops = transaction.route.hops();
                hops.len() == 1 && hops[0].request_fabric_path().is_none()
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
            let hop = &transaction.route.hops()[0];
            let arrival_tick = start_tick.checked_add(transaction.first_hop_delay).ok_or(
                TransportError::Scheduler(SchedulerError::TickOverflow {
                    now: start_tick,
                    delay: transaction.first_hop_delay,
                }),
            )?;
            groups
                .entry((arrival_tick, hop.partition(), hop.endpoint().clone()))
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
        let source_endpoint = route.source().clone();
        let fabric = self.fabric.clone();
        trace.record(MemoryTraceEvent::request(
            start_tick,
            route_id,
            source_endpoint,
            MemoryTraceKind::RequestSent,
            request_id,
        ));

        scheduler
            .schedule_parallel_at(target_partition, arrival_tick, move |context| {
                let hop_index = 0;
                let hop = route.hops()[hop_index].clone();
                trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    hop.endpoint().clone(),
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));
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
                            hop_index,
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
                                    hop_index,
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
