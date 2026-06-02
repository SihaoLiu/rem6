use rem6_workload::{
    WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadExpectedCleanParallelDiagnostics,
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchPartitionStreak,
    WorkloadParallelBatchWorkerCount, WorkloadParallelExecutionSummary, WorkloadTopology,
};

mod conversions;

use self::conversions::{
    full_system_planned_batch_timeline, full_system_planned_batch_worker_capacity_ticks,
    full_system_planned_batch_worker_lanes, workload_data_cache_protocol,
    workload_parallel_batch_timeline_record, workload_parallel_batch_worker_lane_record,
};
use super::workload_replay_dma::WorkloadAcceleratorDmaActivity;
use crate::workload_replay_heterogeneous::{
    wait_for_blocked_node_windows_from_edges, wait_for_edge_kind_windows_from_edges,
    wait_for_target_node_windows_from_edges, WorkloadAcceleratorActivity, WorkloadGpuActivity,
};
use crate::RiscvSystemRun;

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
    let scheduler_livelock_diagnostics = livelock_transition_threshold
        .and_then(|threshold| run.parallel_scheduler_livelock_diagnostics(threshold).ok())
        .unwrap_or_default();
    let data_cache_scheduler_livelock_diagnostics = livelock_transition_threshold
        .and_then(|threshold| {
            run.data_cache_parallel_scheduler_livelock_diagnostics(threshold)
                .ok()
        })
        .unwrap_or_default();
    let full_system_livelock_diagnostics = livelock_transition_threshold
        .and_then(|threshold| run.full_system_livelock_diagnostics(threshold).ok())
        .unwrap_or_default();
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
        .with_parallel_scheduler_livelock_diagnostic_records(
            scheduler_progress_transition_count,
            scheduler_livelock_diagnostics,
        )
        .with_parallel_scheduler_progress_transitions(run.parallel_scheduler_progress_transitions())
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
        .with_parallel_scheduler_batch_timeline(
            run.parallel_scheduler_batch_timeline()
                .into_iter()
                .map(workload_parallel_batch_timeline_record),
        )
        .with_parallel_scheduler_recorded_batch_worker_capacity_ticks(
            run.parallel_scheduler_batch_worker_capacity_ticks(),
        )
        .with_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
            run.parallel_scheduler_batch_worker_slot_tick_summaries(),
        )
        .with_parallel_scheduler_planned_batch_timeline(
            run.parallel_scheduler_planned_batch_timeline()
                .into_iter()
                .map(workload_parallel_batch_timeline_record),
        )
        .with_parallel_scheduler_planned_batch_worker_lanes(
            run.parallel_scheduler_planned_batch_worker_lanes()
                .into_iter()
                .map(workload_parallel_batch_worker_lane_record),
        )
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(
            run.parallel_scheduler_planned_batch_worker_capacity_ticks(),
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
        .with_data_cache_parallel_scheduler_livelock_diagnostic_records(
            data_cache_scheduler_progress_transition_count,
            data_cache_scheduler_livelock_diagnostics,
        )
        .with_data_cache_parallel_scheduler_progress_transitions(
            run.data_cache_parallel_scheduler_progress_transitions(),
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
        .with_data_cache_parallel_scheduler_batch_timeline(
            run.data_cache_parallel_scheduler_batch_timeline()
                .into_iter()
                .map(workload_parallel_batch_timeline_record),
        )
        .with_data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(
            run.data_cache_parallel_scheduler_batch_worker_capacity_ticks(),
        )
        .with_data_cache_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
            run.data_cache_parallel_scheduler_batch_worker_slot_tick_summaries(),
        )
        .with_data_cache_parallel_scheduler_planned_batch_timeline(
            run.data_cache_parallel_scheduler_planned_batch_timeline()
                .into_iter()
                .map(workload_parallel_batch_timeline_record),
        )
        .with_data_cache_parallel_scheduler_planned_batch_worker_lanes(
            run.data_cache_parallel_scheduler_planned_batch_worker_lanes()
                .into_iter()
                .map(workload_parallel_batch_worker_lane_record),
        )
        .with_data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(
            run.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
        .with_full_system_parallel_scheduler_planned_batch_timeline(
            full_system_planned_batch_timeline(run, &activities),
        )
        .with_full_system_parallel_scheduler_planned_batch_worker_lanes(
            full_system_planned_batch_worker_lanes(run, &activities),
        )
        .with_full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(
            full_system_planned_batch_worker_capacity_ticks(run, &activities),
        )
        .with_full_system_parallel_scheduler_batch_partition_streaks(
            run.full_system_parallel_scheduler_batch_partition_streak_summaries()
                .into_iter()
                .filter(|(partitions, _)| partitions.len() >= 2)
                .map(|(partitions, batch_count)| {
                    WorkloadParallelBatchPartitionStreak::new(partitions, batch_count)
                }),
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
        .with_full_system_livelock_diagnostic_records(full_system_livelock_diagnostics)
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
        .with_data_cache_wait_for_edge_kind_counts(run.data_cache_wait_for_edge_kind_counts())
        .with_data_cache_wait_for_edge_kind_windows(wait_for_edge_kind_windows_from_edges(
            run.data_cache_wait_for_edges(),
        ))
        .with_data_cache_wait_for_blocked_node_windows(wait_for_blocked_node_windows_from_edges(
            run.data_cache_wait_for_edges(),
        ))
        .with_data_cache_wait_for_target_node_windows(wait_for_target_node_windows_from_edges(
            run.data_cache_wait_for_edges(),
        ))
        .with_fabric_activity(
            fabric.active_lane_count(),
            fabric.transfer_count(),
            fabric.byte_count(),
            fabric.occupied_ticks(),
            fabric.queue_delay_ticks(),
            fabric.max_queue_delay_ticks(),
            fabric.contended_lane_count(),
        )
        .with_fabric_lane_activities(run.fabric_activities().into_values())
        .with_fabric_hop_activities(run.fabric_hop_activities().iter().cloned())
        .with_fabric_virtual_network_activities(
            run.fabric_virtual_network_activities().into_values(),
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
        .with_dram_low_power_activity(
            dram.low_power_entry_count(rem6_dram::DramLowPowerState::PrechargePowerdown),
            dram.low_power_cycle_count(rem6_dram::DramLowPowerState::PrechargePowerdown),
            dram.low_power_entry_count(rem6_dram::DramLowPowerState::SelfRefresh),
            dram.low_power_cycle_count(rem6_dram::DramLowPowerState::SelfRefresh),
            dram.low_power_exit_count(),
            dram.low_power_exit_latency_cycles(),
        )
        .with_resource_diagnostics(
            run.fabric_wait_for_edge_count(),
            run.fabric_deadlock_diagnostic_count(),
            run.dram_wait_for_edge_count(),
            run.dram_deadlock_diagnostic_count(),
        )
        .with_resource_wait_for_edge_kind_counts(
            run.fabric_wait_for_edge_kind_counts(),
            run.dram_wait_for_edge_kind_counts(),
        )
        .with_resource_wait_for_edge_kind_windows(
            wait_for_edge_kind_windows_from_edges(run.fabric_wait_for_edges()),
            wait_for_edge_kind_windows_from_edges(run.dram_wait_for_edges()),
        )
        .with_resource_wait_for_blocked_node_windows(
            wait_for_blocked_node_windows_from_edges(run.fabric_wait_for_edges()),
            wait_for_blocked_node_windows_from_edges(run.dram_wait_for_edges()),
        )
        .with_resource_wait_for_target_node_windows(
            wait_for_target_node_windows_from_edges(run.fabric_wait_for_edges()),
            wait_for_target_node_windows_from_edges(run.dram_wait_for_edges()),
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
        .with_gpu_compute_wait_for_edge_kind_counts(
            activities.gpu.wait_for_edge_kind_counts.clone(),
        )
        .with_gpu_compute_wait_for_edge_kind_windows(
            activities.gpu.wait_for_edge_kind_windows.iter().copied(),
        )
        .with_gpu_compute_wait_for_blocked_node_windows(
            activities.gpu.wait_for_blocked_node_windows.iter().cloned(),
        )
        .with_gpu_compute_wait_for_target_node_windows(
            activities.gpu.wait_for_target_node_windows.iter().cloned(),
        )
        .with_gpu_dma_counts(
            activities.gpu_dma.copy_count,
            activities.gpu_dma.completion_count,
            activities.gpu_dma.active_device_count,
        )
        .with_gpu_dma_scheduler_counts(
            activities.gpu_dma.scheduler_epoch_count,
            activities.gpu_dma.scheduler_dispatch_count,
            activities.gpu_dma.scheduler_batch_count,
            activities
                .gpu_dma
                .scheduler_batch_worker_count_ticks
                .iter()
                .copied(),
        )
        .with_gpu_dma_scheduler_empty_epoch_count(activities.gpu_dma.scheduler_empty_epoch_count)
        .with_gpu_dma_scheduler_batch_worker_counts(
            activities
                .gpu_dma
                .scheduler_batch_worker_counts
                .iter()
                .copied(),
        )
        .with_gpu_dma_scheduler_batch_timeline(
            activities.gpu_dma.scheduler_batch_timeline.iter().cloned(),
        )
        .with_gpu_dma_scheduler_planned_batch_timeline(
            activities
                .gpu_dma
                .scheduler_planned_batch_timeline
                .iter()
                .cloned(),
        )
        .with_gpu_dma_scheduler_planned_batch_worker_capacity_ticks(
            activities
                .gpu_dma
                .scheduler_planned_batch_worker_capacity_ticks,
        )
        .with_gpu_dma_scheduler_planned_batch_worker_lanes(
            activities
                .gpu_dma
                .scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
        .with_gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(
            activities
                .gpu_dma
                .scheduler_recorded_batch_worker_capacity_ticks,
        )
        .with_gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries(
            activities
                .gpu_dma
                .scheduler_recorded_batch_worker_slot_tick_summaries
                .iter()
                .copied(),
        )
        .with_gpu_dma_scheduler_frontiers(
            activities
                .gpu_dma
                .scheduler_initial_frontiers
                .iter()
                .copied(),
            activities.gpu_dma.scheduler_final_frontiers.iter().copied(),
        )
        .with_gpu_dma_scheduler_remote_flows(
            activities.gpu_dma.scheduler_remote_flows.iter().copied(),
        )
        .with_gpu_dma_scheduler_remote_sends(
            activities.gpu_dma.scheduler_remote_sends.iter().copied(),
        )
        .with_gpu_dma_diagnostics(
            activities.gpu_dma.wait_for_edge_count,
            activities.gpu_dma.deadlock_diagnostic_count,
        )
        .with_gpu_dma_wait_for_edge_kind_counts(
            activities.gpu_dma.wait_for_edge_kind_counts.clone(),
        )
        .with_gpu_dma_wait_for_edge_kind_windows(
            activities
                .gpu_dma
                .wait_for_edge_kind_windows
                .iter()
                .copied(),
        )
        .with_gpu_dma_wait_for_blocked_node_windows(
            activities
                .gpu_dma
                .wait_for_blocked_node_windows
                .iter()
                .cloned(),
        )
        .with_gpu_dma_wait_for_target_node_windows(
            activities
                .gpu_dma
                .wait_for_target_node_windows
                .iter()
                .cloned(),
        )
        .with_accelerator_compute_counts(
            activities.accelerator.command_count,
            activities.accelerator.trace_event_count,
            activities.accelerator.completion_count,
            activities.accelerator.active_device_count,
        )
        .with_accelerator_command_kind_counts(
            activities.accelerator.gpu_kernel_command_count,
            activities.accelerator.npu_inference_command_count,
            activities.accelerator.dma_command_count,
        )
        .with_accelerator_completion_kind_counts(
            activities.accelerator.gpu_kernel_completion_count,
            activities.accelerator.npu_inference_completion_count,
            activities.accelerator.dma_command_completion_count,
        )
        .with_accelerator_compute_diagnostics(
            activities.accelerator.wait_for_edge_count,
            activities.accelerator.deadlock_diagnostic_count,
        )
        .with_accelerator_compute_wait_for_edge_kind_counts(
            activities.accelerator.wait_for_edge_kind_counts.clone(),
        )
        .with_accelerator_compute_wait_for_edge_kind_windows(
            activities
                .accelerator
                .wait_for_edge_kind_windows
                .iter()
                .copied(),
        )
        .with_accelerator_compute_wait_for_blocked_node_windows(
            activities
                .accelerator
                .wait_for_blocked_node_windows
                .iter()
                .cloned(),
        )
        .with_accelerator_compute_wait_for_target_node_windows(
            activities
                .accelerator
                .wait_for_target_node_windows
                .iter()
                .cloned(),
        )
        .with_accelerator_dma_counts(
            activities.accelerator_dma.copy_count,
            activities.accelerator_dma.completion_count,
            activities.accelerator_dma.active_device_count,
        )
        .with_accelerator_dma_scheduler_counts(
            activities.accelerator_dma.scheduler_epoch_count,
            activities.accelerator_dma.scheduler_dispatch_count,
            activities.accelerator_dma.scheduler_batch_count,
            activities
                .accelerator_dma
                .scheduler_batch_worker_count_ticks
                .iter()
                .copied(),
        )
        .with_accelerator_dma_scheduler_empty_epoch_count(
            activities.accelerator_dma.scheduler_empty_epoch_count,
        )
        .with_accelerator_dma_scheduler_batch_worker_counts(
            activities
                .accelerator_dma
                .scheduler_batch_worker_counts
                .iter()
                .copied(),
        )
        .with_accelerator_dma_scheduler_batch_timeline(
            activities
                .accelerator_dma
                .scheduler_batch_timeline
                .iter()
                .cloned(),
        )
        .with_accelerator_dma_scheduler_planned_batch_timeline(
            activities
                .accelerator_dma
                .scheduler_planned_batch_timeline
                .iter()
                .cloned(),
        )
        .with_accelerator_dma_scheduler_planned_batch_worker_capacity_ticks(
            activities
                .accelerator_dma
                .scheduler_planned_batch_worker_capacity_ticks,
        )
        .with_accelerator_dma_scheduler_planned_batch_worker_lanes(
            activities
                .accelerator_dma
                .scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
        .with_accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(
            activities
                .accelerator_dma
                .scheduler_recorded_batch_worker_capacity_ticks,
        )
        .with_accelerator_dma_scheduler_recorded_batch_worker_slot_tick_summaries(
            activities
                .accelerator_dma
                .scheduler_recorded_batch_worker_slot_tick_summaries
                .iter()
                .copied(),
        )
        .with_accelerator_dma_scheduler_frontiers(
            activities
                .accelerator_dma
                .scheduler_initial_frontiers
                .iter()
                .copied(),
            activities
                .accelerator_dma
                .scheduler_final_frontiers
                .iter()
                .copied(),
        )
        .with_accelerator_dma_scheduler_remote_flows(
            activities
                .accelerator_dma
                .scheduler_remote_flows
                .iter()
                .copied(),
        )
        .with_accelerator_dma_scheduler_remote_sends(
            activities
                .accelerator_dma
                .scheduler_remote_sends
                .iter()
                .copied(),
        )
        .with_accelerator_dma_diagnostics(
            activities.accelerator_dma.wait_for_edge_count,
            activities.accelerator_dma.deadlock_diagnostic_count,
        )
        .with_accelerator_dma_wait_for_edge_kind_counts(
            activities.accelerator_dma.wait_for_edge_kind_counts.clone(),
        )
        .with_accelerator_dma_wait_for_edge_kind_windows(
            activities
                .accelerator_dma
                .wait_for_edge_kind_windows
                .iter()
                .copied(),
        )
        .with_accelerator_dma_wait_for_blocked_node_windows(
            activities
                .accelerator_dma
                .wait_for_blocked_node_windows
                .iter()
                .cloned(),
        )
        .with_accelerator_dma_wait_for_target_node_windows(
            activities
                .accelerator_dma
                .wait_for_target_node_windows
                .iter()
                .cloned(),
        )
}
pub(super) fn livelock_transition_threshold(
    expected: &[WorkloadExpectedCleanParallelDiagnostics],
) -> Option<u64> {
    expected
        .iter()
        .filter_map(|diagnostics| diagnostics.livelock_transition_threshold())
        .min()
}
#[cfg(test)]
mod planned_batch_timeline_tests;

#[cfg(test)]
mod parallel_execution_summary_tests;
