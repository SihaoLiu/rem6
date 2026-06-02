use super::*;
use crate::workload_replay::{WorkloadGpuDmaActivity, WorkloadReplayActivityRefs};
use crate::workload_replay_heterogeneous::{WorkloadAcceleratorActivity, WorkloadGpuActivity};
use crate::{RiscvClusterTurn, RiscvSystemRunStopReason};
use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_dram::{
    DramController, DramGeometry, DramLowPowerState, DramLowPowerTiming, DramQosRequest,
    DramQosSchedulingPolicy, DramQosTurnaroundPolicy, DramTargetActivity, DramTiming,
};
use rem6_fabric::{QosPriority, QosQueueArbiter, QosQueuePolicyKind, QosRequestorId};
use rem6_kernel::{
    LivelockTransitionKind, ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionFrontier,
    PartitionId, PartitionedScheduler, WaitForEdgeKind, WaitForGraph, WaitForNode,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
};
use rem6_workload::{
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelDiagnosticScope,
    WorkloadWaitForBlockedNodeWindow, WorkloadWaitForEdgeKindWindow,
    WorkloadWaitForTargetNodeWindow,
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
fn expected_clean(
    scope: WorkloadParallelDiagnosticScope,
) -> WorkloadExpectedCleanParallelDiagnostics {
    WorkloadExpectedCleanParallelDiagnostics::new(scope)
}
#[test]
fn livelock_transition_threshold_uses_lowest_declared_clean_threshold() {
    assert_eq!(
        livelock_transition_threshold(&[expected_clean(
            WorkloadParallelDiagnosticScope::FullSystem,
        )]),
        None,
    );
    let full_system = expected_clean(WorkloadParallelDiagnosticScope::FullSystem)
        .with_livelock_transition_threshold(5)
        .unwrap();
    let data_cache = expected_clean(WorkloadParallelDiagnosticScope::DataCache)
        .with_livelock_transition_threshold(3)
        .unwrap();
    assert_eq!(
        livelock_transition_threshold(&[full_system, data_cache]),
        Some(3),
    );
}
fn component_wait_node(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}
fn transaction_wait_node(name: &str) -> WaitForNode {
    WaitForNode::transaction(name).unwrap()
}
fn resource_wait_node(name: &str) -> WaitForNode {
    WaitForNode::resource(name).unwrap()
}
fn empty_coherence_wait_for_graphs() -> ParallelCoherenceWaitForGraphs {
    ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new())
}
fn batch_scheduler_turn(
    partitions: u32,
    worker_limit: usize,
    scheduled_partitions: &[PartitionId],
) -> RiscvClusterTurn {
    let mut turns = batch_scheduler_turns_at(partitions, worker_limit, 0, scheduled_partitions);
    turns.remove(0)
}
fn batch_scheduler_turns_at(
    partitions: u32,
    worker_limit: usize,
    tick: u64,
    scheduled_partitions: &[PartitionId],
) -> Vec<RiscvClusterTurn> {
    let mut scheduler =
        PartitionedScheduler::with_parallel_worker_limit(partitions, 4, worker_limit).unwrap();
    for partition in scheduled_partitions {
        scheduler
            .schedule_parallel_at(*partition, tick, |_| {})
            .unwrap();
    }
    let mut turns = Vec::new();
    while let Some(plan) = scheduler.plan_next_parallel_epoch().unwrap() {
        let before = scheduler.now();
        let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
        let summary = recorded.summary();
        turns.push(RiscvClusterTurn::parallel_scheduler(plan, recorded));
        if summary.final_tick() == before && summary.executed_events() == 0 {
            break;
        }
    }
    turns
}
fn data_cache_batch_run(
    partitions: u32,
    worker_limit: usize,
    scheduled_partitions: &[PartitionId],
) -> ParallelCoherenceRunSummary {
    data_cache_batch_run_at(partitions, worker_limit, 0, scheduled_partitions)
}
fn data_cache_batch_run_at(
    partitions: u32,
    worker_limit: usize,
    tick: u64,
    scheduled_partitions: &[PartitionId],
) -> ParallelCoherenceRunSummary {
    let mut scheduler =
        PartitionedScheduler::with_parallel_worker_limit(partitions, 4, worker_limit).unwrap();
    for partition in scheduled_partitions {
        scheduler
            .schedule_parallel_at(*partition, tick, |_| {})
            .unwrap();
    }
    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        empty_coherence_wait_for_graphs(),
    )
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

