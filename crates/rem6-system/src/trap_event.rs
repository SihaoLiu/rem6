mod scheduler_checkpoint_delivery;

use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterTurn, RiscvCore, RiscvCoreDriveAction};
use rem6_isa_riscv::{RiscvSystemEvent, RiscvTrap, RiscvTrapKind};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler,
    SchedulerContext, SchedulerError, Tick,
};

use crate::{
    ExecutionMode, ExecutionModeTarget, GuestEvent, GuestEventChannel, GuestEventDelivery,
    GuestEventId, GuestEventKind, GuestSourceId, GuestTrap, GuestTrapKind, HostEventPolicy,
    RiscvSbiFirmware, RiscvSbiOutcome, RiscvSyscallEmulation, RiscvSyscallOutcome,
    SchedulerCheckpointError, SystemError, SystemHostController, SystemRunController,
};

use self::scheduler_checkpoint_delivery::handle_host_delivery_with_scheduler_checkpoint;

const GEM5_M5_CHECKPOINT_LABEL: &str = "gem5-m5-checkpoint";

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
        reject_raw_scheduler_checkpoint_event(&event)?;
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
        reject_raw_scheduler_checkpoint_event(&event)?;
        let controller = Arc::clone(&self.controller);
        self.channel.emit_parallel(context, event, move |delivery| {
            controller
                .lock()
                .expect("system host controller lock")
                .handle_delivery(delivery);
        })
    }

    fn emit_with_period(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
        period: Tick,
    ) -> Result<PartitionEventId, SystemError> {
        if period == 0 {
            return self.emit(context, event);
        }

        let controller = Arc::clone(&self.controller);
        let channel = self.channel;
        let source_partition = context.partition();
        let host_partition = channel.host_partition();
        context
            .schedule_remote_after(host_partition, channel.host_latency(), move |context| {
                handle_periodic_host_delivery(
                    context,
                    source_partition,
                    host_partition,
                    event,
                    period,
                    controller,
                );
            })
            .map_err(SystemError::Scheduler)
    }

    fn emit_with_scheduler_checkpoint(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
        period: Tick,
        component: CheckpointComponentId,
    ) -> Result<PartitionEventId, SystemError> {
        let delivery_controller = Arc::clone(&self.controller);
        let registration_controller = Arc::clone(&self.controller);
        let scheduler_event = self.channel.emit_with_scheduler_checkpoint(
            context,
            event,
            move |delivery, context| {
                handle_host_delivery_with_scheduler_checkpoint(
                    context,
                    delivery,
                    period,
                    component,
                    delivery_controller,
                );
            },
        )?;
        let scheduler = context.checkpoint_access();
        let event = scheduler
            .pending_event_snapshot(scheduler_event)
            .expect("new scheduler checkpoint control delivery is pending");
        registration_controller
            .lock()
            .expect("system host controller lock")
            .executor_mut()
            .register_scheduler_checkpoint_control_event(scheduler.instance_id(), event);
        Ok(scheduler_event)
    }

    fn emit_with_period_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEvent,
        period: Tick,
    ) -> Result<PartitionEventId, SystemError> {
        if period == 0 {
            return self.emit_parallel(context, event);
        }

        let controller = Arc::clone(&self.controller);
        let channel = self.channel;
        let source_partition = context.partition();
        let host_partition = channel.host_partition();
        context
            .schedule_remote_after(host_partition, channel.host_latency(), move |context| {
                handle_periodic_host_delivery_parallel(
                    context,
                    source_partition,
                    host_partition,
                    event,
                    period,
                    controller,
                );
            })
            .map_err(SystemError::Scheduler)
    }
}

fn handle_periodic_host_delivery(
    context: &mut SchedulerContext<'_>,
    source_partition: PartitionId,
    host_partition: PartitionId,
    event: GuestEvent,
    period: Tick,
    controller: Arc<Mutex<SystemHostController>>,
) {
    controller
        .lock()
        .expect("system host controller lock")
        .handle_delivery(GuestEventDelivery::new(
            context.now(),
            source_partition,
            host_partition,
            event.clone(),
        ));

    if context.now().checked_add(period).is_none() {
        return;
    }

    let next_controller = Arc::clone(&controller);
    let _ = context.schedule_local_after(period, move |context| {
        handle_periodic_host_delivery(
            context,
            source_partition,
            host_partition,
            event,
            period,
            next_controller,
        );
    });
}

