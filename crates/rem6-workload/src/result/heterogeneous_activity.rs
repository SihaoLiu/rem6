use std::collections::BTreeMap;

use rem6_kernel::{
    ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionFrontier, PartitionId, Tick,
    WaitForEdgeKind,
};

use crate::parallel_batch::{
    collect_parallel_batch_worker_counts, max_parallel_batch_worker_count,
    total_parallel_batch_worker_count, WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_conservative_partition_frontiers, collect_parallel_remote_flow_aggregates,
    collect_parallel_remote_flow_evidence, collect_parallel_remote_flows,
    collect_parallel_remote_sends, collect_partition_frontiers, is_parallel_remote_send_evidence,
    parallel_remote_flow_evidence_count, parallel_remote_send_count,
};

use super::{
    wait_for_diagnostics::{
        merge_wait_for_edge_kind_counts, merge_wait_for_edge_kind_windows,
        wait_for_blocked_node_window_count_sum, wait_for_edge_kind_count,
        wait_for_edge_kind_count_sum, wait_for_edge_kind_window,
        wait_for_edge_kind_window_count_sum, wait_for_target_node_window_count_sum,
    },
    WorkloadParallelExecutionSummary, WorkloadWaitForEdgeKindWindow,
};

impl WorkloadParallelExecutionSummary {
    pub fn with_gpu_dma_scheduler_counts(
        mut self,
        epoch_count: usize,
        dispatch_count: usize,
        batch_count: usize,
        batch_worker_count_ticks: impl IntoIterator<Item = (usize, Tick)>,
    ) -> Self {
        self.gpu_dma_scheduler_epoch_count = epoch_count;
        self.gpu_dma_scheduler_dispatch_count = dispatch_count;
        self.gpu_dma_scheduler_batch_count = batch_count;
        self.gpu_dma_scheduler_batch_worker_count_ticks =
            collect_batch_worker_count_ticks(batch_worker_count_ticks);
        self
    }

    pub const fn with_gpu_dma_scheduler_empty_epoch_count(
        mut self,
        empty_epoch_count: usize,
    ) -> Self {
        self.gpu_dma_scheduler_empty_epoch_count = empty_epoch_count;
        self
    }

