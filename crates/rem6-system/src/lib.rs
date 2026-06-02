use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::CheckpointError;
use rem6_coherence::ParallelCoherenceRunSummary;
use rem6_cpu::{
    CpuId, RiscvCluster, RiscvClusterError, RiscvClusterSchedulerEpoch, RiscvClusterTurn,
    RiscvCoreDriveAction, RiscvStoreConditionalFailureDiagnostic,
};
use rem6_dram::{DramMemoryActivityProfile, DramTargetActivity};
use rem6_fabric::{
    FabricActivityProfile, FabricHopActivity, FabricLaneActivity, FabricLinkId,
    FabricVirtualNetworkActivity, VirtualNetworkId,
};
use rem6_kernel::{
    ParallelEpochBatchRecord, ParallelPartitionActivity, ParallelRunProfile,
    ParallelSchedulerContext, ParallelWorkerRecord, PartitionFrontier, PartitionId,
    PartitionedScheduler, ReadyPartition, SchedulerContext, SchedulerDispatchRecord,
    SchedulerError, Tick, WaitForGraph,
};
use rem6_memory::MemoryTargetId;
use rem6_mmio::MmioBus;
use rem6_stats::{StatId, StatsError};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

mod clint_checkpoint;
mod coherence_checkpoint;
mod cpu_local_timer_checkpoint;
mod data_cache_run;
mod fabric_checkpoint;
mod fabric_wait_run;
mod guest_event;
mod guest_fd;
mod guest_futex;
mod heterogeneous_checkpoint;
mod host;
mod host_assist;
mod interrupt_checkpoint;
mod memory_checkpoint;
mod pci_checkpoint;
mod pci_interrupt_checkpoint;
mod pl031_checkpoint;
mod plic_checkpoint;
mod riscv_checkpoint;
mod riscv_debug;
mod riscv_run_activity;
mod riscv_run_control;
mod riscv_run_translation;
mod rtc_checkpoint;
mod scheduler_checkpoint;
mod sp804_checkpoint;
mod sp805_checkpoint;
mod system_run_parallel_batch;
mod system_run_planned_lanes;
mod system_run_progress;
mod system_run_qos;
mod system_run_remote_flow;
mod system_run_sc_progress;
mod system_run_worker_lanes;
mod timer_checkpoint;
mod topology;
mod trap_event;
mod uart_checkpoint;
mod virtio_checkpoint;
mod wait_status;
mod workload_replay;
mod workload_replay_heterogeneous;
mod workload_replay_host;

