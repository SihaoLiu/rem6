use std::sync::{Arc, Mutex};

use rem6_fabric::{
    FabricLinkId, FabricModel, FabricPath, FabricPathHop, QosFixedPriorityPolicy, QosPriority,
    QosQueueArbiter, QosQueuePolicyKind,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAccessOrdering, MemoryBarrierSet,
    MemoryRequest, MemoryRequestId,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryTrace, MemoryTransport, ParallelMemoryTransaction,
    TargetBatchOutcome, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn fabric_link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn fabric_path(name: &str) -> FabricPath {
    FabricPath::new([FabricPathHop::new(fabric_link(name), 2, 4).unwrap()]).unwrap()
}

fn request(sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(7), sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
}

fn fabric_ordering_route(
    transport: &mut MemoryTransport,
    path_name: &str,
) -> rem6_transport::MemoryRouteId {
    transport
        .add_route(
            MemoryRoute::new_path(
                endpoint("core0.dmem"),
                PartitionId::new(0),
                [
                    MemoryRouteHop::new(endpoint("memory0"), PartitionId::new(1), 2, 2)
                        .unwrap()
                        .with_request_fabric_path(fabric_path(path_name)),
                ],
            )
            .unwrap(),
        )
        .unwrap()
}

fn record_no_response(
    deliveries: Arc<Mutex<Vec<(u64, MemoryRequestId)>>>,
) -> impl FnOnce(
    rem6_transport::RequestDelivery,
    &mut rem6_kernel::ParallelSchedulerContext<'_>,
) -> TargetOutcome
       + Send
       + 'static {
    move |delivery, _context| {
        deliveries
            .lock()
            .unwrap()
            .push((delivery.tick(), delivery.request().id()));
        TargetOutcome::NoResponse
    }
}

#[test]
fn direct_qos_batch_preserves_same_agent_release_ordering() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let responder_log = Arc::clone(&deliveries);
    let mut transport = MemoryTransport::with_qos_policy(
        QosQueueArbiter::new(QosQueuePolicyKind::Fifo),
        QosFixedPriorityPolicy::new(2, QosPriority::new(1)).unwrap(),
    )
    .with_direct_target_batch_responder(move |batch, _context| {
        responder_log.lock().unwrap().push(
            batch
                .iter()
                .map(|delivery| delivery.request().id())
                .collect::<Vec<_>>(),
        );
        Some(
            batch
                .iter()
                .map(|delivery| {
                    TargetBatchOutcome::new(delivery.request().id(), TargetOutcome::NoResponse)
                })
                .collect(),
        )
    });

    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0.dmem"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                4,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let prior = request(1, 0x1000);
    let release = request(2, 0x2000).with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        None,
    ));
    let trace = MemoryTrace::new();

    transport
        .submit_parallel_batch(
            &mut scheduler,
            [
                ParallelMemoryTransaction::new(
                    route,
                    prior.clone(),
                    trace.clone(),
                    |_, _| panic!("batch responder should handle the request"),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(1)),
                ParallelMemoryTransaction::new(
                    route,
                    release.clone(),
                    trace,
                    |_, _| panic!("batch responder should handle the request"),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(0)),
            ],
        )
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 1);
    assert_eq!(
        *deliveries.lock().unwrap(),
        vec![vec![prior.id(), release.id()]]
    );
}

#[test]
fn direct_qos_batch_preserves_same_agent_acquire_ordering() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let responder_log = Arc::clone(&deliveries);
    let mut transport = MemoryTransport::with_qos_policy(
        QosQueueArbiter::new(QosQueuePolicyKind::Fifo),
        QosFixedPriorityPolicy::new(2, QosPriority::new(1)).unwrap(),
    )
    .with_direct_target_batch_responder(move |batch, _context| {
        responder_log.lock().unwrap().push(
            batch
                .iter()
                .map(|delivery| delivery.request().id())
                .collect::<Vec<_>>(),
        );
        Some(
            batch
                .iter()
                .map(|delivery| {
                    TargetBatchOutcome::new(delivery.request().id(), TargetOutcome::NoResponse)
                })
                .collect(),
        )
    });

    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0.dmem"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                4,
                2,
            )
            .unwrap(),
        )
        .unwrap();
    let acquire = request(3, 0x3000).with_ordering(MemoryAccessOrdering::new(
        None,
        Some(MemoryBarrierSet::memory()),
    ));
    let later = request(4, 0x4000);
    let trace = MemoryTrace::new();

    transport
        .submit_parallel_batch(
            &mut scheduler,
            [
                ParallelMemoryTransaction::new(
                    route,
                    acquire.clone(),
                    trace.clone(),
                    |_, _| panic!("batch responder should handle the request"),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(1)),
                ParallelMemoryTransaction::new(
                    route,
                    later.clone(),
                    trace,
                    |_, _| panic!("batch responder should handle the request"),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(0)),
            ],
        )
        .unwrap();

    let summary = scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(summary.executed_events(), 1);
    assert_eq!(
        *deliveries.lock().unwrap(),
        vec![vec![acquire.id(), later.id()]]
    );
}

#[test]
fn shared_fabric_qos_batch_preserves_same_agent_release_ordering() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::with_fabric_qos(
        FabricModel::new(),
        QosQueueArbiter::new(QosQueuePolicyKind::Fifo),
    );
    let route = fabric_ordering_route(&mut transport, "mesh_order_release");
    let prior = request(11, 0x1100);
    let release = request(12, 0x1200).with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        None,
    ));
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let trace = MemoryTrace::new();

    transport
        .submit_parallel_batch(
            &mut scheduler,
            [
                ParallelMemoryTransaction::new(
                    route,
                    prior.clone(),
                    trace.clone(),
                    record_no_response(Arc::clone(&deliveries)),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(1)),
                ParallelMemoryTransaction::new(
                    route,
                    release.clone(),
                    trace,
                    record_no_response(Arc::clone(&deliveries)),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(0)),
            ],
        )
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        deliveries
            .lock()
            .unwrap()
            .iter()
            .map(|(_, request)| *request)
            .collect::<Vec<_>>(),
        vec![prior.id(), release.id()]
    );
}

#[test]
fn shared_fabric_qos_batch_preserves_same_agent_acquire_ordering() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::with_fabric_qos(
        FabricModel::new(),
        QosQueueArbiter::new(QosQueuePolicyKind::Fifo),
    );
    let route = fabric_ordering_route(&mut transport, "mesh_order_acquire");
    let acquire = request(13, 0x1300).with_ordering(MemoryAccessOrdering::new(
        None,
        Some(MemoryBarrierSet::memory()),
    ));
    let later = request(14, 0x1400);
    let deliveries = Arc::new(Mutex::new(Vec::new()));
    let trace = MemoryTrace::new();

    transport
        .submit_parallel_batch(
            &mut scheduler,
            [
                ParallelMemoryTransaction::new(
                    route,
                    acquire.clone(),
                    trace.clone(),
                    record_no_response(Arc::clone(&deliveries)),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(1)),
                ParallelMemoryTransaction::new(
                    route,
                    later.clone(),
                    trace,
                    record_no_response(Arc::clone(&deliveries)),
                    |_| panic!("request-only transfer must not deliver a response"),
                )
                .with_qos_priority(QosPriority::new(0)),
            ],
        )
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(
        deliveries
            .lock()
            .unwrap()
            .iter()
            .map(|(_, request)| *request)
            .collect::<Vec<_>>(),
        vec![acquire.id(), later.id()]
    );
}
