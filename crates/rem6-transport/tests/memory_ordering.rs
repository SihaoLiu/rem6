use std::sync::{Arc, Mutex};

use rem6_fabric::{QosFixedPriorityPolicy, QosPriority, QosQueueArbiter, QosQueuePolicyKind};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAccessOrdering, MemoryBarrierSet,
    MemoryRequest, MemoryRequestId,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, TargetBatchOutcome,
    TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
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