fn handle_periodic_host_delivery_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    source_partition: PartitionId,
    host_partition: PartitionId,
    event: GuestEvent,
    period: Tick,
    controller: Arc<Mutex<SystemHostController>>,
) {
    controller
        .lock()
        .expect("system host controller lock")
        .handle_delivery(GuestEventDelivery::new(
            context.now(),
            source_partition,
            host_partition,
            event.clone(),
        ));

    if context.now().checked_add(period).is_none() {
        return;
    }

    let next_controller = Arc::clone(&controller);
    let _ = context.schedule_local_after(period, move |context| {
        handle_periodic_host_delivery_parallel(
            context,
            source_partition,
            host_partition,
            event,
            period,
            next_controller,
        );
    });
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

#[derive(Clone, Debug)]
pub struct RiscvTrapEventPort {
    host: SystemHostEventPort,
    source: GuestSourceId,
    m5_switch_cpu_mode: ExecutionMode,
    scheduler_checkpoint_component: Option<CheckpointComponentId>,
}

impl RiscvTrapEventPort {
    pub const fn new(host: SystemHostEventPort, source: GuestSourceId) -> Self {
        Self {
            host,
            source,
            m5_switch_cpu_mode: ExecutionMode::Detailed,
            scheduler_checkpoint_component: None,
        }
    }

    pub const fn with_m5_switch_cpu_mode(mut self, mode: ExecutionMode) -> Self {
        self.m5_switch_cpu_mode = mode;
        self
    }

    pub fn with_scheduler_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.scheduler_checkpoint_component = Some(component);
        self
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

    fn emit_guest_event_kind_with_period(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEventId,
        kind: GuestEventKind,
        period: Tick,
    ) -> Result<PartitionEventId, SystemError> {
        let uses_scheduler_checkpoint =
            guest_event_uses_scheduler_checkpoint_context(&kind, period);
        let event = GuestEvent::new(event, self.source, kind);
        if uses_scheduler_checkpoint {
            let component = self.scheduler_checkpoint_component.clone().ok_or(
                SystemError::SchedulerCheckpoint(
                    SchedulerCheckpointError::BorrowedSchedulerContextRequired,
                ),
            )?;
            return self
                .host
                .emit_with_scheduler_checkpoint(context, event, period, component);
        }
        self.host.emit_with_period(context, event, period)
    }

    fn emit_guest_event_kind_with_period_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEventId,
        kind: GuestEventKind,
        period: Tick,
    ) -> Result<PartitionEventId, SystemError> {
        if guest_event_uses_scheduler_checkpoint_context(&kind, period) {
            return Err(SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerContextRequired,
            ));
        }
        self.host.emit_with_period_parallel(
            context,
            GuestEvent::new(event, self.source, kind),
            period,
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
        self.validate_parallel_scheduled_emit(scheduler, source, source_tick)?;
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
            if matches!(trap.kind(), RiscvTrapKind::Interrupt { .. })
                && core.complete_pending_interrupt_delivery().is_some()
            {
                continue;
            }
            if core
                .complete_pending_supervisor_exception_delivery()
                .is_some()
            {
                continue;
            }

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
            if matches!(trap.kind(), RiscvTrapKind::Interrupt { .. })
                && core.complete_pending_interrupt_delivery().is_some()
            {
                continue;
            }
            if core
                .complete_pending_supervisor_exception_delivery()
                .is_some()
            {
                continue;
            }

            let cpu = core.id();
            let event = event_for(cpu);
            let source = core.partition();
            let source_tick = scheduler
                .partition_now(source)
                .map_err(SystemError::Scheduler)?;
            self.validate_parallel_scheduled_emit(scheduler, source, source_tick)?;
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

    pub fn schedule_pending_core_traps_with_syscall_emulation<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        syscalls: &RiscvSyscallEmulation,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        self.schedule_pending_core_traps_with_riscv_emulation_and_mode(
            scheduler,
            cores,
            None,
            Some(syscalls),
            event_for,
            false,
        )
    }

    pub fn schedule_pending_core_traps_with_syscall_emulation_parallel<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        syscalls: &RiscvSyscallEmulation,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        self.schedule_pending_core_traps_with_riscv_emulation_and_mode(
            scheduler,
            cores,
            None,
            Some(syscalls),
            event_for,
            true,
        )
    }

    pub fn schedule_pending_core_traps_with_riscv_emulation<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        sbi: Option<&RiscvSbiFirmware>,
        syscalls: Option<&RiscvSyscallEmulation>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        self.schedule_pending_core_traps_with_riscv_emulation_and_mode(
            scheduler, cores, sbi, syscalls, event_for, false,
        )
    }

    pub fn schedule_pending_core_traps_with_riscv_emulation_parallel<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        sbi: Option<&RiscvSbiFirmware>,
        syscalls: Option<&RiscvSyscallEmulation>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        self.schedule_pending_core_traps_with_riscv_emulation_and_mode(
            scheduler, cores, sbi, syscalls, event_for, true,
        )
    }

    pub fn schedule_riscv_system_events_from_turn<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        turn: &RiscvClusterTurn,
        event_for: F,
    ) -> Result<Vec<PartitionEventId>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        self.schedule_riscv_system_events_from_turn_with_mode(scheduler, turn, event_for, false)
    }

    pub fn schedule_riscv_system_events_from_turn_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        turn: &RiscvClusterTurn,
        event_for: F,
    ) -> Result<Vec<PartitionEventId>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        self.schedule_riscv_system_events_from_turn_with_mode(scheduler, turn, event_for, true)
    }

    pub fn schedule_host_checkpoint_event(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        label: String,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_host_checkpoint_event_kind(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::Checkpoint { label },
        )
    }

    pub fn schedule_host_checkpoint_restore_event(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        label: String,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_host_checkpoint_event_kind(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::RestoreCheckpoint { label },
        )
    }

    pub fn schedule_host_checkpoint_event_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        label: String,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_host_checkpoint_event_kind_parallel(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::Checkpoint { label },
        )
    }

    pub fn schedule_host_checkpoint_restore_event_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        label: String,
    ) -> Result<PartitionEventId, SystemError> {
        self.schedule_host_checkpoint_event_kind_parallel(
            scheduler,
            event,
            source,
            source_tick,
            GuestEventKind::RestoreCheckpoint { label },
        )
    }

    fn schedule_riscv_system_events_from_turn_with_mode<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        turn: &RiscvClusterTurn,
        mut event_for: F,
        parallel: bool,
    ) -> Result<Vec<PartitionEventId>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        let mut pending = Vec::new();
        for event in turn.core_events() {
            let RiscvCoreDriveAction::InstructionExecuted(execution) = event.action() else {
                continue;
            };
            let Some(system_event) = guest_event_from_riscv_system_event(
                event.cpu(),
                execution.execution().system_event(),
                self.m5_switch_cpu_mode,
            ) else {
                continue;
            };
            if guest_event_uses_scheduler_checkpoint_context(
                &system_event.kind,
                system_event.period,
            ) && self.scheduler_checkpoint_component.is_none()
            {
                return Err(SystemError::SchedulerCheckpoint(
                    SchedulerCheckpointError::BorrowedSchedulerContextRequired,
                ));
            }
            let source = execution.fetch().partition();
            let source_tick = scheduler
                .partition_now(source)
                .map_err(SystemError::Scheduler)?;
            let source_tick =
                source_tick
                    .checked_add(system_event.delay)
                    .ok_or(SystemError::Scheduler(SchedulerError::TickOverflow {
                        now: source_tick,
                        delay: system_event.delay,
                    }))?;
            if parallel {
                self.validate_parallel_scheduled_emit(scheduler, source, source_tick)?;
            } else {
                self.validate_scheduled_emit(scheduler, source, source_tick)?;
            }
            pending.push(PendingRiscvSystemEventSchedule {
                event: event_for(event.cpu()),
                source,
                source_tick,
                kind: system_event.kind,
                period: system_event.period,
            });
        }

        let mut scheduled = Vec::new();
        for pending in pending {
            let event = if parallel {
                self.schedule_prevalidated_system_event_parallel(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.kind,
                    pending.period,
                )?
            } else {
                self.schedule_prevalidated_system_event(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.kind,
                    pending.period,
                )?
            };
            scheduled.push(event);
        }
        Ok(scheduled)
    }

    fn schedule_host_checkpoint_event_kind(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        kind: GuestEventKind,
    ) -> Result<PartitionEventId, SystemError> {
        self.validate_scheduled_emit(scheduler, source, source_tick)?;
        let scheduler_event = self.schedule_prevalidated_system_event(
            scheduler,
            event,
            source,
            source_tick,
            kind,
            0,
        )?;
        Ok(scheduler_event)
    }

    fn schedule_host_checkpoint_event_kind_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        kind: GuestEventKind,
    ) -> Result<PartitionEventId, SystemError> {
        self.validate_parallel_scheduled_emit(scheduler, source, source_tick)?;
        let scheduler_event = self.schedule_prevalidated_system_event_parallel(
            scheduler,
            event,
            source,
            source_tick,
            kind,
            0,
        )?;
        Ok(scheduler_event)
    }

    fn register_scheduler_checkpoint_control_event(
        &self,
        scheduler: &PartitionedScheduler,
        event: PartitionEventId,
    ) {
        if self.scheduler_checkpoint_component.is_none() {
            return;
        }
        let event = scheduler
            .pending_event_snapshot(event)
            .expect("new scheduler checkpoint control event is pending");
        self.controller()
            .lock()
            .expect("system host controller lock")
            .executor_mut()
            .register_scheduler_checkpoint_control_event(scheduler.instance_id(), event);
    }

    fn schedule_pending_core_traps_with_riscv_emulation_and_mode<I, F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: I,
        sbi: Option<&RiscvSbiFirmware>,
        syscalls: Option<&RiscvSyscallEmulation>,
        mut event_for: F,
        parallel: bool,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        I: IntoIterator<Item = RiscvCore>,
        F: FnMut(CpuId) -> GuestEventId,
    {
        let mut pending_traps = Vec::new();
        let mut pending_syscalls = Vec::new();
        for core in cores {
            let Some(trap) = core.pending_trap() else {
                continue;
            };
            if matches!(trap.kind(), RiscvTrapKind::Interrupt { .. })
                && core.complete_pending_interrupt_delivery().is_some()
            {
                continue;
            }
            if core
                .complete_pending_supervisor_exception_delivery()
                .is_some()
            {
                continue;
            }

            let cpu = core.id();
            let event = event_for(cpu);
            let source = core.partition();
            let source_tick = scheduler
                .partition_now(source)
                .map_err(SystemError::Scheduler)?;
            if parallel {
                self.validate_parallel_scheduled_emit(scheduler, source, source_tick)?;
            } else {
                self.validate_scheduled_emit(scheduler, source, source_tick)?;
            }

            if let Some(firmware) = sbi {
                if let Some(outcome) =
                    firmware.handle_pending_core_trap(scheduler, &core, parallel)?
                {
                    match outcome {
                        RiscvSbiOutcome::Return { error, value } => {
                            if core
                                .complete_pending_supervisor_environment_call(error, value)
                                .is_none()
                            {
                                pending_traps.push(PendingRiscvTrapSchedule {
                                    cpu,
                                    event,
                                    source,
                                    source_tick,
                                    trap,
                                });
                            }
                        }
                        RiscvSbiOutcome::LegacyReturn { value } => {
                            if core
                                .complete_pending_supervisor_legacy_environment_call(value)
                                .is_none()
                            {
                                pending_traps.push(PendingRiscvTrapSchedule {
                                    cpu,
                                    event,
                                    source,
                                    source_tick,
                                    trap,
                                });
                            }
                        }
                        RiscvSbiOutcome::Stopped | RiscvSbiOutcome::Resumed => {}
                        RiscvSbiOutcome::SystemReset {
                            reset_type,
                            reset_reason,
                            code,
                        } => {
                            pending_syscalls.push(PendingRiscvSystemEventSchedule {
                                event,
                                source,
                                source_tick,
                                kind: GuestEventKind::SystemReset {
                                    reset_type,
                                    reset_reason,
                                    code,
                                },
                                period: 0,
                            });
                        }
                    }
                    continue;
                }
            }

            let Some(syscalls) = syscalls else {
                pending_traps.push(PendingRiscvTrapSchedule {
                    cpu,
                    event,
                    source,
                    source_tick,
                    trap,
                });
                continue;
            };

            match syscalls.handle_pending_core_trap(&core, source_tick) {
                Some(RiscvSyscallOutcome::Blocked) => {}
                Some(RiscvSyscallOutcome::Exit { code }) => {
                    pending_syscalls.push(PendingRiscvSystemEventSchedule {
                        event,
                        source,
                        source_tick,
                        kind: GuestEventKind::Terminate { code },
                        period: 0,
                    });
                }
                Some(RiscvSyscallOutcome::Return { value }) => {
                    if core.complete_pending_user_environment_call(value).is_none() {
                        pending_traps.push(PendingRiscvTrapSchedule {
                            cpu,
                            event,
                            source,
                            source_tick,
                            trap,
                        });
                    }
                }
                None => {
                    pending_traps.push(PendingRiscvTrapSchedule {
                        cpu,
                        event,
                        source,
                        source_tick,
                        trap,
                    });
                }
            }
        }

        let mut scheduled = Vec::new();
        for pending in pending_syscalls {
            if parallel {
                self.schedule_prevalidated_system_event_parallel(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.kind,
                    pending.period,
                )?;
            } else {
                self.schedule_prevalidated_system_event(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.kind,
                    pending.period,
                )?;
            }
        }

        for pending in pending_traps {
            let scheduler_event = if parallel {
                self.schedule_prevalidated_trap_parallel(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.trap,
                )?
            } else {
                self.schedule_prevalidated_trap(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.trap,
                )?
            };
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

    fn schedule_prevalidated_system_event(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        kind: GuestEventKind,
        period: Tick,
    ) -> Result<PartitionEventId, SystemError> {
        let register_scheduler_checkpoint =
            guest_event_uses_scheduler_checkpoint_context(&kind, period);
        if register_scheduler_checkpoint && self.scheduler_checkpoint_component.is_none() {
            return Err(SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerContextRequired,
            ));
        }
        let port = self.clone();
        let scheduler_event = scheduler
            .schedule_at(source, source_tick, move |context| {
                port.emit_guest_event_kind_with_period(context, event, kind, period)
                    .expect("validated RISC-V system event scheduling");
            })
            .map_err(SystemError::Scheduler)?;
        if register_scheduler_checkpoint {
            self.register_scheduler_checkpoint_control_event(scheduler, scheduler_event);
        }
        Ok(scheduler_event)
    }

    fn schedule_prevalidated_system_event_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        kind: GuestEventKind,
        period: Tick,
    ) -> Result<PartitionEventId, SystemError> {
        if guest_event_uses_scheduler_checkpoint_context(&kind, period) {
            return self.schedule_prevalidated_system_event(
                scheduler,
                event,
                source,
                source_tick,
                kind,
                period,
            );
        }
        let port = self.clone();
        scheduler
            .schedule_parallel_at(source, source_tick, move |context| {
                port.emit_guest_event_kind_with_period_parallel(context, event, kind, period)
                    .expect("validated parallel RISC-V system event scheduling");
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

        let delivery_tick = source_tick
            .checked_add(latency)
            .ok_or(SystemError::Scheduler(SchedulerError::TickOverflow {
                now: source_tick,
                delay: latency,
            }))?;
        if host != source {
            let minimum_delivery_tick = source_tick
                .checked_add(scheduler.min_remote_delay())
                .ok_or(SystemError::Scheduler(SchedulerError::TickOverflow {
                    now: source_tick,
                    delay: scheduler.min_remote_delay(),
                }))?;
            if delivery_tick < minimum_delivery_tick {
                return Err(SystemError::Scheduler(
                    SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                        source,
                        target: host,
                        source_tick,
                        delivery_tick,
                        minimum_delivery_tick,
                    },
                ));
            }
        }
        Ok(())
    }

    fn validate_parallel_scheduled_emit(
        &self,
        scheduler: &PartitionedScheduler,
        source: PartitionId,
        source_tick: Tick,
    ) -> Result<(), SystemError> {
        self.validate_scheduled_emit(scheduler, source, source_tick)
    }
}

