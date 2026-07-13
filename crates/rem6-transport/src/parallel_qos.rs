use std::collections::BTreeMap;

use rem6_fabric::{QosQueuedRequest, QosRequestId};
use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError, Tick};

use crate::{
    ordering, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, PreparedParallelTransaction,
    RequestDelivery, ResponseSink, TargetBatchOutcome, TargetOutcome, TransportEndpointId,
    TransportError,
};

struct DirectTargetEntry {
    route_id: crate::MemoryRouteId,
    route: crate::MemoryRoute,
    trace: crate::MemoryTrace,
    responder: crate::ParallelRequestResponder,
    response_sink: ResponseSink,
    response_qos: Option<crate::response_qos::ResponseQosContext>,
    last_hop_index: usize,
    request_id: rem6_memory::MemoryRequestId,
    delivery: RequestDelivery,
}

impl MemoryTransport {
    pub(crate) fn can_submit_direct_qos_parallel_batch(
        &self,
        transactions: &[PreparedParallelTransaction],
    ) -> bool {
        self.qos_state.is_some()
            && (transactions.len() > 1 || self.direct_target_batch_responder.is_some())
            && !transactions.is_empty()
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
            if self.direct_target_batch_responder.is_some() {
                let indexes = group.iter().map(|(index, _)| *index).collect::<Vec<_>>();
                let event = self.schedule_or_append_direct_parallel_target_batch(
                    scheduler,
                    start_tick,
                    arrival_tick,
                    target_partition,
                    group
                        .into_iter()
                        .map(|(_, transaction)| transaction)
                        .collect(),
                );
                for index in indexes {
                    events[index] = Some(event);
                }
                continue;
            }

            let ordered = Self::order_direct_qos_group(self.qos_state.as_ref(), group);
            for (index, transaction) in ordered {
                let event = self.schedule_direct_parallel_target_event(
                    scheduler,
                    start_tick,
                    arrival_tick,
                    target_partition,
                    transaction,
                );
                events[index] = Some(event);
            }
        }

