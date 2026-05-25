use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchWorkerCount, WorkloadParallelExecutionSummary, WorkloadTopology,
};

use super::workload_replay_dma::WorkloadAcceleratorDmaActivity;
use crate::workload_replay_heterogeneous::{WorkloadAcceleratorActivity, WorkloadGpuActivity};
use crate::{RiscvDataCacheProtocol, RiscvSystemRun};

pub(super) struct WorkloadReplayActivityRefs<'a> {
    pub(super) gpu: &'a WorkloadGpuActivity,
    pub(super) gpu_dma: &'a super::WorkloadGpuDmaActivity,
    pub(super) accelerator: &'a WorkloadAcceleratorActivity,
    pub(super) accelerator_dma: &'a WorkloadAcceleratorDmaActivity,
}

pub(super) fn parallel_execution_summary(
    run: &RiscvSystemRun,
    topology: &WorkloadTopology,
    activities: WorkloadReplayActivityRefs<'_>,
    livelock_transition_threshold: Option<u64>,
) -> WorkloadParallelExecutionSummary {
    let scheduler = run.parallel_scheduler_profile();
    let fabric = run.fabric_profile();
    let dram = run.dram_profile();
    let cpu_activities = run.cpu_activities();
    let scheduler_progress_transition_count = run.parallel_scheduler_progress_transition_count();
    let data_cache_scheduler_progress_transition_count =
        run.data_cache_parallel_scheduler_progress_transition_count();
    let scheduler_livelock_diagnostic_count = livelock_transition_threshold
        .and_then(|threshold| {
            run.parallel_scheduler_livelock_diagnostic_count(threshold)
                .ok()
        })
        .unwrap_or(0);
    let data_cache_scheduler_livelock_diagnostic_count = livelock_transition_threshold
        .and_then(|threshold| {
            run.data_cache_parallel_scheduler_livelock_diagnostic_count(threshold)
                .ok()
        })
        .unwrap_or(0);
    let riscv_fetch_issue_count = cpu_activities
        .values()
        .map(|activity| activity.fetch_issue_count())
        .sum();
    let riscv_committed_instruction_count = cpu_activities
        .values()
        .map(|activity| activity.instruction_execution_count())
        .sum();
    let riscv_data_access_issue_count = cpu_activities
        .values()
        .map(|activity| activity.data_access_issue_count())
        .sum();
    let riscv_scheduled_trap_count = cpu_activities
        .values()
        .map(|activity| activity.scheduled_trap_count())
        .sum();
    WorkloadParallelExecutionSummary::default()
        .with_scheduler_counts(
            scheduler.epoch_count(),
            scheduler.empty_epoch_count(),
            scheduler.dispatch_count(),
            scheduler.batch_count(),
        )
        .with_scheduler_partitions(
            run.active_parallel_scheduler_partition_count(),
            run.max_parallel_scheduler_workers(),
        )
        .with_scheduler_worker_count(scheduler.total_parallel_workers())
        .with_parallel_scheduler_livelock_diagnostics(
            scheduler_progress_transition_count,
            scheduler_livelock_diagnostic_count,
        )
        .with_parallel_scheduler_batch_worker_counts(
            run.parallel_scheduler_batches()
                .into_iter()
                .map(|batch| WorkloadParallelBatchWorkerCount::new(batch.worker_count(), 1)),
        )
        .with_parallel_scheduler_batch_partition_sets(
            run.parallel_scheduler_batches()
                .into_iter()
                .map(|batch| WorkloadParallelBatchPartitionSet::new(batch.worker_partitions(), 1)),
        )
        .with_parallel_scheduler_batch_partition_streak_sequence(
            run.parallel_scheduler_batches()
                .into_iter()
                .map(|batch| WorkloadParallelBatchPartitionSet::new(batch.worker_partitions(), 1)),
        )
        .with_parallel_scheduler_partition_activities(run.parallel_scheduler_partition_activities())
        .with_parallel_scheduler_remote_flows(run.parallel_scheduler_remote_flows())
        .with_parallel_scheduler_remote_sends(run.parallel_scheduler_remote_sends())
        .with_parallel_scheduler_frontiers(
            run.parallel_scheduler_frontiers(),
            run.parallel_scheduler_final_frontiers(),
        )
        .with_riscv_core_counts(
            topology.riscv_cores().len(),
            cpu_activities.len(),
            riscv_fetch_issue_count,
            riscv_committed_instruction_count,
            riscv_data_access_issue_count,
            riscv_scheduled_trap_count,
        )
        .with_data_cache_parallel_counts(
            run.data_cache_run_count(),
            run.data_cache_parallel_scheduler_epoch_count(),
            run.data_cache_parallel_scheduler_dispatch_count(),
            run.data_cache_parallel_scheduler_batch_count(),
            run.data_cache_parallel_scheduler_max_workers(),
        )
        .with_data_cache_parallel_empty_epoch_count(
            run.data_cache_parallel_scheduler_empty_epoch_count(),
        )
        .with_data_cache_parallel_partitions(
            run.active_data_cache_parallel_scheduler_partition_count(),
        )
        .with_data_cache_parallel_worker_count(run.data_cache_parallel_scheduler_total_workers())
        .with_data_cache_parallel_scheduler_livelock_diagnostics(
            data_cache_scheduler_progress_transition_count,
            data_cache_scheduler_livelock_diagnostic_count,
        )
        .with_data_cache_parallel_scheduler_batch_worker_counts(
            run.data_cache_parallel_scheduler_batches()
                .into_iter()
                .map(|batch| WorkloadParallelBatchWorkerCount::new(batch.worker_count(), 1)),
        )
        .with_data_cache_parallel_scheduler_batch_partition_sets(
            run.data_cache_parallel_scheduler_batches()
                .into_iter()
                .map(|batch| WorkloadParallelBatchPartitionSet::new(batch.worker_partitions(), 1)),
        )
        .with_data_cache_parallel_scheduler_batch_partition_streak_sequence(
            run.data_cache_parallel_scheduler_batches()
                .into_iter()
                .map(|batch| WorkloadParallelBatchPartitionSet::new(batch.worker_partitions(), 1)),
        )
        .with_data_cache_parallel_scheduler_partition_activities(
            run.data_cache_parallel_scheduler_partition_activities(),
        )
        .with_full_system_parallel_partitions(
            run.active_full_system_parallel_scheduler_partition_count(),
        )
        .with_data_cache_parallel_scheduler_remote_flows(
            run.data_cache_parallel_scheduler_remote_flows(),
        )
        .with_data_cache_parallel_scheduler_remote_sends(
            run.data_cache_parallel_scheduler_remote_sends(),
        )
        .with_data_cache_parallel_scheduler_frontiers(
            run.data_cache_parallel_scheduler_initial_frontiers(),
            run.data_cache_parallel_scheduler_final_frontiers(),
        )
        .with_data_cache_run_attribution(
            run.attributed_data_cache_parallel_run_count(),
            run.unattributed_data_cache_parallel_run_count(),
        )
        .with_data_cache_protocol_counts(run.data_cache_protocol_counts().into_iter().map(
            |(protocol, run_count)| {
                WorkloadDataCacheProtocolCount::new(
                    workload_data_cache_protocol(protocol),
                    run_count,
                )
            },
        ))
        .with_data_cache_diagnostics(
            run.data_cache_wait_for_edge_count(),
            run.data_cache_deadlock_diagnostic_count(),
        )
        .with_fabric_activity(
            fabric.active_lane_count(),
            fabric.transfer_count(),
            fabric.byte_count(),
            fabric.occupied_ticks(),
            fabric.queue_delay_ticks(),
            fabric.max_queue_delay_ticks(),
            fabric.contended_lane_count(),
        )
        .with_dram_activity(
            dram.active_target_count(),
            dram.active_port_count(),
            dram.active_bank_count(),
            dram.access_count(),
            dram.read_count(),
            dram.write_count(),
            dram.row_hit_count(),
            dram.row_miss_count(),
            dram.command_count(),
            dram.turnaround_count(),
            dram.total_ready_latency_cycles(),
            dram.max_ready_latency_cycles(),
        )
        .with_dram_qos_activity(
            dram.qos_access_count(),
            dram.qos_byte_count(),
            dram.qos_escalated_access_count(),
            dram.qos_priorities().into_iter().map(|priority| {
                WorkloadDramQosPrioritySummary::new(
                    priority,
                    dram.qos_priority_access_count(priority),
                    dram.qos_priority_byte_count(priority),
                )
            }),
            dram.qos_requestors().into_iter().map(|requestor| {
                WorkloadDramQosRequestorSummary::new(
                    requestor,
                    dram.qos_requestor_access_count(requestor),
                    dram.qos_requestor_byte_count(requestor),
                )
            }),
        )
        .with_resource_diagnostics(
            run.fabric_wait_for_edge_count(),
            run.fabric_deadlock_diagnostic_count(),
            run.dram_wait_for_edge_count(),
            run.dram_deadlock_diagnostic_count(),
        )
        .with_merged_resource_deadlock_diagnostics(run.resource_deadlock_diagnostic_count())
        .with_merged_full_system_deadlock_diagnostics(run.full_system_deadlock_diagnostic_count())
        .with_gpu_compute_counts(
            activities.gpu.kernel_launch_count,
            activities.gpu.trace_event_count,
            activities.gpu.workgroup_completion_count,
            activities.gpu.active_device_count,
        )
        .with_gpu_compute_diagnostics(
            activities.gpu.wait_for_edge_count,
            activities.gpu.deadlock_diagnostic_count,
        )
        .with_gpu_dma_counts(
            activities.gpu_dma.copy_count,
            activities.gpu_dma.completion_count,
            activities.gpu_dma.active_device_count,
        )
        .with_gpu_dma_diagnostics(
            activities.gpu_dma.wait_for_edge_count,
            activities.gpu_dma.deadlock_diagnostic_count,
        )
        .with_accelerator_compute_counts(
            activities.accelerator.command_count,
            activities.accelerator.trace_event_count,
            activities.accelerator.completion_count,
            activities.accelerator.active_device_count,
        )
        .with_accelerator_compute_diagnostics(
            activities.accelerator.wait_for_edge_count,
            activities.accelerator.deadlock_diagnostic_count,
        )
        .with_accelerator_dma_counts(
            activities.accelerator_dma.copy_count,
            activities.accelerator_dma.completion_count,
            activities.accelerator_dma.active_device_count,
        )
        .with_accelerator_dma_diagnostics(
            activities.accelerator_dma.wait_for_edge_count,
            activities.accelerator_dma.deadlock_diagnostic_count,
        )
}

