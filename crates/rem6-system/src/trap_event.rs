use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterTurn, RiscvCore, RiscvCoreDriveAction};
use rem6_isa_riscv::{RiscvSystemEvent, RiscvTrap, RiscvTrapKind};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler,
    SchedulerContext, SchedulerError, Tick,
};

use crate::{
    GuestEvent, GuestEventChannel, GuestEventId, GuestEventKind, GuestSourceId, GuestTrap,
    GuestTrapKind, HostEventPolicy, SystemError, SystemHostController, SystemRunController,
};

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

    fn emit_guest_event_kind(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEventId,
        kind: GuestEventKind,
    ) -> Result<PartitionEventId, SystemError> {
        self.host
            .emit(context, GuestEvent::new(event, self.source, kind))
    }

    fn emit_guest_event_kind_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEventId,
        kind: GuestEventKind,
    ) -> Result<PartitionEventId, SystemError> {
        self.host
            .emit_parallel(context, GuestEvent::new(event, self.source, kind))
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
            let Some(system_event) =
                guest_event_from_riscv_system_event(execution.execution().system_event())
            else {
                continue;
            };
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
                )?
            } else {
                self.schedule_prevalidated_system_event(
                    scheduler,
                    pending.event,
                    pending.source,
                    pending.source_tick,
                    pending.kind,
                )?
            };
            scheduled.push(event);
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
    ) -> Result<PartitionEventId, SystemError> {
        let port = self.clone();
        scheduler
            .schedule_at(source, source_tick, move |context| {
                port.emit_guest_event_kind(context, event, kind)
                    .expect("validated RISC-V system event scheduling");
            })
            .map_err(SystemError::Scheduler)
    }

    fn schedule_prevalidated_system_event_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: GuestEventId,
        source: PartitionId,
        source_tick: Tick,
        kind: GuestEventKind,
    ) -> Result<PartitionEventId, SystemError> {
        let port = self.clone();
        scheduler
            .schedule_parallel_at(source, source_tick, move |context| {
                port.emit_guest_event_kind_parallel(context, event, kind)
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvGuestEventSchedule {
    delay: Tick,
    kind: GuestEventKind,
}

fn guest_event_from_riscv_system_event(
    event: Option<&RiscvSystemEvent>,
) -> Option<RiscvGuestEventSchedule> {
    match event {
        Some(RiscvSystemEvent::Gem5Exit { delay, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            kind: GuestEventKind::Terminate { code: 0 },
        }),
        Some(RiscvSystemEvent::Gem5Fail { delay, code, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            kind: GuestEventKind::Terminate {
                code: gem5_fail_stop_code(*code),
            },
        }),
        Some(RiscvSystemEvent::Gem5ResetStats { delay, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            kind: GuestEventKind::StatsReset,
        }),
        Some(RiscvSystemEvent::Gem5DumpStats { delay, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            kind: GuestEventKind::StatsDump,
        }),
        Some(RiscvSystemEvent::Gem5DumpResetStats { delay, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            kind: GuestEventKind::StatsDumpReset,
        }),
        Some(RiscvSystemEvent::Gem5Checkpoint { delay, .. }) => Some(RiscvGuestEventSchedule {
            delay: *delay,
            kind: GuestEventKind::Checkpoint {
                label: GEM5_M5_CHECKPOINT_LABEL.to_string(),
            },
        }),
        Some(RiscvSystemEvent::Gem5WorkBegin {
            work_id, thread_id, ..
        }) => Some(RiscvGuestEventSchedule {
            delay: 0,
            kind: GuestEventKind::WorkBegin {
                work_id: *work_id,
                thread_id: *thread_id,
            },
        }),
        Some(RiscvSystemEvent::Gem5WorkEnd {
            work_id, thread_id, ..
        }) => Some(RiscvGuestEventSchedule {
            delay: 0,
            kind: GuestEventKind::WorkEnd {
                work_id: *work_id,
                thread_id: *thread_id,
            },
        }),
        Some(RiscvSystemEvent::WaitForInterrupt { .. } | RiscvSystemEvent::SfenceVma { .. })
        | None => None,
    }
}

fn gem5_fail_stop_code(code: u64) -> i32 {
    code.min(i32::MAX as u64) as i32
}

pub const fn guest_trap_from_riscv(trap: RiscvTrap) -> GuestTrap {
    GuestTrap::new(guest_trap_kind_from_riscv(trap.kind()), trap.pc())
}

pub const fn guest_trap_kind_from_riscv(kind: RiscvTrapKind) -> GuestTrapKind {
    match kind {
        RiscvTrapKind::EnvironmentCall => GuestTrapKind::EnvironmentCall,
        RiscvTrapKind::Breakpoint => GuestTrapKind::Breakpoint,
        RiscvTrapKind::IllegalInstruction => GuestTrapKind::IllegalInstruction,
        RiscvTrapKind::Interrupt { code } => GuestTrapKind::Interrupt { code },
    }
}
