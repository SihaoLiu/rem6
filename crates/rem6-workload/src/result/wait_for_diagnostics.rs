use std::collections::BTreeMap;

use rem6_kernel::{Tick, WaitForEdgeKind, WaitForNode};

use crate::{WorkloadError, WorkloadParallelDiagnosticScope};

pub(super) use super::wait_for_node_windows::{
    collect_wait_for_blocked_node_windows, collect_wait_for_target_node_windows,
    merge_wait_for_blocked_node_windows, merge_wait_for_blocked_node_windows_by_strongest,
    merge_wait_for_target_node_windows, merge_wait_for_target_node_windows_by_strongest,
    wait_for_blocked_node_window, wait_for_blocked_node_window_count_sum,
    wait_for_target_node_window, wait_for_target_node_window_count_sum,
};
use super::{
    WorkloadParallelExecutionSummary, WorkloadWaitForBlockedNodeWindow,
    WorkloadWaitForEdgeKindWindow, WorkloadWaitForTargetNodeWindow,
};

impl WorkloadParallelExecutionSummary {
    pub(crate) fn validate_parallel_diagnostic_scope_summary(
        &self,
        scope: WorkloadParallelDiagnosticScope,
    ) -> Result<(), WorkloadError> {
        match scope {
            WorkloadParallelDiagnosticScope::Resource => {
                self.validate_resource_diagnostic_summary()
            }
            WorkloadParallelDiagnosticScope::DataCache => {
                validate_wait_for_edge_count_summary(
                    scope,
                    self.data_cache_wait_for_edge_count,
                    &self.data_cache_wait_for_edge_kind_counts,
                    &self.data_cache_wait_for_edge_kind_windows,
                    &self.data_cache_wait_for_blocked_node_windows,
                    &self.data_cache_wait_for_target_node_windows,
                )?;
                validate_wait_for_edge_kind_window_summary(
                    scope,
                    &self.data_cache_wait_for_edge_kind_counts,
                    &self.data_cache_wait_for_edge_kind_windows,
                )?;
                validate_livelock_transition_count_summary(
                    scope,
                    self.data_cache_parallel_scheduler_progress_transition_count(),
                    self.data_cache_parallel_scheduler_livelock_diagnostic_count(),
                    self.data_cache_parallel_scheduler_livelock_diagnostic_subject_summaries(),
                )
            }
            WorkloadParallelDiagnosticScope::Compute => {
                validate_wait_for_edge_count_summary(
                    scope,
                    self.gpu_compute_wait_for_edge_count,
                    &self.gpu_compute_wait_for_edge_kind_counts,
                    &self.gpu_compute_wait_for_edge_kind_windows,
                    &self.gpu_compute_wait_for_blocked_node_windows,
                    &self.gpu_compute_wait_for_target_node_windows,
                )?;
                validate_wait_for_edge_kind_window_summary(
                    scope,
                    &self.gpu_compute_wait_for_edge_kind_counts,
                    &self.gpu_compute_wait_for_edge_kind_windows,
                )?;
                validate_wait_for_edge_count_summary(
                    scope,
                    self.accelerator_compute_wait_for_edge_count,
                    &self.accelerator_compute_wait_for_edge_kind_counts,
                    &self.accelerator_compute_wait_for_edge_kind_windows,
                    &self.accelerator_compute_wait_for_blocked_node_windows,
                    &self.accelerator_compute_wait_for_target_node_windows,
                )?;
                validate_wait_for_edge_kind_window_summary(
                    scope,
                    &self.accelerator_compute_wait_for_edge_kind_counts,
                    &self.accelerator_compute_wait_for_edge_kind_windows,
                )
            }
            WorkloadParallelDiagnosticScope::Dma => {
                validate_wait_for_edge_count_summary(
                    scope,
                    self.gpu_dma_wait_for_edge_count,
                    &self.gpu_dma_wait_for_edge_kind_counts,
                    &self.gpu_dma_wait_for_edge_kind_windows,
                    &self.gpu_dma_wait_for_blocked_node_windows,
                    &self.gpu_dma_wait_for_target_node_windows,
                )?;
                validate_wait_for_edge_kind_window_summary(
                    scope,
                    &self.gpu_dma_wait_for_edge_kind_counts,
                    &self.gpu_dma_wait_for_edge_kind_windows,
                )?;
                validate_wait_for_edge_count_summary(
                    scope,
                    self.accelerator_dma_wait_for_edge_count,
                    &self.accelerator_dma_wait_for_edge_kind_counts,
                    &self.accelerator_dma_wait_for_edge_kind_windows,
                    &self.accelerator_dma_wait_for_blocked_node_windows,
                    &self.accelerator_dma_wait_for_target_node_windows,
                )?;
                validate_wait_for_edge_kind_window_summary(
                    scope,
                    &self.accelerator_dma_wait_for_edge_kind_counts,
                    &self.accelerator_dma_wait_for_edge_kind_windows,
                )
            }
            WorkloadParallelDiagnosticScope::FullSystem => {
                self.validate_parallel_diagnostic_scope_summary(
                    WorkloadParallelDiagnosticScope::Resource,
                )?;
                self.validate_parallel_diagnostic_scope_summary(
                    WorkloadParallelDiagnosticScope::DataCache,
                )?;
                self.validate_parallel_diagnostic_scope_summary(
                    WorkloadParallelDiagnosticScope::Compute,
                )?;
                self.validate_parallel_diagnostic_scope_summary(
                    WorkloadParallelDiagnosticScope::Dma,
                )?;
                let scoped_wait_for_counts = self.scoped_full_system_wait_for_edge_kind_counts();
                validate_wait_for_edge_kind_count_merge_summary(
                    scope,
                    &self.full_system_wait_for_edge_kind_counts,
                    &scoped_wait_for_counts,
                )?;
                let scoped_wait_for_windows = self.scoped_full_system_wait_for_edge_kind_windows();
                validate_wait_for_edge_kind_window_merge_summary(
                    scope,
                    &self.full_system_wait_for_edge_kind_windows,
                    &scoped_wait_for_windows,
                )?;
                let scoped_blocked_node_windows =
                    self.scoped_full_system_wait_for_blocked_node_windows();
                validate_wait_for_blocked_node_window_merge_summary(
                    scope,
                    &self.full_system_wait_for_blocked_node_windows,
                    &scoped_blocked_node_windows,
                )?;
                let scoped_target_node_windows =
                    self.scoped_full_system_wait_for_target_node_windows();
                validate_wait_for_target_node_window_merge_summary(
                    scope,
                    &self.full_system_wait_for_target_node_windows,
                    &scoped_target_node_windows,
                )?;
                validate_deadlock_merge_summary(
                    scope,
                    self.merged_full_system_deadlock_diagnostic_count,
                    self.resource_deadlock_diagnostic_count()
                        .saturating_add(self.data_cache_deadlock_diagnostic_count()),
                )?;
                validate_livelock_transition_count_summary(
                    scope,
                    self.parallel_scheduler_progress_transition_count(),
                    self.parallel_scheduler_livelock_diagnostic_count(),
                    self.parallel_scheduler_livelock_diagnostic_subject_summaries(),
                )?;
                validate_livelock_transition_count_summary(
                    scope,
                    self.full_system_progress_transition_count(),
                    self.full_system_livelock_diagnostic_count(),
                    self.full_system_livelock_diagnostic_subject_summaries(),
                )?;
                self.validate_full_system_livelock_merge_summary()
            }
        }
    }

