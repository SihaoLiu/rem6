use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_dram::{
    DramController, DramGeometry, DramQosRequest, DramQosSchedulingPolicy, DramQosTurnaroundPolicy,
    DramTargetActivity, DramTiming,
};
use rem6_fabric::{QosPriority, QosQueueArbiter, QosQueuePolicyKind, QosRequestorId};
use rem6_kernel::{RecordedConservativeRunSummary, WaitForGraph};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request(agent: u32, address: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(agent), sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap()
}

fn qos_dram_activity(target: MemoryTargetId) -> DramTargetActivity {
    let mut controller = DramController::new(
        DramGeometry::new(4, 256, 64).unwrap(),
        DramTiming::new(3, 5, 7, 2, 4).unwrap(),
    );
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let low = request(7, 0x0000, 50);
    let other = request(8, 0x0040, 51);
    let high = request(7, 0x0100, 52);

    controller
        .schedule_qos_batch_with_policy(
            0,
            [
                DramQosRequest::new(&low, QosPriority::new(2), 0),
                DramQosRequest::new(&other, QosPriority::new(1), 1),
                DramQosRequest::new(&high, QosPriority::new(0), 2),
            ],
            &mut arbiter,
            DramQosSchedulingPolicy::new()
                .with_priority_escalation()
                .with_turnaround(DramQosTurnaroundPolicy::RequestOrder),
        )
        .unwrap();

    DramTargetActivity::new(target, controller.activity_profile())
}

#[test]
fn coherence_run_summary_reports_dram_qos_diagnostics() {
    let summary = ParallelCoherenceRunSummary::new(
        RecordedConservativeRunSummary::empty(0),
        0,
        0,
        3,
        Vec::new(),
        vec![qos_dram_activity(MemoryTargetId::new(3))],
        ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new()),
    );

    assert!(summary.has_dram_qos_activity());
    assert_eq!(summary.dram_qos_access_count(), 3);
    assert_eq!(summary.dram_qos_byte_count(), 24);
    assert_eq!(summary.dram_qos_escalated_access_count(), 1);
    assert_eq!(
        summary.dram_qos_priority_access_count(QosPriority::new(0)),
        2
    );
    assert_eq!(
        summary.dram_qos_priority_byte_count(QosPriority::new(0)),
        16
    );
    assert_eq!(
        summary.dram_qos_requestor_access_count(QosRequestorId::new(7)),
        2
    );
    assert_eq!(
        summary.dram_qos_requestor_byte_count(QosRequestorId::new(7)),
        16
    );
}
