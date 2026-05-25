use crate::{
    WorkloadError, WorkloadManifest, WorkloadManifestBuilder, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope, WorkloadReplayPlan,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchWorkerBucket {
    scope: WorkloadParallelRemoteFlowScope,
    worker_count: usize,
    minimum_batch_count: usize,
}

impl WorkloadExpectedParallelBatchWorkerBucket {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        worker_count: usize,
        minimum_batch_count: usize,
    ) -> Result<Self, WorkloadError> {
        if worker_count < 2 {
            return Err(WorkloadError::InvalidExpectedParallelBatchWorkerBucket {
                scope,
                worker_count,
            });
        }
        if minimum_batch_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerBucket {
                scope,
                worker_count,
            });
        }
        Ok(Self {
            scope,
            worker_count,
            minimum_batch_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    pub const fn minimum_batch_count(self) -> usize {
        self.minimum_batch_count
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.worker_count)
    }

    pub(crate) fn actual_batch_count(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                summary.parallel_scheduler_batch_count_for_worker_count(self.worker_count)
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_count_for_worker_count(self.worker_count),
            WorkloadParallelRemoteFlowScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_count_for_worker_count(self.worker_count),
        }
    }
}

impl WorkloadManifest {
    pub fn expected_parallel_batch_worker_buckets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerBucket] {
        &self.expected_parallel_batch_worker_buckets
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_parallel_batch_worker_bucket(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerBucket,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_buckets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchWorkerBucket {
                scope: expected.scope(),
                worker_count: expected.worker_count(),
            });
        }
        self.expected_parallel_batch_worker_buckets.push(expected);
        self.expected_parallel_batch_worker_buckets
            .sort_by_key(|bucket| bucket.sort_key());
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_parallel_batch_worker_bucket(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerBucket,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_buckets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchWorkerBucket {
                scope: expected.scope(),
                worker_count: expected.worker_count(),
            });
        }
        self.expected_parallel_batch_worker_buckets.push(expected);
        self.expected_parallel_batch_worker_buckets
            .sort_by_key(|bucket| bucket.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_worker_buckets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerBucket] {
        &self.expected_parallel_batch_worker_buckets
    }
}
