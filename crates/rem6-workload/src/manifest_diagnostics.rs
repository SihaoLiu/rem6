use crate::{
    WorkloadError, WorkloadExpectedCleanParallelDiagnostics,
    WorkloadExpectedParallelWaitForEdgeKindCount, WorkloadManifest, WorkloadManifestBuilder,
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

    pub fn expected_parallel_wait_for_edge_kind_counts(
        &self,
    ) -> &[WorkloadExpectedParallelWaitForEdgeKindCount] {
        &self.expected_parallel_wait_for_edge_kind_counts
    }
}
