use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointError, CheckpointManifest};
use rem6_cpu::{
    CpuId, RiscvCluster, RiscvClusterError, RiscvClusterSchedulerEpoch, RiscvClusterTurn,
    RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::{RiscvTrap, RiscvTrapKind};
use rem6_kernel::{
    ParallelEpochBatchRecord, ParallelRunProfile, ParallelSchedulerContext, ParallelWorkerRecord,
    PartitionEventId, PartitionFrontier, PartitionId, PartitionedScheduler, ReadyPartition,
    SchedulerContext, SchedulerDispatchRecord, SchedulerError, Tick,
};
use rem6_mmio::MmioBus;
use rem6_stats::{StatId, StatsError};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

mod fabric_checkpoint;
mod heterogeneous_checkpoint;
mod host;
mod interrupt_checkpoint;
mod memory_checkpoint;
mod riscv_checkpoint;
mod riscv_run_activity;
mod scheduler_checkpoint;
mod timer_checkpoint;
mod topology;
mod uart_checkpoint;

pub use fabric_checkpoint::{
    FabricCheckpointBank, FabricCheckpointError, FabricCheckpointPort, FabricCheckpointRecord,
};
pub use heterogeneous_checkpoint::{
    AcceleratorCheckpointBank, AcceleratorCheckpointError, AcceleratorCheckpointPort,
    AcceleratorCheckpointRecord, GpuCheckpointBank, GpuCheckpointError, GpuCheckpointPort,
    GpuCheckpointRecord,
};
pub use host::{
    SystemActionExecutor, SystemActionOutcome, SystemHostController, SystemRunController,
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
pub use riscv_checkpoint::{
    RiscvCoreCheckpointBank, RiscvCoreCheckpointError, RiscvCoreCheckpointPort,
    RiscvCoreCheckpointRecord,
};
pub use riscv_run_activity::{RiscvSystemRunCpuActivity, RiscvSystemRunPartitionActivity};
pub use scheduler_checkpoint::{
    SchedulerCheckpointBank, SchedulerCheckpointError, SchedulerCheckpointPort,
    SchedulerCheckpointRecord,
};
pub use timer_checkpoint::{
    TimerCheckpointBank, TimerCheckpointError, TimerCheckpointPort, TimerCheckpointRecord,
};
pub use topology::{
    RiscvTopologyAcceleratorComputeActivity, RiscvTopologyDmaCopy, RiscvTopologyDmaDeviceActivity,
    RiscvTopologyDmaRunSummary, RiscvTopologyDmaStageRunSummary, RiscvTopologyDramConfig,
    RiscvTopologyGpuComputeActivity, RiscvTopologyHeterogeneousRunSummary,
    RiscvTopologyHeterogeneousWork, RiscvTopologyHostConfig, RiscvTopologyMemoryConfig,
    RiscvTopologyMemoryRegion, RiscvTopologySystem, RiscvTopologySystemError,
};
pub use uart_checkpoint::{
    UartCheckpointBank, UartCheckpointError, UartCheckpointPort, UartCheckpointRecord,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestEventId(u64);

impl GuestEventId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestSourceId(u32);

impl GuestSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestEvent {
    id: GuestEventId,
    source: GuestSourceId,
    kind: GuestEventKind,
}

impl GuestEvent {
    pub const fn new(id: GuestEventId, source: GuestSourceId, kind: GuestEventKind) -> Self {
        Self { id, source, kind }
    }

    pub const fn id(&self) -> GuestEventId {
        self.id
    }

    pub const fn source(&self) -> GuestSourceId {
        self.source
    }

    pub const fn kind(&self) -> &GuestEventKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestEventKind {
    BootMilestone { name: String },
    Command { command: String },
    RoiBegin,
    RoiEnd,
    StatsReset,
    StatsDump,
    Checkpoint { label: String },
    Trap { trap: GuestTrap },
    Terminate { code: i32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestTrapKind {
    EnvironmentCall,
    Breakpoint,
}

impl GuestTrapKind {
    pub const fn default_stop_code(self) -> i32 {
        match self {
            Self::EnvironmentCall => 0,
            Self::Breakpoint => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestTrap {
    kind: GuestTrapKind,
    pc: u64,
}

impl GuestTrap {
    pub const fn new(kind: GuestTrapKind, pc: u64) -> Self {
        Self { kind, pc }
    }

    pub const fn kind(self) -> GuestTrapKind {
        self.kind
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestEventDelivery {
    tick: Tick,
    source_partition: PartitionId,
    host_partition: PartitionId,
    event: GuestEvent,
}

impl GuestEventDelivery {
    pub const fn new(
        tick: Tick,
        source_partition: PartitionId,
        host_partition: PartitionId,
        event: GuestEvent,
    ) -> Self {
        Self {
            tick,
            source_partition,
            host_partition,
            event,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn host_partition(&self) -> PartitionId {
        self.host_partition
    }

    pub const fn event(&self) -> &GuestEvent {
        &self.event
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestEventChannel {
    host_partition: PartitionId,
    host_latency: Tick,
}

impl GuestEventChannel {
    pub const fn new(host_partition: PartitionId, host_latency: Tick) -> Result<Self, SystemError> {
        if host_latency == 0 {
            return Err(SystemError::ZeroHostLatency);
        }

        Ok(Self {
            host_partition,
            host_latency,
        })
    }

    pub const fn host_partition(self) -> PartitionId {
        self.host_partition
    }

    pub const fn host_latency(self) -> Tick {
        self.host_latency
    }

    pub fn emit<F>(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
        handler: F,
    ) -> Result<PartitionEventId, SystemError>
    where
        F: FnOnce(GuestEventDelivery) + Send + 'static,
    {
        let source_partition = context.partition();
        let host_partition = self.host_partition;
        context
            .schedule_remote_after(self.host_partition, self.host_latency, move |context| {
                handler(GuestEventDelivery::new(
                    context.now(),
                    source_partition,
                    host_partition,
                    event,
                ));
            })
            .map_err(SystemError::Scheduler)
    }

    pub fn emit_parallel<F>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEvent,
        handler: F,
    ) -> Result<PartitionEventId, SystemError>
    where
        F: FnOnce(GuestEventDelivery) + Send + 'static,
    {
        let source_partition = context.partition();
        let host_partition = self.host_partition;
        context
            .schedule_remote_after(self.host_partition, self.host_latency, move |context| {
                handler(GuestEventDelivery::new(
                    context.now(),
                    source_partition,
                    host_partition,
                    event,
                ));
            })
            .map_err(SystemError::Scheduler)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAction {
    InjectCommand { command: String },
    ResetStats,
    DumpStats,
    Checkpoint { label: String },
    RestoreCheckpoint { manifest: CheckpointManifest },
    Stop { code: i32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostActionRecord {
    tick: Tick,
    source_partition: PartitionId,
    host_partition: PartitionId,
    event: GuestEventId,
    source: GuestSourceId,
    action: HostAction,
}

impl HostActionRecord {
    pub const fn new(
        tick: Tick,
        source_partition: PartitionId,
        host_partition: PartitionId,
        event: GuestEventId,
        source: GuestSourceId,
        action: HostAction,
    ) -> Self {
        Self {
            tick,
            source_partition,
            host_partition,
            event,
            source,
            action,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn host_partition(&self) -> PartitionId {
        self.host_partition
    }

    pub const fn event(&self) -> GuestEventId {
        self.event
    }

    pub const fn source(&self) -> GuestSourceId {
        self.source
    }

    pub const fn action(&self) -> &HostAction {
        &self.action
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostEventPolicy;

impl HostEventPolicy {
    pub fn actions_for(&self, event: &GuestEvent) -> Vec<HostAction> {
        match event.kind() {
            GuestEventKind::BootMilestone { .. } => Vec::new(),
            GuestEventKind::Command { command } => vec![HostAction::InjectCommand {
                command: command.clone(),
            }],
            GuestEventKind::RoiBegin | GuestEventKind::StatsReset => {
                vec![HostAction::ResetStats]
            }
            GuestEventKind::RoiEnd | GuestEventKind::StatsDump => vec![HostAction::DumpStats],
            GuestEventKind::Checkpoint { label } => vec![HostAction::Checkpoint {
                label: label.clone(),
            }],
            GuestEventKind::Trap { trap } => vec![HostAction::Stop {
                code: trap.kind().default_stop_code(),
            }],
            GuestEventKind::Terminate { code } => vec![HostAction::Stop { code: *code }],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StopRequest {
    tick: Tick,
    event: GuestEventId,
    source: GuestSourceId,
    code: i32,
}

impl StopRequest {
    pub const fn new(tick: Tick, event: GuestEventId, source: GuestSourceId, code: i32) -> Self {
        Self {
            tick,
            event,
            source,
            code,
        }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn event(self) -> GuestEventId {
        self.event
    }

    pub const fn source(self) -> GuestSourceId {
        self.source
    }

    pub const fn code(self) -> i32 {
        self.code
    }
}

#[derive(Clone, Debug)]
pub struct SystemEventPort {
    channel: GuestEventChannel,
    controller: Arc<Mutex<SystemRunController>>,
}

impl SystemEventPort {
    pub fn new(channel: GuestEventChannel, controller: Arc<Mutex<SystemRunController>>) -> Self {
        Self {
            channel,
            controller,
        }
    }

    pub fn with_controller(
        host_partition: PartitionId,
        host_latency: Tick,
        policy: HostEventPolicy,
    ) -> Result<Self, SystemError> {
        Ok(Self::new(
            GuestEventChannel::new(host_partition, host_latency)?,
            Arc::new(Mutex::new(SystemRunController::new(policy))),
        ))
    }

    pub const fn channel(&self) -> GuestEventChannel {
        self.channel
    }

    pub fn controller(&self) -> Arc<Mutex<SystemRunController>> {
        Arc::clone(&self.controller)
    }

    pub fn emit(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
    ) -> Result<PartitionEventId, SystemError> {
        let controller = Arc::clone(&self.controller);
        self.channel.emit(context, event, move |delivery| {
            controller
                .lock()
                .expect("system run controller lock")
                .handle_delivery(delivery);
        })
    }

    pub fn emit_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEvent,
    ) -> Result<PartitionEventId, SystemError> {
        let controller = Arc::clone(&self.controller);
        self.channel.emit_parallel(context, event, move |delivery| {
            controller
                .lock()
                .expect("system run controller lock")
                .handle_delivery(delivery);
        })
    }
}

#[derive(Clone, Debug)]
pub struct SystemHostEventPort {
    channel: GuestEventChannel,
    controller: Arc<Mutex<SystemHostController>>,
}

impl SystemHostEventPort {
    pub fn new(channel: GuestEventChannel, controller: Arc<Mutex<SystemHostController>>) -> Self {
        Self {
            channel,
            controller,
        }
    }

    pub fn with_controller(
        host_partition: PartitionId,
        host_latency: Tick,
        controller: Arc<Mutex<SystemHostController>>,
    ) -> Result<Self, SystemError> {
        Ok(Self::new(
            GuestEventChannel::new(host_partition, host_latency)?,
            controller,
        ))
    }

    pub const fn channel(&self) -> GuestEventChannel {
        self.channel
    }

    pub fn controller(&self) -> Arc<Mutex<SystemHostController>> {
        Arc::clone(&self.controller)
    }

    pub fn emit(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
    ) -> Result<PartitionEventId, SystemError> {
        let controller = Arc::clone(&self.controller);
        self.channel.emit(context, event, move |delivery| {
            controller
                .lock()
                .expect("system host controller lock")
                .handle_delivery(delivery);
        })
    }

    pub fn emit_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEvent,
    ) -> Result<PartitionEventId, SystemError> {
        let controller = Arc::clone(&self.controller);
        self.channel.emit_parallel(context, event, move |delivery| {
            controller
                .lock()
                .expect("system host controller lock")
                .handle_delivery(delivery);
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScheduledRiscvTrap {
    cpu: CpuId,
    event: GuestEventId,
    source_partition: PartitionId,
    scheduler_event: PartitionEventId,
    trap: GuestTrap,
}

impl ScheduledRiscvTrap {
    pub const fn new(
        cpu: CpuId,
        event: GuestEventId,
        source_partition: PartitionId,
        scheduler_event: PartitionEventId,
        trap: GuestTrap,
    ) -> Self {
        Self {
            cpu,
            event,
            source_partition,
            scheduler_event,
            trap,
        }
    }

    pub const fn cpu(self) -> CpuId {
        self.cpu
    }

    pub const fn event(self) -> GuestEventId {
        self.event
    }

    pub const fn source_partition(self) -> PartitionId {
        self.source_partition
    }

    pub const fn scheduler_event(self) -> PartitionEventId {
        self.scheduler_event
    }

    pub const fn trap(self) -> GuestTrap {
        self.trap
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSystemRunStopReason {
    HostStop(StopRequest),
    Idle { tick: Tick },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSystemRun {
    turns: Vec<RiscvClusterTurn>,
    scheduled_traps: Vec<ScheduledRiscvTrap>,
    stop_reason: RiscvSystemRunStopReason,
}

impl RiscvSystemRun {
    pub const fn new(
        turns: Vec<RiscvClusterTurn>,
        scheduled_traps: Vec<ScheduledRiscvTrap>,
        stop_reason: RiscvSystemRunStopReason,
    ) -> Self {
        Self {
            turns,
            scheduled_traps,
            stop_reason,
        }
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
            RiscvSystemRunStopReason::Idle { .. } => None,
        }
    }

    pub const fn final_tick(&self) -> Option<Tick> {
        match self.stop_reason {
            RiscvSystemRunStopReason::HostStop(stop) => Some(stop.tick()),
            RiscvSystemRunStopReason::Idle { tick } => Some(tick),
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

    pub fn parallel_scheduler_ready_partitions(&self) -> Vec<ReadyPartition> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.ready_partitions().iter().copied())
            .collect()
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
            return Ok(RiscvSystemRun::new(
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
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port.schedule_pending_core_traps(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(RiscvSystemRun::new(
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(RiscvSystemRun::new(
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
            return Ok(RiscvSystemRun::new(
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
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port.schedule_pending_core_traps_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(RiscvSystemRun::new(
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(RiscvSystemRun::new(
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
            return Ok(RiscvSystemRun::new(
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
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port.schedule_pending_core_traps_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(RiscvSystemRun::new(
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(RiscvSystemRun::new(
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

    fn host_stop_request(&self) -> Option<StopRequest> {
        self.trap_port
            .controller()
            .lock()
            .expect("system host controller lock")
            .run()
            .stop_request()
            .copied()
    }

    fn record_instruction_stats(&self, turn: &RiscvClusterTurn) -> Result<(), SystemError> {
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

fn pending_trap_cores_from_turn(
    cluster: &RiscvCluster,
    turn: &RiscvClusterTurn,
) -> Result<Vec<RiscvCore>, SystemError> {
    let mut cores = Vec::new();
    for event in turn.core_events() {
        if matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)) {
            let core = cluster
                .core(event.cpu())
                .map_err(SystemError::RiscvCluster)?;
            if core.has_pending_trap() {
                cores.push(core);
            }
        }
    }
    Ok(cores)
}

#[derive(Clone, Debug)]
pub struct RiscvTrapEventPort {
    host: SystemHostEventPort,
    source: GuestSourceId,
}

impl RiscvTrapEventPort {
    pub const fn new(host: SystemHostEventPort, source: GuestSourceId) -> Self {
        Self { host, source }
    }

    pub const fn source(&self) -> GuestSourceId {
        self.source
    }

    pub fn controller(&self) -> Arc<Mutex<SystemHostController>> {
        self.host.controller()
    }

    pub fn emit(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEventId,
        trap: RiscvTrap,
    ) -> Result<PartitionEventId, SystemError> {
        self.host.emit(
            context,
            GuestEvent::new(
                event,
                self.source,
                GuestEventKind::Trap {
                    trap: guest_trap_from_riscv(trap),
                },
            ),
        )
    }

    pub fn emit_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEventId,
        trap: RiscvTrap,
    ) -> Result<PartitionEventId, SystemError> {
        self.host.emit_parallel(
            context,
            GuestEvent::new(
                event,
                self.source,
                GuestEventKind::Trap {
                    trap: guest_trap_from_riscv(trap),
                },
            ),
        )
    }

    pub fn emit_pending_core_trap(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEventId,
        core: &RiscvCore,
    ) -> Result<Option<PartitionEventId>, SystemError> {
        let Some(trap) = core.pending_trap() else {
            return Ok(None);
        };

        self.emit(context, event, trap).map(Some)
    }

    pub fn schedule_pending_core_trap(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        core: &RiscvCore,
    ) -> Result<Option<PartitionEventId>, SystemError> {
        let Some(trap) = core.pending_trap() else {
            return Ok(None);
        };

        let source = core.partition();
        let source_tick = scheduler
            .partition_now(source)
            .map_err(SystemError::Scheduler)?;
        self.validate_scheduled_emit(scheduler, source, source_tick)?;
        self.schedule_prevalidated_trap(scheduler, event, source, source_tick, trap)
            .map(Some)
    }

    pub fn schedule_pending_core_trap_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        core: &RiscvCore,
    ) -> Result<Option<PartitionEventId>, SystemError> {
        let Some(trap) = core.pending_trap() else {
            return Ok(None);
        };

        let source = core.partition();
        let source_tick = scheduler
            .partition_now(source)
            .map_err(SystemError::Scheduler)?;
        self.validate_scheduled_emit(scheduler, source, source_tick)?;
        self.schedule_prevalidated_trap_parallel(scheduler, event, source, source_tick, trap)
            .map(Some)
    }

    pub fn schedule_pending_core_traps<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        mut event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        let mut pending = Vec::new();
        for core in cores {
            let Some(trap) = core.pending_trap() else {
                continue;
            };
            let cpu = core.id();
            let event = event_for(cpu);
            let source = core.partition();
            let source_tick = scheduler
                .partition_now(source)
                .map_err(SystemError::Scheduler)?;
            self.validate_scheduled_emit(scheduler, source, source_tick)?;
            pending.push(PendingRiscvTrapSchedule {
                cpu,
                event,
                source,
                source_tick,
                trap,
            });
        }

        let mut scheduled = Vec::new();
        for pending in pending {
            let scheduler_event = self.schedule_prevalidated_trap(
                scheduler,
                pending.event,
                pending.source,
                pending.source_tick,
                pending.trap,
            )?;
            scheduled.push(ScheduledRiscvTrap::new(
                pending.cpu,
                pending.event,
                pending.source,
                scheduler_event,
                guest_trap_from_riscv(pending.trap),
            ));
        }

        Ok(scheduled)
    }

    pub fn schedule_pending_core_traps_parallel<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        mut event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        let mut pending = Vec::new();
        for core in cores {
            let Some(trap) = core.pending_trap() else {
                continue;
            };
            let cpu = core.id();
            let event = event_for(cpu);
            let source = core.partition();
            let source_tick = scheduler
                .partition_now(source)
                .map_err(SystemError::Scheduler)?;
            self.validate_scheduled_emit(scheduler, source, source_tick)?;
            pending.push(PendingRiscvTrapSchedule {
                cpu,
                event,
                source,
                source_tick,
                trap,
            });
        }

        let mut scheduled = Vec::new();
        for pending in pending {
            let scheduler_event = self.schedule_prevalidated_trap_parallel(
                scheduler,
                pending.event,
                pending.source,
                pending.source_tick,
                pending.trap,
            )?;
            scheduled.push(ScheduledRiscvTrap::new(
                pending.cpu,
                pending.event,
                pending.source,
                scheduler_event,
                guest_trap_from_riscv(pending.trap),
            ));
        }

        Ok(scheduled)
    }

    fn schedule_prevalidated_trap(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        trap: RiscvTrap,
    ) -> Result<PartitionEventId, SystemError> {
        let port = self.clone();
        scheduler
            .schedule_at(source, source_tick, move |context| {
                port.emit(context, event, trap)
                    .expect("validated RISC-V trap event scheduling");
            })
            .map_err(SystemError::Scheduler)
    }

    fn schedule_prevalidated_trap_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        trap: RiscvTrap,
    ) -> Result<PartitionEventId, SystemError> {
        let port = self.clone();
        scheduler
            .schedule_parallel_at(source, source_tick, move |context| {
                port.emit_parallel(context, event, trap)
                    .expect("validated parallel RISC-V trap event scheduling");
            })
            .map_err(SystemError::Scheduler)
    }

    fn validate_scheduled_emit(
        &self,
        scheduler: &PartitionedScheduler,
        source: PartitionId,
        source_tick: Tick,
    ) -> Result<(), SystemError> {
        let channel = self.host.channel();
        let host = channel.host_partition();
        let latency = channel.host_latency();
        scheduler
            .partition_now(host)
            .map_err(SystemError::Scheduler)?;

        if host != source && latency < scheduler.min_remote_delay() {
            return Err(SystemError::Scheduler(
                SchedulerError::RemoteDelayBelowLookahead {
                    source,
                    target: host,
                    delay: latency,
                    minimum: scheduler.min_remote_delay(),
                },
            ));
        }

        source_tick
            .checked_add(latency)
            .ok_or(SystemError::Scheduler(SchedulerError::TickOverflow {
                now: source_tick,
                delay: latency,
            }))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRiscvTrapSchedule {
    cpu: CpuId,
    event: GuestEventId,
    source: PartitionId,
    source_tick: Tick,
    trap: RiscvTrap,
}

pub const fn guest_trap_from_riscv(trap: RiscvTrap) -> GuestTrap {
    GuestTrap::new(guest_trap_kind_from_riscv(trap.kind()), trap.pc())
}

pub const fn guest_trap_kind_from_riscv(kind: RiscvTrapKind) -> GuestTrapKind {
    match kind {
        RiscvTrapKind::EnvironmentCall => GuestTrapKind::EnvironmentCall,
        RiscvTrapKind::Breakpoint => GuestTrapKind::Breakpoint,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemError {
    ZeroHostLatency,
    Scheduler(SchedulerError),
    RiscvCluster(RiscvClusterError),
    Stats(StatsError),
    Checkpoint(CheckpointError),
    AcceleratorCheckpoint(AcceleratorCheckpointError),
    FabricCheckpoint(FabricCheckpointError),
    GpuCheckpoint(GpuCheckpointError),
    RiscvCheckpoint(RiscvCoreCheckpointError),
    SchedulerCheckpoint(SchedulerCheckpointError),
    MemoryCheckpoint(MemoryStoreCheckpointError),
    DramMemoryCheckpoint(DramMemoryCheckpointError),
    InterruptControllerCheckpoint(InterruptControllerCheckpointError),
    TimerCheckpoint(TimerCheckpointError),
    UartCheckpoint(UartCheckpointError),
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
            Self::AcceleratorCheckpoint(error) => write!(formatter, "{error}"),
            Self::FabricCheckpoint(error) => write!(formatter, "{error}"),
            Self::GpuCheckpoint(error) => write!(formatter, "{error}"),
            Self::RiscvCheckpoint(error) => write!(formatter, "{error}"),
            Self::SchedulerCheckpoint(error) => write!(formatter, "{error}"),
            Self::MemoryCheckpoint(error) => write!(formatter, "{error}"),
            Self::DramMemoryCheckpoint(error) => write!(formatter, "{error}"),
            Self::InterruptControllerCheckpoint(error) => write!(formatter, "{error}"),
            Self::TimerCheckpoint(error) => write!(formatter, "{error}"),
            Self::UartCheckpoint(error) => write!(formatter, "{error}"),
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
            Self::AcceleratorCheckpoint(error) => Some(error),
            Self::FabricCheckpoint(error) => Some(error),
            Self::GpuCheckpoint(error) => Some(error),
            Self::RiscvCheckpoint(error) => Some(error),
            Self::SchedulerCheckpoint(error) => Some(error),
            Self::MemoryCheckpoint(error) => Some(error),
            Self::DramMemoryCheckpoint(error) => Some(error),
            Self::InterruptControllerCheckpoint(error) => Some(error),
            Self::TimerCheckpoint(error) => Some(error),
            Self::UartCheckpoint(error) => Some(error),
            Self::ZeroHostLatency => None,
        }
    }
}
