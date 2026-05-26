use std::collections::BTreeMap;

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorEngine,
    AcceleratorEngineConfig, AcceleratorEngineId, AcceleratorEngineSnapshot,
    AcceleratorWaitForMarker,
};
use rem6_gpu::{
    GpuComputeConfig, GpuDevice, GpuDeviceId, GpuDeviceSnapshot, GpuKernelId, GpuKernelLaunch,
    GpuWaitForMarker,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, WaitForEdgeKind, WaitForGraph};
use rem6_workload::{
    WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind, WorkloadAcceleratorDevice,
    WorkloadError, WorkloadGpuDevice, WorkloadMemoryRoute, WorkloadTopology,
};

use crate::workload_replay::RiscvWorkloadReplayError;

#[derive(Clone)]
pub(crate) struct WorkloadGpuRuntime {
    pub(crate) gpu: GpuDevice,
    pub(crate) source_partition: PartitionId,
    pub(crate) submission_latency: u64,
}

impl WorkloadGpuRuntime {
    fn new(gpu: GpuDevice, source_partition: PartitionId, submission_latency: u64) -> Self {
        Self {
            gpu,
            source_partition,
            submission_latency,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkloadGpuActivity {
    pub(crate) kernel_launch_count: usize,
    pub(crate) trace_event_count: usize,
    pub(crate) workgroup_completion_count: usize,
    pub(crate) active_device_count: usize,
    pub(crate) wait_for_edge_count: usize,
    pub(crate) wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    pub(crate) deadlock_diagnostic_count: usize,
}

impl WorkloadGpuActivity {
    pub(crate) fn from_snapshots(
        kernel_launch_count: usize,
        before: &BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
        after: &BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
    ) -> Self {
        let mut activity = Self {
            kernel_launch_count,
            ..Self::default()
        };

        for (device, after) in after {
            let Some(before) = before.get(device) else {
                continue;
            };
            let trace_event_count = after.trace().len().saturating_sub(before.trace().len());
            let workgroup_completion_count = after
                .completions()
                .len()
                .saturating_sub(before.completions().len());
            if trace_event_count != 0 || workgroup_completion_count != 0 {
                activity.active_device_count += 1;
            }
            activity.trace_event_count += trace_event_count;
            activity.workgroup_completion_count += workgroup_completion_count;
        }

        activity
    }

    pub(crate) fn with_wait_for_graph(mut self, wait_for: WaitForGraph) -> Self {
        self.wait_for_edge_count = wait_for.edge_count();
        self.wait_for_edge_kind_counts = wait_for.snapshot().edge_kind_counts();
        self.deadlock_diagnostic_count = wait_for.deadlock_diagnostic().into_iter().count();
        self
    }
}

#[derive(Clone)]
pub(crate) struct WorkloadAcceleratorRuntime {
    pub(crate) engine: AcceleratorEngine,
    pub(crate) source_partition: PartitionId,
    pub(crate) submission_latency: u64,
}

impl WorkloadAcceleratorRuntime {
    fn new(
        engine: AcceleratorEngine,
        source_partition: PartitionId,
        submission_latency: u64,
    ) -> Self {
        Self {
            engine,
            source_partition,
            submission_latency,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkloadAcceleratorActivity {
    pub(crate) command_count: usize,
    pub(crate) gpu_kernel_command_count: usize,
    pub(crate) npu_inference_command_count: usize,
    pub(crate) dma_command_count: usize,
    pub(crate) trace_event_count: usize,
    pub(crate) completion_count: usize,
    pub(crate) gpu_kernel_completion_count: usize,
    pub(crate) npu_inference_completion_count: usize,
    pub(crate) dma_command_completion_count: usize,
    pub(crate) active_device_count: usize,
    pub(crate) wait_for_edge_count: usize,
    pub(crate) wait_for_edge_kind_counts: BTreeMap<WaitForEdgeKind, usize>,
    pub(crate) deadlock_diagnostic_count: usize,
}

impl WorkloadAcceleratorActivity {
    pub(crate) fn from_snapshots(
        command_count: usize,
        command_kind_counts: WorkloadAcceleratorCommandKindCounts,
        before: &BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
        after: &BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
    ) -> Self {
        let mut activity = Self {
            command_count,
            gpu_kernel_command_count: command_kind_counts.gpu_kernel_count,
            npu_inference_command_count: command_kind_counts.npu_inference_count,
            dma_command_count: command_kind_counts.dma_count,
            ..Self::default()
        };

        for (engine, after) in after {
            let Some(before) = before.get(engine) else {
                continue;
            };
            let trace_event_count = after.trace().len().saturating_sub(before.trace().len());
            let completion_count = after
                .completed()
                .len()
                .saturating_sub(before.completed().len());
            if trace_event_count != 0 || completion_count != 0 {
                activity.active_device_count += 1;
            }
            activity.trace_event_count += trace_event_count;
            activity.completion_count += completion_count;
            for completion in after.completed().iter().skip(before.completed().len()) {
                match completion.kind() {
                    AcceleratorCommandKind::GpuKernel { .. } => {
                        activity.gpu_kernel_completion_count += 1;
                    }
                    AcceleratorCommandKind::NpuInference { .. } => {
                        activity.npu_inference_completion_count += 1;
                    }
                    AcceleratorCommandKind::DmaCopy { .. } => {
                        activity.dma_command_completion_count += 1;
                    }
                }
            }
        }

        activity
    }

    pub(crate) fn with_wait_for_graph(mut self, wait_for: WaitForGraph) -> Self {
        self.wait_for_edge_count = wait_for.edge_count();
        self.wait_for_edge_kind_counts = wait_for.snapshot().edge_kind_counts();
        self.deadlock_diagnostic_count = wait_for.deadlock_diagnostic().into_iter().count();
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkloadAcceleratorCommandKindCounts {
    pub(crate) gpu_kernel_count: usize,
    pub(crate) npu_inference_count: usize,
    pub(crate) dma_count: usize,
}

pub(crate) fn accelerator_command_kind_counts(
    commands: &[WorkloadAcceleratorCommand],
) -> WorkloadAcceleratorCommandKindCounts {
    let mut counts = WorkloadAcceleratorCommandKindCounts::default();
    for command in commands {
        match command.kind() {
            WorkloadAcceleratorCommandKind::GpuKernel { .. } => {
                counts.gpu_kernel_count += 1;
            }
            WorkloadAcceleratorCommandKind::NpuInference { .. } => {
                counts.npu_inference_count += 1;
            }
            WorkloadAcceleratorCommandKind::DmaCopy { .. } => {
                counts.dma_count += 1;
            }
        }
    }
    counts
}

pub(crate) fn build_gpu_devices(
    topology: &WorkloadTopology,
) -> Result<BTreeMap<GpuDeviceId, WorkloadGpuRuntime>, RiscvWorkloadReplayError> {
    let mut devices = BTreeMap::new();
    for device in topology.gpu_devices() {
        let route = gpu_command_route(topology, device)?;
        let gpu = GpuDevice::new(
            GpuComputeConfig::new(
                GpuDeviceId::new(device.device()),
                PartitionId::new(device.partition()),
                device.compute_units(),
                device.wave_slots_per_compute_unit(),
            )
            .map_err(RiscvWorkloadReplayError::Gpu)?,
        );
        devices.insert(
            GpuDeviceId::new(device.device()),
            WorkloadGpuRuntime::new(
                gpu,
                PartitionId::new(route.source_partition()),
                route.request_latency(),
            ),
        );
    }

    Ok(devices)
}

pub(crate) fn schedule_gpu_kernel_launches(
    topology: &WorkloadTopology,
    devices: &BTreeMap<GpuDeviceId, WorkloadGpuRuntime>,
    scheduler: &mut PartitionedScheduler,
) -> Result<usize, RiscvWorkloadReplayError> {
    let mut launch_count = 0;
    for launch in topology.gpu_kernel_launches() {
        let device = GpuDeviceId::new(launch.device());
        let runtime = devices.get(&device).ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingGpuDevice {
                device: launch.device(),
            })
        })?;
        runtime
            .gpu
            .submit_kernel_from_partition(
                scheduler,
                runtime.source_partition,
                runtime.submission_latency,
                GpuKernelLaunch::new(
                    GpuKernelId::new(launch.kernel()),
                    launch.workgroups(),
                    launch.workgroup_latency(),
                )
                .map_err(RiscvWorkloadReplayError::Gpu)?,
            )
            .map_err(RiscvWorkloadReplayError::Gpu)?;
        launch_count += 1;
    }

    Ok(launch_count)
}

pub(crate) fn gpu_snapshots(
    devices: &BTreeMap<GpuDeviceId, WorkloadGpuRuntime>,
) -> BTreeMap<GpuDeviceId, GpuDeviceSnapshot> {
    devices
        .iter()
        .map(|(device, runtime)| (*device, runtime.gpu.snapshot()))
        .collect()
}

pub(crate) fn gpu_wait_for_markers(
    devices: &BTreeMap<GpuDeviceId, WorkloadGpuRuntime>,
) -> BTreeMap<GpuDeviceId, GpuWaitForMarker> {
    devices
        .iter()
        .map(|(device, runtime)| (*device, runtime.gpu.mark_wait_for()))
        .collect()
}

pub(crate) fn gpu_wait_for_graph_since(
    devices: &BTreeMap<GpuDeviceId, WorkloadGpuRuntime>,
    markers: &BTreeMap<GpuDeviceId, GpuWaitForMarker>,
) -> WaitForGraph {
    let mut graph = WaitForGraph::new();
    for (device, marker) in markers {
        let Some(runtime) = devices.get(device) else {
            continue;
        };
        merge_wait_for_graph(&mut graph, runtime.gpu.wait_for_graph_since(*marker));
    }
    graph
}

pub(crate) fn build_accelerator_devices(
    topology: &WorkloadTopology,
) -> Result<BTreeMap<AcceleratorEngineId, WorkloadAcceleratorRuntime>, RiscvWorkloadReplayError> {
    let mut devices = BTreeMap::new();
    for device in topology.accelerator_devices() {
        let route = accelerator_command_route(topology, device)?;
        let engine = AcceleratorEngine::new(
            AcceleratorEngineConfig::new(
                AcceleratorEngineId::new(device.engine()),
                PartitionId::new(device.partition()),
                device.lanes(),
            )
            .map_err(RiscvWorkloadReplayError::Accelerator)?,
        );
        devices.insert(
            AcceleratorEngineId::new(device.engine()),
            WorkloadAcceleratorRuntime::new(
                engine,
                PartitionId::new(route.source_partition()),
                route.request_latency(),
            ),
        );
    }

    Ok(devices)
}

pub(crate) fn schedule_accelerator_commands(
    topology: &WorkloadTopology,
    devices: &BTreeMap<AcceleratorEngineId, WorkloadAcceleratorRuntime>,
    scheduler: &mut PartitionedScheduler,
) -> Result<usize, RiscvWorkloadReplayError> {
    let mut command_count = 0;
    for command in topology.accelerator_commands() {
        let engine = AcceleratorEngineId::new(command.engine());
        let runtime = devices.get(&engine).ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingAcceleratorDevice {
                engine: command.engine(),
            })
        })?;
        runtime
            .engine
            .submit_from_partition(
                scheduler,
                runtime.source_partition,
                runtime.submission_latency,
                accelerator_command(command)?,
            )
            .map_err(RiscvWorkloadReplayError::Accelerator)?;
        command_count += 1;
    }

    Ok(command_count)
}

pub(crate) fn accelerator_snapshots(
    devices: &BTreeMap<AcceleratorEngineId, WorkloadAcceleratorRuntime>,
) -> BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot> {
    devices
        .iter()
        .map(|(engine, runtime)| (*engine, runtime.engine.snapshot()))
        .collect()
}

pub(crate) fn accelerator_wait_for_markers(
    devices: &BTreeMap<AcceleratorEngineId, WorkloadAcceleratorRuntime>,
) -> BTreeMap<AcceleratorEngineId, AcceleratorWaitForMarker> {
    devices
        .iter()
        .map(|(engine, runtime)| (*engine, runtime.engine.mark_wait_for()))
        .collect()
}

pub(crate) fn accelerator_wait_for_graph_since(
    devices: &BTreeMap<AcceleratorEngineId, WorkloadAcceleratorRuntime>,
    markers: &BTreeMap<AcceleratorEngineId, AcceleratorWaitForMarker>,
) -> WaitForGraph {
    let mut graph = WaitForGraph::new();
    for (engine, marker) in markers {
        let Some(runtime) = devices.get(engine) else {
            continue;
        };
        merge_wait_for_graph(&mut graph, runtime.engine.wait_for_graph_since(*marker));
    }
    graph
}

fn gpu_command_route<'a>(
    topology: &'a WorkloadTopology,
    device: &WorkloadGpuDevice,
) -> Result<&'a WorkloadMemoryRoute, RiscvWorkloadReplayError> {
    topology
        .memory_routes()
        .iter()
        .find(|route| route.id() == device.command_route())
        .ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingGpuCommandRoute {
                device: device.device(),
                route: device.command_route().clone(),
            })
        })
}

