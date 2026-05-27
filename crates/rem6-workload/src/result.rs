use std::collections::BTreeMap;

use rem6_fabric::{
    FabricHopActivity, FabricLaneActivity, FabricLinkActivity, FabricVirtualNetworkActivity,
    QosPriority, QosRequestorId,
};
use rem6_kernel::{
    LivelockDiagnostic, ParallelPartitionActivity, ParallelProgressTransitionRecord,
    ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionFrontier, PartitionId, Tick,
    WaitForEdgeKind,
};

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets, collect_parallel_batch_partition_streaks,
    collect_parallel_batch_partition_streaks_from_sequence, collect_parallel_batch_worker_counts,
    max_parallel_batch_activity_worker_count, parallel_batch_active_partition_count,
    parallel_batch_activity_count_at_or_above, parallel_batch_count_for_partition_set,
    parallel_batch_partition_activity_for_partition, parallel_batch_streak_activity_for_partition,
    parallel_batch_streak_count_for_partition_set, strongest_parallel_batch_count,
    total_parallel_batch_activity_worker_count, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_parallel_partition_activities, collect_parallel_remote_flow_evidence,
    collect_parallel_remote_flows, collect_parallel_remote_sends, collect_partition_frontiers,
    collect_priority_summaries, collect_requestor_summaries, is_parallel_remote_send_evidence,
    parallel_remote_flow_evidence_count, parallel_remote_send_count,
};
use crate::result_partition_activity::{
    merge_parallel_partition_activity_evidence_options, parallel_active_partition_count,
    parallel_partition_activity_for_partition, parallel_partition_dispatch_count,
    parallel_partition_worker_count,
};
mod batch_timeline;
mod batch_worker_count;
mod data_cache;
mod fabric_activity;
mod full_system_parallel;
mod heterogeneous_activity;
mod progress;
mod remote_endpoints;
mod wait_for_diagnostics;

