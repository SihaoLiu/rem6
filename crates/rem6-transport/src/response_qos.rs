use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_fabric::{
    FabricModel, FabricPacket, FabricPacketId, FabricTransfer, QosPriority, QosRequestorId,
};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, Tick};
use rem6_memory::MemoryResponse;

use crate::{
    fabric_packet_id, ordering, response_hop_delay, response_packet_bytes, FabricQosGrantDirection,
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, ResponseDelivery, ResponseSink,
    SharedFabricQosState, TransportLatency,
};

type ResponseQosBatchKey = (Tick, PartitionId, usize);
type SharedPendingResponseQosBatch = Arc<Mutex<PendingResponseQosBatch>>;

#[derive(Clone, Default)]
pub(crate) struct ResponseQosBatches {
    inner: Arc<Mutex<BTreeMap<ResponseQosBatchKey, SharedPendingResponseQosBatch>>>,
}

impl fmt::Debug for ResponseQosBatches {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResponseQosBatches")
            .field(
                "pending",
                &self.inner.lock().expect("response QoS batch lock").len(),
            )
            .finish()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResponseQosContext {
    state: SharedFabricQosState,
    requestor: QosRequestorId,
    priority: QosPriority,
}

impl ResponseQosContext {
    fn new(state: SharedFabricQosState, requestor: QosRequestorId, priority: QosPriority) -> Self {
        Self {
            state,
            requestor,
            priority,
        }
    }
}

struct PendingResponseQosBatch {
    entries: Vec<ResponseQosEntry>,
}

struct ResponseQosEntry {
    route_id: MemoryRouteId,
    route: MemoryRoute,
    hop_index: usize,
    response: MemoryResponse,
    trace: MemoryTrace,
    response_sink: ResponseSink,
    qos: ResponseQosContext,
}

impl crate::MemoryTransport {
    pub(crate) fn response_qos_context(
        &self,
        requestor: QosRequestorId,
        priority: QosPriority,
    ) -> Option<ResponseQosContext> {
        self.qos_state
            .clone()
            .map(|state| ResponseQosContext::new(state, requestor, priority))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn schedule_parallel_response_hop(
        context: &mut ParallelSchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        response: MemoryResponse,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        qos: Option<ResponseQosContext>,
        response_sink: ResponseSink,
    ) {
        let hop = route.hops()[hop_index].clone();
        if hop.response_fabric_path().is_some() {
            if let Some(qos) = qos {
                Self::enqueue_parallel_response_qos_hop(
                    context,
                    route_id,
                    route,
                    hop_index,
                    response,
                    trace,
                    fabric.expect("validated response fabric model"),
                    qos,
                    response_sink,
                );
                return;
            }
        }

        let delay = response_hop_delay(&fabric, context.now(), route_id, &route, &hop, &response)
            .expect("validated response fabric timing");
        Self::schedule_parallel_response_arrival(
            context,
            route_id,
            route,
            hop_index,
            response,
            trace,
            fabric,
            qos,
            response_sink,
            delay,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn enqueue_parallel_response_qos_hop(
        context: &mut ParallelSchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        response: MemoryResponse,
        trace: MemoryTrace,
        fabric: Arc<Mutex<FabricModel>>,
        qos: ResponseQosContext,
        response_sink: ResponseSink,
    ) {
        let key = (
            context.now(),
            context.partition(),
            Arc::as_ptr(&fabric) as usize,
        );
        let entry = ResponseQosEntry {
            route_id,
            route,
            hop_index,
            response,
            trace,
            response_sink,
            qos: qos.clone(),
        };
        let mut batches = qos
            .state
            .response_batches
            .inner
            .lock()
            .expect("response QoS batch lock");
        if batches
            .get(&key)
            .is_some_and(|pending| Arc::strong_count(pending) == 1)
        {
            batches.remove(&key);
        }
        if let Some(pending) = batches.get(&key) {
            pending
                .lock()
                .expect("pending response QoS batch lock")
                .entries
                .push(entry);
            return;
        }

        let pending = Arc::new(Mutex::new(PendingResponseQosBatch {
            entries: vec![entry],
        }));
        batches.insert(key, Arc::clone(&pending));
        drop(batches);

        let response_batches = qos.state.response_batches.clone();
        context
            .schedule_local_after(0, move |context| {
                response_batches
                    .inner
                    .lock()
                    .expect("response QoS batch lock")
                    .remove(&key);
                let entries = std::mem::take(
                    &mut pending
                        .lock()
                        .expect("pending response QoS batch lock")
                        .entries,
                );
                Self::dispatch_parallel_response_qos_batch(context, entries, fabric);
            })
            .expect("validated response QoS batch schedule");
    }

    fn dispatch_parallel_response_qos_batch(
        context: &mut ParallelSchedulerContext<'_>,
        entries: Vec<ResponseQosEntry>,
        fabric: Arc<Mutex<FabricModel>>,
    ) {
        let requests = entries
            .iter()
            .enumerate()
            .map(|(order, entry)| {
                let hop = &entry.route.hops()[entry.hop_index];
                let packet = FabricPacket::new(
                    fabric_packet_id(
                        entry.route_id,
                        entry.response.request_id(),
                        TransportLatency::Response,
                    ),
                    response_packet_bytes(&entry.response),
                    entry.route.response_virtual_network(),
                )
                .expect("memory responses always produce valid fabric packets");
                ordering::FabricQosTransfer::new(
                    packet,
                    hop.response_fabric_path()
                        .expect("response QoS entries have fabric paths")
                        .clone(),
                    entry.qos.requestor,
                    entry.qos.priority,
                    order as u64,
                )
            })
            .collect::<Vec<_>>();
        let state = entries
            .first()
            .expect("response QoS batch is nonempty")
            .qos
            .state
            .clone();
        let transfers = {
            let mut fabric = fabric.lock().expect("fabric lock");
            let mut state = state.inner.lock().expect("fabric QoS state lock");
            let arbiter_checkpoint = state.response_arbiter.clone();
            let batch = state.activity.next_batch();
            let result = fabric.try_transaction(|fabric| {
                ordering::transmit_qos_fabric_batch(
                    FabricQosGrantDirection::Response,
                    context.now(),
                    batch,
                    &requests,
                    fabric,
                    &mut state.response_arbiter,
                )
            });
            match result {
                Ok((transfers, activities)) => {
                    state.activity.commit_batch(batch, activities);
                    Ok(transfers)
                }
                Err(error) => {
                    state.response_arbiter = arbiter_checkpoint;
                    Err(error)
                }
            }
        }
        .unwrap_or_else(|error| panic!("validated response fabric timing: {error}"));
        let delays = response_transfer_delays(context.now(), transfers);

        for (index, entry) in entries.into_iter().enumerate() {
            Self::schedule_parallel_response_arrival(
                context,
                entry.route_id,
                entry.route,
                entry.hop_index,
                entry.response,
                entry.trace,
                Some(Arc::clone(&fabric)),
                Some(entry.qos),
                entry.response_sink,
                delays[&requests[index].packet().id()],
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_parallel_response_arrival(
        context: &mut ParallelSchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        response: MemoryResponse,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        qos: Option<ResponseQosContext>,
        response_sink: ResponseSink,
        delay: Tick,
    ) {
        let endpoint = if hop_index == 0 {
            route.source().clone()
        } else {
            route.hops()[hop_index - 1].endpoint().clone()
        };
        let partition = if hop_index == 0 {
            route.source_partition()
        } else {
            route.hops()[hop_index - 1].partition()
        };
        context
            .schedule_remote_after(partition, delay, move |context| {
                trace.record(MemoryTraceEvent::response(
                    context.now(),
                    route_id,
                    endpoint.clone(),
                    response.request_id(),
                    response.status(),
                ));

                if hop_index == 0 {
                    response_sink(ResponseDelivery {
                        tick: context.now(),
                        route: route_id,
                        endpoint,
                        response,
                    });
                } else {
                    Self::schedule_parallel_response_hop(
                        context,
                        route_id,
                        route,
                        hop_index - 1,
                        response,
                        trace,
                        fabric,
                        qos,
                        response_sink,
                    );
                }
            })
            .expect("validated response transport latency");
    }
}

fn response_transfer_delays(
    now: Tick,
    transfers: Vec<FabricTransfer>,
) -> BTreeMap<FabricPacketId, Tick> {
    transfers
        .into_iter()
        .map(|transfer| {
            let delay = transfer
                .arrival_tick()
                .checked_sub(now)
                .expect("fabric response arrival cannot precede injection");
            (transfer.packet().id(), delay)
        })
        .collect()
}
