use std::collections::BTreeSet;

use rem6_kernel::{
    LivelockTransitionKind, ParallelPartitionActivity, ParallelProgressTransitionRecord,
    ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionFrontier, PartitionId,
    WaitForNode,
};

use crate::{
    result_collect::collect_conservative_partition_frontiers, WorkloadDataCacheProtocol,
    WorkloadError, WorkloadParallelBatchPartitionScope, WorkloadParallelBatchWorkerScope,
    WorkloadParallelExecutionSummary,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelRemoteFlowScope {
    Scheduler,
    DataCacheScheduler,
    FullSystem,
}

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

impl WorkloadParallelRemoteFlowScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
            Self::FullSystem => "full-system",
        }
    }

    pub(crate) const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
            Self::FullSystem => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelSchedulerScope {
    Scheduler,
    DataCacheScheduler,
    GpuDmaScheduler,
    AcceleratorDmaScheduler,
    FullSystem,
}

impl WorkloadParallelSchedulerScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
            Self::GpuDmaScheduler => "gpu-dma-scheduler",
            Self::AcceleratorDmaScheduler => "accelerator-dma-scheduler",
            Self::FullSystem => "full-system",
        }
    }

    pub(crate) const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
            Self::GpuDmaScheduler => 2,
            Self::AcceleratorDmaScheduler => 3,
            Self::FullSystem => 4,
        }
    }
}

impl From<WorkloadParallelRemoteFlowScope> for WorkloadParallelSchedulerScope {
    fn from(scope: WorkloadParallelRemoteFlowScope) -> Self {
        match scope {
            WorkloadParallelRemoteFlowScope::Scheduler => Self::Scheduler,
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => Self::DataCacheScheduler,
            WorkloadParallelRemoteFlowScope::FullSystem => Self::FullSystem,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelFrontierStage {
    Initial,
    Final,
}

impl WorkloadParallelFrontierStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initial => "initial",
            Self::Final => "final",
        }
    }

    const fn sort_rank(self) -> u8 {
        match self {
            Self::Initial => 0,
            Self::Final => 1,
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceActivityScope {
    Fabric,
    Dram,
    Resource,
}

impl WorkloadResourceActivityScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Dram => "dram",
            Self::Resource => "resource",
        }
    }