    fn validate_full_system_livelock_merge_summary(&self) -> Result<(), WorkloadError> {
        if !self.has_merged_full_system_livelock_diagnostic_count {
            return Ok(());
        }

        let merged_evidence_count = livelock_summary_evidence_count(
            self.merged_full_system_livelock_diagnostic_count,
            self.full_system_livelock_diagnostic_subject_summaries(),
        );
        let scoped_subject_summaries = self
            .parallel_scheduler_livelock_diagnostic_subject_summaries()
            .into_iter()
            .chain(self.data_cache_parallel_scheduler_livelock_diagnostic_subject_summaries())
            .collect();
        let scoped_evidence_count = livelock_summary_evidence_count(
            self.parallel_scheduler_livelock_diagnostic_count()
                .saturating_add(self.data_cache_parallel_scheduler_livelock_diagnostic_count()),
            scoped_subject_summaries,
        );
        if merged_evidence_count < scoped_evidence_count {
            return Err(WorkloadError::InvalidParallelLivelockMergeSummary {
                scope: WorkloadParallelDiagnosticScope::FullSystem,
                merged_evidence_count,
                scoped_evidence_count,
            });
        }
        Ok(())
    }

    fn validate_resource_diagnostic_summary(&self) -> Result<(), WorkloadError> {
        validate_wait_for_edge_count_summary(
            WorkloadParallelDiagnosticScope::Resource,
            self.fabric_wait_for_edge_count,
            &self.fabric_wait_for_edge_kind_counts,
            &self.fabric_wait_for_edge_kind_windows,
            &self.fabric_wait_for_blocked_node_windows,
            &self.fabric_wait_for_target_node_windows,
        )?;
        validate_wait_for_edge_kind_window_summary(
            WorkloadParallelDiagnosticScope::Resource,
            &self.fabric_wait_for_edge_kind_counts,
            &self.fabric_wait_for_edge_kind_windows,
        )?;
        validate_wait_for_edge_count_summary(
            WorkloadParallelDiagnosticScope::Resource,
            self.dram_wait_for_edge_count,
            &self.dram_wait_for_edge_kind_counts,
            &self.dram_wait_for_edge_kind_windows,
            &self.dram_wait_for_blocked_node_windows,
            &self.dram_wait_for_target_node_windows,
        )?;
        validate_wait_for_edge_kind_window_summary(
            WorkloadParallelDiagnosticScope::Resource,
            &self.dram_wait_for_edge_kind_counts,
            &self.dram_wait_for_edge_kind_windows,
        )?;
        validate_deadlock_merge_summary(
            WorkloadParallelDiagnosticScope::Resource,
            self.merged_resource_deadlock_diagnostic_count,
            self.fabric_deadlock_diagnostic_count
                .saturating_add(self.dram_deadlock_diagnostic_count),
        )
    }

