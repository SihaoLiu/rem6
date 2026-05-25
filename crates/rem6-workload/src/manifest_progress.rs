use crate::{
    WorkloadError, WorkloadExpectedParallelProgressTransition, WorkloadManifest,
    WorkloadManifestBuilder, WorkloadParallelProgressTransitionExpectationFailure,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_parallel_progress_transitions(
        &self,
    ) -> &[WorkloadExpectedParallelProgressTransition] {
        &self.expected_parallel_progress_transitions
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_parallel_progress_transition(
        mut self,
        expected: WorkloadExpectedParallelProgressTransition,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_progress_transitions
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                expected.to_error(WorkloadParallelProgressTransitionExpectationFailure::Duplicate)
            );
        }
        self.expected_parallel_progress_transitions.push(expected);
        self.expected_parallel_progress_transitions
            .sort_by_key(WorkloadExpectedParallelProgressTransition::sort_key);
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_parallel_progress_transition(
        mut self,
        expected: WorkloadExpectedParallelProgressTransition,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_progress_transitions
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                expected.to_error(WorkloadParallelProgressTransitionExpectationFailure::Duplicate)
            );
        }
        self.expected_parallel_progress_transitions.push(expected);
        self.expected_parallel_progress_transitions
            .sort_by_key(WorkloadExpectedParallelProgressTransition::sort_key);
        Ok(self)
    }

    pub fn expected_parallel_progress_transitions(
        &self,
    ) -> &[WorkloadExpectedParallelProgressTransition] {
        &self.expected_parallel_progress_transitions
    }
}
