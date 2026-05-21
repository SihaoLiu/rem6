use std::collections::BTreeMap;

use rem6_accelerator::{AcceleratorDmaCopy, AcceleratorEngineId};
use rem6_gpu::{GpuDeviceId, GpuDmaCopy};
use rem6_kernel::{ParallelRunProfile, PartitionEventId, RecordedConservativeRunSummary, Tick};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologyDmaCopy {
    Accelerator {
        engine: AcceleratorEngineId,
        copy: AcceleratorDmaCopy,
    },
    Gpu {
        device: GpuDeviceId,
        copy: GpuDmaCopy,
    },
}

impl RiscvTopologyDmaCopy {
    pub const fn accelerator(engine: AcceleratorEngineId, copy: AcceleratorDmaCopy) -> Self {
        Self::Accelerator { engine, copy }
    }

    pub const fn gpu(device: GpuDeviceId, copy: GpuDmaCopy) -> Self {
        Self::Gpu { device, copy }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvTopologyDmaDeviceActivity {
    trace_event_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
}

impl RiscvTopologyDmaDeviceActivity {
    pub const fn new(
        trace_event_count: usize,
        pending_dma_write_count: usize,
        dma_completion_count: usize,
    ) -> Self {
        Self {
            trace_event_count,
            pending_dma_write_count,
            dma_completion_count,
        }
    }

    pub const fn trace_event_count(self) -> usize {
        self.trace_event_count
    }

    pub const fn pending_dma_write_count(self) -> usize {
        self.pending_dma_write_count
    }

    pub const fn dma_completion_count(self) -> usize {
        self.dma_completion_count
    }

    pub const fn merge_window(self, later: Self) -> Self {
        Self {
            trace_event_count: self.trace_event_count + later.trace_event_count,
            pending_dma_write_count: later.pending_dma_write_count,
            dma_completion_count: self.dma_completion_count + later.dma_completion_count,
        }
    }

    pub const fn device_activity_count(self) -> usize {
        self.trace_event_count + self.pending_dma_write_count + self.dma_completion_count
    }

    pub const fn has_dma_activity(self) -> bool {
        self.device_activity_count() != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyDmaStageRunSummary {
    events: Vec<PartitionEventId>,
    scheduler_run: RecordedConservativeRunSummary,
    trace_event_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
    accelerator_activity: BTreeMap<AcceleratorEngineId, RiscvTopologyDmaDeviceActivity>,
    gpu_activity: BTreeMap<GpuDeviceId, RiscvTopologyDmaDeviceActivity>,
}

impl RiscvTopologyDmaStageRunSummary {
    pub fn new(
        events: Vec<PartitionEventId>,
        scheduler_run: RecordedConservativeRunSummary,
        trace_event_count: usize,
        pending_dma_write_count: usize,
        dma_completion_count: usize,
    ) -> Self {
        Self {
            events,
            scheduler_run,
            trace_event_count,
            pending_dma_write_count,
            dma_completion_count,
            accelerator_activity: BTreeMap::new(),
            gpu_activity: BTreeMap::new(),
        }
    }

    pub fn with_device_activity(
        mut self,
        accelerator_activity: BTreeMap<AcceleratorEngineId, RiscvTopologyDmaDeviceActivity>,
        gpu_activity: BTreeMap<GpuDeviceId, RiscvTopologyDmaDeviceActivity>,
    ) -> Self {
        self.accelerator_activity = accelerator_activity;
        self.gpu_activity = gpu_activity;
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

    pub fn final_tick(&self) -> Tick {
        self.scheduler_run.summary().final_tick()
    }

    pub const fn trace_event_count(&self) -> usize {
        self.trace_event_count
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.pending_dma_write_count
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.dma_completion_count
    }

    pub fn accelerator_activity(
        &self,
        engine: AcceleratorEngineId,
    ) -> Option<RiscvTopologyDmaDeviceActivity> {
        self.accelerator_activity.get(&engine).copied()
    }

    pub fn accelerator_activities(
        &self,
    ) -> &BTreeMap<AcceleratorEngineId, RiscvTopologyDmaDeviceActivity> {
        &self.accelerator_activity
    }

    pub fn gpu_activity(&self, device: GpuDeviceId) -> Option<RiscvTopologyDmaDeviceActivity> {
        self.gpu_activity.get(&device).copied()
    }

    pub fn gpu_activities(&self) -> &BTreeMap<GpuDeviceId, RiscvTopologyDmaDeviceActivity> {
        &self.gpu_activity
    }

    pub const fn device_activity_count(&self) -> usize {
        self.trace_event_count + self.pending_dma_write_count + self.dma_completion_count
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.device_activity_count() != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyDmaRunSummary {
    read: RiscvTopologyDmaStageRunSummary,
    write: RiscvTopologyDmaStageRunSummary,
}

impl RiscvTopologyDmaRunSummary {
    pub const fn new(
        read: RiscvTopologyDmaStageRunSummary,
        write: RiscvTopologyDmaStageRunSummary,
    ) -> Self {
        Self { read, write }
    }

    pub const fn read(&self) -> &RiscvTopologyDmaStageRunSummary {
        &self.read
    }

    pub const fn write(&self) -> &RiscvTopologyDmaStageRunSummary {
        &self.write
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.read.profile().merge(self.write.profile())
    }

    pub fn event_count(&self) -> usize {
        self.read.event_count() + self.write.event_count()
    }

    pub const fn trace_event_count(&self) -> usize {
        self.read.trace_event_count() + self.write.trace_event_count()
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.write.pending_dma_write_count()
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.read.dma_completion_count() + self.write.dma_completion_count()
    }

    pub fn accelerator_activity(
        &self,
        engine: AcceleratorEngineId,
    ) -> Option<RiscvTopologyDmaDeviceActivity> {
        merge_window_activity(
            self.read.accelerator_activity(engine),
            self.write.accelerator_activity(engine),
        )
    }

    pub fn accelerator_activities(
        &self,
    ) -> BTreeMap<AcceleratorEngineId, RiscvTopologyDmaDeviceActivity> {
        merge_window_activity_maps(
            self.read.accelerator_activities(),
            self.write.accelerator_activities(),
        )
    }

    pub fn gpu_activity(&self, device: GpuDeviceId) -> Option<RiscvTopologyDmaDeviceActivity> {
        merge_window_activity(
            self.read.gpu_activity(device),
            self.write.gpu_activity(device),
        )
    }

    pub fn gpu_activities(&self) -> BTreeMap<GpuDeviceId, RiscvTopologyDmaDeviceActivity> {
        merge_window_activity_maps(self.read.gpu_activities(), self.write.gpu_activities())
    }

    pub const fn device_activity_count(&self) -> usize {
        self.read.device_activity_count() + self.write.device_activity_count()
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.device_activity_count() != 0
    }

    pub fn has_parallel_work(&self) -> bool {
        self.read.has_parallel_work() || self.write.has_parallel_work()
    }

    pub fn final_tick(&self) -> Tick {
        self.write.final_tick()
    }
}

fn merge_window_activity(
    read: Option<RiscvTopologyDmaDeviceActivity>,
    write: Option<RiscvTopologyDmaDeviceActivity>,
) -> Option<RiscvTopologyDmaDeviceActivity> {
    match (read, write) {
        (Some(read), Some(write)) => Some(read.merge_window(write)),
        (Some(read), None) => Some(read),
        (None, Some(write)) => Some(write),
        (None, None) => None,
    }
}

fn merge_window_activity_maps<K>(
    read: &BTreeMap<K, RiscvTopologyDmaDeviceActivity>,
    write: &BTreeMap<K, RiscvTopologyDmaDeviceActivity>,
) -> BTreeMap<K, RiscvTopologyDmaDeviceActivity>
where
    K: Copy + Ord,
{
    let mut merged = BTreeMap::new();
    for key in read.keys().chain(write.keys()) {
        if !merged.contains_key(key) {
            if let Some(activity) =
                merge_window_activity(read.get(key).copied(), write.get(key).copied())
            {
                merged.insert(*key, activity);
            }
        }
    }
    merged
}