    pub fn with_data_cache_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.data_cache_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.data_cache_wait_for_edge_count =
            self.data_cache_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.data_cache_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_data_cache_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.data_cache_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.data_cache_wait_for_edge_kind_counts,
            &self.data_cache_wait_for_edge_kind_windows,
        );
        self.data_cache_wait_for_edge_count =
            self.data_cache_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.data_cache_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_data_cache_wait_for_blocked_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.data_cache_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(windows);
        self.data_cache_wait_for_edge_count =
            self.data_cache_wait_for_edge_count
                .max(wait_for_blocked_node_window_count_sum(
                    &self.data_cache_wait_for_blocked_node_windows,
                ));
        self
    }

    pub fn with_data_cache_wait_for_target_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.data_cache_wait_for_target_node_windows =
            collect_wait_for_target_node_windows(windows);
        self.data_cache_wait_for_edge_count =
            self.data_cache_wait_for_edge_count
                .max(wait_for_target_node_window_count_sum(
                    &self.data_cache_wait_for_target_node_windows,
                ));
        self
    }

    pub fn with_resource_wait_for_edge_kind_counts(
        mut self,
        fabric_counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
        dram_counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.fabric_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(fabric_counts);
        self.dram_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(dram_counts);
        self.fabric_wait_for_edge_count =
            self.fabric_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.fabric_wait_for_edge_kind_counts,
                ));
        self.dram_wait_for_edge_count =
            self.dram_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.dram_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_resource_wait_for_edge_kind_windows(
        mut self,
        fabric_windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
        dram_windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.fabric_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(fabric_windows);
        self.dram_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(dram_windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.fabric_wait_for_edge_kind_counts,
            &self.fabric_wait_for_edge_kind_windows,
        );
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.dram_wait_for_edge_kind_counts,
            &self.dram_wait_for_edge_kind_windows,
        );
        self.fabric_wait_for_edge_count =
            self.fabric_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.fabric_wait_for_edge_kind_counts,
                ));
        self.dram_wait_for_edge_count =
            self.dram_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.dram_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_resource_wait_for_blocked_node_windows(
        mut self,
        fabric_windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
        dram_windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.fabric_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(fabric_windows);
        self.dram_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(dram_windows);
        self.fabric_wait_for_edge_count =
            self.fabric_wait_for_edge_count
                .max(wait_for_blocked_node_window_count_sum(
                    &self.fabric_wait_for_blocked_node_windows,
                ));
        self.dram_wait_for_edge_count =
            self.dram_wait_for_edge_count
                .max(wait_for_blocked_node_window_count_sum(
                    &self.dram_wait_for_blocked_node_windows,
                ));
        self
    }

    pub fn with_resource_wait_for_target_node_windows(
        mut self,
        fabric_windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
        dram_windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.fabric_wait_for_target_node_windows =
            collect_wait_for_target_node_windows(fabric_windows);
        self.dram_wait_for_target_node_windows = collect_wait_for_target_node_windows(dram_windows);
        self.fabric_wait_for_edge_count =
            self.fabric_wait_for_edge_count
                .max(wait_for_target_node_window_count_sum(
                    &self.fabric_wait_for_target_node_windows,
                ));
        self.dram_wait_for_edge_count =
            self.dram_wait_for_edge_count
                .max(wait_for_target_node_window_count_sum(
                    &self.dram_wait_for_target_node_windows,
                ));
        self
    }

    pub fn with_full_system_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.full_system_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self
    }

    pub fn with_full_system_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.full_system_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.full_system_wait_for_edge_kind_counts,
            &self.full_system_wait_for_edge_kind_windows,
        );
        self
    }

    pub fn with_full_system_wait_for_blocked_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.full_system_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(windows);
        self
    }

    pub fn with_full_system_wait_for_target_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.full_system_wait_for_target_node_windows =
            collect_wait_for_target_node_windows(windows);
        self
    }

    pub fn with_gpu_compute_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.gpu_compute_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.gpu_compute_wait_for_edge_count =
            self.gpu_compute_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.gpu_compute_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_gpu_compute_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.gpu_compute_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.gpu_compute_wait_for_edge_kind_counts,
            &self.gpu_compute_wait_for_edge_kind_windows,
        );
        self.gpu_compute_wait_for_edge_count =
            self.gpu_compute_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.gpu_compute_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_gpu_compute_wait_for_blocked_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.gpu_compute_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(windows);
        self.gpu_compute_wait_for_edge_count =
            self.gpu_compute_wait_for_edge_count
                .max(wait_for_blocked_node_window_count_sum(
                    &self.gpu_compute_wait_for_blocked_node_windows,
                ));
        self
    }

    pub fn with_gpu_compute_wait_for_target_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.gpu_compute_wait_for_target_node_windows =
            collect_wait_for_target_node_windows(windows);
        self.gpu_compute_wait_for_edge_count =
            self.gpu_compute_wait_for_edge_count
                .max(wait_for_target_node_window_count_sum(
                    &self.gpu_compute_wait_for_target_node_windows,
                ));
        self
    }

    pub fn with_gpu_dma_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.gpu_dma_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.gpu_dma_wait_for_edge_count =
            self.gpu_dma_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.gpu_dma_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_gpu_dma_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.gpu_dma_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.gpu_dma_wait_for_edge_kind_counts,
            &self.gpu_dma_wait_for_edge_kind_windows,
        );
        self.gpu_dma_wait_for_edge_count =
            self.gpu_dma_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.gpu_dma_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_gpu_dma_wait_for_blocked_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.gpu_dma_wait_for_blocked_node_windows = collect_wait_for_blocked_node_windows(windows);
        self.gpu_dma_wait_for_edge_count =
            self.gpu_dma_wait_for_edge_count
                .max(wait_for_blocked_node_window_count_sum(
                    &self.gpu_dma_wait_for_blocked_node_windows,
                ));
        self
    }

    pub fn with_gpu_dma_wait_for_target_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.gpu_dma_wait_for_target_node_windows = collect_wait_for_target_node_windows(windows);
        self.gpu_dma_wait_for_edge_count =
            self.gpu_dma_wait_for_edge_count
                .max(wait_for_target_node_window_count_sum(
                    &self.gpu_dma_wait_for_target_node_windows,
                ));
        self
    }

    pub fn with_accelerator_compute_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.accelerator_compute_wait_for_edge_kind_counts =
            collect_wait_for_edge_kind_counts(counts);
        self.accelerator_compute_wait_for_edge_count = self
            .accelerator_compute_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.accelerator_compute_wait_for_edge_kind_counts,
            ));
        self
    }

    pub fn with_accelerator_compute_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.accelerator_compute_wait_for_edge_kind_windows =
            collect_wait_for_edge_kind_windows(windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.accelerator_compute_wait_for_edge_kind_counts,
            &self.accelerator_compute_wait_for_edge_kind_windows,
        );
        self.accelerator_compute_wait_for_edge_count = self
            .accelerator_compute_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.accelerator_compute_wait_for_edge_kind_counts,
            ));
        self
    }

    pub fn with_accelerator_compute_wait_for_blocked_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.accelerator_compute_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(windows);
        self.accelerator_compute_wait_for_edge_count = self
            .accelerator_compute_wait_for_edge_count
            .max(wait_for_blocked_node_window_count_sum(
                &self.accelerator_compute_wait_for_blocked_node_windows,
            ));
        self
    }

    pub fn with_accelerator_compute_wait_for_target_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.accelerator_compute_wait_for_target_node_windows =
            collect_wait_for_target_node_windows(windows);
        self.accelerator_compute_wait_for_edge_count = self
            .accelerator_compute_wait_for_edge_count
            .max(wait_for_target_node_window_count_sum(
                &self.accelerator_compute_wait_for_target_node_windows,
            ));
        self
    }

    pub fn with_accelerator_dma_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.accelerator_dma_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.accelerator_dma_wait_for_edge_count =
            self.accelerator_dma_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.accelerator_dma_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_accelerator_dma_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.accelerator_dma_wait_for_edge_kind_windows =
            collect_wait_for_edge_kind_windows(windows);
        merge_wait_for_edge_kind_counts_from_windows(
            &mut self.accelerator_dma_wait_for_edge_kind_counts,
            &self.accelerator_dma_wait_for_edge_kind_windows,
        );
        self.accelerator_dma_wait_for_edge_count =
            self.accelerator_dma_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.accelerator_dma_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_accelerator_dma_wait_for_blocked_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
    ) -> Self {
        self.accelerator_dma_wait_for_blocked_node_windows =
            collect_wait_for_blocked_node_windows(windows);
        self.accelerator_dma_wait_for_edge_count =
            self.accelerator_dma_wait_for_edge_count
                .max(wait_for_blocked_node_window_count_sum(
                    &self.accelerator_dma_wait_for_blocked_node_windows,
                ));
        self
    }

    pub fn with_accelerator_dma_wait_for_target_node_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
    ) -> Self {
        self.accelerator_dma_wait_for_target_node_windows =
            collect_wait_for_target_node_windows(windows);
        self.accelerator_dma_wait_for_edge_count =
            self.accelerator_dma_wait_for_edge_count
                .max(wait_for_target_node_window_count_sum(
                    &self.accelerator_dma_wait_for_target_node_windows,
                ));
        self
    }

    pub fn data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.data_cache_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.data_cache_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.data_cache_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.data_cache_wait_for_target_node_windows,
            ))
    }

    pub fn data_cache_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.data_cache_wait_for_edge_kind_counts
    }

    pub fn data_cache_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.data_cache_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.data_cache_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn data_cache_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.data_cache_wait_for_edge_kind_windows
    }

    pub fn data_cache_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.data_cache_wait_for_edge_kind_windows, kind)
    }

    pub fn data_cache_wait_for_blocked_node_windows(&self) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.data_cache_wait_for_blocked_node_windows
    }

    pub fn data_cache_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.data_cache_wait_for_blocked_node_windows, node)
    }

    pub fn data_cache_wait_for_target_node_windows(&self) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.data_cache_wait_for_target_node_windows
    }

    pub fn data_cache_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.data_cache_wait_for_target_node_windows, node)
    }

    pub fn fabric_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.fabric_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.fabric_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.fabric_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.fabric_wait_for_target_node_windows,
            ))
    }

    pub fn fabric_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.fabric_wait_for_edge_kind_counts
    }

    pub fn fabric_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.fabric_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.fabric_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn fabric_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.fabric_wait_for_edge_kind_windows
    }

    pub fn fabric_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.fabric_wait_for_edge_kind_windows, kind)
    }

    pub fn fabric_wait_for_blocked_node_windows(&self) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.fabric_wait_for_blocked_node_windows
    }

    pub fn fabric_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.fabric_wait_for_blocked_node_windows, node)
    }

    pub fn fabric_wait_for_target_node_windows(&self) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.fabric_wait_for_target_node_windows
    }

    pub fn fabric_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.fabric_wait_for_target_node_windows, node)
    }

    pub fn dram_wait_for_edge_count(&self) -> usize {
        self.dram_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.dram_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.dram_wait_for_edge_kind_windows,
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &self.dram_wait_for_blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(
                &self.dram_wait_for_target_node_windows,
            ))
    }

    pub fn dram_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.dram_wait_for_edge_kind_counts
    }

    pub fn dram_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.dram_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.dram_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn dram_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.dram_wait_for_edge_kind_windows
    }

    pub fn dram_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.dram_wait_for_edge_kind_windows, kind)
    }

    pub fn dram_wait_for_blocked_node_windows(&self) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.dram_wait_for_blocked_node_windows
    }

    pub fn dram_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.dram_wait_for_blocked_node_windows, node)
    }

    pub fn dram_wait_for_target_node_windows(&self) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.dram_wait_for_target_node_windows
    }

    pub fn dram_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.dram_wait_for_target_node_windows, node)
    }

    pub fn resource_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count() + self.dram_wait_for_edge_count()
    }

    pub fn resource_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        merge_wait_for_edge_kind_counts([
            self.fabric_wait_for_edge_kind_counts(),
            self.dram_wait_for_edge_kind_counts(),
        ])
    }

    pub fn resource_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.fabric_wait_for_edge_count_by_kind(kind) + self.dram_wait_for_edge_count_by_kind(kind)
    }

    pub fn resource_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        merge_wait_for_edge_kind_windows(
            self.fabric_wait_for_edge_kind_windows
                .iter()
                .copied()
                .chain(self.dram_wait_for_edge_kind_windows.iter().copied()),
        )
    }

    pub fn resource_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.resource_wait_for_edge_kind_windows(), kind)
    }

    pub fn resource_wait_for_blocked_node_windows(&self) -> Vec<WorkloadWaitForBlockedNodeWindow> {
        merge_wait_for_blocked_node_windows(
            self.fabric_wait_for_blocked_node_windows
                .iter()
                .cloned()
                .chain(self.dram_wait_for_blocked_node_windows.iter().cloned()),
        )
    }

    pub fn resource_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.resource_wait_for_blocked_node_windows(), node)
    }

    pub fn resource_wait_for_target_node_windows(&self) -> Vec<WorkloadWaitForTargetNodeWindow> {
        merge_wait_for_target_node_windows(
            self.fabric_wait_for_target_node_windows
                .iter()
                .cloned()
                .chain(self.dram_wait_for_target_node_windows.iter().cloned()),
        )
    }

    pub fn resource_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.resource_wait_for_target_node_windows(), node)
    }

    pub fn resource_activity_count(&self) -> usize {
        self.fabric_transfer_count
            .saturating_add(self.dram_operation_count())
            .saturating_add(self.resource_wait_for_edge_count())
    }

    pub const fn has_fabric_activity(&self) -> bool {
        self.fabric_transfer_count != 0
            || self.fabric_byte_count != 0
            || self.fabric_occupied_ticks != 0
            || self.fabric_queue_delay_ticks != 0
            || self.fabric_max_queue_delay_ticks != 0
            || self.contended_fabric_lane_count != 0
    }

    pub fn active_fabric_resource_count(&self) -> usize {
        let activity_floor = usize::from(
            self.has_fabric_activity()
                || self.fabric_wait_for_edge_count() != 0
                || !self.fabric_wait_for_target_node_windows.is_empty(),
        );
        self.active_fabric_lane_count
            .max(self.contended_fabric_lane_count)
            .max(self.fabric_wait_for_target_node_windows.len())
            .max(activity_floor)
    }

    pub fn dram_operation_count(&self) -> usize {
        let qos_priority_access_count = self
            .dram_qos_priority_summaries
            .iter()
            .fold(0usize, |count, summary| {
                count.saturating_add(summary.access_count())
            });
        let qos_requestor_access_count = self
            .dram_qos_requestor_summaries
            .iter()
            .fold(0usize, |count, summary| {
                count.saturating_add(summary.access_count())
            });

        self.dram_access_count
            .max(self.dram_read_count.saturating_add(self.dram_write_count))
            .max(
                self.dram_row_hit_count
                    .saturating_add(self.dram_row_miss_count),
            )
            .max(self.dram_command_count)
            .max(self.dram_qos_access_count)
            .max(qos_priority_access_count)
            .max(qos_requestor_access_count)
    }

    pub fn active_dram_resource_count(&self) -> usize {
        let activity_floor = usize::from(
            self.has_dram_activity()
                || self.dram_wait_for_edge_count() != 0
                || !self.dram_wait_for_target_node_windows.is_empty(),
        );
        self.active_dram_target_count
            .max(usize::from(self.active_dram_port_count != 0))
            .max(usize::from(self.active_dram_bank_count != 0))
            .max(self.dram_wait_for_target_node_windows.len())
            .max(activity_floor)
    }

    pub fn active_resource_count(&self) -> usize {
        self.active_fabric_resource_count()
            .saturating_add(self.active_dram_resource_count())
    }

    pub fn has_dram_qos_activity(&self) -> bool {
        self.dram_qos_access_count != 0
            || self.dram_qos_byte_count != 0
            || self.dram_qos_escalated_access_count != 0
            || !self.dram_qos_priority_summaries.is_empty()
            || !self.dram_qos_requestor_summaries.is_empty()
    }

    pub fn has_dram_activity(&self) -> bool {
        self.dram_access_count != 0
            || self.dram_read_count != 0
            || self.dram_write_count != 0
            || self.dram_row_hit_count != 0
            || self.dram_row_miss_count != 0
            || self.dram_command_count != 0
            || self.dram_turnaround_count != 0
            || self.dram_total_ready_latency_cycles != 0
            || self.dram_max_ready_latency_cycles != 0
            || self.has_dram_qos_activity()
    }

    pub fn has_resource_activity(&self) -> bool {
        self.has_fabric_activity()
            || self.has_dram_activity()
            || self.resource_wait_for_edge_count() != 0
    }

    fn scoped_full_system_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        let resource_counts = self.resource_wait_for_edge_kind_counts();
        let compute_counts = self.compute_wait_for_edge_kind_counts();
        let dma_counts = self.dma_wait_for_edge_kind_counts();
        merge_wait_for_edge_kind_counts([
            &resource_counts,
            self.data_cache_wait_for_edge_kind_counts(),
            &compute_counts,
            &dma_counts,
        ])
    }

    fn scoped_full_system_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        let resource_windows = self.resource_wait_for_edge_kind_windows();
        let compute_windows = self.compute_wait_for_edge_kind_windows();
        let dma_windows = self.dma_wait_for_edge_kind_windows();
        merge_wait_for_edge_kind_windows(
            resource_windows
                .into_iter()
                .chain(self.data_cache_wait_for_edge_kind_windows.iter().copied())
                .chain(compute_windows)
                .chain(dma_windows),
        )
    }

    fn scoped_full_system_wait_for_blocked_node_windows(
        &self,
    ) -> Vec<WorkloadWaitForBlockedNodeWindow> {
        let resource_windows = self.resource_wait_for_blocked_node_windows();
        let compute_windows = self.compute_wait_for_blocked_node_windows();
        let dma_windows = self.dma_wait_for_blocked_node_windows();
        merge_wait_for_blocked_node_windows(
            resource_windows
                .into_iter()
                .chain(
                    self.data_cache_wait_for_blocked_node_windows
                        .iter()
                        .cloned(),
                )
                .chain(compute_windows)
                .chain(dma_windows),
        )
    }

    fn scoped_full_system_wait_for_target_node_windows(
        &self,
    ) -> Vec<WorkloadWaitForTargetNodeWindow> {
        let resource_windows = self.resource_wait_for_target_node_windows();
        let compute_windows = self.compute_wait_for_target_node_windows();
        let dma_windows = self.dma_wait_for_target_node_windows();
        merge_wait_for_target_node_windows(
            resource_windows
                .into_iter()
                .chain(self.data_cache_wait_for_target_node_windows.iter().cloned())
                .chain(compute_windows)
                .chain(dma_windows),
        )
    }

    pub fn full_system_wait_for_edge_count(&self) -> usize {
        let scoped_edge_count = self.resource_wait_for_edge_count()
            + self.data_cache_wait_for_edge_count()
            + self.compute_wait_for_edge_count()
            + self.dma_wait_for_edge_count();
        let blocked_node_windows = self.full_system_wait_for_blocked_node_windows();
        let target_node_windows = self.full_system_wait_for_target_node_windows();
        scoped_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.full_system_wait_for_edge_kind_counts(),
            ))
            .max(wait_for_blocked_node_window_count_sum(
                &blocked_node_windows,
            ))
            .max(wait_for_target_node_window_count_sum(&target_node_windows))
    }

    pub fn full_system_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = self.scoped_full_system_wait_for_edge_kind_counts();
        for (kind, count) in &self.full_system_wait_for_edge_kind_counts {
            counts
                .entry(*kind)
                .and_modify(|stored| *stored = (*stored).max(*count))
                .or_insert(*count);
        }
        counts
    }

    pub fn full_system_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.full_system_wait_for_edge_kind_counts(), kind)
    }

    pub fn full_system_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        let scoped_windows = self.scoped_full_system_wait_for_edge_kind_windows();
        merge_wait_for_edge_kind_windows_by_strongest(
            scoped_windows
                .into_iter()
                .chain(self.full_system_wait_for_edge_kind_windows.iter().copied()),
        )
    }

    pub fn full_system_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.full_system_wait_for_edge_kind_windows(), kind)
    }

    pub fn gpu_compute_wait_for_blocked_node_windows(&self) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.gpu_compute_wait_for_blocked_node_windows
    }

    pub fn gpu_compute_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.gpu_compute_wait_for_blocked_node_windows, node)
    }

    pub fn gpu_compute_wait_for_target_node_windows(&self) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.gpu_compute_wait_for_target_node_windows
    }

    pub fn gpu_compute_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.gpu_compute_wait_for_target_node_windows, node)
    }

    pub fn gpu_dma_wait_for_target_node_windows(&self) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.gpu_dma_wait_for_target_node_windows
    }

    pub fn gpu_dma_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.gpu_dma_wait_for_target_node_windows, node)
    }

    pub fn gpu_dma_wait_for_blocked_node_windows(&self) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.gpu_dma_wait_for_blocked_node_windows
    }

    pub fn gpu_dma_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.gpu_dma_wait_for_blocked_node_windows, node)
    }

    pub fn accelerator_compute_wait_for_target_node_windows(
        &self,
    ) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.accelerator_compute_wait_for_target_node_windows
    }

    pub fn accelerator_compute_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.accelerator_compute_wait_for_target_node_windows, node)
    }

    pub fn accelerator_compute_wait_for_blocked_node_windows(
        &self,
    ) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.accelerator_compute_wait_for_blocked_node_windows
    }

    pub fn accelerator_compute_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(
            &self.accelerator_compute_wait_for_blocked_node_windows,
            node,
        )
    }

    pub fn accelerator_dma_wait_for_target_node_windows(
        &self,
    ) -> &[WorkloadWaitForTargetNodeWindow] {
        &self.accelerator_dma_wait_for_target_node_windows
    }

    pub fn accelerator_dma_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.accelerator_dma_wait_for_target_node_windows, node)
    }

    pub fn accelerator_dma_wait_for_blocked_node_windows(
        &self,
    ) -> &[WorkloadWaitForBlockedNodeWindow] {
        &self.accelerator_dma_wait_for_blocked_node_windows
    }

    pub fn accelerator_dma_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.accelerator_dma_wait_for_blocked_node_windows, node)
    }

    pub fn compute_wait_for_blocked_node_windows(&self) -> Vec<WorkloadWaitForBlockedNodeWindow> {
        merge_wait_for_blocked_node_windows(
            self.gpu_compute_wait_for_blocked_node_windows
                .iter()
                .cloned()
                .chain(
                    self.accelerator_compute_wait_for_blocked_node_windows
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn compute_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.compute_wait_for_blocked_node_windows(), node)
    }

    pub fn compute_wait_for_target_node_windows(&self) -> Vec<WorkloadWaitForTargetNodeWindow> {
        merge_wait_for_target_node_windows(
            self.gpu_compute_wait_for_target_node_windows
                .iter()
                .cloned()
                .chain(
                    self.accelerator_compute_wait_for_target_node_windows
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn compute_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.compute_wait_for_target_node_windows(), node)
    }

    pub fn dma_wait_for_target_node_windows(&self) -> Vec<WorkloadWaitForTargetNodeWindow> {
        merge_wait_for_target_node_windows(
            self.gpu_dma_wait_for_target_node_windows
                .iter()
                .cloned()
                .chain(
                    self.accelerator_dma_wait_for_target_node_windows
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn dma_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.dma_wait_for_target_node_windows(), node)
    }

    pub fn dma_wait_for_blocked_node_windows(&self) -> Vec<WorkloadWaitForBlockedNodeWindow> {
        merge_wait_for_blocked_node_windows(
            self.gpu_dma_wait_for_blocked_node_windows
                .iter()
                .cloned()
                .chain(
                    self.accelerator_dma_wait_for_blocked_node_windows
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn dma_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.dma_wait_for_blocked_node_windows(), node)
    }

    pub fn full_system_wait_for_blocked_node_windows(
        &self,
    ) -> Vec<WorkloadWaitForBlockedNodeWindow> {
        let scoped_windows = self.scoped_full_system_wait_for_blocked_node_windows();
        merge_wait_for_blocked_node_windows_by_strongest(
            scoped_windows.into_iter().chain(
                self.full_system_wait_for_blocked_node_windows
                    .iter()
                    .cloned(),
            ),
        )
    }

    pub fn full_system_wait_for_blocked_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        wait_for_blocked_node_window(&self.full_system_wait_for_blocked_node_windows(), node)
    }

    pub fn full_system_wait_for_target_node_windows(&self) -> Vec<WorkloadWaitForTargetNodeWindow> {
        let scoped_windows = self.scoped_full_system_wait_for_target_node_windows();
        merge_wait_for_target_node_windows_by_strongest(
            scoped_windows.into_iter().chain(
                self.full_system_wait_for_target_node_windows
                    .iter()
                    .cloned(),
            ),
        )
    }

    pub fn full_system_wait_for_target_node_window(
        &self,
        node: &WaitForNode,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        wait_for_target_node_window(&self.full_system_wait_for_target_node_windows(), node)
    }
}

pub(super) fn collect_wait_for_edge_kind_counts(
    counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut by_kind = BTreeMap::new();
    for (kind, count) in counts {
        if count == 0 {
            continue;
        }
        let stored = by_kind.entry(kind).or_insert(0usize);
        *stored = stored.saturating_add(count);
    }
    by_kind
}

fn validate_wait_for_edge_count_summary(
    scope: WorkloadParallelDiagnosticScope,
    wait_for_edge_count: usize,
    edge_kind_counts: &BTreeMap<WaitForEdgeKind, usize>,
    edge_kind_windows: &[WorkloadWaitForEdgeKindWindow],
    blocked_node_windows: &[WorkloadWaitForBlockedNodeWindow],
    target_node_windows: &[WorkloadWaitForTargetNodeWindow],
) -> Result<(), WorkloadError> {
    let evidence_edge_count = wait_for_edge_kind_count_sum(edge_kind_counts)
        .max(wait_for_edge_kind_window_count_sum(edge_kind_windows))
        .max(wait_for_blocked_node_window_count_sum(blocked_node_windows))
        .max(wait_for_target_node_window_count_sum(target_node_windows));
    if wait_for_edge_count < evidence_edge_count {
        return Err(WorkloadError::InvalidParallelWaitForEdgeCountSummary {
            scope,
            wait_for_edge_count,
            evidence_edge_count,
        });
    }
    Ok(())
}

fn validate_wait_for_edge_kind_window_summary(
    scope: WorkloadParallelDiagnosticScope,
    counts: &BTreeMap<WaitForEdgeKind, usize>,
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> Result<(), WorkloadError> {
    for window in windows {
        let edge_kind_count = wait_for_edge_kind_count(counts, window.kind());
        if edge_kind_count < window.edge_count() {
            return Err(WorkloadError::InvalidParallelWaitForEdgeKindWindowSummary {
                scope,
                kind: window.kind(),
                edge_kind_count,
                window_edge_count: window.edge_count(),
            });
        }
    }
    Ok(())
}

fn validate_wait_for_edge_kind_count_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &BTreeMap<WaitForEdgeKind, usize>,
    scoped: &BTreeMap<WaitForEdgeKind, usize>,
) -> Result<(), WorkloadError> {
    for (kind, scoped_edge_count) in scoped {
        let Some(merged_edge_count) = merged.get(kind).copied() else {
            continue;
        };
        if merged_edge_count < *scoped_edge_count {
            return Err(
                WorkloadError::InvalidParallelWaitForEdgeKindCountMergeSummary {
                    scope,
                    kind: *kind,
                    merged_edge_count,
                    scoped_edge_count: *scoped_edge_count,
                },
            );
        }
    }
    Ok(())
}

fn validate_wait_for_edge_kind_window_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &[WorkloadWaitForEdgeKindWindow],
    scoped: &[WorkloadWaitForEdgeKindWindow],
) -> Result<(), WorkloadError> {
    for scoped_window in scoped {
        let Some(merged_window) = wait_for_edge_kind_window(merged, scoped_window.kind()) else {
            continue;
        };
        if merged_window.edge_count() < scoped_window.edge_count()
            || merged_window.first_tick() > scoped_window.first_tick()
            || merged_window.last_tick() < scoped_window.last_tick()
        {
            return Err(
                WorkloadError::InvalidParallelWaitForEdgeKindWindowMergeSummary {
                    scope,
                    kind: scoped_window.kind(),
                    merged_edge_count: merged_window.edge_count(),
                    scoped_edge_count: scoped_window.edge_count(),
                    merged_first_tick: merged_window.first_tick(),
                    scoped_first_tick: scoped_window.first_tick(),
                    merged_last_tick: merged_window.last_tick(),
                    scoped_last_tick: scoped_window.last_tick(),
                },
            );
        }
    }
    Ok(())
}

fn validate_wait_for_blocked_node_window_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &[WorkloadWaitForBlockedNodeWindow],
    scoped: &[WorkloadWaitForBlockedNodeWindow],
) -> Result<(), WorkloadError> {
    for scoped_window in scoped {
        let Some(merged_window) = wait_for_blocked_node_window(merged, scoped_window.node()) else {
            continue;
        };
        if merged_window.edge_count() < scoped_window.edge_count()
            || merged_window.first_tick() > scoped_window.first_tick()
            || merged_window.last_tick() < scoped_window.last_tick()
        {
            return Err(
                WorkloadError::InvalidParallelWaitForBlockedNodeWindowMergeSummary {
                    scope,
                    node: scoped_window.node().clone(),
                    merged_edge_count: merged_window.edge_count(),
                    scoped_edge_count: scoped_window.edge_count(),
                    merged_first_tick: merged_window.first_tick(),
                    scoped_first_tick: scoped_window.first_tick(),
                    merged_last_tick: merged_window.last_tick(),
                    scoped_last_tick: scoped_window.last_tick(),
                },
            );
        }
    }
    Ok(())
}

