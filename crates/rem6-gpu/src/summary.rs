use std::collections::BTreeMap;

use rem6_kernel::{
    ConservativeRunSummary, ParallelEpochBatchRecord, ParallelPartitionActivity,
    ParallelRunProfile, PartitionId, RecordedConservativeRunSummary, RecordedRunSummary,
    SchedulerDispatchRecord, Tick,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuComputeUnitQueueWaitSummary {
    compute_unit: u32,
    waited_workgroups: u64,
    wait_ticks: Tick,
    max_wait_ticks: Tick,
}

impl GpuComputeUnitQueueWaitSummary {
    pub const fn new(
        compute_unit: u32,
        waited_workgroups: u64,
        wait_ticks: Tick,
        max_wait_ticks: Tick,
    ) -> Self {
        Self {
            compute_unit,
            waited_workgroups,
            wait_ticks,
            max_wait_ticks,
        }
    }

    pub const fn compute_unit(&self) -> u32 {
        self.compute_unit
    }

    pub const fn waited_workgroups(&self) -> u64 {
        self.waited_workgroups
    }

    pub const fn wait_ticks(&self) -> Tick {
        self.wait_ticks
    }

    pub const fn max_wait_ticks(&self) -> Tick {
        self.max_wait_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelGpuRunSummary {
    scheduler_run: RecordedConservativeRunSummary,
    trace_event_count: usize,
    workgroup_completion_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
    memory_access_count: usize,
    coalesced_memory_access_count: usize,
    workgroup_queue_wait_count: u64,
    workgroup_queue_wait_ticks: Tick,
    max_workgroup_queue_wait_ticks: Tick,
    compute_unit_queue_waits: Vec<GpuComputeUnitQueueWaitSummary>,
}

impl ParallelGpuRunSummary {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scheduler_run: RecordedConservativeRunSummary,
        trace_event_count: usize,
        workgroup_completion_count: usize,
        pending_dma_write_count: usize,
        dma_completion_count: usize,
        memory_access_count: usize,
        coalesced_memory_access_count: usize,
        workgroup_queue_wait_count: u64,
        workgroup_queue_wait_ticks: Tick,
        max_workgroup_queue_wait_ticks: Tick,
        compute_unit_queue_waits: Vec<GpuComputeUnitQueueWaitSummary>,
    ) -> Self {
        Self {
            scheduler_run,
            trace_event_count,
            workgroup_completion_count,
            pending_dma_write_count,
            dma_completion_count,
            memory_access_count,
            coalesced_memory_access_count,
            workgroup_queue_wait_count,
            workgroup_queue_wait_ticks,
            max_workgroup_queue_wait_ticks,
            compute_unit_queue_waits,
        }
    }

    pub const fn scheduler_run(&self) -> &RecordedConservativeRunSummary {
        &self.scheduler_run
    }

    pub fn scheduler_epochs(&self) -> &[RecordedRunSummary] {
        self.scheduler_run.epochs()
    }

    pub fn summary(&self) -> ConservativeRunSummary {
        self.scheduler_run.summary()
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

    pub fn max_parallel_workers(&self) -> usize {
        self.scheduler_run.max_parallel_workers()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.scheduler_run.total_parallel_workers()
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

    pub fn dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.scheduler_run.dispatches()
    }

    pub fn batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.scheduler_run.batches()
    }

    pub fn executed_events(&self) -> usize {
        self.summary().executed_events()
    }

    pub fn final_tick(&self) -> Tick {
        self.summary().final_tick()
    }

    pub const fn trace_event_count(&self) -> usize {
        self.trace_event_count
    }

    pub const fn workgroup_completion_count(&self) -> usize {
        self.workgroup_completion_count
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.pending_dma_write_count
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.dma_completion_count
    }

    pub const fn memory_access_count(&self) -> usize {
        self.memory_access_count
    }

    pub const fn coalesced_memory_access_count(&self) -> usize {
        self.coalesced_memory_access_count
    }

    pub const fn workgroup_queue_wait_count(&self) -> u64 {
        self.workgroup_queue_wait_count
    }

    pub const fn workgroup_queue_wait_ticks(&self) -> Tick {
        self.workgroup_queue_wait_ticks
    }

    pub const fn max_workgroup_queue_wait_ticks(&self) -> Tick {
        self.max_workgroup_queue_wait_ticks
    }

    pub fn compute_unit_queue_waits(&self) -> &[GpuComputeUnitQueueWaitSummary] {
        &self.compute_unit_queue_waits
    }

    pub const fn device_activity_count(&self) -> usize {
        self.trace_event_count
            + self.workgroup_completion_count
            + self.pending_dma_write_count
            + self.dma_completion_count
            + self.coalesced_memory_access_count
    }

    pub const fn has_device_activity(&self) -> bool {
        self.device_activity_count() != 0
    }

    pub const fn has_compute_activity(&self) -> bool {
        self.workgroup_completion_count != 0
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.pending_dma_write_count != 0 || self.dma_completion_count != 0
    }

    pub const fn has_memory_activity(&self) -> bool {
        self.coalesced_memory_access_count != 0
    }
}
