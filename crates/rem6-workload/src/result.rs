use std::collections::BTreeMap;

use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::{ParallelPartitionActivity, ParallelRemoteFlowRecord, PartitionId};

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets, collect_parallel_batch_partition_streaks,
    collect_parallel_batch_partition_streaks_from_sequence, collect_parallel_batch_worker_counts,
    normalize_partition_set, parallel_batch_count_at_or_above,
    parallel_batch_count_for_partition_set, parallel_batch_streak_count_for_partition_set,
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchPartitionStreak,
    WorkloadParallelBatchWorkerCount,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadDataCacheProtocol {
    Msi,
    Mesi,
    Moesi,
    Chi,
}

impl WorkloadDataCacheProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Msi => "msi",
            Self::Mesi => "mesi",
            Self::Moesi => "moesi",
            Self::Chi => "chi",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadDataCacheProtocolCount {
    protocol: WorkloadDataCacheProtocol,
    run_count: usize,
}

impl WorkloadDataCacheProtocolCount {
    pub const fn new(protocol: WorkloadDataCacheProtocol, run_count: usize) -> Self {
        Self {
            protocol,
            run_count,
        }
    }

    pub const fn protocol(&self) -> WorkloadDataCacheProtocol {
        self.protocol
    }

    pub const fn run_count(&self) -> usize {
        self.run_count
    }

