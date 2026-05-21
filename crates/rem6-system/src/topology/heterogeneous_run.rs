use std::collections::BTreeMap;

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorEngineId, AcceleratorEngineSnapshot, AcceleratorError,
    AcceleratorTopologyDevice, AcceleratorWaitForMarker,
};
use rem6_gpu::{
    GpuDeviceId, GpuDeviceSnapshot, GpuKernelLaunch, GpuTopologyDevice, GpuWaitForMarker,
};
use rem6_kernel::{
    ParallelPartitionActivity, ParallelRunProfile, PartitionEventId, PartitionId,
    RecordedConservativeRunSummary, Tick, WaitForEdge, WaitForEdgeKind, WaitForGraph,
};

use super::{RiscvTopologySystem, RiscvTopologySystemError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologyHeterogeneousWork {
    Accelerator {
        engine: AcceleratorEngineId,
        command: AcceleratorCommand,
    },
    Gpu {
        device: GpuDeviceId,
        launch: GpuKernelLaunch,
    },
}

impl RiscvTopologyHeterogeneousWork {
    pub const fn accelerator(engine: AcceleratorEngineId, command: AcceleratorCommand) -> Self {
        Self::Accelerator { engine, command }
    }

    pub const fn gpu(device: GpuDeviceId, launch: GpuKernelLaunch) -> Self {
        Self::Gpu { device, launch }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyHeterogeneousRunSummary {
    events: Vec<PartitionEventId>,
    scheduler_run: RecordedConservativeRunSummary,
    gpu_trace_event_count: usize,
    gpu_workgroup_completion_count: usize,
    accelerator_trace_event_count: usize,
    accelerator_command_completion_count: usize,
    gpu_activity: BTreeMap<GpuDeviceId, RiscvTopologyGpuComputeActivity>,
    accelerator_activity: BTreeMap<AcceleratorEngineId, RiscvTopologyAcceleratorComputeActivity>,
    gpu_wait_for: BTreeMap<GpuDeviceId, WaitForGraph>,
    accelerator_wait_for: BTreeMap<AcceleratorEngineId, WaitForGraph>,
}

impl RiscvTopologyHeterogeneousRunSummary {
    pub fn new(
        events: Vec<PartitionEventId>,
        scheduler_run: RecordedConservativeRunSummary,
        gpu_trace_event_count: usize,
        gpu_workgroup_completion_count: usize,
        accelerator_trace_event_count: usize,
        accelerator_command_completion_count: usize,
    ) -> Self {
        Self {
            events,
            scheduler_run,
            gpu_trace_event_count,
            gpu_workgroup_completion_count,
            accelerator_trace_event_count,
            accelerator_command_completion_count,
            gpu_activity: BTreeMap::new(),
            accelerator_activity: BTreeMap::new(),
            gpu_wait_for: BTreeMap::new(),
            accelerator_wait_for: BTreeMap::new(),
        }
    }

    pub fn with_device_activity(
        mut self,
        gpu_activity: BTreeMap<GpuDeviceId, RiscvTopologyGpuComputeActivity>,
        accelerator_activity: BTreeMap<
            AcceleratorEngineId,
            RiscvTopologyAcceleratorComputeActivity,
        >,
    ) -> Self {
        self.gpu_activity = gpu_activity;
        self.accelerator_activity = accelerator_activity;
        self
    }

    pub fn with_device_wait_for(
        mut self,
        gpu_wait_for: BTreeMap<GpuDeviceId, WaitForGraph>,
        accelerator_wait_for: BTreeMap<AcceleratorEngineId, WaitForGraph>,
    ) -> Self {
        self.gpu_wait_for = gpu_wait_for;
        self.accelerator_wait_for = accelerator_wait_for;
        self
    }

    pub fn events(&self) -> &[PartitionEventId] {
        &self.events
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub const fn scheduler_run(&self) -> &RecordedConservativeRunSummary {
        &self.scheduler_run
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.scheduler_run.profile()
    }

    pub fn epoch_count(&self) -> usize {
        self.scheduler_run.epoch_count()
    }

    pub fn empty_epoch_count(&self) -> usize {
        self.scheduler_run.empty_epoch_count()
    }

    pub fn dispatch_count(&self) -> usize {
        self.scheduler_run.dispatch_count()
    }

    pub fn batch_count(&self) -> usize {
        self.scheduler_run.batch_count()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.scheduler_run.total_parallel_workers()
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.scheduler_run.max_parallel_workers()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.scheduler_run.has_parallel_work()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.scheduler_run.partition_activity(partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.scheduler_run.has_partition_activity(partition)
    }

    pub fn active_partition_count(&self) -> usize {
        self.scheduler_run.active_partition_count()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        self.scheduler_run.partition_activities()
    }

    pub fn executed_events(&self) -> usize {
        self.scheduler_run.summary().executed_events()
    }

    pub fn final_tick(&self) -> Tick {
        self.scheduler_run.summary().final_tick()
    }

    pub const fn gpu_trace_event_count(&self) -> usize {
        self.gpu_trace_event_count
    }

    pub const fn gpu_workgroup_completion_count(&self) -> usize {
        self.gpu_workgroup_completion_count
    }

    pub const fn accelerator_trace_event_count(&self) -> usize {
        self.accelerator_trace_event_count
    }

    pub const fn accelerator_command_completion_count(&self) -> usize {
        self.accelerator_command_completion_count
    }

    pub fn gpu_activity(&self, device: GpuDeviceId) -> Option<RiscvTopologyGpuComputeActivity> {
        self.gpu_activity.get(&device).copied()
    }

    pub fn gpu_activities(&self) -> &BTreeMap<GpuDeviceId, RiscvTopologyGpuComputeActivity> {
        &self.gpu_activity
    }

    pub fn accelerator_activity(
        &self,
        engine: AcceleratorEngineId,
    ) -> Option<RiscvTopologyAcceleratorComputeActivity> {
        self.accelerator_activity.get(&engine).copied()
    }

    pub fn accelerator_activities(
        &self,
    ) -> &BTreeMap<AcceleratorEngineId, RiscvTopologyAcceleratorComputeActivity> {
        &self.accelerator_activity
    }

    pub const fn trace_event_count(&self) -> usize {
        self.gpu_trace_event_count + self.accelerator_trace_event_count
    }

    pub const fn compute_completion_count(&self) -> usize {
        self.gpu_workgroup_completion_count + self.accelerator_command_completion_count
    }

    pub const fn has_gpu_activity(&self) -> bool {
        self.gpu_trace_event_count != 0 || self.gpu_workgroup_completion_count != 0
    }

    pub const fn has_accelerator_activity(&self) -> bool {
        self.accelerator_trace_event_count != 0 || self.accelerator_command_completion_count != 0
    }

    pub const fn has_compute_activity(&self) -> bool {
        self.compute_completion_count() != 0
    }

    pub fn gpu_wait_for_edges(&self, device: GpuDeviceId) -> Vec<WaitForEdge> {
        self.gpu_wait_for
            .get(&device)
            .map(WaitForGraph::edges)
            .unwrap_or_default()
    }

    pub fn gpu_wait_for_edge_count(&self, device: GpuDeviceId) -> usize {
        self.gpu_wait_for
            .get(&device)
            .map(WaitForGraph::edge_count)
            .unwrap_or(0)
    }

    pub fn gpu_wait_for_edge_count_by_kind(
        &self,
        device: GpuDeviceId,
        kind: WaitForEdgeKind,
    ) -> usize {
        self.gpu_wait_for_edges(device)
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn has_gpu_wait_for_edges(&self) -> bool {
        self.gpu_wait_for.values().any(|graph| !graph.is_empty())
    }

    pub fn accelerator_wait_for_edges(&self, engine: AcceleratorEngineId) -> Vec<WaitForEdge> {
        self.accelerator_wait_for
            .get(&engine)
            .map(WaitForGraph::edges)
            .unwrap_or_default()
    }

    pub fn accelerator_wait_for_edge_count(&self, engine: AcceleratorEngineId) -> usize {
        self.accelerator_wait_for
            .get(&engine)
            .map(WaitForGraph::edge_count)
            .unwrap_or(0)
    }

    pub fn accelerator_wait_for_edge_count_by_kind(
        &self,
        engine: AcceleratorEngineId,
        kind: WaitForEdgeKind,
    ) -> usize {
        self.accelerator_wait_for_edges(engine)
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn has_accelerator_wait_for_edges(&self) -> bool {
        self.accelerator_wait_for
            .values()
            .any(|graph| !graph.is_empty())
    }

    pub fn compute_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.gpu_wait_for
            .values()
            .chain(self.accelerator_wait_for.values())
            .flat_map(WaitForGraph::edges)
            .collect()
    }

    pub fn compute_wait_for_edge_count(&self) -> usize {
        self.gpu_wait_for
            .values()
            .chain(self.accelerator_wait_for.values())
            .map(WaitForGraph::edge_count)
            .sum()
    }

    pub fn compute_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.compute_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn has_compute_wait_for_edges(&self) -> bool {
        self.compute_wait_for_edge_count() != 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvTopologyGpuComputeActivity {
    trace_event_count: usize,
    workgroup_completion_count: usize,
}

impl RiscvTopologyGpuComputeActivity {
    pub const fn new(trace_event_count: usize, workgroup_completion_count: usize) -> Self {
        Self {
            trace_event_count,
            workgroup_completion_count,
        }
    }

    pub const fn trace_event_count(self) -> usize {
        self.trace_event_count
    }

    pub const fn workgroup_completion_count(self) -> usize {
        self.workgroup_completion_count
    }

    pub const fn has_compute_activity(self) -> bool {
        self.workgroup_completion_count != 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvTopologyAcceleratorComputeActivity {
    trace_event_count: usize,
    command_completion_count: usize,
}

impl RiscvTopologyAcceleratorComputeActivity {
    pub const fn new(trace_event_count: usize, command_completion_count: usize) -> Self {
        Self {
            trace_event_count,
            command_completion_count,
        }
    }

    pub const fn trace_event_count(self) -> usize {
        self.trace_event_count
    }

    pub const fn command_completion_count(self) -> usize {
        self.command_completion_count
    }

    pub const fn has_compute_activity(self) -> bool {
        self.command_completion_count != 0
    }
}

#[derive(Clone, Debug)]
enum ResolvedHeterogeneousWork {
    Accelerator {
        engine: AcceleratorEngineId,
        device: AcceleratorTopologyDevice,
        command: AcceleratorCommand,
    },
    Gpu {
        device_id: GpuDeviceId,
        device: GpuTopologyDevice,
        launch: GpuKernelLaunch,
    },
}

#[derive(Clone, Debug)]
struct HeterogeneousDeviceSnapshots {
    accelerators: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
    gpus: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HeterogeneousActivityCounts {
    gpu_trace_event_count: usize,
    gpu_workgroup_completion_count: usize,
    accelerator_trace_event_count: usize,
    accelerator_command_completion_count: usize,
    gpu_activity: BTreeMap<GpuDeviceId, RiscvTopologyGpuComputeActivity>,
    accelerator_activity: BTreeMap<AcceleratorEngineId, RiscvTopologyAcceleratorComputeActivity>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HeterogeneousWaitForGraphs {
    accelerators: BTreeMap<AcceleratorEngineId, WaitForGraph>,
    gpus: BTreeMap<GpuDeviceId, WaitForGraph>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HeterogeneousWaitForMarkers {
    accelerators: BTreeMap<AcceleratorEngineId, AcceleratorWaitForMarker>,
    gpus: BTreeMap<GpuDeviceId, GpuWaitForMarker>,
}

impl RiscvTopologySystem {
    pub fn run_heterogeneous_work_parallel_recorded<I>(
        &mut self,
        work: I,
    ) -> Result<RiscvTopologyHeterogeneousRunSummary, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyHeterogeneousWork>,
    {
        let work: Vec<_> = work.into_iter().collect();
        if work.is_empty() {
            return Ok(self.empty_heterogeneous_run_summary());
        }

        let work = self.resolve_heterogeneous_work(work)?;
        let before = self.heterogeneous_device_snapshots(&work);
        let wait_markers = self.heterogeneous_wait_for_markers(&work);
        let mut scheduler = self.scheduler_mut();
        let mut events = Vec::with_capacity(work.len());

        for item in &work {
            let event = match &item {
                ResolvedHeterogeneousWork::Accelerator {
                    device, command, ..
                } => device
                    .submit_command(&mut scheduler, command.clone())
                    .map_err(RiscvTopologySystemError::Accelerator)?,
                ResolvedHeterogeneousWork::Gpu { device, launch, .. } => device
                    .submit_kernel(&mut scheduler, launch.clone())
                    .map_err(RiscvTopologySystemError::Gpu)?,
            };
            events.push(event);
        }

        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        drop(scheduler);

        let activity = self.heterogeneous_activity_since(&before);
        let wait_for = self.heterogeneous_wait_for_graphs_since(&wait_markers);
        Ok(RiscvTopologyHeterogeneousRunSummary::new(
            events,
            scheduler_run,
            activity.gpu_trace_event_count,
            activity.gpu_workgroup_completion_count,
            activity.accelerator_trace_event_count,
            activity.accelerator_command_completion_count,
        )
        .with_device_activity(activity.gpu_activity, activity.accelerator_activity)
        .with_device_wait_for(wait_for.gpus, wait_for.accelerators))
    }

    fn resolve_heterogeneous_work(
        &self,
        work: Vec<RiscvTopologyHeterogeneousWork>,
    ) -> Result<Vec<ResolvedHeterogeneousWork>, RiscvTopologySystemError> {
        work.into_iter()
            .map(|item| match item {
                RiscvTopologyHeterogeneousWork::Accelerator { engine, command } => {
                    let device = self
                        .accelerators
                        .get(&engine)
                        .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?;
                    if device.command_path().is_none() {
                        return Err(RiscvTopologySystemError::Accelerator(
                            AcceleratorError::MissingCommandSubmission { engine },
                        ));
                    }
                    Ok(ResolvedHeterogeneousWork::Accelerator {
                        engine,
                        device: device.clone(),
                        command,
                    })
                }
                RiscvTopologyHeterogeneousWork::Gpu { device, launch } => {
                    let topology_device = self
                        .gpus
                        .get(&device)
                        .ok_or(RiscvTopologySystemError::UnknownGpu { device })?;
                    Ok(ResolvedHeterogeneousWork::Gpu {
                        device_id: device,
                        device: topology_device.clone(),
                        launch,
                    })
                }
            })
            .collect()
    }

    fn heterogeneous_device_snapshots(
        &self,
        work: &[ResolvedHeterogeneousWork],
    ) -> HeterogeneousDeviceSnapshots {
        let mut accelerators = BTreeMap::new();
        let mut gpus = BTreeMap::new();

        for item in work {
            match item {
                ResolvedHeterogeneousWork::Accelerator { engine, device, .. } => {
                    accelerators
                        .entry(*engine)
                        .or_insert_with(|| device.engine().snapshot());
                }
                ResolvedHeterogeneousWork::Gpu {
                    device_id, device, ..
                } => {
                    gpus.entry(*device_id)
                        .or_insert_with(|| device.gpu().snapshot());
                }
            }
        }

        HeterogeneousDeviceSnapshots { accelerators, gpus }
    }

    fn empty_heterogeneous_run_summary(&self) -> RiscvTopologyHeterogeneousRunSummary {
        let final_tick = self.scheduler().now();
        RiscvTopologyHeterogeneousRunSummary::new(
            Vec::new(),
            RecordedConservativeRunSummary::empty(final_tick),
            0,
            0,
            0,
            0,
        )
    }

    fn heterogeneous_activity_since(
        &self,
        before: &HeterogeneousDeviceSnapshots,
    ) -> HeterogeneousActivityCounts {
        let mut activity = HeterogeneousActivityCounts::default();

        for (engine, before) in &before.accelerators {
            let after = self
                .accelerators
                .get(engine)
                .expect("resolved accelerator remains attached")
                .engine()
                .snapshot();
            let device_activity = RiscvTopologyAcceleratorComputeActivity::new(
                after.trace().len().saturating_sub(before.trace().len()),
                after
                    .completed()
                    .len()
                    .saturating_sub(before.completed().len()),
            );
            activity.accelerator_trace_event_count += device_activity.trace_event_count();
            activity.accelerator_command_completion_count +=
                device_activity.command_completion_count();
            activity
                .accelerator_activity
                .insert(*engine, device_activity);
        }

        for (device, before) in &before.gpus {
            let after = self
                .gpus
                .get(device)
                .expect("resolved GPU remains attached")
                .gpu()
                .snapshot();
            let device_activity = RiscvTopologyGpuComputeActivity::new(
                after.trace().len().saturating_sub(before.trace().len()),
                after
                    .completions()
                    .len()
                    .saturating_sub(before.completions().len()),
            );
            activity.gpu_trace_event_count += device_activity.trace_event_count();
            activity.gpu_workgroup_completion_count += device_activity.workgroup_completion_count();
            activity.gpu_activity.insert(*device, device_activity);
        }

        activity
    }

    fn heterogeneous_wait_for_markers(
        &self,
        work: &[ResolvedHeterogeneousWork],
    ) -> HeterogeneousWaitForMarkers {
        let mut markers = HeterogeneousWaitForMarkers::default();
        for item in work {
            match item {
                ResolvedHeterogeneousWork::Accelerator { engine, device, .. } => {
                    markers
                        .accelerators
                        .entry(*engine)
                        .or_insert_with(|| device.engine().mark_wait_for());
                }
                ResolvedHeterogeneousWork::Gpu {
                    device_id, device, ..
                } => {
                    markers
                        .gpus
                        .entry(*device_id)
                        .or_insert_with(|| device.gpu().mark_wait_for());
                }
            }
        }
        markers
    }

    fn heterogeneous_wait_for_graphs_since(
        &self,
        markers: &HeterogeneousWaitForMarkers,
    ) -> HeterogeneousWaitForGraphs {
        let mut wait_for = HeterogeneousWaitForGraphs::default();
        for (engine, marker) in &markers.accelerators {
            let Some(device) = self.accelerators.get(engine) else {
                continue;
            };
            wait_for
                .accelerators
                .insert(*engine, device.engine().wait_for_graph_since(*marker));
        }
        for (device_id, marker) in &markers.gpus {
            let Some(device) = self.gpus.get(device_id) else {
                continue;
            };
            wait_for
                .gpus
                .insert(*device_id, device.gpu().wait_for_graph_since(*marker));
        }
        wait_for
    }
}