fn validate_wait_for_target_node_window_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &[WorkloadWaitForTargetNodeWindow],
    scoped: &[WorkloadWaitForTargetNodeWindow],
) -> Result<(), WorkloadError> {
    for scoped_window in scoped {
        let Some(merged_window) = wait_for_target_node_window(merged, scoped_window.node()) else {
            continue;
        };
        if merged_window.edge_count() < scoped_window.edge_count()
            || merged_window.first_tick() > scoped_window.first_tick()
            || merged_window.last_tick() < scoped_window.last_tick()
        {
            return Err(
                WorkloadError::InvalidParallelWaitForTargetNodeWindowMergeSummary {
                    scope,
                    node: scoped_window.node().clone(),
                    merged_edge_count: merged_window.edge_count(),
                    scoped_edge_count: scoped_window.edge_count(),
                    merged_first_tick: merged_window.first_tick(),
                    scoped_first_tick: scoped_window.first_tick(),
                    merged_last_tick: merged_window.last_tick(),
                    scoped_last_tick: scoped_window.last_tick(),
                },
            );
        }
    }
    Ok(())
}

fn validate_deadlock_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged_diagnostic_count: usize,
    scoped_diagnostic_count: usize,
) -> Result<(), WorkloadError> {
    if merged_diagnostic_count != 0 && merged_diagnostic_count < scoped_diagnostic_count {
        return Err(WorkloadError::InvalidParallelDeadlockMergeSummary {
            scope,
            merged_diagnostic_count,
            scoped_diagnostic_count,
        });
    }
    Ok(())
}