    pub fn with_gpu_dma_scheduler_batch_worker_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
    ) -> Self {
        self.gpu_dma_scheduler_batch_worker_counts = collect_parallel_batch_worker_counts(counts);
        self.gpu_dma_scheduler_batch_count =
            total_parallel_batch_count(&self.gpu_dma_scheduler_batch_worker_counts);
        self
    }

    pub fn with_gpu_dma_scheduler_frontiers(
        mut self,
        initial_frontiers: impl IntoIterator<Item = PartitionFrontier>,
        final_frontiers: impl IntoIterator<Item = PartitionFrontier>,
    ) -> Self {
        self.gpu_dma_scheduler_initial_frontiers = collect_partition_frontiers(initial_frontiers);
        self.gpu_dma_scheduler_final_frontiers = collect_partition_frontiers(final_frontiers);
        self
    }

    pub fn with_gpu_dma_scheduler_remote_flows(
        mut self,
        flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    ) -> Self {
        self.gpu_dma_scheduler_remote_flows = collect_parallel_remote_flows(flows);
        self
    }

    pub fn with_gpu_dma_scheduler_remote_sends(
        mut self,
        sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
    ) -> Self {
        self.gpu_dma_scheduler_remote_sends = collect_parallel_remote_sends(sends);
        self
    }

    pub fn with_accelerator_dma_scheduler_counts(
        mut self,
        epoch_count: usize,
        dispatch_count: usize,
        batch_count: usize,
        batch_worker_count_ticks: impl IntoIterator<Item = (usize, Tick)>,
    ) -> Self {
        self.accelerator_dma_scheduler_epoch_count = epoch_count;
        self.accelerator_dma_scheduler_dispatch_count = dispatch_count;
        self.accelerator_dma_scheduler_batch_count = batch_count;
        self.accelerator_dma_scheduler_batch_worker_count_ticks =
            collect_batch_worker_count_ticks(batch_worker_count_ticks);
        self
    }

    pub const fn with_accelerator_dma_scheduler_empty_epoch_count(
        mut self,
        empty_epoch_count: usize,
    ) -> Self {
        self.accelerator_dma_scheduler_empty_epoch_count = empty_epoch_count;
        self
    }

    pub fn with_accelerator_dma_scheduler_batch_worker_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
    ) -> Self {
        self.accelerator_dma_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts(counts);
        self.accelerator_dma_scheduler_batch_count =
            total_parallel_batch_count(&self.accelerator_dma_scheduler_batch_worker_counts);
        self
    }

    pub fn with_accelerator_dma_scheduler_frontiers(
        mut self,
        initial_frontiers: impl IntoIterator<Item = PartitionFrontier>,
        final_frontiers: impl IntoIterator<Item = PartitionFrontier>,
    ) -> Self {
        self.accelerator_dma_scheduler_initial_frontiers =
            collect_partition_frontiers(initial_frontiers);
        self.accelerator_dma_scheduler_final_frontiers =
            collect_partition_frontiers(final_frontiers);
        self
    }

    pub fn with_accelerator_dma_scheduler_remote_flows(
        mut self,
        flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    ) -> Self {
        self.accelerator_dma_scheduler_remote_flows = collect_parallel_remote_flows(flows);
        self
    }

    pub fn with_accelerator_dma_scheduler_remote_sends(
        mut self,
        sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
    ) -> Self {
        self.accelerator_dma_scheduler_remote_sends = collect_parallel_remote_sends(sends);
        self
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

    pub fn gpu_compute_wait_for_edge_count(&self) -> usize {
        self.gpu_compute_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.gpu_compute_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.gpu_compute_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.gpu_compute_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.gpu_compute_wait_for_target_node_windows,
            ))
    }

    pub fn gpu_compute_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.gpu_compute_wait_for_edge_kind_counts
    }

    pub fn gpu_compute_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.gpu_compute_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.gpu_compute_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn gpu_compute_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.gpu_compute_wait_for_edge_kind_windows
    }

    pub fn gpu_compute_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.gpu_compute_wait_for_edge_kind_windows, kind)
    }

    pub const fn gpu_compute_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_compute_deadlock_diagnostic_count
    }

    pub fn has_gpu_compute_diagnostics(&self) -> bool {
        self.gpu_compute_wait_for_edge_count() != 0
            || self.gpu_compute_deadlock_diagnostic_count != 0
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

    pub const fn gpu_dma_scheduler_epoch_count(&self) -> usize {
        self.gpu_dma_scheduler_epoch_count
    }

    pub const fn gpu_dma_scheduler_empty_epoch_count(&self) -> usize {
        self.gpu_dma_scheduler_empty_epoch_count
    }

    pub const fn gpu_dma_scheduler_dispatch_count(&self) -> usize {
        self.gpu_dma_scheduler_dispatch_count
    }

    pub const fn gpu_dma_scheduler_batch_count(&self) -> usize {
        self.gpu_dma_scheduler_batch_count
    }

    pub fn gpu_dma_scheduler_batch_worker_counts(&self) -> &[WorkloadParallelBatchWorkerCount] {
        &self.gpu_dma_scheduler_batch_worker_counts
    }

    pub fn gpu_dma_scheduler_batch_worker_count_tick_summaries(&self) -> &[(usize, Tick)] {
        &self.gpu_dma_scheduler_batch_worker_count_ticks
    }

    pub fn gpu_dma_scheduler_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        batch_count_for_worker_count(&self.gpu_dma_scheduler_batch_worker_counts, worker_count)
    }

    pub fn gpu_dma_scheduler_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        batch_count_at_or_above(
            &self.gpu_dma_scheduler_batch_worker_counts,
            minimum_worker_count,
        )
    }

    pub fn gpu_dma_scheduler_max_workers(&self) -> usize {
        max_parallel_batch_worker_count(&self.gpu_dma_scheduler_batch_worker_counts).max(
            max_batch_worker_count_ticks(&self.gpu_dma_scheduler_batch_worker_count_ticks),
        )
    }

    pub fn gpu_dma_scheduler_total_workers(&self) -> usize {
        total_parallel_batch_worker_count(&self.gpu_dma_scheduler_batch_worker_counts)
    }

    pub fn gpu_dma_scheduler_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        batch_ticks_for_worker_count(
            &self.gpu_dma_scheduler_batch_worker_count_ticks,
            worker_count,
        )
    }

    pub fn gpu_dma_scheduler_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        batch_ticks_at_or_above(
            &self.gpu_dma_scheduler_batch_worker_count_ticks,
            minimum_worker_count,
        )
    }

    pub fn gpu_dma_scheduler_batch_worker_ticks(&self) -> Tick {
        batch_worker_ticks(&self.gpu_dma_scheduler_batch_worker_count_ticks)
    }

    pub fn gpu_dma_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        batch_worker_ticks_at_or_above(
            &self.gpu_dma_scheduler_batch_worker_count_ticks,
            minimum_worker_count,
        )
    }

    pub fn gpu_dma_scheduler_initial_frontiers(&self) -> &[PartitionFrontier] {
        &self.gpu_dma_scheduler_initial_frontiers
    }

    pub fn gpu_dma_scheduler_final_frontiers(&self) -> &[PartitionFrontier] {
        &self.gpu_dma_scheduler_final_frontiers
    }

    pub fn gpu_dma_scheduler_initial_frontier_count(&self) -> usize {
        self.gpu_dma_scheduler_initial_frontiers.len()
    }

    pub fn gpu_dma_scheduler_final_frontier_count(&self) -> usize {
        self.gpu_dma_scheduler_final_frontiers.len()
    }

    pub fn has_gpu_dma_scheduler_frontiers(&self) -> bool {
        !self.gpu_dma_scheduler_initial_frontiers.is_empty()
            || !self.gpu_dma_scheduler_final_frontiers.is_empty()
    }

    pub fn gpu_dma_scheduler_remote_flows(&self) -> &[ParallelRemoteFlowRecord] {
        &self.gpu_dma_scheduler_remote_flows
    }

    pub fn gpu_dma_scheduler_remote_flow_evidence(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_evidence(
            self.gpu_dma_scheduler_remote_flows.iter().copied(),
            self.gpu_dma_scheduler_remote_sends.iter().copied(),
        )
    }

    pub fn gpu_dma_scheduler_remote_sends(&self) -> &[ParallelRemoteSendRecord] {
        &self.gpu_dma_scheduler_remote_sends
    }

    pub fn gpu_dma_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_evidence_count(
            &self.gpu_dma_scheduler_remote_flows,
            &self.gpu_dma_scheduler_remote_sends,
            source,
            target,
        )
    }

    pub fn gpu_dma_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_send_count(&self.gpu_dma_scheduler_remote_sends, source, target)
    }

    pub fn has_gpu_dma_scheduler_remote_flows(&self) -> bool {
        !self.gpu_dma_scheduler_remote_flow_evidence().is_empty()
    }

    pub fn has_gpu_dma_scheduler_remote_sends(&self) -> bool {
        self.gpu_dma_scheduler_remote_sends
            .iter()
            .copied()
            .any(is_parallel_remote_send_evidence)
    }

    pub fn has_gpu_dma_activity(&self) -> bool {
        self.gpu_dma_copy_count != 0
            || self.gpu_dma_completion_count != 0
            || self.gpu_dma_scheduler_epoch_count != 0
            || self.gpu_dma_scheduler_empty_epoch_count != 0
            || self.gpu_dma_scheduler_dispatch_count != 0
            || self.gpu_dma_scheduler_batch_count != 0
            || !self.gpu_dma_scheduler_batch_worker_counts.is_empty()
            || !self.gpu_dma_scheduler_batch_worker_count_ticks.is_empty()
            || self.has_gpu_dma_scheduler_frontiers()
            || self.has_gpu_dma_scheduler_remote_flows()
            || self.has_gpu_dma_scheduler_remote_sends()
    }

    pub fn gpu_dma_wait_for_edge_count(&self) -> usize {
        self.gpu_dma_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.gpu_dma_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.gpu_dma_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.gpu_dma_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.gpu_dma_wait_for_target_node_windows,
            ))
    }

    pub fn gpu_dma_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.gpu_dma_wait_for_edge_kind_counts
    }

    pub fn gpu_dma_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.gpu_dma_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.gpu_dma_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn gpu_dma_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.gpu_dma_wait_for_edge_kind_windows
    }

    pub fn gpu_dma_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.gpu_dma_wait_for_edge_kind_windows, kind)
    }

    pub const fn gpu_dma_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_dma_deadlock_diagnostic_count
    }

    pub fn has_gpu_dma_diagnostics(&self) -> bool {
        self.gpu_dma_wait_for_edge_count() != 0 || self.gpu_dma_deadlock_diagnostic_count != 0
    }

    pub const fn accelerator_command_count(&self) -> usize {
        self.accelerator_command_count
    }

    pub const fn accelerator_gpu_kernel_command_count(&self) -> usize {
        self.accelerator_gpu_kernel_command_count
    }

    pub const fn accelerator_npu_inference_command_count(&self) -> usize {
        self.accelerator_npu_inference_command_count
    }

    pub const fn accelerator_dma_command_count(&self) -> usize {
        self.accelerator_dma_command_count
    }

    pub const fn accelerator_trace_event_count(&self) -> usize {
        self.accelerator_trace_event_count
    }

    pub const fn accelerator_completion_count(&self) -> usize {
        self.accelerator_completion_count
    }

    pub const fn accelerator_gpu_kernel_completion_count(&self) -> usize {
        self.accelerator_gpu_kernel_completion_count
    }

    pub const fn accelerator_npu_inference_completion_count(&self) -> usize {
        self.accelerator_npu_inference_completion_count
    }

    pub const fn accelerator_dma_command_completion_count(&self) -> usize {
        self.accelerator_dma_command_completion_count
    }

    pub const fn active_accelerator_device_count(&self) -> usize {
        self.active_accelerator_device_count
    }

    pub const fn has_accelerator_compute_activity(&self) -> bool {
        self.accelerator_command_count != 0
            || self.accelerator_gpu_kernel_command_count != 0
            || self.accelerator_npu_inference_command_count != 0
            || self.accelerator_dma_command_count != 0
            || self.accelerator_trace_event_count != 0
            || self.accelerator_completion_count != 0
            || self.accelerator_gpu_kernel_completion_count != 0
            || self.accelerator_npu_inference_completion_count != 0
            || self.accelerator_dma_command_completion_count != 0
    }

    pub const fn has_accelerator_npu_activity(&self) -> bool {
        self.accelerator_npu_inference_command_count != 0
            || self.accelerator_npu_inference_completion_count != 0
    }

    pub fn accelerator_compute_wait_for_edge_count(&self) -> usize {
        self.accelerator_compute_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.accelerator_compute_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.accelerator_compute_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.accelerator_compute_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.accelerator_compute_wait_for_target_node_windows,
            ))
    }

    pub fn accelerator_compute_wait_for_edge_kind_counts(
        &self,
    ) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.accelerator_compute_wait_for_edge_kind_counts
    }

    pub fn accelerator_compute_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.accelerator_compute_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.accelerator_compute_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn accelerator_compute_wait_for_edge_kind_windows(
        &self,
    ) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.accelerator_compute_wait_for_edge_kind_windows
    }

    pub fn accelerator_compute_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.accelerator_compute_wait_for_edge_kind_windows, kind)
    }

    pub const fn accelerator_compute_deadlock_diagnostic_count(&self) -> usize {
        self.accelerator_compute_deadlock_diagnostic_count
    }

    pub fn has_accelerator_compute_diagnostics(&self) -> bool {
        self.accelerator_compute_wait_for_edge_count() != 0
            || self.accelerator_compute_deadlock_diagnostic_count != 0
    }

    pub fn compute_wait_for_edge_count(&self) -> usize {
        self.gpu_compute_wait_for_edge_count() + self.accelerator_compute_wait_for_edge_count()
    }

    pub fn compute_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        merge_wait_for_edge_kind_counts([
            self.gpu_compute_wait_for_edge_kind_counts(),
            self.accelerator_compute_wait_for_edge_kind_counts(),
        ])
    }

    pub fn compute_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.gpu_compute_wait_for_edge_count_by_kind(kind)
            + self.accelerator_compute_wait_for_edge_count_by_kind(kind)
    }

    pub fn compute_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        merge_wait_for_edge_kind_windows(
            self.gpu_compute_wait_for_edge_kind_windows
                .iter()
                .copied()
                .chain(
                    self.accelerator_compute_wait_for_edge_kind_windows
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn compute_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.compute_wait_for_edge_kind_windows(), kind)
    }

    pub const fn compute_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_compute_deadlock_diagnostic_count
            + self.accelerator_compute_deadlock_diagnostic_count
    }

    pub fn has_compute_diagnostics(&self) -> bool {
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

    pub const fn accelerator_dma_scheduler_epoch_count(&self) -> usize {
        self.accelerator_dma_scheduler_epoch_count
    }

    pub const fn accelerator_dma_scheduler_empty_epoch_count(&self) -> usize {
        self.accelerator_dma_scheduler_empty_epoch_count
    }

    pub const fn accelerator_dma_scheduler_dispatch_count(&self) -> usize {
        self.accelerator_dma_scheduler_dispatch_count
    }

    pub const fn accelerator_dma_scheduler_batch_count(&self) -> usize {
        self.accelerator_dma_scheduler_batch_count
    }

    pub fn accelerator_dma_scheduler_batch_worker_counts(
        &self,
    ) -> &[WorkloadParallelBatchWorkerCount] {
        &self.accelerator_dma_scheduler_batch_worker_counts
    }

    pub fn accelerator_dma_scheduler_batch_worker_count_tick_summaries(&self) -> &[(usize, Tick)] {
        &self.accelerator_dma_scheduler_batch_worker_count_ticks
    }

    pub fn accelerator_dma_scheduler_batch_count_for_worker_count(
        &self,
        worker_count: usize,
    ) -> usize {
        batch_count_for_worker_count(
            &self.accelerator_dma_scheduler_batch_worker_counts,
            worker_count,
        )
    }

    pub fn accelerator_dma_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        batch_count_at_or_above(
            &self.accelerator_dma_scheduler_batch_worker_counts,
            minimum_worker_count,
        )
    }

    pub fn accelerator_dma_scheduler_max_workers(&self) -> usize {
        max_parallel_batch_worker_count(&self.accelerator_dma_scheduler_batch_worker_counts).max(
            max_batch_worker_count_ticks(&self.accelerator_dma_scheduler_batch_worker_count_ticks),
        )
    }

    pub fn accelerator_dma_scheduler_total_workers(&self) -> usize {
        total_parallel_batch_worker_count(&self.accelerator_dma_scheduler_batch_worker_counts)
    }

    pub fn accelerator_dma_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        batch_ticks_for_worker_count(
            &self.accelerator_dma_scheduler_batch_worker_count_ticks,
            worker_count,
        )
    }

    pub fn accelerator_dma_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        batch_ticks_at_or_above(
            &self.accelerator_dma_scheduler_batch_worker_count_ticks,
            minimum_worker_count,
        )
    }

    pub fn accelerator_dma_scheduler_batch_worker_ticks(&self) -> Tick {
        batch_worker_ticks(&self.accelerator_dma_scheduler_batch_worker_count_ticks)
    }

    pub fn accelerator_dma_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        batch_worker_ticks_at_or_above(
            &self.accelerator_dma_scheduler_batch_worker_count_ticks,
            minimum_worker_count,
        )
    }

    pub fn accelerator_dma_scheduler_initial_frontiers(&self) -> &[PartitionFrontier] {
        &self.accelerator_dma_scheduler_initial_frontiers
    }

    pub fn accelerator_dma_scheduler_final_frontiers(&self) -> &[PartitionFrontier] {
        &self.accelerator_dma_scheduler_final_frontiers
    }

    pub fn accelerator_dma_scheduler_initial_frontier_count(&self) -> usize {
        self.accelerator_dma_scheduler_initial_frontiers.len()
    }

    pub fn accelerator_dma_scheduler_final_frontier_count(&self) -> usize {
        self.accelerator_dma_scheduler_final_frontiers.len()
    }

    pub fn has_accelerator_dma_scheduler_frontiers(&self) -> bool {
        !self.accelerator_dma_scheduler_initial_frontiers.is_empty()
            || !self.accelerator_dma_scheduler_final_frontiers.is_empty()
    }

    pub fn accelerator_dma_scheduler_remote_flows(&self) -> &[ParallelRemoteFlowRecord] {
        &self.accelerator_dma_scheduler_remote_flows
    }

    pub fn accelerator_dma_scheduler_remote_flow_evidence(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_evidence(
            self.accelerator_dma_scheduler_remote_flows.iter().copied(),
            self.accelerator_dma_scheduler_remote_sends.iter().copied(),
        )
    }

    pub fn accelerator_dma_scheduler_remote_sends(&self) -> &[ParallelRemoteSendRecord] {
        &self.accelerator_dma_scheduler_remote_sends
    }

    pub fn accelerator_dma_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_evidence_count(
            &self.accelerator_dma_scheduler_remote_flows,
            &self.accelerator_dma_scheduler_remote_sends,
            source,
            target,
        )
    }

    pub fn accelerator_dma_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_send_count(&self.accelerator_dma_scheduler_remote_sends, source, target)
    }

    pub fn has_accelerator_dma_scheduler_remote_flows(&self) -> bool {
        !self
            .accelerator_dma_scheduler_remote_flow_evidence()
            .is_empty()
    }

    pub fn has_accelerator_dma_scheduler_remote_sends(&self) -> bool {
        self.accelerator_dma_scheduler_remote_sends
            .iter()
            .copied()
            .any(is_parallel_remote_send_evidence)
    }

    pub fn has_accelerator_dma_activity(&self) -> bool {
        self.accelerator_dma_copy_count != 0
            || self.accelerator_dma_completion_count != 0
            || self.accelerator_dma_scheduler_epoch_count != 0
            || self.accelerator_dma_scheduler_empty_epoch_count != 0
            || self.accelerator_dma_scheduler_dispatch_count != 0
            || self.accelerator_dma_scheduler_batch_count != 0
            || !self
                .accelerator_dma_scheduler_batch_worker_counts
                .is_empty()
            || !self
                .accelerator_dma_scheduler_batch_worker_count_ticks
                .is_empty()
            || self.has_accelerator_dma_scheduler_frontiers()
            || self.has_accelerator_dma_scheduler_remote_flows()
            || self.has_accelerator_dma_scheduler_remote_sends()
    }

    pub fn accelerator_dma_wait_for_edge_count(&self) -> usize {
        self.accelerator_dma_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.accelerator_dma_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.accelerator_dma_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.accelerator_dma_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.accelerator_dma_wait_for_target_node_windows,
            ))
    }

    pub fn accelerator_dma_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.accelerator_dma_wait_for_edge_kind_counts
    }

    pub fn accelerator_dma_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.accelerator_dma_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.accelerator_dma_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn accelerator_dma_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.accelerator_dma_wait_for_edge_kind_windows
    }

    pub fn accelerator_dma_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.accelerator_dma_wait_for_edge_kind_windows, kind)
    }

    pub const fn accelerator_dma_deadlock_diagnostic_count(&self) -> usize {
        self.accelerator_dma_deadlock_diagnostic_count
    }

    pub fn has_accelerator_dma_diagnostics(&self) -> bool {
        self.accelerator_dma_wait_for_edge_count() != 0
            || self.accelerator_dma_deadlock_diagnostic_count != 0
    }

    pub const fn dma_scheduler_epoch_count(&self) -> usize {
        self.gpu_dma_scheduler_epoch_count + self.accelerator_dma_scheduler_epoch_count
    }

    pub const fn dma_scheduler_empty_epoch_count(&self) -> usize {
        self.gpu_dma_scheduler_empty_epoch_count + self.accelerator_dma_scheduler_empty_epoch_count
    }

    pub const fn dma_scheduler_dispatch_count(&self) -> usize {
        self.gpu_dma_scheduler_dispatch_count + self.accelerator_dma_scheduler_dispatch_count
    }

    pub const fn dma_scheduler_batch_count(&self) -> usize {
        self.gpu_dma_scheduler_batch_count + self.accelerator_dma_scheduler_batch_count
    }

    pub fn dma_scheduler_batch_worker_counts(&self) -> Vec<WorkloadParallelBatchWorkerCount> {
        collect_parallel_batch_worker_counts(
            self.gpu_dma_scheduler_batch_worker_counts
                .iter()
                .copied()
                .chain(
                    self.accelerator_dma_scheduler_batch_worker_counts
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn dma_scheduler_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        self.gpu_dma_scheduler_batch_count_for_worker_count(worker_count)
            + self.accelerator_dma_scheduler_batch_count_for_worker_count(worker_count)
    }

    pub fn dma_scheduler_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        self.gpu_dma_scheduler_batch_count_at_or_above(minimum_worker_count)
            + self.accelerator_dma_scheduler_batch_count_at_or_above(minimum_worker_count)
    }

    pub fn dma_scheduler_max_workers(&self) -> usize {
        self.gpu_dma_scheduler_max_workers()
            .max(self.accelerator_dma_scheduler_max_workers())
    }

    pub fn dma_scheduler_total_workers(&self) -> usize {
        self.gpu_dma_scheduler_total_workers()
            .saturating_add(self.accelerator_dma_scheduler_total_workers())
    }

    pub fn dma_scheduler_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        self.gpu_dma_scheduler_batch_ticks_for_worker_count(worker_count)
            .saturating_add(
                self.accelerator_dma_scheduler_batch_ticks_for_worker_count(worker_count),
            )
    }

    pub fn dma_scheduler_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        self.gpu_dma_scheduler_batch_ticks_at_or_above(minimum_worker_count)
            .saturating_add(
                self.accelerator_dma_scheduler_batch_ticks_at_or_above(minimum_worker_count),
            )
    }

    pub fn dma_scheduler_batch_worker_ticks(&self) -> Tick {
        self.gpu_dma_scheduler_batch_worker_ticks()
            .saturating_add(self.accelerator_dma_scheduler_batch_worker_ticks())
    }

    pub fn dma_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        self.gpu_dma_scheduler_batch_worker_ticks_at_or_above(minimum_worker_count)
            .saturating_add(
                self.accelerator_dma_scheduler_batch_worker_ticks_at_or_above(minimum_worker_count),
            )
    }

    pub fn dma_scheduler_initial_frontiers(&self) -> Vec<PartitionFrontier> {
        collect_conservative_partition_frontiers(
            self.gpu_dma_scheduler_initial_frontiers
                .iter()
                .copied()
                .chain(
                    self.accelerator_dma_scheduler_initial_frontiers
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn dma_scheduler_final_frontiers(&self) -> Vec<PartitionFrontier> {
        collect_conservative_partition_frontiers(
            self.gpu_dma_scheduler_final_frontiers
                .iter()
                .copied()
                .chain(
                    self.accelerator_dma_scheduler_final_frontiers
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn dma_scheduler_initial_frontier_count(&self) -> usize {
        self.dma_scheduler_initial_frontiers().len()
    }

    pub fn dma_scheduler_final_frontier_count(&self) -> usize {
        self.dma_scheduler_final_frontiers().len()
    }

    pub fn has_dma_scheduler_frontiers(&self) -> bool {
        self.has_gpu_dma_scheduler_frontiers() || self.has_accelerator_dma_scheduler_frontiers()
    }

    pub fn dma_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_aggregates(
            self.gpu_dma_scheduler_remote_flow_evidence()
                .into_iter()
                .chain(self.accelerator_dma_scheduler_remote_flow_evidence()),
        )
    }

    pub fn dma_scheduler_remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        collect_parallel_remote_sends(
            self.gpu_dma_scheduler_remote_sends
                .iter()
                .copied()
                .chain(self.accelerator_dma_scheduler_remote_sends.iter().copied()),
        )
    }

    pub fn dma_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_evidence_count(
            &self.dma_scheduler_remote_flows(),
            &self.dma_scheduler_remote_sends(),
            source,
            target,
        )
    }

    pub fn dma_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_send_count(&self.dma_scheduler_remote_sends(), source, target)
    }

    pub fn has_dma_scheduler_remote_flows(&self) -> bool {
        self.has_gpu_dma_scheduler_remote_flows()
            || self.has_accelerator_dma_scheduler_remote_flows()
    }

    pub fn has_dma_scheduler_remote_sends(&self) -> bool {
        self.has_gpu_dma_scheduler_remote_sends()
            || self.has_accelerator_dma_scheduler_remote_sends()
    }

    pub fn dma_wait_for_edge_count(&self) -> usize {
        self.gpu_dma_wait_for_edge_count() + self.accelerator_dma_wait_for_edge_count()
    }

    pub fn dma_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        merge_wait_for_edge_kind_counts([
            self.gpu_dma_wait_for_edge_kind_counts(),
            self.accelerator_dma_wait_for_edge_kind_counts(),
        ])
    }

    pub fn dma_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.gpu_dma_wait_for_edge_count_by_kind(kind)
            + self.accelerator_dma_wait_for_edge_count_by_kind(kind)
    }

    pub fn dma_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        merge_wait_for_edge_kind_windows(
            self.gpu_dma_wait_for_edge_kind_windows
                .iter()
                .copied()
                .chain(
                    self.accelerator_dma_wait_for_edge_kind_windows
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn dma_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.dma_wait_for_edge_kind_windows(), kind)
    }

    pub const fn dma_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_dma_deadlock_diagnostic_count + self.accelerator_dma_deadlock_diagnostic_count
    }

    pub fn has_dma_diagnostics(&self) -> bool {
        self.has_gpu_dma_diagnostics() || self.has_accelerator_dma_diagnostics()
    }
}