pub use wait_for_diagnostics::{WorkloadWaitForBlockedNodeWindow, WorkloadWaitForTargetNodeWindow};

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadWaitForEdgeKindWindow {
    kind: WaitForEdgeKind,
    edge_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl WorkloadWaitForEdgeKindWindow {
    pub const fn new(
        kind: WaitForEdgeKind,
        edge_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Self {
        let stored_first_tick = if first_tick <= last_tick {
            first_tick
        } else {
            last_tick
        };
        let stored_last_tick = if first_tick <= last_tick {
            last_tick
        } else {
            first_tick
        };
        Self {
            kind,
            edge_count,
            first_tick: stored_first_tick,
            last_tick: stored_last_tick,
        }
    }

    pub const fn kind(&self) -> WaitForEdgeKind {
        self.kind
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub const fn is_empty(&self) -> bool {
        self.edge_count == 0
    }

    pub(crate) fn merge(&mut self, other: Self) {
        debug_assert_eq!(self.kind, other.kind);
        self.edge_count = self.edge_count.saturating_add(other.edge_count);
        self.first_tick = self.first_tick.min(other.first_tick);
        self.last_tick = self.last_tick.max(other.last_tick);
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
    scheduler_livelock_diagnostics: Vec<LivelockDiagnostic>,
    parallel_scheduler_progress_transitions: Vec<ParallelProgressTransitionRecord>,
    parallel_scheduler_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    parallel_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    parallel_scheduler_batch_partition_sets: Vec<WorkloadParallelBatchPartitionSet>,
    parallel_scheduler_batch_partition_streaks: Vec<WorkloadParallelBatchPartitionStreak>,
    parallel_scheduler_partition_activities: Vec<(PartitionId, ParallelPartitionActivity)>,
    parallel_scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    parallel_scheduler_remote_sends: Vec<ParallelRemoteSendRecord>,
    parallel_scheduler_initial_frontiers: Vec<PartitionFrontier>,
    parallel_scheduler_final_frontiers: Vec<PartitionFrontier>,
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
    data_cache_parallel_scheduler_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    data_cache_parallel_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    data_cache_parallel_scheduler_batch_partition_sets: Vec<WorkloadParallelBatchPartitionSet>,
    data_cache_parallel_scheduler_batch_partition_streaks:
        Vec<WorkloadParallelBatchPartitionStreak>,
    active_full_system_parallel_scheduler_partition_count: usize,
    full_system_parallel_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    full_system_parallel_scheduler_batch_worker_count_ticks: Vec<(usize, Tick)>,
    full_system_parallel_scheduler_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    full_system_parallel_scheduler_batch_partition_sets: Vec<WorkloadParallelBatchPartitionSet>,
    full_system_parallel_scheduler_batch_partition_streaks:
        Vec<WorkloadParallelBatchPartitionStreak>,
    data_cache_parallel_scheduler_partition_activities:
        Vec<(PartitionId, ParallelPartitionActivity)>,
    data_cache_parallel_scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    data_cache_parallel_scheduler_remote_sends: Vec<ParallelRemoteSendRecord>,
    data_cache_parallel_scheduler_initial_frontiers: Vec<PartitionFrontier>,
    data_cache_parallel_scheduler_final_frontiers: Vec<PartitionFrontier>,
    attributed_data_cache_parallel_run_count: usize,
    unattributed_data_cache_parallel_run_count: usize,
    data_cache_protocol_counts: Vec<WorkloadDataCacheProtocolCount>,
    data_cache_wait_for_edge_count: usize,
    data_cache_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    data_cache_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    data_cache_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    data_cache_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
    data_cache_deadlock_diagnostic_count: usize,
    data_cache_parallel_scheduler_progress_transition_count: usize,
    data_cache_parallel_scheduler_livelock_diagnostic_count: usize,
    data_cache_parallel_scheduler_livelock_diagnostics: Vec<LivelockDiagnostic>,
    data_cache_parallel_scheduler_progress_transitions: Vec<ParallelProgressTransitionRecord>,
    active_fabric_lane_count: usize,
    fabric_transfer_count: usize,
    fabric_byte_count: u64,
    fabric_occupied_ticks: u64,
    fabric_queue_delay_ticks: u64,
    fabric_max_queue_delay_ticks: u64,
    contended_fabric_lane_count: usize,
    fabric_hop_activities: Vec<FabricHopActivity>,
    fabric_lane_activities: Vec<FabricLaneActivity>,
    fabric_link_activities: Vec<FabricLinkActivity>,
    fabric_virtual_network_activities: Vec<FabricVirtualNetworkActivity>,
    fabric_wait_for_edge_count: usize,
    fabric_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    fabric_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    fabric_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    fabric_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
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
    dram_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    dram_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    dram_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    dram_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
    dram_deadlock_diagnostic_count: usize,
    merged_resource_deadlock_diagnostic_count: usize,
    merged_full_system_deadlock_diagnostic_count: usize,
    merged_full_system_livelock_diagnostic_count: usize,
    has_merged_full_system_livelock_diagnostic_count: bool,
    merged_full_system_livelock_diagnostics: Vec<LivelockDiagnostic>,
    gpu_kernel_launch_count: usize,
    gpu_trace_event_count: usize,
    gpu_workgroup_completion_count: usize,
    active_gpu_device_count: usize,
    gpu_compute_wait_for_edge_count: usize,
    gpu_compute_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    gpu_compute_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    gpu_compute_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    gpu_compute_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
    gpu_compute_deadlock_diagnostic_count: usize,
    gpu_dma_copy_count: usize,
    gpu_dma_completion_count: usize,
    active_gpu_dma_device_count: usize,
    gpu_dma_scheduler_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    gpu_dma_scheduler_epoch_count: usize,
    gpu_dma_scheduler_empty_epoch_count: usize,
    gpu_dma_scheduler_dispatch_count: usize,
    gpu_dma_scheduler_batch_count: usize,
    gpu_dma_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    gpu_dma_scheduler_batch_worker_count_ticks: Vec<(usize, Tick)>,
    gpu_dma_scheduler_initial_frontiers: Vec<PartitionFrontier>,
    gpu_dma_scheduler_final_frontiers: Vec<PartitionFrontier>,
    gpu_dma_scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    gpu_dma_scheduler_remote_sends: Vec<ParallelRemoteSendRecord>,
    gpu_dma_wait_for_edge_count: usize,
    gpu_dma_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    gpu_dma_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    gpu_dma_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    gpu_dma_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
    gpu_dma_deadlock_diagnostic_count: usize,
    accelerator_command_count: usize,
    accelerator_gpu_kernel_command_count: usize,
    accelerator_npu_inference_command_count: usize,
    accelerator_dma_command_count: usize,
    accelerator_trace_event_count: usize,
    accelerator_completion_count: usize,
    accelerator_gpu_kernel_completion_count: usize,
    accelerator_npu_inference_completion_count: usize,
    accelerator_dma_command_completion_count: usize,
    active_accelerator_device_count: usize,
    accelerator_compute_wait_for_edge_count: usize,
    accelerator_compute_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    accelerator_compute_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    accelerator_compute_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    accelerator_compute_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
    accelerator_compute_deadlock_diagnostic_count: usize,
    accelerator_dma_copy_count: usize,
    accelerator_dma_completion_count: usize,
    active_accelerator_dma_device_count: usize,
    accelerator_dma_scheduler_batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    accelerator_dma_scheduler_epoch_count: usize,
    accelerator_dma_scheduler_empty_epoch_count: usize,
    accelerator_dma_scheduler_dispatch_count: usize,
    accelerator_dma_scheduler_batch_count: usize,
    accelerator_dma_scheduler_batch_worker_counts: Vec<WorkloadParallelBatchWorkerCount>,
    accelerator_dma_scheduler_batch_worker_count_ticks: Vec<(usize, Tick)>,
    accelerator_dma_scheduler_initial_frontiers: Vec<PartitionFrontier>,
    accelerator_dma_scheduler_final_frontiers: Vec<PartitionFrontier>,
    accelerator_dma_scheduler_remote_flows: Vec<ParallelRemoteFlowRecord>,
    accelerator_dma_scheduler_remote_sends: Vec<ParallelRemoteSendRecord>,
    accelerator_dma_wait_for_edge_count: usize,
    accelerator_dma_wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    accelerator_dma_wait_for_edge_kind_windows: Vec<WorkloadWaitForEdgeKindWindow>,
    accelerator_dma_wait_for_blocked_node_windows: Vec<WorkloadWaitForBlockedNodeWindow>,
    accelerator_dma_wait_for_target_node_windows: Vec<WorkloadWaitForTargetNodeWindow>,
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

    pub fn with_parallel_scheduler_remote_sends(
        mut self,
        sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
    ) -> Self {
        self.parallel_scheduler_remote_sends = collect_parallel_remote_sends(sends);
        self
    }

    pub fn with_parallel_scheduler_frontiers(
        mut self,
        initial_frontiers: impl IntoIterator<Item = PartitionFrontier>,
        final_frontiers: impl IntoIterator<Item = PartitionFrontier>,
    ) -> Self {
        self.parallel_scheduler_initial_frontiers = collect_partition_frontiers(initial_frontiers);
        self.parallel_scheduler_final_frontiers = collect_partition_frontiers(final_frontiers);
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

    pub fn with_parallel_scheduler_livelock_diagnostics(
        mut self,
        progress_transition_count: usize,
        livelock_diagnostic_count: usize,
    ) -> Self {
        self.scheduler_progress_transition_count =
            progress_transition_count.max(self.parallel_scheduler_progress_transitions.len());
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

    pub const fn with_full_system_parallel_partitions(
        mut self,
        active_partition_count: usize,
    ) -> Self {
        self.active_full_system_parallel_scheduler_partition_count = active_partition_count;
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_worker_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
    ) -> Self {
        self.full_system_parallel_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts(counts);
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_partition_sets(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.full_system_parallel_scheduler_batch_partition_sets =
            collect_parallel_batch_partition_sets(sets);
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_partition_streaks(
        mut self,
        streaks: impl IntoIterator<Item = WorkloadParallelBatchPartitionStreak>,
    ) -> Self {
        self.full_system_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks(streaks);
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_partition_streak_sequence(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.full_system_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks_from_sequence(sets);
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

    pub const fn with_accelerator_command_kind_counts(
        mut self,
        gpu_kernel_count: usize,
        npu_inference_count: usize,
        dma_count: usize,
    ) -> Self {
        self.accelerator_gpu_kernel_command_count = gpu_kernel_count;
        self.accelerator_npu_inference_command_count = npu_inference_count;
        self.accelerator_dma_command_count = dma_count;
        self
    }

    pub const fn with_accelerator_completion_kind_counts(
        mut self,
        gpu_kernel_count: usize,
        npu_inference_count: usize,
        dma_count: usize,
    ) -> Self {
        self.accelerator_gpu_kernel_completion_count = gpu_kernel_count;
        self.accelerator_npu_inference_completion_count = npu_inference_count;
        self.accelerator_dma_command_completion_count = dma_count;
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

    pub fn scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count.max(
            total_parallel_batch_activity_worker_count(
                &self.parallel_scheduler_batch_worker_counts,
                &self.parallel_scheduler_batch_partition_sets,
                &self.parallel_scheduler_batch_partition_streaks,
            )
            .max(parallel_partition_dispatch_count(
                &self.parallel_scheduler_partition_activities,
            )),
        )
    }

    pub fn scheduler_batch_count(&self) -> usize {
        self.scheduler_batch_count
            .max(strongest_parallel_batch_count(
                &self.parallel_scheduler_batch_worker_counts,
                &self.parallel_scheduler_batch_partition_sets,
                &self.parallel_scheduler_batch_partition_streaks,
            ))
    }

    pub fn active_scheduler_partition_count(&self) -> usize {
        self.active_scheduler_partition_count
            .max(parallel_batch_active_partition_count(
                &self.parallel_scheduler_batch_partition_sets,
                &self.parallel_scheduler_batch_partition_streaks,
            ))
            .max(parallel_active_partition_count(
                &self.parallel_scheduler_partition_activities,
                &self.parallel_scheduler_remote_flows,
                &self.parallel_scheduler_remote_sends,
            ))
    }

    pub fn max_parallel_scheduler_workers(&self) -> usize {
        self.max_parallel_scheduler_workers
            .max(max_parallel_batch_activity_worker_count(
                &self.parallel_scheduler_batch_worker_counts,
                &self.parallel_scheduler_batch_partition_sets,
                &self.parallel_scheduler_batch_partition_streaks,
            ))
    }

    pub fn total_parallel_scheduler_workers(&self) -> usize {
        self.total_parallel_scheduler_workers.max(
            total_parallel_batch_activity_worker_count(
                &self.parallel_scheduler_batch_worker_counts,
                &self.parallel_scheduler_batch_partition_sets,
                &self.parallel_scheduler_batch_partition_streaks,
            )
            .max(parallel_partition_worker_count(
                &self.parallel_scheduler_partition_activities,
            )),
        )
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

    pub fn parallel_scheduler_remote_flow_evidence(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_evidence(
            self.parallel_scheduler_remote_flows.iter().copied(),
            self.parallel_scheduler_remote_sends.iter().copied(),
        )
    }

    pub fn parallel_scheduler_remote_sends(&self) -> &[ParallelRemoteSendRecord] {
        &self.parallel_scheduler_remote_sends
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
        parallel_batch_activity_count_at_or_above(
            &self.parallel_scheduler_batch_worker_counts,
            &self.parallel_scheduler_batch_partition_sets,
            &self.parallel_scheduler_batch_partition_streaks,
            minimum_worker_count,
        )
    }

    pub fn parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_count_for_partition_set(
            &self.parallel_scheduler_batch_partition_sets,
            &self.parallel_scheduler_batch_partition_streaks,
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
        merge_parallel_partition_activity_evidence_options(
            merge_parallel_partition_activity_evidence_options(
                parallel_partition_activity_for_partition(
                    &self.parallel_scheduler_partition_activities,
                    &self.parallel_scheduler_remote_flows,
                    &self.parallel_scheduler_remote_sends,
                    partition,
                ),
                parallel_batch_partition_activity_for_partition(
                    &self.parallel_scheduler_batch_partition_sets,
                    partition,
                ),
            ),
            parallel_batch_streak_activity_for_partition(
                &self.parallel_scheduler_batch_partition_streaks,
                partition,
            ),
        )
    }

    pub fn parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_evidence_count(
            &self.parallel_scheduler_remote_flows,
            &self.parallel_scheduler_remote_sends,
            source,
            target,
        )
    }

    pub fn has_parallel_scheduler_remote_flows(&self) -> bool {
        !self.parallel_scheduler_remote_flow_evidence().is_empty()
    }

    pub fn parallel_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_send_count(&self.parallel_scheduler_remote_sends, source, target)
    }

    pub fn has_parallel_scheduler_remote_sends(&self) -> bool {
        self.parallel_scheduler_remote_sends
            .iter()
            .copied()
            .any(is_parallel_remote_send_evidence)
    }

    pub fn parallel_scheduler_initial_frontiers(&self) -> &[PartitionFrontier] {
        &self.parallel_scheduler_initial_frontiers
    }

    pub fn parallel_scheduler_final_frontiers(&self) -> &[PartitionFrontier] {
        &self.parallel_scheduler_final_frontiers
    }

    pub fn parallel_scheduler_initial_frontier_count(&self) -> usize {
        self.parallel_scheduler_initial_frontiers.len()
    }

    pub fn parallel_scheduler_final_frontier_count(&self) -> usize {
        self.parallel_scheduler_final_frontiers.len()
    }

    pub fn has_parallel_scheduler_frontiers(&self) -> bool {
        !self.parallel_scheduler_initial_frontiers.is_empty()
            || !self.parallel_scheduler_final_frontiers.is_empty()
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
        if self.has_merged_full_system_livelock_diagnostic_count {
            self.merged_full_system_livelock_diagnostic_count
        } else {
            self.scheduler_livelock_diagnostic_count
                + self.data_cache_parallel_scheduler_livelock_diagnostic_count
        }
    }

    pub fn has_full_system_diagnostics(&self) -> bool {
        self.has_resource_diagnostics()
            || self.has_data_cache_diagnostics()
            || self.has_compute_diagnostics()
            || self.has_dma_diagnostics()
            || self.merged_full_system_deadlock_diagnostic_count != 0
            || self.full_system_livelock_diagnostic_count() != 0
    }
}