fn workload_data_cache_protocol(protocol: RiscvDataCacheProtocol) -> WorkloadDataCacheProtocol {
    match protocol {
        RiscvDataCacheProtocol::Msi => WorkloadDataCacheProtocol::Msi,
        RiscvDataCacheProtocol::Mesi => WorkloadDataCacheProtocol::Mesi,
        RiscvDataCacheProtocol::Moesi => WorkloadDataCacheProtocol::Moesi,
        RiscvDataCacheProtocol::Chi => WorkloadDataCacheProtocol::Chi,
    }
}

#[cfg(test)]
mod tests {
    use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
    use rem6_dram::{
        DramController, DramGeometry, DramQosRequest, DramQosSchedulingPolicy,
        DramQosTurnaroundPolicy, DramTargetActivity, DramTiming,
    };
    use rem6_fabric::{QosPriority, QosQueueArbiter, QosQueuePolicyKind, QosRequestorId};
    use rem6_kernel::{
        LivelockTransitionKind, PartitionId, PartitionedScheduler, WaitForEdgeKind, WaitForGraph,
        WaitForNode,
    };
    use rem6_memory::{
        AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId,
        MemoryTargetId,
    };
    use rem6_workload::WorkloadParallelBatchPartitionStreak;

    use super::*;
    use crate::workload_replay::{WorkloadGpuDmaActivity, WorkloadReplayActivityRefs};
    use crate::workload_replay_heterogeneous::{WorkloadAcceleratorActivity, WorkloadGpuActivity};
    use crate::{RiscvClusterTurn, RiscvSystemRunStopReason};

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
        assert_eq!(
            summary.parallel_scheduler_batch_worker_counts(),
            &[WorkloadParallelBatchWorkerCount::new(1, 2)],
        );
        assert_eq!(summary.parallel_scheduler_batch_count_at_or_above(1), 2);
        assert_eq!(
            summary.full_system_parallel_scheduler_batch_worker_counts(),
            vec![WorkloadParallelBatchWorkerCount::new(1, 2)],
        );
        assert_eq!(
            summary.parallel_scheduler_batch_partition_sets(),
            &[
                WorkloadParallelBatchPartitionSet::new([source], 1),
                WorkloadParallelBatchPartitionSet::new([target], 1),
            ],
        );
        assert_eq!(
            summary.full_system_parallel_scheduler_batch_count_for_partition_set([source]),
            1,
        );
        assert_eq!(
            summary.parallel_scheduler_batch_partition_streaks(),
            &[
                WorkloadParallelBatchPartitionStreak::new([source], 1),
                WorkloadParallelBatchPartitionStreak::new([target], 1),
            ],
        );
        assert_eq!(
            summary.full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set([
                source
            ]),
            1,
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
    fn parallel_execution_summary_preserves_cross_subsystem_deadlocks() {
        let packet = transaction_wait_node("fabric.packet.42");
        let line = resource_wait_node("cache.0.line.4000");
        let mut fabric_wait_for = WaitForGraph::new();
        fabric_wait_for
            .record_wait(packet.clone(), line.clone(), WaitForEdgeKind::Queue, 5)
            .unwrap();
        let mut data_cache_wait_for = WaitForGraph::new();
        data_cache_wait_for
            .record_wait(line, packet, WaitForEdgeKind::Protocol, 7)
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
        assert_eq!(summary.full_system_deadlock_diagnostic_count(), 1);
        assert!(summary.has_full_system_diagnostics());
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
        let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 4, 1).unwrap();
        let subject = component_wait_node("data-cache-scheduler");
        scheduler
            .schedule_parallel_at(partition, 0, move |context| {
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
}
