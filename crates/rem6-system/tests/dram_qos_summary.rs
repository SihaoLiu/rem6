use rem6_dram::{
    DramController, DramGeometry, DramLowPowerState, DramLowPowerTiming, DramQosRequest,
    DramQosSchedulingPolicy, DramQosTurnaroundPolicy, DramTargetActivity, DramTiming,
};
use rem6_fabric::{QosPriority, QosQueueArbiter, QosQueuePolicyKind, QosRequestorId};
use rem6_kernel::RecordedConservativeRunSummary;
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
};
use rem6_system::{
    RiscvSystemRun, RiscvSystemRunStopReason, RiscvTopologyDmaRunSummary,
    RiscvTopologyDmaStageRunSummary,
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

fn low_power_dram_activity(target: MemoryTargetId) -> DramTargetActivity {
    let timing = DramTiming::new(4, 8, 10, 3, 5)
        .unwrap()
        .with_low_power_timing(DramLowPowerTiming::new(20, 80, 7).unwrap());
    let mut controller = DramController::new(DramGeometry::new(4, 256, 64).unwrap(), timing);

    controller.schedule(0, &request(7, 0x0000, 70)).unwrap();
    controller.schedule(120, &request(7, 0x0000, 71)).unwrap();

    DramTargetActivity::new(target, controller.activity_profile())
}

fn dma_stage(target: MemoryTargetId) -> RiscvTopologyDmaStageRunSummary {
    RiscvTopologyDmaStageRunSummary::new(
        Vec::new(),
        RecordedConservativeRunSummary::empty(0),
        0,
        0,
        0,
    )
    .with_dram_activity(vec![qos_dram_activity(target)])
}

#[test]
fn system_run_reports_dram_qos_diagnostics() {
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    )
    .with_dram_activity(vec![qos_dram_activity(MemoryTargetId::new(2))]);

    assert!(run.has_dram_qos_activity());
    assert_eq!(run.dram_qos_access_count(), 3);
    assert_eq!(run.dram_qos_byte_count(), 24);
    assert_eq!(run.dram_qos_escalated_access_count(), 1);
    assert_eq!(run.dram_qos_priority_access_count(QosPriority::new(0)), 2);
    assert_eq!(run.dram_qos_priority_byte_count(QosPriority::new(0)), 16);
    assert_eq!(
        run.dram_qos_requestor_access_count(QosRequestorId::new(7)),
        2
    );
    assert_eq!(
        run.dram_qos_requestor_byte_count(QosRequestorId::new(7)),
        16
    );
}

#[test]
fn dma_run_summary_merges_dram_qos_diagnostics() {
    let read = dma_stage(MemoryTargetId::new(2));
    let write = dma_stage(MemoryTargetId::new(2));
    let summary = RiscvTopologyDmaRunSummary::new(read, write);

    assert!(summary.read().has_dram_qos_activity());
    assert_eq!(summary.read().dram_qos_access_count(), 3);
    assert!(summary.has_dram_qos_activity());
    assert_eq!(summary.dram_qos_access_count(), 6);
    assert_eq!(summary.dram_qos_byte_count(), 48);
    assert_eq!(summary.dram_qos_escalated_access_count(), 2);
    assert_eq!(
        summary.dram_qos_priority_access_count(QosPriority::new(0)),
        4
    );
    assert_eq!(
        summary.dram_qos_priority_byte_count(QosPriority::new(0)),
        32
    );
    assert_eq!(
        summary.dram_qos_requestor_access_count(QosRequestorId::new(7)),
        4
    );
    assert_eq!(
        summary.dram_qos_requestor_byte_count(QosRequestorId::new(7)),
        32
    );
}

#[test]
fn system_run_reports_dram_low_power_diagnostics() {
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    )
    .with_dram_activity(vec![low_power_dram_activity(MemoryTargetId::new(3))]);

    assert!(run.has_dram_low_power_activity());
    assert!(run.has_dram_activity());
    assert_eq!(
        run.dram_low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        1
    );
    assert_eq!(
        run.dram_low_power_cycle_count(DramLowPowerState::PrechargePowerdown),
        60
    );
    assert_eq!(
        run.dram_low_power_entry_count(DramLowPowerState::SelfRefresh),
        1
    );
    assert_eq!(
        run.dram_low_power_cycle_count(DramLowPowerState::SelfRefresh),
        28
    );
    assert_eq!(run.dram_low_power_exit_count(), 1);
    assert_eq!(run.dram_low_power_exit_latency_cycles(), 7);
}
