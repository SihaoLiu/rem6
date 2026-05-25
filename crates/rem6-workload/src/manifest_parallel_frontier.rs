use crate::{
    WorkloadError, WorkloadExpectedParallelFrontier, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_parallel_frontiers(&self) -> &[WorkloadExpectedParallelFrontier] {
        &self.expected_parallel_frontiers
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_parallel_frontier(
        mut self,
        expected: WorkloadExpectedParallelFrontier,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_frontiers
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelFrontier {
                scope: expected.scope(),
                stage: expected.stage(),
                partition: expected.partition().index(),
            });
        }
        self.expected_parallel_frontiers.push(expected);
        self.expected_parallel_frontiers
            .sort_by_key(|frontier| frontier.sort_key());
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_parallel_frontier(
        mut self,
        expected: WorkloadExpectedParallelFrontier,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_frontiers
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelFrontier {
                scope: expected.scope(),
                stage: expected.stage(),
                partition: expected.partition().index(),
            });
        }
        self.expected_parallel_frontiers.push(expected);
        self.expected_parallel_frontiers
            .sort_by_key(|frontier| frontier.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_frontiers(&self) -> &[WorkloadExpectedParallelFrontier] {
        &self.expected_parallel_frontiers
    }
}