    pub const fn is_empty(&self) -> bool {
        self.run_count == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadDramQosPrioritySummary {
    priority: QosPriority,
    access_count: usize,
    byte_count: u64,
}

impl WorkloadDramQosPrioritySummary {
    pub const fn new(priority: QosPriority, access_count: usize, byte_count: u64) -> Self {
        Self {
            priority,
            access_count,
            byte_count,
        }
    }

    pub const fn priority(&self) -> QosPriority {
        self.priority
    }

    pub const fn access_count(&self) -> usize {
        self.access_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn is_empty(&self) -> bool {
        self.access_count == 0 && self.byte_count == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadDramQosRequestorSummary {
    requestor: QosRequestorId,
    access_count: usize,
    byte_count: u64,
}

impl WorkloadDramQosRequestorSummary {
    pub const fn new(requestor: QosRequestorId, access_count: usize, byte_count: u64) -> Self {
        Self {
            requestor,
            access_count,
            byte_count,
        }
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub const fn access_count(&self) -> usize {
        self.access_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub const fn is_empty(&self) -> bool {
        self.access_count == 0 && self.byte_count == 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkloadParallelExecutionSummary {
    scheduler_epoch_count: usize,
    scheduler_empty_epoch_count: usize,
    scheduler_dispatch_count: usize,
    scheduler_batch_count: usize,
    active_scheduler_partition_count: usize,
    max_parallel_scheduler_workers: usize,
    total_parallel_scheduler_workers: usize,
    scheduler_progress_transition_count: usize,
    scheduler_livelock_diagnostic_count: usize,
    parallel_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    parallel_scheduler_batch_partition_sets: Vec<WorkloadParallelBatchPartitionSet>,
    parallel_scheduler_batch_partition_streaks: Vec<WorkloadParallelBatchPartitionStreak>,
    parallel_scheduler_partition_activities: Vec<(PartitionId, ParallelPartitionActivity)>,
    parallel_scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    riscv_core_count: usize,
    active_riscv_core_count: usize,
    riscv_fetch_issue_count: usize,
    riscv_committed_instruction_count: usize,
    riscv_data_access_issue_count: usize,
    riscv_scheduled_trap_count: usize,
    data_cache_parallel_run_count: usize,
    data_cache_parallel_scheduler_epoch_count: usize,
    data_cache_parallel_scheduler_empty_epoch_count: usize,
    data_cache_parallel_scheduler_dispatch_count: usize,
    data_cache_parallel_scheduler_batch_count: usize,
    active_data_cache_parallel_scheduler_partition_count: usize,
    data_cache_parallel_scheduler_max_workers: usize,
    data_cache_parallel_scheduler_total_workers: usize,
    data_cache_parallel_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    data_cache_parallel_scheduler_batch_partition_sets: Vec<WorkloadParallelBatchPartitionSet>,
    data_cache_parallel_scheduler_batch_partition_streaks:
        Vec<WorkloadParallelBatchPartitionStreak>,
    active_full_system_parallel_scheduler_partition_count: usize,
    data_cache_parallel_scheduler_partition_activities:
        Vec<(PartitionId, ParallelPartitionActivity)>,
    data_cache_parallel_scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    attributed_data_cache_parallel_run_count: usize,
    unattributed_data_cache_parallel_run_count: usize,
    data_cache_protocol_counts: Vec<WorkloadDataCacheProtocolCount>,
    data_cache_wait_for_edge_count: usize,
    data_cache_deadlock_diagnostic_count: usize,
    data_cache_parallel_scheduler_progress_transition_count: usize,
    data_cache_parallel_scheduler_livelock_diagnostic_count: usize,
    active_fabric_lane_count: usize,
    fabric_transfer_count: usize,
    fabric_byte_count: u64,
    fabric_occupied_ticks: u64,
    fabric_queue_delay_ticks: u64,
    fabric_max_queue_delay_ticks: u64,
    contended_fabric_lane_count: usize,
    fabric_wait_for_edge_count: usize,
    fabric_deadlock_diagnostic_count: usize,
    active_dram_target_count: usize,
    active_dram_port_count: usize,
    active_dram_bank_count: usize,
    dram_access_count: usize,
    dram_read_count: usize,
    dram_write_count: usize,
    dram_row_hit_count: usize,
    dram_row_miss_count: usize,
    dram_command_count: usize,
    dram_turnaround_count: usize,
    dram_total_ready_latency_cycles: u64,
    dram_max_ready_latency_cycles: u64,
    dram_qos_access_count: usize,
    dram_qos_byte_count: u64,
    dram_qos_escalated_access_count: usize,
    dram_qos_priority_summaries: Vec<WorkloadDramQosPrioritySummary>,
    dram_qos_requestor_summaries: Vec<WorkloadDramQosRequestorSummary>,
    dram_wait_for_edge_count: usize,
    dram_deadlock_diagnostic_count: usize,
    merged_resource_deadlock_diagnostic_count: usize,
    merged_full_system_deadlock_diagnostic_count: usize,
    gpu_kernel_launch_count: usize,
    gpu_trace_event_count: usize,
    gpu_workgroup_completion_count: usize,
    active_gpu_device_count: usize,
    gpu_compute_wait_for_edge_count: usize,
    gpu_compute_deadlock_diagnostic_count: usize,
    gpu_dma_copy_count: usize,
    gpu_dma_completion_count: usize,
    active_gpu_dma_device_count: usize,
    gpu_dma_wait_for_edge_count: usize,
    gpu_dma_deadlock_diagnostic_count: usize,
    accelerator_command_count: usize,
    accelerator_trace_event_count: usize,
    accelerator_completion_count: usize,
    active_accelerator_device_count: usize,
    accelerator_compute_wait_for_edge_count: usize,
    accelerator_compute_deadlock_diagnostic_count: usize,
    accelerator_dma_copy_count: usize,
    accelerator_dma_completion_count: usize,
    active_accelerator_dma_device_count: usize,
    accelerator_dma_wait_for_edge_count: usize,
    accelerator_dma_deadlock_diagnostic_count: usize,
}

impl WorkloadParallelExecutionSummary {
    pub const fn with_scheduler_counts(
        mut self,
        epoch_count: usize,
        empty_epoch_count: usize,
        dispatch_count: usize,
        batch_count: usize,
    ) -> Self {
        self.scheduler_epoch_count = epoch_count;
        self.scheduler_empty_epoch_count = empty_epoch_count;
        self.scheduler_dispatch_count = dispatch_count;
        self.scheduler_batch_count = batch_count;
        self
    }

    pub const fn with_scheduler_partitions(
        mut self,
        active_partition_count: usize,
        max_parallel_workers: usize,
    ) -> Self {
        self.active_scheduler_partition_count = active_partition_count;
        self.max_parallel_scheduler_workers = max_parallel_workers;
        self
    }

    pub const fn with_scheduler_worker_count(mut self, total_worker_count: usize) -> Self {
        self.total_parallel_scheduler_workers = total_worker_count;
        self
    }

    pub fn with_parallel_scheduler_batch_worker_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
    ) -> Self {
        self.parallel_scheduler_batch_worker_counts = collect_parallel_batch_worker_counts(counts);
        self.scheduler_batch_count = self
            .parallel_scheduler_batch_worker_counts
            .iter()
            .map(WorkloadParallelBatchWorkerCount::batch_count)
            .sum();
        self
    }

    pub fn with_parallel_scheduler_batch_partition_sets(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.parallel_scheduler_batch_partition_sets = collect_parallel_batch_partition_sets(sets);
        self.scheduler_batch_count = self
            .parallel_scheduler_batch_partition_sets
            .iter()
            .map(WorkloadParallelBatchPartitionSet::batch_count)
            .sum();
        self
    }

    pub fn with_parallel_scheduler_batch_partition_streaks(
        mut self,
        streaks: impl IntoIterator<Item = WorkloadParallelBatchPartitionStreak>,
    ) -> Self {
        self.parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks(streaks);
        self
    }

    pub fn with_parallel_scheduler_batch_partition_streak_sequence(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks_from_sequence(sets);
        self
    }

    pub fn with_parallel_scheduler_remote_flows(
        mut self,
        flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    ) -> Self {
        self.parallel_scheduler_remote_flows = collect_parallel_remote_flows(flows);
        self
    }

    pub fn with_parallel_scheduler_partition_activities(
        mut self,
        activities: impl IntoIterator<Item = (PartitionId, ParallelPartitionActivity)>,
    ) -> Self {
        self.parallel_scheduler_partition_activities =
            collect_parallel_partition_activities(activities);
        self.active_scheduler_partition_count = self.parallel_scheduler_partition_activities.len();
        self
    }

    pub const fn with_parallel_scheduler_livelock_diagnostics(
        mut self,
        progress_transition_count: usize,
        livelock_diagnostic_count: usize,
    ) -> Self {
        self.scheduler_progress_transition_count = progress_transition_count;
        self.scheduler_livelock_diagnostic_count = livelock_diagnostic_count;
        self
    }

    pub const fn with_riscv_core_counts(
        mut self,
        core_count: usize,
        active_core_count: usize,
        fetch_issue_count: usize,
        committed_instruction_count: usize,
        data_access_issue_count: usize,
        scheduled_trap_count: usize,
    ) -> Self {
        self.riscv_core_count = core_count;
        self.active_riscv_core_count = active_core_count;
        self.riscv_fetch_issue_count = fetch_issue_count;
        self.riscv_committed_instruction_count = committed_instruction_count;
        self.riscv_data_access_issue_count = data_access_issue_count;
        self.riscv_scheduled_trap_count = scheduled_trap_count;
        self
    }

    pub const fn with_data_cache_parallel_counts(
        mut self,
        run_count: usize,
        scheduler_epoch_count: usize,
        scheduler_dispatch_count: usize,
        scheduler_batch_count: usize,
        scheduler_max_workers: usize,
    ) -> Self {
        self.data_cache_parallel_run_count = run_count;
        self.data_cache_parallel_scheduler_epoch_count = scheduler_epoch_count;
        self.data_cache_parallel_scheduler_dispatch_count = scheduler_dispatch_count;
        self.data_cache_parallel_scheduler_batch_count = scheduler_batch_count;
        self.data_cache_parallel_scheduler_max_workers = scheduler_max_workers;
        self
    }

    pub const fn with_data_cache_parallel_empty_epoch_count(
        mut self,
        empty_epoch_count: usize,
    ) -> Self {
        self.data_cache_parallel_scheduler_empty_epoch_count = empty_epoch_count;
        self
    }

    pub const fn with_data_cache_parallel_partitions(
        mut self,
        active_partition_count: usize,
    ) -> Self {
        self.active_data_cache_parallel_scheduler_partition_count = active_partition_count;
        self
    }

    pub const fn with_data_cache_parallel_worker_count(
        mut self,
        total_worker_count: usize,
    ) -> Self {
        self.data_cache_parallel_scheduler_total_workers = total_worker_count;
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_worker_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts(counts);
        self.data_cache_parallel_scheduler_batch_count = self
            .data_cache_parallel_scheduler_batch_worker_counts
            .iter()
            .map(WorkloadParallelBatchWorkerCount::batch_count)
            .sum();
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_partition_sets(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_partition_sets =
            collect_parallel_batch_partition_sets(sets);
        self.data_cache_parallel_scheduler_batch_count = self
            .data_cache_parallel_scheduler_batch_partition_sets
            .iter()
            .map(WorkloadParallelBatchPartitionSet::batch_count)
            .sum();
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_partition_streaks(
        mut self,
        streaks: impl IntoIterator<Item = WorkloadParallelBatchPartitionStreak>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks(streaks);
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_partition_streak_sequence(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks_from_sequence(sets);
        self
    }

    pub const fn with_full_system_parallel_partitions(
        mut self,
        active_partition_count: usize,
    ) -> Self {
        self.active_full_system_parallel_scheduler_partition_count = active_partition_count;
        self
    }

    pub fn with_data_cache_parallel_scheduler_remote_flows(
        mut self,
        flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    ) -> Self {
        self.data_cache_parallel_scheduler_remote_flows = collect_parallel_remote_flows(flows);
        self
    }

    pub fn with_data_cache_parallel_scheduler_partition_activities(
        mut self,
        activities: impl IntoIterator<Item = (PartitionId, ParallelPartitionActivity)>,
    ) -> Self {
        self.data_cache_parallel_scheduler_partition_activities =
            collect_parallel_partition_activities(activities);
        self.active_data_cache_parallel_scheduler_partition_count = self
            .data_cache_parallel_scheduler_partition_activities
            .len();
        self
    }

    pub const fn with_data_cache_run_attribution(
        mut self,
        attributed_run_count: usize,
        unattributed_run_count: usize,
    ) -> Self {
        self.attributed_data_cache_parallel_run_count = attributed_run_count;
        self.unattributed_data_cache_parallel_run_count = unattributed_run_count;
        self
    }

    pub fn with_data_cache_protocol_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadDataCacheProtocolCount>,
    ) -> Self {
        let mut by_protocol = BTreeMap::new();
        for count in counts {
            if count.run_count() != 0 {
                by_protocol.insert(count.protocol(), count.run_count());
            }
        }
        self.data_cache_protocol_counts = by_protocol
            .into_iter()
            .map(|(protocol, run_count)| WorkloadDataCacheProtocolCount::new(protocol, run_count))
            .collect();
        self
    }

    pub const fn with_data_cache_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.data_cache_wait_for_edge_count = wait_for_edge_count;
        self.data_cache_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn with_data_cache_parallel_scheduler_livelock_diagnostics(
        mut self,
        progress_transition_count: usize,
        livelock_diagnostic_count: usize,
    ) -> Self {
        self.data_cache_parallel_scheduler_progress_transition_count = progress_transition_count;
        self.data_cache_parallel_scheduler_livelock_diagnostic_count = livelock_diagnostic_count;
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn with_fabric_activity(
        mut self,
        active_lane_count: usize,
        transfer_count: usize,
        byte_count: u64,
        occupied_ticks: u64,
        queue_delay_ticks: u64,
        max_queue_delay_ticks: u64,
        contended_lane_count: usize,
    ) -> Self {
        self.active_fabric_lane_count = active_lane_count;
        self.fabric_transfer_count = transfer_count;
        self.fabric_byte_count = byte_count;
        self.fabric_occupied_ticks = occupied_ticks;
        self.fabric_queue_delay_ticks = queue_delay_ticks;
        self.fabric_max_queue_delay_ticks = max_queue_delay_ticks;
        self.contended_fabric_lane_count = contended_lane_count;
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn with_dram_activity(
        mut self,
        active_target_count: usize,
        active_port_count: usize,
        active_bank_count: usize,
        access_count: usize,
        read_count: usize,
        write_count: usize,
        row_hit_count: usize,
        row_miss_count: usize,
        command_count: usize,
        turnaround_count: usize,
        total_ready_latency_cycles: u64,
        max_ready_latency_cycles: u64,
    ) -> Self {
        self.active_dram_target_count = active_target_count;
        self.active_dram_port_count = active_port_count;
        self.active_dram_bank_count = active_bank_count;
        self.dram_access_count = access_count;
        self.dram_read_count = read_count;
        self.dram_write_count = write_count;
        self.dram_row_hit_count = row_hit_count;
        self.dram_row_miss_count = row_miss_count;
        self.dram_command_count = command_count;
        self.dram_turnaround_count = turnaround_count;
        self.dram_total_ready_latency_cycles = total_ready_latency_cycles;
        self.dram_max_ready_latency_cycles = max_ready_latency_cycles;
        self
    }

    pub fn with_dram_qos_activity(
        mut self,
        access_count: usize,
        byte_count: u64,
        escalated_access_count: usize,
        priority_summaries: impl IntoIterator<Item = WorkloadDramQosPrioritySummary>,
        requestor_summaries: impl IntoIterator<Item = WorkloadDramQosRequestorSummary>,
    ) -> Self {
        self.dram_qos_access_count = access_count;
        self.dram_qos_byte_count = byte_count;
        self.dram_qos_escalated_access_count = escalated_access_count;
        self.dram_qos_priority_summaries = collect_priority_summaries(priority_summaries);
        self.dram_qos_requestor_summaries = collect_requestor_summaries(requestor_summaries);
        self
    }

    pub const fn with_resource_diagnostics(
        mut self,
        fabric_wait_for_edge_count: usize,
        fabric_deadlock_diagnostic_count: usize,
        dram_wait_for_edge_count: usize,
        dram_deadlock_diagnostic_count: usize,
    ) -> Self {
        self.fabric_wait_for_edge_count = fabric_wait_for_edge_count;
        self.fabric_deadlock_diagnostic_count = fabric_deadlock_diagnostic_count;
        self.dram_wait_for_edge_count = dram_wait_for_edge_count;
        self.dram_deadlock_diagnostic_count = dram_deadlock_diagnostic_count;
        self
    }

    pub const fn with_merged_resource_deadlock_diagnostics(
        mut self,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.merged_resource_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn with_merged_full_system_deadlock_diagnostics(
        mut self,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.merged_full_system_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn with_gpu_compute_counts(
        mut self,
        kernel_launch_count: usize,
        trace_event_count: usize,
        workgroup_completion_count: usize,
        active_device_count: usize,
    ) -> Self {
        self.gpu_kernel_launch_count = kernel_launch_count;
        self.gpu_trace_event_count = trace_event_count;
        self.gpu_workgroup_completion_count = workgroup_completion_count;
        self.active_gpu_device_count = active_device_count;
        self
    }

    pub const fn with_gpu_compute_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.gpu_compute_wait_for_edge_count = wait_for_edge_count;
        self.gpu_compute_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn with_gpu_dma_counts(
        mut self,
        copy_count: usize,
        completion_count: usize,
        active_device_count: usize,
    ) -> Self {
        self.gpu_dma_copy_count = copy_count;
        self.gpu_dma_completion_count = completion_count;
        self.active_gpu_dma_device_count = active_device_count;
        self
    }

    pub const fn with_gpu_dma_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.gpu_dma_wait_for_edge_count = wait_for_edge_count;
        self.gpu_dma_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn with_accelerator_compute_counts(
        mut self,
        command_count: usize,
        trace_event_count: usize,
        completion_count: usize,
        active_device_count: usize,
    ) -> Self {
        self.accelerator_command_count = command_count;
        self.accelerator_trace_event_count = trace_event_count;
        self.accelerator_completion_count = completion_count;
        self.active_accelerator_device_count = active_device_count;
        self
    }

    pub const fn with_accelerator_compute_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.accelerator_compute_wait_for_edge_count = wait_for_edge_count;
        self.accelerator_compute_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn with_accelerator_dma_counts(
        mut self,
        copy_count: usize,
        completion_count: usize,
        active_device_count: usize,
    ) -> Self {
        self.accelerator_dma_copy_count = copy_count;
        self.accelerator_dma_completion_count = completion_count;
        self.active_accelerator_dma_device_count = active_device_count;
        self
    }

    pub const fn with_accelerator_dma_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.accelerator_dma_wait_for_edge_count = wait_for_edge_count;
        self.accelerator_dma_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn scheduler_epoch_count(&self) -> usize {
        self.scheduler_epoch_count
    }

    pub const fn scheduler_empty_epoch_count(&self) -> usize {
        self.scheduler_empty_epoch_count
    }

    pub const fn scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count
    }

    pub const fn scheduler_batch_count(&self) -> usize {
        self.scheduler_batch_count
    }

    pub const fn active_scheduler_partition_count(&self) -> usize {
        self.active_scheduler_partition_count
    }

    pub const fn max_parallel_scheduler_workers(&self) -> usize {
        self.max_parallel_scheduler_workers
    }

    pub const fn total_parallel_scheduler_workers(&self) -> usize {
        self.total_parallel_scheduler_workers
    }

    pub const fn parallel_scheduler_progress_transition_count(&self) -> usize {
        self.scheduler_progress_transition_count
    }

    pub const fn parallel_scheduler_livelock_diagnostic_count(&self) -> usize {
        self.scheduler_livelock_diagnostic_count
    }

    pub const fn has_parallel_scheduler_livelock_diagnostics(&self) -> bool {
        self.scheduler_livelock_diagnostic_count != 0
    }

    pub fn parallel_scheduler_remote_flows(&self) -> &[ParallelRemoteFlowRecord] {
        &self.parallel_scheduler_remote_flows
    }

    pub fn parallel_scheduler_batch_worker_counts(&self) -> &[WorkloadParallelBatchWorkerCount] {
        &self.parallel_scheduler_batch_worker_counts
    }

    pub fn parallel_scheduler_batch_partition_sets(&self) -> &[WorkloadParallelBatchPartitionSet] {
        &self.parallel_scheduler_batch_partition_sets
    }

    pub fn parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> &[WorkloadParallelBatchPartitionStreak] {
        &self.parallel_scheduler_batch_partition_streaks
    }

    pub fn parallel_scheduler_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        parallel_batch_count_at_or_above(
            &self.parallel_scheduler_batch_worker_counts,
            minimum_worker_count,
        )
    }

    pub fn parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_count_for_partition_set(
            &self.parallel_scheduler_batch_partition_sets,
            partitions,
        )
    }

    pub fn parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_streak_count_for_partition_set(
            &self.parallel_scheduler_batch_partition_streaks,
            partitions,
        )
    }

    pub fn parallel_scheduler_partition_activities(
        &self,
    ) -> &[(PartitionId, ParallelPartitionActivity)] {
        &self.parallel_scheduler_partition_activities
    }

    pub fn parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.parallel_scheduler_partition_activities
            .iter()
            .find(|(existing, _)| *existing == partition)
            .map(|(_, activity)| *activity)
    }

    pub fn parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_count(&self.parallel_scheduler_remote_flows, source, target)
    }

    pub fn has_parallel_scheduler_remote_flows(&self) -> bool {
        !self.parallel_scheduler_remote_flows.is_empty()
    }

    pub const fn riscv_core_count(&self) -> usize {
        self.riscv_core_count
    }

    pub const fn active_riscv_core_count(&self) -> usize {
        self.active_riscv_core_count
    }

    pub const fn riscv_fetch_issue_count(&self) -> usize {
        self.riscv_fetch_issue_count
    }

    pub const fn riscv_committed_instruction_count(&self) -> usize {
        self.riscv_committed_instruction_count
    }

    pub const fn riscv_data_access_issue_count(&self) -> usize {
        self.riscv_data_access_issue_count
    }

    pub const fn riscv_scheduled_trap_count(&self) -> usize {
        self.riscv_scheduled_trap_count
    }

    pub const fn has_riscv_core_activity(&self) -> bool {
        self.riscv_fetch_issue_count != 0
            || self.riscv_committed_instruction_count != 0
            || self.riscv_data_access_issue_count != 0
            || self.riscv_scheduled_trap_count != 0
    }

    pub const fn data_cache_parallel_run_count(&self) -> usize {
        self.data_cache_parallel_run_count
    }

    pub const fn data_cache_parallel_scheduler_epoch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_epoch_count
    }

    pub const fn data_cache_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_empty_epoch_count
    }

    pub const fn data_cache_parallel_scheduler_dispatch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_dispatch_count
    }

    pub const fn data_cache_parallel_scheduler_batch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_batch_count
    }

    pub const fn active_data_cache_parallel_scheduler_partition_count(&self) -> usize {
        self.active_data_cache_parallel_scheduler_partition_count
    }

    pub const fn data_cache_parallel_scheduler_max_workers(&self) -> usize {
        self.data_cache_parallel_scheduler_max_workers
    }

    pub const fn data_cache_parallel_scheduler_total_workers(&self) -> usize {
        self.data_cache_parallel_scheduler_total_workers
    }

    pub fn data_cache_parallel_scheduler_remote_flows(&self) -> &[ParallelRemoteFlowRecord] {
        &self.data_cache_parallel_scheduler_remote_flows
    }

    pub fn data_cache_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> &[WorkloadParallelBatchWorkerCount] {
        &self.data_cache_parallel_scheduler_batch_worker_counts
    }

    pub fn data_cache_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> &[WorkloadParallelBatchPartitionSet] {
        &self.data_cache_parallel_scheduler_batch_partition_sets
    }

    pub fn data_cache_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> &[WorkloadParallelBatchPartitionStreak] {
        &self.data_cache_parallel_scheduler_batch_partition_streaks
    }

    pub fn data_cache_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        parallel_batch_count_at_or_above(
            &self.data_cache_parallel_scheduler_batch_worker_counts,
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_count_for_partition_set(
            &self.data_cache_parallel_scheduler_batch_partition_sets,
            partitions,
        )
    }

    pub fn data_cache_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_streak_count_for_partition_set(
            &self.data_cache_parallel_scheduler_batch_partition_streaks,
            partitions,
        )
    }

    pub fn data_cache_parallel_scheduler_partition_activities(
        &self,
    ) -> &[(PartitionId, ParallelPartitionActivity)] {
        &self.data_cache_parallel_scheduler_partition_activities
    }

    pub fn data_cache_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.data_cache_parallel_scheduler_partition_activities
            .iter()
            .find(|(existing, _)| *existing == partition)
            .map(|(_, activity)| *activity)
    }

    pub fn data_cache_parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_count(
            &self.data_cache_parallel_scheduler_remote_flows,
            source,
            target,
        )
    }

    pub fn has_data_cache_parallel_scheduler_remote_flows(&self) -> bool {
        !self.data_cache_parallel_scheduler_remote_flows.is_empty()
    }

    pub const fn attributed_data_cache_parallel_run_count(&self) -> usize {
        self.attributed_data_cache_parallel_run_count
    }

    pub const fn unattributed_data_cache_parallel_run_count(&self) -> usize {
        self.unattributed_data_cache_parallel_run_count
    }

    pub fn data_cache_protocol_counts(&self) -> &[WorkloadDataCacheProtocolCount] {
        &self.data_cache_protocol_counts
    }

    pub fn data_cache_protocols(&self) -> Vec<WorkloadDataCacheProtocol> {
        self.data_cache_protocol_counts
            .iter()
            .map(WorkloadDataCacheProtocolCount::protocol)
            .collect()
    }

    pub fn data_cache_protocol_count_map(&self) -> BTreeMap<WorkloadDataCacheProtocol, usize> {
        self.data_cache_protocol_counts
            .iter()
            .map(|count| (count.protocol(), count.run_count()))
            .collect()
    }

    pub fn attributed_data_cache_protocol_run_count(&self) -> usize {
        self.data_cache_protocol_counts
            .iter()
            .map(WorkloadDataCacheProtocolCount::run_count)
            .sum()
    }

    pub fn data_cache_parallel_run_count_for_protocol(
        &self,
        protocol: WorkloadDataCacheProtocol,
    ) -> usize {
        self.data_cache_protocol_counts
            .iter()
            .find(|count| count.protocol() == protocol)
            .map(WorkloadDataCacheProtocolCount::run_count)
            .unwrap_or(0)
    }

    pub fn has_data_cache_protocol(&self, protocol: WorkloadDataCacheProtocol) -> bool {
        self.data_cache_parallel_run_count_for_protocol(protocol) != 0
    }

    pub const fn has_unattributed_data_cache_parallel_runs(&self) -> bool {
        self.unattributed_data_cache_parallel_run_count != 0
    }

    pub const fn data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_wait_for_edge_count
    }

