use rem6_boot::BootError;
use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::Tick;
use rem6_memory::MemoryError;

use crate::{
    WorkloadDataCacheProtocol, WorkloadExecutionMode, WorkloadId, WorkloadManifestIdentity,
    WorkloadParallelBatchScope, WorkloadParallelDiagnosticScope, WorkloadParallelFrontierStage,
    WorkloadParallelProgressTransitionExpectationError, WorkloadParallelRemoteFlowScope,
    WorkloadResourceActivityScope, WorkloadResourceId, WorkloadResourceKind, WorkloadRouteId,
    WorkloadRouteLatency, WorkloadSuiteIdentity,
};

mod display;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelRemoteTrafficConsistencyMismatch {
    pub scope: WorkloadParallelRemoteFlowScope,
    pub source: u32,
    pub target: u32,
    pub flow_send_count: usize,
    pub send_record_count: usize,
    pub flow_first_tick: Tick,
    pub send_first_tick: Option<Tick>,
    pub flow_last_tick: Tick,
    pub send_last_tick: Option<Tick>,
    pub flow_minimum_delay: Option<Tick>,
    pub send_minimum_delay: Option<Tick>,
    pub flow_maximum_delay: Option<Tick>,
    pub send_maximum_delay: Option<Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadError {
    Boot(BootError),
    Memory(MemoryError),
    EmptyWorkloadId,
    EmptyWorkloadSuiteId,
    ZeroWorkloadSuiteWorkers,
    ZeroSuiteParallelismRequirement,
    EmptyResourceId,
    EmptyRouteId,
    EmptyEndpoint,
    EmptyResourceDigest {
        resource: WorkloadResourceId,
    },
    EmptyResourceLocator {
        resource: WorkloadResourceId,
    },
    DuplicateResource {
        resource: WorkloadResourceId,
    },
    MissingRequiredResource {
        resource: WorkloadResourceId,
    },
    DuplicateResourcePayload {
        resource: WorkloadResourceId,
    },
    MissingResourcePayload {
        resource: WorkloadResourceId,
    },
    UnexpectedResourcePayload {
        resource: WorkloadResourceId,
    },
    ResourcePayloadDigestMismatch {
        resource: WorkloadResourceId,
        expected: String,
        actual: String,
    },
    ResourcePayloadSizeMismatch {
        resource: WorkloadResourceId,
        expected_bytes: usize,
        actual_bytes: usize,
    },
    ResourceKindMismatch {
        resource: WorkloadResourceId,
        expected: WorkloadResourceKind,
        actual: WorkloadResourceKind,
    },
    ZeroHostLatency,
    ZeroLineBytes {
        target: u32,
    },
    MemoryProfileTargetMismatch {
        target: u32,
        profile_target: u32,
    },
    MemoryProfileLineSizeMismatch {
        target: u32,
        line_bytes: u64,
        profile_line_bytes: u64,
    },
    MemoryProfileGeometryLineSizeMismatch {
        target: u32,
        layout_line_bytes: u64,
        geometry_line_bytes: u64,
    },
    ZeroRouteLatency {
        route: WorkloadRouteId,
        latency: WorkloadRouteLatency,
    },
    EmptyMemoryRoutePath {
        route: WorkloadRouteId,
    },
    ZeroRouteHopLatency {
        endpoint: String,
        latency: WorkloadRouteLatency,
    },
    EmptyFabricLink,
    ZeroFabricBandwidth {
        link: String,
    },
    ZeroFabricCreditDepth {
        link: String,
    },
    ZeroTopologyPartitions,
    ZeroMinRemoteDelay,
    ZeroParallelWorkerLimit,
    PartitionOutOfRange {
        partition: u32,
        partition_count: u32,
    },
    DuplicateMemoryTarget {
        target: u32,
    },
    MissingMemoryTarget {
        target: u32,
    },
    DuplicateRoute {
        route: WorkloadRouteId,
    },
    DuplicateRiscvCore {
        cpu: u32,
    },
    MissingCoreFetchRoute {
        cpu: u32,
        route: WorkloadRouteId,
    },
    CoreFetchRouteSourceMismatch {
        cpu: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    CoreFetchRouteEndpointMismatch {
        cpu: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    MissingCoreDataRoute {
        cpu: u32,
        route: WorkloadRouteId,
    },
    CoreDataRouteSourceMismatch {
        cpu: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    CoreDataRouteEndpointMismatch {
        cpu: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    MissingDataCacheBackingRoute {
        route: WorkloadRouteId,
    },
    DataCacheBackingRouteSourceMismatch {
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    DataCacheBackingRouteEndpointMismatch {
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    ZeroGpuComputeUnits {
        device: u32,
    },
    ZeroGpuWaveSlots {
        device: u32,
    },
    DuplicateGpuDevice {
        device: u32,
    },
    MissingGpuCommandRoute {
        device: u32,
        route: WorkloadRouteId,
    },
    GpuCommandRouteTargetMismatch {
        device: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    GpuCommandRouteEndpointMismatch {
        device: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    MissingGpuDevice {
        device: u32,
    },
    ZeroGpuKernelWorkgroups {
        device: u32,
        kernel: u64,
    },
    ZeroGpuKernelLatency {
        device: u32,
        kernel: u64,
    },
    ZeroGpuDmaBytes {
        device: u32,
        transfer: u64,
    },
    MissingGpuDmaRoute {
        device: u32,
        route: WorkloadRouteId,
    },
    GpuDmaRouteSourceMismatch {
        device: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    GpuDmaRouteEndpointMismatch {
        device: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    ZeroAcceleratorLanes {
        engine: u32,
    },
    DuplicateAcceleratorDevice {
        engine: u32,
    },
    MissingAcceleratorCommandRoute {
        engine: u32,
        route: WorkloadRouteId,
    },
    AcceleratorCommandRouteTargetMismatch {
        engine: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    AcceleratorCommandRouteEndpointMismatch {
        engine: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    MissingAcceleratorDevice {
        engine: u32,
    },
    ZeroAcceleratorExecutionLatency {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorGpuWorkgroups {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorNpuTiles {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorDmaBytes {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorDmaCopyBytes {
        engine: u32,
        transfer: u64,
    },
    MissingAcceleratorDmaRoute {
        engine: u32,
        route: WorkloadRouteId,
    },
    AcceleratorDmaRouteSourceMismatch {
        engine: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    AcceleratorDmaRouteEndpointMismatch {
        engine: u32,
        route: WorkloadRouteId,
        expected: String,
        actual: String,
    },
    ZeroQosPriorityLevels,
    QosPriorityOutOfRange {
        priority: QosPriority,
        priority_levels: u8,
    },
    DuplicateQosRequestorPriority {
        requestor: QosRequestorId,
    },
    ManifestIdentityMismatch {
        expected: WorkloadManifestIdentity,
        actual: WorkloadManifestIdentity,
    },
    WorkloadSuiteIdentityMismatch {
        expected: WorkloadSuiteIdentity,
        actual: WorkloadSuiteIdentity,
    },
    DuplicateSuiteWorkload {
        workload: WorkloadId,
    },
    DuplicateSuiteManifest {
        manifest: WorkloadManifestIdentity,
    },
    DuplicateSuiteWorkloadResult {
        workload: WorkloadId,
    },
    DuplicateSuiteDispatchCompletion {
        workload: WorkloadId,
    },
    DuplicateSuiteDispatchWeight {
        workload: WorkloadId,
    },
    MissingSuiteWorkloadResult {
        workload: WorkloadId,
    },
    MissingSuiteDispatchCompletion {
        workload: WorkloadId,
    },
    MissingSuiteDispatchEstimate {
        workload: WorkloadId,
    },
    MissingSuiteDispatchWeight {
        workload: WorkloadId,
    },
    UnexpectedSuiteWorkloadResult {
        workload: WorkloadId,
    },
    UnexpectedSuiteDispatchCompletion {
        workload: WorkloadId,
    },
    UnexpectedSuiteDispatchWeight {
        workload: WorkloadId,
    },
    ZeroSuiteDispatchWeight {
        workload: WorkloadId,
    },
    SuiteWorkloadResultManifestMismatch {
        workload: WorkloadId,
        expected: WorkloadManifestIdentity,
        actual: WorkloadManifestIdentity,
    },
    SuiteDispatchOrderMismatch {
        workload: WorkloadId,
        expected: usize,
        actual: usize,
    },
    SuiteDispatchWorkerMismatch {
        workload: WorkloadId,
        expected: usize,
        actual: usize,
    },
    SuiteDispatchWorkerCountMismatch {
        expected: usize,
        actual: usize,
    },
    SuiteDispatchTimelineWindowMismatch {
        workload: WorkloadId,
        expected_start_tick: Tick,
        expected_final_tick: Tick,
        actual_start_tick: Tick,
        actual_final_tick: Tick,
    },
    SuiteDispatchCompletionWindowInvalid {
        workload: WorkloadId,
        start_tick: Tick,
        final_tick: Tick,
    },
    SuiteParallelismBelowMinimum {
        minimum_workers: usize,
        actual_workers: usize,
    },
    SuiteParallelismRequirementExceedsActiveWorkers {
        minimum_workers: usize,
        active_workers: usize,
    },
    SuiteExecutionWorkerCountBelowActiveWorkers {
        worker_count: usize,
        active_workers: usize,
    },
    SuiteExecutionCapacityBelowCompletionTicks {
        worker_capacity_ticks: Tick,
        serial_completion_ticks: Tick,
    },
    SuiteParallelSpeedupBelowMinimum {
        minimum_numerator: Tick,
        minimum_denominator: Tick,
        actual_numerator: Tick,
        actual_denominator: Tick,
    },
    SuiteWorkerUtilizationBelowMinimum {
        minimum_numerator: Tick,
        minimum_denominator: Tick,
        actual_numerator: Tick,
        actual_denominator: Tick,
    },
    SuitePlannedParallelSpeedupBelowMinimum {
        minimum_numerator: Tick,
        minimum_denominator: Tick,
        actual_numerator: Tick,
        actual_denominator: Tick,
    },
    SuitePlannedWorkerUtilizationBelowMinimum {
        minimum_numerator: Tick,
        minimum_denominator: Tick,
        actual_numerator: Tick,
        actual_denominator: Tick,
    },
    SuitePlannedFullOccupancyTicksBelowMinimum {
        minimum_ticks: Tick,
        actual_ticks: Tick,
    },
    SuitePlannedOccupancyWorkerCountTicksBelowMinimum {
        worker_count: usize,
        minimum_ticks: Tick,
        actual_ticks: Tick,
    },
    SuitePlannedUnderoccupiedTicksAboveMaximum {
        maximum_ticks: Tick,
        actual_ticks: Tick,
    },
    ZeroSuiteExecutionRatioDenominator,
    StatsAfterFinalTick {
        stats_tick: Tick,
        final_tick: Tick,
    },
    ResultStartAfterFinalTick {
        start_tick: Tick,
        final_tick: Tick,
    },
    PlannedHostEventAfterFinalTick {
        event_tick: Tick,
        final_tick: Tick,
    },
    MissingCheckpointLabel {
        label: String,
    },
    UnexpectedCheckpointLabel {
        label: String,
    },
    MissingCheckpointRestoreLabel {
        label: String,
    },
    UnexpectedCheckpointRestoreLabel {
        label: String,
    },
    MissingExecutionModeSwitch {
        tick: Tick,
        target: String,
        mode: WorkloadExecutionMode,
    },
    UnexpectedExecutionModeSwitch {
        tick: Tick,
        target: String,
        mode: WorkloadExecutionMode,
    },
    StopReasonMismatch {
        expected: String,
        actual: Option<String>,
    },
    UnexpectedStopReason {
        actual: String,
    },
    ZeroExpectedParallelRemoteFlowCount {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
    },
    DuplicateExpectedParallelRemoteFlow {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
    },
    EmptyExpectedParallelRemoteEndpointSources {
        scope: WorkloadParallelRemoteFlowScope,
    },
    EmptyExpectedParallelRemoteEndpointTargets {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelRemoteEndpoints {
        scope: WorkloadParallelRemoteFlowScope,
    },
    MissingParallelRemoteEndpointSummary {
        scope: WorkloadParallelRemoteFlowScope,
        expected_sources: Vec<u32>,
        expected_targets: Vec<u32>,
    },
    ExpectedParallelRemoteEndpointsMismatch {
        scope: WorkloadParallelRemoteFlowScope,
        expected_sources: Vec<u32>,
        actual_sources: Vec<u32>,
        expected_targets: Vec<u32>,
        actual_targets: Vec<u32>,
    },
    DuplicateExpectedParallelRemoteSend {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        source_tick: Tick,
        delivery_tick: Tick,
        order: u64,
    },
    MissingParallelExecutionSummary {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        expected_send_count: usize,
    },
    ExpectedParallelRemoteFlowCountMismatch {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        expected_send_count: usize,
        actual_send_count: usize,
    },
    UnexpectedParallelRemoteFlow {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        actual_send_count: usize,
    },
    MissingParallelRemoteSendSummary {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        source_tick: Tick,
        delivery_tick: Tick,
        order: u64,
    },
    ExpectedParallelRemoteSendMissing {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        source_tick: Tick,
        delivery_tick: Tick,
        order: u64,
    },
    UnexpectedParallelRemoteSend {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        source_tick: Tick,
        delivery_tick: Tick,
        order: u64,
    },
    ParallelProgressTransitionExpectation(WorkloadParallelProgressTransitionExpectationError),
    ZeroExpectedParallelRemoteDelayFloor {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelRemoteDelayFloor {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelRemoteDelayCeiling {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelRemoteTrafficConsistency {
        scope: WorkloadParallelRemoteFlowScope,
    },
    MissingParallelRemoteDelayFloorSummary {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_delay: Tick,
    },
    MissingParallelRemoteDelayEvidence {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_delay: Tick,
    },
    MissingParallelRemoteFlowDelayEvidence {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        minimum_delay: Tick,
    },
    MissingParallelRemoteDelayCeilingSummary {
        scope: WorkloadParallelRemoteFlowScope,
        maximum_delay: Tick,
    },
    MissingParallelRemoteDelayCeilingEvidence {
        scope: WorkloadParallelRemoteFlowScope,
        maximum_delay: Tick,
    },
    MissingParallelRemoteFlowMaximumDelayEvidence {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        maximum_delay: Tick,
    },
    ExpectedParallelRemoteDelayBelowFloor {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        minimum_delay: Tick,
        actual_minimum_delay: Tick,
    },
    ExpectedParallelRemoteDelayAboveCeiling {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        maximum_delay: Tick,
        actual_maximum_delay: Tick,
    },
    MissingParallelRemoteTrafficConsistencySummary {
        scope: WorkloadParallelRemoteFlowScope,
    },
    ParallelRemoteTrafficConsistencyMismatch(Box<WorkloadParallelRemoteTrafficConsistencyMismatch>),
    InvalidExpectedParallelRemoteFlowTimingWindow {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        first_tick: Tick,
        last_tick: Tick,
    },
    InvalidExpectedParallelRemoteFlowDelayBounds {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        minimum_delay: Tick,
        maximum_delay: Tick,
    },
    DuplicateExpectedParallelRemoteFlowTiming {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
    },
    MissingParallelRemoteFlowTimingSummary {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        expected_send_count: usize,
        expected_first_tick: Tick,
        expected_last_tick: Tick,
    },
    ExpectedParallelRemoteFlowTimingMismatch {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        expected_send_count: usize,
        actual_send_count: usize,
        expected_first_tick: Tick,
        actual_first_tick: Option<Tick>,
        expected_last_tick: Tick,
        actual_last_tick: Option<Tick>,
    },
    UnexpectedParallelRemoteFlowTiming {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        actual_send_count: usize,
        actual_first_tick: Tick,
        actual_last_tick: Tick,
    },
    ExpectedParallelRemoteFlowDelayBoundsMismatch {
        scope: WorkloadParallelRemoteFlowScope,
        source: u32,
        target: u32,
        expected_minimum_delay: Tick,
        actual_minimum_delay: Option<Tick>,
        expected_maximum_delay: Tick,
        actual_maximum_delay: Option<Tick>,
    },
    ZeroExpectedParallelWorkerCount {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelWorkerUse {
        scope: WorkloadParallelRemoteFlowScope,
    },
    ZeroExpectedParallelWorkerActivity {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelWorkerActivity {
        scope: WorkloadParallelRemoteFlowScope,
    },
    ZeroExpectedDataCacheProtocolRunCount {
        protocol: WorkloadDataCacheProtocol,
    },
    DuplicateExpectedDataCacheProtocolRunCount {
        protocol: WorkloadDataCacheProtocol,
    },
    DuplicateExpectedDataCacheRunAttribution,
    MissingDataCacheProtocolSummary {
        protocol: WorkloadDataCacheProtocol,
        minimum_run_count: usize,
    },
    ExpectedDataCacheProtocolRunCountBelowMinimum {
        protocol: WorkloadDataCacheProtocol,
        minimum_run_count: usize,
        actual_run_count: usize,
    },
    MissingDataCacheRunAttributionSummary {
        minimum_attributed_run_count: usize,
        maximum_unattributed_run_count: usize,
    },
    ExpectedDataCacheRunAttributionBelowMinimum {
        minimum_attributed_run_count: usize,
        actual_attributed_run_count: usize,
    },
    ExpectedDataCacheRunAttributionAboveMaximum {
        maximum_unattributed_run_count: usize,
        actual_unattributed_run_count: usize,
    },
    DataCacheRunAccountingMismatch {
        data_cache_parallel_run_count: usize,
        attributed_run_count: usize,
        unattributed_run_count: usize,
    },
    DataCacheProtocolAccountingMismatch {
        attributed_run_count: usize,
        protocol_run_count: usize,
    },
    MissingParallelWorkerSummary {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_max_workers: usize,
    },
    ExpectedParallelWorkerCountBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_max_workers: usize,
        actual_max_workers: usize,
    },
    MissingParallelWorkerActivitySummary {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_total_workers: usize,
    },
    ExpectedParallelWorkerActivityBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_total_workers: usize,
        actual_total_workers: usize,
    },
    ZeroExpectedParallelSchedulerProgress {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelSchedulerProgress {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelSchedulerIdleBound {
        scope: WorkloadParallelRemoteFlowScope,
    },
    MissingParallelSchedulerProgressSummary {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_epoch_count: usize,
        minimum_dispatch_count: usize,
    },
    MissingParallelSchedulerIdleSummary {
        scope: WorkloadParallelRemoteFlowScope,
        maximum_empty_epoch_count: usize,
    },
    ExpectedParallelSchedulerProgressBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_epoch_count: usize,
        actual_epoch_count: usize,
        minimum_dispatch_count: usize,
        actual_dispatch_count: usize,
    },
    ExpectedParallelSchedulerIdleAboveMaximum {
        scope: WorkloadParallelRemoteFlowScope,
        maximum_empty_epoch_count: usize,
        actual_empty_epoch_count: usize,
    },
    ZeroExpectedParallelFrontier {
        scope: WorkloadParallelRemoteFlowScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
    },
    DuplicateExpectedParallelFrontier {
        scope: WorkloadParallelRemoteFlowScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
    },
    MissingParallelFrontierSummary {
        scope: WorkloadParallelRemoteFlowScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
        minimum_now: Tick,
        minimum_safe_until: Tick,
    },
    ExpectedParallelFrontierBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
        minimum_now: Tick,
        actual_now: Option<Tick>,
        minimum_safe_until: Tick,
        actual_safe_until: Option<Tick>,
    },
    InvalidExpectedParallelBatchWorkerCount {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_worker_count: usize,
    },
    ZeroExpectedParallelBatchCount {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_worker_count: usize,
    },
    DuplicateExpectedParallelBatchActivity {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_worker_count: usize,
    },
    MissingParallelBatchActivitySummary {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_worker_count: usize,
        minimum_batch_count: usize,
    },
    ExpectedParallelBatchActivityBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_worker_count: usize,
        minimum_batch_count: usize,
        actual_batch_count: usize,
    },
    InvalidExpectedParallelBatchPartitionSet {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
    },
    ZeroExpectedParallelBatchPartitionSetCount {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
    },
    DuplicateExpectedParallelBatchPartitionSet {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
    },
    MissingParallelBatchPartitionSetSummary {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
        minimum_batch_count: usize,
    },
    ExpectedParallelBatchPartitionSetBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
        minimum_batch_count: usize,
        actual_batch_count: usize,
    },
    InvalidExpectedParallelBatchPartitionStreak {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
    },
    ZeroExpectedParallelBatchPartitionStreakCount {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
    },
    DuplicateExpectedParallelBatchPartitionStreak {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
    },
    MissingParallelBatchPartitionStreakSummary {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
        minimum_consecutive_batch_count: usize,
    },
    ExpectedParallelBatchPartitionStreakBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        partitions: Vec<u32>,
        minimum_consecutive_batch_count: usize,
        actual_consecutive_batch_count: usize,
    },
    InvalidExpectedParallelBatchTimelineRecord {
        scope: WorkloadParallelRemoteFlowScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    DuplicateExpectedParallelBatchTimelineRecord {
        scope: WorkloadParallelRemoteFlowScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    MissingParallelBatchTimelineSummary {
        scope: WorkloadParallelRemoteFlowScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    ExpectedParallelBatchTimelineRecordMissing {
        scope: WorkloadParallelRemoteFlowScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    UnexpectedParallelBatchTimelineRecord {
        scope: WorkloadParallelRemoteFlowScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    ZeroExpectedParallelPartitionCount {
        scope: WorkloadParallelRemoteFlowScope,
    },
    DuplicateExpectedParallelPartitionUse {
        scope: WorkloadParallelRemoteFlowScope,
    },
    ZeroExpectedParallelPartitionActivity {
        scope: WorkloadParallelRemoteFlowScope,
        partition: u32,
    },
    DuplicateExpectedParallelPartitionActivity {
        scope: WorkloadParallelRemoteFlowScope,
        partition: u32,
    },
    MissingParallelPartitionSummary {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_active_partitions: usize,
    },
    ExpectedParallelPartitionCountBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        minimum_active_partitions: usize,
        actual_active_partitions: usize,
    },
    MissingParallelPartitionActivitySummary {
        scope: WorkloadParallelRemoteFlowScope,
        partition: u32,
    },
    ExpectedParallelPartitionActivityBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        partition: u32,
        minimum_worker_count: usize,
        actual_worker_count: usize,
        minimum_dispatch_count: usize,
        actual_dispatch_count: usize,
        minimum_remote_send_count: usize,
        actual_remote_send_count: usize,
        minimum_remote_receive_count: usize,
        actual_remote_receive_count: usize,
    },
    DuplicateExpectedCleanParallelDiagnostics {
        scope: WorkloadParallelDiagnosticScope,
    },
    ZeroExpectedLivelockTransitionThreshold {
        scope: WorkloadParallelDiagnosticScope,
    },
    ZeroExpectedResourceActivity {
        scope: WorkloadResourceActivityScope,
    },
    DuplicateExpectedResourceActivity {
        scope: WorkloadResourceActivityScope,
    },
    MissingResourceActivitySummary {
        scope: WorkloadResourceActivityScope,
        minimum_operation_count: usize,
        minimum_active_resource_count: usize,
    },
    ExpectedResourceActivityBelowMinimum {
        scope: WorkloadResourceActivityScope,
        minimum_operation_count: usize,
        actual_operation_count: usize,
        minimum_active_resource_count: usize,
        actual_active_resource_count: usize,
    },
    MissingParallelDiagnosticSummary {
        scope: WorkloadParallelDiagnosticScope,
    },
    ExpectedCleanParallelDiagnosticsViolation {
        scope: WorkloadParallelDiagnosticScope,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
        livelock_diagnostic_count: usize,
        livelock_subjects: Vec<String>,
    },
}