#[test]
fn parallel_execution_summary_copies_dram_qos_activity() {
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    )
    .with_dram_activity(vec![qos_dram_activity(MemoryTargetId::new(2))]);
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert!(summary.has_dram_qos_activity());
    assert_eq!(summary.dram_qos_access_count(), 3);
    assert_eq!(summary.dram_qos_byte_count(), 24);
    assert_eq!(summary.dram_qos_escalated_access_count(), 1);
    assert_eq!(
        summary.dram_qos_priority_access_count(QosPriority::new(0)),
        2,
    );
    assert_eq!(
        summary.dram_qos_priority_byte_count(QosPriority::new(0)),
        16
    );
    assert_eq!(
        summary.dram_qos_requestor_access_count(QosRequestorId::new(7)),
        2,
    );
    assert_eq!(
        summary.dram_qos_requestor_byte_count(QosRequestorId::new(7)),
        16,
    );
}

#[test]
fn parallel_execution_summary_copies_dram_low_power_activity() {
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    )
    .with_dram_activity(vec![low_power_dram_activity(MemoryTargetId::new(2))]);
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );

    assert!(summary.has_dram_low_power_activity());
    assert_eq!(
        summary.dram_low_power_entry_count(DramLowPowerState::ActivePowerdown),
        1
    );
    assert_eq!(
        summary.dram_low_power_cycle_count(DramLowPowerState::ActivePowerdown),
        88
    );
    assert_eq!(
        summary.dram_low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        0
    );
    assert_eq!(
        summary.dram_low_power_entry_count(DramLowPowerState::SelfRefresh),
        0
    );
    assert_eq!(summary.dram_low_power_exit_count(), 1);
    assert_eq!(summary.dram_low_power_exit_latency_cycles(), 7);
}