fn validate_livelock_transition_count_summary(
    scope: WorkloadParallelDiagnosticScope,
    progress_transition_count: usize,
    livelock_diagnostic_count: usize,
    subject_summaries: Vec<(WaitForNode, usize, u64, Tick, Tick)>,
) -> Result<(), WorkloadError> {
    if progress_transition_count < livelock_diagnostic_count {
        return Err(
            WorkloadError::InvalidParallelLivelockDiagnosticCountSummary {
                scope,
                progress_transition_count,
                livelock_diagnostic_count,
            },
        );
    }
    let evidence_transition_count = subject_summaries
        .into_iter()
        .map(|(_, _, transition_count, _, _)| transition_count)
        .sum::<u64>();
    let progress_transition_count_u64 =
        u64::try_from(progress_transition_count).unwrap_or(u64::MAX);
    if progress_transition_count_u64 < evidence_transition_count {
        return Err(
            WorkloadError::InvalidParallelLivelockTransitionCountSummary {
                scope,
                progress_transition_count,
                evidence_transition_count,
            },
        );
    }
    Ok(())
}

fn livelock_summary_evidence_count(
    livelock_diagnostic_count: usize,
    subject_summaries: Vec<(WaitForNode, usize, u64, Tick, Tick)>,
) -> u64 {
    let transition_evidence_count = subject_summaries
        .into_iter()
        .map(|(_, _, transition_count, _, _)| transition_count)
        .sum::<u64>();
    let diagnostic_evidence_count = u64::try_from(livelock_diagnostic_count).unwrap_or(u64::MAX);
    diagnostic_evidence_count.max(transition_evidence_count)
}