fn accelerator_command_route<'a>(
    topology: &'a WorkloadTopology,
    device: &WorkloadAcceleratorDevice,
) -> Result<&'a WorkloadMemoryRoute, RiscvWorkloadReplayError> {
    topology
        .memory_routes()
        .iter()
        .find(|route| route.id() == device.command_route())
        .ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingAcceleratorCommandRoute {
                engine: device.engine(),
                route: device.command_route().clone(),
            })
        })
}

fn accelerator_command(
    command: &WorkloadAcceleratorCommand,
) -> Result<AcceleratorCommand, RiscvWorkloadReplayError> {
    AcceleratorCommand::new(
        AcceleratorCommandId::new(command.command()),
        accelerator_command_kind(command.kind()),
        command.execution_latency(),
    )
    .map_err(RiscvWorkloadReplayError::Accelerator)
}

fn accelerator_command_kind(kind: &WorkloadAcceleratorCommandKind) -> AcceleratorCommandKind {
    match kind {
        WorkloadAcceleratorCommandKind::GpuKernel { workgroups } => {
            AcceleratorCommandKind::GpuKernel {
                workgroups: *workgroups,
            }
        }
        WorkloadAcceleratorCommandKind::NpuInference { tiles } => {
            AcceleratorCommandKind::NpuInference { tiles: *tiles }
        }
        WorkloadAcceleratorCommandKind::DmaCopy { bytes } => {
            AcceleratorCommandKind::DmaCopy { bytes: *bytes }
        }
    }
}

pub(crate) fn merge_wait_for_graph(target: &mut WaitForGraph, source: WaitForGraph) {
    for edge in source.edges() {
        target
            .record_wait(
                edge.source().clone(),
                edge.target().clone(),
                edge.kind(),
                edge.first_observed_tick(),
            )
            .expect("merged wait-for graph already contains valid labels");
        if edge.last_observed_tick() != edge.first_observed_tick() {
            target
                .record_wait(
                    edge.source().clone(),
                    edge.target().clone(),
                    edge.kind(),
                    edge.last_observed_tick(),
                )
                .expect("merged wait-for graph already contains valid labels");
        }
    }
}