    const fn sort_rank(self) -> u8 {
        match self {
            Self::Fabric => 0,
            Self::Dram => 1,
            Self::Resource => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelFrontier {
    scope: WorkloadParallelRemoteFlowScope,
    stage: WorkloadParallelFrontierStage,
    partition: PartitionId,
    minimum_now: u64,
    minimum_safe_until: u64,
}

impl WorkloadExpectedParallelFrontier {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        stage: WorkloadParallelFrontierStage,
        partition: PartitionId,
        minimum_now: u64,
        minimum_safe_until: u64,
    ) -> Result<Self, WorkloadError> {
        if minimum_now == 0 && minimum_safe_until == 0 {
            return Err(WorkloadError::ZeroExpectedParallelFrontier {
                scope,
                stage,
                partition: partition.index(),
            });
        }
        Ok(Self {
            scope,
            stage,
            partition,
            minimum_now,
            minimum_safe_until,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn stage(self) -> WorkloadParallelFrontierStage {
        self.stage
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn minimum_now(self) -> u64 {
        self.minimum_now
    }

    pub const fn minimum_safe_until(self) -> u64 {
        self.minimum_safe_until
    }

    pub(crate) const fn sort_key(self) -> (u8, u8, u32) {
        (
            self.scope.sort_rank(),
            self.stage.sort_rank(),
            self.partition.index(),
        )
    }

    pub(crate) fn actual_frontier(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<PartitionFrontier> {
        match (self.scope, self.stage) {
            (
                WorkloadParallelRemoteFlowScope::Scheduler,
                WorkloadParallelFrontierStage::Initial,
            ) => find_frontier(
                summary
                    .parallel_scheduler_initial_frontiers()
                    .iter()
                    .copied(),
                self.partition,
            ),
            (WorkloadParallelRemoteFlowScope::Scheduler, WorkloadParallelFrontierStage::Final) => {
                find_frontier(
                    summary.parallel_scheduler_final_frontiers().iter().copied(),
                    self.partition,
                )
            }
            (
                WorkloadParallelRemoteFlowScope::DataCacheScheduler,
                WorkloadParallelFrontierStage::Initial,
            ) => find_frontier(
                summary
                    .data_cache_parallel_scheduler_initial_frontiers()
                    .iter()
                    .copied(),
                self.partition,
            ),
            (
                WorkloadParallelRemoteFlowScope::DataCacheScheduler,
                WorkloadParallelFrontierStage::Final,
            ) => find_frontier(
                summary
                    .data_cache_parallel_scheduler_final_frontiers()
                    .iter()
                    .copied(),
                self.partition,
            ),
            (
                WorkloadParallelRemoteFlowScope::FullSystem,
                WorkloadParallelFrontierStage::Initial,
            ) => find_frontier(
                summary.full_system_parallel_scheduler_initial_frontiers(),
                self.partition,
            ),
            (WorkloadParallelRemoteFlowScope::FullSystem, WorkloadParallelFrontierStage::Final) => {
                find_frontier(
                    summary.full_system_parallel_scheduler_final_frontiers(),
                    self.partition,
                )
            }
        }
    }
}

fn find_frontier<I>(frontiers: I, partition: PartitionId) -> Option<PartitionFrontier>
where
    I: IntoIterator<Item = PartitionFrontier>,
{
    collect_conservative_partition_frontiers(frontiers)
        .into_iter()
        .find(|frontier| frontier.partition() == partition)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedResourceActivity {
    scope: WorkloadResourceActivityScope,
    minimum_operation_count: usize,
    minimum_active_resource_count: usize,
}

impl WorkloadExpectedResourceActivity {
    pub fn new(
        scope: WorkloadResourceActivityScope,
        minimum_operation_count: usize,
        minimum_active_resource_count: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_operation_count == 0 && minimum_active_resource_count == 0 {
            return Err(WorkloadError::ZeroExpectedResourceActivity { scope });
        }
        Ok(Self {
            scope,
            minimum_operation_count,
            minimum_active_resource_count,
        })
    }

    pub const fn scope(self) -> WorkloadResourceActivityScope {
        self.scope
    }

    pub const fn minimum_operation_count(self) -> usize {
        self.minimum_operation_count
    }

    pub const fn minimum_active_resource_count(self) -> usize {
        self.minimum_active_resource_count
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_counts(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> (usize, usize) {
        match self.scope {
            WorkloadResourceActivityScope::Fabric => (
                summary.fabric_transfer_count(),
                summary.active_fabric_lane_count(),
            ),
            WorkloadResourceActivityScope::Dram => (
                summary.dram_access_count(),
                summary.active_dram_target_count(),
            ),
            WorkloadResourceActivityScope::Resource => (
                summary
                    .fabric_transfer_count()
                    .saturating_add(summary.dram_access_count()),
                summary
                    .active_fabric_lane_count()
                    .saturating_add(summary.active_dram_target_count()),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedCleanParallelDiagnostics {
    scope: WorkloadParallelDiagnosticScope,
    livelock_transition_threshold: Option<u64>,
}

impl WorkloadExpectedCleanParallelDiagnostics {
    pub const fn new(scope: WorkloadParallelDiagnosticScope) -> Self {
        Self {
            scope,
            livelock_transition_threshold: None,
        }
    }

    pub const fn scope(self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub fn with_livelock_transition_threshold(
        mut self,
        threshold: u64,
    ) -> Result<Self, WorkloadError> {
        if threshold == 0 {
            return Err(WorkloadError::ZeroExpectedLivelockTransitionThreshold {
                scope: self.scope,
            });
        }
        self.livelock_transition_threshold = Some(threshold);
        Ok(self)
    }

    pub const fn livelock_transition_threshold(self) -> Option<u64> {
        self.livelock_transition_threshold
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) const fn actual_counts(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> (usize, usize, usize) {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => (
                summary.resource_wait_for_edge_count(),
                summary.resource_deadlock_diagnostic_count(),
                0,
            ),
            WorkloadParallelDiagnosticScope::DataCache => (
                summary.data_cache_wait_for_edge_count(),
                summary.data_cache_deadlock_diagnostic_count(),
                summary.data_cache_parallel_scheduler_livelock_diagnostic_count(),
            ),
            WorkloadParallelDiagnosticScope::Compute => (
                summary.compute_wait_for_edge_count(),
                summary.compute_deadlock_diagnostic_count(),
                0,
            ),
            WorkloadParallelDiagnosticScope::Dma => (
                summary.dma_wait_for_edge_count(),
                summary.dma_deadlock_diagnostic_count(),
                0,
            ),
            WorkloadParallelDiagnosticScope::FullSystem => (
                summary.full_system_wait_for_edge_count(),
                summary.full_system_deadlock_diagnostic_count(),
                summary.full_system_livelock_diagnostic_count(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedDataCacheProtocolRunCount {
    protocol: WorkloadDataCacheProtocol,
    minimum_run_count: usize,
}

impl WorkloadExpectedDataCacheProtocolRunCount {
    pub fn new(
        protocol: WorkloadDataCacheProtocol,
        minimum_run_count: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_run_count == 0 {
            return Err(WorkloadError::ZeroExpectedDataCacheProtocolRunCount { protocol });
        }
        Ok(Self {
            protocol,
            minimum_run_count,
        })
    }

    pub const fn protocol(self) -> WorkloadDataCacheProtocol {
        self.protocol
    }

    pub const fn minimum_run_count(self) -> usize {
        self.minimum_run_count
    }

    pub(crate) const fn sort_key(self) -> WorkloadDataCacheProtocol {
        self.protocol
    }

    pub(crate) fn actual_run_count(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        summary.data_cache_parallel_run_count_for_protocol(self.protocol)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedDataCacheRunAttribution {
    minimum_attributed_run_count: usize,
    maximum_unattributed_run_count: usize,
}

impl WorkloadExpectedDataCacheRunAttribution {
    pub const fn new(
        minimum_attributed_run_count: usize,
        maximum_unattributed_run_count: usize,
    ) -> Self {
        Self {
            minimum_attributed_run_count,
            maximum_unattributed_run_count,
        }
    }

    pub const fn minimum_attributed_run_count(self) -> usize {
        self.minimum_attributed_run_count
    }

    pub const fn maximum_unattributed_run_count(self) -> usize {
        self.maximum_unattributed_run_count
    }

    pub(crate) fn actual_counts(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> (usize, usize) {
        (
            summary.attributed_data_cache_parallel_run_count(),
            summary.unattributed_data_cache_parallel_run_count(),
        )
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

    pub(crate) fn matches_record(self, flow: ParallelRemoteFlowRecord) -> bool {
        flow.source() == self.source
            && flow.target() == self.target
            && flow.send_count() == self.send_count
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteEndpoints {
    scope: WorkloadParallelRemoteFlowScope,
    source_partitions: Vec<PartitionId>,
    target_partitions: Vec<PartitionId>,
}

impl WorkloadExpectedParallelRemoteEndpoints {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        source_partitions: impl IntoIterator<Item = PartitionId>,
        target_partitions: impl IntoIterator<Item = PartitionId>,
    ) -> Result<Self, WorkloadError> {
        let source_partitions = normalize_parallel_endpoint_partitions(source_partitions);
        if source_partitions.is_empty() {
            return Err(WorkloadError::EmptyExpectedParallelRemoteEndpointSources { scope });
        }
        let target_partitions = normalize_parallel_endpoint_partitions(target_partitions);
        if target_partitions.is_empty() {
            return Err(WorkloadError::EmptyExpectedParallelRemoteEndpointTargets { scope });
        }
        Ok(Self {
            scope,
            source_partitions,
            target_partitions,
        })
    }

    pub const fn scope(&self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub fn source_partitions(&self) -> &[PartitionId] {
        &self.source_partitions
    }

    pub fn target_partitions(&self) -> &[PartitionId] {
        &self.target_partitions
    }

    pub(crate) fn source_partition_indexes(&self) -> Vec<u32> {
        self.source_partitions
            .iter()
            .map(|partition| partition.index())
            .collect()
    }

    pub(crate) fn target_partition_indexes(&self) -> Vec<u32> {
        self.target_partitions
            .iter()
            .map(|partition| partition.index())
            .collect()
    }

    pub(crate) const fn sort_key(&self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_source_partitions(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Vec<PartitionId> {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                summary.parallel_scheduler_remote_source_partitions()
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_remote_source_partitions()
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                summary.full_system_parallel_scheduler_remote_source_partitions()
            }
        }
    }

    pub(crate) fn actual_target_partitions(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Vec<PartitionId> {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                summary.parallel_scheduler_remote_target_partitions()
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_remote_target_partitions()
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                summary.full_system_parallel_scheduler_remote_target_partitions()
            }
        }
    }
}

fn normalize_parallel_endpoint_partitions(
    partitions: impl IntoIterator<Item = PartitionId>,
) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteDelayFloor {
    scope: WorkloadParallelRemoteFlowScope,
    minimum_delay: u64,
}

impl WorkloadExpectedParallelRemoteDelayFloor {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        minimum_delay: u64,
    ) -> Result<Self, WorkloadError> {
        if minimum_delay == 0 {
            return Err(WorkloadError::ZeroExpectedParallelRemoteDelayFloor { scope });
        }
        Ok(Self {
            scope,
            minimum_delay,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn minimum_delay(self) -> u64 {
        self.minimum_delay
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteDelayCeiling {
    scope: WorkloadParallelRemoteFlowScope,
    maximum_delay: u64,
}

impl WorkloadExpectedParallelRemoteDelayCeiling {
    pub const fn new(scope: WorkloadParallelRemoteFlowScope, maximum_delay: u64) -> Self {
        Self {
            scope,
            maximum_delay,
        }
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn maximum_delay(self) -> u64 {
        self.maximum_delay
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteTrafficConsistency {
    scope: WorkloadParallelRemoteFlowScope,
}

impl WorkloadExpectedParallelRemoteTrafficConsistency {
    pub const fn new(scope: WorkloadParallelRemoteFlowScope) -> Self {
        Self { scope }
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteSend {
    scope: WorkloadParallelRemoteFlowScope,
    source: PartitionId,
    target: PartitionId,
    source_tick: u64,
    delivery_tick: u64,
    order: u64,
}

impl WorkloadExpectedParallelRemoteSend {
    pub const fn new(
        scope: WorkloadParallelRemoteFlowScope,
        source: PartitionId,
        target: PartitionId,
        source_tick: u64,
        delivery_tick: u64,
        order: u64,
    ) -> Self {
        Self {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        }
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

    pub const fn source_tick(self) -> u64 {
        self.source_tick
    }

    pub const fn delivery_tick(self) -> u64 {
        self.delivery_tick
    }

    pub fn delay(self) -> u64 {
        self.delivery_tick.saturating_sub(self.source_tick)
    }

    pub const fn order(self) -> u64 {
        self.order
    }

    pub(crate) const fn sort_key(self) -> (u8, u32, u32, u64, u64, u64) {
        (
            self.scope.sort_rank(),
            self.source.index(),
            self.target.index(),
            self.source_tick,
            self.delivery_tick,
            self.order,
        )
    }

    pub(crate) fn matches_record(self, send: ParallelRemoteSendRecord) -> bool {
        send.source() == self.source
            && send.target() == self.target
            && send.source_tick() == self.source_tick
            && send.delivery_tick() == self.delivery_tick
            && send.order() == self.order
    }

    pub(crate) fn actual_record(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<ParallelRemoteSendRecord> {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => find_parallel_remote_send(
                summary.parallel_scheduler_remote_sends().iter().copied(),
                self,
            ),
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => find_parallel_remote_send(
                summary
                    .data_cache_parallel_scheduler_remote_sends()
                    .iter()
                    .copied(),
                self,
            ),
            WorkloadParallelRemoteFlowScope::FullSystem => find_parallel_remote_send(
                summary.full_system_parallel_scheduler_remote_sends(),
                self,
            ),
        }
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelRemoteFlowTiming {
    scope: WorkloadParallelRemoteFlowScope,
    source: PartitionId,
    target: PartitionId,
    send_count: usize,
    first_tick: u64,
    last_tick: u64,
    minimum_delay: Option<u64>,
    maximum_delay: Option<u64>,
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
        Self::from_parts(
            scope, source, target, send_count, first_tick, last_tick, None,
        )
    }

    pub fn with_delay_bounds(
        scope: WorkloadParallelRemoteFlowScope,
        source: PartitionId,
        target: PartitionId,
        send_count: usize,
        first_tick: u64,
        last_tick: u64,
        delay_bounds: (u64, u64),
    ) -> Result<Self, WorkloadError> {
        let (minimum_delay, maximum_delay) = delay_bounds;
        if minimum_delay > maximum_delay {
            return Err(
                WorkloadError::InvalidExpectedParallelRemoteFlowDelayBounds {
                    scope,
                    source: source.index(),
                    target: target.index(),
                    minimum_delay,
                    maximum_delay,
                },
            );
        }
        Self::from_parts(
            scope,
            source,
            target,
            send_count,
            first_tick,
            last_tick,
            Some(delay_bounds),
        )
    }

    fn from_parts(
        scope: WorkloadParallelRemoteFlowScope,
        source: PartitionId,
        target: PartitionId,
        send_count: usize,
        first_tick: u64,
        last_tick: u64,
        delay_bounds: Option<(u64, u64)>,
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
            minimum_delay: delay_bounds.map(|(minimum_delay, _)| minimum_delay),
            maximum_delay: delay_bounds.map(|(_, maximum_delay)| maximum_delay),
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

    pub const fn minimum_delay(self) -> Option<u64> {
        self.minimum_delay
    }

    pub const fn maximum_delay(self) -> Option<u64> {
        self.maximum_delay
    }

    pub const fn delay_bounds(self) -> Option<(u64, u64)> {
        match (self.minimum_delay, self.maximum_delay) {
            (Some(minimum_delay), Some(maximum_delay)) => Some((minimum_delay, maximum_delay)),
            _ => None,
        }
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
                summary.parallel_scheduler_remote_flow_evidence(),
                self.source,
                self.target,
            ),
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => find_parallel_remote_flow(
                summary.data_cache_parallel_scheduler_remote_flow_evidence(),
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

    pub(crate) fn matches_timing_record(self, flow: ParallelRemoteFlowRecord) -> bool {
        flow.source() == self.source
            && flow.target() == self.target
            && flow.send_count() == self.send_count
            && flow.first_tick() == self.first_tick
            && flow.last_tick() == self.last_tick
    }

    pub(crate) fn delay_bounds_mismatch(
        self,
        actual: Option<ParallelRemoteFlowRecord>,
    ) -> Option<WorkloadError> {
        let (expected_minimum_delay, expected_maximum_delay) = self.delay_bounds()?;
        let actual_minimum_delay = actual.and_then(|record| record.minimum_delay());
        let actual_maximum_delay = actual.and_then(|record| record.maximum_delay());
        if actual_minimum_delay == Some(expected_minimum_delay)
            && actual_maximum_delay == Some(expected_maximum_delay)
        {
            return None;
        }
        Some(
            WorkloadError::ExpectedParallelRemoteFlowDelayBoundsMismatch {
                scope: self.scope(),
                source: self.source().index(),
                target: self.target().index(),
                expected_minimum_delay,
                actual_minimum_delay,
                expected_maximum_delay,
                actual_maximum_delay,
            },
        )
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

fn find_parallel_remote_send<I>(
    sends: I,
    expected: WorkloadExpectedParallelRemoteSend,
) -> Option<ParallelRemoteSendRecord>
where
    I: IntoIterator<Item = ParallelRemoteSendRecord>,
{
    sends
        .into_iter()
        .find(|send| expected.matches_record(*send))
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWorkerUse {
    scope: WorkloadParallelBatchWorkerScope,
    minimum_max_workers: usize,
}

impl WorkloadExpectedParallelWorkerUse {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_max_workers: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_max_workers == 0 {
            return Err(WorkloadError::ZeroExpectedParallelWorkerCount { scope });
        }
        Ok(Self {
            scope,
            minimum_max_workers,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn minimum_max_workers(self) -> usize {
        self.minimum_max_workers
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_max_workers(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => summary.max_parallel_scheduler_workers(),
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_max_workers()
            }
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_max_workers()
            }
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_max_workers()
            }
            WorkloadParallelBatchWorkerScope::FullSystem => {
                summary.full_system_parallel_scheduler_max_workers()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWorkerActivity {
    scope: WorkloadParallelBatchWorkerScope,
    minimum_total_workers: usize,
}

impl WorkloadExpectedParallelWorkerActivity {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_total_workers: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_total_workers == 0 {
            return Err(WorkloadError::ZeroExpectedParallelWorkerActivity { scope });
        }
        Ok(Self {
            scope,
            minimum_total_workers,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn minimum_total_workers(self) -> usize {
        self.minimum_total_workers
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_total_workers(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => {
                summary.total_parallel_scheduler_workers()
            }
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_total_workers()
            }
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_total_workers()
            }
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_total_workers()
            }
            WorkloadParallelBatchWorkerScope::FullSystem => {
                summary.full_system_parallel_scheduler_total_workers()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelSchedulerProgress {
    scope: WorkloadParallelSchedulerScope,
    minimum_epoch_count: usize,
    minimum_dispatch_count: usize,
}

impl WorkloadExpectedParallelSchedulerProgress {
    pub fn new(
        scope: impl Into<WorkloadParallelSchedulerScope>,
        minimum_epoch_count: usize,
        minimum_dispatch_count: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_epoch_count == 0 && minimum_dispatch_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelSchedulerProgress { scope });
        }
        Ok(Self {
            scope,
            minimum_epoch_count,
            minimum_dispatch_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelSchedulerScope {
        self.scope
    }

    pub const fn minimum_epoch_count(self) -> usize {
        self.minimum_epoch_count
    }

    pub const fn minimum_dispatch_count(self) -> usize {
        self.minimum_dispatch_count
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_counts(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> (usize, usize) {
        match self.scope {
            WorkloadParallelSchedulerScope::Scheduler => (
                summary.scheduler_epoch_count(),
                summary.scheduler_dispatch_count(),
            ),
            WorkloadParallelSchedulerScope::DataCacheScheduler => (
                summary.data_cache_parallel_scheduler_epoch_count(),
                summary.data_cache_parallel_scheduler_dispatch_count(),
            ),
            WorkloadParallelSchedulerScope::GpuDmaScheduler => (
                summary.gpu_dma_scheduler_epoch_count(),
                summary.gpu_dma_scheduler_dispatch_count(),
            ),
            WorkloadParallelSchedulerScope::AcceleratorDmaScheduler => (
                summary.accelerator_dma_scheduler_epoch_count(),
                summary.accelerator_dma_scheduler_dispatch_count(),
            ),
            WorkloadParallelSchedulerScope::FullSystem => (
                summary.full_system_parallel_scheduler_epoch_count(),
                summary.full_system_parallel_scheduler_dispatch_count(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelSchedulerIdleBound {
    scope: WorkloadParallelRemoteFlowScope,
    maximum_empty_epoch_count: usize,
}

impl WorkloadExpectedParallelSchedulerIdleBound {
    pub const fn new(
        scope: WorkloadParallelRemoteFlowScope,
        maximum_empty_epoch_count: usize,
    ) -> Self {
        Self {
            scope,
            maximum_empty_epoch_count,
        }
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn maximum_empty_epoch_count(self) -> usize {
        self.maximum_empty_epoch_count
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) const fn actual_empty_epoch_count(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> usize {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => summary.scheduler_empty_epoch_count(),
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_empty_epoch_count()
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                summary.full_system_parallel_scheduler_empty_epoch_count()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchActivity {
    scope: WorkloadParallelRemoteFlowScope,
    minimum_worker_count: usize,
    minimum_batch_count: usize,
}

impl WorkloadExpectedParallelBatchActivity {
    pub fn new(
        scope: WorkloadParallelRemoteFlowScope,
        minimum_worker_count: usize,
        minimum_batch_count: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_worker_count < 2 {
            return Err(WorkloadError::InvalidExpectedParallelBatchWorkerCount {
                scope,
                minimum_worker_count,
            });
        }
        if minimum_batch_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchCount {
                scope,
                minimum_worker_count,
            });
        }
        Ok(Self {
            scope,
            minimum_worker_count,
            minimum_batch_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelRemoteFlowScope {
        self.scope
    }

    pub const fn minimum_worker_count(self) -> usize {
        self.minimum_worker_count
    }

    pub const fn minimum_batch_count(self) -> usize {
        self.minimum_batch_count
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.minimum_worker_count)
    }

    pub(crate) fn actual_batch_count(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                summary.parallel_scheduler_batch_count_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_count_at_or_above(self.minimum_worker_count),
            WorkloadParallelRemoteFlowScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_count_at_or_above(self.minimum_worker_count),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelPartitionActivity {
    scope: WorkloadParallelBatchPartitionScope,
    partition: PartitionId,
    minimum_worker_count: usize,
    minimum_dispatch_count: usize,
    minimum_remote_send_count: usize,
    minimum_remote_receive_count: usize,
}

impl WorkloadExpectedParallelPartitionActivity {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchPartitionScope>,
        partition: PartitionId,
        minimum_worker_count: usize,
        minimum_dispatch_count: usize,
        minimum_remote_send_count: usize,
        minimum_remote_receive_count: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_worker_count == 0
            && minimum_dispatch_count == 0
            && minimum_remote_send_count == 0
            && minimum_remote_receive_count == 0
        {
            return Err(WorkloadError::ZeroExpectedParallelPartitionActivity {
                scope,
                partition: partition.index(),
            });
        }
        Ok(Self {
            scope,
            partition,
            minimum_worker_count,
            minimum_dispatch_count,
            minimum_remote_send_count,
            minimum_remote_receive_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchPartitionScope {
        self.scope
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn minimum_worker_count(self) -> usize {
        self.minimum_worker_count
    }

    pub const fn minimum_dispatch_count(self) -> usize {
        self.minimum_dispatch_count
    }

    pub const fn minimum_remote_send_count(self) -> usize {
        self.minimum_remote_send_count
    }

    pub const fn minimum_remote_receive_count(self) -> usize {
        self.minimum_remote_receive_count
    }

    pub(crate) const fn sort_key(self) -> (u8, u32) {
        (self.scope.sort_rank(), self.partition.index())
    }

    pub(crate) fn actual_activity(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<ParallelPartitionActivity> {
        match self.scope {
            WorkloadParallelBatchPartitionScope::Scheduler => {
                summary.parallel_scheduler_partition_activity(self.partition)
            }
            WorkloadParallelBatchPartitionScope::DataCacheScheduler => {
                summary.data_cache_parallel_scheduler_partition_activity(self.partition)
            }
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_partition_activity(self.partition)
            }
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_partition_activity(self.partition)
            }
            WorkloadParallelBatchPartitionScope::FullSystem => {
                summary.full_system_parallel_scheduler_partition_activity(self.partition)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelPartitionUse {
    scope: WorkloadParallelBatchPartitionScope,
    minimum_active_partitions: usize,
}

impl WorkloadExpectedParallelPartitionUse {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchPartitionScope>,
        minimum_active_partitions: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_active_partitions == 0 {
            return Err(WorkloadError::ZeroExpectedParallelPartitionCount { scope });
        }
        Ok(Self {
            scope,
            minimum_active_partitions,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchPartitionScope {
        self.scope
    }

    pub const fn minimum_active_partitions(self) -> usize {
        self.minimum_active_partitions
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_active_partitions(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> usize {
        match self.scope {
            WorkloadParallelBatchPartitionScope::Scheduler => {
                summary.active_scheduler_partition_count()
            }
            WorkloadParallelBatchPartitionScope::DataCacheScheduler => {
                summary.active_data_cache_parallel_scheduler_partition_count()
            }
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler => {
                summary.active_gpu_dma_scheduler_partition_count()
            }
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler => {
                summary.active_accelerator_dma_scheduler_partition_count()
            }
            WorkloadParallelBatchPartitionScope::FullSystem => {
                summary.active_full_system_parallel_scheduler_partition_count()
            }
        }
    }
}
