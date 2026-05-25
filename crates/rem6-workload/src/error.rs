use std::error::Error;
use std::fmt;

use rem6_boot::BootError;
use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::Tick;
use rem6_memory::MemoryError;

use crate::{
    WorkloadDataCacheProtocol, WorkloadExecutionMode, WorkloadManifestIdentity,
    WorkloadParallelDiagnosticScope, WorkloadParallelFrontierStage,
    WorkloadParallelRemoteFlowScope, WorkloadResourceActivityScope, WorkloadResourceId,
    WorkloadResourceKind, WorkloadRouteId, WorkloadRouteLatency,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadError {
    Boot(BootError),
    Memory(MemoryError),
    EmptyWorkloadId,
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
    StatsAfterFinalTick {
        stats_tick: Tick,
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
    },
}

impl fmt::Display for WorkloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boot(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::EmptyWorkloadId => write!(formatter, "workload id must not be empty"),
            Self::EmptyResourceId => write!(formatter, "resource id must not be empty"),
            Self::EmptyRouteId => write!(formatter, "route id must not be empty"),
            Self::EmptyEndpoint => write!(formatter, "endpoint id must not be empty"),
            Self::EmptyResourceDigest { resource } => write!(
                formatter,
                "resource {} must include a digest",
                resource.as_str()
            ),
            Self::EmptyResourceLocator { resource } => write!(
                formatter,
                "resource {} must include a locator",
                resource.as_str()
            ),
            Self::DuplicateResource { resource } => {
                write!(
                    formatter,
                    "resource {} is already defined",
                    resource.as_str()
                )
            }
            Self::MissingRequiredResource { resource } => write!(
                formatter,
                "required resource {} is not defined",
                resource.as_str()
            ),
            Self::DuplicateResourcePayload { resource } => write!(
                formatter,
                "payload for resource {} is already defined",
                resource.as_str()
            ),
            Self::MissingResourcePayload { resource } => write!(
                formatter,
                "required resource {} has no resolved payload",
                resource.as_str()
            ),
            Self::UnexpectedResourcePayload { resource } => write!(
                formatter,
                "resource payload {} is not required by the workload",
                resource.as_str()
            ),
            Self::ResourcePayloadDigestMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "resource payload {} has digest {actual}, expected {expected}",
                resource.as_str()
            ),
            Self::ResourcePayloadSizeMismatch {
                resource,
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "resource payload {} has {actual_bytes} bytes, expected {expected_bytes}",
                resource.as_str()
            ),
            Self::ResourceKindMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "resource {} has kind {}, expected {}",
                resource.as_str(),
                actual.as_str(),
                expected.as_str()
            ),
            Self::ZeroHostLatency => write!(formatter, "host latency must be positive"),
            Self::ZeroLineBytes { target } => {
                write!(
                    formatter,
                    "memory target {target} line bytes must be positive"
                )
            }
            Self::MemoryProfileTargetMismatch {
                target,
                profile_target,
            } => write!(
                formatter,
                "memory target {target} cannot use external memory profile for target {profile_target}"
            ),
            Self::MemoryProfileLineSizeMismatch {
                target,
                line_bytes,
                profile_line_bytes,
            } => write!(
                formatter,
                "memory target {target} has {line_bytes}-byte lines but external memory profile uses {profile_line_bytes}"
            ),
            Self::MemoryProfileGeometryLineSizeMismatch {
                target,
                layout_line_bytes,
                geometry_line_bytes,
            } => write!(
                formatter,
                "memory target {target} external memory profile has {layout_line_bytes}-byte layout lines but {geometry_line_bytes}-byte DRAM geometry lines"
            ),
            Self::ZeroRouteLatency { route, latency } => write!(
                formatter,
                "route {} {latency:?} latency must be positive",
                route.as_str()
            ),
            Self::EmptyMemoryRoutePath { route } => {
                write!(
                    formatter,
                    "memory route {} must include at least one hop",
                    route.as_str()
                )
            }
            Self::ZeroRouteHopLatency { endpoint, latency } => write!(
                formatter,
                "route hop {endpoint} {latency:?} latency must be positive"
            ),
            Self::EmptyFabricLink => write!(formatter, "fabric link id must not be empty"),
            Self::ZeroFabricBandwidth { link } => {
                write!(
                    formatter,
                    "fabric link {link} bandwidth bytes per tick must be positive"
                )
            }
            Self::ZeroFabricCreditDepth { link } => {
                write!(formatter, "fabric link {link} credit depth must be positive")
            }
            Self::ZeroTopologyPartitions => {
                write!(formatter, "topology partition count must be positive")
            }
            Self::ZeroMinRemoteDelay => write!(formatter, "minimum remote delay must be positive"),
            Self::ZeroParallelWorkerLimit => {
                write!(formatter, "parallel worker limit must be positive")
            }
            Self::PartitionOutOfRange {
                partition,
                partition_count,
            } => write!(
                formatter,
                "partition {partition} is outside topology partition count {partition_count}"
            ),
            Self::DuplicateMemoryTarget { target } => {
                write!(formatter, "memory target {target} is already defined")
            }
            Self::MissingMemoryTarget { target } => {
                write!(formatter, "memory target {target} is not defined")
            }
            Self::DuplicateRoute { route } => {
                write!(formatter, "route {} is already defined", route.as_str())
            }
            Self::DuplicateRiscvCore { cpu } => {
                write!(formatter, "RISC-V core {cpu} is already defined")
            }
            Self::MissingCoreFetchRoute { cpu, route } => write!(
                formatter,
                "RISC-V core {cpu} fetch route {} is not defined",
                route.as_str()
            ),
            Self::CoreFetchRouteSourceMismatch {
                cpu,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core {cpu} fetch route {} starts at partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::CoreFetchRouteEndpointMismatch {
                cpu,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core {cpu} fetch route {} starts at endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MissingCoreDataRoute { cpu, route } => write!(
                formatter,
                "RISC-V core {cpu} data route {} is not defined",
                route.as_str()
            ),
            Self::CoreDataRouteSourceMismatch {
                cpu,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core {cpu} data route {} starts at partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::CoreDataRouteEndpointMismatch {
                cpu,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core {cpu} data route {} starts at endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MissingDataCacheBackingRoute { route } => write!(
                formatter,
                "RISC-V data cache backing route {} is not defined",
                route.as_str()
            ),
            Self::DataCacheBackingRouteSourceMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V data cache backing route {} starts at partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::DataCacheBackingRouteEndpointMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V data cache backing route {} starts at endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::ZeroGpuComputeUnits { device } => {
                write!(formatter, "GPU device {device} needs at least one compute unit")
            }
            Self::ZeroGpuWaveSlots { device } => write!(
                formatter,
                "GPU device {device} needs at least one wave slot per compute unit"
            ),
            Self::DuplicateGpuDevice { device } => {
                write!(formatter, "GPU device {device} is already defined")
            }
            Self::MissingGpuCommandRoute { device, route } => write!(
                formatter,
                "GPU device {device} command route {} is not defined",
                route.as_str()
            ),
            Self::GpuCommandRouteTargetMismatch {
                device,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "GPU device {device} command route {} targets partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::GpuCommandRouteEndpointMismatch {
                device,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "GPU device {device} command route {} targets endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MissingGpuDevice { device } => {
                write!(formatter, "GPU device {device} is not defined")
            }
            Self::ZeroGpuKernelWorkgroups { device, kernel } => write!(
                formatter,
                "GPU kernel {kernel} on device {device} needs at least one workgroup"
            ),
            Self::ZeroGpuKernelLatency { device, kernel } => write!(
                formatter,
                "GPU kernel {kernel} on device {device} needs positive workgroup latency"
            ),
            Self::ZeroGpuDmaBytes { device, transfer } => write!(
                formatter,
                "GPU DMA transfer {transfer} on device {device} needs at least one byte"
            ),
            Self::MissingGpuDmaRoute { device, route } => write!(
                formatter,
                "GPU DMA route {} for device {device} is not defined",
                route.as_str()
            ),
            Self::GpuDmaRouteSourceMismatch {
                device,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "GPU DMA route {} for device {device} starts on partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::GpuDmaRouteEndpointMismatch {
                device,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "GPU DMA route {} for device {device} starts on endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::ZeroAcceleratorLanes { engine } => {
                write!(
                    formatter,
                    "accelerator engine {engine} needs at least one lane"
                )
            }
            Self::DuplicateAcceleratorDevice { engine } => {
                write!(formatter, "accelerator engine {engine} is already defined")
            }
            Self::MissingAcceleratorCommandRoute { engine, route } => write!(
                formatter,
                "accelerator engine {engine} command route {} is not defined",
                route.as_str()
            ),
            Self::AcceleratorCommandRouteTargetMismatch {
                engine,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "accelerator engine {engine} command route {} targets partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::AcceleratorCommandRouteEndpointMismatch {
                engine,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "accelerator engine {engine} command route {} targets endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MissingAcceleratorDevice { engine } => {
                write!(formatter, "accelerator engine {engine} is not defined")
            }
            Self::ZeroAcceleratorExecutionLatency { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs positive execution latency"
            ),
            Self::ZeroAcceleratorGpuWorkgroups { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs at least one GPU workgroup"
            ),
            Self::ZeroAcceleratorNpuTiles { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs at least one NPU tile"
            ),
            Self::ZeroAcceleratorDmaBytes { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs at least one DMA byte"
            ),
            Self::ZeroAcceleratorDmaCopyBytes { engine, transfer } => write!(
                formatter,
                "accelerator DMA transfer {transfer} on engine {engine} needs at least one byte"
            ),
            Self::MissingAcceleratorDmaRoute { engine, route } => write!(
                formatter,
                "accelerator engine {engine} DMA route {} is not defined",
                route.as_str()
            ),
            Self::AcceleratorDmaRouteSourceMismatch {
                engine,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "accelerator engine {engine} DMA route {} starts on partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::AcceleratorDmaRouteEndpointMismatch {
                engine,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "accelerator engine {engine} DMA route {} starts on endpoint {actual}, expected {expected}",
                route.as_str()
            ),
            Self::ZeroQosPriorityLevels => {
                write!(formatter, "QoS priority levels must be positive")
            }
            Self::QosPriorityOutOfRange {
                priority,
                priority_levels,
            } => write!(
                formatter,
                "QoS priority {} is outside {priority_levels} configured levels",
                priority.get()
            ),
            Self::DuplicateQosRequestorPriority { requestor } => write!(
                formatter,
                "QoS requestor {} has more than one priority declaration",
                requestor.get()
            ),
            Self::ManifestIdentityMismatch { expected, actual } => write!(
                formatter,
                "workload result belongs to manifest {}, expected {}",
                actual.as_str(),
                expected.as_str()
            ),
            Self::StatsAfterFinalTick {
                stats_tick,
                final_tick,
            } => write!(
                formatter,
                "stats snapshot tick {stats_tick} is after final tick {final_tick}"
            ),
            Self::PlannedHostEventAfterFinalTick {
                event_tick,
                final_tick,
            } => write!(
                formatter,
                "planned host event at tick {event_tick} is after final tick {final_tick}"
            ),
            Self::MissingCheckpointLabel { label } => {
                write!(
                    formatter,
                    "planned checkpoint label {label} was not recorded"
                )
            }
            Self::UnexpectedCheckpointLabel { label } => {
                write!(formatter, "checkpoint label {label} was not planned")
            }
            Self::MissingCheckpointRestoreLabel { label } => {
                write!(
                    formatter,
                    "planned checkpoint restore label {label} was not recorded"
                )
            }
            Self::UnexpectedCheckpointRestoreLabel { label } => {
                write!(
                    formatter,
                    "checkpoint restore label {label} was not planned"
                )
            }
            Self::MissingExecutionModeSwitch { tick, target, mode } => write!(
                formatter,
                "planned execution mode switch for {target} to {} at tick {tick} was not recorded",
                mode.as_str()
            ),
            Self::UnexpectedExecutionModeSwitch { tick, target, mode } => write!(
                formatter,
                "execution mode switch for {target} to {} at tick {tick} was not planned",
                mode.as_str()
            ),
            Self::StopReasonMismatch { expected, actual } => match actual {
                Some(actual) => write!(
                    formatter,
                    "stop reason {actual} does not match planned reason {expected}"
                ),
                None => write!(formatter, "missing planned stop reason {expected}"),
            },
            Self::UnexpectedStopReason { actual } => {
                write!(formatter, "stop reason {actual} was not planned")
            }
            Self::ZeroExpectedParallelRemoteFlowCount {
                scope,
                source,
                target,
            } => write!(
                formatter,
                "expected {} remote flow {source}->{target} must have a positive send count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelRemoteFlow {
                scope,
                source,
                target,
            } => write!(
                formatter,
                "expected {} remote flow {source}->{target} is already declared",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelRemoteSend {
                scope,
                source,
                target,
                source_tick,
                delivery_tick,
                order,
            } => write!(
                formatter,
                "expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} is already declared",
                scope.as_str()
            ),
            Self::MissingParallelExecutionSummary {
                scope,
                source,
                target,
                expected_send_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} remote flow {source}->{target} with {expected_send_count} sends",
                scope.as_str()
            ),
            Self::ExpectedParallelRemoteFlowCountMismatch {
                scope,
                source,
                target,
                expected_send_count,
                actual_send_count,
            } => write!(
                formatter,
                "expected {} remote flow {source}->{target} to have {expected_send_count} sends, got {actual_send_count}",
                scope.as_str()
            ),
            Self::MissingParallelRemoteSendSummary {
                scope,
                source,
                target,
                source_tick,
                delivery_tick,
                order,
            } => write!(
                formatter,
                "missing parallel summary for expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order}",
                scope.as_str()
            ),
            Self::ExpectedParallelRemoteSendMissing {
                scope,
                source,
                target,
                source_tick,
                delivery_tick,
                order,
            } => write!(
                formatter,
                "expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} was not recorded",
                scope.as_str()
            ),
            Self::InvalidExpectedParallelRemoteFlowTimingWindow {
                scope,
                source,
                target,
                first_tick,
                last_tick,
            } => write!(
                formatter,
                "expected {} remote flow timing {source}->{target} first tick {first_tick} is after last tick {last_tick}",
                scope.as_str()
            ),
            Self::InvalidExpectedParallelRemoteFlowDelayBounds {
                scope,
                source,
                target,
                minimum_delay,
                maximum_delay,
            } => write!(
                formatter,
                "expected {} remote flow timing {source}->{target} minimum delay {minimum_delay} is above maximum delay {maximum_delay}",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelRemoteFlowTiming {
                scope,
                source,
                target,
            } => write!(
                formatter,
                "expected {} remote flow timing {source}->{target} is already declared",
                scope.as_str()
            ),
            Self::MissingParallelRemoteFlowTimingSummary {
                scope,
                source,
                target,
                expected_send_count,
                expected_first_tick,
                expected_last_tick,
            } => write!(
                formatter,
                "missing parallel summary for expected {} remote flow timing {source}->{target} with {expected_send_count} sends from tick {expected_first_tick} to {expected_last_tick}",
                scope.as_str()
            ),
            Self::ExpectedParallelRemoteFlowTimingMismatch {
                scope,
                source,
                target,
                expected_send_count,
                actual_send_count,
                expected_first_tick,
                actual_first_tick,
                expected_last_tick,
                actual_last_tick,
            } => {
                let actual_first_tick = actual_first_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let actual_last_tick = actual_last_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "none".to_string());
                write!(
                    formatter,
                    "expected {} remote flow timing {source}->{target} to have {expected_send_count} sends from tick {expected_first_tick} to {expected_last_tick}, got {actual_send_count} sends from tick {actual_first_tick} to {actual_last_tick}",
                    scope.as_str()
                )
            }
            Self::ExpectedParallelRemoteFlowDelayBoundsMismatch {
                scope,
                source,
                target,
                expected_minimum_delay,
                actual_minimum_delay,
                expected_maximum_delay,
                actual_maximum_delay,
            } => {
                let actual_minimum_delay = actual_minimum_delay
                    .map(|delay| delay.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let actual_maximum_delay = actual_maximum_delay
                    .map(|delay| delay.to_string())
                    .unwrap_or_else(|| "none".to_string());
                write!(
                    formatter,
                    "expected {} remote flow timing {source}->{target} delay bounds {expected_minimum_delay} to {expected_maximum_delay}, got {actual_minimum_delay} to {actual_maximum_delay}",
                    scope.as_str()
                )
            }
            Self::ZeroExpectedParallelWorkerCount { scope } => write!(
                formatter,
                "expected {} worker use must require a positive maximum worker count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelWorkerUse { scope } => write!(
                formatter,
                "expected {} worker use is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelWorkerActivity { scope } => write!(
                formatter,
                "expected {} worker activity must require a positive total worker count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelWorkerActivity { scope } => write!(
                formatter,
                "expected {} worker activity is already declared",
                scope.as_str()
            ),
            Self::MissingParallelWorkerSummary {
                scope,
                minimum_max_workers,
            } => write!(
                formatter,
                "missing parallel summary for expected {} worker use with at least {minimum_max_workers} workers",
                scope.as_str()
            ),
            Self::ExpectedParallelWorkerCountBelowMinimum {
                scope,
                minimum_max_workers,
                actual_max_workers,
            } => write!(
                formatter,
                "expected {} worker use to reach at least {minimum_max_workers} workers, got {actual_max_workers}",
                scope.as_str()
            ),
            Self::MissingParallelWorkerActivitySummary {
                scope,
                minimum_total_workers,
            } => write!(
                formatter,
                "missing parallel summary for expected {} worker activity with at least {minimum_total_workers} total workers",
                scope.as_str()
            ),
            Self::ExpectedParallelWorkerActivityBelowMinimum {
                scope,
                minimum_total_workers,
                actual_total_workers,
            } => write!(
                formatter,
                "expected {} worker activity to reach at least {minimum_total_workers} total workers, got {actual_total_workers}",
                scope.as_str()
            ),
            Self::ZeroExpectedDataCacheProtocolRunCount { protocol } => write!(
                formatter,
                "expected {} data-cache protocol run count must be positive",
                protocol.as_str()
            ),
            Self::DuplicateExpectedDataCacheProtocolRunCount { protocol } => write!(
                formatter,
                "expected {} data-cache protocol run count is already declared",
                protocol.as_str()
            ),
            Self::DuplicateExpectedDataCacheRunAttribution => write!(
                formatter,
                "expected data-cache run attribution is already declared"
            ),
            Self::MissingDataCacheProtocolSummary {
                protocol,
                minimum_run_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} data-cache protocol with at least {minimum_run_count} runs",
                protocol.as_str()
            ),
            Self::ExpectedDataCacheProtocolRunCountBelowMinimum {
                protocol,
                minimum_run_count,
                actual_run_count,
            } => write!(
                formatter,
                "expected {} data-cache protocol to run at least {minimum_run_count} times, got {actual_run_count}",
                protocol.as_str()
            ),
            Self::MissingDataCacheRunAttributionSummary {
                minimum_attributed_run_count,
                maximum_unattributed_run_count,
            } => write!(
                formatter,
                "missing parallel summary for expected data-cache run attribution with at least {minimum_attributed_run_count} attributed runs and at most {maximum_unattributed_run_count} unattributed runs"
            ),
            Self::ExpectedDataCacheRunAttributionBelowMinimum {
                minimum_attributed_run_count,
                actual_attributed_run_count,
            } => write!(
                formatter,
                "expected data-cache run attribution to reach at least {minimum_attributed_run_count} attributed runs, got {actual_attributed_run_count}"
            ),
            Self::ExpectedDataCacheRunAttributionAboveMaximum {
                maximum_unattributed_run_count,
                actual_unattributed_run_count,
            } => write!(
                formatter,
                "expected data-cache run attribution to keep unattributed runs at or below {maximum_unattributed_run_count}, got {actual_unattributed_run_count}"
            ),
            Self::DataCacheRunAccountingMismatch {
                data_cache_parallel_run_count,
                attributed_run_count,
                unattributed_run_count,
            } => write!(
                formatter,
                "data-cache run accounting mismatch: total {data_cache_parallel_run_count}, attributed {attributed_run_count}, unattributed {unattributed_run_count}"
            ),
            Self::DataCacheProtocolAccountingMismatch {
                attributed_run_count,
                protocol_run_count,
            } => write!(
                formatter,
                "data-cache protocol accounting mismatch: attributed {attributed_run_count}, protocol-count total {protocol_run_count}"
            ),
            Self::ZeroExpectedParallelSchedulerProgress { scope } => write!(
                formatter,
                "expected {} scheduler progress must require a positive epoch or dispatch count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelSchedulerProgress { scope } => write!(
                formatter,
                "expected {} scheduler progress is already declared",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelSchedulerIdleBound { scope } => write!(
                formatter,
                "expected {} scheduler idle bound is already declared",
                scope.as_str()
            ),
            Self::MissingParallelSchedulerProgressSummary {
                scope,
                minimum_epoch_count,
                minimum_dispatch_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} scheduler progress with at least {minimum_epoch_count} epochs and {minimum_dispatch_count} dispatches",
                scope.as_str()
            ),
            Self::MissingParallelSchedulerIdleSummary {
                scope,
                maximum_empty_epoch_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} scheduler idle bound with at most {maximum_empty_epoch_count} empty epochs",
                scope.as_str()
            ),
            Self::ExpectedParallelSchedulerProgressBelowMinimum {
                scope,
                minimum_epoch_count,
                actual_epoch_count,
                minimum_dispatch_count,
                actual_dispatch_count,
            } => write!(
                formatter,
                "expected {} scheduler progress to reach at least {minimum_epoch_count} epochs and {minimum_dispatch_count} dispatches, got {actual_epoch_count} epochs and {actual_dispatch_count} dispatches",
                scope.as_str()
            ),
            Self::ExpectedParallelSchedulerIdleAboveMaximum {
                scope,
                maximum_empty_epoch_count,
                actual_empty_epoch_count,
            } => write!(
                formatter,
                "expected {} scheduler idle bound to allow at most {maximum_empty_epoch_count} empty epochs, got {actual_empty_epoch_count}",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelFrontier {
                scope,
                stage,
                partition,
            } => write!(
                formatter,
                "expected {} {} frontier for partition {partition} must require positive time",
                scope.as_str(),
                stage.as_str()
            ),
            Self::DuplicateExpectedParallelFrontier {
                scope,
                stage,
                partition,
            } => write!(
                formatter,
                "expected {} {} frontier for partition {partition} is already declared",
                scope.as_str(),
                stage.as_str()
            ),
            Self::MissingParallelFrontierSummary {
                scope,
                stage,
                partition,
                minimum_now,
                minimum_safe_until,
            } => write!(
                formatter,
                "missing parallel summary for expected {} {} frontier on partition {partition} with now at least {minimum_now} and safe-until at least {minimum_safe_until}",
                scope.as_str(),
                stage.as_str()
            ),
            Self::ExpectedParallelFrontierBelowMinimum {
                scope,
                stage,
                partition,
                minimum_now,
                actual_now,
                minimum_safe_until,
                actual_safe_until,
            } => {
                let actual_now = actual_now
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "none".to_string());
                let actual_safe_until = actual_safe_until
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "none".to_string());
                write!(
                    formatter,
                    "expected {} {} frontier on partition {partition} to reach now {minimum_now} and safe-until {minimum_safe_until}, got now {actual_now} and safe-until {actual_safe_until}",
                    scope.as_str(),
                    stage.as_str()
                )
            }
            Self::InvalidExpectedParallelBatchWorkerCount {
                scope,
                minimum_worker_count,
            } => write!(
                formatter,
                "expected {} batch activity must require at least 2 workers, got {minimum_worker_count}",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelBatchCount {
                scope,
                minimum_worker_count,
            } => write!(
                formatter,
                "expected {} batch activity with at least {minimum_worker_count} workers must require a positive batch count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelBatchActivity {
                scope,
                minimum_worker_count,
            } => write!(
                formatter,
                "expected {} batch activity with at least {minimum_worker_count} workers is already declared",
                scope.as_str()
            ),
            Self::MissingParallelBatchActivitySummary {
                scope,
                minimum_worker_count,
                minimum_batch_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} batch activity with at least {minimum_batch_count} batches at {minimum_worker_count} workers",
                scope.as_str()
            ),
            Self::ExpectedParallelBatchActivityBelowMinimum {
                scope,
                minimum_worker_count,
                minimum_batch_count,
                actual_batch_count,
            } => write!(
                formatter,
                "expected {} batch activity to reach at least {minimum_batch_count} batches at {minimum_worker_count} workers, got {actual_batch_count}",
                scope.as_str()
            ),
            Self::InvalidExpectedParallelBatchPartitionSet { scope, partitions } => write!(
                formatter,
                "expected {} batch partition set {} must include at least 2 partitions",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::ZeroExpectedParallelBatchPartitionSetCount { scope, partitions } => write!(
                formatter,
                "expected {} batch partition set {} must require a positive batch count",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::DuplicateExpectedParallelBatchPartitionSet { scope, partitions } => write!(
                formatter,
                "expected {} batch partition set {} is already declared",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::MissingParallelBatchPartitionSetSummary {
                scope,
                partitions,
                minimum_batch_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} batch partition set {} with at least {minimum_batch_count} batches",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::ExpectedParallelBatchPartitionSetBelowMinimum {
                scope,
                partitions,
                minimum_batch_count,
                actual_batch_count,
            } => write!(
                formatter,
                "expected {} batch partition set {} to reach at least {minimum_batch_count} batches, got {actual_batch_count}",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::InvalidExpectedParallelBatchPartitionStreak { scope, partitions } => write!(
                formatter,
                "expected {} batch partition streak {} must include at least 2 partitions",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::ZeroExpectedParallelBatchPartitionStreakCount { scope, partitions } => write!(
                formatter,
                "expected {} batch partition streak {} must require a positive consecutive batch count",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::DuplicateExpectedParallelBatchPartitionStreak { scope, partitions } => write!(
                formatter,
                "expected {} batch partition streak {} is already declared",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::MissingParallelBatchPartitionStreakSummary {
                scope,
                partitions,
                minimum_consecutive_batch_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} batch partition streak {} with at least {minimum_consecutive_batch_count} consecutive batches",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::ExpectedParallelBatchPartitionStreakBelowMinimum {
                scope,
                partitions,
                minimum_consecutive_batch_count,
                actual_consecutive_batch_count,
            } => write!(
                formatter,
                "expected {} batch partition streak {} to reach at least {minimum_consecutive_batch_count} consecutive batches, got {actual_consecutive_batch_count}",
                scope.as_str(),
                format_partition_indexes(partitions)
            ),
            Self::ZeroExpectedParallelPartitionCount { scope } => write!(
                formatter,
                "expected {} partition use must require a positive active partition count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelPartitionUse { scope } => write!(
                formatter,
                "expected {} partition use is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelPartitionActivity { scope, partition } => write!(
                formatter,
                "expected {} partition {partition} activity must require at least one positive activity count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelPartitionActivity { scope, partition } => write!(
                formatter,
                "expected {} partition {partition} activity is already declared",
                scope.as_str()
            ),
            Self::MissingParallelPartitionSummary {
                scope,
                minimum_active_partitions,
            } => write!(
                formatter,
                "missing parallel summary for expected {} partition use with at least {minimum_active_partitions} active partitions",
                scope.as_str()
            ),
            Self::ExpectedParallelPartitionCountBelowMinimum {
                scope,
                minimum_active_partitions,
                actual_active_partitions,
            } => write!(
                formatter,
                "expected {} partition use to reach at least {minimum_active_partitions} active partitions, got {actual_active_partitions}",
                scope.as_str()
            ),
            Self::MissingParallelPartitionActivitySummary { scope, partition } => write!(
                formatter,
                "missing parallel summary for expected {} partition {partition} activity",
                scope.as_str()
            ),
            Self::ExpectedParallelPartitionActivityBelowMinimum {
                scope,
                partition,
                minimum_worker_count,
                actual_worker_count,
                minimum_dispatch_count,
                actual_dispatch_count,
                minimum_remote_send_count,
                actual_remote_send_count,
                minimum_remote_receive_count,
                actual_remote_receive_count,
            } => write!(
                formatter,
                "expected {} partition {partition} activity to reach workers {minimum_worker_count}, dispatches {minimum_dispatch_count}, remote sends {minimum_remote_send_count}, and remote receives {minimum_remote_receive_count}; got workers {actual_worker_count}, dispatches {actual_dispatch_count}, remote sends {actual_remote_send_count}, and remote receives {actual_remote_receive_count}",
                scope.as_str()
            ),
            Self::DuplicateExpectedCleanParallelDiagnostics { scope } => write!(
                formatter,
                "expected {} clean parallel diagnostics is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedLivelockTransitionThreshold { scope } => write!(
                formatter,
                "expected {} livelock transition threshold must be positive",
                scope.as_str()
            ),
            Self::ZeroExpectedResourceActivity { scope } => write!(
                formatter,
                "expected {} resource activity must require a positive operation or active resource count",
                scope.as_str()
            ),
            Self::DuplicateExpectedResourceActivity { scope } => write!(
                formatter,
                "expected {} resource activity is already declared",
                scope.as_str()
            ),
            Self::MissingResourceActivitySummary {
                scope,
                minimum_operation_count,
                minimum_active_resource_count,
            } => write!(
                formatter,
                "missing parallel summary for expected {} resource activity with at least {minimum_operation_count} operations and {minimum_active_resource_count} active resources",
                scope.as_str()
            ),
            Self::ExpectedResourceActivityBelowMinimum {
                scope,
                minimum_operation_count,
                actual_operation_count,
                minimum_active_resource_count,
                actual_active_resource_count,
            } => write!(
                formatter,
                "expected {} resource activity to reach at least {minimum_operation_count} operations and {minimum_active_resource_count} active resources, got {actual_operation_count} operations and {actual_active_resource_count} active resources",
                scope.as_str()
            ),
            Self::MissingParallelDiagnosticSummary { scope } => write!(
                formatter,
                "missing parallel summary for expected clean {} diagnostics",
                scope.as_str()
            ),
            Self::ExpectedCleanParallelDiagnosticsViolation {
                scope,
                wait_for_edge_count,
                deadlock_diagnostic_count,
                livelock_diagnostic_count,
            } => write!(
                formatter,
                "expected clean {} diagnostics, got {wait_for_edge_count} wait-for edges, {deadlock_diagnostic_count} deadlock diagnostics, and {livelock_diagnostic_count} livelock diagnostics",
                scope.as_str()
            ),
        }
    }
}

fn format_partition_indexes(partitions: &[u32]) -> String {
    let values = partitions
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

impl Error for WorkloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Boot(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}