    pub const fn data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.data_cache_deadlock_diagnostic_count
    }

    pub const fn data_cache_parallel_scheduler_progress_transition_count(&self) -> usize {
        self.data_cache_parallel_scheduler_progress_transition_count
    }

    pub const fn data_cache_parallel_scheduler_livelock_diagnostic_count(&self) -> usize {
        self.data_cache_parallel_scheduler_livelock_diagnostic_count
    }

    pub const fn has_data_cache_parallel_scheduler_livelock_diagnostics(&self) -> bool {
        self.data_cache_parallel_scheduler_livelock_diagnostic_count != 0
    }

    pub const fn has_data_cache_diagnostics(&self) -> bool {
        self.data_cache_wait_for_edge_count != 0
            || self.data_cache_deadlock_diagnostic_count != 0
            || self.has_data_cache_parallel_scheduler_livelock_diagnostics()
    }

    pub const fn fabric_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count
    }

    pub const fn fabric_deadlock_diagnostic_count(&self) -> usize {
        self.fabric_deadlock_diagnostic_count
    }

    pub const fn has_fabric_diagnostics(&self) -> bool {
        self.fabric_wait_for_edge_count != 0 || self.fabric_deadlock_diagnostic_count != 0
    }

    pub const fn active_fabric_lane_count(&self) -> usize {
        self.active_fabric_lane_count
    }

    pub const fn fabric_transfer_count(&self) -> usize {
        self.fabric_transfer_count
    }

    pub const fn fabric_byte_count(&self) -> u64 {
        self.fabric_byte_count
    }

    pub const fn fabric_occupied_ticks(&self) -> u64 {
        self.fabric_occupied_ticks
    }

    pub const fn fabric_queue_delay_ticks(&self) -> u64 {
        self.fabric_queue_delay_ticks
    }

    pub const fn fabric_max_queue_delay_ticks(&self) -> u64 {
        self.fabric_max_queue_delay_ticks
    }

    pub const fn contended_fabric_lane_count(&self) -> usize {
        self.contended_fabric_lane_count
    }

    pub const fn has_fabric_activity(&self) -> bool {
        self.fabric_transfer_count != 0
    }

    pub const fn has_fabric_contention(&self) -> bool {
        self.contended_fabric_lane_count != 0
    }

    pub const fn dram_wait_for_edge_count(&self) -> usize {
        self.dram_wait_for_edge_count
    }

    pub const fn dram_deadlock_diagnostic_count(&self) -> usize {
        self.dram_deadlock_diagnostic_count
    }

    pub const fn has_dram_diagnostics(&self) -> bool {
        self.dram_wait_for_edge_count != 0 || self.dram_deadlock_diagnostic_count != 0
    }

    pub const fn active_dram_target_count(&self) -> usize {
        self.active_dram_target_count
    }

    pub const fn active_dram_port_count(&self) -> usize {
        self.active_dram_port_count
    }

    pub const fn active_dram_bank_count(&self) -> usize {
        self.active_dram_bank_count
    }

    pub const fn dram_access_count(&self) -> usize {
        self.dram_access_count
    }

    pub const fn dram_read_count(&self) -> usize {
        self.dram_read_count
    }

    pub const fn dram_write_count(&self) -> usize {
        self.dram_write_count
    }

    pub const fn dram_row_hit_count(&self) -> usize {
        self.dram_row_hit_count
    }

    pub const fn dram_row_miss_count(&self) -> usize {
        self.dram_row_miss_count
    }

    pub const fn dram_command_count(&self) -> usize {
        self.dram_command_count
    }

    pub const fn dram_turnaround_count(&self) -> usize {
        self.dram_turnaround_count
    }

    pub const fn dram_total_ready_latency_cycles(&self) -> u64 {
        self.dram_total_ready_latency_cycles
    }

    pub const fn dram_max_ready_latency_cycles(&self) -> u64 {
        self.dram_max_ready_latency_cycles
    }

    pub const fn has_dram_activity(&self) -> bool {
        self.dram_access_count != 0
    }

    pub const fn dram_qos_access_count(&self) -> usize {
        self.dram_qos_access_count
    }

    pub const fn dram_qos_byte_count(&self) -> u64 {
        self.dram_qos_byte_count
    }

    pub const fn dram_qos_escalated_access_count(&self) -> usize {
        self.dram_qos_escalated_access_count
    }

    pub fn dram_qos_priority_summaries(&self) -> &[WorkloadDramQosPrioritySummary] {
        &self.dram_qos_priority_summaries
    }

    pub fn dram_qos_requestor_summaries(&self) -> &[WorkloadDramQosRequestorSummary] {
        &self.dram_qos_requestor_summaries
    }

    pub fn dram_qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.dram_qos_priority_summaries
            .iter()
            .find(|summary| summary.priority() == priority)
            .map(WorkloadDramQosPrioritySummary::access_count)
            .unwrap_or(0)
    }

    pub fn dram_qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.dram_qos_priority_summaries
            .iter()
            .find(|summary| summary.priority() == priority)
            .map(WorkloadDramQosPrioritySummary::byte_count)
            .unwrap_or(0)
    }

    pub fn dram_qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.dram_qos_requestor_summaries
            .iter()
            .find(|summary| summary.requestor() == requestor)
            .map(WorkloadDramQosRequestorSummary::access_count)
            .unwrap_or(0)
    }

    pub fn dram_qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.dram_qos_requestor_summaries
            .iter()
            .find(|summary| summary.requestor() == requestor)
            .map(WorkloadDramQosRequestorSummary::byte_count)
            .unwrap_or(0)
    }

    pub const fn has_dram_qos_activity(&self) -> bool {
        self.dram_qos_access_count != 0
            || self.dram_qos_byte_count != 0
            || self.dram_qos_escalated_access_count != 0
    }

    pub const fn has_dram_row_misses(&self) -> bool {
        self.dram_row_miss_count != 0
    }

    pub const fn resource_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count + self.dram_wait_for_edge_count
    }

    pub const fn resource_deadlock_diagnostic_count(&self) -> usize {
        if self.merged_resource_deadlock_diagnostic_count == 0 {
            self.fabric_deadlock_diagnostic_count + self.dram_deadlock_diagnostic_count
        } else {
            self.merged_resource_deadlock_diagnostic_count
        }
    }

    pub const fn merged_resource_deadlock_diagnostic_count(&self) -> usize {
        self.merged_resource_deadlock_diagnostic_count
    }

    pub const fn has_resource_diagnostics(&self) -> bool {
        self.has_fabric_diagnostics()
            || self.has_dram_diagnostics()
            || self.merged_resource_deadlock_diagnostic_count != 0
    }

    pub const fn resource_activity_count(&self) -> usize {
        self.fabric_transfer_count + self.dram_access_count + self.resource_wait_for_edge_count()
    }

    pub const fn has_resource_activity(&self) -> bool {
        self.resource_activity_count() != 0
    }

    pub const fn full_system_wait_for_edge_count(&self) -> usize {
        self.resource_wait_for_edge_count()
            + self.data_cache_wait_for_edge_count
            + self.compute_wait_for_edge_count()
            + self.dma_wait_for_edge_count()
    }

    pub const fn full_system_deadlock_diagnostic_count(&self) -> usize {
        if self.merged_full_system_deadlock_diagnostic_count == 0 {
            self.resource_deadlock_diagnostic_count()
                + self.data_cache_deadlock_diagnostic_count
                + self.compute_deadlock_diagnostic_count()
                + self.dma_deadlock_diagnostic_count()
        } else {
            self.merged_full_system_deadlock_diagnostic_count
                + self.compute_deadlock_diagnostic_count()
                + self.dma_deadlock_diagnostic_count()
        }
    }

    pub const fn merged_full_system_deadlock_diagnostic_count(&self) -> usize {
        self.merged_full_system_deadlock_diagnostic_count
    }

    pub const fn full_system_progress_transition_count(&self) -> usize {
        self.scheduler_progress_transition_count
            + self.data_cache_parallel_scheduler_progress_transition_count
    }

    pub const fn full_system_livelock_diagnostic_count(&self) -> usize {
        self.scheduler_livelock_diagnostic_count
            + self.data_cache_parallel_scheduler_livelock_diagnostic_count
    }

    pub const fn has_full_system_diagnostics(&self) -> bool {
        self.has_resource_diagnostics()
            || self.has_data_cache_diagnostics()
            || self.has_compute_diagnostics()
            || self.has_dma_diagnostics()
            || self.full_system_livelock_diagnostic_count() != 0
    }

    pub const fn gpu_kernel_launch_count(&self) -> usize {
        self.gpu_kernel_launch_count
    }

    pub const fn gpu_trace_event_count(&self) -> usize {
        self.gpu_trace_event_count
    }

    pub const fn gpu_workgroup_completion_count(&self) -> usize {
        self.gpu_workgroup_completion_count
    }

    pub const fn active_gpu_device_count(&self) -> usize {
        self.active_gpu_device_count
    }

    pub const fn has_gpu_compute_activity(&self) -> bool {
        self.gpu_kernel_launch_count != 0
            || self.gpu_trace_event_count != 0
            || self.gpu_workgroup_completion_count != 0
    }

    pub const fn gpu_compute_wait_for_edge_count(&self) -> usize {
        self.gpu_compute_wait_for_edge_count
    }

    pub const fn gpu_compute_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_compute_deadlock_diagnostic_count
    }

    pub const fn has_gpu_compute_diagnostics(&self) -> bool {
        self.gpu_compute_wait_for_edge_count != 0 || self.gpu_compute_deadlock_diagnostic_count != 0
    }

    pub const fn gpu_dma_copy_count(&self) -> usize {
        self.gpu_dma_copy_count
    }

    pub const fn gpu_dma_completion_count(&self) -> usize {
        self.gpu_dma_completion_count
    }

    pub const fn active_gpu_dma_device_count(&self) -> usize {
        self.active_gpu_dma_device_count
    }

    pub const fn has_gpu_dma_activity(&self) -> bool {
        self.gpu_dma_copy_count != 0 || self.gpu_dma_completion_count != 0
    }

    pub const fn gpu_dma_wait_for_edge_count(&self) -> usize {
        self.gpu_dma_wait_for_edge_count
    }

    pub const fn gpu_dma_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_dma_deadlock_diagnostic_count
    }

    pub const fn has_gpu_dma_diagnostics(&self) -> bool {
        self.gpu_dma_wait_for_edge_count != 0 || self.gpu_dma_deadlock_diagnostic_count != 0
    }

    pub const fn accelerator_command_count(&self) -> usize {
        self.accelerator_command_count
    }

    pub const fn accelerator_trace_event_count(&self) -> usize {
        self.accelerator_trace_event_count
    }

    pub const fn accelerator_completion_count(&self) -> usize {
        self.accelerator_completion_count
    }

    pub const fn active_accelerator_device_count(&self) -> usize {
        self.active_accelerator_device_count
    }

    pub const fn has_accelerator_compute_activity(&self) -> bool {
        self.accelerator_command_count != 0
            || self.accelerator_trace_event_count != 0
            || self.accelerator_completion_count != 0
    }

    pub const fn accelerator_compute_wait_for_edge_count(&self) -> usize {
        self.accelerator_compute_wait_for_edge_count
    }

    pub const fn accelerator_compute_deadlock_diagnostic_count(&self) -> usize {
        self.accelerator_compute_deadlock_diagnostic_count
    }

    pub const fn has_accelerator_compute_diagnostics(&self) -> bool {
        self.accelerator_compute_wait_for_edge_count != 0
            || self.accelerator_compute_deadlock_diagnostic_count != 0
    }

    pub const fn compute_wait_for_edge_count(&self) -> usize {
        self.gpu_compute_wait_for_edge_count + self.accelerator_compute_wait_for_edge_count
    }

    pub const fn compute_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_compute_deadlock_diagnostic_count
            + self.accelerator_compute_deadlock_diagnostic_count
    }

    pub const fn has_compute_diagnostics(&self) -> bool {
        self.has_gpu_compute_diagnostics() || self.has_accelerator_compute_diagnostics()
    }

    pub const fn accelerator_dma_copy_count(&self) -> usize {
        self.accelerator_dma_copy_count
    }

    pub const fn accelerator_dma_completion_count(&self) -> usize {
        self.accelerator_dma_completion_count
    }

    pub const fn active_accelerator_dma_device_count(&self) -> usize {
        self.active_accelerator_dma_device_count
    }

    pub const fn has_accelerator_dma_activity(&self) -> bool {
        self.accelerator_dma_copy_count != 0 || self.accelerator_dma_completion_count != 0
    }

    pub const fn accelerator_dma_wait_for_edge_count(&self) -> usize {
        self.accelerator_dma_wait_for_edge_count
    }

    pub const fn accelerator_dma_deadlock_diagnostic_count(&self) -> usize {
        self.accelerator_dma_deadlock_diagnostic_count
    }

    pub const fn has_accelerator_dma_diagnostics(&self) -> bool {
        self.accelerator_dma_wait_for_edge_count != 0
            || self.accelerator_dma_deadlock_diagnostic_count != 0
    }

    pub const fn dma_wait_for_edge_count(&self) -> usize {
        self.gpu_dma_wait_for_edge_count + self.accelerator_dma_wait_for_edge_count
    }

    pub const fn dma_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_dma_deadlock_diagnostic_count + self.accelerator_dma_deadlock_diagnostic_count
    }

    pub const fn has_dma_diagnostics(&self) -> bool {
        self.has_gpu_dma_diagnostics() || self.has_accelerator_dma_diagnostics()
    }

    pub const fn full_system_parallel_scheduler_epoch_count(&self) -> usize {
        self.scheduler_epoch_count + self.data_cache_parallel_scheduler_epoch_count
    }

    pub const fn full_system_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.scheduler_empty_epoch_count + self.data_cache_parallel_scheduler_empty_epoch_count
    }

    pub const fn full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count + self.data_cache_parallel_scheduler_dispatch_count
    }

    pub const fn full_system_parallel_scheduler_batch_count(&self) -> usize {
        self.scheduler_batch_count + self.data_cache_parallel_scheduler_batch_count
    }

    pub const fn active_full_system_parallel_scheduler_partition_count(&self) -> usize {
        self.active_full_system_parallel_scheduler_partition_count
    }

    pub const fn full_system_parallel_scheduler_max_workers(&self) -> usize {
        if self.max_parallel_scheduler_workers > self.data_cache_parallel_scheduler_max_workers {
            self.max_parallel_scheduler_workers
        } else {
            self.data_cache_parallel_scheduler_max_workers
        }
    }

    pub const fn full_system_parallel_scheduler_total_workers(&self) -> usize {
        self.total_parallel_scheduler_workers + self.data_cache_parallel_scheduler_total_workers
    }

    pub fn full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        collect_parallel_batch_worker_counts(
            self.parallel_scheduler_batch_worker_counts
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_batch_worker_counts
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        self.parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
            + self.data_cache_parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionSet> {
        collect_parallel_batch_partition_sets(
            self.parallel_scheduler_batch_partition_sets
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_batch_partition_sets
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks(
            self.parallel_scheduler_batch_partition_streaks
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_batch_partition_streaks
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_batch_count_for_partition_set(partitions.iter().copied())
            + self.data_cache_parallel_scheduler_batch_count_for_partition_set(
                partitions.iter().copied(),
            )
    }

    pub fn full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_max_consecutive_batch_count_for_partition_set(
            partitions.iter().copied(),
        )
        .max(
            self.data_cache_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
                partitions.iter().copied(),
            ),
        )
    }

    pub fn full_system_parallel_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flows(
            self.parallel_scheduler_remote_flows.iter().copied().chain(
                self.data_cache_parallel_scheduler_remote_flows
                    .iter()
                    .copied(),
            ),
        )
    }

    pub fn full_system_parallel_scheduler_partition_activities(
        &self,
    ) -> Vec<(PartitionId, ParallelPartitionActivity)> {
        collect_parallel_partition_activities(
            self.parallel_scheduler_partition_activities
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_partition_activities
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.full_system_parallel_scheduler_partition_activities()
            .into_iter()
            .find(|(existing, _)| *existing == partition)
            .map(|(_, activity)| activity)
    }

    pub fn full_system_parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        self.parallel_scheduler_remote_flow_count(source, target)
            + self.data_cache_parallel_scheduler_remote_flow_count(source, target)
    }

    pub fn has_full_system_parallel_scheduler_remote_flows(&self) -> bool {
        self.has_parallel_scheduler_remote_flows()
            || self.has_data_cache_parallel_scheduler_remote_flows()
    }

    pub const fn has_full_system_parallel_scheduler_work(&self) -> bool {
        self.has_parallel_scheduler_work() || self.has_data_cache_parallel_work()
    }

    pub const fn has_parallel_scheduler_work(&self) -> bool {
        self.scheduler_dispatch_count != 0
            || self.scheduler_batch_count != 0
            || self.total_parallel_scheduler_workers != 0
            || self.max_parallel_scheduler_workers != 0
            || !self.parallel_scheduler_batch_worker_counts.is_empty()
            || !self.parallel_scheduler_batch_partition_sets.is_empty()
            || !self.parallel_scheduler_batch_partition_streaks.is_empty()
    }

    pub const fn has_data_cache_parallel_work(&self) -> bool {
        self.data_cache_parallel_run_count != 0
            || self.data_cache_parallel_scheduler_dispatch_count != 0
            || self.data_cache_parallel_scheduler_total_workers != 0
            || self.data_cache_parallel_scheduler_max_workers != 0
            || !self
                .data_cache_parallel_scheduler_batch_worker_counts
                .is_empty()
            || !self
                .data_cache_parallel_scheduler_batch_partition_sets
                .is_empty()
            || !self
                .data_cache_parallel_scheduler_batch_partition_streaks
                .is_empty()
    }
}