        Ok(events
            .into_iter()
            .map(|event| event.expect("every direct QoS transaction schedules one event"))
            .collect())
    }

    fn order_direct_qos_group(
        qos_state: Option<&crate::SharedFabricQosState>,
        group: Vec<(usize, PreparedParallelTransaction)>,
    ) -> Vec<(usize, PreparedParallelTransaction)> {
        let Some(qos_state) = qos_state else {
            return group;
        };
        let mut qos_state = qos_state.inner.lock().expect("QoS state lock");
        let mut pending = group;
        let mut ordered = Vec::with_capacity(pending.len());

        while !pending.is_empty() {
            let eligible_indexes = eligible_direct_qos_transactions(&pending);
            let queue = pending
                .iter()
                .enumerate()
                .filter(|(index, _)| eligible_indexes.contains(index))
                .enumerate()
                .map(|(order, (_, (_, transaction)))| {
                    QosQueuedRequest::new(
                        QosRequestId::new(transaction.request.id().sequence()),
                        transaction.qos_requestor,
                        transaction.qos_priority,
                        transaction.request.size().bytes(),
                        order as u64,
                    )
                    .expect("memory requests always have nonzero size")
                })
                .collect::<Vec<_>>();
            let grant = qos_state
                .request_arbiter
                .grant(&queue)
                .expect("nonempty direct QoS queue must produce a grant");
            ordered.push(pending.remove(eligible_indexes[grant.queue_index()]));
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
    ) -> PartitionEventId {
        let PreparedParallelTransaction {
            route_id,
            route,
            request,
            trace,
            responder,
            response_sink,
            first_hop_delay: _,
            qos_requestor: _,
            qos_priority: _,
            response_qos,
        } = transaction;
        let request_id = request.id();
        let last_hop_index = route.hops().len() - 1;
        let fabric = self.fabric.clone();
        record_direct_request_trace(&trace, start_tick, route_id, &route, request_id)
            .expect("validated direct route trace timing");

        scheduler
            .schedule_parallel_at(target_partition, arrival_tick, move |context| {
                let hop = route.hops()[last_hop_index].clone();
                let delivery = RequestDelivery {
                    tick: context.now(),
                    route: route_id,
                    endpoint: hop.endpoint().clone(),
                    request,
                };

                let outcome = responder(delivery, context);
                Self::schedule_direct_parallel_target_outcome(
                    context,
                    route_id,
                    route,
                    last_hop_index,
                    outcome,
                    trace,
                    fabric,
                    response_qos,
                    response_sink,
                );
            })
            .expect("validated direct target schedule")
    }

    fn schedule_or_append_direct_parallel_target_batch(
        &self,
        scheduler: &mut PartitionedScheduler,
        start_tick: Tick,
        arrival_tick: Tick,
        target_partition: PartitionId,
        transactions: Vec<PreparedParallelTransaction>,
    ) -> PartitionEventId {
        let key = (
            arrival_tick,
            target_partition,
            transactions
                .first()
                .expect("direct target batch is nonempty")
                .route
                .target()
                .clone(),
        );
        for transaction in &transactions {
            record_direct_request_trace(
                &transaction.trace,
                start_tick,
                transaction.route_id,
                &transaction.route,
                transaction.request.id(),
            )
            .expect("validated direct batch trace timing");
        }

        let pending = {
            let mut batches = self
                .direct_target_batches
                .lock()
                .expect("direct target batch lock");
            let pending = batches.get(&key).cloned();
            if pending.as_ref().is_some_and(|pending| {
                let event = pending
                    .lock()
                    .expect("pending direct target batch lock")
                    .event
                    .expect("scheduled direct target batch has an event");
                scheduler.pending_event_snapshot(event).is_none()
            }) {
                batches.remove(&key);
                None
            } else {
                pending
            }
        };
        if let Some(pending) = pending {
            let mut pending = pending.lock().expect("pending direct target batch lock");
            pending.transactions.extend(transactions);
            return pending
                .event
                .expect("scheduled direct target batch has an event");
        }

        let pending = std::sync::Arc::new(std::sync::Mutex::new(crate::PendingDirectTargetBatch {
            event: None,
            transactions,
        }));
        let fabric = self.fabric.clone();
        let batch_responder = self
            .direct_target_batch_responder
            .as_ref()
            .map(std::sync::Arc::clone);
        let qos_state = self.qos_state.clone();
        let pending_for_event = std::sync::Arc::clone(&pending);
        let batches = std::sync::Arc::clone(&self.direct_target_batches);
        let key_for_event = key.clone();

        let event = scheduler
            .schedule_parallel_at(target_partition, arrival_tick, move |context| {
                batches
                    .lock()
                    .expect("direct target batch lock")
                    .remove(&key_for_event);
                let transactions = std::mem::take(
                    &mut pending_for_event
                        .lock()
                        .expect("pending direct target batch lock")
                        .transactions,
                );
                let transactions = Self::order_direct_qos_group(
                    qos_state.as_ref(),
                    transactions.into_iter().enumerate().collect(),
                )
                .into_iter()
                .map(|(_, transaction)| transaction)
                .collect();
                Self::run_direct_parallel_target_batch(
                    context,
                    transactions,
                    batch_responder,
                    fabric,
                );
            })
            .expect("validated direct batch target schedule");
        pending
            .lock()
            .expect("pending direct target batch lock")
            .event = Some(event);
        self.direct_target_batches
            .lock()
            .expect("direct target batch lock")
            .insert(key, pending);
        event
    }

    fn run_direct_parallel_target_batch(
        context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
        transactions: Vec<PreparedParallelTransaction>,
        batch_responder: Option<crate::ParallelTargetBatchResponder>,
        fabric: Option<std::sync::Arc<std::sync::Mutex<rem6_fabric::FabricModel>>>,
    ) {
        let mut entries = Vec::with_capacity(transactions.len());
        let mut deliveries = Vec::with_capacity(transactions.len());
        for transaction in transactions {
            let PreparedParallelTransaction {
                route_id,
                route,
                request,
                trace,
                responder,
                response_sink,
                first_hop_delay: _,
                qos_requestor: _,
                qos_priority: _,
                response_qos,
            } = transaction;
            let request_id = request.id();
            let last_hop_index = route.hops().len() - 1;
            let hop = route.hops()[last_hop_index].clone();
            let delivery = RequestDelivery {
                tick: context.now(),
                route: route_id,
                endpoint: hop.endpoint().clone(),
                request,
            };
            deliveries.push(delivery.clone());
            entries.push(DirectTargetEntry {
                route_id,
                route,
                trace,
                responder,
                response_sink,
                response_qos,
                last_hop_index,
                request_id,
                delivery,
            });
        }

        if let Some(batch_responder) = &batch_responder {
            if let Some(outcomes) = batch_responder(deliveries, context) {
                Self::schedule_direct_parallel_batch_outcomes(context, outcomes, entries, fabric);
                return;
            }
        }

        for entry in entries {
            let outcome = (entry.responder)(entry.delivery, context);
            Self::schedule_direct_parallel_target_outcome(
                context,
                entry.route_id,
                entry.route,
                entry.last_hop_index,
                outcome,
                entry.trace,
                fabric.clone(),
                entry.response_qos,
                entry.response_sink,
            );
        }
    }

    fn schedule_direct_parallel_batch_outcomes(
        context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
        outcomes: Vec<TargetBatchOutcome>,
        mut entries: Vec<DirectTargetEntry>,
        fabric: Option<std::sync::Arc<std::sync::Mutex<rem6_fabric::FabricModel>>>,
    ) {
        for outcome in outcomes {
            let entry_index = entries
                .iter()
                .position(|entry| entry.request_id == outcome.request())
                .expect("batch responder returned an unknown request");
            let entry = entries.remove(entry_index);
            Self::schedule_direct_parallel_target_outcome(
                context,
                entry.route_id,
                entry.route,
                entry.last_hop_index,
                outcome.into_outcome(),
                entry.trace,
                fabric.clone(),
                entry.response_qos,
                entry.response_sink,
            );
        }
        assert!(entries.is_empty(), "batch responder omitted a request");
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_direct_parallel_target_outcome(
        context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
        route_id: crate::MemoryRouteId,
        route: crate::MemoryRoute,
        hop_index: usize,
        outcome: TargetOutcome,
        trace: crate::MemoryTrace,
        fabric: Option<std::sync::Arc<std::sync::Mutex<rem6_fabric::FabricModel>>>,
        response_qos: Option<crate::response_qos::ResponseQosContext>,
        response_sink: ResponseSink,
    ) {
        match outcome {
            TargetOutcome::Respond(response) => {
                Self::schedule_parallel_response_hop(
                    context,
                    route_id,
                    route,
                    hop_index,
                    response,
                    trace,
                    fabric,
                    response_qos,
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
                            response_qos,
                            response_sink,
                        );
                    })
                    .expect("validated target response delay");
            }
            TargetOutcome::NoResponse => {}
        }
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

fn eligible_direct_qos_transactions(group: &[(usize, PreparedParallelTransaction)]) -> Vec<usize> {
    let eligible = group
        .iter()
        .enumerate()
        .filter_map(|(candidate_index, _)| {
            (!direct_qos_transaction_is_order_blocked(candidate_index, group))
                .then_some(candidate_index)
        })
        .collect::<Vec<_>>();
    debug_assert!(
        !eligible.is_empty(),
        "oldest direct QoS transaction is always ordering-eligible"
    );
    eligible
}

fn direct_qos_transaction_is_order_blocked(
    candidate_index: usize,
    group: &[(usize, PreparedParallelTransaction)],
) -> bool {
    let (candidate_order, candidate) = &group[candidate_index];
    group.iter().any(|(other_order, other)| {
        other_order < candidate_order && ordering::transaction_orders_before(other, candidate)
    })
}
