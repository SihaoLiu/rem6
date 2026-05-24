use std::collections::{BTreeMap, BTreeSet};

use rem6_accelerator::{AcceleratorDmaCopy, AcceleratorEngineId};
use rem6_dram::{DramMemoryActivityProfile, DramTargetActivity};
use rem6_fabric::{
    FabricActivityProfile, FabricLaneActivity, FabricLinkId, QosPriority, QosRequestorId,
    VirtualNetworkId,
};
use rem6_gpu::{GpuDeviceId, GpuDmaCopy};
use rem6_kernel::{
    ParallelPartitionActivity, ParallelRunProfile, PartitionEventId, PartitionId,
    RecordedConservativeRunSummary, Tick, WaitForEdge, WaitForEdgeKind, WaitForGraph, WaitForNode,
};
use rem6_memory::MemoryTargetId;

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
    fabric_activity: Vec<FabricLaneActivity>,
    fabric_wait_for: WaitForGraph,
    dram_activity: Vec<DramTargetActivity>,
    dram_wait_for: WaitForGraph,
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
            fabric_activity: Vec::new(),
            fabric_wait_for: WaitForGraph::new(),
            dram_activity: Vec::new(),
            dram_wait_for: WaitForGraph::new(),
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

    pub fn with_fabric_activity(mut self, fabric_activity: Vec<FabricLaneActivity>) -> Self {
        self.fabric_activity = fabric_activity;
        self
    }

    pub fn with_fabric_wait_for(mut self, fabric_wait_for: WaitForGraph) -> Self {
        self.fabric_wait_for = fabric_wait_for;
        self
    }

    pub fn with_dram_activity(mut self, dram_activity: Vec<DramTargetActivity>) -> Self {
        self.dram_activity = dram_activity;
        self
    }

    pub fn with_dram_wait_for(mut self, dram_wait_for: WaitForGraph) -> Self {
        self.dram_wait_for = dram_wait_for;
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

    pub fn fabric_activity(
        &self,
        link: &FabricLinkId,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricLaneActivity> {
        self.fabric_activity
            .iter()
            .find(|activity| {
                activity.link() == link && activity.virtual_network() == virtual_network
            })
            .cloned()
    }

    pub fn fabric_activities(&self) -> &[FabricLaneActivity] {
        &self.fabric_activity
    }

    pub fn fabric_profile(&self) -> FabricActivityProfile {
        FabricActivityProfile::from_lanes(self.fabric_activity.iter())
    }

    pub fn active_fabric_lane_count(&self) -> usize {
        self.fabric_activity.len()
    }

    pub fn fabric_transfer_count(&self) -> usize {
        self.fabric_activity
            .iter()
            .map(FabricLaneActivity::transfer_count)
            .sum()
    }

    pub fn has_fabric_activity(&self) -> bool {
        self.fabric_transfer_count() != 0
    }

    pub fn fabric_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.fabric_wait_for.edges()
    }

    pub fn fabric_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for.edge_count()
    }

    pub fn has_fabric_wait_for_edges(&self) -> bool {
        self.fabric_wait_for_edge_count() != 0
    }

    pub fn fabric_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.fabric_wait_for.blocked_nodes()
    }

    pub fn fabric_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        wait_for_edge_kind_counts(self.fabric_wait_for_edges())
    }

    pub fn fabric_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.fabric_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn dram_target_activity(&self, target: MemoryTargetId) -> Option<DramTargetActivity> {
        self.dram_activity
            .iter()
            .find(|activity| activity.target() == target)
            .cloned()
    }

    pub fn dram_target_activities(&self) -> &[DramTargetActivity] {
        &self.dram_activity
    }

    pub fn dram_profile(&self) -> DramMemoryActivityProfile {
        DramMemoryActivityProfile::from_target_activities(self.dram_activity.iter())
    }

    pub fn active_dram_target_count(&self) -> usize {
        self.dram_profile().active_target_count()
    }

    pub fn dram_access_count(&self) -> usize {
        self.dram_profile().access_count()
    }

    pub fn dram_qos_access_count(&self) -> usize {
        self.dram_profile().qos_access_count()
    }

    pub fn dram_qos_byte_count(&self) -> u64 {
        self.dram_profile().qos_byte_count()
    }

    pub fn dram_qos_escalated_access_count(&self) -> usize {
        self.dram_profile().qos_escalated_access_count()
    }

    pub fn dram_qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.dram_profile().qos_priority_access_count(priority)
    }

    pub fn dram_qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.dram_profile().qos_priority_byte_count(priority)
    }

    pub fn dram_qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.dram_profile().qos_requestor_access_count(requestor)
    }

    pub fn dram_qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.dram_profile().qos_requestor_byte_count(requestor)
    }

    pub fn has_dram_qos_activity(&self) -> bool {
        self.dram_qos_access_count() != 0
    }

    pub fn has_dram_activity(&self) -> bool {
        self.dram_access_count() != 0
    }

    pub fn dram_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.dram_wait_for.edges()
    }

    pub fn dram_wait_for_edge_count(&self) -> usize {
        self.dram_wait_for.edge_count()
    }

    pub fn has_dram_wait_for_edges(&self) -> bool {
        self.dram_wait_for_edge_count() != 0
    }

    pub fn dram_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.dram_wait_for.blocked_nodes()
    }

    pub fn dram_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        wait_for_edge_kind_counts(self.dram_wait_for_edges())
    }

    pub fn dram_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.dram_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
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

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = self.read.partition_activities();
        merge_parallel_partition_activity_maps(&mut activities, self.write.partition_activities());
        activities
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
        let mut activities = collect_fabric_activity(self.read.fabric_activities());
        merge_fabric_activity_maps(&mut activities, self.write.fabric_activities());
        activities
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

    pub fn fabric_wait_for_edges(&self) -> Vec<WaitForEdge> {
        let mut edges = self.read.fabric_wait_for_edges();
        edges.extend(self.write.fabric_wait_for_edges());
        edges
    }

    pub fn fabric_wait_for_edge_count(&self) -> usize {
        self.read.fabric_wait_for_edge_count() + self.write.fabric_wait_for_edge_count()
    }

    pub fn has_fabric_wait_for_edges(&self) -> bool {
        self.fabric_wait_for_edge_count() != 0
    }

    pub fn fabric_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.fabric_wait_for_edges()
            .into_iter()
            .map(|edge| edge.source().clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn fabric_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        wait_for_edge_kind_counts(self.fabric_wait_for_edges())
    }

    pub fn fabric_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.fabric_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn dram_target_activity(&self, target: MemoryTargetId) -> Option<DramTargetActivity> {
        self.dram_target_activities().remove(&target)
    }

    pub fn dram_target_activities(&self) -> BTreeMap<MemoryTargetId, DramTargetActivity> {
        let mut activities = collect_dram_activity(self.read.dram_target_activities());
        merge_dram_activity_maps(&mut activities, self.write.dram_target_activities());
        activities
    }

    pub fn dram_profile(&self) -> DramMemoryActivityProfile {
        let activities = self.dram_target_activities();
        DramMemoryActivityProfile::from_target_activities(activities.values())
    }

    pub fn active_dram_target_count(&self) -> usize {
        self.dram_profile().active_target_count()
    }

    pub fn dram_access_count(&self) -> usize {
        self.dram_profile().access_count()
    }

    pub fn dram_qos_access_count(&self) -> usize {
        self.dram_profile().qos_access_count()
    }

    pub fn dram_qos_byte_count(&self) -> u64 {
        self.dram_profile().qos_byte_count()
    }

    pub fn dram_qos_escalated_access_count(&self) -> usize {
        self.dram_profile().qos_escalated_access_count()
    }

    pub fn dram_qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.dram_profile().qos_priority_access_count(priority)
    }

    pub fn dram_qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.dram_profile().qos_priority_byte_count(priority)
    }

    pub fn dram_qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.dram_profile().qos_requestor_access_count(requestor)
    }

    pub fn dram_qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.dram_profile().qos_requestor_byte_count(requestor)
    }

    pub fn has_dram_qos_activity(&self) -> bool {
        self.dram_qos_access_count() != 0
    }

    pub fn has_dram_activity(&self) -> bool {
        self.dram_access_count() != 0
    }

    pub fn dram_wait_for_edges(&self) -> Vec<WaitForEdge> {
        let mut edges = self.read.dram_wait_for_edges();
        edges.extend(self.write.dram_wait_for_edges());
        edges
    }

    pub fn dram_wait_for_edge_count(&self) -> usize {
        self.read.dram_wait_for_edge_count() + self.write.dram_wait_for_edge_count()
    }

    pub fn has_dram_wait_for_edges(&self) -> bool {
        self.dram_wait_for_edge_count() != 0
    }

    pub fn dram_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.dram_wait_for_edges()
            .into_iter()
            .map(|edge| edge.source().clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn dram_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        wait_for_edge_kind_counts(self.dram_wait_for_edges())
    }

    pub fn dram_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.dram_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
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