fn collect_priority_summaries(
    summaries: impl IntoIterator<Item = WorkloadDramQosPrioritySummary>,
) -> Vec<WorkloadDramQosPrioritySummary> {
    let mut by_priority = BTreeMap::<QosPriority, (usize, u64)>::new();
    for summary in summaries {
        if summary.is_empty() {
            continue;
        }
        let entry = by_priority.entry(summary.priority()).or_default();
        entry.0 += summary.access_count();
        entry.1 += summary.byte_count();
    }
    by_priority
        .into_iter()
        .map(|(priority, (access_count, byte_count))| {
            WorkloadDramQosPrioritySummary::new(priority, access_count, byte_count)
        })
        .collect()
}

fn collect_parallel_remote_flows(
    flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
) -> Vec<ParallelRemoteFlowRecord> {
    let mut by_route = BTreeMap::<(PartitionId, PartitionId), ParallelRemoteFlowRecord>::new();
    for flow in flows {
        if flow.send_count() == 0 {
            continue;
        }
        by_route
            .entry((flow.source(), flow.target()))
            .and_modify(|stored| {
                *stored = ParallelRemoteFlowRecord::new(
                    stored.source(),
                    stored.target(),
                    stored.send_count() + flow.send_count(),
                    stored.first_tick().min(flow.first_tick()),
                    stored.last_tick().max(flow.last_tick()),
                );
            })
            .or_insert(flow);
    }
    by_route.into_values().collect()
}