fn collect_batch_worker_count_ticks(
    summaries: impl IntoIterator<Item = (usize, Tick)>,
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in summaries {
        if worker_count == 0 || ticks == 0 {
            continue;
        }
        let stored = by_worker_count.entry(worker_count).or_default();
        *stored = stored.saturating_add(ticks);
    }
    by_worker_count.into_iter().collect()
}

fn batch_ticks_for_worker_count(summaries: &[(usize, Tick)], worker_count: usize) -> Tick {
    summaries
        .iter()
        .filter(|(stored_worker_count, _)| *stored_worker_count == worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

fn batch_count_for_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
    worker_count: usize,
) -> usize {
    counts
        .iter()
        .filter(|count| count.worker_count() == worker_count)
        .map(WorkloadParallelBatchWorkerCount::batch_count)
        .sum()
}

fn batch_count_at_or_above(
    counts: &[WorkloadParallelBatchWorkerCount],
    minimum_worker_count: usize,
) -> usize {
    counts
        .iter()
        .filter(|count| count.worker_count() >= minimum_worker_count)
        .map(WorkloadParallelBatchWorkerCount::batch_count)
        .sum()
}

fn max_batch_worker_count_ticks(summaries: &[(usize, Tick)]) -> usize {
    summaries
        .iter()
        .filter(|(_, ticks)| *ticks != 0)
        .map(|(worker_count, _)| *worker_count)
        .max()
        .unwrap_or(0)
}

fn total_parallel_batch_count(counts: &[WorkloadParallelBatchWorkerCount]) -> usize {
    counts
        .iter()
        .map(WorkloadParallelBatchWorkerCount::batch_count)
        .sum()
}

fn batch_ticks_at_or_above(summaries: &[(usize, Tick)], minimum_worker_count: usize) -> Tick {
    summaries
        .iter()
        .filter(|(worker_count, _)| *worker_count >= minimum_worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

fn batch_worker_ticks(summaries: &[(usize, Tick)]) -> Tick {
    summaries
        .iter()
        .map(|(worker_count, ticks)| ticks.saturating_mul(*worker_count as Tick))
        .fold(0, Tick::saturating_add)
}

fn batch_worker_ticks_at_or_above(
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(worker_count, _)| *worker_count >= minimum_worker_count)
        .map(|(worker_count, ticks)| ticks.saturating_mul(*worker_count as Tick))
        .fold(0, Tick::saturating_add)
}