pub use clint_checkpoint::{
    ClintCheckpointBank, ClintCheckpointError, ClintCheckpointPort, ClintCheckpointRecord,
};
pub use coherence_checkpoint::{
    MsiBankCheckpointBank, MsiBankCheckpointError, MsiBankCheckpointPort, MsiBankCheckpointRecord,
};
pub use cpu_local_timer_checkpoint::{
    CpuLocalTimerCheckpointBank, CpuLocalTimerCheckpointError, CpuLocalTimerCheckpointPort,
    CpuLocalTimerCheckpointRecord,
};
pub use data_cache_run::{
    RiscvDataCacheProtocol, RiscvDataCacheRunHistoryRecord, RiscvDataCacheRunRecord,
};
pub use fabric_checkpoint::{
    FabricCheckpointBank, FabricCheckpointError, FabricCheckpointPort, FabricCheckpointRecord,
};
pub use guest_event::{
    ExecutionMode, ExecutionModeTarget, GuestEvent, GuestEventChannel, GuestEventDelivery,
    GuestEventId, GuestEventKind, GuestHostCallResponse, GuestSourceId, GuestTrap, GuestTrapKind,
    HostAction, HostActionRecord, HostEventPolicy, StopRequest,
};
pub use guest_fd::{GuestFd, GuestFdEntry, GuestFdError, GuestFdTable, GuestFileDescriptionId};
pub use guest_futex::{
    GuestFutexAddress, GuestFutexError, GuestFutexKey, GuestFutexRequeueOutcome,
    GuestFutexRequeueRecord, GuestFutexTable, GuestFutexWaitOutcome, GuestFutexWaitRequest,
    GuestFutexWaiter, GuestFutexWakeOutcome, GuestFutexWakeRecord, GuestThreadGroupId,
    GuestThreadId,
};
pub use heterogeneous_checkpoint::{
    AcceleratorCheckpointBank, AcceleratorCheckpointError, AcceleratorCheckpointPort,
    AcceleratorCheckpointRecord, GpuCheckpointBank, GpuCheckpointError, GpuCheckpointPort,
    GpuCheckpointRecord,
};
pub use host::{
    ExecutionModeCheckpointError, SystemActionExecutor, SystemActionOutcome, SystemHostController,
    SystemRunController,
};
pub use host_assist::{
    HostAssistedArchitecture, HostAssistedMemoryMode, HostAssistedPendingService,
    HostAssistedRegisterId, HostAssistedRegisterSpace, HostAssistedSimulationMode,
    HostAssistedStateComponent, HostAssistedSwitchAction, HostAssistedSwitchError,
    HostAssistedSwitchPlan, HostAssistedSwitchPlanner, HostAssistedSwitchRequest,
};
pub use interrupt_checkpoint::{
    InterruptControllerCheckpointBank, InterruptControllerCheckpointError,
    InterruptControllerCheckpointPort, InterruptControllerCheckpointRecord,
};
pub use memory_checkpoint::{
    DramMemoryCheckpointBank, DramMemoryCheckpointError, DramMemoryCheckpointPort,
    DramMemoryCheckpointRecord, MemoryStoreCheckpointBank, MemoryStoreCheckpointError,
    MemoryStoreCheckpointPort, MemoryStoreCheckpointRecord,
};
pub use pci_checkpoint::{
    PciHostCheckpointBank, PciHostCheckpointError, PciHostCheckpointPort, PciHostCheckpointRecord,
};
pub use pci_interrupt_checkpoint::{
    PciLegacyInterruptRouterCheckpointBank, PciLegacyInterruptRouterCheckpointError,
    PciLegacyInterruptRouterCheckpointPort, PciLegacyInterruptRouterCheckpointRecord,
};
pub use pl031_checkpoint::{
    Pl031CheckpointBank, Pl031CheckpointError, Pl031CheckpointPort, Pl031CheckpointRecord,
};
pub use plic_checkpoint::{
    PlicCheckpointBank, PlicCheckpointError, PlicCheckpointPort, PlicCheckpointRecord,
};
pub use rem6_storage::{
    IdeControllerCheckpointBank, IdeControllerCheckpointPort, IdeControllerCheckpointRecord,
    StorageCheckpointError, StorageImageCheckpointBank, StorageImageCheckpointPort,
    StorageImageCheckpointRecord, StorageImageCheckpointSnapshot,
};
pub use riscv_checkpoint::{
    RiscvCoreCheckpointBank, RiscvCoreCheckpointError, RiscvCoreCheckpointPort,
    RiscvCoreCheckpointRecord,
};
pub use riscv_debug::{
    apply_riscv_gdb_remote_core_register_write, apply_riscv_gdb_remote_register_write,
    handle_riscv_gdb_remote_core_packet, handle_riscv_gdb_remote_packet, riscv_gdb_remote_session,
    riscv_gdb_remote_session_from_core, riscv_gdb_remote_session_from_hart,
    RiscvGdbRegisterWriteError, RiscvGdbRemotePacketError,
};
pub use riscv_run_activity::{RiscvSystemRunCpuActivity, RiscvSystemRunPartitionActivity};
pub use rtc_checkpoint::{
    RtcCheckpointBank, RtcCheckpointError, RtcCheckpointPort, RtcCheckpointRecord,
};
pub use scheduler_checkpoint::{
    SchedulerCheckpointBank, SchedulerCheckpointError, SchedulerCheckpointPendingPartition,
    SchedulerCheckpointPort, SchedulerCheckpointQuiescenceReport, SchedulerCheckpointRecord,
};
pub use sp804_checkpoint::{
    Sp804CheckpointBank, Sp804CheckpointError, Sp804CheckpointPort, Sp804CheckpointRecord,
};
pub use sp805_checkpoint::{
    Sp805CheckpointBank, Sp805CheckpointError, Sp805CheckpointPort, Sp805CheckpointRecord,
};
pub use system_run_parallel_batch::{
    RiscvSystemParallelBatchScope, RiscvSystemParallelBatchTimelineRecord,
};
pub use system_run_planned_lanes::RiscvSystemParallelBatchWorkerLaneRecord;
pub use system_run_worker_lanes::RiscvSystemParallelWorkerLaneRecord;
pub use timer_checkpoint::{
    TimerCheckpointBank, TimerCheckpointError, TimerCheckpointPort, TimerCheckpointRecord,
};
pub use topology::{
    RiscvDtbHandoffReport, RiscvLinuxBootHandoffConfig, RiscvLinuxBootHandoffReport,
    RiscvLinuxInitrdImage, RiscvTopologyAcceleratorComputeActivity, RiscvTopologyDmaCopy,
    RiscvTopologyDmaDeviceActivity, RiscvTopologyDmaRunSummary, RiscvTopologyDmaStageRunSummary,
    RiscvTopologyDramConfig, RiscvTopologyGpuComputeActivity, RiscvTopologyHeterogeneousRunSummary,
    RiscvTopologyHeterogeneousWork, RiscvTopologyHostConfig, RiscvTopologyMemoryConfig,
    RiscvTopologyMemoryRegion, RiscvTopologySystem, RiscvTopologySystemError,
};
pub(crate) use trap_event::pending_trap_cores_from_turn;
pub use trap_event::{
    guest_trap_from_riscv, guest_trap_kind_from_riscv, RiscvTrapEventPort, ScheduledRiscvTrap,
    SystemEventPort, SystemHostEventPort,
};
pub use uart_checkpoint::{
    Pl011UartCheckpointBank, Pl011UartCheckpointError, Pl011UartCheckpointPort,
    Pl011UartCheckpointRecord, UartCheckpointBank, UartCheckpointError, UartCheckpointPort,
    UartCheckpointRecord,
};
pub use virtio_checkpoint::{
    VirtioPciCommonCheckpointBank, VirtioPciCommonCheckpointError, VirtioPciCommonCheckpointPort,
    VirtioPciCommonCheckpointRecord, VirtioPciDeviceConfigCheckpointBank,
    VirtioPciDeviceConfigCheckpointError, VirtioPciDeviceConfigCheckpointPort,
    VirtioPciDeviceConfigCheckpointRecord, VirtioPciIsrCheckpointBank, VirtioPciIsrCheckpointError,
    VirtioPciIsrCheckpointPort, VirtioPciIsrCheckpointRecord, VirtioPciNotifyCheckpointBank,
    VirtioPciNotifyCheckpointError, VirtioPciNotifyCheckpointPort, VirtioPciNotifyCheckpointRecord,
    VirtioSplitQueueCheckpointBank, VirtioSplitQueueCheckpointError,
    VirtioSplitQueueCheckpointPort, VirtioSplitQueueCheckpointRecord,
};
pub use wait_status::{GuestSignal, GuestWaitStatus, GuestWaitStatusError};
pub use workload_replay::{
    RiscvWorkloadReplay, RiscvWorkloadReplayError, RiscvWorkloadReplayOutcome,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSystemRunStopReason {
    HostStop(StopRequest),
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
    pub(crate) store_conditional_failure_diagnostics: Vec<RiscvStoreConditionalFailureDiagnostic>,
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
            store_conditional_failure_diagnostics: Vec::new(),
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
            RiscvSystemRunStopReason::Idle { .. }
            | RiscvSystemRunStopReason::TickLimit { .. }
            | RiscvSystemRunStopReason::InstructionLimit { .. } => None,
        }
    }

    pub const fn final_tick(&self) -> Option<Tick> {
        match self.stop_reason {
            RiscvSystemRunStopReason::HostStop(stop) => Some(stop.tick()),
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

#[derive(Clone, Debug)]
pub struct RiscvSystemRunDriver {
    trap_port: RiscvTrapEventPort,
    instruction_stats: Option<RiscvInstructionStats>,
}

impl RiscvSystemRunDriver {
    pub const fn new(trap_port: RiscvTrapEventPort) -> Self {
        Self {
            trap_port,
            instruction_stats: None,
        }
    }

    pub const fn with_instruction_stats(
        trap_port: RiscvTrapEventPort,
        instruction_stats: RiscvInstructionStats,
    ) -> Self {
        Self {
            trap_port,
            instruction_stats: Some(instruction_stats),
        }
    }

    pub const fn trap_port(&self) -> &RiscvTrapEventPort {
        &self.trap_port
    }

    pub const fn instruction_stats(&self) -> Option<&RiscvInstructionStats> {
        self.instruction_stats.as_ref()
    }

    pub(crate) fn run_result(
        &self,
        cluster: &RiscvCluster,
        turns: Vec<RiscvClusterTurn>,
        scheduled_traps: Vec<ScheduledRiscvTrap>,
        stop_reason: RiscvSystemRunStopReason,
    ) -> RiscvSystemRun {
        RiscvSystemRun::new(turns, scheduled_traps, stop_reason)
            .with_store_conditional_failure_diagnostics(
                cluster.store_conditional_failure_diagnostics(),
            )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        DR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            let turn = cluster
                .drive_turn(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_instruction_stats(&turn)?;
            let trap_cores = trap_event::pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port.schedule_pending_core_traps(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_parallel<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            let turn = cluster
                .drive_turn_parallel(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_instruction_stats(&turn)?;
            let trap_cores = trap_event::pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port.schedule_pending_core_traps_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_parallel_with_mmio<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            let turn = cluster
                .drive_turn_parallel_with_mmio(
                    scheduler,
                    transport,
                    bus,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_instruction_stats(&turn)?;
            let trap_cores = trap_event::pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port.schedule_pending_core_traps_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }

    pub(crate) fn host_stop_request(&self) -> Option<StopRequest> {
        self.trap_port
            .controller()
            .lock()
            .expect("system host controller lock")
            .run()
            .stop_request()
            .copied()
    }

    pub(crate) fn record_instruction_stats(
        &self,
        turn: &RiscvClusterTurn,
    ) -> Result<(), SystemError> {
        let Some(instruction_stats) = &self.instruction_stats else {
            return Ok(());
        };

        let controller = self.trap_port.controller();
        let mut controller = controller.lock().expect("system host controller lock");
        for event in turn.core_events() {
            if matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)) {
                if let Some(stat) = instruction_stats.committed_stat(event.cpu()) {
                    controller
                        .executor_mut()
                        .stats_mut()
                        .increment(stat, 1)
                        .map_err(SystemError::Stats)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RiscvInstructionStats {
    committed: BTreeMap<CpuId, StatId>,
}

impl RiscvInstructionStats {
    pub fn new<I>(committed: I) -> Self
    where
        I: IntoIterator<Item = (CpuId, StatId)>,
    {
        Self {
            committed: committed.into_iter().collect(),
        }
    }

    pub fn committed_stat(&self, cpu: CpuId) -> Option<StatId> {
        self.committed.get(&cpu).copied()
    }

    pub fn committed_stats(&self) -> &BTreeMap<CpuId, StatId> {
        &self.committed
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemError {
    ZeroHostLatency,
    Scheduler(SchedulerError),
    RiscvCluster(RiscvClusterError),
    Stats(StatsError),
    Checkpoint(CheckpointError),
    MissingCheckpointManifest { label: String },
    ExecutionModeCheckpoint(ExecutionModeCheckpointError),
    AcceleratorCheckpoint(AcceleratorCheckpointError),
    CpuLocalTimerCheckpoint(CpuLocalTimerCheckpointError),
    MsiBankCheckpoint(MsiBankCheckpointError),
    FabricCheckpoint(FabricCheckpointError),
    GpuCheckpoint(GpuCheckpointError),
    PciHostCheckpoint(PciHostCheckpointError),
    PciLegacyInterruptRouterCheckpoint(PciLegacyInterruptRouterCheckpointError),
    Pl031Checkpoint(Pl031CheckpointError),
    Sp804Checkpoint(Sp804CheckpointError),
    Sp805Checkpoint(Sp805CheckpointError),
    RiscvCheckpoint(RiscvCoreCheckpointError),
    RtcCheckpoint(RtcCheckpointError),
    SchedulerCheckpoint(SchedulerCheckpointError),
    MemoryCheckpoint(MemoryStoreCheckpointError),
    StorageCheckpoint(StorageCheckpointError),
    DramMemoryCheckpoint(DramMemoryCheckpointError),
    InterruptControllerCheckpoint(InterruptControllerCheckpointError),
    ClintCheckpoint(ClintCheckpointError),
    TimerCheckpoint(TimerCheckpointError),
    UartCheckpoint(UartCheckpointError),
    Pl011UartCheckpoint(Pl011UartCheckpointError),
    PlicCheckpoint(PlicCheckpointError),
    VirtioPciCommonCheckpoint(VirtioPciCommonCheckpointError),
    VirtioPciDeviceConfigCheckpoint(VirtioPciDeviceConfigCheckpointError),
    VirtioPciIsrCheckpoint(VirtioPciIsrCheckpointError),
    VirtioPciNotifyCheckpoint(VirtioPciNotifyCheckpointError),
    VirtioCheckpoint(VirtioSplitQueueCheckpointError),
}

impl fmt::Display for SystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroHostLatency => {
                write!(formatter, "guest event channel latency must be positive")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::RiscvCluster(error) => write!(formatter, "{error}"),
            Self::Stats(error) => write!(formatter, "{error}"),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::MissingCheckpointManifest { label } => {
                write!(formatter, "checkpoint manifest {label} is not available")
            }
            Self::ExecutionModeCheckpoint(error) => write!(formatter, "{error}"),
            Self::AcceleratorCheckpoint(error) => write!(formatter, "{error}"),
            Self::CpuLocalTimerCheckpoint(error) => write!(formatter, "{error}"),
            Self::MsiBankCheckpoint(error) => write!(formatter, "{error}"),
            Self::FabricCheckpoint(error) => write!(formatter, "{error}"),
            Self::GpuCheckpoint(error) => write!(formatter, "{error}"),
            Self::PciHostCheckpoint(error) => write!(formatter, "{error}"),
            Self::PciLegacyInterruptRouterCheckpoint(error) => write!(formatter, "{error}"),
            Self::Pl031Checkpoint(error) => write!(formatter, "{error}"),
            Self::Sp804Checkpoint(error) => write!(formatter, "{error}"),
            Self::Sp805Checkpoint(error) => write!(formatter, "{error}"),
            Self::RiscvCheckpoint(error) => write!(formatter, "{error}"),
            Self::RtcCheckpoint(error) => write!(formatter, "{error}"),
            Self::SchedulerCheckpoint(error) => write!(formatter, "{error}"),
            Self::MemoryCheckpoint(error) => write!(formatter, "{error}"),
            Self::StorageCheckpoint(error) => write!(formatter, "{error}"),
            Self::DramMemoryCheckpoint(error) => write!(formatter, "{error}"),
            Self::InterruptControllerCheckpoint(error) => write!(formatter, "{error}"),
            Self::ClintCheckpoint(error) => write!(formatter, "{error}"),
            Self::TimerCheckpoint(error) => write!(formatter, "{error}"),
            Self::UartCheckpoint(error) => write!(formatter, "{error}"),
            Self::Pl011UartCheckpoint(error) => write!(formatter, "{error}"),
            Self::PlicCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciCommonCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciDeviceConfigCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciIsrCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciNotifyCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioCheckpoint(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for SystemError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::RiscvCluster(error) => Some(error),
            Self::Stats(error) => Some(error),
            Self::Checkpoint(error) => Some(error),
            Self::MissingCheckpointManifest { .. } => None,
            Self::ExecutionModeCheckpoint(error) => Some(error),
            Self::AcceleratorCheckpoint(error) => Some(error),
            Self::CpuLocalTimerCheckpoint(error) => Some(error),
            Self::MsiBankCheckpoint(error) => Some(error),
            Self::FabricCheckpoint(error) => Some(error),
            Self::GpuCheckpoint(error) => Some(error),
            Self::PciHostCheckpoint(error) => Some(error),
            Self::PciLegacyInterruptRouterCheckpoint(error) => Some(error),
            Self::Pl031Checkpoint(error) => Some(error),
            Self::Sp804Checkpoint(error) => Some(error),
            Self::Sp805Checkpoint(error) => Some(error),
            Self::RiscvCheckpoint(error) => Some(error),
            Self::RtcCheckpoint(error) => Some(error),
            Self::SchedulerCheckpoint(error) => Some(error),
            Self::MemoryCheckpoint(error) => Some(error),
            Self::StorageCheckpoint(error) => Some(error),
            Self::DramMemoryCheckpoint(error) => Some(error),
            Self::InterruptControllerCheckpoint(error) => Some(error),
            Self::ClintCheckpoint(error) => Some(error),
            Self::TimerCheckpoint(error) => Some(error),
            Self::UartCheckpoint(error) => Some(error),
            Self::Pl011UartCheckpoint(error) => Some(error),
            Self::PlicCheckpoint(error) => Some(error),
            Self::VirtioPciCommonCheckpoint(error) => Some(error),
            Self::VirtioPciDeviceConfigCheckpoint(error) => Some(error),
            Self::VirtioPciIsrCheckpoint(error) => Some(error),
            Self::VirtioPciNotifyCheckpoint(error) => Some(error),
            Self::VirtioCheckpoint(error) => Some(error),
            Self::ZeroHostLatency => None,
        }
    }
}