fn merge_parallel_partition_activity_maps(
    target: &mut BTreeMap<PartitionId, ParallelPartitionActivity>,
    source: BTreeMap<PartitionId, ParallelPartitionActivity>,
) {
    for (partition, activity) in source {
        target
            .entry(partition)
            .and_modify(|stored| {
                *stored = ParallelPartitionActivity::new(
                    stored.worker_count() + activity.worker_count(),
                    stored.dispatch_count() + activity.dispatch_count(),
                    stored
                        .max_pending_events()
                        .max(activity.max_pending_events()),
                );
            })
            .or_insert(activity);
    }
}

fn collect_fabric_activity(
    source: &[FabricLaneActivity],
) -> BTreeMap<(FabricLinkId, VirtualNetworkId), FabricLaneActivity> {
    source
        .iter()
        .map(|activity| {
            (
                (activity.link().clone(), activity.virtual_network()),
                activity.clone(),
            )
        })
        .collect()
}

fn merge_fabric_activity_maps(
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

fn wait_for_edge_kind_counts<I>(edges: I) -> BTreeMap<WaitForEdgeKind, usize>
where
    I: IntoIterator<Item = WaitForEdge>,
{
    let mut counts = BTreeMap::new();
    for edge in edges {
        *counts.entry(edge.kind()).or_insert(0) += 1;
    }
    counts
}

fn collect_dram_activity(
    source: &[DramTargetActivity],
) -> BTreeMap<MemoryTargetId, DramTargetActivity> {
    source
        .iter()
        .map(|activity| (activity.target(), activity.clone()))
        .collect()
}

fn merge_dram_activity_maps(
    target: &mut BTreeMap<MemoryTargetId, DramTargetActivity>,
    source: &[DramTargetActivity],
) {
    for activity in source {
        target
            .entry(activity.target())
            .and_modify(|stored| {
                *stored = stored.clone().merge_window(activity.clone());
            })
            .or_insert_with(|| activity.clone());
    }
}
