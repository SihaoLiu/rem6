use rem6_boot::BootError;
use rem6_fabric::{FabricLinkId, QosPriority, QosRequestorId, VirtualNetworkId};
use rem6_kernel::{Tick, WaitForEdgeKind, WaitForNode};
use rem6_memory::MemoryError;

use crate::{
    WorkloadDataCacheProtocol, WorkloadExecutionMode, WorkloadId, WorkloadManifestIdentity,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineScope, WorkloadParallelBatchWorkerScope,
    WorkloadParallelDiagnosticScope, WorkloadParallelFrontierStage,
    WorkloadParallelProgressTransitionExpectationError, WorkloadParallelRemoteFlowScope,
    WorkloadParallelSchedulerScope, WorkloadResourceAcquisitionField,
    WorkloadResourceActivityScope, WorkloadResourceId, WorkloadResourceKind, WorkloadRouteId,
    WorkloadRouteLatency, WorkloadSuiteIdentity,
};

mod display;
mod fabric_display;

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
    EmptyResourceAcquisitionField {
        field: WorkloadResourceAcquisitionField,
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
    SuiteExecutionOccupancyWorkerCountTicksBelowMinimum {
        worker_count: usize,
        minimum_ticks: Tick,
        actual_ticks: Tick,
    },
    SuiteExecutionFullOccupancyTicksBelowMinimum {
        minimum_ticks: Tick,
        actual_ticks: Tick,
    },
    SuiteExecutionUnderoccupiedTicksAboveMaximum {
        maximum_ticks: Tick,
        actual_ticks: Tick,
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
    DuplicateExpectedCheckpointManifestSummary {
        label: String,
    },
    DuplicateExpectedCheckpointRestoreManifestSummary {
        label: String,
    },
    DuplicateExpectedCheckpointComponentSummary {
        label: String,
        component: String,
    },
    DuplicateExpectedCheckpointRestoreComponentSummary {
        label: String,
        component: String,
    },
    MissingCheckpointManifestSummary {
        label: String,
    },
    MissingCheckpointRestoreManifestSummary {
        label: String,
    },
    MissingCheckpointComponentSummary {
        label: String,
        component: String,
    },
    MissingCheckpointRestoreComponentSummary {
        label: String,
        component: String,
    },
    CheckpointManifestSummaryBelowMinimum {
        label: String,
        minimum_component_count: usize,
        actual_component_count: usize,
        minimum_chunk_count: usize,
        actual_chunk_count: usize,
        minimum_payload_bytes: usize,
        actual_payload_bytes: usize,
    },
    CheckpointRestoreManifestSummaryBelowMinimum {
        label: String,
        minimum_component_count: usize,
        actual_component_count: usize,
        minimum_chunk_count: usize,
        actual_chunk_count: usize,
        minimum_payload_bytes: usize,
        actual_payload_bytes: usize,
    },
    CheckpointComponentSummaryBelowMinimum {
        label: String,
        component: String,
        minimum_chunk_count: usize,
        actual_chunk_count: usize,
        minimum_payload_bytes: usize,
        actual_payload_bytes: usize,
    },
    CheckpointRestoreComponentSummaryBelowMinimum {
        label: String,
        component: String,
        minimum_chunk_count: usize,
        actual_chunk_count: usize,
        minimum_payload_bytes: usize,
        actual_payload_bytes: usize,
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
        scope: WorkloadParallelBatchWorkerScope,
    },
    DuplicateExpectedParallelWorkerUse {
        scope: WorkloadParallelBatchWorkerScope,
    },
    ZeroExpectedParallelWorkerActivity {
        scope: WorkloadParallelBatchWorkerScope,
    },
    DuplicateExpectedParallelWorkerActivity {
        scope: WorkloadParallelBatchWorkerScope,
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
        scope: WorkloadParallelBatchWorkerScope,
        minimum_max_workers: usize,
    },
    ExpectedParallelWorkerCountBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_max_workers: usize,
        actual_max_workers: usize,
    },
    MissingParallelWorkerActivitySummary {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_total_workers: usize,
    },
    ExpectedParallelWorkerActivityBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_total_workers: usize,
        actual_total_workers: usize,
    },
    ZeroExpectedParallelSchedulerProgress {
        scope: WorkloadParallelSchedulerScope,
    },
    DuplicateExpectedParallelSchedulerProgress {
        scope: WorkloadParallelSchedulerScope,
    },
    DuplicateExpectedParallelSchedulerIdleBound {
        scope: WorkloadParallelSchedulerScope,
    },
    MissingParallelSchedulerProgressSummary {
        scope: WorkloadParallelSchedulerScope,
        minimum_epoch_count: usize,
        minimum_dispatch_count: usize,
    },
    MissingParallelSchedulerIdleSummary {
        scope: WorkloadParallelSchedulerScope,
        maximum_empty_epoch_count: usize,
    },
    ExpectedParallelSchedulerProgressBelowMinimum {
        scope: WorkloadParallelSchedulerScope,
        minimum_epoch_count: usize,
        actual_epoch_count: usize,
        minimum_dispatch_count: usize,
        actual_dispatch_count: usize,
    },
    ExpectedParallelSchedulerIdleAboveMaximum {
        scope: WorkloadParallelSchedulerScope,
        maximum_empty_epoch_count: usize,
        actual_empty_epoch_count: usize,
    },
    ZeroExpectedParallelFrontier {
        scope: WorkloadParallelSchedulerScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
    },
    DuplicateExpectedParallelFrontier {
        scope: WorkloadParallelSchedulerScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
    },
    MissingParallelFrontierSummary {
        scope: WorkloadParallelSchedulerScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
        minimum_now: Tick,
        minimum_safe_until: Tick,
    },
    ExpectedParallelFrontierBelowMinimum {
        scope: WorkloadParallelSchedulerScope,
        stage: WorkloadParallelFrontierStage,
        partition: u32,
        minimum_now: Tick,
        actual_now: Option<Tick>,
        minimum_safe_until: Tick,
        actual_safe_until: Option<Tick>,
    },
    InvalidExpectedParallelBatchWorkerCount {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    ZeroExpectedParallelBatchCount {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    DuplicateExpectedParallelBatchActivity {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    MissingParallelBatchActivitySummary {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_batch_count: usize,
    },
    ExpectedParallelBatchActivityBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_batch_count: usize,
        actual_batch_count: usize,
    },
    ParallelBatchWorkerCountBelowMinimum {
        scope: WorkloadParallelRemoteFlowScope,
        worker_count: usize,
        minimum_batch_count: usize,
        actual_batch_count: usize,
    },
    InvalidExpectedParallelBatchWorkerBucket {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
    },
    ZeroExpectedParallelBatchWorkerBucket {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
    },
    DuplicateExpectedParallelBatchWorkerBucket {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
    },
    MissingParallelBatchWorkerBucketSummary {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
        minimum_batch_count: usize,
    },
    ExpectedParallelBatchWorkerBucketBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
        minimum_batch_count: usize,
        actual_batch_count: usize,
    },
    InvalidExpectedParallelBatchWorkerTickBucket {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
    },
    ZeroExpectedParallelBatchWorkerTickBucket {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
    },
    DuplicateExpectedParallelBatchWorkerTickBucket {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
    },
    MissingParallelBatchWorkerTickBucketSummary {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
        minimum_ticks: Tick,
    },
    ExpectedParallelBatchWorkerTickBucketBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        worker_count: usize,
        minimum_ticks: Tick,
        actual_ticks: Tick,
    },
    InvalidExpectedParallelBatchWorkerTickActivity {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    ZeroExpectedParallelBatchWorkerTickActivity {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    DuplicateExpectedParallelBatchWorkerTickActivity {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    MissingParallelBatchWorkerTickActivitySummary {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_ticks: Tick,
    },
    ExpectedParallelBatchWorkerTickActivityBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_ticks: Tick,
        actual_ticks: Tick,
    },
    InvalidExpectedParallelBatchWorkerTickStreak {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    ZeroExpectedParallelBatchWorkerTickStreak {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    DuplicateExpectedParallelBatchWorkerTickStreak {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    MissingParallelBatchWorkerTickStreakSummary {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_consecutive_ticks: Tick,
    },
    ExpectedParallelBatchWorkerTickStreakBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_consecutive_ticks: Tick,
        actual_consecutive_ticks: Tick,
    },
    InvalidExpectedParallelBatchWorkerTicks {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    ZeroExpectedParallelBatchWorkerTicks {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    DuplicateExpectedParallelBatchWorkerTicks {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
    },
    MissingParallelBatchWorkerTicksSummary {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_worker_ticks: Tick,
    },
    ExpectedParallelBatchWorkerTicksBelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_worker_count: usize,
        minimum_worker_ticks: Tick,
        actual_worker_ticks: Tick,
    },
    InvalidExpectedParallelBatchPartitionSet {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
    },
    ZeroExpectedParallelBatchPartitionSetCount {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
    },
    DuplicateExpectedParallelBatchPartitionSet {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
    },
    MissingParallelBatchPartitionSetSummary {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
        minimum_batch_count: usize,
    },
    ExpectedParallelBatchPartitionSetBelowMinimum {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
        minimum_batch_count: usize,
        actual_batch_count: usize,
    },
    InvalidExpectedParallelBatchPartitionStreak {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
    },
    ZeroExpectedParallelBatchPartitionStreakCount {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
    },
    DuplicateExpectedParallelBatchPartitionStreak {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
    },
    MissingParallelBatchPartitionStreakSummary {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
        minimum_consecutive_batch_count: usize,
    },
    ExpectedParallelBatchPartitionStreakBelowMinimum {
        scope: WorkloadParallelBatchPartitionScope,
        partitions: Vec<u32>,
        minimum_consecutive_batch_count: usize,
        actual_consecutive_batch_count: usize,
    },
    InvalidExpectedParallelBatchTimelineRecord {
        scope: WorkloadParallelBatchTimelineScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    DuplicateExpectedParallelBatchTimelineRecord {
        scope: WorkloadParallelBatchTimelineScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    MissingParallelBatchTimelineSummary {
        scope: WorkloadParallelBatchTimelineScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    ExpectedParallelBatchTimelineRecordMissing {
        scope: WorkloadParallelBatchTimelineScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    UnexpectedParallelBatchTimelineRecord {
        scope: WorkloadParallelBatchTimelineScope,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: Vec<u32>,
        worker_count: usize,
    },
    ZeroExpectedParallelPartitionCount {
        scope: WorkloadParallelBatchPartitionScope,
    },
    DuplicateExpectedParallelPartitionUse {
        scope: WorkloadParallelBatchPartitionScope,
    },
    ZeroExpectedParallelPartitionActivity {
        scope: WorkloadParallelBatchPartitionScope,
        partition: u32,
    },
    DuplicateExpectedParallelPartitionActivity {
        scope: WorkloadParallelBatchPartitionScope,
        partition: u32,
    },
    MissingParallelPartitionSummary {
        scope: WorkloadParallelBatchPartitionScope,
        minimum_active_partitions: usize,
    },
    ExpectedParallelPartitionCountBelowMinimum {
        scope: WorkloadParallelBatchPartitionScope,
        minimum_active_partitions: usize,
        actual_active_partitions: usize,
    },
    MissingParallelPartitionActivitySummary {
        scope: WorkloadParallelBatchPartitionScope,
        partition: u32,
    },
    ExpectedParallelPartitionActivityBelowMinimum {
        scope: WorkloadParallelBatchPartitionScope,
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
    ZeroExpectedParallelWaitForEdgeKindCount {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
    },
    DuplicateExpectedParallelWaitForEdgeKindCount {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
    },
    ZeroExpectedParallelWaitForEdgeKindWindow {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
    },
    InvalidExpectedParallelWaitForEdgeKindWindow {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
        first_tick: Tick,
        last_tick: Tick,
    },
    DuplicateExpectedParallelWaitForEdgeKindWindow {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
    },
    ZeroExpectedParallelWaitForBlockedNodeWindow {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
    },
    InvalidExpectedParallelWaitForBlockedNodeWindow {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
        first_tick: Tick,
        last_tick: Tick,
    },
    DuplicateExpectedParallelWaitForBlockedNodeWindow {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
    },
    ZeroExpectedParallelWaitForTargetNodeWindow {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
    },
    InvalidExpectedParallelWaitForTargetNodeWindow {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
        first_tick: Tick,
        last_tick: Tick,
    },
    DuplicateExpectedParallelWaitForTargetNodeWindow {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
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
    ZeroExpectedFabricHopActivity {
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
    },
    DuplicateExpectedFabricHopActivity {
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
    },
    InvalidExpectedFabricHopActivityWindow {
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        first_tick: Tick,
        last_tick: Tick,
    },
    MissingFabricHopActivitySummary {
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        minimum_byte_count: u64,
        minimum_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
        required_first_tick: Option<Tick>,
        required_last_tick: Option<Tick>,
    },
    ExpectedFabricHopActivityBelowMinimum {
        hop_index: usize,
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_byte_count: u64,
        actual_byte_count: u64,
        minimum_occupied_ticks: Tick,
        actual_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    },
    ZeroExpectedFabricLaneActivity {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
    },
    DuplicateExpectedFabricLaneActivity {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
    },
    InvalidExpectedFabricLaneActivityWindow {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        first_tick: Tick,
        last_tick: Tick,
    },
    InvalidExpectedFabricLaneActivityQueueDelayBudget {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        maximum_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
    },
    MissingFabricLaneActivitySummary {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        minimum_byte_count: u64,
        minimum_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
        minimum_max_queue_delay_ticks: Tick,
        required_first_tick: Option<Tick>,
        required_last_tick: Option<Tick>,
    },
    ExpectedFabricLaneActivityBelowMinimum {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_byte_count: u64,
        actual_byte_count: u64,
        minimum_occupied_ticks: Tick,
        actual_occupied_ticks: Tick,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        minimum_max_queue_delay_ticks: Tick,
        actual_max_queue_delay_ticks: Tick,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    },
    ExpectedFabricLaneActivityAboveMaximum {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
        maximum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
        actual_max_queue_delay_ticks: Tick,
    },
    ZeroExpectedFabricLinkActivity {
        link: FabricLinkId,
    },
    DuplicateExpectedFabricLinkActivity {
        link: FabricLinkId,
    },
    InvalidExpectedFabricLinkActivityWindow {
        link: FabricLinkId,
        first_tick: Tick,
        last_tick: Tick,
    },
    InvalidExpectedFabricLinkActivityQueueDelayBudget {
        link: FabricLinkId,
        maximum_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
    },
    MissingFabricLinkActivitySummary {
        link: FabricLinkId,
        minimum_transfer_count: usize,
        minimum_active_virtual_network_count: usize,
        minimum_queue_delay_ticks: Tick,
        minimum_contended_virtual_network_count: usize,
        required_first_tick: Option<Tick>,
        required_last_tick: Option<Tick>,
    },
    ExpectedFabricLinkActivityBelowMinimum {
        link: FabricLinkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_active_virtual_network_count: usize,
        actual_active_virtual_network_count: usize,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        minimum_contended_virtual_network_count: usize,
        actual_contended_virtual_network_count: usize,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    },
    ExpectedFabricLinkActivityAboveMaximum {
        link: FabricLinkId,
        maximum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
        actual_max_queue_delay_ticks: Tick,
    },
    ZeroExpectedFabricVirtualNetworkActivity {
        virtual_network: VirtualNetworkId,
    },
    DuplicateExpectedFabricVirtualNetworkActivity {
        virtual_network: VirtualNetworkId,
    },
    DuplicateExpectedFabricVirtualNetworkActivityCoverageLink {
        virtual_network: VirtualNetworkId,
        link: FabricLinkId,
    },
    InvalidExpectedFabricVirtualNetworkActivityWindow {
        virtual_network: VirtualNetworkId,
        first_tick: Tick,
        last_tick: Tick,
    },
    InvalidExpectedFabricVirtualNetworkActivityQueueDelayBudget {
        virtual_network: VirtualNetworkId,
        maximum_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
    },
    InvalidExpectedFabricVirtualNetworkActivityLaneBudget {
        virtual_network: VirtualNetworkId,
        maximum_active_lane_count: usize,
        maximum_contended_lane_count: usize,
    },
    MissingFabricVirtualNetworkActivitySummary {
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        minimum_active_lane_count: usize,
        minimum_queue_delay_ticks: Tick,
        minimum_contended_lane_count: usize,
        required_first_tick: Option<Tick>,
        required_last_tick: Option<Tick>,
    },
    MissingFabricVirtualNetworkLinkCoverage {
        virtual_network: VirtualNetworkId,
        required_links: Vec<FabricLinkId>,
    },
    ExpectedFabricVirtualNetworkActivityBelowMinimum {
        virtual_network: VirtualNetworkId,
        minimum_transfer_count: usize,
        actual_transfer_count: usize,
        minimum_active_lane_count: usize,
        actual_active_lane_count: usize,
        minimum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        minimum_contended_lane_count: usize,
        actual_contended_lane_count: usize,
        required_first_tick: Option<Tick>,
        actual_first_tick: Tick,
        required_last_tick: Option<Tick>,
        actual_last_tick: Tick,
    },
    ExpectedFabricVirtualNetworkActivityAboveMaximum {
        virtual_network: VirtualNetworkId,
        maximum_queue_delay_ticks: Tick,
        actual_queue_delay_ticks: Tick,
        maximum_max_queue_delay_ticks: Tick,
        actual_max_queue_delay_ticks: Tick,
    },
    ExpectedFabricVirtualNetworkActivityAboveLaneBudget {
        virtual_network: VirtualNetworkId,
        maximum_active_lane_count: usize,
        actual_active_lane_count: usize,
        maximum_contended_lane_count: usize,
        actual_contended_lane_count: usize,
    },
    ExpectedFabricVirtualNetworkLinkCoverageMissing {
        virtual_network: VirtualNetworkId,
        required_links: Vec<FabricLinkId>,
        actual_links: Vec<FabricLinkId>,
        missing_links: Vec<FabricLinkId>,
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
    ExpectedParallelWaitForEdgeKindCountBelowMinimum {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
        minimum_edge_count: usize,
        actual_edge_count: usize,
    },
    ExpectedParallelWaitForEdgeKindWindowMismatch {
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
        expected_edge_count: usize,
        actual_edge_count: usize,
        expected_first_tick: Tick,
        actual_first_tick: Option<Tick>,
        expected_last_tick: Tick,
        actual_last_tick: Option<Tick>,
    },
    ExpectedParallelWaitForBlockedNodeWindowMismatch {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
        expected_edge_count: usize,
        actual_edge_count: usize,
        expected_first_tick: Tick,
        actual_first_tick: Option<Tick>,
        expected_last_tick: Tick,
        actual_last_tick: Option<Tick>,
    },
    ExpectedParallelWaitForTargetNodeWindowMismatch {
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
        expected_edge_count: usize,
        actual_edge_count: usize,
        expected_first_tick: Tick,
        actual_first_tick: Option<Tick>,
        expected_last_tick: Tick,
        actual_last_tick: Option<Tick>,
    },
}
