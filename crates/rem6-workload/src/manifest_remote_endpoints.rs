use crate::{
    WorkloadError, WorkloadExpectedParallelRemoteEndpoints, WorkloadManifest,
    WorkloadManifestBuilder, WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_parallel_remote_endpoints(&self) -> &[WorkloadExpectedParallelRemoteEndpoints] {
        &self.expected_parallel_remote_endpoints
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_parallel_remote_endpoints(
        mut self,
        expected: WorkloadExpectedParallelRemoteEndpoints,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_endpoints
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteEndpoints {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_remote_endpoints.push(expected);
        self.expected_parallel_remote_endpoints
            .sort_by_key(WorkloadExpectedParallelRemoteEndpoints::sort_key);
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_parallel_remote_endpoints(
        mut self,
        expected: WorkloadExpectedParallelRemoteEndpoints,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_endpoints
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteEndpoints {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_remote_endpoints.push(expected);
        self.expected_parallel_remote_endpoints
            .sort_by_key(WorkloadExpectedParallelRemoteEndpoints::sort_key);
        Ok(self)
    }

    pub fn expected_parallel_remote_endpoints(&self) -> &[WorkloadExpectedParallelRemoteEndpoints] {
        &self.expected_parallel_remote_endpoints
    }
}