fn parallel_remote_flow_count(
    flows: &[ParallelRemoteFlowRecord],
    source: PartitionId,
    target: PartitionId,
) -> usize {
    flows
        .iter()
        .find(|flow| flow.source() == source && flow.target() == target)
        .map(|flow| flow.send_count())
        .unwrap_or(0)
}

fn collect_parallel_partition_activities(
    activities: impl IntoIterator<Item = (PartitionId, ParallelPartitionActivity)>,
) -> Vec<(PartitionId, ParallelPartitionActivity)> {
    let mut by_partition = BTreeMap::new();
    for (partition, activity) in activities {
        if !activity.has_activity() {
            continue;
        }
        by_partition
            .entry(partition)
            .and_modify(|stored: &mut ParallelPartitionActivity| {
                *stored = merge_parallel_partition_activity(*stored, activity);
            })
            .or_insert(activity);
    }
    by_partition.into_iter().collect()
}

fn merge_parallel_partition_activity(
    left: ParallelPartitionActivity,
    right: ParallelPartitionActivity,
) -> ParallelPartitionActivity {
    ParallelPartitionActivity::with_remote_counts(
        left.worker_count() + right.worker_count(),
        left.dispatch_count() + right.dispatch_count(),
        left.remote_send_count() + right.remote_send_count(),
        left.remote_receive_count() + right.remote_receive_count(),
        left.max_pending_events().max(right.max_pending_events()),
    )
}

fn collect_requestor_summaries(
    summaries: impl IntoIterator<Item = WorkloadDramQosRequestorSummary>,
) -> Vec<WorkloadDramQosRequestorSummary> {
    let mut by_requestor = BTreeMap::<QosRequestorId, (usize, u64)>::new();
    for summary in summaries {
        if summary.is_empty() {
            continue;
        }
        let entry = by_requestor.entry(summary.requestor()).or_default();
        entry.0 += summary.access_count();
        entry.1 += summary.byte_count();
    }
    by_requestor
        .into_iter()
        .map(|(requestor, (access_count, byte_count))| {
            WorkloadDramQosRequestorSummary::new(requestor, access_count, byte_count)
        })
        .collect()
}
