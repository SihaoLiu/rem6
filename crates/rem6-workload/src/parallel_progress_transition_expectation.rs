use rem6_kernel::{
    LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId, WaitForNode,
};

use crate::{WorkloadError, WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WorkloadParallelProgressTransitionExpectationFailure {
    Duplicate,
    MissingSummary,
    MissingRecord,
    UnexpectedRecord,
}

impl WorkloadParallelProgressTransitionExpectationFailure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate expected",
            Self::MissingSummary => "missing summary for expected",
            Self::MissingRecord => "missing expected",
            Self::UnexpectedRecord => "unexpected",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelProgressTransitionExpectationError {
    failure: WorkloadParallelProgressTransitionExpectationFailure,
    scope: WorkloadParallelRemoteFlowScope,
    partition: PartitionId,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
}

impl WorkloadParallelProgressTransitionExpectationError {
    pub const fn new(
        failure: WorkloadParallelProgressTransitionExpectationFailure,
        scope: WorkloadParallelRemoteFlowScope,
        partition: PartitionId,
        subject: WaitForNode,
        kind: LivelockTransitionKind,
        tick: u64,
        order: u64,
    ) -> Self {
        Self {
            failure,
            scope,
            partition,
            subject,
            kind,
            tick,
            order,
        }
    }

    pub const fn failure(&self) -> WorkloadParallelProgressTransitionExpectationFailure {
        self.failure
    }

    pub const fn scope(&self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn subject(&self) -> &WaitForNode {
        &self.subject
    }

    pub const fn kind(&self) -> LivelockTransitionKind {
        self.kind
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelProgressTransition {
    scope: WorkloadParallelRemoteFlowScope,
    partition: PartitionId,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
}

impl WorkloadExpectedParallelProgressTransition {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        partition: PartitionId,
        subject: WaitForNode,
        kind: LivelockTransitionKind,
        tick: u64,
        order: u64,
    ) -> Self {
        Self {
            scope,
            partition,
            subject,
            kind,
            tick,
            order,
        }
    }

    pub const fn scope(&self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn subject(&self) -> &WaitForNode {
        &self.subject
    }

    pub const fn kind(&self) -> LivelockTransitionKind {
        self.kind
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub(crate) fn sort_key(&self) -> (u8, u32, WaitForNode, LivelockTransitionKind, u64, u64) {
        (
            self.scope.sort_rank(),
            self.partition.index(),
            self.subject.clone(),
            self.kind,
            self.tick,
            self.order,
        )
    }

    pub(crate) fn matches_record(&self, transition: &ParallelProgressTransitionRecord) -> bool {
        transition.partition() == self.partition
            && transition.subject() == &self.subject
            && transition.kind() == self.kind
            && transition.tick() == self.tick
            && transition.order() == self.order
    }

    pub(crate) fn actual_record(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<ParallelProgressTransitionRecord> {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => find_parallel_progress_transition(
                summary
                    .parallel_scheduler_progress_transitions()
                    .iter()
                    .cloned(),
                self,
            ),
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                find_parallel_progress_transition(
                    summary
                        .data_cache_parallel_scheduler_progress_transitions()
                        .iter()
                        .cloned(),
                    self,
                )
            }
            WorkloadParallelRemoteFlowScope::GpuDmaScheduler => find_parallel_progress_transition(
                summary
                    .gpu_dma_scheduler_progress_transitions()
                    .iter()
                    .cloned(),
                self,
            ),
            WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
                find_parallel_progress_transition(
                    summary
                        .accelerator_dma_scheduler_progress_transitions()
                        .iter()
                        .cloned(),
                    self,
                )
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                find_parallel_progress_transition(summary.full_system_progress_transitions(), self)
            }
        }
    }

    pub(crate) fn to_error(
        &self,
        failure: WorkloadParallelProgressTransitionExpectationFailure,
    ) -> WorkloadError {
        WorkloadError::ParallelProgressTransitionExpectation(
            WorkloadParallelProgressTransitionExpectationError::new(
                failure,
                self.scope,
                self.partition,
                self.subject.clone(),
                self.kind,
                self.tick,
                self.order,
            ),
        )
    }
}

fn find_parallel_progress_transition<I>(
    transitions: I,
    expected: &WorkloadExpectedParallelProgressTransition,
) -> Option<ParallelProgressTransitionRecord>
where
    I: IntoIterator<Item = ParallelProgressTransitionRecord>,
{
    transitions
        .into_iter()
        .find(|transition| expected.matches_record(transition))
}
