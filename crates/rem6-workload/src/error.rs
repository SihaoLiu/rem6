use std::error::Error;
use std::fmt;

use crate::WorkloadError;

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
            Self::DuplicateExpectedCleanParallelDiagnostics { scope } => write!(
                formatter,
                "expected {} clean parallel diagnostics is already declared",
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
            } => write!(
                formatter,
                "expected clean {} diagnostics, got {wait_for_edge_count} wait-for edges and {deadlock_diagnostic_count} deadlock diagnostics",
                scope.as_str()
            ),
        }
    }
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
