use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_dram::{
    DramMemoryActivityMarker, DramMemoryActivityProfile, DramMemoryController, DramTargetActivity,
};
use rem6_fabric::{
    FabricActivityMarker, FabricActivityProfile, FabricLaneActivity, FabricLinkId, QosPriority,
    QosRequestorId, VirtualNetworkId,
};
use rem6_kernel::{
    ConservativeRunSummary, DeadlockDiagnostic, ParallelEpochBatchRecord, ParallelRemoteFlowRecord,
    ParallelRemoteSendRecord, ParallelRunProfile, PartitionFrontier, PartitionId,
    RecordedConservativeRunSummary, RecordedRunSummary, SchedulerDispatchRecord, Tick, WaitForEdge,
    WaitForEdgeKind, WaitForGraph, WaitForGraphSnapshot, WaitForNode,
};
use rem6_memory::MemoryTargetId;
use rem6_transport::MemoryTransport;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelCoherenceRunSummary {
    scheduler_run: RecordedConservativeRunSummary,
    cpu_response_count: usize,
    directory_decision_count: usize,
    dram_access_count: usize,
    bank_accepted_count: usize,
    bank_immediate_hit_count: usize,
    bank_scheduled_miss_count: usize,
    bank_coalesced_miss_count: usize,
    fabric_activity: Vec<FabricLaneActivity>,
    dram_activity: Vec<DramTargetActivity>,
    initial_wait_for: WaitForGraphSnapshot,
    remaining_wait_for: WaitForGraphSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelCoherenceWaitForGraphs {
    initial: WaitForGraph,
    remaining: WaitForGraph,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParallelCoherenceRunHistory {
    profile: ParallelRunProfile,
    run_count: usize,
    runs_with_parallel_work: usize,
    runs_with_directory_activity: usize,
    runs_with_dram_activity: usize,
    runs_with_resource_activity: usize,
    runs_with_wait_for_edges: usize,
    total_cpu_responses: usize,
    total_directory_decisions: usize,
    total_dram_accesses: usize,
    total_fabric_transfers: usize,
}

impl ParallelCoherenceWaitForGraphs {
    pub const fn new(initial: WaitForGraph, remaining: WaitForGraph) -> Self {
        Self { initial, remaining }
    }
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
        wait_for_graphs: ParallelCoherenceWaitForGraphs,
    ) -> Self {
        Self {
            scheduler_run,
            cpu_response_count,
            directory_decision_count,
            dram_access_count,
            bank_accepted_count: 0,
            bank_immediate_hit_count: 0,
            bank_scheduled_miss_count: 0,
            bank_coalesced_miss_count: 0,
            fabric_activity,
            dram_activity,
            initial_wait_for: wait_for_graphs.initial.snapshot(),
            remaining_wait_for: wait_for_graphs.remaining.snapshot(),
        }
    }

    pub fn with_bank_activity(
        mut self,
        accepted_count: usize,
        immediate_hit_count: usize,
        scheduled_miss_count: usize,
        coalesced_miss_count: usize,
    ) -> Self {
        self.bank_accepted_count = accepted_count;
        self.bank_immediate_hit_count = immediate_hit_count;
        self.bank_scheduled_miss_count = scheduled_miss_count;
        self.bank_coalesced_miss_count = coalesced_miss_count;
        self
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

    pub fn initial_frontiers(&self) -> Vec<PartitionFrontier> {
        self.scheduler_run
            .epochs()
            .iter()
            .flat_map(|epoch| epoch.initial_frontiers().iter().copied())
            .collect()
    }

    pub fn final_frontiers(&self) -> Vec<PartitionFrontier> {
        self.scheduler_run
            .epochs()
            .iter()
            .flat_map(|epoch| epoch.final_frontiers().iter().copied())
            .collect()
    }

    pub fn remote_flow_count(&self, source: PartitionId, target: PartitionId) -> usize {
        self.scheduler_run.remote_flow_count(source, target)
    }

    pub fn remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        self.scheduler_run.remote_flows()
    }

    pub fn remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        self.scheduler_run.remote_sends()
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

    pub const fn bank_accepted_count(&self) -> usize {
        self.bank_accepted_count
    }

    pub const fn bank_immediate_hit_count(&self) -> usize {
        self.bank_immediate_hit_count
    }

    pub const fn bank_scheduled_miss_count(&self) -> usize {
        self.bank_scheduled_miss_count
    }

    pub const fn bank_coalesced_miss_count(&self) -> usize {
        self.bank_coalesced_miss_count
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

    pub const fn protocol_activity_count(&self) -> usize {
        self.cpu_response_count + self.directory_decision_count + self.dram_access_count
    }

    pub const fn has_directory_activity(&self) -> bool {
        self.directory_decision_count != 0
    }

    pub const fn has_dram_activity(&self) -> bool {
        self.dram_access_count != 0
    }

    pub const fn initial_wait_for_snapshot(&self) -> &WaitForGraphSnapshot {
        &self.initial_wait_for
    }

    pub fn initial_wait_for_edges(&self) -> &[WaitForEdge] {
        self.initial_wait_for.edges()
    }

    pub fn initial_wait_for_edge_count(&self) -> usize {
        self.initial_wait_for.edge_count()
    }

    pub fn initial_has_wait_for_edges(&self) -> bool {
        self.initial_wait_for.has_edges()
    }

    pub fn initial_deadlock_diagnostic(&self) -> Option<&DeadlockDiagnostic> {
        self.initial_wait_for.deadlock_diagnostic()
    }

    pub fn initial_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.initial_wait_for.blocked_nodes()
    }

    pub fn initial_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        self.initial_wait_for.edge_kind_counts()
    }

    pub fn initial_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.initial_wait_for.edge_count_by_kind(kind)
    }

    pub fn initial_oldest_wait_edge(&self) -> Option<&WaitForEdge> {
        self.initial_wait_for.oldest_wait_edge()
    }

    pub fn initial_newest_observed_wait_edge(&self) -> Option<&WaitForEdge> {
        self.initial_wait_for.newest_observed_edge()
    }

    pub fn initial_total_wait_observation_count(&self) -> u64 {
        self.initial_wait_for.total_observation_count()
    }

    pub const fn initial_first_wait_tick(&self) -> Option<Tick> {
        self.initial_wait_for.first_observed_tick()
    }

    pub const fn initial_last_wait_tick(&self) -> Option<Tick> {
        self.initial_wait_for.last_observed_tick()
    }

    pub fn initial_longest_observed_wait_span(&self) -> Option<Tick> {
        self.initial_wait_for.longest_observed_span()
    }

    pub fn initial_has_deadlock(&self) -> bool {
        self.initial_wait_for.has_deadlock()
    }

    pub const fn remaining_wait_for_snapshot(&self) -> &WaitForGraphSnapshot {
        &self.remaining_wait_for
    }

    pub fn remaining_wait_for_edges(&self) -> &[WaitForEdge] {
        self.remaining_wait_for.edges()
    }

    pub fn remaining_wait_for_edge_count(&self) -> usize {
        self.remaining_wait_for.edge_count()
    }

    pub fn remaining_has_wait_for_edges(&self) -> bool {
        self.remaining_wait_for.has_edges()
    }

    pub fn remaining_deadlock_diagnostic(&self) -> Option<&DeadlockDiagnostic> {
        self.remaining_wait_for.deadlock_diagnostic()
    }

    pub fn remaining_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.remaining_wait_for.blocked_nodes()
    }

    pub fn remaining_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        self.remaining_wait_for.edge_kind_counts()
    }

    pub fn remaining_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.remaining_wait_for.edge_count_by_kind(kind)
    }

    pub fn remaining_oldest_wait_edge(&self) -> Option<&WaitForEdge> {
        self.remaining_wait_for.oldest_wait_edge()
    }

    pub fn remaining_newest_observed_wait_edge(&self) -> Option<&WaitForEdge> {
        self.remaining_wait_for.newest_observed_edge()
    }

    pub fn remaining_total_wait_observation_count(&self) -> u64 {
        self.remaining_wait_for.total_observation_count()
    }

    pub const fn remaining_first_wait_tick(&self) -> Option<Tick> {
        self.remaining_wait_for.first_observed_tick()
    }

    pub const fn remaining_last_wait_tick(&self) -> Option<Tick> {
        self.remaining_wait_for.last_observed_tick()
    }

    pub fn remaining_longest_observed_wait_span(&self) -> Option<Tick> {
        self.remaining_wait_for.longest_observed_span()
    }

    pub fn remaining_has_deadlock(&self) -> bool {
        self.remaining_wait_for.has_deadlock()
    }

    pub const fn wait_for_snapshot(&self) -> &WaitForGraphSnapshot {
        self.remaining_wait_for_snapshot()
    }

    pub fn wait_for_edges(&self) -> &[WaitForEdge] {
        self.remaining_wait_for_edges()
    }

    pub fn wait_for_edge_count(&self) -> usize {
        self.remaining_wait_for_edge_count()
    }

    pub fn has_wait_for_edges(&self) -> bool {
        self.remaining_has_wait_for_edges()
    }

    pub fn deadlock_diagnostic(&self) -> Option<&DeadlockDiagnostic> {
        self.remaining_deadlock_diagnostic()
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

    pub fn has_parallel_observation(&self) -> bool {
        self.has_parallel_work()
            || self.protocol_activity_count() != 0
            || self.has_resource_activity()
            || self.initial_has_wait_for_edges()
            || self.remaining_has_wait_for_edges()
    }
}