pub(super) fn wait_for_edge_kind_count(
    counts: &BTreeMap<WaitForEdgeKind, usize>,
    kind: WaitForEdgeKind,
) -> usize {
    counts.get(&kind).copied().unwrap_or(0)
}

pub(super) fn wait_for_edge_kind_count_sum(counts: &BTreeMap<WaitForEdgeKind, usize>) -> usize {
    counts.values().copied().sum()
}

pub(super) fn merge_wait_for_edge_kind_counts<'a>(
    maps: impl IntoIterator<Item = &'a BTreeMap<WaitForEdgeKind, usize>>,
) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut merged = BTreeMap::new();
    for map in maps {
        for (kind, count) in map {
            let stored = merged.entry(*kind).or_insert(0usize);
            *stored = stored.saturating_add(*count);
        }
    }
    merged
}

pub(super) fn collect_wait_for_edge_kind_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    let mut by_kind = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_kind
            .entry(window.kind())
            .and_modify(|stored: &mut WorkloadWaitForEdgeKindWindow| stored.merge(window))
            .or_insert(window);
    }
    by_kind.into_values().collect()
}

pub(super) fn merge_wait_for_edge_kind_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    collect_wait_for_edge_kind_windows(windows)
}

fn merge_wait_for_edge_kind_windows_by_strongest(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    let mut by_kind = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_kind
            .entry(window.kind())
            .and_modify(|stored: &mut WorkloadWaitForEdgeKindWindow| {
                *stored = WorkloadWaitForEdgeKindWindow::new(
                    stored.kind(),
                    stored.edge_count().max(window.edge_count()),
                    stored.first_tick().min(window.first_tick()),
                    stored.last_tick().max(window.last_tick()),
                );
            })
            .or_insert(window);
    }
    by_kind.into_values().collect()
}

pub(super) fn wait_for_edge_kind_window(
    windows: &[WorkloadWaitForEdgeKindWindow],
    kind: WaitForEdgeKind,
) -> Option<WorkloadWaitForEdgeKindWindow> {
    windows.iter().copied().find(|window| window.kind() == kind)
}

pub(super) fn wait_for_edge_kind_window_count_sum(
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> usize {
    windows
        .iter()
        .map(WorkloadWaitForEdgeKindWindow::edge_count)
        .sum()
}

fn merge_wait_for_edge_kind_counts_from_windows(
    counts: &mut BTreeMap<WaitForEdgeKind, usize>,
    windows: &[WorkloadWaitForEdgeKindWindow],
) {
    for window in windows {
        counts
            .entry(window.kind())
            .and_modify(|count| *count = (*count).max(window.edge_count()))
            .or_insert(window.edge_count());
    }
}
