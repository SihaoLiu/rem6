use crate::{
    WorkloadError, WorkloadExpectedCleanParallelDiagnostics,
    WorkloadExpectedParallelWaitForEdgeKindCount, WorkloadExpectedParallelWaitForEdgeKindWindow,
    WorkloadExpectedParallelWaitForTargetNodeWindow, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_clean_parallel_diagnostics(
        &self,
    ) -> &[WorkloadExpectedCleanParallelDiagnostics] {
        &self.expected_clean_parallel_diagnostics
    }

    pub fn expected_parallel_wait_for_edge_kind_counts(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForEdgeKindCount] {
        &self.expected_parallel_wait_for_edge_kind_counts
    }

    pub fn expected_parallel_wait_for_edge_kind_windows(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForEdgeKindWindow] {
        &self.expected_parallel_wait_for_edge_kind_windows
    }

    pub fn expected_parallel_wait_for_target_node_windows(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForTargetNodeWindow] {
        &self.expected_parallel_wait_for_target_node_windows
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_clean_parallel_diagnostics(
        mut self,
        expected: WorkloadExpectedCleanParallelDiagnostics,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_clean_parallel_diagnostics
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedCleanParallelDiagnostics {
                scope: expected.scope(),
            });
        }
        self.expected_clean_parallel_diagnostics.push(expected);
        self.expected_clean_parallel_diagnostics
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_wait_for_edge_kind_count(
        mut self,
        expected: WorkloadExpectedParallelWaitForEdgeKindCount,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_wait_for_edge_kind_counts
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelWaitForEdgeKindCount {
                    scope: expected.scope(),
                    kind: expected.kind(),
                },
            );
        }
        self.expected_parallel_wait_for_edge_kind_counts
            .push(expected);
        self.expected_parallel_wait_for_edge_kind_counts
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_wait_for_edge_kind_window(
        mut self,
        expected: WorkloadExpectedParallelWaitForEdgeKindWindow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_wait_for_edge_kind_windows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelWaitForEdgeKindWindow {
                    scope: expected.scope(),
                    kind: expected.kind(),
                },
            );
        }
        self.expected_parallel_wait_for_edge_kind_windows
            .push(expected);
        self.expected_parallel_wait_for_edge_kind_windows
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_wait_for_target_node_window(
        mut self,
        expected: WorkloadExpectedParallelWaitForTargetNodeWindow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_wait_for_target_node_windows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelWaitForTargetNodeWindow {
                    scope: expected.scope(),
                    node: expected.node().clone(),
                },
            );
        }
        self.expected_parallel_wait_for_target_node_windows
            .push(expected);
        self.expected_parallel_wait_for_target_node_windows
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_clean_parallel_diagnostics(
        mut self,
        expected: WorkloadExpectedCleanParallelDiagnostics,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_clean_parallel_diagnostics
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedCleanParallelDiagnostics {
                scope: expected.scope(),
            });
        }
        self.expected_clean_parallel_diagnostics.push(expected);
        self.expected_clean_parallel_diagnostics
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn expected_clean_parallel_diagnostics(
        &self,
    ) -> &[WorkloadExpectedCleanParallelDiagnostics] {
        &self.expected_clean_parallel_diagnostics
    }

    pub fn add_expected_parallel_wait_for_edge_kind_count(
        mut self,
        expected: WorkloadExpectedParallelWaitForEdgeKindCount,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_wait_for_edge_kind_counts
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelWaitForEdgeKindCount {
                    scope: expected.scope(),
                    kind: expected.kind(),
                },
            );
        }
        self.expected_parallel_wait_for_edge_kind_counts
            .push(expected);
        self.expected_parallel_wait_for_edge_kind_counts
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_wait_for_edge_kind_window(
        mut self,
        expected: WorkloadExpectedParallelWaitForEdgeKindWindow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_wait_for_edge_kind_windows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelWaitForEdgeKindWindow {
                    scope: expected.scope(),
                    kind: expected.kind(),
                },
            );
        }
        self.expected_parallel_wait_for_edge_kind_windows
            .push(expected);
        self.expected_parallel_wait_for_edge_kind_windows
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_wait_for_target_node_window(
        mut self,
        expected: WorkloadExpectedParallelWaitForTargetNodeWindow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_wait_for_target_node_windows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelWaitForTargetNodeWindow {
                    scope: expected.scope(),
                    node: expected.node().clone(),
                },
            );
        }
        self.expected_parallel_wait_for_target_node_windows
            .push(expected);
        self.expected_parallel_wait_for_target_node_windows
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_wait_for_edge_kind_counts(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForEdgeKindCount] {
        &self.expected_parallel_wait_for_edge_kind_counts
    }

    pub fn expected_parallel_wait_for_edge_kind_windows(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForEdgeKindWindow] {
        &self.expected_parallel_wait_for_edge_kind_windows
    }

    pub fn expected_parallel_wait_for_target_node_windows(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForTargetNodeWindow] {
        &self.expected_parallel_wait_for_target_node_windows
    }
}
