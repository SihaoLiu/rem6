use rem6_kernel::{ParallelRemoteFlowRecord, PartitionId};

use crate::{WorkloadError, WorkloadParallelExecutionSummary};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelRemoteFlowScope {
    Scheduler,
    DataCacheScheduler,
    FullSystem,
}

impl WorkloadParallelRemoteFlowScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
            Self::FullSystem => "full-system",
        }
    }

    const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
            Self::FullSystem => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelDiagnosticScope {
    Resource,
    DataCache,
    Compute,
    Dma,
    FullSystem,
}

impl WorkloadParallelDiagnosticScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resource => "resource",
            Self::DataCache => "data-cache",
            Self::Compute => "compute",
            Self::Dma => "dma",
            Self::FullSystem => "full-system",
        }
    }

    const fn sort_rank(self) -> u8 {
        match self {
            Self::Resource => 0,
            Self::DataCache => 1,
            Self::Compute => 2,
            Self::Dma => 3,
            Self::FullSystem => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedCleanParallelDiagnostics {
    scope: WorkloadParallelDiagnosticScope,
}

impl WorkloadExpectedCleanParallelDiagnostics {
    pub const fn new(scope: WorkloadParallelDiagnosticScope) -> Self {
        Self { scope }
    }

    pub const fn scope(self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) const fn actual_counts(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> (usize, usize) {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => (
                summary.resource_wait_for_edge_count(),
                summary.resource_deadlock_diagnostic_count(),
            ),
            WorkloadParallelDiagnosticScope::DataCache => (
                summary.data_cache_wait_for_edge_count(),
                summary.data_cache_deadlock_diagnostic_count(),
            ),
            WorkloadParallelDiagnosticScope::Compute => (
                summary.compute_wait_for_edge_count(),
                summary.compute_deadlock_diagnostic_count(),
            ),
            WorkloadParallelDiagnosticScope::Dma => (
                summary.dma_wait_for_edge_count(),
                summary.dma_deadlock_diagnostic_count(),
            ),
            WorkloadParallelDiagnosticScope::FullSystem => (
                summary.full_system_wait_for_edge_count(),
                summary.full_system_deadlock_diagnostic_count(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteFlow {
    scope: WorkloadParallelRemoteFlowScope,
    source: PartitionId,
    target: PartitionId,
    send_count: usize,
}

impl WorkloadExpectedParallelRemoteFlow {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        source: PartitionId,
        target: PartitionId,
        send_count: usize,
    ) -> Result<Self, WorkloadError> {
        if send_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelRemoteFlowCount {
                scope,
                source: source.index(),
                target: target.index(),
            });
        }
        Ok(Self {
            scope,
            source,
            target,
            send_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn source(self) -> PartitionId {
        self.source
    }

    pub const fn target(self) -> PartitionId {
        self.target
    }

    pub const fn send_count(self) -> usize {
        self.send_count
    }

    pub(crate) const fn sort_key(self) -> (u8, u32, u32) {
        (
            self.scope.sort_rank(),
            self.source.index(),
            self.target.index(),
        )
    }

    pub(crate) fn actual_send_count(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                summary.parallel_scheduler_remote_flow_count(self.source, self.target)
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_remote_flow_count(self.source, self.target)
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                summary.full_system_parallel_scheduler_remote_flow_count(self.source, self.target)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteFlowTiming {
    scope: WorkloadParallelRemoteFlowScope,
    source: PartitionId,
    target: PartitionId,
    send_count: usize,
    first_tick: u64,
    last_tick: u64,
}

impl WorkloadExpectedParallelRemoteFlowTiming {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        source: PartitionId,
        target: PartitionId,
        send_count: usize,
        first_tick: u64,
        last_tick: u64,
    ) -> Result<Self, WorkloadError> {
        if send_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelRemoteFlowCount {
                scope,
                source: source.index(),
                target: target.index(),
            });
        }
        if first_tick > last_tick {
            return Err(
                WorkloadError::InvalidExpectedParallelRemoteFlowTimingWindow {
                    scope,
                    source: source.index(),
                    target: target.index(),
                    first_tick,
                    last_tick,
                },
            );
        }
        Ok(Self {
            scope,
            source,
            target,
            send_count,
            first_tick,
            last_tick,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn source(self) -> PartitionId {
        self.source
    }

    pub const fn target(self) -> PartitionId {
        self.target
    }

    pub const fn send_count(self) -> usize {
        self.send_count
    }

    pub const fn first_tick(self) -> u64 {
        self.first_tick
    }

    pub const fn last_tick(self) -> u64 {
        self.last_tick
    }

    pub(crate) const fn sort_key(self) -> (u8, u32, u32) {
        (
            self.scope.sort_rank(),
            self.source.index(),
            self.target.index(),
        )
    }

    pub(crate) fn actual_record(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<ParallelRemoteFlowRecord> {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => find_parallel_remote_flow(
                summary.parallel_scheduler_remote_flows().iter().copied(),
                self.source,
                self.target,
            ),
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => find_parallel_remote_flow(
                summary
                    .data_cache_parallel_scheduler_remote_flows()
                    .iter()
                    .copied(),
                self.source,
                self.target,
            ),
            WorkloadParallelRemoteFlowScope::FullSystem => find_parallel_remote_flow(
                summary.full_system_parallel_scheduler_remote_flows(),
                self.source,
                self.target,
            ),
        }
    }
}

fn find_parallel_remote_flow<I>(
    flows: I,
    source: PartitionId,
    target: PartitionId,
) -> Option<ParallelRemoteFlowRecord>
where
    I: IntoIterator<Item = ParallelRemoteFlowRecord>,
{
    flows
        .into_iter()
        .find(|flow| flow.source() == source && flow.target() == target)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWorkerUse {
    scope: WorkloadParallelRemoteFlowScope,
    minimum_max_workers: usize,
}

impl WorkloadExpectedParallelWorkerUse {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        minimum_max_workers: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_max_workers == 0 {
            return Err(WorkloadError::ZeroExpectedParallelWorkerCount { scope });
        }
        Ok(Self {
            scope,
            minimum_max_workers,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn minimum_max_workers(self) -> usize {
        self.minimum_max_workers
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) const fn actual_max_workers(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> usize {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => summary.max_parallel_scheduler_workers(),
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_max_workers()
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                summary.full_system_parallel_scheduler_max_workers()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelPartitionUse {
    scope: WorkloadParallelRemoteFlowScope,
    minimum_active_partitions: usize,
}

impl WorkloadExpectedParallelPartitionUse {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        minimum_active_partitions: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_active_partitions == 0 {
            return Err(WorkloadError::ZeroExpectedParallelPartitionCount { scope });
        }
        Ok(Self {
            scope,
            minimum_active_partitions,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn minimum_active_partitions(self) -> usize {
        self.minimum_active_partitions
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) const fn actual_active_partitions(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> usize {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                summary.active_scheduler_partition_count()
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                summary.active_data_cache_parallel_scheduler_partition_count()
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                summary.active_full_system_parallel_scheduler_partition_count()
            }
        }
    }
}
