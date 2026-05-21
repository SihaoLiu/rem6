use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_dram::{
    DramMemoryActivityMarker, DramMemoryActivityProfile, DramMemoryController, DramTargetActivity,
};
use rem6_fabric::{
    FabricActivityMarker, FabricActivityProfile, FabricLaneActivity, FabricLinkId, VirtualNetworkId,
};
use rem6_kernel::{
    ConservativeRunSummary, ParallelEpochBatchRecord, ParallelRunProfile, PartitionId,
    RecordedConservativeRunSummary, RecordedRunSummary, SchedulerDispatchRecord, Tick,
};
use rem6_memory::MemoryTargetId;
use rem6_transport::MemoryTransport;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelCoherenceRunSummary {
    scheduler_run: RecordedConservativeRunSummary,
    cpu_response_count: usize,
    directory_decision_count: usize,
    dram_access_count: usize,
    fabric_activity: Vec<FabricLaneActivity>,
    dram_activity: Vec<DramTargetActivity>,
}

pub(crate) struct CoherenceResourceActivityWindow {
    fabric_marker: Option<FabricActivityMarker>,
    dram_marker: Option<DramMemoryActivityMarker>,
}

impl CoherenceResourceActivityWindow {
    pub(crate) fn mark(
        transport: &MemoryTransport,
        dram_memory: Option<&Arc<Mutex<DramMemoryController>>>,
    ) -> Self {
        let fabric_marker = transport.mark_fabric_activity();
        let dram_marker =
            dram_memory.map(|memory| memory.lock().expect("DRAM memory lock").mark_activity());

        Self {
            fabric_marker,
            dram_marker,
        }
    }

    pub(crate) fn collect(
        self,
        transport: &MemoryTransport,
        dram_memory: Option<&Arc<Mutex<DramMemoryController>>>,
    ) -> (Vec<FabricLaneActivity>, Vec<DramTargetActivity>) {
        let fabric_activity = self
            .fabric_marker
            .and_then(|marker| transport.fabric_lane_activities_since(marker))
            .unwrap_or_default();
        let dram_activity = match (dram_memory, self.dram_marker.as_ref()) {
            (Some(memory), Some(marker)) => memory
                .lock()
                .expect("DRAM memory lock")
                .target_activities_since(marker),
            _ => Vec::new(),
        };

        (fabric_activity, dram_activity)
    }
}

impl ParallelCoherenceRunSummary {
    pub fn new(
        scheduler_run: RecordedConservativeRunSummary,
        cpu_response_count: usize,
        directory_decision_count: usize,
        dram_access_count: usize,
        fabric_activity: Vec<FabricLaneActivity>,
        dram_activity: Vec<DramTargetActivity>,
    ) -> Self {
        Self {
            scheduler_run,
            cpu_response_count,
            directory_decision_count,
            dram_access_count,
            fabric_activity,
            dram_activity,
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

    pub fn dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.scheduler_run.dispatches()
    }

    pub fn batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.scheduler_run.batches()
    }

    pub fn parallel_worker_partitions(&self) -> Vec<PartitionId> {
        self.scheduler_run.parallel_worker_partitions()
    }

    pub fn executed_events(&self) -> usize {
        self.summary().executed_events()
    }

    pub fn final_tick(&self) -> Tick {
        self.summary().final_tick()
    }

    pub const fn cpu_response_count(&self) -> usize {
        self.cpu_response_count
    }

    pub const fn directory_decision_count(&self) -> usize {
        self.directory_decision_count
    }

    pub const fn dram_access_count(&self) -> usize {
        self.dram_access_count
    }

    pub const fn protocol_activity_count(&self) -> usize {
        self.cpu_response_count + self.directory_decision_count + self.dram_access_count
    }

    pub const fn has_directory_activity(&self) -> bool {
        self.directory_decision_count != 0
    }

    pub const fn has_dram_activity(&self) -> bool {
        self.dram_access_count != 0
    }

    pub fn fabric_activity(
        &self,
        link: &FabricLinkId,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricLaneActivity> {
        self.fabric_activities()
            .remove(&(link.clone(), virtual_network))
    }

    pub fn fabric_activities(
        &self,
    ) -> BTreeMap<(FabricLinkId, VirtualNetworkId), FabricLaneActivity> {
        collect_run_fabric_activity(&self.fabric_activity)
    }

    pub fn fabric_profile(&self) -> FabricActivityProfile {
        let activities = self.fabric_activities();
        FabricActivityProfile::from_lanes(activities.values())
    }

    pub fn active_fabric_lane_count(&self) -> usize {
        self.fabric_activities().len()
    }

    pub fn fabric_transfer_count(&self) -> usize {
        self.fabric_activities()
            .values()
            .map(FabricLaneActivity::transfer_count)
            .sum()
    }

    pub fn has_fabric_activity(&self) -> bool {
        self.fabric_transfer_count() != 0
    }

    pub fn dram_target_activity(&self, target: MemoryTargetId) -> Option<DramTargetActivity> {
        self.dram_target_activities().remove(&target)
    }

    pub fn dram_target_activities(&self) -> BTreeMap<MemoryTargetId, DramTargetActivity> {
        collect_run_dram_activity(&self.dram_activity)
    }

    pub fn dram_profile(&self) -> DramMemoryActivityProfile {
        let activities = self.dram_target_activities();
        DramMemoryActivityProfile::from_target_activities(activities.values())
    }

    pub fn active_dram_target_count(&self) -> usize {
        self.dram_profile().active_target_count()
    }

    pub fn resource_activity_count(&self) -> usize {
        self.fabric_transfer_count() + self.dram_access_count
    }

    pub fn has_resource_activity(&self) -> bool {
        self.resource_activity_count() != 0
    }
}

fn collect_run_fabric_activity(
    source: &[FabricLaneActivity],
) -> BTreeMap<(FabricLinkId, VirtualNetworkId), FabricLaneActivity> {
    let mut activities = BTreeMap::new();
    merge_run_fabric_activity_maps(&mut activities, source);
    activities
}

fn merge_run_fabric_activity_maps(
    target: &mut BTreeMap<(FabricLinkId, VirtualNetworkId), FabricLaneActivity>,
    source: &[FabricLaneActivity],
) {
    for activity in source {
        target
            .entry((activity.link().clone(), activity.virtual_network()))
            .and_modify(|stored| *stored = stored.clone().merge_window(activity.clone()))
            .or_insert_with(|| activity.clone());
    }
}

fn collect_run_dram_activity(
    source: &[DramTargetActivity],
) -> BTreeMap<MemoryTargetId, DramTargetActivity> {
    let mut activities = BTreeMap::new();
    merge_run_dram_activity_maps(&mut activities, source);
    activities
}

fn merge_run_dram_activity_maps(
    target: &mut BTreeMap<MemoryTargetId, DramTargetActivity>,
    source: &[DramTargetActivity],
) {
    for activity in source {
        target
            .entry(activity.target())
            .and_modify(|stored| {
                *stored = DramTargetActivity::new(
                    stored.target(),
                    stored.profile().merge_window(activity.profile()),
                );
            })
            .or_insert(*activity);
    }
}