impl ParallelCoherenceRunHistory {
    pub fn from_runs(runs: &[ParallelCoherenceRunSummary]) -> Self {
        let mut history = Self {
            profile: ParallelRunProfile::default(),
            run_count: runs.len(),
            runs_with_parallel_work: 0,
            runs_with_directory_activity: 0,
            runs_with_dram_activity: 0,
            runs_with_resource_activity: 0,
            runs_with_wait_for_edges: 0,
            total_cpu_responses: 0,
            total_directory_decisions: 0,
            total_dram_accesses: 0,
            total_fabric_transfers: 0,
        };

        for run in runs {
            history.profile = history.profile.merge(run.profile());
            history.runs_with_parallel_work += usize::from(run.has_parallel_work());
            history.runs_with_directory_activity += usize::from(run.has_directory_activity());
            history.runs_with_dram_activity += usize::from(run.has_dram_activity());
            history.runs_with_resource_activity += usize::from(run.has_resource_activity());
            history.runs_with_wait_for_edges +=
                usize::from(run.initial_has_wait_for_edges() || run.remaining_has_wait_for_edges());
            history.total_cpu_responses += run.cpu_response_count();
            history.total_directory_decisions += run.directory_decision_count();
            history.total_dram_accesses += run.dram_access_count();
            history.total_fabric_transfers += run.fabric_transfer_count();
        }

        history
    }