pub(crate) fn pending_trap_cores_from_turn(
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRiscvTrapSchedule {
    cpu: CpuId,
    event: GuestEventId,
    source: PartitionId,
    source_tick: Tick,
    trap: RiscvTrap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingRiscvSystemEventSchedule {
    event: GuestEventId,
    source: PartitionId,
    source_tick: Tick,
    kind: GuestEventKind,
    period: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvGuestEventSchedule {
    delay: Tick,
    period: Tick,
    kind: GuestEventKind,
}

fn guest_event_from_riscv_system_event(
    cpu: CpuId,
    event: Option<&RiscvSystemEvent>,
    m5_switch_cpu_mode: ExecutionMode,
) -> Option<RiscvGuestEventSchedule> {
    match event {
        Some(RiscvSystemEvent::Gem5Exit { delay, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            period: 0,
            kind: GuestEventKind::Terminate { code: 0 },
        }),
        Some(RiscvSystemEvent::Gem5Fail { delay, code, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            period: 0,
            kind: GuestEventKind::Terminate {
                code: gem5_fail_stop_code(*code),
            },
        }),
        Some(RiscvSystemEvent::Gem5ResetStats { delay, period, .. }) => {
            Some(RiscvGuestEventSchedule {
                delay: *delay,
                period: *period,
                kind: GuestEventKind::StatsReset,
            })
        }
        Some(RiscvSystemEvent::Gem5DumpStats { delay, period, .. }) => {
            Some(RiscvGuestEventSchedule {
                delay: *delay,
                period: *period,
                kind: GuestEventKind::StatsDump,
            })
        }
        Some(RiscvSystemEvent::Gem5DumpResetStats { delay, period, .. }) => {
            Some(RiscvGuestEventSchedule {
                delay: *delay,
                period: *period,
                kind: GuestEventKind::StatsDumpReset,
            })
        }
        Some(RiscvSystemEvent::Gem5Checkpoint { delay, period, .. }) => {
            Some(RiscvGuestEventSchedule {
                delay: *delay,
                period: *period,
                kind: GuestEventKind::Checkpoint {
                    label: GEM5_M5_CHECKPOINT_LABEL.to_string(),
                },
            })
        }
        Some(RiscvSystemEvent::Gem5SwitchCpu { .. }) => Some(RiscvGuestEventSchedule {
            delay: 0,
            period: 0,
            kind: GuestEventKind::ExecutionModeSwitch {
                target: execution_mode_target_for_cpu(cpu),
                mode: m5_switch_cpu_mode,
            },
        }),
        Some(RiscvSystemEvent::Gem5Hypercall {
            selector,
            arguments,
            ..
        }) => Some(RiscvGuestEventSchedule {
            delay: 0,
            period: 0,
            kind: GuestEventKind::GuestHostCall {
                selector: *selector,
                arguments: arguments.to_vec(),
                payload: Vec::new(),
            },
        }),
        Some(RiscvSystemEvent::Gem5WorkBegin {
            work_id, thread_id, ..
        }) => Some(RiscvGuestEventSchedule {
            delay: 0,
            period: 0,
            kind: GuestEventKind::WorkBegin {
                work_id: *work_id,
                thread_id: *thread_id,
            },
        }),
        Some(RiscvSystemEvent::Gem5WorkEnd {
            work_id, thread_id, ..
        }) => Some(RiscvGuestEventSchedule {
            delay: 0,
            period: 0,
            kind: GuestEventKind::WorkEnd {
                work_id: *work_id,
                thread_id: *thread_id,
            },
        }),
        Some(RiscvSystemEvent::WaitForInterrupt { .. } | RiscvSystemEvent::SfenceVma { .. })
        | None => None,
    }
}

pub fn execution_mode_target_for_cpu(cpu: CpuId) -> ExecutionModeTarget {
    ExecutionModeTarget::new(format!("cpu{}", cpu.get()))
}

fn gem5_fail_stop_code(code: u64) -> i32 {
    code.min(i32::MAX as u64) as i32
}

fn guest_event_uses_scheduler_checkpoint_context(kind: &GuestEventKind, _period: Tick) -> bool {
    guest_event_requires_scheduler_checkpoint_context(kind)
}

fn guest_event_requires_scheduler_checkpoint_context(kind: &GuestEventKind) -> bool {
    matches!(
        kind,
        GuestEventKind::Checkpoint { .. }
            | GuestEventKind::RestoreCheckpoint { .. }
            | GuestEventKind::ExecutionModeSwitch { .. }
    )
}

fn reject_raw_scheduler_checkpoint_event(event: &GuestEvent) -> Result<(), SystemError> {
    if guest_event_requires_scheduler_checkpoint_context(event.kind()) {
        Err(SystemError::SchedulerCheckpoint(
            SchedulerCheckpointError::BorrowedSchedulerContextRequired,
        ))
    } else {
        Ok(())
    }
}

pub const fn guest_trap_from_riscv(trap: RiscvTrap) -> GuestTrap {
    GuestTrap::new(guest_trap_kind_from_riscv(trap.kind()), trap.pc())
}

pub const fn guest_trap_kind_from_riscv(kind: RiscvTrapKind) -> GuestTrapKind {
    match kind {
        RiscvTrapKind::EnvironmentCall => GuestTrapKind::EnvironmentCall,
        RiscvTrapKind::Breakpoint => GuestTrapKind::Breakpoint,
        RiscvTrapKind::IllegalInstruction => GuestTrapKind::IllegalInstruction,
        RiscvTrapKind::InstructionPageFault { .. } => GuestTrapKind::InstructionPageFault,
        RiscvTrapKind::LoadPageFault { .. } => GuestTrapKind::LoadPageFault,
        RiscvTrapKind::StorePageFault { .. } => GuestTrapKind::StorePageFault,
        RiscvTrapKind::Interrupt { code } => GuestTrapKind::Interrupt { code },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rem6_kernel::ScheduledEventKind;
    use rem6_stats::StatsRegistry;

    use crate::scheduler_checkpoint::{SchedulerCheckpointBank, SchedulerCheckpointPort};
    use crate::{HostAction, HostActionRecord};

    use super::*;

    #[test]
    fn scheduler_checkpoint_context_is_limited_to_state_transfer_events() {
        assert!(guest_event_uses_scheduler_checkpoint_context(
            &GuestEventKind::Checkpoint {
                label: "capture".to_string(),
            },
            0,
        ));
        assert!(guest_event_uses_scheduler_checkpoint_context(
            &GuestEventKind::RestoreCheckpoint {
                label: "capture".to_string(),
            },
            0,
        ));
        assert!(guest_event_uses_scheduler_checkpoint_context(
            &GuestEventKind::ExecutionModeSwitch {
                target: ExecutionModeTarget::new("cpu0"),
                mode: ExecutionMode::Detailed,
            },
            0,
        ));
        assert!(!guest_event_uses_scheduler_checkpoint_context(
            &GuestEventKind::StatsDump,
            0,
        ));
        assert!(guest_event_uses_scheduler_checkpoint_context(
            &GuestEventKind::Checkpoint {
                label: "periodic".to_string(),
            },
            1,
        ));
    }

    #[test]
    fn periodic_checkpoint_preserves_registered_recurrence_with_attached_scheduler() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        let host_port =
            SystemHostEventPort::with_controller(PartitionId::new(1), 1, Arc::clone(&controller))
                .unwrap();
        let trap_port = RiscvTrapEventPort::new(host_port, GuestSourceId::new(1))
            .with_scheduler_checkpoint_component(component.clone());
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(2).unwrap()));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component,
            Arc::clone(&scheduler),
        )])
        .unwrap();
        controller
            .lock()
            .unwrap()
            .executor_mut()
            .attach_scheduler_checkpoint_bank(bank)
            .unwrap();
        let mut scheduler = scheduler.lock().unwrap();
        trap_port
            .schedule_prevalidated_system_event_parallel(
                &mut scheduler,
                GuestEventId::new(1),
                PartitionId::new(0),
                0,
                GuestEventKind::Checkpoint {
                    label: "periodic".to_string(),
                },
                100,
            )
            .unwrap();
        trap_port
            .schedule_host_checkpoint_event_parallel(
                &mut scheduler,
                GuestEventId::new(2),
                PartitionId::new(0),
                2,
                "after-periodic".to_string(),
            )
            .unwrap();

        let first = scheduler.run_next_epoch();
        let second = scheduler.run_next_epoch();
        let third = scheduler.run_next_epoch();

        assert_eq!(first.executed_events(), 2);
        assert_eq!(second.executed_events(), 1);
        assert_eq!(third.executed_events(), 1);
        assert_eq!(controller.lock().unwrap().run().action_outcomes().len(), 2);
        let pending = scheduler.snapshot();
        assert_eq!(pending.total_pending_events(), 1);
        assert_eq!(pending.partitions()[1].pending_events()[0].tick(), 101);
    }

    #[test]
    fn raw_host_checkpoint_emit_requires_component_binding() {
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        let port =
            SystemHostEventPort::with_controller(PartitionId::new(0), 1, controller).unwrap();
        let observed = Arc::new(Mutex::new(None));
        let callback_observed = Arc::clone(&observed);
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        scheduler
            .schedule_at(PartitionId::new(0), 0, move |context| {
                let error = port
                    .emit(
                        context,
                        GuestEvent::new(
                            GuestEventId::new(1),
                            GuestSourceId::new(1),
                            GuestEventKind::Checkpoint {
                                label: "raw".to_string(),
                            },
                        ),
                    )
                    .unwrap_err();
                *callback_observed.lock().unwrap() = Some(error);
            })
            .unwrap();

        scheduler.run_next_epoch();

        assert_eq!(
            observed.lock().unwrap().as_ref(),
            Some(&SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerContextRequired
            ))
        );
    }

    #[test]
    fn checkpoint_schedule_requires_component_binding() {
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        let host_port =
            SystemHostEventPort::with_controller(PartitionId::new(1), 1, controller).unwrap();
        let trap_port = RiscvTrapEventPort::new(host_port, GuestSourceId::new(1));
        let mut scheduler = PartitionedScheduler::new(2).unwrap();

        let error = trap_port
            .schedule_host_checkpoint_event(
                &mut scheduler,
                GuestEventId::new(1),
                PartitionId::new(0),
                0,
                "unbound".to_string(),
            )
            .unwrap_err();

        assert_eq!(
            error,
            SystemError::SchedulerCheckpoint(
                SchedulerCheckpointError::BorrowedSchedulerContextRequired
            )
        );
        assert!(scheduler.is_idle());
    }

    #[test]
    fn parallel_state_transfer_source_is_a_registered_serial_control_event() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        let host_port =
            SystemHostEventPort::with_controller(PartitionId::new(1), 1, Arc::clone(&controller))
                .unwrap();
        let trap_port = RiscvTrapEventPort::new(host_port, GuestSourceId::new(1))
            .with_scheduler_checkpoint_component(component.clone());
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(2).unwrap()));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        controller
            .lock()
            .unwrap()
            .executor_mut()
            .attach_scheduler_checkpoint_bank(bank)
            .unwrap();
        let mut scheduler_guard = scheduler.lock().unwrap();
        let checkpoint_source = trap_port
            .schedule_prevalidated_system_event_parallel(
                &mut scheduler_guard,
                GuestEventId::new(1),
                PartitionId::new(0),
                5,
                GuestEventKind::Checkpoint {
                    label: "guest".to_string(),
                },
                0,
            )
            .unwrap();
        let ordinary_source = trap_port
            .schedule_prevalidated_system_event_parallel(
                &mut scheduler_guard,
                GuestEventId::new(2),
                PartitionId::new(0),
                6,
                GuestEventKind::StatsDump,
                0,
            )
            .unwrap();
        assert_eq!(
            scheduler_guard
                .pending_event_snapshot(checkpoint_source)
                .unwrap()
                .kind(),
            ScheduledEventKind::Serial
        );
        assert_eq!(
            scheduler_guard
                .pending_event_snapshot(ordinary_source)
                .unwrap()
                .kind(),
            ScheduledEventKind::Parallel
        );
        scheduler_guard.cancel_event(ordinary_source).unwrap();
        drop(scheduler_guard);
        let record = HostActionRecord::new(
            0,
            PartitionId::new(1),
            PartitionId::new(1),
            GuestEventId::new(3),
            GuestSourceId::new(1),
            HostAction::Checkpoint {
                label: "project-control-source".to_string(),
            },
        );

        controller
            .lock()
            .unwrap()
            .executor_mut()
            .apply(&record)
            .unwrap();

        assert!(controller
            .lock()
            .unwrap()
            .executor()
            .checkpoints()
            .chunk(&component, "scheduler")
            .is_some());
    }

    #[test]
    fn borrowed_scheduler_restore_aborts_the_outer_serial_epoch() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            StatsRegistry::new(),
        )));
        let host_port =
            SystemHostEventPort::with_controller(PartitionId::new(0), 1, Arc::clone(&controller))
                .unwrap();
        let trap_port = RiscvTrapEventPort::new(host_port, GuestSourceId::new(1))
            .with_scheduler_checkpoint_component(component.clone());
        let scheduler = Arc::new(Mutex::new(
            PartitionedScheduler::with_min_remote_delay(2, 20).unwrap(),
        ));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component,
            Arc::clone(&scheduler),
        )])
        .unwrap();
        controller
            .lock()
            .unwrap()
            .executor_mut()
            .attach_scheduler_checkpoint_bank(bank)
            .unwrap();
        let mut scheduler_guard = scheduler.lock().unwrap();
        trap_port
            .schedule_host_checkpoint_event(
                &mut scheduler_guard,
                GuestEventId::new(1),
                PartitionId::new(0),
                2,
                "baseline".to_string(),
            )
            .unwrap();
        trap_port
            .schedule_host_checkpoint_restore_event(
                &mut scheduler_guard,
                GuestEventId::new(2),
                PartitionId::new(0),
                8,
                "baseline".to_string(),
            )
            .unwrap();

        let summary = scheduler_guard.run_next_epoch();

        assert_eq!(summary.executed_events(), 4);
        assert_eq!(summary.final_tick(), 3);
        assert_eq!(scheduler_guard.now(), 3);
        assert_eq!(controller.lock().unwrap().run().action_outcomes().len(), 2);
    }
}
