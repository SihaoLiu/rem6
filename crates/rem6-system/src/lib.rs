use std::collections::BTreeSet;

use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterError, RiscvClusterTurn, RiscvCoreDriveAction};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, SchedulerContext, Tick,
};
use rem6_mmio::MmioBus;
use rem6_stats::StatsRegistry;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

mod clint_checkpoint;
mod coherence_checkpoint;
mod cpu_local_timer_checkpoint;
mod data_cache_controller_error;
mod data_cache_run;
mod fabric_checkpoint;
mod fabric_wait_run;
mod guest_event;
mod guest_fd;
mod guest_fd_checkpoint;
mod guest_futex;
mod guest_futex_checkpoint;
mod heterogeneous_checkpoint;
mod host;
mod host_assist;
mod interrupt_checkpoint;
mod memory_checkpoint;
mod net_checkpoint;
mod pci_checkpoint;
mod pci_interrupt_checkpoint;
mod pl031_checkpoint;
mod plic_checkpoint;
mod readfile_checkpoint;
mod riscv_checkpoint;
mod riscv_data_access_stats;
mod riscv_debug;
mod riscv_debug_layout;
mod riscv_debug_page_table;
mod riscv_debug_pmp;
mod riscv_instruction_stats;
mod riscv_o3_runtime_stats;
mod riscv_run_activity;
mod riscv_run_control;
mod riscv_run_driver;
mod riscv_run_translation;
mod riscv_sbi;
mod riscv_syscall;
mod riscv_system_run;
mod rtc_checkpoint;
mod scheduler_checkpoint;
mod sp804_checkpoint;
mod sp805_checkpoint;
mod system_error;
mod system_run_parallel_batch;
mod system_run_planned_lanes;
mod system_run_progress;
mod system_run_qos;
mod system_run_remote_flow;
mod system_run_sc_progress;
mod system_run_worker_lanes;
mod timer_checkpoint;
mod topology;
mod trace_diagnostic;
mod trace_error;
mod trace_htm_access;
mod traffic_gups;
mod traffic_replay;
mod traffic_replay_cpu;
mod trap_event;
mod uart_checkpoint;
mod virtio_checkpoint;
mod wait_checkpoint;
mod wait_status;
mod workload_gups;
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
pub use data_cache_controller_error::{
    RiscvDataCacheControllerError, RiscvDataCacheControllerErrorRecord,
    RiscvDataCacheControllerErrorSource,
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
pub use guest_fd::{
    GuestFd, GuestFdCloseRecord, GuestFdDup2Record, GuestFdEntry, GuestFdError,
    GuestFdSnapshotEntry, GuestFdTable, GuestFdTableSnapshot, GuestFileDescription,
    GuestFileDescriptionId, GuestFileOffset, GuestFileSignalOwner, GuestFileSignalOwnerError,
    GuestFileSignalOwnerKind, GuestFileStatusFlags, GuestHostFd,
};
pub use guest_fd_checkpoint::{
    GuestFdCheckpointBank, GuestFdCheckpointError, GuestFdCheckpointPort, GuestFdCheckpointRecord,
};
pub use guest_futex::{
    GuestFutexAddress, GuestFutexError, GuestFutexKey, GuestFutexRequeueOutcome,
    GuestFutexRequeueRecord, GuestFutexTable, GuestFutexTableSnapshot, GuestFutexWaitOutcome,
    GuestFutexWaitRequest, GuestFutexWaiter, GuestFutexWakeOutcome, GuestFutexWakeRecord,
    GuestThreadGroupId, GuestThreadId,
};
pub use guest_futex_checkpoint::{
    GuestFutexCheckpointBank, GuestFutexCheckpointError, GuestFutexCheckpointPort,
    GuestFutexCheckpointRecord,
};
pub use heterogeneous_checkpoint::{
    AcceleratorCheckpointBank, AcceleratorCheckpointError, AcceleratorCheckpointPort,
    AcceleratorCheckpointRecord, GpuCheckpointBank, GpuCheckpointError, GpuCheckpointPort,
    GpuCheckpointRecord,
};
pub use host::{
    ExecutionModeCheckpointError, ExecutionModeSwitchCheckerGate,
    ExecutionModeSwitchQuiescenceGate, ExecutionModeSwitchStateTransfer,
    ExecutionModeSwitchStateTransferChunk, ExecutionModeSwitchStateTransferComponent,
    SystemActionExecutor, SystemActionOutcome, SystemHostController, SystemRunController,
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
pub use net_checkpoint::{
    SinicFifoCheckpointBank, SinicFifoCheckpointError, SinicFifoCheckpointPort,
    SinicFifoCheckpointRecord, SinicRegisterCheckpointBank, SinicRegisterCheckpointError,
    SinicRegisterCheckpointPort, SinicRegisterCheckpointRecord,
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
pub use readfile_checkpoint::*;
pub use rem6_storage::{
    IdeControllerCheckpointBank, IdeControllerCheckpointPort, IdeControllerCheckpointRecord,
    StorageCheckpointError, StorageImageCheckpointBank, StorageImageCheckpointPort,
    StorageImageCheckpointRecord, StorageImageCheckpointSnapshot,
};
pub use riscv_checkpoint::{
    RiscvCoreCheckpointBank, RiscvCoreCheckpointError, RiscvCoreCheckpointPort,
    RiscvCoreCheckpointRecord,
};
pub use riscv_data_access_stats::{RiscvDataAccessProbeSnapshot, RiscvDataAccessStats};
pub use riscv_debug::{
    apply_riscv_gdb_remote_core_register_write, apply_riscv_gdb_remote_register_write,
    handle_riscv_gdb_remote_cluster_packet, handle_riscv_gdb_remote_core_packet,
    handle_riscv_gdb_remote_memory_packet, handle_riscv_gdb_remote_packet,
    handle_riscv_gdb_remote_system_packet,
    handle_riscv_gdb_remote_system_packet_with_data_translation,
    riscv_gdb_page_table_dump_from_translation_map, riscv_gdb_remote_session,
    riscv_gdb_remote_session_from_cluster, riscv_gdb_remote_session_from_core,
    riscv_gdb_remote_session_from_hart, riscv_gdb_remote_session_from_translation_map,
    riscv_gdb_remote_session_with_page_table_dump, riscv_gdb_remote_thread_id,
    sync_riscv_gdb_remote_threads_from_cluster, RiscvGdbRegisterWriteError,
    RiscvGdbRemotePacketError,
};
pub use riscv_instruction_stats::{RiscvInstructionStats, RiscvRetiredInstructionProbeSnapshot};
pub use riscv_o3_runtime_stats::RiscvO3RuntimeStats;
pub use riscv_run_activity::{RiscvSystemRunCpuActivity, RiscvSystemRunPartitionActivity};
pub use riscv_sbi::{
    RiscvSbiFirmware, RiscvSbiHsmRecord, RiscvSbiHsmStatusRecord, RiscvSbiHsmWakeRecord,
    RiscvSbiIpiRecord, RiscvSbiOutcome, RiscvSbiRequest, RiscvSbiResetRecord,
    RiscvSbiRfenceCompletionRecord, RiscvSbiRfenceRecord,
};
pub use riscv_syscall::{
    RiscvGuestFileIdentity, RiscvGuestMemoryMapRequest, RiscvGuestMemoryMapResult,
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvGuestOpenRecord, RiscvGuestWriteRecord,
    RiscvMmapRegion, RiscvSeAuxvEntry, RiscvSeStartupConfig, RiscvSeStartupError,
    RiscvSeStartupImage, RiscvSeStartupStringField, RiscvSyscallEmulation,
    RiscvSyscallImageLayoutError, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable, RiscvSyscallTraceOutcome, RiscvSyscallTraceRecord,
    RiscvUnknownSyscallRecord, RISCV_LINUX_AT_ENTRY, RISCV_LINUX_AT_NULL, RISCV_LINUX_AT_PAGESZ,
    RISCV_LINUX_AT_PHDR, RISCV_LINUX_AT_PHENT, RISCV_LINUX_AT_PHNUM, RISCV_LINUX_AT_RANDOM,
    RISCV_LINUX_AT_SECURE, RISCV_LINUX_STACK_LIMIT_BYTES,
};
pub use riscv_system_run::{RiscvSystemRun, RiscvSystemRunStopReason};
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
pub use system_error::SystemError;
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
    RiscvTopologyMemoryRegion, RiscvTopologySinicPciDeviceConfig, RiscvTopologySystem,
    RiscvTopologySystemError, RiscvTopologyWorkloadSinicPciError,
};
pub use trace_diagnostic::{RiscvTraceDiagnosticKind, RiscvTraceDiagnosticRecord};
pub use trace_error::RiscvTraceErrorRecord;
pub use trace_htm_access::{RiscvTraceHtmAccessKind, RiscvTraceHtmAccessRecord};
pub use traffic_gups::{
    traffic_gups_controller_transport_run, TrafficGupsTargetResponder, TrafficGupsTransportError,
    TrafficGupsTransportResponseStats, TrafficGupsTransportRun,
};
pub use traffic_replay::{
    traffic_trace_replay_controller_control_completion,
    traffic_trace_replay_controller_control_event,
    traffic_trace_replay_controller_runtime_control_completion_parallel,
    traffic_trace_replay_controller_runtime_memory_write_completions,
    traffic_trace_replay_controller_runtime_sideband_events,
    traffic_trace_replay_controller_runtime_target_outcome_parallel,
    traffic_trace_replay_controller_target_event, traffic_trace_replay_controller_target_outcome,
    traffic_trace_replay_runtime_control_completion, traffic_trace_replay_runtime_sideband_events,
    traffic_trace_replay_runtime_target_outcome, traffic_trace_replay_target_event,
    traffic_trace_replay_target_outcome, TrafficTraceReplayControlError,
    TrafficTraceReplayControlEvent, TrafficTraceReplayControlEventContext,
    TrafficTraceReplayControlRuntime, TrafficTraceReplayControllerControlError,
    TrafficTraceReplayControllerParallelErrors, TrafficTraceReplayControllerParallelExecutor,
    TrafficTraceReplayControllerParallelSubmitError, TrafficTraceReplayControllerRuntime,
    TrafficTraceReplayControllerTargetError, TrafficTraceReplayOrder,
    TrafficTraceReplayScheduledControlAck, TrafficTraceReplayScheduledControlFailure,
    TrafficTraceReplayScheduledMemoryFailure, TrafficTraceReplayScheduledMemoryWriteCompletion,
    TrafficTraceReplayScheduledSidebandEvent, TrafficTraceReplaySidebandCompletion,
    TrafficTraceReplaySidebandEvent, TrafficTraceReplaySidebandRuntime,
    TrafficTraceReplayTargetError, TrafficTraceReplayTargetEvent,
    TrafficTraceReplayTargetEventContext, TrafficTraceReplayTargetRuntime,
};
pub use traffic_replay_cpu::{
    traffic_trace_replay_runtime_data_target_outcome,
    traffic_trace_replay_runtime_fetch_target_outcome,
};
pub(crate) use trap_event::pending_trap_cores_from_turn;
pub use trap_event::{
    execution_mode_target_for_cpu as riscv_execution_mode_target_for_cpu, guest_trap_from_riscv,
    guest_trap_kind_from_riscv, RiscvTrapEventPort, ScheduledRiscvTrap, SystemEventPort,
    SystemHostEventPort,
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
pub use wait_checkpoint::{
    GuestWaitCheckpointBank, GuestWaitCheckpointError, GuestWaitCheckpointPort,
    GuestWaitCheckpointRecord,
};
pub use wait_status::{
    GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestSignal, GuestWaitOptions,
    GuestWaitOutcome, GuestWaitQueue, GuestWaitQueueSnapshot, GuestWaitSelector, GuestWaitStatus,
    GuestWaitStatusError,
};
pub use workload_gups::{run_workload_gups_plan, WorkloadGupsRunError};
pub use workload_replay::{
    RiscvWorkloadReplay, RiscvWorkloadReplayError, RiscvWorkloadReplayOutcome,
    RiscvWorkloadTraceCacheFlushRecord, RiscvWorkloadTraceHtmAbortRecord,
    RiscvWorkloadTraceHtmBeginRecord, RiscvWorkloadTraceL1InvalidationRecord,
    RiscvWorkloadTraceMemoryFailureRecord, RiscvWorkloadTraceMemoryResponseRecord,
    RiscvWorkloadTraceSidebandFailureRecord, RiscvWorkloadTraceSyncOutcome,
    RiscvWorkloadTraceSyncRecord, RiscvWorkloadTraceTlbSyncRecord, RiscvWorkloadTrafficTraceReplay,
    RiscvWorkloadTrafficTraceReplayOutcome,
};

#[derive(Clone, Debug)]
pub struct RiscvSystemRunDriver {
    trap_port: RiscvTrapEventPort,
    instruction_stats: Option<RiscvInstructionStats>,
    o3_runtime_stats: Option<RiscvO3RuntimeStats>,
    data_access_stats: Option<RiscvDataAccessStats>,
    riscv_sbi_firmware: Option<RiscvSbiFirmware>,
    riscv_syscall_emulation: Option<RiscvSyscallEmulation>,
    o3_runtime_trace_enabled: bool,
}

impl RiscvSystemRunDriver {
    pub(crate) fn reset_stats_for_run(&self, cluster: &RiscvCluster) -> Result<(), SystemError> {
        if let Some(firmware) = &self.riscv_sbi_firmware {
            firmware.register_cluster(cluster)?;
        }
        self.install_o3_runtime_stats_host_sync(cluster);
        self.reset_runtime_stats(cluster)
    }

    fn install_o3_runtime_stats_host_sync(&self, cluster: &RiscvCluster) {
        let Some(o3_runtime_stats) = self.o3_runtime_stats.clone() else {
            return;
        };
        let cluster = cluster.clone();
        let trace_enabled = self.o3_runtime_trace_enabled;
        self.trap_port
            .controller()
            .lock()
            .expect("system host controller lock")
            .executor_mut()
            .set_pre_stats_sync(move |registry, phase| {
                if phase == host::StatsSyncPhase::AfterReset {
                    Self::reset_o3_runtime_stats_for(&o3_runtime_stats, &cluster)?;
                    o3_runtime_stats.mark_host_reset_applied();
                    return Ok(());
                }
                Self::sync_o3_runtime_stats(
                    &o3_runtime_stats,
                    &cluster,
                    trace_enabled,
                    registry,
                    cluster.core_ids(),
                )
            });
    }

    pub(crate) fn schedule_riscv_system_events_from_turn<F>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        turn: &RiscvClusterTurn,
        event_for: F,
    ) -> Result<Vec<PartitionEventId>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        let scheduled = self
            .trap_port
            .schedule_riscv_system_events_from_turn(scheduler, turn, event_for)?;
        self.reset_runtime_stats_for_new_stats_resets(cluster)?;
        Ok(scheduled)
    }

    pub(crate) fn schedule_riscv_system_events_from_turn_parallel<F>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        turn: &RiscvClusterTurn,
        event_for: F,
    ) -> Result<Vec<PartitionEventId>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        let scheduled = self
            .trap_port
            .schedule_riscv_system_events_from_turn_parallel(scheduler, turn, event_for)?;
        self.reset_runtime_stats_for_new_stats_resets(cluster)?;
        Ok(scheduled)
    }

    fn reset_runtime_stats_for_new_stats_resets(
        &self,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        let saw_stats_reset = self
            .trap_port
            .controller()
            .lock()
            .expect("system host controller lock")
            .consume_stats_reset_outcomes();
        if saw_stats_reset {
            self.reset_runtime_stats(cluster)?;
        }
        Ok(())
    }

    fn reset_runtime_stats(&self, cluster: &RiscvCluster) -> Result<(), SystemError> {
        if let Some(instruction_stats) = &self.instruction_stats {
            instruction_stats.reset_retired_instruction_probes();
        }
        let reset_o3 = self
            .o3_runtime_stats
            .as_ref()
            .map_or(true, |stats| !stats.take_host_reset_applied());
        if reset_o3 {
            self.reset_o3_runtime_stats(cluster)?;
        }
        self.reset_data_access_stats_for_run(cluster)
    }

    fn reset_o3_runtime_stats(&self, cluster: &RiscvCluster) -> Result<(), SystemError> {
        if let Some(o3_runtime_stats) = &self.o3_runtime_stats {
            Self::reset_o3_runtime_stats_for(o3_runtime_stats, cluster)?;
        }
        Ok(())
    }

    fn reset_o3_runtime_stats_for(
        o3_runtime_stats: &RiscvO3RuntimeStats,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        let cycle_baselines = cluster
            .core_ids()
            .into_iter()
            .map(|cpu| {
                cluster
                    .core(cpu)
                    .map(|core| (cpu, core.in_order_pipeline_snapshot().cycle()))
                    .map_err(SystemError::RiscvCluster)
            })
            .collect::<Result<Vec<_>, _>>()?;
        o3_runtime_stats.reset_snapshots(cycle_baselines);
        for cpu in cluster.core_ids() {
            cluster
                .core(cpu)
                .map_err(SystemError::RiscvCluster)?
                .reset_o3_runtime_stats();
        }
        Ok(())
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
            .with_retired_instruction_probes(
                self.instruction_stats
                    .as_ref()
                    .map(RiscvInstructionStats::retired_instruction_probe_snapshot),
            )
            .with_data_access_probes(
                self.data_access_stats
                    .as_ref()
                    .map(RiscvDataAccessStats::data_access_probe_snapshot),
            )
            .with_riscv_debug_console_bytes(
                self.riscv_sbi_firmware
                    .as_ref()
                    .map(RiscvSbiFirmware::debug_console_bytes)
                    .unwrap_or_default(),
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
        self.reset_stats_for_run(cluster)?;

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
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.schedule_riscv_system_events_from_turn(cluster, scheduler, &turn, &mut event_for)?;
            let trap_cores = trap_event::pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events(
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
        self.reset_stats_for_run(cluster)?;

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
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.schedule_riscv_system_events_from_turn_parallel(
                cluster,
                scheduler,
                &turn,
                &mut event_for,
            )?;
            let trap_cores = trap_event::pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
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
        self.reset_stats_for_run(cluster)?;

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
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.schedule_riscv_system_events_from_turn_parallel(
                cluster,
                scheduler,
                &turn,
                &mut event_for,
            )?;
            let trap_cores = trap_event::pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
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

    pub(crate) fn record_run_stats(
        &self,
        cluster: &RiscvCluster,
        tick: Tick,
        turn: &RiscvClusterTurn,
    ) -> Result<(), SystemError> {
        self.reset_runtime_stats_for_new_stats_resets(cluster)?;
        self.record_o3_runtime_stats(cluster, turn)?;
        self.record_instruction_stats(tick, turn)?;
        self.record_data_access_stats(cluster)
    }

    pub(crate) fn record_o3_runtime_stats(
        &self,
        cluster: &RiscvCluster,
        turn: &RiscvClusterTurn,
    ) -> Result<(), SystemError> {
        let retired = turn
            .core_events()
            .iter()
            .filter_map(|event| match event.action() {
                RiscvCoreDriveAction::InstructionExecuted(instruction)
                    if instruction.counts_as_retired_instruction() =>
                {
                    Some((event.cpu(), instruction.as_ref()))
                }
                RiscvCoreDriveAction::InstructionExecuted(_) => None,
                RiscvCoreDriveAction::FetchIssued { .. }
                | RiscvCoreDriveAction::DataAccessIssued { .. } => None,
            })
            .collect::<Vec<_>>();

        let detailed_retired = {
            let controller = self.trap_port.controller();
            let controller = controller.lock().expect("system host controller lock");
            retired
                .into_iter()
                .filter(|(cpu, _instruction)| {
                    controller
                        .executor()
                        .execution_mode(&trap_event::execution_mode_target_for_cpu(*cpu))
                        .is_some_and(|mode| mode == ExecutionMode::Detailed)
                })
                .collect::<Vec<_>>()
        };

        let mut updated_cpus = BTreeSet::new();
        for (cpu, instruction) in detailed_retired {
            cluster
                .core(cpu)
                .map_err(SystemError::RiscvCluster)?
                .record_o3_retired_instruction_with_trace(
                    instruction,
                    self.o3_runtime_trace_enabled,
                );
            updated_cpus.insert(cpu);
        }
        if self.o3_runtime_trace_enabled {
            updated_cpus.extend(cluster.core_ids());
        }
        if let Some(o3_runtime_stats) = &self.o3_runtime_stats {
            let controller = self.trap_port.controller();
            let mut controller = controller.lock().expect("system host controller lock");
            Self::sync_o3_runtime_stats(
                o3_runtime_stats,
                cluster,
                self.o3_runtime_trace_enabled,
                controller.executor_mut().stats_mut(),
                updated_cpus,
            )?;
        }
        Ok(())
    }

    fn sync_o3_runtime_stats<I>(
        o3_runtime_stats: &RiscvO3RuntimeStats,
        cluster: &RiscvCluster,
        trace_enabled: bool,
        registry: &mut StatsRegistry,
        cpus: I,
    ) -> Result<(), SystemError>
    where
        I: IntoIterator<Item = CpuId>,
    {
        for cpu in cpus {
            let core = cluster.core(cpu).map_err(SystemError::RiscvCluster)?;
            let snapshot = core.o3_runtime_stats();
            let runtime_snapshot = core.o3_runtime_snapshot();
            let trace_records = if trace_enabled {
                let trace_offset = o3_runtime_stats.trace_record_offset(cpu);
                let (next_trace_offset, trace_records) =
                    core.take_o3_runtime_trace_updates(trace_offset);
                o3_runtime_stats.set_trace_record_offset(cpu, next_trace_offset);
                trace_records
            } else {
                Vec::new()
            };
            let in_order_pipeline_cycles = core.in_order_pipeline_snapshot().cycle();
            o3_runtime_stats
                .record_cpu_snapshot(
                    registry,
                    cpu,
                    snapshot,
                    &runtime_snapshot,
                    &trace_records,
                    in_order_pipeline_cycles,
                )
                .map_err(SystemError::Stats)?;
        }
        Ok(())
    }

    pub(crate) fn record_instruction_stats(
        &self,
        tick: Tick,
        turn: &RiscvClusterTurn,
    ) -> Result<(), SystemError> {
        let Some(instruction_stats) = &self.instruction_stats else {
            return Ok(());
        };

        let controller = self.trap_port.controller();
        let mut controller = controller.lock().expect("system host controller lock");
        let mut retired = turn
            .core_events()
            .iter()
            .filter_map(|event| match event.action() {
                RiscvCoreDriveAction::InstructionExecuted(instruction)
                    if instruction.counts_as_retired_instruction() =>
                {
                    Some((tick, event.cpu(), instruction.fetch_pc().get()))
                }
                RiscvCoreDriveAction::InstructionExecuted(_) => None,
                RiscvCoreDriveAction::FetchIssued { .. }
                | RiscvCoreDriveAction::DataAccessIssued { .. } => None,
            })
            .collect::<Vec<_>>();
        retired.sort_by_key(|(tick, cpu, _pc)| (*tick, *cpu));

        for (tick, cpu, pc) in retired {
            instruction_stats
                .record_retired_instruction_probe(cpu, tick, pc)
                .map_err(SystemError::Stats)?;
            if let Some(stat) = instruction_stats.committed_stat(cpu) {
                controller
                    .executor_mut()
                    .stats_mut()
                    .increment(stat, 1)
                    .map_err(SystemError::Stats)?;
            }
        }
        Ok(())
    }
}