#[test]
fn parallel_execution_summary_copies_dma_scheduler_empty_epochs() {
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    );
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity {
        scheduler_empty_epoch_count: 2,
        ..WorkloadGpuDmaActivity::default()
    };
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity {
        scheduler_empty_epoch_count: 3,
        ..WorkloadAcceleratorDmaActivity::default()
    };
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(summary.gpu_dma_scheduler_empty_epoch_count(), 2);
    assert_eq!(summary.accelerator_dma_scheduler_empty_epoch_count(), 3);
    assert_eq!(summary.dma_scheduler_empty_epoch_count(), 5);
    assert_eq!(
        summary.full_system_parallel_scheduler_empty_epoch_count(),
        5
    );
}
#[test]
fn parallel_execution_summary_copies_dma_scheduler_frontiers() {
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    );
    let gpu_initial = PartitionFrontier::new(PartitionId::new(6), 5, 15, Some(9), 1);
    let gpu_final = PartitionFrontier::new(PartitionId::new(6), 15, 25, None, 0);
    let accelerator_initial = PartitionFrontier::new(PartitionId::new(7), 3, 13, Some(11), 2);
    let accelerator_final = PartitionFrontier::new(PartitionId::new(7), 21, 31, None, 0);
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity {
        scheduler_initial_frontiers: vec![gpu_initial],
        scheduler_final_frontiers: vec![gpu_final],
        ..WorkloadGpuDmaActivity::default()
    };
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity {
        scheduler_initial_frontiers: vec![accelerator_initial],
        scheduler_final_frontiers: vec![accelerator_final],
        ..WorkloadAcceleratorDmaActivity::default()
    };
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.gpu_dma_scheduler_initial_frontiers(),
        &[gpu_initial],
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_final_frontiers(),
        &[accelerator_final],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_initial_frontiers(),
        vec![gpu_initial, accelerator_initial],
    );
}
#[test]
fn parallel_execution_summary_copies_dma_scheduler_remote_traffic() {
    let gpu_source = PartitionId::new(6);
    let gpu_target = PartitionId::new(9);
    let accelerator_source = PartitionId::new(7);
    let accelerator_target = PartitionId::new(10);
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 16 },
    );
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity {
        scheduler_remote_flows: vec![ParallelRemoteFlowRecord::with_delay_bounds(
            gpu_source, gpu_target, 1, 11, 11, 8, 8,
        )],
        scheduler_remote_sends: vec![ParallelRemoteSendRecord::with_timing(
            gpu_source, gpu_target, 3, 11, 0,
        )],
        ..WorkloadGpuDmaActivity::default()
    };
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity {
        scheduler_remote_sends: vec![ParallelRemoteSendRecord::with_timing(
            accelerator_source,
            accelerator_target,
            2,
            10,
            0,
        )],
        ..WorkloadAcceleratorDmaActivity::default()
    };
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.gpu_dma_scheduler_remote_flow_count(gpu_source, gpu_target),
        1,
    );
    assert_eq!(
        summary
            .accelerator_dma_scheduler_remote_send_count(accelerator_source, accelerator_target,),
        1,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_flow_count(gpu_source, gpu_target),
        1,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_send_count(
            accelerator_source,
            accelerator_target,
        ),
        1,
    );
    assert!(summary.has_full_system_parallel_scheduler_remote_flows());
    assert!(summary.has_full_system_parallel_scheduler_remote_sends());
}
#[test]
fn parallel_execution_summary_copies_scheduler_remote_flows() {
    let source = PartitionId::new(0);
    let target = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 2).unwrap();
    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.schedule_remote_after(target, 4, |_| {}).unwrap();
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvSystemRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    );
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.parallel_scheduler_remote_flow_count(source, target),
        1
    );
    assert_eq!(
        summary.parallel_scheduler_initial_frontiers(),
        run.parallel_scheduler_frontiers().as_slice(),
    );
    assert_eq!(
        summary.parallel_scheduler_final_frontiers(),
        run.parallel_scheduler_final_frontiers().as_slice(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_remote_flow_count(source, target),
        1,
    );
    let scheduler_timeline = summary.parallel_scheduler_batch_timeline();
    assert_eq!(scheduler_timeline.len(), 2);
    assert!(scheduler_timeline.iter().any(|record| {
        record.partitions() == [source].as_slice() && record.worker_count() == 1
    }));
    assert!(scheduler_timeline.iter().any(|record| {
        record.partitions() == [target].as_slice() && record.worker_count() == 1
    }));
    assert_eq!(summary.parallel_scheduler_batch_worker_counts(), &[]);
    assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(1), 0);
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_worker_counts(),
        Vec::<WorkloadParallelBatchWorkerCount>::new(),
    );
    assert_eq!(summary.parallel_scheduler_batch_partition_sets(), &[]);
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_count_for_partition_set([source]),
        0,
    );
    assert_eq!(summary.parallel_scheduler_batch_partition_streaks(), &[]);
    assert_eq!(
        summary
            .full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([source]),
        0,
    );
    let flows = summary.full_system_parallel_scheduler_remote_flows();
    assert_eq!(flows.len(), 1);
    assert_eq!(flows[0].source(), source);
    assert_eq!(flows[0].target(), target);
    assert_eq!(flows[0].send_count(), 1);
    assert!(summary.has_full_system_parallel_scheduler_remote_flows());
    let sends = summary.full_system_parallel_scheduler_remote_sends();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].source(), source);
    assert_eq!(sends[0].target(), target);
    assert_eq!(sends[0].source_tick(), 0);
    assert_eq!(sends[0].delivery_tick(), 4);
    assert_eq!(sends[0].delay(), 4);
    assert_eq!(sends[0].order(), 0);
    assert!(summary.has_full_system_parallel_scheduler_remote_sends());
}
#[test]
fn parallel_execution_summary_copies_full_system_batch_partition_streaks() {
    let cpu = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let run = RiscvSystemRun::new(
        vec![batch_scheduler_turn(3, 2, &[cpu, cache])],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_batch_run(3, 2, &[cpu, cache])]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_partition_streaks(),
        vec![WorkloadParallelBatchPartitionStreak::new([cpu, cache], 2)],
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([
            cpu, cache,
        ]),
        2,
    );
}
#[test]
fn parallel_execution_summary_copies_scoped_batch_timeline() {
    let cpu = PartitionId::new(1);
    let cache = PartitionId::new(2);
    let run = RiscvSystemRun::new(
        batch_scheduler_turns_at(3, 2, 8, &[cache]),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 16 },
    )
    .with_data_cache_runs(vec![data_cache_batch_run_at(3, 2, 0, &[cpu, cache])]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_batch_timeline(),
        vec![
            WorkloadParallelBatchTimelineRecord::new(
                WorkloadParallelBatchScope::DataCacheScheduler,
                0,
                4,
                [cpu, cache],
                2,
            ),
            WorkloadParallelBatchTimelineRecord::new(
                WorkloadParallelBatchScope::Scheduler,
                4,
                8,
                [cache],
                1,
            ),
        ],
    );
}
#[test]
fn parallel_execution_summary_copies_scheduler_progress_transitions() {
    let source = PartitionId::new(0);
    let data_cache = PartitionId::new(2);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 1).unwrap();
    let scheduler_subject = component_wait_node("cpu-scheduler");
    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            context.record_progress_transition(
                scheduler_subject,
                LivelockTransitionKind::SchedulerEpoch,
            );
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvSystemRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run_with_progress(data_cache)]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(summary.parallel_scheduler_progress_transition_count(), 1);
    assert_eq!(summary.parallel_scheduler_livelock_diagnostic_count(), 0);
    assert!(!summary.has_parallel_scheduler_livelock_diagnostics());
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count(),
        1,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_livelock_diagnostic_count(),
        0,
    );
    assert!(!summary.has_data_cache_parallel_scheduler_livelock_diagnostics());
    assert_eq!(summary.full_system_progress_transition_count(), 2);
    let scheduler_transitions = summary.parallel_scheduler_progress_transitions();
    assert_eq!(scheduler_transitions.len(), 1);
    assert_eq!(scheduler_transitions[0].partition(), source);
    assert_eq!(scheduler_transitions[0].tick(), 0);
    assert_eq!(scheduler_transitions[0].order(), 0);
    assert_eq!(
        scheduler_transitions[0].kind(),
        LivelockTransitionKind::SchedulerEpoch,
    );
    let data_cache_transitions = summary.data_cache_parallel_scheduler_progress_transitions();
    assert_eq!(data_cache_transitions.len(), 1);
    assert_eq!(data_cache_transitions[0].partition(), data_cache);
    assert_eq!(data_cache_transitions[0].tick(), 0);
    assert_eq!(data_cache_transitions[0].order(), 0);
    assert_eq!(
        data_cache_transitions[0].kind(),
        LivelockTransitionKind::QueueRotation,
    );
    let full_system_transitions = summary.full_system_progress_transitions();
    assert_eq!(full_system_transitions.len(), 2);
    assert_eq!(full_system_transitions[0], scheduler_transitions[0]);
    assert_eq!(full_system_transitions[1], data_cache_transitions[0]);
    assert_eq!(summary.full_system_livelock_diagnostic_count(), 0);
    assert!(!summary.has_full_system_diagnostics());
}
#[test]
fn parallel_execution_summary_uses_livelock_transition_threshold() {
    let source = PartitionId::new(0);
    let data_cache = PartitionId::new(1);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();
    let scheduler_subject = component_wait_node("cpu-scheduler");
    scheduler
        .schedule_parallel_at(source, 0, move |context| {
            for _ in 0..2 {
                context.record_progress_transition(
                    scheduler_subject.clone(),
                    LivelockTransitionKind::ProtocolRetry,
                );
            }
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvSystemRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run_with_repeated_progress(data_cache, 2)]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        Some(2),
    );
    assert_eq!(summary.parallel_scheduler_progress_transition_count(), 2);
    assert_eq!(summary.parallel_scheduler_livelock_diagnostic_count(), 1);
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_count(),
        2,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_livelock_diagnostic_count(),
        1,
    );
    assert_eq!(summary.full_system_progress_transition_count(), 4);
    assert_eq!(summary.full_system_livelock_diagnostic_count(), 2);
    assert!(summary.has_full_system_diagnostics());
}
#[test]
fn parallel_execution_summary_preserves_livelock_diagnostic_records() {
    let cpu = PartitionId::new(0);
    let data_cache = PartitionId::new(1);
    let shared_subject = component_wait_node("shared-progress-loop");
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();
    let scheduler_subject = shared_subject.clone();
    scheduler
        .schedule_parallel_at(cpu, 0, move |context| {
            context.record_progress_transition(
                scheduler_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
            );
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvSystemRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run_with_repeated_progress_for_subject(
        data_cache,
        3,
        shared_subject.clone(),
        1,
    )]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        Some(2),
    );
    assert!(summary.parallel_scheduler_livelock_diagnostics().is_empty());
    assert!(summary
        .data_cache_parallel_scheduler_livelock_diagnostics()
        .is_empty());
    let diagnostics = summary.full_system_livelock_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].subject(), &shared_subject);
    assert_eq!(diagnostics[0].threshold(), 2);
    assert_eq!(diagnostics[0].transition_count(), 2);
    assert_eq!(
        diagnostics[0].transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        1,
    );
    assert_eq!(
        diagnostics[0].transition_count_by_kind(LivelockTransitionKind::MessageRetry),
        1,
    );
    assert_eq!(diagnostics[0].first_transition_tick(), 0);
    assert_eq!(diagnostics[0].last_transition_tick(), 3);
    assert_eq!(summary.full_system_livelock_diagnostic_count(), 1);
}
#[test]
fn parallel_execution_summary_preserves_cross_subsystem_deadlocks() {
    let packet = transaction_wait_node("fabric.packet.42");
    let line = resource_wait_node("cache.0.line.4000");
    let mut fabric_wait_for = WaitForGraph::new();
    fabric_wait_for
        .record_wait(packet.clone(), line.clone(), WaitForEdgeKind::Queue, 5)
        .unwrap();
    let mut data_cache_wait_for = WaitForGraph::new();
    data_cache_wait_for
        .record_wait(line.clone(), packet.clone(), WaitForEdgeKind::Protocol, 7)
        .unwrap();
    let data_cache_run = ParallelCoherenceRunSummary::new(
        rem6_kernel::RecordedConservativeRunSummary::empty(9),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), data_cache_wait_for),
    );
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 9 },
    )
    .with_fabric_wait_for(fabric_wait_for)
    .with_data_cache_runs(vec![data_cache_run]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    assert_eq!(run.full_system_deadlock_diagnostic_count(), 1);
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(summary.resource_deadlock_diagnostic_count(), 0);
    assert_eq!(summary.data_cache_deadlock_diagnostic_count(), 0);
    assert_eq!(summary.full_system_wait_for_edge_count(), 2);
    assert_eq!(
        summary.fabric_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(
        summary.data_cache_wait_for_edge_count_by_kind(WaitForEdgeKind::Protocol),
        1,
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Queue),
        1,
    );
    assert_eq!(
        summary.full_system_wait_for_edge_count_by_kind(WaitForEdgeKind::Protocol),
        1,
    );
    assert_eq!(
        summary.fabric_wait_for_edge_kind_window(WaitForEdgeKind::Queue),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Queue,
            1,
            5,
            5,
        )),
    );
    assert_eq!(
        summary.data_cache_wait_for_edge_kind_window(WaitForEdgeKind::Protocol),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Protocol,
            1,
            7,
            7,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_kind_window(WaitForEdgeKind::Queue),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Queue,
            1,
            5,
            5,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_edge_kind_window(WaitForEdgeKind::Protocol),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Protocol,
            1,
            7,
            7,
        )),
    );
    assert_eq!(
        summary.fabric_wait_for_blocked_node_window(&packet),
        Some(WorkloadWaitForBlockedNodeWindow::new(
            packet.clone(),
            1,
            5,
            5,
        )),
    );
    assert_eq!(
        summary.data_cache_wait_for_blocked_node_window(&line),
        Some(WorkloadWaitForBlockedNodeWindow::new(line.clone(), 1, 7, 7)),
    );
    assert_eq!(
        summary.full_system_wait_for_blocked_node_windows(),
        vec![
            WorkloadWaitForBlockedNodeWindow::new(line.clone(), 1, 7, 7),
            WorkloadWaitForBlockedNodeWindow::new(packet.clone(), 1, 5, 5),
        ],
    );
    assert_eq!(
        summary.fabric_wait_for_target_node_window(&line),
        Some(WorkloadWaitForTargetNodeWindow::new(line.clone(), 1, 5, 5)),
    );
    assert_eq!(
        summary.data_cache_wait_for_target_node_window(&packet),
        Some(WorkloadWaitForTargetNodeWindow::new(
            packet.clone(),
            1,
            7,
            7,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_target_node_windows(),
        vec![
            WorkloadWaitForTargetNodeWindow::new(line, 1, 5, 5),
            WorkloadWaitForTargetNodeWindow::new(packet, 1, 7, 7),
        ],
    );
    assert_eq!(summary.full_system_deadlock_diagnostic_count(), 1);
    assert!(summary.has_full_system_diagnostics());
}
#[test]
fn parallel_execution_summary_preserves_compute_and_dma_wait_for_edge_kinds() {
    let mut gpu_wait_for = WaitForGraph::new();
    gpu_wait_for
        .record_wait(
            transaction_wait_node("gpu.workgroup.0"),
            resource_wait_node("gpu.compute.slot.0"),
            WaitForEdgeKind::Resource,
            3,
        )
        .unwrap();
    gpu_wait_for
        .record_wait(
            transaction_wait_node("gpu.workgroup.0"),
            resource_wait_node("gpu.compute.slot.0"),
            WaitForEdgeKind::Resource,
            8,
        )
        .unwrap();
    let mut accelerator_wait_for = WaitForGraph::new();
    accelerator_wait_for
        .record_wait(
            transaction_wait_node("npu.inference.0"),
            component_wait_node("host.command.portal"),
            WaitForEdgeKind::HostAction,
            5,
        )
        .unwrap();
    let mut gpu_dma_wait_for = WaitForGraph::new();
    gpu_dma_wait_for
        .record_wait(
            transaction_wait_node("gpu.dma.0"),
            component_wait_node("fabric.route.0"),
            WaitForEdgeKind::Message,
            7,
        )
        .unwrap();
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 9 },
    );
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default().with_wait_for_graph(gpu_wait_for);
    let gpu_dma = WorkloadGpuDmaActivity::default().with_wait_for_graph(gpu_dma_wait_for);
    let accelerator =
        WorkloadAcceleratorActivity::default().with_wait_for_graph(accelerator_wait_for);
    let accelerator_dma = WorkloadAcceleratorDmaActivity {
        wait_for_edge_count: 1,
        wait_for_edge_kind_counts: [(WaitForEdgeKind::Credit, 1)].into(),
        wait_for_edge_kind_windows: vec![WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Credit,
            1,
            11,
            11,
        )],
        ..WorkloadAcceleratorDmaActivity::default()
    };
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.compute_wait_for_edge_count_by_kind(WaitForEdgeKind::Resource),
        1,
    );
    assert_eq!(
        summary.compute_wait_for_edge_count_by_kind(WaitForEdgeKind::HostAction),
        1,
    );
    assert_eq!(
        summary.dma_wait_for_edge_count_by_kind(WaitForEdgeKind::Message),
        1,
    );
    assert_eq!(
        summary.dma_wait_for_edge_count_by_kind(WaitForEdgeKind::Credit),
        1,
    );
    assert_eq!(
        summary.gpu_compute_wait_for_edge_kind_window(WaitForEdgeKind::Resource),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Resource,
            1,
            3,
            8,
        )),
    );
    assert_eq!(
        summary.accelerator_compute_wait_for_edge_kind_window(WaitForEdgeKind::HostAction),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::HostAction,
            1,
            5,
            5,
        )),
    );
    assert_eq!(
        summary.dma_wait_for_edge_kind_window(WaitForEdgeKind::Credit),
        Some(WorkloadWaitForEdgeKindWindow::new(
            WaitForEdgeKind::Credit,
            1,
            11,
            11,
        )),
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 4);
}
#[test]
fn parallel_execution_summary_copies_data_cache_scheduler_frontiers() {
    let data_cache_run = data_cache_run_with_progress(PartitionId::new(2));
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 9 },
    )
    .with_data_cache_runs(vec![data_cache_run]);
    let topology = WorkloadTopology::new(
        1,
        1,
        1,
        rem6_workload::WorkloadHostPlacement::new(0, 1, 0).unwrap(),
    )
    .unwrap();
    let gpu = WorkloadGpuActivity::default();
    let gpu_dma = WorkloadGpuDmaActivity::default();
    let accelerator = WorkloadAcceleratorActivity::default();
    let accelerator_dma = WorkloadAcceleratorDmaActivity::default();
    let summary = parallel_execution_summary(
        &run,
        &topology,
        WorkloadReplayActivityRefs {
            gpu: &gpu,
            gpu_dma: &gpu_dma,
            accelerator: &accelerator,
            accelerator_dma: &accelerator_dma,
        },
        None,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_initial_frontiers(),
        run.data_cache_parallel_scheduler_initial_frontiers()
            .as_slice(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_final_frontiers(),
        run.data_cache_parallel_scheduler_final_frontiers()
            .as_slice(),
    );
}
fn data_cache_run_with_progress(partition: PartitionId) -> ParallelCoherenceRunSummary {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 1).unwrap();
    let subject = component_wait_node("data-cache-scheduler");
    scheduler
        .schedule_parallel_at(partition, 0, move |context| {
            context.record_progress_transition(subject, LivelockTransitionKind::QueueRotation);
        })
        .unwrap();
    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        empty_coherence_wait_for_graphs(),
    )
}
fn data_cache_run_with_repeated_progress(
    partition: PartitionId,
    transition_count: usize,
) -> ParallelCoherenceRunSummary {
    data_cache_run_with_repeated_progress_for_subject(
        partition,
        0,
        component_wait_node("data-cache-scheduler"),
        transition_count,
    )
}
fn data_cache_run_with_repeated_progress_for_subject(
    partition: PartitionId,
    tick: u64,
    subject: WaitForNode,
    transition_count: usize,
) -> ParallelCoherenceRunSummary {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();
    scheduler
        .schedule_parallel_at(partition, tick, move |context| {
            for _ in 0..transition_count {
                context.record_progress_transition(
                    subject.clone(),
                    LivelockTransitionKind::MessageRetry,
                );
            }
        })
        .unwrap();
    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        empty_coherence_wait_for_graphs(),
    )
}
