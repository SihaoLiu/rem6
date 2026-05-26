use std::collections::BTreeMap;

use rem6_kernel::{PartitionFrontier, Tick};

use crate::parallel_batch::{
    collect_parallel_batch_worker_counts, max_parallel_batch_worker_count,
    total_parallel_batch_worker_count, WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_conservative_partition_frontiers, collect_partition_frontiers,
};

use super::WorkloadParallelExecutionSummary;

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

    pub const fn has_gpu_dma_activity(&self) -> bool {
        self.gpu_dma_copy_count != 0
            || self.gpu_dma_completion_count != 0
            || self.gpu_dma_scheduler_batch_count != 0
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

    pub const fn has_accelerator_dma_activity(&self) -> bool {
        self.accelerator_dma_copy_count != 0
            || self.accelerator_dma_completion_count != 0
            || self.accelerator_dma_scheduler_batch_count != 0
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

    pub const fn dma_wait_for_edge_count(&self) -> usize {
        self.gpu_dma_wait_for_edge_count + self.accelerator_dma_wait_for_edge_count
    }

    pub const fn dma_deadlock_diagnostic_count(&self) -> usize {
        self.gpu_dma_deadlock_diagnostic_count + self.accelerator_dma_deadlock_diagnostic_count
    }

    pub const fn has_dma_diagnostics(&self) -> bool {
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