    pub fn from_histories<I>(histories: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        histories
            .into_iter()
            .fold(Self::default(), |merged, history| merged.merge(history))
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            profile: self.profile.merge(other.profile),
            run_count: self.run_count + other.run_count,
            runs_with_parallel_work: self.runs_with_parallel_work + other.runs_with_parallel_work,
            runs_with_directory_activity: self.runs_with_directory_activity
                + other.runs_with_directory_activity,
            runs_with_dram_activity: self.runs_with_dram_activity + other.runs_with_dram_activity,
            runs_with_resource_activity: self.runs_with_resource_activity
                + other.runs_with_resource_activity,
            runs_with_wait_for_edges: self.runs_with_wait_for_edges
                + other.runs_with_wait_for_edges,
            total_cpu_responses: self.total_cpu_responses + other.total_cpu_responses,
            total_directory_decisions: self.total_directory_decisions
                + other.total_directory_decisions,
            total_dram_accesses: self.total_dram_accesses + other.total_dram_accesses,
            total_fabric_transfers: self.total_fabric_transfers + other.total_fabric_transfers,
        }
    }

    pub const fn profile(&self) -> ParallelRunProfile {
        self.profile
    }

    pub const fn run_count(&self) -> usize {
        self.run_count
    }

    pub const fn is_empty(&self) -> bool {
        self.run_count == 0
    }

    pub const fn total_epochs(&self) -> usize {
        self.profile.epoch_count()
    }

    pub const fn total_empty_epochs(&self) -> usize {
        self.profile.empty_epoch_count()
    }

    pub const fn total_batches(&self) -> usize {
        self.profile.batch_count()
    }

    pub const fn total_dispatches(&self) -> usize {
        self.profile.dispatch_count()
    }

    pub const fn total_parallel_workers(&self) -> usize {
        self.profile.total_parallel_workers()
    }

    pub const fn max_parallel_workers(&self) -> usize {
        self.profile.max_parallel_workers()
    }

    pub const fn runs_with_parallel_work(&self) -> usize {
        self.runs_with_parallel_work
    }

    pub const fn has_parallel_work(&self) -> bool {
        self.runs_with_parallel_work != 0
    }

    pub const fn runs_with_directory_activity(&self) -> usize {
        self.runs_with_directory_activity
    }

    pub const fn has_directory_activity(&self) -> bool {
        self.runs_with_directory_activity != 0
    }

    pub const fn runs_with_dram_activity(&self) -> usize {
        self.runs_with_dram_activity
    }

    pub const fn has_dram_activity(&self) -> bool {
        self.runs_with_dram_activity != 0
    }

    pub const fn runs_with_resource_activity(&self) -> usize {
        self.runs_with_resource_activity
    }

    pub const fn has_resource_activity(&self) -> bool {
        self.runs_with_resource_activity != 0
    }

    pub const fn runs_with_wait_for_edges(&self) -> usize {
        self.runs_with_wait_for_edges
    }

    pub const fn has_wait_for_edges(&self) -> bool {
        self.runs_with_wait_for_edges != 0
    }

    pub const fn total_cpu_responses(&self) -> usize {
        self.total_cpu_responses
    }

    pub const fn total_directory_decisions(&self) -> usize {
        self.total_directory_decisions
    }

    pub const fn total_dram_accesses(&self) -> usize {
        self.total_dram_accesses
    }

    pub const fn total_fabric_transfers(&self) -> usize {
        self.total_fabric_transfers
    }

    pub const fn total_protocol_activity(&self) -> usize {
        self.total_cpu_responses + self.total_directory_decisions + self.total_dram_accesses
    }

    pub const fn total_resource_activity(&self) -> usize {
        self.total_fabric_transfers + self.total_dram_accesses
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
            .or_insert_with(|| activity.clone());
    }
}
