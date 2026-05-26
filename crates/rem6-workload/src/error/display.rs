use std::fmt;

use crate::error_support::{
    format_remote_delay_error, format_remote_endpoint_error, format_remote_traffic_error,
};

use super::fabric_display::format_fabric_activity_error;
use super::WorkloadError;

mod parallel_batch;
mod parallel_frontier;

use self::parallel_batch::format_parallel_batch_error;
use self::parallel_frontier::format_parallel_frontier_error;

impl fmt::Display for WorkloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boot(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::EmptyWorkloadId => write!(formatter, "workload id must not be empty"),
            Self::EmptyWorkloadSuiteId => {
                write!(formatter, "workload suite id must not be empty")
            }
            Self::ZeroWorkloadSuiteWorkers => {
                write!(formatter, "workload suite worker count must be positive")
            }
            Self::ZeroSuiteParallelismRequirement => write!(
                formatter,
                "workload suite parallelism requirement must be positive"
            ),
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
            Self::WorkloadSuiteIdentityMismatch { expected, actual } => write!(
                formatter,
                "workload suite result belongs to suite {}, expected {}",
                actual.as_str(),
                expected.as_str()
            ),
            Self::DuplicateSuiteWorkload { workload } => write!(
                formatter,
                "workload suite contains duplicate workload {}",
                workload.as_str()
            ),
            Self::DuplicateSuiteManifest { manifest } => write!(
                formatter,
                "workload suite contains duplicate manifest {}",
                manifest.as_str()
            ),
            Self::DuplicateSuiteWorkloadResult { workload } => write!(
                formatter,
                "workload suite result contains duplicate workload {}",
                workload.as_str()
            ),
            Self::DuplicateSuiteDispatchCompletion { workload } => write!(
                formatter,
                "workload suite execution contains duplicate workload {}",
                workload.as_str()
            ),
            Self::DuplicateSuiteDispatchWeight { workload } => write!(
                formatter,
                "workload suite dispatch has duplicate weight for workload {}",
                workload.as_str()
            ),
            Self::MissingSuiteWorkloadResult { workload } => write!(
                formatter,
                "workload suite result is missing workload {}",
                workload.as_str()
            ),
            Self::MissingSuiteDispatchCompletion { workload } => write!(
                formatter,
                "workload suite execution is missing workload {}",
                workload.as_str()
            ),
            Self::MissingSuiteDispatchEstimate { workload } => write!(
                formatter,
                "workload suite dispatch is missing estimated ticks for workload {}",
                workload.as_str()
            ),
            Self::MissingSuiteDispatchWeight { workload } => write!(
                formatter,
                "workload suite dispatch is missing weight for workload {}",
                workload.as_str()
            ),
            Self::UnexpectedSuiteWorkloadResult { workload } => write!(
                formatter,
                "workload suite result contains unexpected workload {}",
                workload.as_str()
            ),
            Self::UnexpectedSuiteDispatchCompletion { workload } => write!(
                formatter,
                "workload suite execution contains unexpected workload {}",
                workload.as_str()
            ),
            Self::UnexpectedSuiteDispatchWeight { workload } => write!(
                formatter,
                "workload suite dispatch has unexpected weight for workload {}",
                workload.as_str()
            ),
            Self::ZeroSuiteDispatchWeight { workload } => write!(
                formatter,
                "workload suite dispatch weight for workload {} must be positive",
                workload.as_str()
            ),
            Self::SuiteWorkloadResultManifestMismatch {
                workload,
                expected,
                actual,
            } => write!(
                formatter,
                "workload suite result for {} belongs to manifest {}, expected {}",
                workload.as_str(),
                actual.as_str(),
                expected.as_str()
            ),
            Self::SuiteDispatchOrderMismatch {
                workload,
                expected,
                actual,
            } => write!(
                formatter,
                "workload suite execution for {} used dispatch order {actual}, expected {expected}",
                workload.as_str()
            ),
            Self::SuiteDispatchWorkerMismatch {
                workload,
                expected,
                actual,
            } => write!(
                formatter,
                "workload suite execution for {} used worker {actual}, expected {expected}",
                workload.as_str()
            ),
            Self::SuiteDispatchWorkerCountMismatch { expected, actual } => write!(
                formatter,
                "workload suite dispatch load uses {actual} workers, expected {expected}"
            ),
            Self::SuiteDispatchTimelineWindowMismatch {
                workload,
                expected_start_tick,
                expected_final_tick,
                actual_start_tick,
                actual_final_tick,
            } => write!(
                formatter,
                "workload suite execution for {} used tick window {actual_start_tick}..{actual_final_tick}, expected {expected_start_tick}..{expected_final_tick}",
                workload.as_str()
            ),
            Self::SuiteDispatchCompletionWindowInvalid {
                workload,
                start_tick,
                final_tick,
            } => write!(
                formatter,
                "workload suite execution for {} has start tick {start_tick} after final tick {final_tick}",
                workload.as_str()
            ),
            Self::SuiteParallelismBelowMinimum {
                minimum_workers,
                actual_workers,
            } => write!(
                formatter,
                "workload suite execution reached {actual_workers} simultaneous workers, expected at least {minimum_workers}"
            ),
            Self::SuiteParallelismRequirementExceedsActiveWorkers {
                minimum_workers,
                active_workers,
            } => write!(
                formatter,
                "workload suite parallelism requirement needs {minimum_workers} simultaneous workers but dispatch uses {active_workers} active workers"
            ),
            Self::SuiteExecutionWorkerCountBelowActiveWorkers {
                worker_count,
                active_workers,
            } => write!(
                formatter,
                "workload suite execution efficiency used {worker_count} workers but execution touched {active_workers} active workers"
            ),
            Self::SuiteExecutionCapacityBelowCompletionTicks {
                worker_capacity_ticks,
                serial_completion_ticks,
            } => write!(
                formatter,
                "workload suite execution capacity {worker_capacity_ticks} is below serial completion ticks {serial_completion_ticks}"
            ),
            Self::SuiteParallelSpeedupBelowMinimum {
                minimum_numerator,
                minimum_denominator,
                actual_numerator,
                actual_denominator,
            } => write!(
                formatter,
                "workload suite execution speedup {actual_numerator}/{actual_denominator} is below minimum {minimum_numerator}/{minimum_denominator}"
            ),
            Self::SuiteWorkerUtilizationBelowMinimum {
                minimum_numerator,
                minimum_denominator,
                actual_numerator,
                actual_denominator,
            } => write!(
                formatter,
                "workload suite worker utilization {actual_numerator}/{actual_denominator} is below minimum {minimum_numerator}/{minimum_denominator}"
            ),
            Self::SuiteExecutionOccupancyWorkerCountTicksBelowMinimum {
                worker_count,
                minimum_ticks,
                actual_ticks,
            } => write!(
                formatter,
                "workload suite execution occupancy bucket {worker_count} has {actual_ticks} ticks, below {minimum_ticks}"
            ),
            Self::SuiteExecutionFullOccupancyTicksBelowMinimum {
                minimum_ticks,
                actual_ticks,
            } => write!(
                formatter,
                "workload suite execution full occupancy ticks {actual_ticks} is below minimum {minimum_ticks}"
            ),
            Self::SuiteExecutionUnderoccupiedTicksAboveMaximum {
                maximum_ticks,
                actual_ticks,
            } => write!(
                formatter,
                "workload suite execution underoccupied ticks {actual_ticks} is above maximum {maximum_ticks}"
            ),
            Self::SuitePlannedParallelSpeedupBelowMinimum {
                minimum_numerator,
                minimum_denominator,
                actual_numerator,
                actual_denominator,
            } => write!(
                formatter,
                "workload suite planned speedup {actual_numerator}/{actual_denominator} is below minimum {minimum_numerator}/{minimum_denominator}"
            ),
            Self::SuitePlannedWorkerUtilizationBelowMinimum {
                minimum_numerator,
                minimum_denominator,
                actual_numerator,
                actual_denominator,
            } => write!(
                formatter,
                "workload suite planned worker utilization {actual_numerator}/{actual_denominator} is below minimum {minimum_numerator}/{minimum_denominator}"
            ),
            Self::SuitePlannedFullOccupancyTicksBelowMinimum {
                minimum_ticks,
                actual_ticks,
            } => write!(
                formatter,
                "workload suite planned full occupancy ticks {actual_ticks} is below minimum {minimum_ticks}"
            ),
            Self::SuitePlannedOccupancyWorkerCountTicksBelowMinimum {
                worker_count,
                minimum_ticks,
                actual_ticks,
            } => write!(formatter, "planned occupancy bucket {worker_count} has {actual_ticks} ticks, below {minimum_ticks}"),
            Self::SuitePlannedUnderoccupiedTicksAboveMaximum {
                maximum_ticks,
                actual_ticks,
            } => write!(
                formatter,
                "workload suite planned underoccupied ticks {actual_ticks} is above maximum {maximum_ticks}"
            ),
            Self::ZeroSuiteExecutionRatioDenominator => write!(
                formatter,
                "workload suite execution ratio denominator must be positive"
            ),
            Self::StatsAfterFinalTick {
                stats_tick,
                final_tick,
            } => write!(
                formatter,
                "stats snapshot tick {stats_tick} is after final tick {final_tick}"
            ),
            Self::ResultStartAfterFinalTick {
                start_tick,
                final_tick,
            } => write!(
                formatter,
                "workload result start tick {start_tick} is after final tick {final_tick}"
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
            Self::ZeroExpectedParallelRemoteFlowCount { .. }
            | Self::DuplicateExpectedParallelRemoteFlow { .. }
            | Self::DuplicateExpectedParallelRemoteSend { .. }
            | Self::MissingParallelExecutionSummary { .. }
            | Self::ExpectedParallelRemoteFlowCountMismatch { .. }
            | Self::UnexpectedParallelRemoteFlow { .. }
            | Self::MissingParallelRemoteSendSummary { .. }
            | Self::ExpectedParallelRemoteSendMissing { .. }
            | Self::UnexpectedParallelRemoteSend { .. } => {
                format_remote_traffic_error(self, formatter)
            }
            Self::ParallelProgressTransitionExpectation(error) => write!(
                formatter,
                "{} {} progress transition on partition {} for {} kind {} at tick {} with order {}",
                error.failure().as_str(),
                error.scope().as_str(),
                error.partition().index(),
                error.subject(),
                error.kind().as_str(),
                error.tick(),
                error.order()
            ),
            Self::EmptyExpectedParallelRemoteEndpointSources { .. }
            | Self::EmptyExpectedParallelRemoteEndpointTargets { .. }
            | Self::DuplicateExpectedParallelRemoteEndpoints { .. }
            | Self::MissingParallelRemoteEndpointSummary { .. }
            | Self::ExpectedParallelRemoteEndpointsMismatch { .. } => {
                format_remote_endpoint_error(self, formatter)
            }
            Self::ZeroExpectedParallelRemoteDelayFloor { .. }
            | Self::DuplicateExpectedParallelRemoteDelayFloor { .. }
            | Self::DuplicateExpectedParallelRemoteDelayCeiling { .. }
            | Self::DuplicateExpectedParallelRemoteTrafficConsistency { .. }
            | Self::MissingParallelRemoteDelayFloorSummary { .. }
            | Self::MissingParallelRemoteDelayEvidence { .. }
            | Self::MissingParallelRemoteFlowDelayEvidence { .. }
            | Self::ExpectedParallelRemoteDelayBelowFloor { .. }
            | Self::MissingParallelRemoteDelayCeilingSummary { .. }
            | Self::MissingParallelRemoteDelayCeilingEvidence { .. }
            | Self::MissingParallelRemoteFlowMaximumDelayEvidence { .. }
            | Self::ExpectedParallelRemoteDelayAboveCeiling { .. }
            | Self::MissingParallelRemoteTrafficConsistencySummary { .. }
            | Self::ParallelRemoteTrafficConsistencyMismatch(_)
            | Self::InvalidExpectedParallelRemoteFlowTimingWindow { .. }
            | Self::InvalidExpectedParallelRemoteFlowDelayBounds { .. }
            | Self::DuplicateExpectedParallelRemoteFlowTiming { .. }
            | Self::MissingParallelRemoteFlowTimingSummary { .. }
            | Self::ExpectedParallelRemoteFlowTimingMismatch { .. }
            | Self::UnexpectedParallelRemoteFlowTiming { .. }
            | Self::ExpectedParallelRemoteFlowDelayBoundsMismatch { .. } => {
                format_remote_delay_error(self, formatter)
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
            Self::ZeroExpectedParallelFrontier { .. }
            | Self::DuplicateExpectedParallelFrontier { .. }
            | Self::MissingParallelFrontierSummary { .. }
            | Self::ExpectedParallelFrontierBelowMinimum { .. } => {
                format_parallel_frontier_error(self, formatter)
            }
            Self::InvalidExpectedParallelBatchWorkerCount { .. }
            | Self::ZeroExpectedParallelBatchCount { .. }
            | Self::DuplicateExpectedParallelBatchActivity { .. }
            | Self::MissingParallelBatchActivitySummary { .. }
            | Self::ExpectedParallelBatchActivityBelowMinimum { .. }
            | Self::ParallelBatchWorkerCountBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchWorkerBucket { .. }
            | Self::ZeroExpectedParallelBatchWorkerBucket { .. }
            | Self::DuplicateExpectedParallelBatchWorkerBucket { .. }
            | Self::MissingParallelBatchWorkerBucketSummary { .. }
            | Self::ExpectedParallelBatchWorkerBucketBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchWorkerTickBucket { .. }
            | Self::ZeroExpectedParallelBatchWorkerTickBucket { .. }
            | Self::DuplicateExpectedParallelBatchWorkerTickBucket { .. }
            | Self::MissingParallelBatchWorkerTickBucketSummary { .. }
            | Self::ExpectedParallelBatchWorkerTickBucketBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchWorkerTickActivity { .. }
            | Self::ZeroExpectedParallelBatchWorkerTickActivity { .. }
            | Self::DuplicateExpectedParallelBatchWorkerTickActivity { .. }
            | Self::MissingParallelBatchWorkerTickActivitySummary { .. }
            | Self::ExpectedParallelBatchWorkerTickActivityBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchWorkerTickStreak { .. }
            | Self::ZeroExpectedParallelBatchWorkerTickStreak { .. }
            | Self::DuplicateExpectedParallelBatchWorkerTickStreak { .. }
            | Self::MissingParallelBatchWorkerTickStreakSummary { .. }
            | Self::ExpectedParallelBatchWorkerTickStreakBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchWorkerTicks { .. }
            | Self::ZeroExpectedParallelBatchWorkerTicks { .. }
            | Self::DuplicateExpectedParallelBatchWorkerTicks { .. }
            | Self::MissingParallelBatchWorkerTicksSummary { .. }
            | Self::ExpectedParallelBatchWorkerTicksBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchPartitionSet { .. }
            | Self::ZeroExpectedParallelBatchPartitionSetCount { .. }
            | Self::DuplicateExpectedParallelBatchPartitionSet { .. }
            | Self::MissingParallelBatchPartitionSetSummary { .. }
            | Self::ExpectedParallelBatchPartitionSetBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchPartitionStreak { .. }
            | Self::ZeroExpectedParallelBatchPartitionStreakCount { .. }
            | Self::DuplicateExpectedParallelBatchPartitionStreak { .. }
            | Self::MissingParallelBatchPartitionStreakSummary { .. }
            | Self::ExpectedParallelBatchPartitionStreakBelowMinimum { .. }
            | Self::InvalidExpectedParallelBatchTimelineRecord { .. }
            | Self::DuplicateExpectedParallelBatchTimelineRecord { .. }
            | Self::MissingParallelBatchTimelineSummary { .. }
            | Self::ExpectedParallelBatchTimelineRecordMissing { .. }
            | Self::UnexpectedParallelBatchTimelineRecord { .. }
            | Self::ZeroExpectedParallelPartitionCount { .. }
            | Self::DuplicateExpectedParallelPartitionUse { .. }
            | Self::ZeroExpectedParallelPartitionActivity { .. }
            | Self::DuplicateExpectedParallelPartitionActivity { .. }
            | Self::MissingParallelPartitionSummary { .. }
            | Self::ExpectedParallelPartitionCountBelowMinimum { .. }
            | Self::MissingParallelPartitionActivitySummary { .. }
            | Self::ExpectedParallelPartitionActivityBelowMinimum { .. } => {
                format_parallel_batch_error(self, formatter)
            }
            Self::DuplicateExpectedCleanParallelDiagnostics { scope } => write!(
                formatter,
                "expected {} clean parallel diagnostics is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelWaitForEdgeKindCount { scope, kind } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} must require a positive edge count",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelWaitForEdgeKindCount { scope, kind } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelWaitForEdgeKindWindow { scope, kind } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} window must require a positive edge count",
                scope.as_str()
            ),
            Self::InvalidExpectedParallelWaitForEdgeKindWindow {
                scope,
                kind,
                first_tick,
                last_tick,
            } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} window first tick {first_tick} is after last tick {last_tick}",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelWaitForEdgeKindWindow { scope, kind } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} window is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelWaitForBlockedNodeWindow { scope, node } => write!(
                formatter,
                "expected {} wait-for blocked node {node} window must require a positive edge count",
                scope.as_str()
            ),
            Self::InvalidExpectedParallelWaitForBlockedNodeWindow {
                scope,
                node,
                first_tick,
                last_tick,
            } => write!(
                formatter,
                "expected {} wait-for blocked node {node} window first tick {first_tick} is after last tick {last_tick}",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelWaitForBlockedNodeWindow { scope, node } => write!(
                formatter,
                "expected {} wait-for blocked node {node} window is already declared",
                scope.as_str()
            ),
            Self::ZeroExpectedParallelWaitForTargetNodeWindow { scope, node } => write!(
                formatter,
                "expected {} wait-for target node {node} window must require a positive edge count",
                scope.as_str()
            ),
            Self::InvalidExpectedParallelWaitForTargetNodeWindow {
                scope,
                node,
                first_tick,
                last_tick,
            } => write!(
                formatter,
                "expected {} wait-for target node {node} window first tick {first_tick} is after last tick {last_tick}",
                scope.as_str()
            ),
            Self::DuplicateExpectedParallelWaitForTargetNodeWindow { scope, node } => write!(
                formatter,
                "expected {} wait-for target node {node} window is already declared",
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
            Self::ZeroExpectedFabricHopActivity { .. }
            | Self::DuplicateExpectedFabricHopActivity { .. }
            | Self::InvalidExpectedFabricHopActivityWindow { .. }
            | Self::MissingFabricHopActivitySummary { .. }
            | Self::ExpectedFabricHopActivityBelowMinimum { .. }
            | Self::ZeroExpectedFabricLaneActivity { .. }
            | Self::DuplicateExpectedFabricLaneActivity { .. }
            | Self::InvalidExpectedFabricLaneActivityWindow { .. }
            | Self::InvalidExpectedFabricLaneActivityQueueDelayBudget { .. }
            | Self::MissingFabricLaneActivitySummary { .. }
            | Self::ExpectedFabricLaneActivityBelowMinimum { .. }
            | Self::ExpectedFabricLaneActivityAboveMaximum { .. }
            | Self::ZeroExpectedFabricLinkActivity { .. }
            | Self::DuplicateExpectedFabricLinkActivity { .. }
            | Self::InvalidExpectedFabricLinkActivityWindow { .. }
            | Self::InvalidExpectedFabricLinkActivityQueueDelayBudget { .. }
            | Self::MissingFabricLinkActivitySummary { .. }
            | Self::ExpectedFabricLinkActivityBelowMinimum { .. }
            | Self::ExpectedFabricLinkActivityAboveMaximum { .. }
            | Self::ZeroExpectedFabricVirtualNetworkActivity { .. }
            | Self::DuplicateExpectedFabricVirtualNetworkActivity { .. }
            | Self::DuplicateExpectedFabricVirtualNetworkActivityCoverageLink { .. }
            | Self::InvalidExpectedFabricVirtualNetworkActivityWindow { .. }
            | Self::InvalidExpectedFabricVirtualNetworkActivityQueueDelayBudget { .. }
            | Self::InvalidExpectedFabricVirtualNetworkActivityLaneBudget { .. }
            | Self::MissingFabricVirtualNetworkActivitySummary { .. }
            | Self::MissingFabricVirtualNetworkLinkCoverage { .. }
            | Self::ExpectedFabricVirtualNetworkActivityBelowMinimum { .. }
            | Self::ExpectedFabricVirtualNetworkActivityAboveMaximum { .. }
            | Self::ExpectedFabricVirtualNetworkActivityAboveLaneBudget { .. }
            | Self::ExpectedFabricVirtualNetworkLinkCoverageMissing { .. } => {
                format_fabric_activity_error(self, formatter)
            }
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
                livelock_subjects,
            } => {
                write!(
                    formatter,
                    "expected clean {} diagnostics, got {wait_for_edge_count} wait-for edges, {deadlock_diagnostic_count} deadlock diagnostics, and {livelock_diagnostic_count} livelock diagnostics",
                    scope.as_str()
                )?;
                if !livelock_subjects.is_empty() {
                    write!(
                        formatter,
                        " for livelock subjects {}",
                        livelock_subjects.join(", ")
                    )?;
                }
                Ok(())
            }
            Self::ExpectedParallelWaitForEdgeKindCountBelowMinimum {
                scope,
                kind,
                minimum_edge_count,
                actual_edge_count,
            } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} to reach at least {minimum_edge_count} edges, got {actual_edge_count}",
                scope.as_str()
            ),
            Self::ExpectedParallelWaitForEdgeKindWindowMismatch {
                scope,
                kind,
                expected_edge_count,
                actual_edge_count,
                expected_first_tick,
                actual_first_tick,
                expected_last_tick,
                actual_last_tick,
            } => write!(
                formatter,
                "expected {} wait-for edge kind {kind:?} window to have {expected_edge_count} edges from tick {expected_first_tick} to {expected_last_tick}, got {actual_edge_count} edges from tick {} to {}",
                scope.as_str(),
                actual_first_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "missing".to_string()),
                actual_last_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "missing".to_string()),
            ),
            Self::ExpectedParallelWaitForBlockedNodeWindowMismatch {
                scope,
                node,
                expected_edge_count,
                actual_edge_count,
                expected_first_tick,
                actual_first_tick,
                expected_last_tick,
                actual_last_tick,
            } => write!(
                formatter,
                "expected {} wait-for blocked node {node} window to have {expected_edge_count} edges from tick {expected_first_tick} to {expected_last_tick}, got {actual_edge_count} edges from tick {} to {}",
                scope.as_str(),
                actual_first_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "missing".to_string()),
                actual_last_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "missing".to_string()),
            ),
            Self::ExpectedParallelWaitForTargetNodeWindowMismatch {
                scope,
                node,
                expected_edge_count,
                actual_edge_count,
                expected_first_tick,
                actual_first_tick,
                expected_last_tick,
                actual_last_tick,
            } => write!(
                formatter,
                "expected {} wait-for target node {node} window to have {expected_edge_count} edges from tick {expected_first_tick} to {expected_last_tick}, got {actual_edge_count} edges from tick {} to {}",
                scope.as_str(),
                actual_first_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "missing".to_string()),
                actual_last_tick
                    .map(|tick| tick.to_string())
                    .unwrap_or_else(|| "missing".to_string()),
            ),
        }
    }
}
