use std::collections::BTreeMap;

use rem6_coherence::ParallelCoherenceRunSummary;
use rem6_cpu::{
    CpuId, RiscvClusterSchedulerEpoch, RiscvClusterTurn, RiscvStoreConditionalFailureDiagnostic,
};
use rem6_dram::{DramMemoryActivityProfile, DramTargetActivity};
use rem6_fabric::{
    FabricActivityProfile, FabricHopActivity, FabricLaneActivity, FabricLinkId,
    FabricVirtualNetworkActivity, VirtualNetworkId,
};
use rem6_kernel::{
    ParallelEpochBatchRecord, ParallelPartitionActivity, ParallelRunProfile, ParallelWorkerRecord,
    PartitionFrontier, PartitionId, ReadyPartition, SchedulerDispatchRecord, Tick, WaitForGraph,
};
use rem6_memory::MemoryTargetId;

use crate::data_cache_run::{RiscvDataCacheProtocol, RiscvDataCacheRunRecord};
use crate::guest_event::StopRequest;
use crate::riscv_data_access_stats::RiscvDataAccessProbeSnapshot;
use crate::riscv_instruction_stats::RiscvRetiredInstructionProbeSnapshot;
use crate::riscv_run_activity::{self, RiscvSystemRunCpuActivity, RiscvSystemRunPartitionActivity};
use crate::trace_diagnostic::RiscvTraceDiagnosticRecord;
use crate::trace_error::RiscvTraceErrorRecord;
use crate::trace_htm_access::RiscvTraceHtmAccessRecord;
use crate::trap_event::ScheduledRiscvTrap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSystemRunStopReason {
    HostStop(StopRequest),
    DebugStop {
        tick: Tick,
    },
    Idle {
        tick: Tick,
    },
    TickLimit {
        tick: Tick,
        limit: u64,
    },
    InstructionLimit {
        tick: Tick,
        limit: u64,
        committed: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSystemRun {
    turns: Vec<RiscvClusterTurn>,
    scheduled_traps: Vec<ScheduledRiscvTrap>,
    stop_reason: RiscvSystemRunStopReason,
    fabric_hop_activity: Vec<FabricHopActivity>,
    fabric_activity: Vec<FabricLaneActivity>,
    pub(crate) fabric_wait_for: WaitForGraph,
    dram_activity: Vec<DramTargetActivity>,
    pub(crate) dram_wait_for: WaitForGraph,
    pub(crate) data_cache_runs: Vec<ParallelCoherenceRunSummary>,
    pub(crate) data_cache_run_protocols: Vec<Option<RiscvDataCacheProtocol>>,
    pub(crate) trace_diagnostic_records: Vec<RiscvTraceDiagnosticRecord>,
    pub(crate) trace_error_records: Vec<RiscvTraceErrorRecord>,
    pub(crate) data_cache_error_records: Vec<RiscvTraceErrorRecord>,
    pub(crate) trace_htm_access_records: Vec<RiscvTraceHtmAccessRecord>,
    pub(crate) store_conditional_failure_diagnostics: Vec<RiscvStoreConditionalFailureDiagnostic>,
    pub(crate) retired_instruction_probes: Option<RiscvRetiredInstructionProbeSnapshot>,
    pub(crate) data_access_probes: Option<RiscvDataAccessProbeSnapshot>,
    riscv_debug_console: Vec<u8>,
}

impl RiscvSystemRun {
    pub fn new(
        turns: Vec<RiscvClusterTurn>,
        scheduled_traps: Vec<ScheduledRiscvTrap>,
        stop_reason: RiscvSystemRunStopReason,
    ) -> Self {
        Self {
            turns,
            scheduled_traps,
            stop_reason,
            fabric_hop_activity: Vec::new(),
            fabric_activity: Vec::new(),
            fabric_wait_for: WaitForGraph::new(),
            dram_activity: Vec::new(),
            dram_wait_for: WaitForGraph::new(),
            data_cache_runs: Vec::new(),
            data_cache_run_protocols: Vec::new(),
            trace_diagnostic_records: Vec::new(),
            trace_error_records: Vec::new(),
            data_cache_error_records: Vec::new(),
            trace_htm_access_records: Vec::new(),
            store_conditional_failure_diagnostics: Vec::new(),
            retired_instruction_probes: None,
            data_access_probes: None,
            riscv_debug_console: Vec::new(),
        }
    }

    pub fn with_fabric_hop_activity(mut self, fabric_hop_activity: Vec<FabricHopActivity>) -> Self {
        self.fabric_hop_activity = fabric_hop_activity;
        self
    }

    pub fn with_fabric_activity(mut self, fabric_activity: Vec<FabricLaneActivity>) -> Self {
        self.fabric_activity = fabric_activity;
        self
    }

    pub fn with_dram_activity(mut self, dram_activity: Vec<DramTargetActivity>) -> Self {
        self.dram_activity = dram_activity;
        self
    }

    pub fn with_data_cache_runs(
        mut self,
        data_cache_runs: Vec<ParallelCoherenceRunSummary>,
    ) -> Self {
        self.data_cache_run_protocols = vec![None; data_cache_runs.len()];
        self.data_cache_runs = data_cache_runs;
        self
    }

    pub fn with_data_cache_run_records(
        mut self,
        data_cache_run_records: Vec<RiscvDataCacheRunRecord>,
    ) -> Self {
        self.data_cache_run_protocols = data_cache_run_records
            .iter()
            .map(RiscvDataCacheRunRecord::protocol)
            .collect();
        self.data_cache_runs = data_cache_run_records
            .into_iter()
            .map(RiscvDataCacheRunRecord::into_summary)
            .collect();
        self
    }

    pub fn turns(&self) -> &[RiscvClusterTurn] {
        &self.turns
    }

    pub fn scheduled_traps(&self) -> &[ScheduledRiscvTrap] {
        &self.scheduled_traps
    }

    pub fn with_riscv_debug_console_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.riscv_debug_console = bytes;
        self
    }

    pub fn riscv_debug_console_bytes(&self) -> &[u8] {
        &self.riscv_debug_console
    }

    pub fn cpu_activity(&self, cpu: CpuId) -> Option<RiscvSystemRunCpuActivity> {
        self.cpu_activities().remove(&cpu)
    }

    pub fn has_cpu_activity(&self, cpu: CpuId) -> bool {
        self.cpu_activity(cpu)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_cpu_count(&self) -> usize {
        self.cpu_activities().len()
    }

    pub fn cpu_activities(&self) -> BTreeMap<CpuId, RiscvSystemRunCpuActivity> {
        riscv_run_activity::collect_riscv_system_run_cpu_activity(
            &self.turns,
            &self.scheduled_traps,
        )
    }

    pub fn partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<RiscvSystemRunPartitionActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, RiscvSystemRunPartitionActivity> {
        riscv_run_activity::collect_riscv_system_run_partition_activity(
            &self.turns,
            &self.scheduled_traps,
        )
    }

    pub const fn stop_reason(&self) -> RiscvSystemRunStopReason {
        self.stop_reason
    }

    pub const fn host_stop(&self) -> Option<StopRequest> {
        match self.stop_reason {
            RiscvSystemRunStopReason::HostStop(stop) => Some(stop),
            RiscvSystemRunStopReason::DebugStop { .. }
            | RiscvSystemRunStopReason::Idle { .. }
            | RiscvSystemRunStopReason::TickLimit { .. }
            | RiscvSystemRunStopReason::InstructionLimit { .. } => None,
        }
    }

    pub const fn final_tick(&self) -> Option<Tick> {
        match self.stop_reason {
            RiscvSystemRunStopReason::HostStop(stop) => Some(stop.tick()),
            RiscvSystemRunStopReason::DebugStop { tick } => Some(tick),
            RiscvSystemRunStopReason::Idle { tick } => Some(tick),
            RiscvSystemRunStopReason::TickLimit { tick, .. } => Some(tick),
            RiscvSystemRunStopReason::InstructionLimit { tick, .. } => Some(tick),
        }
    }

    pub fn parallel_scheduler_epochs(&self) -> Vec<&RiscvClusterSchedulerEpoch> {
        self.turns
            .iter()
            .filter_map(RiscvClusterTurn::parallel_scheduler_epoch)
            .collect()
    }

    pub(crate) fn parallel_safe_scheduler_epochs(&self) -> Vec<&RiscvClusterSchedulerEpoch> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .filter(|epoch| epoch.is_parallel_safe())
            .collect()
    }

    pub fn parallel_scheduler_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.dispatches().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.batches().iter().cloned())
            .collect()
    }

    pub fn parallel_scheduler_workers(&self) -> Vec<ParallelWorkerRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(RiscvClusterSchedulerEpoch::workers)
            .collect()
    }

    pub fn parallel_scheduler_worker_partitions(&self) -> Vec<PartitionId> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(RiscvClusterSchedulerEpoch::parallel_worker_partitions)
            .collect()
    }

    pub fn max_parallel_scheduler_workers(&self) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(RiscvClusterSchedulerEpoch::max_parallel_workers)
            .max()
            .unwrap_or(0)
    }

    pub fn parallel_scheduler_profile(&self) -> ParallelRunProfile {
        self.parallel_scheduler_epochs()
            .into_iter()
            .fold(ParallelRunProfile::default(), |profile, epoch| {
                profile.merge(epoch.profile())
            })
    }

    pub fn parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.parallel_scheduler_partition_activities()
            .remove(&partition)
    }

    pub fn has_parallel_scheduler_partition_activity(&self, partition: PartitionId) -> bool {
        self.parallel_scheduler_partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_parallel_scheduler_partition_count(&self) -> usize {
        self.parallel_scheduler_partition_activities().len()
    }

    pub fn parallel_scheduler_partition_activities(
        &self,
    ) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = BTreeMap::new();
        for epoch in self.parallel_scheduler_epochs() {
            merge_parallel_partition_activity_maps(&mut activities, epoch.partition_activities());
        }
        activities
    }

    pub fn parallel_scheduler_dispatches_for_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<SchedulerDispatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.dispatches_for_partition(partition))
            .collect()
    }

    pub fn parallel_scheduler_frontiers(&self) -> Vec<PartitionFrontier> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.frontiers().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_final_frontiers(&self) -> Vec<PartitionFrontier> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.final_frontiers().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_ready_partitions(&self) -> Vec<ReadyPartition> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.ready_partitions().iter().copied())
            .collect()
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

    pub fn fabric_hop_activities(&self) -> &[FabricHopActivity] {
        &self.fabric_hop_activity
    }

    pub fn fabric_virtual_network_activity(
        &self,
        virtual_network: VirtualNetworkId,
    ) -> Option<FabricVirtualNetworkActivity> {
        self.fabric_virtual_network_activities()
            .remove(&virtual_network)
    }

    pub fn fabric_virtual_network_activities(
        &self,
    ) -> BTreeMap<VirtualNetworkId, FabricVirtualNetworkActivity> {
        collect_run_fabric_virtual_network_activity(self.fabric_activities().values())
    }

    pub fn fabric_profile(&self) -> FabricActivityProfile {
        let activities = self.fabric_activities();
        FabricActivityProfile::from_lanes(activities.values())
    }

    pub fn active_fabric_lane_count(&self) -> usize {
        self.fabric_activities().len()
    }

    pub fn active_fabric_virtual_network_count(&self) -> usize {
        self.fabric_virtual_network_activities().len()
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

    pub fn dram_access_count(&self) -> usize {
        self.dram_profile().access_count()
    }

    pub fn has_dram_activity(&self) -> bool {
        let dram = self.dram_profile();
        self.dram_operation_count() != 0
            || dram.turnaround_count() != 0
            || dram.total_ready_latency_cycles() != 0
            || dram.max_ready_latency_cycles() != 0
            || self.has_dram_qos_activity()
            || self.has_dram_low_power_activity()
    }

    pub fn resource_activity_count(&self) -> usize {
        self.fabric_transfer_count()
            .saturating_add(self.dram_operation_count())
            .saturating_add(self.fabric_wait_for_edge_count())
            .saturating_add(self.dram_wait_for_edge_count())
    }

    pub fn has_resource_activity(&self) -> bool {
        self.resource_activity_count() != 0
    }
}

fn merge_parallel_partition_activity_maps(
    target: &mut BTreeMap<PartitionId, ParallelPartitionActivity>,
    source: &BTreeMap<PartitionId, ParallelPartitionActivity>,
) {
    for (partition, activity) in source {
        target
            .entry(*partition)
            .and_modify(|stored| {
                *stored = ParallelPartitionActivity::with_remote_counts(
                    stored.worker_count() + activity.worker_count(),
                    stored.dispatch_count() + activity.dispatch_count(),
                    stored.remote_send_count() + activity.remote_send_count(),
                    stored.remote_receive_count() + activity.remote_receive_count(),
                    stored
                        .max_pending_events()
                        .max(activity.max_pending_events()),
                );
            })
            .or_insert(*activity);
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

fn collect_run_fabric_virtual_network_activity<'a>(
    source: impl IntoIterator<Item = &'a FabricLaneActivity>,
) -> BTreeMap<VirtualNetworkId, FabricVirtualNetworkActivity> {
    FabricVirtualNetworkActivity::from_lanes(source)
        .into_iter()
        .map(|activity| (activity.virtual_network(), activity))
        .collect()
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
                *stored = stored.clone().merge_window(activity.clone());
            })
            .or_insert_with(|| activity.clone());
    }
}
